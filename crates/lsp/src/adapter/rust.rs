use super::common::CommonAdapter;
use lsp_types::SymbolKind;

/// Rust言語用のLSPアダプタ
pub struct RustAdapter {
    common: CommonAdapter,
}

impl Default for RustAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RustAdapter {
    pub fn new() -> Self {
        Self {
            common: CommonAdapter::new("rust", "rust-analyzer", vec!["rs"], vec!["//", "/*", "*/"]),
        }
    }

    pub fn get_adapter(&self) -> &CommonAdapter {
        &self.common
    }

    /// Rust固有の定義パターン
    pub fn get_definition_keywords(&self) -> Vec<&str> {
        vec![
            "fn", "struct", "enum", "trait", "impl", "mod", "type", "const", "static", "macro",
        ]
    }

    /// Rust固有の参照パターン
    pub fn get_reference_patterns(&self) -> Vec<&str> {
        vec![
            r"\b{}\s*\(",            // 関数呼び出し
            r"\b{}::",               // モジュールパス
            r"::{}\b",               // use文
            r"\b{}\s*\{{",           // 構造体初期化
            r"<\s*{}\s*>",           // ジェネリクス
            r":\s*{}\b",             // 型注釈
            r"impl\s+.*\s+for\s+{}", // trait実装
            r"as\s+{}\b",            // 型キャスト
        ]
    }

    /// Rustのシンボル種別を判定
    pub fn infer_symbol_kind(&self, context: &str) -> SymbolKind {
        let trimmed = context.trim();
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
            SymbolKind::FUNCTION
        } else if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") {
            SymbolKind::STRUCT
        } else if trimmed.starts_with("enum ") || trimmed.starts_with("pub enum ") {
            SymbolKind::ENUM
        } else if trimmed.starts_with("trait ") || trimmed.starts_with("pub trait ") {
            SymbolKind::INTERFACE
        } else if trimmed.starts_with("impl ") {
            SymbolKind::CLASS
        } else if trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") {
            SymbolKind::MODULE
        } else if trimmed.starts_with("type ") || trimmed.starts_with("pub type ") {
            SymbolKind::TYPE_PARAMETER
        } else if trimmed.starts_with("const ") || trimmed.starts_with("pub const ") {
            SymbolKind::CONSTANT
        } else if trimmed.starts_with("static ") || trimmed.starts_with("pub static ") {
            SymbolKind::VARIABLE
        } else if trimmed.starts_with("macro_rules!") {
            SymbolKind::FUNCTION
        } else {
            SymbolKind::VARIABLE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_adapter_basic_info() {
        let adapter = RustAdapter::new();
        assert_eq!(adapter.get_adapter().language_id, "rust");
        assert_eq!(adapter.get_adapter().lsp_server_name, "rust-analyzer");
        assert!(adapter
            .get_adapter()
            .file_extensions
            .contains(&"rs".to_string()));
    }

    #[test]
    fn test_rust_definition_keywords() {
        let adapter = RustAdapter::new();
        let keywords = adapter.get_definition_keywords();
        assert!(keywords.contains(&"fn"));
        assert!(keywords.contains(&"struct"));
        assert!(keywords.contains(&"trait"));
        assert!(keywords.contains(&"impl"));
    }

    #[test]
    fn test_rust_symbol_kind_inference() {
        let adapter = RustAdapter::new();
        assert_eq!(adapter.infer_symbol_kind("fn main()"), SymbolKind::FUNCTION);
        assert_eq!(
            adapter.infer_symbol_kind("pub struct User"),
            SymbolKind::STRUCT
        );
        assert_eq!(
            adapter.infer_symbol_kind("trait Display"),
            SymbolKind::INTERFACE
        );
        assert_eq!(
            adapter.infer_symbol_kind("impl Display for User"),
            SymbolKind::CLASS
        );
        assert_eq!(adapter.infer_symbol_kind("mod utils"), SymbolKind::MODULE);
    }
}
