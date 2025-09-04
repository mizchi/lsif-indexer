//! LSIF Indexer Core Library
//!
//! This crate provides the core logic for code indexing and graph operations.

pub mod call_hierarchy;
pub mod definition_chain;
pub mod fuzzy_search;
pub mod graph;
pub mod graph_builder;
pub mod graph_query;
pub mod graph_serde;
pub mod incremental;
pub mod lsif;
pub mod parallel;
pub mod test_fixtures;
pub mod type_relations;

// パフォーマンス検証用（本番では標準実装を使用）
#[cfg(feature = "experimental-optimizations")]
pub mod interned_graph;
#[cfg(feature = "experimental-optimizations")]
pub mod memory_pool;
#[cfg(feature = "experimental-optimizations")]
pub mod optimized_graph;
#[cfg(feature = "experimental-optimizations")]
pub mod string_interner;

// Re-export main types
pub use call_hierarchy::CallHierarchy;
pub use fuzzy_search::{FuzzySearchIndex, MatchType, SearchResult};
pub use graph::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
pub use graph_builder::GraphBuilder;
pub use graph_query::{
    NodePattern, PropertyFilter, QueryPattern, QueryResult, RelationshipPattern,
};
pub use incremental::IncrementalIndex;
pub use lsif::LsifGenerator;
pub use type_relations::TypeRelations;

// Utility functions
pub use incremental::calculate_file_hash; // Another test
