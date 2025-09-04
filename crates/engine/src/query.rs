//! Query engine for advanced graph traversal

use anyhow::{anyhow, Result};
use lsif_core::{CodeGraph, EdgeKind, Symbol, SymbolKind};
use std::collections::{HashMap, HashSet, VecDeque};

/// Query pattern for graph traversal
#[derive(Debug, Clone)]
pub struct QueryPattern {
    pub nodes: Vec<NodePattern>,
    pub relationships: Vec<RelationshipPattern>,
}

/// Node pattern in a query
#[derive(Debug, Clone)]
pub struct NodePattern {
    pub variable: Option<String>,
    pub label: Option<String>,
    pub properties: Vec<PropertyFilter>,
}

/// Relationship pattern in a query
#[derive(Debug, Clone)]
pub struct RelationshipPattern {
    pub from_index: usize,
    pub to_index: usize,
    pub edge_type: Option<EdgeKind>,
    pub direction: Direction,
    pub min_depth: usize,
    pub max_depth: Option<usize>,
}

/// Direction of relationship traversal
#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Forward,
    Backward,
    Both,
}

/// Property filter for nodes
#[derive(Debug, Clone)]
pub struct PropertyFilter {
    pub key: String,
    pub operator: FilterOperator,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilterOperator {
    Equals,
    Contains,
    StartsWith,
    EndsWith,
    Regex,
}

/// Result of a query execution
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub matches: Vec<Match>,
}

/// A single match from the query
#[derive(Debug, Clone)]
pub struct Match {
    pub bindings: HashMap<String, Symbol>,
    pub paths: Vec<Vec<Symbol>>,
}

/// Query engine for executing graph queries
pub struct QueryEngine {
    graph: CodeGraph,
}

impl QueryEngine {
    /// Create a new query engine
    pub fn new(graph: CodeGraph) -> Self {
        Self { graph }
    }

    /// Execute a query pattern
    pub fn execute(&self, pattern: &QueryPattern) -> Result<QueryResult> {
        let mut matches = Vec::new();

        // Start with the first node pattern
        if pattern.nodes.is_empty() {
            return Ok(QueryResult { matches });
        }

        let first_pattern = &pattern.nodes[0];
        let candidates = self.find_matching_nodes(first_pattern)?;

        for candidate in candidates {
            let mut bindings = HashMap::new();
            if let Some(ref var) = first_pattern.variable {
                bindings.insert(var.clone(), candidate.clone());
            }

            // Try to match the rest of the pattern
            if pattern.nodes.len() == 1 {
                matches.push(Match {
                    bindings,
                    paths: vec![vec![candidate]],
                });
            } else {
                let candidate_clone = candidate.clone();
                self.match_pattern_recursive(
                    &candidate,
                    pattern,
                    1,
                    bindings,
                    vec![candidate_clone],
                    &mut matches,
                )?;
            }
        }

        Ok(QueryResult { matches })
    }

    /// Parse and execute a Cypher-like query string
    pub fn query(&self, query_string: &str) -> Result<QueryResult> {
        let pattern = self.parse_query(query_string)?;
        self.execute(&pattern)
    }

    fn find_matching_nodes(&self, pattern: &NodePattern) -> Result<Vec<Symbol>> {
        let mut results = Vec::new();

        for symbol in self.graph.get_all_symbols() {
            if self.node_matches_pattern(symbol, pattern)? {
                results.push(symbol.clone());
            }
        }

        Ok(results)
    }

