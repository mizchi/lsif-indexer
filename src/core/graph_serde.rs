use serde::{Deserialize, Serialize, Serializer, Deserializer};
use std::collections::HashMap;
use petgraph::graph::NodeIndex;
use super::graph::{CodeGraph, Symbol, EdgeKind};

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
                if let (Some(from_symbol), Some(to_symbol)) = (
                    self.graph.node_weight(from),
                    self.graph.node_weight(to)
                ) {
                    if let Some(edge_kind) = self.graph.edge_weight(edge) {
                        edges.push(SerializedEdge {
                            from_id: from_symbol.id.clone(),
                            to_id: to_symbol.id.clone(),
                            kind: edge_kind.clone(),
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
            if let (Some(&from_idx), Some(&to_idx)) = (
                id_to_node.get(&edge.from_id),
                id_to_node.get(&edge.to_id)
            ) {
                graph.add_edge(from_idx, to_idx, edge.kind);
            }
        }
        
        Ok(graph)
    }
}