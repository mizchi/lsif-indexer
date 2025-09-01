//! CLI and IO Layer
//!
//! This crate provides the command-line interface, storage, and IO operations.

// CLI components
pub mod adaptive_parallel;
pub mod call_hierarchy_cmd;
pub mod differential_indexer;
pub mod indexer;
pub mod parallel_processor;
pub mod reference_finder;
pub mod cli;
pub mod lsp_unified_cli;
pub mod symbol_extraction_strategy;
pub mod workspace_symbol_strategy;

// Storage layer
pub mod storage;

// Utilities
pub mod fuzzy_search;
pub mod generic_helpers;
pub mod git_diff;

// Re-exports
pub use differential_indexer::{DifferentialIndexer, DifferentialIndexResult, SymbolSummary};
pub use indexer::Indexer;
pub use cli::Cli;
pub use storage::IndexStorage;
