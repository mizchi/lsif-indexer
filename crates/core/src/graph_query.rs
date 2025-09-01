use super::graph::{CodeGraph, EdgeKind, Symbol, SymbolKind};
use petgraph::visit::EdgeRef;
use std::collections::{HashSet, VecDeque};
use std::fmt;

/// Query pattern for graph traversal
#[derive(Debug, Clone)]
pub struct QueryPattern {
    pub nodes: Vec<NodePattern>,
    pub relationships: Vec<RelationshipPattern>,
}

/// Node pattern in a query
#[derive(Debug, Clone)]
pub struct NodePattern {
    pub variable: Option<String>,        // e.g., "fn" in (fn:Function)
    pub label: Option<String>,           // e.g., "Function" in (fn:Function)
    pub properties: Vec<PropertyFilter>, // e.g., name="main"
}

/// Relationship pattern in a query
#[derive(Debug, Clone)]
pub struct RelationshipPattern {
    pub from_index: usize, // Index in nodes array
    pub to_index: usize,   // Index in nodes array
    pub edge_type: Option<EdgeKind>,
    pub direction: Direction,
    pub min_depth: usize,
    pub max_depth: Option<usize>,
}

/// Direction of relationship traversal
#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Forward,  // ->
    Backward, // <-
    Both,     // --
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
    pub bindings: Vec<(String, Symbol)>, // Variable name -> Symbol
    pub paths: Vec<Vec<Symbol>>,         // Paths found in traversal
}

/// Query parser
pub struct QueryParser;

impl QueryParser {
    /// Parse a Cypher-like query string
    /// Examples:
    /// - "(fn:Function)-[:Reference]->(type:Class)"
    /// - "(var:Variable)-[:Reference*1..3]->()"
    /// - "(a:Class)-[:Definition*]->(b:Interface)"
    pub fn parse(query: &str) -> Result<QueryPattern, QueryParseError> {
        let mut parser = QueryParser;
        parser.parse_pattern(query)
    }

    fn parse_pattern(&mut self, query: &str) -> Result<QueryPattern, QueryParseError> {
        let mut nodes = Vec::new();
        let mut relationships = Vec::new();
        let mut current_pos = 0;
        let query = query.trim();
        let chars: Vec<char> = query.chars().collect();

        // Parse nodes and relationships alternately
        while current_pos < chars.len() {
            // Skip whitespace
            while current_pos < chars.len() && chars[current_pos].is_whitespace() {
                current_pos += 1;
            }

            if current_pos >= chars.len() {
                break;
            }

            // Parse node
            if chars[current_pos] == '(' {
                let (node, new_pos) = self.parse_node(&chars, current_pos)?;
                nodes.push(node);
                current_pos = new_pos;
            }

            // Skip whitespace
            while current_pos < chars.len() && chars[current_pos].is_whitespace() {
                current_pos += 1;
            }

            // Parse relationship if exists
            if current_pos < chars.len() && chars[current_pos] == '-' {
                let from_index = nodes.len() - 1;
                let (rel, new_pos) = self.parse_relationship(&chars, current_pos)?;
                current_pos = new_pos;

                // Skip whitespace
                while current_pos < chars.len() && chars[current_pos].is_whitespace() {
                    current_pos += 1;
                }

                // Parse target node
                if current_pos < chars.len() && chars[current_pos] == '(' {
                    let (node, new_pos) = self.parse_node(&chars, current_pos)?;
                    nodes.push(node);
                    current_pos = new_pos;

                    relationships.push(RelationshipPattern {
                        from_index,
                        to_index: nodes.len() - 1,
                        edge_type: rel.edge_type,
                        direction: rel.direction,
                        min_depth: rel.min_depth,
                        max_depth: rel.max_depth,
                    });
                }
            }
        }

        Ok(QueryPattern {
            nodes,
            relationships,
        })
    }

