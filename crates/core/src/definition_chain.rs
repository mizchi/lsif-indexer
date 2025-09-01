use super::graph::{CodeGraph, EdgeKind, Symbol};
use petgraph::visit::EdgeRef;
use std::collections::{HashSet, VecDeque};

/// Represents a chain of definitions from a symbol to its ultimate source
#[derive(Debug, Clone)]
pub struct DefinitionChain {
    pub chain: Vec<Symbol>,
    pub has_cycle: bool,
}

/// Analyzer for tracing definition chains
pub struct DefinitionChainAnalyzer<'a> {
    graph: &'a CodeGraph,
}

impl<'a> DefinitionChainAnalyzer<'a> {
    pub fn new(graph: &'a CodeGraph) -> Self {
        Self { graph }
    }

    /// Get the complete definition chain for a symbol
    /// Returns all definitions in order from the symbol to the ultimate source
    pub fn get_definition_chain(&self, symbol_id: &str) -> Option<DefinitionChain> {
        let mut chain = Vec::new();
        let mut visited = HashSet::new();
        let mut current = symbol_id.to_string();
        let mut has_cycle = false;

        // Start with the initial symbol
        if let Some(symbol) = self.graph.find_symbol(&current) {
            chain.push(symbol.clone());
            visited.insert(current.clone());
        } else {
            return None;
        }

        // Follow the definition chain
        while let Some(definition) = self.get_immediate_definition(&current) {
            if visited.contains(&definition.id) {
                // Cycle detected
                has_cycle = true;
                break;
            }

            chain.push(definition.clone());
            visited.insert(definition.id.clone());
            current = definition.id.clone();
        }

        Some(DefinitionChain { chain, has_cycle })
    }

    /// Get all definition chains (handles multiple definitions)
    pub fn get_all_definition_chains(&self, symbol_id: &str) -> Vec<DefinitionChain> {
        let mut all_chains = Vec::new();
        let mut visited_paths = HashSet::new();

        self.trace_all_chains(
            symbol_id,
            Vec::new(),
            &mut visited_paths,
            &mut all_chains,
            HashSet::new(),
        );

        all_chains
    }

    /// Recursively trace all possible definition chains
    fn trace_all_chains(
        &self,
        symbol_id: &str,
        mut current_chain: Vec<Symbol>,
        visited_paths: &mut HashSet<Vec<String>>,
        all_chains: &mut Vec<DefinitionChain>,
        mut visited_in_chain: HashSet<String>,
    ) {
        // Check for cycles
        if visited_in_chain.contains(symbol_id) {
            // Add the chain with cycle marker
            if !current_chain.is_empty() {
                let chain_ids: Vec<String> = current_chain.iter().map(|s| s.id.clone()).collect();
                if !visited_paths.contains(&chain_ids) {
                    visited_paths.insert(chain_ids);
                    all_chains.push(DefinitionChain {
                        chain: current_chain,
                        has_cycle: true,
                    });
                }
            }
            return;
        }

        // Add current symbol to chain
        if let Some(symbol) = self.graph.find_symbol(symbol_id) {
            current_chain.push(symbol.clone());
            visited_in_chain.insert(symbol_id.to_string());
        } else {
            return;
        }

        // Get all definitions for this symbol
        let definitions = self.get_all_immediate_definitions(symbol_id);

        if definitions.is_empty() {
            // This is a terminal node (no more definitions)
            let chain_ids: Vec<String> = current_chain.iter().map(|s| s.id.clone()).collect();
            if !visited_paths.contains(&chain_ids) {
                visited_paths.insert(chain_ids);
                all_chains.push(DefinitionChain {
                    chain: current_chain,
                    has_cycle: false,
                });
            }
        } else {
            // Recursively explore each definition
            for def in definitions {
                self.trace_all_chains(
                    &def.id,
                    current_chain.clone(),
                    visited_paths,
                    all_chains,
                    visited_in_chain.clone(),
                );
            }
        }
    }

    /// Get the immediate definition of a symbol
    fn get_immediate_definition(&self, symbol_id: &str) -> Option<Symbol> {
        if let Some(node_idx) = self.graph.get_node_index(symbol_id) {
            for edge in self.graph.graph.edges(node_idx) {
                if matches!(edge.weight(), EdgeKind::Definition) {
                    return self.graph.graph.node_weight(edge.target()).cloned();
                }
            }
        }
        None
    }

    /// Get all immediate definitions of a symbol (for cases with multiple definitions)
    fn get_all_immediate_definitions(&self, symbol_id: &str) -> Vec<Symbol> {
        let mut definitions = Vec::new();

        if let Some(node_idx) = self.graph.get_node_index(symbol_id) {
            for edge in self.graph.graph.edges(node_idx) {
                if matches!(edge.weight(), EdgeKind::Definition) {
                    if let Some(def) = self.graph.graph.node_weight(edge.target()) {
                        definitions.push(def.clone());
                    }
                }
            }
        }

        definitions
    }

