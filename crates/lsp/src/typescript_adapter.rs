use crate::common_adapter::{c_style_comments, spawn_lsp_server};
use crate::minimal_language_adapter::{CommentStyles, MinimalLanguageAdapter};
use anyhow::Result;

/// TypeScript/JavaScript言語のアダプタ実装
/// typescript-language-serverを使用してTS/JSコードを解析
pub struct TypeScriptAdapter {
    /// JavaScriptのみをサポートするかどうか
    js_only: bool,
}

impl TypeScriptAdapter {
    /// TypeScript/JavaScript両方をサポートするアダプタを作成
    pub fn new() -> Self {
        Self { js_only: false }
    }

    /// JavaScriptのみをサポートするアダプタを作成
    pub fn javascript_only() -> Self {
        Self { js_only: true }
    }

    /// TypeScriptの定義キーワードかどうかを判定
    pub fn is_definition_keyword(&self, keyword: &str) -> bool {
        let js_keywords = matches!(
            keyword,
            "function" | "const" | "let" | "var" | "class" | "async" | "import" | "export"
        );

        if self.js_only {
            js_keywords
        } else {
            js_keywords
                || matches!(
                    keyword,
                    "interface" | "type" | "enum" | "namespace" | "module" | "declare"
                )
        }
    }

    /// TypeScript/JavaScript特有の参照パターンを構築
    pub fn build_reference_pattern(&self, name: &str, is_module: bool) -> String {
        if is_module {
            // モジュール参照の場合、ドットチェーンやimport文を考慮
            format!(
                r#"(?:import\s+.*\s+from\s+['"]{}['"]|\b{}(?:\.\w+)*\b)"#,
                regex::escape(name),
                regex::escape(name)
            )
        } else {
            // 通常の識別子（オプショナルチェーンも考慮）
            format!(r"\b{}(?:\??\.\w+)*\b", regex::escape(name))
        }
    }
}

impl Default for TypeScriptAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimalLanguageAdapter for TypeScriptAdapter {
    fn language_id(&self) -> &str {
        if self.js_only {
            "javascript"
        } else {
            "typescript"
        }
    }

    fn supported_extensions(&self) -> Vec<&str> {
        if self.js_only {
            vec!["js", "jsx", "mjs", "cjs"]
        } else {
            vec!["ts", "tsx", "js", "jsx", "mjs", "cjs", "d.ts"]
        }
    }

    fn spawn_lsp_command(&self) -> Result<std::process::Child> {
        spawn_lsp_server("typescript-language-server", &["--stdio"])
    }

    fn comment_styles(&self) -> CommentStyles {
        // 基本的にC言語スタイルだが、JSDocも考慮
        let mut styles = c_style_comments();
        styles.block_comment.push(("/**", "*/"));
        styles
    }
}

/// JavaScriptアダプタ（TypeScriptAdapterのエイリアス）
#[derive(Default)]
pub struct JavaScriptAdapter;

impl JavaScriptAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn create_typescript_adapter() -> TypeScriptAdapter {
        TypeScriptAdapter::javascript_only()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typescript_adapter_basic_info() {
        let adapter = TypeScriptAdapter::new();
        assert_eq!(adapter.language_id(), "typescript");
        assert!(adapter.supported_extensions().contains(&"ts"));
        assert!(adapter.supported_extensions().contains(&"tsx"));
        assert!(adapter.supported_extensions().contains(&"js"));
    }

    #[test]
    fn test_javascript_adapter_basic_info() {
        let adapter = TypeScriptAdapter::javascript_only();
        assert_eq!(adapter.language_id(), "javascript");
        assert!(adapter.supported_extensions().contains(&"js"));
        assert!(adapter.supported_extensions().contains(&"jsx"));
        assert!(!adapter.supported_extensions().contains(&"ts"));
    }

    #[test]
    fn test_typescript_reference_patterns() {
        let adapter = TypeScriptAdapter::new();

        // 通常の識別子（オプショナルチェーン対応）
        let pattern = adapter.build_reference_pattern("user", false);
        assert!(pattern.contains(r"\buser"));
        assert!(pattern.contains(r"??\."));

        // モジュール参照（import文も考慮）
        let pattern = adapter.build_reference_pattern("react", true);
        assert!(pattern.contains("import"));
        assert!(pattern.contains(r"\breact"));
    }

    #[test]
    fn test_typescript_definition_keywords() {
        let adapter = TypeScriptAdapter::new();

        // JavaScript共通キーワード
        assert!(adapter.is_definition_keyword("function"));
        assert!(adapter.is_definition_keyword("class"));
        assert!(adapter.is_definition_keyword("const"));

        // TypeScript固有キーワード
        assert!(adapter.is_definition_keyword("interface"));
        assert!(adapter.is_definition_keyword("type"));
        assert!(adapter.is_definition_keyword("enum"));

        // 制御構文（定義ではない）
        assert!(!adapter.is_definition_keyword("if"));
        assert!(!adapter.is_definition_keyword("for"));
    }

    #[test]
    fn test_javascript_definition_keywords() {
        let adapter = TypeScriptAdapter::javascript_only();

        // JavaScript共通キーワード
        assert!(adapter.is_definition_keyword("function"));
        assert!(adapter.is_definition_keyword("class"));

        // TypeScript固有キーワード（JSモードでは定義として扱わない）
        assert!(!adapter.is_definition_keyword("interface"));
        assert!(!adapter.is_definition_keyword("type"));
    }

    #[test]
    fn test_comment_styles() {
        let adapter = TypeScriptAdapter::new();
        let styles = adapter.comment_styles();
        assert_eq!(styles.line_comment, vec!["//"]);
        assert_eq!(styles.block_comment.len(), 2); // /* */ と /** */
    }
}
