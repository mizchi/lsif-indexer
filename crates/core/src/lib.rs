//! LSIF Indexer Core Library
//! 
//! This crate provides the core logic for code indexing and graph operations.

pub mod graph;
pub mod graph_builder;
pub mod graph_query;
pub mod graph_serde;
pub mod incremental;
pub mod lsif;
pub mod call_hierarchy;
pub mod type_relations;
pub mod definition_chain;
pub mod parallel;

// Re-export main types
pub use graph::{CodeGraph, EdgeKind, Symbol, SymbolKind, Position, Range};
pub use graph_builder::GraphBuilder;
pub use graph_query::{QueryPattern, NodePattern, RelationshipPattern, PropertyFilter, QueryResult};
pub use incremental::IncrementalIndex;
pub use lsif::LsifGenerator;
pub use call_hierarchy::CallHierarchy;
pub use type_relations::TypeRelations;

// Utility functions
pub use incremental::calculate_file_hash;