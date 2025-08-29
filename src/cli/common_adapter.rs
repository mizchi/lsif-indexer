use crate::cli::minimal_language_adapter::CommentStyles;
use anyhow::{anyhow, Result};
/// 言語アダプタの共通実装
///
/// 各言語アダプタで重複していた処理を集約
use std::process::{Child, Command, Stdio};

/// LSPサーバー起動の共通実装
pub fn spawn_lsp_server(command: &str, args: &[&str]) -> Result<Child> {
    let mut cmd = Command::new(command);

    // 引数を追加
    for arg in args {
        cmd.arg(arg);
    }

    // 標準的なパイプ設定
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // 起動を試みる
    cmd.spawn()
        .map_err(|e| anyhow!("Failed to spawn LSP server '{}': {}", command, e))
}

/// コマンドの利用可能性をチェック
pub fn is_command_available(cmd: &str) -> bool {
    #[cfg(unix)]
    {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        Command::new("where")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

/// C言語系のコメントスタイル
pub fn c_style_comments() -> CommentStyles {
    CommentStyles {
        line_comment: vec!["//"],
        block_comment: vec![("/*", "*/")],
    }
}

/// Python系のコメントスタイル
pub fn python_style_comments() -> CommentStyles {
    CommentStyles {
        line_comment: vec!["#"],
        block_comment: vec![("\"\"\"", "\"\"\""), ("'''", "'''")],
    }
}

/// 汎用的な参照パターン構築
pub fn build_basic_reference_pattern(name: &str, allow_dot_chain: bool) -> String {
    let escaped = regex::escape(name);
    if allow_dot_chain {
        format!(r"\b{}(?:\.\w+)*\b", escaped)
    } else {
        format!(r"\b{}\b", escaped)
    }
}

/// 定義キーワードのマッチング
pub fn is_common_definition_keyword(keyword: &str, keywords: &[&str]) -> bool {
    keywords.contains(&keyword)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_availability() {
        // 一般的なコマンドでテスト
        #[cfg(unix)]
        assert!(is_command_available("ls"));

        #[cfg(windows)]
        assert!(is_command_available("cmd"));

        // 存在しないコマンド
        assert!(!is_command_available("this_command_does_not_exist_12345"));
    }

    #[test]
    fn test_reference_patterns() {
        assert_eq!(build_basic_reference_pattern("test", false), r"\btest\b");

        assert_eq!(
            build_basic_reference_pattern("test", true),
            r"\btest(?:\.\w+)*\b"
        );
    }

    #[test]
    fn test_comment_styles() {
        let c_style = c_style_comments();
        assert_eq!(c_style.line_comment, vec!["//"]);
        assert_eq!(c_style.block_comment, vec![("/*", "*/")]);

        let py_style = python_style_comments();
        assert_eq!(py_style.line_comment, vec!["#"]);
        assert_eq!(py_style.block_comment.len(), 2);
    }
}
