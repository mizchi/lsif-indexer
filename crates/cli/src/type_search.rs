use anyhow::Result;
use lsif_core::{CodeGraph, Symbol, SymbolKind};
use petgraph::visit::EdgeRef;
use regex::Regex;

/// Type-based search filters
#[derive(Debug, Clone)]
pub enum TypeFilter {
    /// Returns specified type
    Returns(String),
    /// Takes specified parameter type
    Takes(String),
    /// Implements/extends specified type
    Implements(String),
    /// Has field of specified type
    HasField(String),
    /// Matches type signature pattern
    Signature(String),
}

impl TypeFilter {
    /// Parse filter from string like "--returns Result<String>"
    pub fn from_arg(arg_type: &str, value: &str) -> Result<Self> {
        match arg_type {
            "returns" => Ok(Self::Returns(value.to_string())),
            "takes" | "param" => Ok(Self::Takes(value.to_string())),
            "implements" | "extends" => Ok(Self::Implements(value.to_string())),
            "field" | "has-field" => Ok(Self::HasField(value.to_string())),
            "signature" | "sig" => Ok(Self::Signature(value.to_string())),
            _ => anyhow::bail!("Unknown type filter: {}", arg_type),
        }
    }
}

/// Type-based search engine
pub struct TypeSearchEngine<'a> {
    graph: &'a CodeGraph,
}

impl<'a> TypeSearchEngine<'a> {
    pub fn new(graph: &'a CodeGraph) -> Self {
        Self { graph }
    }

    /// Search symbols by type filters
    pub fn search(&self, filters: &[TypeFilter], max_results: usize) -> Vec<Symbol> {
        let mut results = Vec::new();

        for symbol in self.graph.get_all_symbols() {
            if self.matches_all_filters(symbol, filters) {
                results.push(symbol.clone());
                if results.len() >= max_results {
                    break;
                }
            }
        }

        results
    }

    /// Check if symbol matches all filters
    fn matches_all_filters(&self, symbol: &Symbol, filters: &[TypeFilter]) -> bool {
        filters
            .iter()
            .all(|filter| self.matches_filter(symbol, filter))
    }

    /// Check if symbol matches a single filter
    fn matches_filter(&self, symbol: &Symbol, filter: &TypeFilter) -> bool {
        match filter {
            TypeFilter::Returns(type_name) => self.matches_return_type(symbol, type_name),
            TypeFilter::Takes(type_name) => self.matches_parameter_type(symbol, type_name),
            TypeFilter::Implements(type_name) => self.matches_implements(symbol, type_name),
            TypeFilter::HasField(type_name) => self.matches_field_type(symbol, type_name),
            TypeFilter::Signature(pattern) => self.matches_signature(symbol, pattern),
        }
    }

    /// Check if function/method returns specified type
    fn matches_return_type(&self, symbol: &Symbol, type_name: &str) -> bool {
        // Only check functions and methods
        if !matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method) {
            return false;
        }

        // Check detail field for type information
        if let Some(detail) = &symbol.detail {
            // Look for return type patterns
            // Examples: "fn() -> Result<String>", "async fn() -> impl Future"
            if detail.contains("->") {
                let return_part = detail.split("->").nth(1).unwrap_or("");
                return self.type_matches(return_part, type_name);
            }
        }

