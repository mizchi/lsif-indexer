use anyhow::Result;
use lsp_types::DocumentSymbol;
use crate::core::{CodeGraph, Symbol, SymbolKind, Range, Position};
use tracing::debug;

pub struct LspIndexer {
    graph: CodeGraph,
    file_path: String,
}

impl LspIndexer {
    pub fn new(file_path: String) -> Self {
        Self {
            graph: CodeGraph::new(),
            file_path,
        }
    }

    pub fn index_from_symbols(&mut self, symbols: Vec<DocumentSymbol>) -> Result<()> {
        for symbol in symbols {
            self.process_symbol(&symbol, None)?;
        }
        Ok(())
    }

    fn process_symbol(&mut self, symbol: &DocumentSymbol, parent_id: Option<String>) -> Result<()> {
        // Create symbol ID
        let symbol_id = format!("{}#{}:{}", 
            self.file_path, 
            symbol.range.start.line,
            symbol.name
        );

        // Convert LSP symbol to our Symbol type
        let our_symbol = Symbol {
            id: symbol_id.clone(),
            kind: self.convert_symbol_kind(symbol.kind),
            name: symbol.name.clone(),
            file_path: self.file_path.clone(),
            range: Range {
                start: Position {
                    line: symbol.range.start.line,
                    character: symbol.range.start.character,
                },
                end: Position {
                    line: symbol.range.end.line,
                    character: symbol.range.end.character,
                },
            },
            documentation: symbol.detail.clone(),
        };

        let _node_index = self.graph.add_symbol(our_symbol);
        debug!("Added symbol: {} ({})", symbol.name, symbol_id);

        // If there's a parent, create a containment relationship
        if let Some(parent_id) = parent_id {
            if let Some(_parent_symbol) = self.graph.find_symbol(&parent_id) {
                // Note: We need to get the parent's node index
                // This requires adding a method to get node index from symbol ID
                // For now, we'll skip edge creation
                debug!("Would create edge from {} to {}", parent_id, symbol_id);
            }
        }

        // Process children recursively
        if let Some(children) = &symbol.children {
            for child in children {
                self.process_symbol(child, Some(symbol_id.clone()))?;
            }
        }

        Ok(())
    }

    fn convert_symbol_kind(&self, lsp_kind: lsp_types::SymbolKind) -> SymbolKind {
        use lsp_types::SymbolKind as LspKind;
        
        match lsp_kind {
            LspKind::FUNCTION => SymbolKind::Function,
            LspKind::METHOD => SymbolKind::Method,
            LspKind::CLASS => SymbolKind::Class,
            LspKind::INTERFACE => SymbolKind::Interface,
            LspKind::MODULE => SymbolKind::Module,
            LspKind::NAMESPACE => SymbolKind::Namespace,
            LspKind::ENUM => SymbolKind::Enum,
            LspKind::STRUCT => SymbolKind::Class,
            LspKind::VARIABLE => SymbolKind::Variable,
            LspKind::CONSTANT => SymbolKind::Constant,
            LspKind::PROPERTY => SymbolKind::Property,
            LspKind::FIELD => SymbolKind::Property,
            LspKind::ENUM_MEMBER => SymbolKind::Constant,
            _ => SymbolKind::Variable,
        }
    }


    pub fn into_graph(self) -> CodeGraph {
        self.graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use anyhow::Context;

    #[test]
    fn test_index_from_lsp_symbols() -> Result<()> {
        // Load the test data
        let json_str = fs::read_to_string("lsp_symbols.json")
            .context("Failed to read lsp_symbols.json")?;
        
        let symbols: Vec<DocumentSymbol> = serde_json::from_str(&json_str)
            .context("Failed to parse symbols")?;
        
        // Create indexer and process symbols
        let mut indexer = LspIndexer::new("src/main.rs".to_string());
        indexer.index_from_symbols(symbols)?;
        
        // Verify we have symbols in the graph
        let graph = indexer.into_graph();
        assert!(graph.symbol_count() > 0);
        
        // Try to find a specific symbol
        let main_symbol = graph.find_symbol("src/main.rs#66:main");
        assert!(main_symbol.is_some());
        assert_eq!(main_symbol.unwrap().name, "main");
        
        Ok(())
    }

}