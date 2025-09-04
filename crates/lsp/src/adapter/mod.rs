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
pub mod tsgo;
pub mod typescript;

// Re-exports for convenience
pub use common::CommonAdapter;
pub use language::{LanguageAdapter, RustLanguageAdapter, TypeScriptLanguageAdapter};
pub use lsp::{
    detect_language, GenericLspClient, LspAdapter, RustAnalyzerAdapter, TypeScriptAdapter,
};
pub use minimal::{detect_minimal_language, CommentStyles};

// 言語別アダプタのre-export
pub use go::GoAdapter;
pub use python::PythonAdapter;
pub use python::PythonAdapter as PythonLspAdapter;
pub use rust::RustAdapter as RustLspAdapter;
pub use tsgo::{JavaScriptAdapter as TsgoJavaScriptAdapter, TsgoAdapter, TypeScriptLSAdapter};
pub use typescript::{JavaScriptAdapter, TypeScriptAdapter as TypeScriptLspAdapter};