    fn node_matches_pattern(&self, symbol: &Symbol, pattern: &NodePattern) -> Result<bool> {
        // Check label (symbol kind)
        if let Some(ref label) = pattern.label {
            let expected_kind = self.parse_symbol_kind(label)?;
            if symbol.kind != expected_kind {
                return Ok(false);
            }
        }

        // Check properties
        for filter in &pattern.properties {
            if !self.property_matches(symbol, filter)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn property_matches(&self, symbol: &Symbol, filter: &PropertyFilter) -> Result<bool> {
        let value = match filter.key.as_str() {
            "name" => &symbol.name,
            "file" | "file_path" => &symbol.file_path,
            "id" => &symbol.id,
            _ => return Ok(false),
        };

        Ok(match filter.operator {
            FilterOperator::Equals => value == &filter.value,
            FilterOperator::Contains => value.contains(&filter.value),
            FilterOperator::StartsWith => value.starts_with(&filter.value),
            FilterOperator::EndsWith => value.ends_with(&filter.value),
            FilterOperator::Regex => {
                let re = regex::Regex::new(&filter.value)?;
                re.is_match(value)
            }
        })
    }

    fn match_pattern_recursive(
        &self,
        current: &Symbol,
        pattern: &QueryPattern,
        node_index: usize,
        bindings: HashMap<String, Symbol>,
        path: Vec<Symbol>,
        matches: &mut Vec<Match>,
    ) -> Result<()> {
        if node_index >= pattern.nodes.len() {
            matches.push(Match {
                bindings,
                paths: vec![path],
            });
            return Ok(());
        }

        // Find relationships from current node
        for rel in &pattern.relationships {
            if rel.from_index == node_index - 1 && rel.to_index == node_index {
                let next_nodes = self.traverse_relationship(current, rel)?;

                for next in next_nodes {
                    if self.node_matches_pattern(&next, &pattern.nodes[node_index])? {
                        let mut new_bindings = bindings.clone();
                        if let Some(ref var) = pattern.nodes[node_index].variable {
                            new_bindings.insert(var.clone(), next.clone());
                        }

                        let mut new_path = path.clone();
                        new_path.push(next.clone());

                        self.match_pattern_recursive(
                            &next,
                            pattern,
                            node_index + 1,
                            new_bindings,
                            new_path,
                            matches,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }

    fn traverse_relationship(
        &self,
        from: &Symbol,
        rel: &RelationshipPattern,
    ) -> Result<Vec<Symbol>> {
        let mut results = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((from.clone(), 0));
        visited.insert(from.id.clone());

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= rel.min_depth {
                if let Some(max) = rel.max_depth {
                    if depth > max {
                        continue;
                    }
                }

                if depth > 0 {
                    results.push(current.clone());
                }
            }

            if rel.max_depth.is_none() || depth < rel.max_depth.unwrap() {
                let neighbors = match rel.direction {
                    Direction::Forward => {
                        self.graph.get_outgoing_edges(&current.id, rel.edge_type)?
                    }
                    Direction::Backward => {
                        self.graph.get_incoming_edges(&current.id, rel.edge_type)?
                    }
                    Direction::Both => {
                        let mut edges =
                            self.graph.get_outgoing_edges(&current.id, rel.edge_type)?;
                        edges.extend(self.graph.get_incoming_edges(&current.id, rel.edge_type)?);
                        edges
                    }
                };

                for neighbor in neighbors {
                    if !visited.contains(&neighbor.id) {
                        visited.insert(neighbor.id.clone());
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
        }

        Ok(results)
    }

    fn parse_symbol_kind(&self, label: &str) -> Result<SymbolKind> {
        match label.to_lowercase().as_str() {
            "function" | "func" => Ok(SymbolKind::Function),
            "class" => Ok(SymbolKind::Class),
            "interface" => Ok(SymbolKind::Interface),
            "struct" => Ok(SymbolKind::Struct),
            "enum" => Ok(SymbolKind::Enum),
            "variable" | "var" => Ok(SymbolKind::Variable),
            "constant" | "const" => Ok(SymbolKind::Constant),
            "method" => Ok(SymbolKind::Method),
            "property" | "prop" => Ok(SymbolKind::Property),
            "module" | "mod" => Ok(SymbolKind::Module),
            "namespace" => Ok(SymbolKind::Namespace),
            _ => Err(anyhow!("Unknown symbol kind: {}", label)),
        }
    }

    /// Parse a simple Cypher-like query
    /// Examples:
    /// - "(fn:Function)-[:Reference]->(type:Class)"
    /// - "(var:Variable{name:'main'})-[:Call]->()"
    pub fn parse_query(&self, query: &str) -> Result<QueryPattern> {
        // This is a simplified parser - in production, you'd want a proper parser
        let mut nodes = Vec::new();
        let mut relationships = Vec::new();

        // For now, just parse simple patterns
        if query.contains("->") {
            let parts: Vec<&str> = query.split("->").collect();

            for (i, part) in parts.iter().enumerate() {
                let node = self.parse_node_pattern(part)?;
                nodes.push(node);

                if i > 0 {
                    relationships.push(RelationshipPattern {
                        from_index: i - 1,
                        to_index: i,
                        edge_type: None,
                        direction: Direction::Forward,
                        min_depth: 1,
                        max_depth: Some(1),
                    });
                }
            }
        } else {
            nodes.push(self.parse_node_pattern(query)?);
        }

        Ok(QueryPattern {
            nodes,
            relationships,
        })
    }

    fn parse_node_pattern(&self, pattern: &str) -> Result<NodePattern> {
        let pattern = pattern.trim();

        // Remove parentheses if present
        let pattern = if pattern.starts_with('(') && pattern.ends_with(')') {
            &pattern[1..pattern.len() - 1]
        } else {
            pattern
        };

        // Parse variable:Label{properties}
        let mut variable = None;
        let mut label = None;
        let properties = Vec::new();

        if let Some(colon_pos) = pattern.find(':') {
            let var_part = pattern[..colon_pos].trim();
            if !var_part.is_empty() {
                variable = Some(var_part.to_string());
            }

            let rest = &pattern[colon_pos + 1..];
            if let Some(brace_pos) = rest.find('{') {
                label = Some(rest[..brace_pos].trim().to_string());
            } else {
                label = Some(rest.trim().to_string());
            }
        }

        Ok(NodePattern {
            variable,
            label,
            properties,
        })
    }
}