    /// Find the ultimate source definition (the root of the definition chain)
    pub fn find_ultimate_source(&self, symbol_id: &str) -> Option<Symbol> {
        let chain = self.get_definition_chain(symbol_id)?;

        if chain.has_cycle {
            // In case of cycle, return the last non-cycling element
            chain.chain.last().cloned()
        } else {
            // Return the ultimate source (last in chain)
            chain.chain.last().cloned()
        }
    }

    /// Check if there's a definition path between two symbols
    pub fn has_definition_path(&self, from: &str, to: &str) -> bool {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(from.to_string());

        while let Some(current) = queue.pop_front() {
            if current == to {
                return true;
            }

            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            // Add all definitions to the queue
            for def in self.get_all_immediate_definitions(&current) {
                queue.push_back(def.id);
            }
        }

        false
    }

    /// Get the shortest definition path between two symbols
    pub fn get_shortest_definition_path(&self, from: &str, to: &str) -> Option<Vec<Symbol>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut parent_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        queue.push_back(from.to_string());
        visited.insert(from.to_string());

        // BFS to find shortest path
        while let Some(current) = queue.pop_front() {
            if current == to {
                // Reconstruct path
                let mut path = Vec::new();
                let mut node = to.to_string();

                while node != from {
                    if let Some(symbol) = self.graph.find_symbol(&node) {
                        path.push(symbol.clone());
                    }
                    node = parent_map.get(&node)?.clone();
                }

                if let Some(symbol) = self.graph.find_symbol(from) {
                    path.push(symbol.clone());
                }

                path.reverse();
                return Some(path);
            }

            // Explore definitions
            for def in self.get_all_immediate_definitions(&current) {
                if !visited.contains(&def.id) {
                    visited.insert(def.id.clone());
                    parent_map.insert(def.id.clone(), current.clone());
                    queue.push_back(def.id);
                }
            }
        }

        None
    }
}

