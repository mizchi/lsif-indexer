use anyhow::Result;
use lsif_core::{CodeGraph, Position, Range, Symbol, SymbolKind};
use lsp_types::DocumentSymbol;
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
        let symbol_id = format!(
            "{}#{}:{}",
            self.file_path, symbol.range.start.line, symbol.name
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
            detail: None,
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

    #[test]
    fn test_index_from_lsp_symbols() -> Result<()> {
        use lsp_types::{Position as LspPosition, Range as LspRange};

        // Create mock symbols
        #[allow(deprecated)]
        let symbols = vec![
            DocumentSymbol {
                name: "main".to_string(),
                detail: Some("fn main()".to_string()),
                kind: lsp_types::SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                range: LspRange {
                    start: LspPosition {
                        line: 65,
                        character: 0,
                    },
                    end: LspPosition {
                        line: 70,
                        character: 1,
                    },
                },
                selection_range: LspRange {
                    start: LspPosition {
                        line: 65,
                        character: 3,
                    },
                    end: LspPosition {
                        line: 65,
                        character: 7,
                    },
                },
                children: None,
            },
            DocumentSymbol {
                name: "TestStruct".to_string(),
                detail: Some("struct TestStruct".to_string()),
                kind: lsp_types::SymbolKind::STRUCT,
                tags: None,
                deprecated: None,
                range: LspRange {
                    start: LspPosition {
                        line: 10,
                        character: 0,
                    },
                    end: LspPosition {
                        line: 15,
                        character: 1,
                    },
                },
                selection_range: LspRange {
                    start: LspPosition {
                        line: 10,
                        character: 7,
                    },
                    end: LspPosition {
                        line: 10,
                        character: 17,
                    },
                },
                children: Some(vec![DocumentSymbol {
                    name: "field1".to_string(),
                    detail: Some("String".to_string()),
                    kind: lsp_types::SymbolKind::FIELD,
                    tags: None,
                    deprecated: None,
                    range: LspRange {
                        start: LspPosition {
                            line: 11,
                            character: 4,
                        },
                        end: LspPosition {
                            line: 11,
                            character: 20,
                        },
                    },
                    selection_range: LspRange {
                        start: LspPosition {
                            line: 11,
                            character: 4,
                        },
                        end: LspPosition {
                            line: 11,
                            character: 10,
                        },
                    },
                    children: None,
                }]),
            },
        ];

        // Create indexer and process symbols
        let mut indexer = LspIndexer::new("src/main.rs".to_string());
        indexer.index_from_symbols(symbols)?;

        // Verify we have symbols in the graph
        let graph = indexer.into_graph();
        assert_eq!(graph.symbol_count(), 3); // main, TestStruct, field1

        // Try to find specific symbols
        let all_symbols: Vec<_> = graph.get_all_symbols().collect();
        assert_eq!(all_symbols.len(), 3);

        // Check that main symbol exists
        let main_exists = all_symbols.iter().any(|s| s.name == "main");
        assert!(main_exists, "main symbol should exist");

        // Check that TestStruct exists
        let struct_exists = all_symbols.iter().any(|s| s.name == "TestStruct");
        assert!(struct_exists, "TestStruct symbol should exist");

        Ok(())
    }
}
