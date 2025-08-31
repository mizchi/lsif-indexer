//! LSP Integration Layer
//! 
//! This crate provides LSP client implementations and language adapters.

// アダプタモジュール（集約）
pub mod adapter;

// LSP clients
pub mod lsp_client;
pub mod lsp_indexer;
pub mod lsp_commands;
pub mod lsp_helpers;
pub mod lsp_integration;
pub mod lsp_features;
pub mod lsp_pool;
pub mod lsp_health_check;

// その他のモジュール
pub mod language_detector;
pub mod fallback_indexer;
pub mod timeout_predictor;

// Re-exports from adapter module
pub use adapter::{
    CommonAdapter, GenericLspClient, GoAdapter, JavaScriptAdapter, LanguageAdapter, LspAdapter,
    PythonAdapter, PythonLspAdapter, RustAnalyzerAdapter,
    RustLspAdapter, TypeScriptAdapter, TypeScriptLspAdapter, detect_language,
    detect_minimal_language,
};

// Other re-exports
pub use language_detector::Language;
pub use lsp_client::LspClient;
pub use lsp_indexer::LspIndexer;
pub use fallback_indexer::{FallbackIndexer, FallbackLanguage};
pub use timeout_predictor::{TimeoutPredictor, PredictorStatistics};