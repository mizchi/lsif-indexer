use anyhow::Result;
use lsp_types::{
    DocumentSymbol, Location, Position as LspPosition,
    ReferenceParams, TextDocumentIdentifier, TextDocumentPositionParams,
    GotoDefinitionParams, WorkDoneProgressParams, PartialResultParams,
    ReferenceContext,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use crate::core::{CodeGraph, Symbol, SymbolKind, EdgeKind, Range, Position};
use super::lsp_adapter::{LspAdapter, GenericLspClient};
use tracing::{info, debug, warn};

/// Enhanced indexer that captures reference relationships using LSP
pub struct Indexer {
    graph: CodeGraph,
    file_symbols: HashMap<String, Vec<Symbol>>,
    processed_files: HashSet<PathBuf>,
}

impl Default for Indexer {
    fn default() -> Self {
        Self::new()
    }
}

impl Indexer {
    pub fn new() -> Self {
        Self {
            graph: CodeGraph::new(),
            file_symbols: HashMap::new(),
            processed_files: HashSet::new(),
        }
    }

    /// Index a single file with full reference analysis
    pub fn index_file_with_references(
        &mut self,
        file_path: &Path,
        client: &mut GenericLspClient,
    ) -> Result<()> {
        let file_uri = format!("file://{}", file_path.canonicalize()?.display());
        
        // Get document symbols first
        info!("Indexing symbols in {}", file_path.display());
        let symbols = client.get_document_symbols(&file_uri)?;
        
        // Convert and store symbols
        let mut file_syms = Vec::new();
        self.process_symbols(&symbols, &file_uri, &mut file_syms, None);
        
        // Now analyze references for each symbol
        info!("Analyzing references for {} symbols", file_syms.len());
        for symbol in &file_syms {
            self.analyze_symbol_references(symbol, &file_uri, client)?;
        }
        
        self.file_symbols.insert(file_uri.clone(), file_syms);
        self.processed_files.insert(file_path.to_path_buf());
        
        Ok(())
    }

    /// Index an entire project
    pub fn index_project(
        &mut self,
        project_root: &Path,
        adapter: Box<dyn LspAdapter>,
    ) -> Result<()> {
        let mut client = GenericLspClient::new(adapter)?;
        
        // Find all source files
        let files = self.find_source_files(project_root)?;
        info!("Found {} source files in project", files.len());
        
        // Index each file
        for (i, file) in files.iter().enumerate() {
            info!("Processing file {}/{}: {}", i + 1, files.len(), file.display());
            if let Err(e) = self.index_file_with_references(file, &mut client) {
                warn!("Failed to index {}: {}", file.display(), e);
            }
        }
        
        // Build cross-file references
        self.build_cross_references(&mut client)?;
        
        client.shutdown()?;
        info!("Project indexing complete. Total symbols: {}", self.graph.symbol_count());
        
        Ok(())
    }

    /// Process document symbols recursively
    fn process_symbols(
        &mut self,
        symbols: &[DocumentSymbol],
        file_uri: &str,
        collected: &mut Vec<Symbol>,
        parent_idx: Option<petgraph::graph::NodeIndex>,
    ) {
        for doc_symbol in symbols {
            let symbol = self.convert_document_symbol(doc_symbol, file_uri);
            let symbol_idx = self.graph.add_symbol(symbol.clone());
            collected.push(symbol);
            
            // Add containment edge if there's a parent
            if let Some(parent) = parent_idx {
                self.graph.add_edge(symbol_idx, parent, EdgeKind::Contains);
            }
            
            // Process children
            if let Some(children) = &doc_symbol.children {
                self.process_symbols(children, file_uri, collected, Some(symbol_idx));
            }
        }
    }

    /// Analyze references for a single symbol
    fn analyze_symbol_references(
        &mut self,
        symbol: &Symbol,
        file_uri: &str,
        client: &mut GenericLspClient,
    ) -> Result<()> {
        let symbol_idx = self.graph.get_node_index(&symbol.id)
            .ok_or_else(|| anyhow::anyhow!("Symbol not found in graph"))?;
        
        // Find references to this symbol
        let references = self.find_references(symbol, file_uri, client)?;
        
        for reference_location in references {
            // Find or create symbol at reference location
            if let Some(ref_symbol) = self.find_symbol_at_location(&reference_location) {
                if let Some(ref_idx) = self.graph.get_node_index(&ref_symbol.id) {
                    // Add reference edge: ref_symbol references symbol
                    self.graph.add_edge(ref_idx, symbol_idx, EdgeKind::Reference);
                    debug!("Added reference: {} -> {}", ref_symbol.name, symbol.name);
                }
            }
        }
        
        // Find definition of this symbol
        if let Ok(definition) = self.find_definition(symbol, file_uri, client) {
            if let Some(def_symbol) = self.find_symbol_at_location(&definition) {
                if let Some(def_idx) = self.graph.get_node_index(&def_symbol.id) {
                    // Add definition edge: symbol is defined by def_symbol
                    self.graph.add_edge(symbol_idx, def_idx, EdgeKind::Definition);
                    debug!("Added definition: {} -> {}", symbol.name, def_symbol.name);
                }
            }
        }
        
        Ok(())
    }

    /// Find references using LSP
    fn find_references(
        &self,
        symbol: &Symbol,
        file_uri: &str,
        client: &mut GenericLspClient,
    ) -> Result<Vec<Location>> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: file_uri.parse()?,
                },
                position: LspPosition {
                    line: symbol.range.start.line,
                    character: symbol.range.start.character,
                },
            },
            context: ReferenceContext {
                include_declaration: false,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        client.find_references(params)
    }

    /// Find definition using LSP
    fn find_definition(
        &self,
        symbol: &Symbol,
        file_uri: &str,
        client: &mut GenericLspClient,
    ) -> Result<Location> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: file_uri.parse()?,
                },
                position: LspPosition {
                    line: symbol.range.start.line,
                    character: symbol.range.start.character,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        client.goto_definition(params)
    }

    /// Find symbol at a specific location
    fn find_symbol_at_location(&self, location: &Location) -> Option<Symbol> {
        let file_uri = location.uri.to_string();
        
        // Look through symbols in the file
        if let Some(symbols) = self.file_symbols.get(&file_uri) {
            for symbol in symbols {
                if self.location_matches_symbol(location, symbol) {
                    return Some(symbol.clone());
                }
            }
        }
        
        None
    }

    /// Check if a location matches a symbol's position
    fn location_matches_symbol(&self, location: &Location, symbol: &Symbol) -> bool {
        let loc_start = location.range.start;
        let sym_start = symbol.range.start;
        
        // Check if location is within symbol's range
        loc_start.line >= sym_start.line && 
        loc_start.line <= symbol.range.end.line
    }

    /// Build cross-file references
    fn build_cross_references(&mut self, client: &mut GenericLspClient) -> Result<()> {
        info!("Building cross-file references...");
        
        // Collect all symbols from all files
        let all_symbols: Vec<(String, Symbol)> = self.file_symbols.iter()
            .flat_map(|(uri, symbols)| {
                symbols.iter().map(|s| (uri.clone(), s.clone()))
            })
            .collect();
        
        // For each symbol, check if it references symbols in other files
        for (file_uri, symbol) in all_symbols {
            // Only process functions and methods for cross-references
            if !matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method) {
                continue;
            }
            
            // Find all references from this symbol
            if let Ok(references) = self.find_references(&symbol, &file_uri, client) {
                for reference in references {
                    let ref_uri = reference.uri.to_string();
                    
                    // Only process cross-file references
                    if ref_uri != file_uri {
                        if let Some(ref_symbol) = self.find_symbol_at_location(&reference) {
                            if let (Some(from_idx), Some(to_idx)) = (
                                self.graph.get_node_index(&symbol.id),
                                self.graph.get_node_index(&ref_symbol.id),
                            ) {
                                self.graph.add_edge(from_idx, to_idx, EdgeKind::Reference);
                                debug!("Cross-file reference: {} -> {}", symbol.name, ref_symbol.name);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Find all source files in a directory
    fn find_source_files(&self, dir: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.find_files_recursive(dir, &mut files)?;
        Ok(files)
    }

    fn find_files_recursive(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if dir.is_file() {
            if let Some(ext) = dir.extension() {
                if ext == "rs" {
                    files.push(dir.to_path_buf());
                }
            }
            return Ok(());
        }
        
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            // Skip hidden directories and target directory
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') || name_str == "target" {
                    continue;
                }
            }
            
            if path.is_dir() {
                self.find_files_recursive(&path, files)?;
            } else if let Some(ext) = path.extension() {
                if ext == "rs" {
                    files.push(path);
                }
            }
        }
        
        Ok(())
    }

    /// Convert LSP DocumentSymbol to our Symbol type
    fn convert_document_symbol(&self, doc_symbol: &DocumentSymbol, file_uri: &str) -> Symbol {
        let file_path = file_uri.strip_prefix("file://").unwrap_or(file_uri);
        
        Symbol {
            id: format!("{}#{}:{}", file_path, doc_symbol.range.start.line, doc_symbol.name),
            name: doc_symbol.name.clone(),
            kind: self.convert_symbol_kind(doc_symbol.kind),
            file_path: file_path.to_string(),
            range: Range {
                start: Position {
                    line: doc_symbol.range.start.line,
                    character: doc_symbol.range.start.character,
                },
                end: Position {
                    line: doc_symbol.range.end.line,
                    character: doc_symbol.range.end.character,
                },
            },
            documentation: doc_symbol.detail.clone(),
        }
    }

    fn convert_symbol_kind(&self, lsp_kind: lsp_types::SymbolKind) -> SymbolKind {
        match lsp_kind {
            lsp_types::SymbolKind::FUNCTION => SymbolKind::Function,
            lsp_types::SymbolKind::METHOD => SymbolKind::Method,
            lsp_types::SymbolKind::CLASS => SymbolKind::Class,
            lsp_types::SymbolKind::INTERFACE => SymbolKind::Interface,
            lsp_types::SymbolKind::ENUM => SymbolKind::Enum,
            lsp_types::SymbolKind::MODULE => SymbolKind::Module,
            lsp_types::SymbolKind::NAMESPACE => SymbolKind::Namespace,
            lsp_types::SymbolKind::PROPERTY => SymbolKind::Property,
            lsp_types::SymbolKind::FIELD => SymbolKind::Field,
            lsp_types::SymbolKind::VARIABLE => SymbolKind::Variable,
            lsp_types::SymbolKind::CONSTANT => SymbolKind::Constant,
            _ => SymbolKind::Variable,
        }
    }

    /// Get the constructed graph
    pub fn into_graph(self) -> CodeGraph {
        self.graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexer_creation() {
        let indexer = Indexer::new();
        assert_eq!(indexer.graph.symbol_count(), 0);
        assert!(indexer.file_symbols.is_empty());
        assert!(indexer.processed_files.is_empty());
    }
}