//! Basic search functionality

use anyhow::Result;
use lsif_core::{CodeGraph, Position, Symbol, SymbolKind};

/// Search options
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Case sensitive search
    pub case_sensitive: bool,
    /// Maximum number of results
    pub limit: Option<usize>,
    /// File path filter
    pub file_filter: Option<String>,
    /// Symbol kind filter
    pub kind_filter: Option<SymbolKind>,
    /// Include private symbols
    pub include_private: bool,
}

/// Search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub symbol: Symbol,
    pub relevance: f32,
    pub context: Option<String>,
}

/// Basic search engine
pub struct SearchEngine {
    graph: CodeGraph,
}

impl SearchEngine {
    /// Create a new search engine
    pub fn new(graph: CodeGraph) -> Self {
        Self { graph }
    }

    /// Find symbol definitions
    pub fn find_definitions(
        &self,
        name: &str,
        options: SearchOptions,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        for symbol in self.graph.get_all_symbols() {
            if self.matches_name(&symbol.name, name, options.case_sensitive) {
                if let Some(ref filter) = options.file_filter {
                    if !symbol.file_path.contains(filter) {
                        continue;
                    }
                }

                if let Some(ref kind) = options.kind_filter {
                    if symbol.kind != *kind {
                        continue;
                    }
                }

                if !options.include_private && self.is_private_symbol(symbol) {
                    continue;
                }

                results.push(SearchResult {
                    symbol: symbol.clone(),
                    relevance: self.calculate_relevance(&symbol.name, name),
                    context: None,
                });

                if let Some(limit) = options.limit {
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        // Sort by relevance
        results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());

        Ok(results)
    }

    /// Find references to a symbol
    pub fn find_references(
        &self,
        symbol_id: &str,
        options: SearchOptions,
    ) -> Result<Vec<SearchResult>> {
        let references = self.graph.find_references(symbol_id)?;
        let mut results = Vec::new();

        for reference in references {
            if let Some(ref filter) = options.file_filter {
                if !reference.file_path.contains(filter) {
                    continue;
                }
            }

            results.push(SearchResult {
                symbol: reference.clone(),
                relevance: 1.0,
                context: None,
            });

            if let Some(limit) = options.limit {
                if results.len() >= limit {
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Find symbols at a specific position
    pub fn find_at_position(&self, file_path: &str, position: Position) -> Result<Option<Symbol>> {
        self.graph.find_symbol_at_position(file_path, position)
    }

    /// Find symbols in a file
    pub fn find_in_file(&self, file_path: &str) -> Result<Vec<Symbol>> {
        self.graph.get_symbols_in_file(file_path)
    }

    /// Search for symbols by pattern (regex)
    pub fn search_pattern(
        &self,
        pattern: &str,
        options: SearchOptions,
    ) -> Result<Vec<SearchResult>> {
        let regex = regex::Regex::new(pattern)?;
        let mut results = Vec::new();

        for symbol in self.graph.get_all_symbols() {
            if regex.is_match(&symbol.name) {
                if let Some(ref filter) = options.file_filter {
                    if !symbol.file_path.contains(filter) {
                        continue;
                    }
                }

                if let Some(ref kind) = options.kind_filter {
                    if symbol.kind != *kind {
                        continue;
                    }
                }

                results.push(SearchResult {
                    symbol: symbol.clone(),
                    relevance: 1.0,
                    context: None,
                });

                if let Some(limit) = options.limit {
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(results)
    }

    fn matches_name(&self, symbol_name: &str, search_name: &str, case_sensitive: bool) -> bool {
        if case_sensitive {
            symbol_name.contains(search_name)
        } else {
            symbol_name
                .to_lowercase()
                .contains(&search_name.to_lowercase())
        }
    }

    fn calculate_relevance(&self, symbol_name: &str, search_name: &str) -> f32 {
        if symbol_name == search_name {
            2.0
        } else if symbol_name.starts_with(search_name) {
            1.5
        } else if symbol_name.ends_with(search_name) {
            1.2
        } else {
            1.0
        }
    }

    fn is_private_symbol(&self, symbol: &Symbol) -> bool {
        symbol.name.starts_with('_') || symbol.name.starts_with("priv")
    }
}