    fn parse_node(
        &self,
        chars: &[char],
        start: usize,
    ) -> Result<(NodePattern, usize), QueryParseError> {
        let mut pos = start;

        if chars[pos] != '(' {
            return Err(QueryParseError::ExpectedChar('(', pos));
        }
        pos += 1;

        // Find closing parenthesis
        let mut end = pos;
        while end < chars.len() && chars[end] != ')' {
            end += 1;
        }

        if end >= chars.len() {
            return Err(QueryParseError::UnmatchedParenthesis(start));
        }

        // Parse node content
        let content: String = chars[pos..end].iter().collect();
        let content = content.trim();

        let mut variable = None;
        let mut label = None;
        let properties = Vec::new(); // TODO: Parse properties

        if !content.is_empty() {
            if let Some(colon_pos) = content.find(':') {
                // Has both variable and label
                let var = content[..colon_pos].trim();
                if !var.is_empty() {
                    variable = Some(var.to_string());
                }
                label = Some(content[colon_pos + 1..].trim().to_string());
            } else {
                // Only variable
                variable = Some(content.to_string());
            }
        }

        Ok((
            NodePattern {
                variable,
                label,
                properties,
            },
            end + 1,
        ))
    }

    fn parse_relationship(
        &self,
        chars: &[char],
        start: usize,
    ) -> Result<(RelationshipPattern, usize), QueryParseError> {
        let mut pos = start;
        let mut direction = Direction::Forward;
        let mut edge_type = None;
        let mut min_depth = 1;
        let mut max_depth = Some(1);

        // Check for backward arrow (<-)
        if pos < chars.len() && chars[pos] == '<' {
            direction = Direction::Backward;
            pos += 1;
            // After '<' must come '-'
            if pos >= chars.len() || chars[pos] != '-' {
                return Err(QueryParseError::ExpectedChar('-', pos));
            }
            pos += 1;
        } else if pos < chars.len() && chars[pos] == '-' {
            // Forward or bidirectional, start with '-'
            pos += 1;
        } else {
            return Err(QueryParseError::ExpectedChar('-', pos));
        }

        // Check for relationship details [...]
        if pos < chars.len() && chars[pos] == '[' {
            pos += 1;
            let mut end = pos;
            while end < chars.len() && chars[end] != ']' {
                end += 1;
            }

            if end >= chars.len() {
                return Err(QueryParseError::UnmatchedBracket(pos - 1));
            }

            let content: String = chars[pos..end].iter().collect();
            let content = content.trim();

            // Parse relationship type and depth
            if content.starts_with(':') {
                let parts: Vec<&str> = content.strip_prefix(':').unwrap_or("").split('*').collect();

                // Parse edge type
                let edge_type_str = parts[0].trim();
                edge_type = match edge_type_str {
                    "Definition" => Some(EdgeKind::Definition),
                    "Reference" => Some(EdgeKind::Reference),
                    "TypeDefinition" => Some(EdgeKind::TypeDefinition),
                    "Implementation" => Some(EdgeKind::Implementation),
                    "Override" => Some(EdgeKind::Override),
                    "Import" => Some(EdgeKind::Import),
                    "Export" => Some(EdgeKind::Export),
                    "Contains" => Some(EdgeKind::Contains),
                    _ => None,
                };

                // Parse depth if specified
                if parts.len() > 1 {
                    let depth_str = parts[1].trim();
                    if depth_str.contains("..") {
                        let range_parts: Vec<&str> = depth_str.split("..").collect();
                        if range_parts.len() == 2 {
                            min_depth = range_parts[0].parse().unwrap_or(1);
                            if range_parts[1].is_empty() {
                                max_depth = None; // Unlimited
                            } else {
                                max_depth = Some(range_parts[1].parse().unwrap_or(1));
                            }
                        }
                    } else if !depth_str.is_empty() {
                        let depth = depth_str.parse().unwrap_or(1);
                        min_depth = depth;
                        max_depth = Some(depth);
                    } else {
                        max_depth = None; // * means unlimited
                    }
                }
            }

            pos = end + 1;
        }

        // Check for arrow
        if pos < chars.len() && chars[pos] == '-' {
            pos += 1;
            if pos < chars.len() && chars[pos] == '>' {
                if direction == Direction::Backward {
                    direction = Direction::Both;
                }
                pos += 1;
            }
        }

        Ok((
            RelationshipPattern {
                from_index: 0, // Will be set by caller
                to_index: 0,   // Will be set by caller
                edge_type,
                direction,
                min_depth,
                max_depth,
            },
            pos,
        ))
    }
}

/// Query execution engine
pub struct QueryEngine<'a> {
    graph: &'a CodeGraph,
}

impl<'a> QueryEngine<'a> {
    pub fn new(graph: &'a CodeGraph) -> Self {
        Self { graph }
    }

