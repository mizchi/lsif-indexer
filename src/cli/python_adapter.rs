use std::process::{Command, Child, Stdio};
use anyhow::Result;
use crate::cli::minimal_language_adapter::{MinimalLanguageAdapter, CommentStyles};

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
        let lsp_server = if Self::is_command_available("pyright-langserver") {
            "pyright".to_string()
        } else if Self::is_command_available("pylsp") {
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
    
    /// コマンドが利用可能かチェック
    fn is_command_available(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
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

    fn spawn_lsp_command(&self) -> Result<Child> {
        let child = match self.lsp_server.as_str() {
            "pyright" => {
                // Pyrightサーバーを起動
                Command::new("pyright-langserver")
                    .arg("--stdio")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?
            }
            "pylsp" | _ => {
                // Python LSPサーバーを起動
                Command::new("pylsp")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?
            }
        };
        Ok(child)
    }

    fn comment_styles(&self) -> CommentStyles {
        CommentStyles {
            line_comment: vec!["#"],
            block_comment: vec![("\"\"\"", "\"\"\""), ("'''", "'''")],
        }
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