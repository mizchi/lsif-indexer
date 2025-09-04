// LanguageAdapterトレイトは language.rs から使用

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

/// ファイル拡張子から言語を検出（後方互換性のため）
pub fn detect_minimal_language(file_path: &str) -> Option<String> {
    let extension = std::path::Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())?;

    match extension {
        "rs" => Some("rust".to_string()),
        "ts" | "tsx" => Some("typescript".to_string()),
        "js" | "jsx" => Some("javascript".to_string()),
        "py" | "pyi" => Some("python".to_string()),
        "go" => Some("go".to_string()),
        "java" => Some("java".to_string()),
        _ => None,
    }
}
