use crate::cli::common_adapter::{is_command_available, python_style_comments, spawn_lsp_server};
use crate::cli::minimal_language_adapter::{CommentStyles, MinimalLanguageAdapter};
use anyhow::Result;

/// Python言語のアダプタ実装
/// pylspまたはpyrightを使用してPythonコードを解析
pub struct PythonAdapter {
    /// 使用するLSPサーバー（"pylsp" または "pyright"）
    lsp_server: String,
}

impl PythonAdapter {
    /// 新しいPythonアダプタを作成
    pub fn new() -> Self {
        // pyrightを優先、なければpylspを使用
        let lsp_server = if is_command_available("pyright-langserver") {
            "pyright".to_string()
        } else if is_command_available("pylsp") {
            "pylsp".to_string()
        } else {
            // デフォルトはpyright（インストールを促すため）
            "pyright".to_string()
        };

        Self { lsp_server }
    }

    /// 指定したLSPサーバーでアダプタを作成
    pub fn with_server(lsp_server: &str) -> Self {
        Self {
            lsp_server: lsp_server.to_string(),
        }
    }

    /// Pythonの定義キーワードかどうかを判定
    pub fn is_definition_keyword(&self, keyword: &str) -> bool {
        matches!(
            keyword,
            "def" | "class" | "async" | "lambda" | "import" | "from"
        )
    }

    /// Python特有の参照パターンを構築
    pub fn build_reference_pattern(&self, name: &str, is_module: bool) -> String {
        if is_module {
            // モジュール参照の場合、ドットチェーンを考慮
            format!(r"\b{}(?:\.\w+)*\b", regex::escape(name))
        } else {
            // 通常の識別子
            format!(r"\b{}\b", regex::escape(name))
        }
    }
}

impl Default for PythonAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimalLanguageAdapter for PythonAdapter {
    fn language_id(&self) -> &str {
        "python"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["py", "pyi"]
    }

    fn spawn_lsp_command(&self) -> Result<std::process::Child> {
        // Pythonサーバーの起動前に少し待機（安定性向上のため）
        std::thread::sleep(std::time::Duration::from_millis(100));

        match self.lsp_server.as_str() {
            "pyright" => spawn_lsp_server("pyright-langserver", &["--stdio"]),
            _ => {
                // pylspの場合は、より安全な設定で起動
                spawn_lsp_server("pylsp", &["-v"]).or_else(|_| spawn_lsp_server("pylsp", &[]))
            }
        }
    }

    fn comment_styles(&self) -> CommentStyles {
        python_style_comments()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_adapter_basic_info() {
        let adapter = PythonAdapter::new();
        assert_eq!(adapter.language_id(), "python");
        assert_eq!(adapter.supported_extensions(), vec!["py", "pyi"]);
    }

    #[test]
    fn test_python_reference_patterns() {
        let adapter = PythonAdapter::new();

        // 通常の識別子
        let pattern = adapter.build_reference_pattern("main", false);
        assert_eq!(pattern, r"\bmain\b");

        // モジュール参照（ドットチェーン対応）
        let pattern = adapter.build_reference_pattern("os", true);
        assert_eq!(pattern, r"\bos(?:\.\w+)*\b");
    }

    #[test]
    fn test_python_definition_keywords() {
        let adapter = PythonAdapter::new();
        assert!(adapter.is_definition_keyword("def"));
        assert!(adapter.is_definition_keyword("class"));
        assert!(adapter.is_definition_keyword("async"));
        assert!(adapter.is_definition_keyword("lambda"));
        assert!(adapter.is_definition_keyword("import"));
        assert!(!adapter.is_definition_keyword("if"));
        assert!(!adapter.is_definition_keyword("for"));
    }

    #[test]
    fn test_comment_styles() {
        let adapter = PythonAdapter::new();
        let styles = adapter.comment_styles();
        assert_eq!(styles.line_comment, vec!["#"]);
        assert_eq!(styles.block_comment.len(), 2);
    }
}
