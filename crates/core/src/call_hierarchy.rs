use super::graph::{CodeGraph, EdgeKind, Symbol};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct CallHierarchy {
    pub symbol: Symbol,
    pub callers: Vec<CallHierarchy>,
    pub callees: Vec<CallHierarchy>,
    pub depth: usize,
}

pub struct CallHierarchyAnalyzer<'a> {
    graph: &'a CodeGraph,
}

impl<'a> CallHierarchyAnalyzer<'a> {
    pub fn new(graph: &'a CodeGraph) -> Self {
        Self { graph }
    }

    /// Get incoming calls (who calls this function)
    pub fn get_incoming_calls(&self, symbol_id: &str, max_depth: usize) -> Option<CallHierarchy> {
        let _symbol = self.graph.find_symbol(symbol_id)?;
        let mut visited = HashSet::new();
        self.build_incoming_hierarchy(symbol_id, &mut visited, 0, max_depth)
    }

    /// Get outgoing calls (what this function calls)
    pub fn get_outgoing_calls(&self, symbol_id: &str, max_depth: usize) -> Option<CallHierarchy> {
        let _symbol = self.graph.find_symbol(symbol_id)?;
        let mut visited = HashSet::new();
        self.build_outgoing_hierarchy(symbol_id, &mut visited, 0, max_depth)
    }

    /// Get full call hierarchy (both incoming and outgoing)
    pub fn get_full_hierarchy(&self, symbol_id: &str, max_depth: usize) -> Option<CallHierarchy> {
        let symbol = self.graph.find_symbol(symbol_id)?.clone();
        let mut visited_in = HashSet::new();
        let mut visited_out = HashSet::new();

        let callers = self.get_callers(symbol_id, &mut visited_in, 1, max_depth);
        let callees = self.get_callees(symbol_id, &mut visited_out, 1, max_depth);

        Some(CallHierarchy {
            symbol,
            callers,
            callees,
            depth: 0,
        })
    }

    fn build_incoming_hierarchy(
        &self,
        symbol_id: &str,
        visited: &mut HashSet<String>,
        depth: usize,
        max_depth: usize,
    ) -> Option<CallHierarchy> {
        if depth > max_depth || visited.contains(symbol_id) {
            return None;
        }

        visited.insert(symbol_id.to_string());
        let symbol = self.graph.find_symbol(symbol_id)?.clone();

        let callers = if depth < max_depth {
            self.get_callers(symbol_id, visited, depth + 1, max_depth)
        } else {
            Vec::new()
        };

        Some(CallHierarchy {
            symbol,
            callers,
            callees: Vec::new(),
            depth,
        })
    }

    fn build_outgoing_hierarchy(
        &self,
        symbol_id: &str,
        visited: &mut HashSet<String>,
        depth: usize,
        max_depth: usize,
    ) -> Option<CallHierarchy> {
        if depth > max_depth || visited.contains(symbol_id) {
            return None;
        }

        visited.insert(symbol_id.to_string());
        let symbol = self.graph.find_symbol(symbol_id)?.clone();

        let callees = if depth < max_depth {
            self.get_callees(symbol_id, visited, depth + 1, max_depth)
        } else {
            Vec::new()
        };

        Some(CallHierarchy {
            symbol,
            callers: Vec::new(),
            callees,
            depth,
        })
    }

    fn get_callers(
        &self,
        symbol_id: &str,
        visited: &mut HashSet<String>,
        depth: usize,
        max_depth: usize,
    ) -> Vec<CallHierarchy> {
        let mut callers = Vec::new();

        if let Some(node_idx) = self.graph.get_node_index(symbol_id) {
            for edge in self
                .graph
                .graph
                .edges_directed(node_idx, Direction::Incoming)
            {
                if matches!(edge.weight(), EdgeKind::Reference) {
                    let source_idx = edge.source();
                    if let Some(caller) = self.graph.graph.node_weight(source_idx) {
                        if !visited.contains(&caller.id) {
                            if let Some(hierarchy) =
                                self.build_incoming_hierarchy(&caller.id, visited, depth, max_depth)
                            {
                                callers.push(hierarchy);
                            }
                        }
                    }
                }
            }
        }

        callers
    }

    fn get_callees(
        &self,
        symbol_id: &str,
        visited: &mut HashSet<String>,
        depth: usize,
        max_depth: usize,
    ) -> Vec<CallHierarchy> {
        let mut callees = Vec::new();

        if let Some(node_idx) = self.graph.get_node_index(symbol_id) {
            for edge in self
                .graph
                .graph
                .edges_directed(node_idx, Direction::Outgoing)
            {
                if matches!(edge.weight(), EdgeKind::Reference) {
                    let target_idx = edge.target();
                    if let Some(callee) = self.graph.graph.node_weight(target_idx) {
                        if !visited.contains(&callee.id) {
                            if let Some(hierarchy) =
                                self.build_outgoing_hierarchy(&callee.id, visited, depth, max_depth)
                            {
                                callees.push(hierarchy);
                            }
                        }
                    }
                }
            }
        }

        callees
    }

    /// Find all paths between two functions
    pub fn find_call_paths(&self, from: &str, to: &str, max_depth: usize) -> Vec<Vec<String>> {
        let mut paths = Vec::new();
        let mut current_path = vec![from.to_string()];
        let mut visited = HashSet::new();

        self.dfs_paths(
            from,
            to,
            &mut current_path,
            &mut visited,
            &mut paths,
            0,
            max_depth,
        );

        paths
    }

