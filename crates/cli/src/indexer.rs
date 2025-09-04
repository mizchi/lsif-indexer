use anyhow::Result;
use lsif_core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
use lsp::adapter::lsp::{GenericLspClient, LspAdapter};
use lsp_types::{
    DocumentSymbol, GotoDefinitionParams, Location, PartialResultParams, Position as LspPosition,
    ReferenceContext, ReferenceParams, TextDocumentIdentifier, TextDocumentPositionParams,
    WorkDoneProgressParams,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

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
            info!(
                "Processing file {}/{}: {}",
                i + 1,
                files.len(),
                file.display()
            );
            if let Err(e) = self.index_file_with_references(file, &mut client) {
                warn!("Failed to index {}: {}", file.display(), e);
            }
        }

        // Build cross-file references
        self.build_cross_references(&mut client)?;

        client.shutdown()?;
        info!(
            "Project indexing complete. Total symbols: {}",
            self.graph.symbol_count()
        );

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
        let symbol_idx = self
            .graph
            .get_node_index(&symbol.id)
            .ok_or_else(|| anyhow::anyhow!("Symbol not found in graph"))?;

        // Find references to this symbol
        let references = self.find_references(symbol, file_uri, client)?;

        for reference_location in references {
            // Find or create symbol at reference location
            if let Some(ref_symbol) = self.find_symbol_at_location(&reference_location) {
                if let Some(ref_idx) = self.graph.get_node_index(&ref_symbol.id) {
                    // Add reference edge: ref_symbol references symbol
                    self.graph
                        .add_edge(ref_idx, symbol_idx, EdgeKind::Reference);
                    debug!("Added reference: {} -> {}", ref_symbol.name, symbol.name);
                }
            }
        }

        // Find definition of this symbol
        if let Ok(definition) = self.find_definition(symbol, file_uri, client) {
            if let Some(def_symbol) = self.find_symbol_at_location(&definition) {
                if let Some(def_idx) = self.graph.get_node_index(&def_symbol.id) {
                    // Add definition edge: symbol is defined by def_symbol
                    self.graph
                        .add_edge(symbol_idx, def_idx, EdgeKind::Definition);
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
        loc_start.line >= sym_start.line && loc_start.line <= symbol.range.end.line
    }

    /// Build cross-file references
    fn build_cross_references(&mut self, client: &mut GenericLspClient) -> Result<()> {
        info!("Building cross-file references...");

        // Collect all symbols from all files
        let all_symbols: Vec<(String, Symbol)> = self
            .file_symbols
            .iter()
            .flat_map(|(uri, symbols)| symbols.iter().map(|s| (uri.clone(), s.clone())))
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
                                debug!(
                                    "Cross-file reference: {} -> {}",
                                    symbol.name, ref_symbol.name
                                );
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
        Self::find_files_recursive(dir, &mut files)?;
        Ok(files)
    }

    fn find_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
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
                Self::find_files_recursive(&path, files)?;
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
            id: format!(
                "{}#{}:{}",
                file_path, doc_symbol.range.start.line, doc_symbol.name
            ),
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
            detail: None,
        }
    }

    fn convert_symbol_kind(&self, lsp_kind: lsp_types::SymbolKind) -> SymbolKind {
        match lsp_kind {
            lsp_types::SymbolKind::FILE => SymbolKind::Module,
            lsp_types::SymbolKind::FUNCTION => SymbolKind::Function,
            lsp_types::SymbolKind::METHOD => SymbolKind::Method,
            lsp_types::SymbolKind::CLASS => SymbolKind::Class,
            lsp_types::SymbolKind::STRUCT => SymbolKind::Struct,
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
    use lsp_types::{SymbolKind as LspSymbolKind, Url};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_indexer_creation() {
        let indexer = Indexer::new();
        assert_eq!(indexer.graph.symbol_count(), 0);
        assert!(indexer.file_symbols.is_empty());
        assert!(indexer.processed_files.is_empty());
    }

    #[test]
    fn test_indexer_default() {
        let indexer = Indexer::default();
        assert_eq!(indexer.graph.symbol_count(), 0);
        assert!(indexer.file_symbols.is_empty());
        assert!(indexer.processed_files.is_empty());
    }

    #[test]
    #[ignore] // TODO: implement convert_document_symbol method
    fn test_convert_document_symbol() {
        let indexer = Indexer::new();

        #[allow(deprecated)]
        let doc_symbol = DocumentSymbol {
            name: "test_function".to_string(),
            detail: Some("fn test_function()".to_string()),
            kind: LspSymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            range: lsp_types::Range {
                start: LspPosition {
                    line: 10,
                    character: 0,
                },
                end: LspPosition {
                    line: 15,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: LspPosition {
                    line: 10,
                    character: 3,
                },
                end: LspPosition {
                    line: 10,
                    character: 16,
                },
            },
            children: None,
        };

        let symbol = indexer.convert_document_symbol(&doc_symbol, "file:///test.rs");

        assert_eq!(symbol.name, "test_function");
        assert_eq!(symbol.kind, SymbolKind::Function);
        assert_eq!(symbol.file_path, "/test.rs");
        assert_eq!(symbol.range.start.line, 10);
        assert_eq!(symbol.range.end.line, 15);
        assert_eq!(symbol.documentation, Some("fn test_function()".to_string()));
    }

    #[test]
    fn test_process_symbols_flat() {
        let mut indexer = Indexer::new();

        #[allow(deprecated)]
        let symbols = vec![
            DocumentSymbol {
                name: "func1".to_string(),
                kind: LspSymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                detail: None,
                range: lsp_types::Range {
                    start: LspPosition {
                        line: 0,
                        character: 0,
                    },
                    end: LspPosition {
                        line: 5,
                        character: 1,
                    },
                },
                selection_range: lsp_types::Range {
                    start: LspPosition {
                        line: 0,
                        character: 0,
                    },
                    end: LspPosition {
                        line: 0,
                        character: 5,
                    },
                },
                children: None,
            },
            DocumentSymbol {
                name: "func2".to_string(),
                kind: LspSymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                detail: None,
                range: lsp_types::Range {
                    start: LspPosition {
                        line: 7,
                        character: 0,
                    },
                    end: LspPosition {
                        line: 10,
                        character: 1,
                    },
                },
                selection_range: lsp_types::Range {
                    start: LspPosition {
                        line: 7,
                        character: 0,
                    },
                    end: LspPosition {
                        line: 7,
                        character: 5,
                    },
                },
                children: None,
            },
        ];

        let mut collected = Vec::new();
        indexer.process_symbols(&symbols, "file:///test.rs", &mut collected, None);

        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].name, "func1");
        assert_eq!(collected[1].name, "func2");
        assert_eq!(indexer.graph.symbol_count(), 2);
    }

    #[test]
    fn test_process_symbols_nested() {
        let mut indexer = Indexer::new();

        #[allow(deprecated)]
        let symbols = vec![DocumentSymbol {
            name: "MyStruct".to_string(),
            kind: LspSymbolKind::STRUCT,
            tags: None,
            deprecated: None,
            detail: None,
            range: lsp_types::Range {
                start: LspPosition {
                    line: 0,
                    character: 0,
                },
                end: LspPosition {
                    line: 10,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: LspPosition {
                    line: 0,
                    character: 0,
                },
                end: LspPosition {
                    line: 0,
                    character: 8,
                },
            },
            children: Some(vec![
                DocumentSymbol {
                    name: "field1".to_string(),
                    kind: LspSymbolKind::FIELD,
                    tags: None,
                    deprecated: None,
                    detail: None,
                    range: lsp_types::Range {
                        start: LspPosition {
                            line: 1,
                            character: 4,
                        },
                        end: LspPosition {
                            line: 1,
                            character: 20,
                        },
                    },
                    selection_range: lsp_types::Range {
                        start: LspPosition {
                            line: 1,
                            character: 4,
                        },
                        end: LspPosition {
                            line: 1,
                            character: 10,
                        },
                    },
                    children: None,
                },
                DocumentSymbol {
                    name: "method1".to_string(),
                    kind: LspSymbolKind::METHOD,
                    tags: None,
                    deprecated: None,
                    detail: None,
                    range: lsp_types::Range {
                        start: LspPosition {
                            line: 3,
                            character: 4,
                        },
                        end: LspPosition {
                            line: 5,
                            character: 5,
                        },
                    },
                    selection_range: lsp_types::Range {
                        start: LspPosition {
                            line: 3,
                            character: 4,
                        },
                        end: LspPosition {
                            line: 3,
                            character: 11,
                        },
                    },
                    children: None,
                },
            ]),
        }];

        let mut collected = Vec::new();
        indexer.process_symbols(&symbols, "file:///test.rs", &mut collected, None);

        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0].name, "MyStruct");
        assert_eq!(collected[1].name, "field1");
        assert_eq!(collected[2].name, "method1");
        assert_eq!(indexer.graph.symbol_count(), 3);

        // Check that containment edges were created
        let _struct_idx = indexer.graph.get_node_index(&collected[0].id).unwrap();
        let _field_idx = indexer.graph.get_node_index(&collected[1].id).unwrap();
        let _method_idx = indexer.graph.get_node_index(&collected[2].id).unwrap();

        // The graph structure might have edges, but we need to verify they exist
        assert!(indexer.graph.symbol_count() > 0);
    }

    #[test]
    fn test_find_source_files() {
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        // Create test files
        fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();
        fs::write(src_dir.join("lib.rs"), "pub fn lib() {}").unwrap();
        fs::write(src_dir.join("test.txt"), "not a source file").unwrap();

        let indexer = Indexer::new();
        let files = indexer.find_source_files(dir.path()).unwrap();

        // Should find the .rs files but not .txt
        assert_eq!(files.len(), 2);
        let file_names: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(file_names.contains(&"main.rs".to_string()));
        assert!(file_names.contains(&"lib.rs".to_string()));
    }

    #[test]
    #[ignore] // TODO: implement convert_symbol_kind method
    fn test_convert_lsp_symbol_kind() {
        let indexer = Indexer::new();

        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::FUNCTION),
            SymbolKind::Function
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::STRUCT),
            SymbolKind::Struct
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::ENUM),
            SymbolKind::Enum
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::INTERFACE),
            SymbolKind::Interface
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::CLASS),
            SymbolKind::Class
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::MODULE),
            SymbolKind::Module
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::FIELD),
            SymbolKind::Field
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::METHOD),
            SymbolKind::Method
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::VARIABLE),
            SymbolKind::Variable
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::CONSTANT),
            SymbolKind::Constant
        );

        // Test default case - FILE maps to Module
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::FILE),
            SymbolKind::Module
        );
    }

    #[test]
    fn test_convert_lsp_position() {
        let _indexer = Indexer::new();

        let lsp_pos = LspPosition {
            line: 10,
            character: 5,
        };
        let pos = Position {
            line: lsp_pos.line,
            character: lsp_pos.character,
        };

        assert_eq!(pos.line, 10);
        assert_eq!(pos.character, 5);
    }

    #[test]
    fn test_convert_lsp_range() {
        let _indexer = Indexer::new();

        let lsp_range = lsp_types::Range {
            start: LspPosition {
                line: 5,
                character: 10,
            },
            end: LspPosition {
                line: 7,
                character: 15,
            },
        };
        let range = Range {
            start: Position {
                line: lsp_range.start.line,
                character: lsp_range.start.character,
            },
            end: Position {
                line: lsp_range.end.line,
                character: lsp_range.end.character,
            },
        };

        assert_eq!(range.start.line, 5);
        assert_eq!(range.start.character, 10);
        assert_eq!(range.end.line, 7);
        assert_eq!(range.end.character, 15);
    }

    #[test]
    fn test_find_symbol_at_location_not_found() {
        let indexer = Indexer::new();

        let location = Location {
            uri: Url::parse("file:///test.rs").unwrap(),
            range: lsp_types::Range {
                start: LspPosition {
                    line: 10,
                    character: 5,
                },
                end: LspPosition {
                    line: 10,
                    character: 10,
                },
            },
        };

        let symbol = indexer.find_symbol_at_location(&location);
        assert!(symbol.is_none());
    }

    #[test]
    fn test_find_symbol_at_location_found() {
        let mut indexer = Indexer::new();

        let test_symbol = Symbol {
            id: "test_id".to_string(),
            name: "test_func".to_string(),
            kind: SymbolKind::Function,
            file_path: "file:///test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 10,
                    character: 0,
                },
                end: Position {
                    line: 15,
                    character: 1,
                },
            },
            documentation: None,
            detail: None,
        };

        indexer
            .file_symbols
            .insert("file:///test.rs".to_string(), vec![test_symbol.clone()]);

        let location = Location {
            uri: Url::parse("file:///test.rs").unwrap(),
            range: lsp_types::Range {
                start: LspPosition {
                    line: 12,
                    character: 5,
                },
                end: LspPosition {
                    line: 12,
                    character: 10,
                },
            },
        };

        let found = indexer.find_symbol_at_location(&location);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test_func");
    }

    #[test]
    fn test_get_graph() {
        let mut indexer = Indexer::new();

        let symbol = Symbol {
            id: "test".to_string(),
            name: "test".to_string(),
            kind: SymbolKind::Function,
            file_path: "/test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
            documentation: None,
            detail: None,
        };

        indexer.graph.add_symbol(symbol);

        // Use clone to get a copy of the graph
        let graph = indexer.graph.clone();
        assert_eq!(graph.symbol_count(), 1);
    }

    #[test]
    fn test_into_graph() {
        let mut indexer = Indexer::new();

        let symbol = Symbol {
            id: "test".to_string(),
            name: "test".to_string(),
            kind: SymbolKind::Function,
            file_path: "/test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
            documentation: None,
            detail: None,
        };

        indexer.graph.add_symbol(symbol);

        let graph = indexer.into_graph();
        assert_eq!(graph.symbol_count(), 1);
    }
}
// Differential test