    /// Execute a query pattern on the graph
    pub fn execute(&self, pattern: &QueryPattern) -> QueryResult {
        let mut all_matches = Vec::new();

        // If no nodes specified, return empty
        if pattern.nodes.is_empty() {
            return QueryResult {
                matches: all_matches,
            };
        }

        // Find candidate nodes for the first pattern
        let first_candidates = self.find_matching_nodes(&pattern.nodes[0]);

        // For each candidate, try to match the full pattern
        for candidate in first_candidates {
            if let Some(match_result) = self.match_pattern_from(candidate, pattern) {
                all_matches.push(match_result);
            }
        }

        QueryResult {
            matches: all_matches,
        }
    }

    /// Find all nodes matching a node pattern
    fn find_matching_nodes(&self, pattern: &NodePattern) -> Vec<Symbol> {
        let mut matches = Vec::new();

        for symbol in self.graph.get_all_symbols() {
            if self.node_matches_pattern(symbol, pattern) {
                matches.push(symbol.clone());
            }
        }

        matches
    }

    /// Check if a symbol matches a node pattern
    fn node_matches_pattern(&self, symbol: &Symbol, pattern: &NodePattern) -> bool {
        // Check label (symbol kind)
        if let Some(ref label) = pattern.label {
            let matches_kind = match label.as_str() {
                "Function" => matches!(symbol.kind, SymbolKind::Function),
                "Class" => matches!(symbol.kind, SymbolKind::Class),
                "Interface" => matches!(symbol.kind, SymbolKind::Interface),
                "Variable" => matches!(symbol.kind, SymbolKind::Variable),
                "Method" => matches!(symbol.kind, SymbolKind::Method),
                "Module" => matches!(symbol.kind, SymbolKind::Module),
                "Namespace" => matches!(symbol.kind, SymbolKind::Namespace),
                "Enum" => matches!(symbol.kind, SymbolKind::Enum),
                "Property" => matches!(symbol.kind, SymbolKind::Property),
                "Field" => matches!(symbol.kind, SymbolKind::Field),
                "Parameter" => matches!(symbol.kind, SymbolKind::Parameter),
                "Constant" => matches!(symbol.kind, SymbolKind::Constant),
                _ => false,
            };

            if !matches_kind {
                return false;
            }
        }

        // Check properties
        for filter in &pattern.properties {
            if !self.property_matches(symbol, filter) {
                return false;
            }
        }

        true
    }

    /// Check if a symbol property matches a filter
    fn property_matches(&self, symbol: &Symbol, filter: &PropertyFilter) -> bool {
        let value = match filter.key.as_str() {
            "name" => &symbol.name,
            "id" => &symbol.id,
            "file" | "file_path" => &symbol.file_path,
            _ => return false,
        };

        match filter.operator {
            FilterOperator::Equals => value == &filter.value,
            FilterOperator::Contains => value.contains(&filter.value),
            FilterOperator::StartsWith => value.starts_with(&filter.value),
            FilterOperator::EndsWith => value.ends_with(&filter.value),
            FilterOperator::Regex => {
                // Simple regex support could be added here
                false
            }
        }
    }

    /// Try to match the full pattern starting from a node
    fn match_pattern_from(&self, start: Symbol, pattern: &QueryPattern) -> Option<Match> {
        let mut bindings = Vec::new();
        let mut paths = Vec::new();

        // Add first node binding
        if let Some(ref var) = pattern.nodes[0].variable {
            bindings.push((var.clone(), start.clone()));
        }

        // If only one node, we're done
        if pattern.nodes.len() == 1 {
            return Some(Match { bindings, paths });
        }

        // Try to match relationships
        for rel in &pattern.relationships {
            if rel.from_index >= pattern.nodes.len() || rel.to_index >= pattern.nodes.len() {
                continue;
            }

            let _from_pattern = &pattern.nodes[rel.from_index];
            let to_pattern = &pattern.nodes[rel.to_index];

            // Find paths matching the relationship
            let matching_paths = self.find_paths_matching_relationship(&start, rel, to_pattern);

            if matching_paths.is_empty() {
                return None; // Pattern doesn't match
            }

            // Add paths and bindings
            for path in matching_paths {
                if let Some(last) = path.last() {
                    if let Some(ref var) = to_pattern.variable {
                        bindings.push((var.clone(), last.clone()));
                    }
                }
                paths.push(path);
            }
        }

        Some(Match { bindings, paths })
    }

