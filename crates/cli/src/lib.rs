//! CLI and IO Layer
//!
//! This crate provides the command-line interface, storage, and IO operations.

// CLI components
pub mod adaptive_parallel;
pub mod batch_graph_updater;
pub mod call_hierarchy_cmd;
pub mod cli;
pub mod commands;
pub mod definition_crawler;
pub mod differential_indexer;
pub mod indexer;
pub mod lsp_unified_cli;
pub mod output_format;
pub mod parallel_processor;
pub mod reference_finder;
pub mod symbol_extraction_strategy;
pub mod type_search;
pub mod workspace_symbol_strategy;

// Storage layer
pub mod storage;

// Utilities
pub mod fast_file_reader;
pub mod fuzzy_search;
pub mod generic_helpers;
pub mod git_diff;

// Re-exports
pub use cli::Cli;
pub use differential_indexer::{DifferentialIndexResult, DifferentialIndexer, SymbolSummary};
pub use indexer::Indexer;
pub use storage::IndexStorage;
// test comment
