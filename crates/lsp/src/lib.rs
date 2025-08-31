//! LSP Integration Layer
//! 
//! This crate provides LSP client implementations and language adapters.

// アダプタモジュール（集約）
pub mod adapter;

// LSP clients
pub mod lsp_client;
pub mod lsp_minimal_client;
pub mod lsp_indexer;
pub mod lsp_commands;
pub mod lsp_helpers;
pub mod lsp_integration;
pub mod lsp_features;

// その他のモジュール
pub mod language_detector;
pub mod fallback_indexer;

// Re-exports from adapter module
pub use adapter::{
    CommonAdapter, GenericLspClient, GoAdapter, JavaScriptAdapter, LanguageAdapter, LspAdapter,
    MinimalLanguageAdapter, PythonAdapter, PythonLspAdapter, RustAnalyzerAdapter,
    RustLspAdapter, TypeScriptAdapter, TypeScriptLspAdapter, detect_language,
    detect_minimal_language,
};

// Other re-exports
pub use language_detector::Language;
pub use lsp_minimal_client::MinimalLspClient;
pub use lsp_indexer::LspIndexer;
pub use fallback_indexer::{FallbackIndexer, FallbackLanguage};