    #[allow(clippy::too_many_arguments)]
    fn dfs_paths(
        &self,
        current: &str,
        target: &str,
        current_path: &mut Vec<String>,
        visited: &mut HashSet<String>,
        all_paths: &mut Vec<Vec<String>>,
        depth: usize,
        max_depth: usize,
    ) {
        if depth > max_depth {
            return;
        }

        if current == target {
            all_paths.push(current_path.clone());
            return;
        }

        visited.insert(current.to_string());

        if let Some(node_idx) = self.graph.get_node_index(current) {
            for edge in self
                .graph
                .graph
                .edges_directed(node_idx, Direction::Outgoing)
            {
                if matches!(edge.weight(), EdgeKind::Reference) {
                    let target_idx = edge.target();
                    if let Some(next_symbol) = self.graph.graph.node_weight(target_idx) {
                        if !visited.contains(&next_symbol.id) {
                            current_path.push(next_symbol.id.clone());
                            self.dfs_paths(
                                &next_symbol.id,
                                target,
                                current_path,
                                visited,
                                all_paths,
                                depth + 1,
                                max_depth,
                            );
                            current_path.pop();
                        }
                    }
                }
            }
        }

        visited.remove(current);
    }
}

/// Format call hierarchy as a tree string
pub fn format_hierarchy(hierarchy: &CallHierarchy, prefix: &str, is_last: bool) -> String {
    let mut result = String::new();

    let connector = if hierarchy.depth == 0 {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };

    result.push_str(&format!(
        "{}{}{}\n",
        prefix, connector, hierarchy.symbol.name
    ));

    let new_prefix = if hierarchy.depth == 0 {
        String::new()
    } else if is_last {
        format!("{prefix}    ")
    } else {
        format!("{prefix}│   ")
    };

    // Format callers
    for (i, caller) in hierarchy.callers.iter().enumerate() {
        let is_last_caller = i == hierarchy.callers.len() - 1 && hierarchy.callees.is_empty();
        result.push_str(&format_hierarchy(caller, &new_prefix, is_last_caller));
    }

    // Format callees
    for (i, callee) in hierarchy.callees.iter().enumerate() {
        let is_last_callee = i == hierarchy.callees.len() - 1;
        result.push_str(&format_hierarchy(callee, &new_prefix, is_last_callee));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range, Symbol, SymbolKind};

    fn create_test_graph() -> CodeGraph {
        let mut graph = CodeGraph::new();

        // Create test symbols
        let main_sym = Symbol {
            id: "main".to_string(),
            name: "main".to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
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
            documentation: None,
            detail: None,
        };

        let calc_sym = Symbol {
            id: "calculate".to_string(),
            name: "calculate".to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 7,
                    character: 0,
                },
                end: Position {
                    line: 10,
                    character: 0,
                },
            },
            documentation: None,
            detail: None,
        };

        let add_sym = Symbol {
            id: "add".to_string(),
            name: "add".to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 12,
                    character: 0,
                },
                end: Position {
                    line: 14,
                    character: 0,
                },
            },
            documentation: None,
            detail: None,
        };

        // Add symbols to graph
        let main_idx = graph.add_symbol(main_sym);
        let calc_idx = graph.add_symbol(calc_sym);
        let add_idx = graph.add_symbol(add_sym);

        // Add edges (main -> calculate -> add)
        graph.add_edge(main_idx, calc_idx, EdgeKind::Reference);
        graph.add_edge(calc_idx, add_idx, EdgeKind::Reference);

        graph
    }

    #[test]
    fn test_outgoing_calls() {
        let graph = create_test_graph();
        let analyzer = CallHierarchyAnalyzer::new(&graph);

        let hierarchy = analyzer.get_outgoing_calls("main", 2).unwrap();
        assert_eq!(hierarchy.symbol.name, "main");
        assert_eq!(hierarchy.callees.len(), 1);
        assert_eq!(hierarchy.callees[0].symbol.name, "calculate");
        assert_eq!(hierarchy.callees[0].callees.len(), 1);
        assert_eq!(hierarchy.callees[0].callees[0].symbol.name, "add");
    }

    #[test]
    fn test_incoming_calls() {
        let graph = create_test_graph();
        let analyzer = CallHierarchyAnalyzer::new(&graph);

        let hierarchy = analyzer.get_incoming_calls("add", 2).unwrap();
        assert_eq!(hierarchy.symbol.name, "add");
        assert_eq!(hierarchy.callers.len(), 1);
        assert_eq!(hierarchy.callers[0].symbol.name, "calculate");
        assert_eq!(hierarchy.callers[0].callers.len(), 1);
        assert_eq!(hierarchy.callers[0].callers[0].symbol.name, "main");
    }

    #[test]
    fn test_call_paths() {
        let graph = create_test_graph();
        let analyzer = CallHierarchyAnalyzer::new(&graph);

        let paths = analyzer.find_call_paths("main", "add", 3);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], vec!["main", "calculate", "add"]);
    }

    #[test]
    fn test_format_hierarchy() {
        let graph = create_test_graph();
        let analyzer = CallHierarchyAnalyzer::new(&graph);

        let hierarchy = analyzer.get_outgoing_calls("main", 2).unwrap();
        let formatted = format_hierarchy(&hierarchy, "", true);

        assert!(formatted.contains("main"));
        assert!(formatted.contains("└── calculate"));
        assert!(formatted.contains("    └── add"));
    }
}
