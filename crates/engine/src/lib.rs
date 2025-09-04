//! LSIF Search Engine
//!
//! This crate provides the search and query engine for indexed code.
//! It includes basic search functionality and language-specific adapters.

pub mod adapters;
pub mod fuzzy;
pub mod query;
pub mod search;

// Re-export main types
pub use adapters::{AdapterRegistry, LanguageAdapter};
pub use fuzzy::{FuzzyMatch, FuzzySearcher, MatchType};
pub use query::{QueryEngine, QueryPattern, QueryResult};
pub use search::{SearchEngine, SearchOptions, SearchResult};

use lsif_core::CodeGraph;

/// Main engine that combines all search functionality
pub struct Engine {
    search_engine: SearchEngine,
    query_engine: QueryEngine,
    fuzzy_searcher: FuzzySearcher,
}

impl Engine {
    /// Create a new engine from a code graph
    pub fn new(graph: CodeGraph) -> Self {
        Self {
            search_engine: SearchEngine::new(graph.clone()),
            query_engine: QueryEngine::new(graph.clone()),
            fuzzy_searcher: FuzzySearcher::new(graph),
        }
    }

    /// Get a reference to the search engine
    pub fn search(&self) -> &SearchEngine {
        &self.search_engine
    }

    /// Get a reference to the query engine
    pub fn query(&self) -> &QueryEngine {
        &self.query_engine
    }

    /// Get a reference to the fuzzy searcher
    pub fn fuzzy(&self) -> &FuzzySearcher {
        &self.fuzzy_searcher
    }

    /// Update the engine with a new graph
    pub fn update(&mut self, graph: CodeGraph) {
        self.search_engine = SearchEngine::new(graph.clone());
        self.query_engine = QueryEngine::new(graph.clone());
        self.fuzzy_searcher = FuzzySearcher::new(graph);
    }
}