/// Format definition chain as a string
pub fn format_definition_chain(chain: &DefinitionChain) -> String {
    let mut result = String::new();

    for (i, symbol) in chain.chain.iter().enumerate() {
        if i > 0 {
            result.push_str(" → ");
        }
        result.push_str(&format!(
            "{} ({}:{})",
            symbol.name,
            symbol.file_path,
            symbol.range.start.line + 1
        ));
    }

    if chain.has_cycle {
        result.push_str(" [CYCLE DETECTED]");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range, Symbol, SymbolKind};

    fn create_test_symbol(id: &str, name: &str) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: name.to_string(),
            kind: SymbolKind::Variable,
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
    fn test_simple_definition_chain() {
        let mut graph = CodeGraph::new();

        // Create chain: a -> b -> c
        let a = create_test_symbol("a", "var_a");
        let b = create_test_symbol("b", "var_b");
        let c = create_test_symbol("c", "var_c");

        let a_idx = graph.add_symbol(a);
        let b_idx = graph.add_symbol(b);
        let c_idx = graph.add_symbol(c);

        graph.add_edge(a_idx, b_idx, EdgeKind::Definition);
        graph.add_edge(b_idx, c_idx, EdgeKind::Definition);

        let analyzer = DefinitionChainAnalyzer::new(&graph);
        let chain = analyzer.get_definition_chain("a").unwrap();

        assert_eq!(chain.chain.len(), 3);
        assert!(!chain.has_cycle);
        assert_eq!(chain.chain[0].id, "a");
        assert_eq!(chain.chain[1].id, "b");
        assert_eq!(chain.chain[2].id, "c");
    }

    #[test]
    fn test_cyclic_definition_chain() {
        let mut graph = CodeGraph::new();

        // Create cycle: a -> b -> c -> a
        let a = create_test_symbol("a", "var_a");
        let b = create_test_symbol("b", "var_b");
        let c = create_test_symbol("c", "var_c");

        let a_idx = graph.add_symbol(a);
        let b_idx = graph.add_symbol(b);
        let c_idx = graph.add_symbol(c);

        graph.add_edge(a_idx, b_idx, EdgeKind::Definition);
        graph.add_edge(b_idx, c_idx, EdgeKind::Definition);
        graph.add_edge(c_idx, a_idx, EdgeKind::Definition);

        let analyzer = DefinitionChainAnalyzer::new(&graph);
        let chain = analyzer.get_definition_chain("a").unwrap();

        assert!(chain.has_cycle);
        assert_eq!(chain.chain.len(), 3);
    }

    #[test]
    fn test_multiple_definition_paths() {
        let mut graph = CodeGraph::new();

        // Create diamond: a -> b, a -> c, b -> d, c -> d
        let a = create_test_symbol("a", "var_a");
        let b = create_test_symbol("b", "var_b");
        let c = create_test_symbol("c", "var_c");
        let d = create_test_symbol("d", "var_d");

        let a_idx = graph.add_symbol(a);
        let b_idx = graph.add_symbol(b);
        let c_idx = graph.add_symbol(c);
        let d_idx = graph.add_symbol(d);

        graph.add_edge(a_idx, b_idx, EdgeKind::Definition);
        graph.add_edge(a_idx, c_idx, EdgeKind::Definition);
        graph.add_edge(b_idx, d_idx, EdgeKind::Definition);
        graph.add_edge(c_idx, d_idx, EdgeKind::Definition);

        let analyzer = DefinitionChainAnalyzer::new(&graph);
        let chains = analyzer.get_all_definition_chains("a");

        // Should have 2 paths: a->b->d and a->c->d
        assert_eq!(chains.len(), 2);

        for chain in &chains {
            assert_eq!(chain.chain.first().unwrap().id, "a");
            assert_eq!(chain.chain.last().unwrap().id, "d");
            assert!(!chain.has_cycle);
        }
    }

    #[test]
    fn test_ultimate_source() {
        let mut graph = CodeGraph::new();

        // Create chain: a -> b -> c -> d
        let a = create_test_symbol("a", "var_a");
        let b = create_test_symbol("b", "var_b");
        let c = create_test_symbol("c", "var_c");
        let d = create_test_symbol("d", "ultimate_source");

        let a_idx = graph.add_symbol(a);
        let b_idx = graph.add_symbol(b);
        let c_idx = graph.add_symbol(c);
        let d_idx = graph.add_symbol(d);

        graph.add_edge(a_idx, b_idx, EdgeKind::Definition);
        graph.add_edge(b_idx, c_idx, EdgeKind::Definition);
        graph.add_edge(c_idx, d_idx, EdgeKind::Definition);

        let analyzer = DefinitionChainAnalyzer::new(&graph);
        let ultimate = analyzer.find_ultimate_source("a").unwrap();

        assert_eq!(ultimate.id, "d");
        assert_eq!(ultimate.name, "ultimate_source");
    }

    #[test]
    fn test_definition_path_checking() {
        let mut graph = CodeGraph::new();

        // Create chain: a -> b -> c, d -> e
        let a = create_test_symbol("a", "var_a");
        let b = create_test_symbol("b", "var_b");
        let c = create_test_symbol("c", "var_c");
        let d = create_test_symbol("d", "var_d");
        let e = create_test_symbol("e", "var_e");

        let a_idx = graph.add_symbol(a);
        let b_idx = graph.add_symbol(b);
        let c_idx = graph.add_symbol(c);
        let d_idx = graph.add_symbol(d);
        let e_idx = graph.add_symbol(e);

        graph.add_edge(a_idx, b_idx, EdgeKind::Definition);
        graph.add_edge(b_idx, c_idx, EdgeKind::Definition);
        graph.add_edge(d_idx, e_idx, EdgeKind::Definition);

        let analyzer = DefinitionChainAnalyzer::new(&graph);

        assert!(analyzer.has_definition_path("a", "c"));
        assert!(analyzer.has_definition_path("a", "b"));
        assert!(!analyzer.has_definition_path("a", "e"));
        assert!(!analyzer.has_definition_path("c", "a")); // Not bidirectional
    }

    #[test]
    fn test_shortest_path() {
        let mut graph = CodeGraph::new();

        // Create graph with multiple paths
        let a = create_test_symbol("a", "var_a");
        let b = create_test_symbol("b", "var_b");
        let c = create_test_symbol("c", "var_c");
        let d = create_test_symbol("d", "var_d");

        let a_idx = graph.add_symbol(a);
        let b_idx = graph.add_symbol(b);
        let c_idx = graph.add_symbol(c);
        let d_idx = graph.add_symbol(d);

        // Path 1: a -> b -> c -> d (length 3)
        // Path 2: a -> d (length 1)
        graph.add_edge(a_idx, b_idx, EdgeKind::Definition);
        graph.add_edge(b_idx, c_idx, EdgeKind::Definition);
        graph.add_edge(c_idx, d_idx, EdgeKind::Definition);
        graph.add_edge(a_idx, d_idx, EdgeKind::Definition); // Direct path

        let analyzer = DefinitionChainAnalyzer::new(&graph);
        let path = analyzer.get_shortest_definition_path("a", "d").unwrap();

        assert_eq!(path.len(), 2); // Shortest path: a -> d
        assert_eq!(path[0].id, "a");
        assert_eq!(path[1].id, "d");
    }
}