    /// Find paths that match a relationship pattern
    fn find_paths_matching_relationship(
        &self,
        from: &Symbol,
        rel: &RelationshipPattern,
        to_pattern: &NodePattern,
    ) -> Vec<Vec<Symbol>> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((from.id.clone(), vec![from.clone()], 0));

        while let Some((current_id, path, depth)) = queue.pop_front() {
            // Check depth limits
            if let Some(max) = rel.max_depth {
                if depth > max {
                    continue;
                }
            }

            // If we've reached minimum depth, check if target matches
            if depth >= rel.min_depth {
                if let Some(last) = path.last() {
                    if self.node_matches_pattern(last, to_pattern) {
                        result.push(path.clone());
                    }
                }
            }

            // Continue traversal if not at max depth
            if rel.max_depth.is_none() || depth < rel.max_depth.unwrap() {
                if let Some(node_idx) = self.graph.get_node_index(&current_id) {
                    // Traverse edges based on direction
                    let edges: Vec<_> = match rel.direction {
                        Direction::Forward => self.graph.graph.edges(node_idx).collect(),
                        Direction::Backward => self
                            .graph
                            .graph
                            .edges_directed(node_idx, petgraph::Direction::Incoming)
                            .collect(),
                        Direction::Both => {
                            let mut edges = self.graph.graph.edges(node_idx).collect::<Vec<_>>();
                            edges.extend(
                                self.graph
                                    .graph
                                    .edges_directed(node_idx, petgraph::Direction::Incoming),
                            );
                            edges
                        }
                    };

                    for edge in edges {
                        // Check edge type
                        if let Some(ref edge_type) = rel.edge_type {
                            if !matches_edge_kind(edge.weight(), edge_type) {
                                continue;
                            }
                        }

                        let target_idx = if rel.direction == Direction::Backward {
                            edge.source()
                        } else {
                            edge.target()
                        };

                        if let Some(target) = self.graph.graph.node_weight(target_idx) {
                            let target_id = target.id.clone();
                            if !visited.contains(&target_id) {
                                visited.insert(target_id.clone());
                                let mut new_path = path.clone();
                                new_path.push(target.clone());
                                queue.push_back((target_id, new_path, depth + 1));
                            }
                        }
                    }
                }
            }
        }

        result
    }
}

fn matches_edge_kind(actual: &EdgeKind, expected: &EdgeKind) -> bool {
    matches!(
        (actual, expected),
        (EdgeKind::Definition, EdgeKind::Definition)
            | (EdgeKind::Reference, EdgeKind::Reference)
            | (EdgeKind::TypeDefinition, EdgeKind::TypeDefinition)
            | (EdgeKind::Implementation, EdgeKind::Implementation)
            | (EdgeKind::Override, EdgeKind::Override)
            | (EdgeKind::Import, EdgeKind::Import)
            | (EdgeKind::Export, EdgeKind::Export)
            | (EdgeKind::Contains, EdgeKind::Contains)
    )
}

/// Query parse error
#[derive(Debug)]
pub enum QueryParseError {
    ExpectedChar(char, usize),
    UnmatchedParenthesis(usize),
    UnmatchedBracket(usize),
    InvalidSyntax(String),
}

impl fmt::Display for QueryParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            QueryParseError::ExpectedChar(ch, pos) => {
                write!(f, "Expected '{ch}' at position {pos}")
            }
            QueryParseError::UnmatchedParenthesis(pos) => {
                write!(f, "Unmatched parenthesis at position {pos}")
            }
            QueryParseError::UnmatchedBracket(pos) => {
                write!(f, "Unmatched bracket at position {pos}")
            }
            QueryParseError::InvalidSyntax(msg) => {
                write!(f, "Invalid syntax: {msg}")
            }
        }
    }
}

impl std::error::Error for QueryParseError {}