        false
    }

    /// Check if function takes specified parameter type
    fn matches_parameter_type(&self, symbol: &Symbol, type_name: &str) -> bool {
        if !matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method) {
            return false;
        }

        if let Some(detail) = &symbol.detail {
            // Extract parameter list from signature
            if let Some(params) = self.extract_parameters(detail) {
                return self.type_matches(&params, type_name);
            }
        }

        false
    }

    /// Check if type implements/extends specified interface/trait
    fn matches_implements(&self, symbol: &Symbol, type_name: &str) -> bool {
        // Check classes, structs, and enums
        if !matches!(
            symbol.kind,
            SymbolKind::Class | SymbolKind::Struct | SymbolKind::Enum
        ) {
            return false;
        }

        // Check detail field for implementation info
        if let Some(detail) = &symbol.detail {
            // Look for patterns like "impl Iterator", "extends Base", ": Trait"
            let patterns = [
                format!("impl {}", type_name),
                format!("implements {}", type_name),
                format!("extends {}", type_name),
                format!(": {}", type_name),
                format!(": dyn {}", type_name),
            ];

            return patterns.iter().any(|pattern| detail.contains(pattern));
        }

        // Also check relationships in graph
        self.check_implementation_edges(symbol, type_name)
    }

    /// Check if struct/class has field of specified type
    fn matches_field_type(&self, symbol: &Symbol, type_name: &str) -> bool {
        if !matches!(symbol.kind, SymbolKind::Class | SymbolKind::Struct) {
            return false;
        }

        // Find all fields/properties of this symbol
        if let Some(node_idx) = self.graph.get_node_index(&symbol.id) {
            for edge in self.graph.graph.edges(node_idx) {
                if matches!(edge.weight(), lsif_core::EdgeKind::Contains) {
                    if let Some(field) = self.graph.graph.node_weight(edge.target()) {
                        if matches!(field.kind, SymbolKind::Field | SymbolKind::Property) {
                            if let Some(detail) = &field.detail {
                                if self.type_matches(detail, type_name) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// Check if symbol matches signature pattern
    fn matches_signature(&self, symbol: &Symbol, pattern: &str) -> bool {
        if let Some(detail) = &symbol.detail {
            // Try regex matching
            if let Ok(re) = Regex::new(pattern) {
                return re.is_match(detail);
            }
            // Fall back to simple contains
            return detail.contains(pattern);
        }
        false
    }

    // Helper methods

    /// Check if a type string matches the search type
    fn type_matches(&self, type_str: &str, search_type: &str) -> bool {
        // Normalize and compare
        let normalized = type_str.trim();

        // Exact match
        if normalized.contains(search_type) {
            return true;
        }

        // Handle generic types
        if search_type.contains('<') {
            // For now, simple contains check
            return normalized.contains(search_type);
        }

        // Handle short names vs full paths
        let search_base = search_type.split("::").last().unwrap_or(search_type);
        normalized
            .split("::")
            .any(|part| part.contains(search_base))
    }

    /// Extract parameter list from function signature
    fn extract_parameters(&self, signature: &str) -> Option<String> {
        // Find content between parentheses
        if let Some(start) = signature.find('(') {
            if let Some(end) = signature.find(')') {
                return Some(signature[start + 1..end].to_string());
            }
        }
        None
    }

    /// Check implementation relationships in graph
    fn check_implementation_edges(&self, symbol: &Symbol, type_name: &str) -> bool {
        if let Some(node_idx) = self.graph.get_node_index(&symbol.id) {
            // Check outgoing Implementation edges
            for edge in self.graph.graph.edges(node_idx) {
                if matches!(edge.weight(), lsif_core::EdgeKind::Implementation) {
                    if let Some(target) = self.graph.graph.node_weight(edge.target()) {
                        if target.name.contains(type_name) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

/// Advanced search combining name and type filters
pub struct AdvancedSearch<'a> {
    graph: &'a CodeGraph,
    type_engine: TypeSearchEngine<'a>,
}

impl<'a> AdvancedSearch<'a> {
    pub fn new(graph: &'a CodeGraph) -> Self {
        Self {
            type_engine: TypeSearchEngine::new(graph),
            graph,
        }
    }

    /// Search with both name pattern and type filters
    pub fn search(
        &self,
        name_pattern: Option<&str>,
        type_filters: &[TypeFilter],
        fuzzy: bool,
        max_results: usize,
    ) -> Vec<Symbol> {
        let mut results = Vec::new();

        for symbol in self.graph.get_all_symbols() {
            // Check name pattern
            if let Some(pattern) = name_pattern {
                if fuzzy {
                    if !symbol.name.to_lowercase().contains(&pattern.to_lowercase()) {
                        continue;
                    }
                } else if !symbol.name.contains(pattern) {
                    continue;
                }
            }

            // Check type filters
            if !type_filters.is_empty()
                && !self.type_engine.matches_all_filters(symbol, type_filters)
            {
                continue;
            }

            results.push(symbol.clone());
            if results.len() >= max_results {
                break;
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_filter_parsing() {
        let filter = TypeFilter::from_arg("returns", "Result<String>").unwrap();
        assert!(matches!(filter, TypeFilter::Returns(s) if s == "Result<String>"));

        let filter = TypeFilter::from_arg("implements", "Iterator").unwrap();
        assert!(matches!(filter, TypeFilter::Implements(s) if s == "Iterator"));
    }
}
