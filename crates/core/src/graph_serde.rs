use super::graph::{CodeGraph, EdgeKind, Symbol};
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
struct SerializedCodeGraph {
    symbols: Vec<Symbol>,
    edges: Vec<SerializedEdge>,
}

#[derive(Serialize, Deserialize)]
struct SerializedEdge {
    from_id: String,
    to_id: String,
    kind: EdgeKind,
}

impl Serialize for CodeGraph {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut symbols = Vec::new();
        let mut edges = Vec::new();

        // Collect all symbols
        for id in self.symbol_index.keys() {
            if let Some(symbol) = self.find_symbol(id) {
                symbols.push(symbol.clone());
            }
        }

        // Collect all edges
        for edge in self.graph.edge_indices() {
            if let Some((from, to)) = self.graph.edge_endpoints(edge) {
                if let (Some(from_symbol), Some(to_symbol)) =
                    (self.graph.node_weight(from), self.graph.node_weight(to))
                {
                    if let Some(edge_kind) = self.graph.edge_weight(edge) {
                        edges.push(SerializedEdge {
                            from_id: from_symbol.id.clone(),
                            to_id: to_symbol.id.clone(),
                            kind: *edge_kind,
                        });
                    }
                }
            }
        }

        let serialized = SerializedCodeGraph { symbols, edges };
        serialized.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CodeGraph {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let serialized = SerializedCodeGraph::deserialize(deserializer)?;
        let mut graph = CodeGraph::new();

        // First, add all symbols to establish node indices
        let mut id_to_node: HashMap<String, NodeIndex> = HashMap::new();
        for symbol in serialized.symbols {
            let node_idx = graph.add_symbol(symbol.clone());
            id_to_node.insert(symbol.id, node_idx);
        }

        // Then, add all edges
        for edge in serialized.edges {
            if let (Some(&from_idx), Some(&to_idx)) =
                (id_to_node.get(&edge.from_id), id_to_node.get(&edge.to_id))
            {
                graph.add_edge(from_idx, to_idx, edge.kind);
            }
        }

        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range, SymbolKind};

    fn create_test_symbol(id: &str, name: &str) -> Symbol {
        Symbol {
            id: id.to_string(),
            kind: SymbolKind::Function,
            name: name.to_string(),
            file_path: "test.rs".to_string(),
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
        }
    }

    #[test]
    fn test_serialize_deserialize_empty_graph() {
        let graph = CodeGraph::new();
        let serialized = serde_json::to_string(&graph).unwrap();
        let deserialized: CodeGraph = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.symbol_count(), 0);
    }

    #[test]
    fn test_serialize_deserialize_with_symbols() {
        let mut graph = CodeGraph::new();
        let symbol1 = create_test_symbol("sym1", "Symbol1");
        let symbol2 = create_test_symbol("sym2", "Symbol2");

        graph.add_symbol(symbol1);
        graph.add_symbol(symbol2);

        let serialized = serde_json::to_string(&graph).unwrap();
        let deserialized: CodeGraph = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.symbol_count(), 2);
        assert!(deserialized.find_symbol("sym1").is_some());
        assert!(deserialized.find_symbol("sym2").is_some());
    }

    #[test]
    fn test_serialize_deserialize_with_edges() {
        let mut graph = CodeGraph::new();
        let symbol1 = create_test_symbol("func1", "Function1");
        let symbol2 = create_test_symbol("ref1", "Reference1");

        let idx1 = graph.add_symbol(symbol1);
        let idx2 = graph.add_symbol(symbol2);
        graph.add_edge(idx2, idx1, EdgeKind::Reference);

        let serialized = serde_json::to_string(&graph).unwrap();
        let deserialized: CodeGraph = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.symbol_count(), 2);

        // Check that the reference relationship is preserved
        let references = deserialized.find_references("func1").unwrap();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].id, "ref1");
    }

    #[test]
    fn test_serialize_deserialize_complex_graph() {
        let mut graph = CodeGraph::new();

        // Create a complex graph with multiple symbols and edges
        let interface = Symbol {
            id: "interface1".to_string(),
            kind: SymbolKind::Interface,
            name: "Interface1".to_string(),
            file_path: "interface.rs".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 0,
                },
            },
            documentation: Some("Interface documentation".to_string()),
            detail: None,
        };

        let class = Symbol {
            id: "class1".to_string(),
            kind: SymbolKind::Class,
            name: "Class1".to_string(),
            file_path: "class.rs".to_string(),
            range: Range {
                start: Position {
                    line: 10,
                    character: 0,
                },
                end: Position {
                    line: 20,
                    character: 0,
                },
            },
            documentation: Some("Class documentation".to_string()),
            detail: None,
        };

        let method = Symbol {
            id: "method1".to_string(),
            kind: SymbolKind::Method,
            name: "method1".to_string(),
            file_path: "class.rs".to_string(),
            range: Range {
                start: Position {
                    line: 12,
                    character: 4,
                },
                end: Position {
                    line: 15,
                    character: 4,
                },
            },
            documentation: None,
            detail: None,
        };

        let interface_idx = graph.add_symbol(interface);
        let class_idx = graph.add_symbol(class);
        let method_idx = graph.add_symbol(method);

        graph.add_edge(class_idx, interface_idx, EdgeKind::Implementation);
        graph.add_edge(method_idx, class_idx, EdgeKind::Contains);

        let serialized = serde_json::to_string(&graph).unwrap();
        let deserialized: CodeGraph = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.symbol_count(), 3);

        // Check that all symbols are preserved
        let interface_symbol = deserialized.find_symbol("interface1").unwrap();
        assert_eq!(interface_symbol.name, "Interface1");
        assert_eq!(
            interface_symbol.documentation,
            Some("Interface documentation".to_string())
        );

        let class_symbol = deserialized.find_symbol("class1").unwrap();
        assert_eq!(class_symbol.name, "Class1");

        let method_symbol = deserialized.find_symbol("method1").unwrap();
        assert_eq!(method_symbol.name, "method1");

        // Check that relationships are preserved
        let implementations = deserialized.find_implementations("interface1");
        assert_eq!(implementations.len(), 1);
        assert_eq!(implementations[0].id, "class1");
    }

    #[test]
    fn test_serialized_edge_structure() {
        let edge = SerializedEdge {
            from_id: "from".to_string(),
            to_id: "to".to_string(),
            kind: EdgeKind::Definition,
        };

        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"from_id\":\"from\""));
        assert!(json.contains("\"to_id\":\"to\""));
        assert!(json.contains("\"Definition\""));
    }

    #[test]
    fn test_multiple_edge_types() {
        let mut graph = CodeGraph::new();
        let symbol1 = create_test_symbol("sym1", "Symbol1");
        let symbol2 = create_test_symbol("sym2", "Symbol2");
        let symbol3 = create_test_symbol("sym3", "Symbol3");

        let idx1 = graph.add_symbol(symbol1);
        let idx2 = graph.add_symbol(symbol2);
        let idx3 = graph.add_symbol(symbol3);

        graph.add_edge(idx1, idx2, EdgeKind::Reference);
        graph.add_edge(idx2, idx3, EdgeKind::Definition);
        graph.add_edge(idx3, idx1, EdgeKind::Implementation);

        let serialized = serde_json::to_string(&graph).unwrap();
        let deserialized: CodeGraph = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.symbol_count(), 3);
        // Verify all symbols and edges are preserved
        assert!(deserialized.find_symbol("sym1").is_some());
        assert!(deserialized.find_symbol("sym2").is_some());
        assert!(deserialized.find_symbol("sym3").is_some());
    }
}