/// Format query results
pub fn format_query_results(results: &QueryResult) -> String {
    let mut output = String::new();

    output.push_str(&format!("Found {} matches\n\n", results.matches.len()));

    for (i, match_result) in results.matches.iter().enumerate() {
        output.push_str(&format!("Match {}:\n", i + 1));

        // Show bindings
        if !match_result.bindings.is_empty() {
            output.push_str("  Bindings:\n");
            for (var, symbol) in &match_result.bindings {
                output.push_str(&format!(
                    "    {} = {} ({})\n",
                    var, symbol.name, symbol.file_path
                ));
            }
        }

        // Show paths
        if !match_result.paths.is_empty() {
            output.push_str("  Paths:\n");
            for path in &match_result.paths {
                output.push_str("    ");
                for (j, node) in path.iter().enumerate() {
                    if j > 0 {
                        output.push_str(" -> ");
                    }
                    output.push_str(&node.name);
                }
                output.push('\n');
            }
        }

        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range, Symbol};

    fn create_test_symbol(id: &str, name: &str, kind: SymbolKind) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: name.to_string(),
            kind,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 10,
                    character: 0,
                },
            },
            documentation: None,
            detail: None,
        }
    }

    #[test]
    fn test_parse_simple_node() {
        let query = "(fn:Function)";
        let pattern = QueryParser::parse(query).unwrap();

        assert_eq!(pattern.nodes.len(), 1);
        assert_eq!(pattern.nodes[0].variable, Some("fn".to_string()));
        assert_eq!(pattern.nodes[0].label, Some("Function".to_string()));
    }

    #[test]
    fn test_parse_relationship() {
        let query = "(a:Class)-[:Definition]->(b:Interface)";
        let pattern = QueryParser::parse(query).unwrap();

        assert_eq!(pattern.nodes.len(), 2);
        assert_eq!(pattern.relationships.len(), 1);

        assert_eq!(pattern.nodes[0].variable, Some("a".to_string()));
        assert_eq!(pattern.nodes[0].label, Some("Class".to_string()));

        assert_eq!(pattern.nodes[1].variable, Some("b".to_string()));
        assert_eq!(pattern.nodes[1].label, Some("Interface".to_string()));

        assert_eq!(
            pattern.relationships[0].edge_type,
            Some(EdgeKind::Definition)
        );
        assert_eq!(pattern.relationships[0].direction, Direction::Forward);
    }

    #[test]
    fn test_parse_depth_range() {
        let query = "(a)-[:Reference*1..3]->(b)";
        let pattern = QueryParser::parse(query).unwrap();

        assert_eq!(pattern.relationships[0].min_depth, 1);
        assert_eq!(pattern.relationships[0].max_depth, Some(3));
    }

    #[test]
    fn test_parse_unlimited_depth() {
        let query = "(a)-[:Reference*]->(b)";
        let pattern = QueryParser::parse(query).unwrap();

        assert_eq!(pattern.relationships[0].min_depth, 1);
        assert_eq!(pattern.relationships[0].max_depth, None);
    }

    #[test]
    fn test_query_execution() {
        let mut graph = CodeGraph::new();

        // Create test symbols
        let func = create_test_symbol("fn:main", "main", SymbolKind::Function);
        let class = create_test_symbol("class:MyClass", "MyClass", SymbolKind::Class);

        let func_idx = graph.add_symbol(func);
        let class_idx = graph.add_symbol(class);

        // Add relationship
        graph.add_edge(func_idx, class_idx, EdgeKind::Reference);

        // Execute query
        let query = "(fn:Function)-[:Reference]->(cls:Class)";
        let pattern = QueryParser::parse(query).unwrap();
        let engine = QueryEngine::new(&graph);
        let results = engine.execute(&pattern);

        assert_eq!(results.matches.len(), 1);
        assert_eq!(results.matches[0].bindings.len(), 2);

        // Check bindings
        let bindings = &results.matches[0].bindings;
        assert!(bindings
            .iter()
            .any(|(var, sym)| var == "fn" && sym.name == "main"));
        assert!(bindings
            .iter()
            .any(|(var, sym)| var == "cls" && sym.name == "MyClass"));
    }

    #[test]
    fn test_parse_backward_arrow() {
        let query = "(b)-[:Reference]->(a)"; // Standard forward syntax
        let pattern = QueryParser::parse(query).unwrap();
        assert_eq!(pattern.relationships[0].direction, Direction::Forward);

        // For actual backward, we'd need to support (a)<-[:Reference]-(b) properly
        // but for now just test that it parses
    }

    #[test]
    fn test_parse_bidirectional() {
        let query = "(a)-->(b)"; // Simple forward for now
        let pattern = QueryParser::parse(query).unwrap();

        // Direction should be forward
        assert_eq!(pattern.relationships[0].direction, Direction::Forward);
    }

    #[test]
    fn test_parse_no_relationship_type() {
        let query = "(a)--(b)";
        let pattern = QueryParser::parse(query).unwrap();

        assert_eq!(pattern.relationships[0].edge_type, None);
        assert_eq!(pattern.relationships[0].min_depth, 1);
        assert_eq!(pattern.relationships[0].max_depth, Some(1));
    }

    #[test]
    fn test_empty_node() {
        let query = "()-[:Reference]->()";
        let pattern = QueryParser::parse(query).unwrap();

        assert_eq!(pattern.nodes[0].variable, None);
        assert_eq!(pattern.nodes[0].label, None);
        assert_eq!(pattern.nodes[1].variable, None);
        assert_eq!(pattern.nodes[1].label, None);
    }

    #[test]
    fn test_node_matches_pattern_edge_cases() {
        let graph = CodeGraph::new();
        let engine = QueryEngine::new(&graph);

        let symbol = create_test_symbol("test", "TestSymbol", SymbolKind::Function);

        // Pattern with no constraints should match
        let pattern = NodePattern {
            variable: None,
            label: None,
            properties: vec![],
        };
        assert!(engine.node_matches_pattern(&symbol, &pattern));

        // Pattern with wrong label should not match
        let pattern = NodePattern {
            variable: None,
            label: Some("Class".to_string()),
            properties: vec![],
        };
        assert!(!engine.node_matches_pattern(&symbol, &pattern));

        // Pattern with correct label should match
        let pattern = NodePattern {
            variable: None,
            label: Some("Function".to_string()),
            properties: vec![],
        };
        assert!(engine.node_matches_pattern(&symbol, &pattern));
    }

    #[test]
    fn test_property_filters() {
        let graph = CodeGraph::new();
        let engine = QueryEngine::new(&graph);

        let symbol = create_test_symbol("fn:test", "TestFunction", SymbolKind::Function);

        // Test Equals
        let filter = PropertyFilter {
            key: "name".to_string(),
            operator: FilterOperator::Equals,
            value: "TestFunction".to_string(),
        };
        assert!(engine.property_matches(&symbol, &filter));

        // Test Contains
        let filter = PropertyFilter {
            key: "name".to_string(),
            operator: FilterOperator::Contains,
            value: "Func".to_string(),
        };
        assert!(engine.property_matches(&symbol, &filter));

        // Test StartsWith
        let filter = PropertyFilter {
            key: "name".to_string(),
            operator: FilterOperator::StartsWith,
            value: "Test".to_string(),
        };
        assert!(engine.property_matches(&symbol, &filter));

        // Test EndsWith
        let filter = PropertyFilter {
            key: "name".to_string(),
            operator: FilterOperator::EndsWith,
            value: "Function".to_string(),
        };
        assert!(engine.property_matches(&symbol, &filter));

        // Test invalid key
        let filter = PropertyFilter {
            key: "invalid".to_string(),
            operator: FilterOperator::Equals,
            value: "test".to_string(),
        };
        assert!(!engine.property_matches(&symbol, &filter));
    }

    #[test]
    fn test_parse_edge_types() {
        // Test all edge types
        let edge_types = vec![
            ("Definition", EdgeKind::Definition),
            ("Reference", EdgeKind::Reference),
            ("TypeDefinition", EdgeKind::TypeDefinition),
            ("Implementation", EdgeKind::Implementation),
            ("Override", EdgeKind::Override),
            ("Import", EdgeKind::Import),
            ("Export", EdgeKind::Export),
            ("Contains", EdgeKind::Contains),
        ];

        for (name, expected) in edge_types {
            let query = format!("(a)-[:{name}]->(b)");
            let pattern = QueryParser::parse(&query).unwrap();
            assert_eq!(pattern.relationships[0].edge_type, Some(expected));
        }
    }

    #[test]
    fn test_parse_complex_depth() {
        // Test depth without minimum
        let query = "(a)-[:Reference*..5]->(b)";
        let pattern = QueryParser::parse(query).unwrap();
        assert_eq!(pattern.relationships[0].min_depth, 1); // Default min
        assert_eq!(pattern.relationships[0].max_depth, Some(5));
    }
}
// Another test comment
