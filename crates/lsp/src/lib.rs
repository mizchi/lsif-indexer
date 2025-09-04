//! LSP Integration Layer
//! 
//! This crate provides LSP client implementations and language adapters.

// アダプタモジュール（集約）
pub mod adapter;

// LSP clients
pub mod auto_switching_client;
pub mod lsp_client;
pub mod lsp_indexer;
pub mod lsp_commands;
pub mod lsp_helpers;
pub mod lsp_integration;
pub mod lsp_features;
pub mod lsp_pool;
pub mod lsp_health_check;
pub mod lsp_rpc_client;
pub mod lsp_manager;
pub mod unified_indexer;
pub mod lsp_performance_benchmark;
pub mod hierarchical_cache;
pub mod lsp_metrics;

// その他のモジュール
pub mod language_detector;
pub mod fallback_indexer;
pub mod timeout_predictor;
pub mod regex_cache;
pub mod optimized_io;
pub mod language_optimization;

// テスト用ユーティリティ
#[cfg(test)]
pub mod test_utils;

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
pub use lsp_rpc_client::LspRpcClient;
pub use lsp_manager::{UnifiedLspManager, LspServerConfig, LspServerRegistry, ProjectIndex};
pub use unified_indexer::{UnifiedIndexer, IndexResult};
pub use language_optimization::{LanguageOptimization, OptimizationStrategy, ProjectOptimizationConfig};