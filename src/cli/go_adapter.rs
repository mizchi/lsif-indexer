use std::process::{Command, Child, Stdio};
use anyhow::Result;
use crate::cli::minimal_language_adapter::{MinimalLanguageAdapter, CommentStyles};

/// Go言語のアダプタ実装
/// goplsを使用してGo言語のコードを解析
pub struct GoAdapter;

impl MinimalLanguageAdapter for GoAdapter {
    fn language_id(&self) -> &str {
        "go"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["go"]
    }

    fn spawn_lsp_command(&self) -> Result<Child> {
        // goplsコマンドを起動
        let child = Command::new("gopls")
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Ok(child)
    }

    fn comment_styles(&self) -> CommentStyles {
        CommentStyles {
            line_comment: vec!["//"],
            block_comment: vec![("/*", "*/")],
        }
    }
}

impl GoAdapter {
    /// Goの定義キーワードかどうかを判定
    pub fn is_definition_keyword(&self, keyword: &str) -> bool {
        matches!(
            keyword,
            "func" | "var" | "const" | "type" | "struct" | "interface" | "package"
        )
    }
    
    /// Go特有の参照パターンを構築
    pub fn build_reference_pattern(&self, name: &str, is_package: bool) -> String {
        if is_package {
            // パッケージ参照の場合、ドットチェーンを考慮
            format!(r"\b{}(?:\.\w+)*\b", regex::escape(name))
        } else {
            // 通常の識別子
            format!(r"\b{}\b", regex::escape(name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_adapter_basic_info() {
        let adapter = GoAdapter;
        assert_eq!(adapter.language_id(), "go");
        assert_eq!(adapter.supported_extensions(), vec!["go"]);
    }

    #[test]
    fn test_go_reference_patterns() {
        let adapter = GoAdapter;

        // 通常の識別子
        let pattern = adapter.build_reference_pattern("main", false);
        assert_eq!(pattern, r"\bmain\b");

        // パッケージ参照（ドットチェーン対応）
        let pattern = adapter.build_reference_pattern("fmt", true);
        assert_eq!(pattern, r"\bfmt(?:\.\w+)*\b");
    }

    #[test]
    fn test_go_definition_keywords() {
        let adapter = GoAdapter;
        assert!(adapter.is_definition_keyword("func"));
        assert!(adapter.is_definition_keyword("type"));
        assert!(adapter.is_definition_keyword("var"));
        assert!(adapter.is_definition_keyword("const"));
        assert!(adapter.is_definition_keyword("struct"));
        assert!(adapter.is_definition_keyword("interface"));
        assert!(!adapter.is_definition_keyword("if"));
        assert!(!adapter.is_definition_keyword("for"));
    }
}