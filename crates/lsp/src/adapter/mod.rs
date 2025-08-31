/// 言語別アダプタモジュール
/// 
/// 各言語のLSP統合とフォールバック実装を提供
// 共通モジュール
pub mod common;
pub mod language;
pub mod lsp;
pub mod minimal;

// 言語別アダプタ
pub mod go;
pub mod python;
pub mod rust;
pub mod typescript;

// Re-exports for convenience
pub use common::CommonAdapter;
pub use language::LanguageAdapter;
pub use lsp::{
    detect_language, GenericLspClient, LspAdapter, RustAnalyzerAdapter, TypeScriptAdapter,
};
pub use minimal::{
    detect_language_adapter as detect_minimal_language, MinimalLanguageAdapter, 
    PythonLanguageAdapter as PythonAdapter,
    RustLanguageAdapter as MinimalRustAdapter, 
    TypeScriptLanguageAdapter as MinimalTypeScriptAdapter,
};

// 言語別アダプタのre-export
pub use go::GoAdapter;
pub use python::PythonAdapter as PythonLspAdapter;
pub use rust::RustAdapter as RustLspAdapter;
pub use typescript::{JavaScriptAdapter, TypeScriptAdapter as TypeScriptLspAdapter};