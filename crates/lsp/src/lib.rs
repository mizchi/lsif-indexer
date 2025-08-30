//! LSP Integration Layer
//! 
//! This crate provides LSP client implementations and language adapters.

// LSP clients
pub mod lsp_adapter;
pub mod lsp_client;
pub mod lsp_minimal_client;
pub mod lsp_indexer;
pub mod lsp_commands;
pub mod lsp_helpers;
pub mod lsp_integration;
pub mod lsp_features;

// Language adapters
pub mod go_adapter;
pub mod python_adapter;
pub mod typescript_adapter;
pub mod minimal_language_adapter;
pub mod language_adapter;
pub mod language_detector;
pub mod common_adapter;
pub mod fallback_indexer;

// Re-exports
pub use language_detector::Language;
pub use lsp_minimal_client::MinimalLspClient;
pub use lsp_indexer::LspIndexer;