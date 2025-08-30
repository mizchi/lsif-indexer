/// 最小限の言語アダプター実装
///
/// 言語非依存設計に基づき、LSPサーバーの起動と基本情報のみを提供
use anyhow::Result;
use std::process::{Child, Command, Stdio};

/// 最小限の言語アダプタートレイト
/// LSPサーバーの起動と基本的な言語情報のみを提供
pub trait MinimalLanguageAdapter: Send + Sync {
    /// 言語ID（例: "rust", "typescript"）
    fn language_id(&self) -> &str;

    /// サポートする拡張子
    fn supported_extensions(&self) -> Vec<&str>;

    /// LSPサーバーを起動
    fn spawn_lsp_command(&self) -> Result<Child>;

    /// コメントスタイル（オプション）
    fn comment_styles(&self) -> CommentStyles {
        CommentStyles::default()
    }

    /// ソースファイルかを判定
    fn is_source_file(&self, path: &std::path::Path) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                return self.supported_extensions().contains(&ext_str);
            }
        }
        false
    }
}

/// コメントスタイル定義
#[derive(Debug, Clone)]
pub struct CommentStyles {
    pub line_comment: Vec<&'static str>,
    pub block_comment: Vec<(&'static str, &'static str)>,
}

impl Default for CommentStyles {
    fn default() -> Self {
        Self {
            line_comment: vec!["//"],
            block_comment: vec![("/*", "*/")],
        }
    }
}

// ==================== 言語実装 ====================

/// Rust言語アダプター
pub struct RustLanguageAdapter;

impl MinimalLanguageAdapter for RustLanguageAdapter {
    fn language_id(&self) -> &str {
        "rust"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["rs"]
    }

    fn spawn_lsp_command(&self) -> Result<Child> {
        Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start rust-analyzer: {}", e))
    }

    fn comment_styles(&self) -> CommentStyles {
        CommentStyles {
            line_comment: vec!["//", "///", "//!"],
            block_comment: vec![("/*", "*/"), ("/**", "*/"), ("/*!", "*/")],
        }
    }
}

/// TypeScript言語アダプター
pub struct TypeScriptLanguageAdapter;

impl MinimalLanguageAdapter for TypeScriptLanguageAdapter {
    fn language_id(&self) -> &str {
        "typescript"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["ts", "tsx", "js", "jsx"]
    }

    fn spawn_lsp_command(&self) -> Result<Child> {
        Command::new("npx")
            .arg("-y")
            .arg("@typescript/native-preview")
            .arg("--lsp")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start TypeScript LSP: {}", e))
    }

    fn comment_styles(&self) -> CommentStyles {
        CommentStyles {
            line_comment: vec!["//"],
            block_comment: vec![("/*", "*/"), ("/**", "*/")],
        }
    }
}

/// Python言語アダプター（新規追加）
pub struct PythonLanguageAdapter;

impl MinimalLanguageAdapter for PythonLanguageAdapter {
    fn language_id(&self) -> &str {
        "python"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["py", "pyi"]
    }

    fn spawn_lsp_command(&self) -> Result<Child> {
        Command::new("pylsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .or_else(|_| {
                // fallback to pyls
                Command::new("pyls")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
            })
            .map_err(|e| anyhow::anyhow!("Failed to start Python LSP: {}", e))
    }

    fn comment_styles(&self) -> CommentStyles {
        CommentStyles {
            line_comment: vec!["#"],
            block_comment: vec![("'''", "'''"), ("\"\"\"", "\"\"\"")],
        }
    }
}

/// Go言語アダプター（新規追加）
pub struct GoLanguageAdapter;

impl MinimalLanguageAdapter for GoLanguageAdapter {
    fn language_id(&self) -> &str {
        "go"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["go"]
    }

    fn spawn_lsp_command(&self) -> Result<Child> {
        Command::new("gopls")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start gopls: {}", e))
    }

    fn comment_styles(&self) -> CommentStyles {
        CommentStyles {
            line_comment: vec!["//"],
            block_comment: vec![("/*", "*/")],
        }
    }
}

/// Java言語アダプター（新規追加）
pub struct JavaLanguageAdapter;

impl MinimalLanguageAdapter for JavaLanguageAdapter {
    fn language_id(&self) -> &str {
        "java"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["java"]
    }

    fn spawn_lsp_command(&self) -> Result<Child> {
        Command::new("jdtls")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start Eclipse JDT Language Server: {}", e))
    }

    fn comment_styles(&self) -> CommentStyles {
        CommentStyles {
            line_comment: vec!["//"],
            block_comment: vec![("/*", "*/"), ("/**", "*/")],
        }
    }
}

/// ファイル拡張子から言語アダプターを検出
pub fn detect_language_adapter(file_path: &str) -> Option<Box<dyn MinimalLanguageAdapter>> {
    let extension = std::path::Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())?;

    match extension {
        "rs" => Some(Box::new(RustLanguageAdapter)),
        "ts" | "tsx" | "js" | "jsx" => Some(Box::new(TypeScriptLanguageAdapter)),
        "py" | "pyi" => Some(Box::new(PythonLanguageAdapter)),
        "go" => Some(Box::new(GoLanguageAdapter)),
        "java" => Some(Box::new(JavaLanguageAdapter)),
        _ => None,
    }
}

/// サポートされている言語の一覧を取得
pub fn supported_languages() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("rust", vec!["rs"]),
        ("typescript", vec!["ts", "tsx", "js", "jsx"]),
        ("python", vec!["py", "pyi"]),
        ("go", vec!["go"]),
        ("java", vec!["java"]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_adapter() {
        let adapter = RustLanguageAdapter;
        assert_eq!(adapter.language_id(), "rust");
        assert_eq!(adapter.supported_extensions(), vec!["rs"]);
        assert_eq!(adapter.comment_styles().line_comment[0], "//");
    }

    #[test]
    fn test_typescript_adapter() {
        let adapter = TypeScriptLanguageAdapter;
        assert_eq!(adapter.language_id(), "typescript");
        assert!(adapter.supported_extensions().contains(&"ts"));
        assert!(adapter.supported_extensions().contains(&"tsx"));
    }

    #[test]
    fn test_python_adapter() {
        let adapter = PythonLanguageAdapter;
        assert_eq!(adapter.language_id(), "python");
        assert_eq!(adapter.supported_extensions(), vec!["py", "pyi"]);
        assert_eq!(adapter.comment_styles().line_comment[0], "#");
    }

    #[test]
    fn test_detect_language() {
        assert!(detect_language_adapter("main.rs").is_some());
        assert!(detect_language_adapter("index.ts").is_some());
        assert!(detect_language_adapter("script.py").is_some());
        assert!(detect_language_adapter("main.go").is_some());
        assert!(detect_language_adapter("App.java").is_some());
        assert!(detect_language_adapter("unknown.xyz").is_none());
    }

    #[test]
    fn test_supported_languages() {
        let langs = supported_languages();
        assert!(langs.len() >= 5);
        assert!(langs.iter().any(|(id, _)| *id == "rust"));
        assert!(langs.iter().any(|(id, _)| *id == "python"));
    }
}
