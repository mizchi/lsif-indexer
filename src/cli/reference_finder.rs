/// 参照検索の実装
/// 
/// ファイル内容を実際に検索して使用箇所を見つける

use crate::core::{Symbol, SymbolKind, Position, Range};
use anyhow::Result;
use regex::Regex;
use std::path::Path;
use walkdir::WalkDir;

/// 参照の検索結果
#[derive(Debug, Clone)]
pub struct Reference {
    pub symbol: Symbol,
    pub is_definition: bool,
}

/// プロジェクト全体から参照を検索
pub fn find_all_references(
    project_root: &Path,
    target_name: &str,
    target_kind: &SymbolKind,
) -> Result<Vec<Reference>> {
    let mut references = Vec::new();
    
    // 検索パターンを構築
    let pattern = build_search_pattern(target_name, target_kind);
    let regex = Regex::new(&pattern)?;
    
    // 対象ファイルを走査
    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_source_file(e.path()))
    {
        let path = entry.path();
        if let Ok(content) = std::fs::read_to_string(path) {
            let refs = find_references_in_file(path, &content, target_name, &regex)?;
            references.extend(refs);
        }
    }
    
    Ok(references)
}

/// ファイル内の参照を検索
fn find_references_in_file(
    path: &Path,
    content: &str,
    target_name: &str,
    regex: &Regex,
) -> Result<Vec<Reference>> {
    let mut references = Vec::new();
    let path_str = path.to_string_lossy().to_string();
    
    for (line_no, line) in content.lines().enumerate() {
        // 正規表現でマッチを検索
        for mat in regex.find_iter(line) {
            let start_col = mat.start();
            let end_col = mat.end();
            
            // コンテキストから定義か使用かを判定
            let is_definition = is_definition_context(line, start_col);
            
            // 文字列リテラルやコメント内は除外
            if is_in_string_or_comment(line, start_col) {
                continue;
            }
            
            let symbol = Symbol {
                id: format!("{}#{}:{}:{}", path_str, line_no + 1, start_col, target_name),
                kind: if is_definition { 
                    SymbolKind::Unknown 
                } else { 
                    SymbolKind::Unknown 
                },
                name: target_name.to_string(),
                file_path: path_str.clone(),
                range: Range {
                    start: Position {
                        line: line_no as u32,
                        character: start_col as u32,
                    },
                    end: Position {
                        line: line_no as u32,
                        character: end_col as u32,
                    },
                },
                documentation: None,
            };
            
            references.push(Reference {
                symbol,
                is_definition,
            });
        }
    }
    
    Ok(references)
}

/// 検索パターンを構築
fn build_search_pattern(name: &str, kind: &SymbolKind) -> String {
    // 単語境界を考慮したパターン
    let escaped = regex::escape(name);
    
    match kind {
        SymbolKind::Function | SymbolKind::Method => {
            // 関数呼び出しまたは定義
            format!(r"\b{}\s*(?:\(|<)", escaped)
        }
        SymbolKind::Class | SymbolKind::Struct => {
            // 型参照または定義
            format!(r"\b{}\b", escaped)
        }
        SymbolKind::Variable | SymbolKind::Constant => {
            // 変数参照
            format!(r"\b{}\b", escaped)
        }
        _ => {
            // その他は単語境界のみ
            format!(r"\b{}\b", escaped)
        }
    }
}

/// ソースファイルかどうかを判定
fn is_source_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        matches!(
            ext.to_str().unwrap_or(""),
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "cpp" | "c" | "h"
        )
    } else {
        false
    }
}

/// 定義のコンテキストかを判定
fn is_definition_context(line: &str, position: usize) -> bool {
    // 位置より前の部分を取得
    let before = &line[..position.min(line.len())];
    let trimmed = before.trim_end();
    
    // 定義パターン
    trimmed.ends_with("fn")
        || trimmed.ends_with("struct")
        || trimmed.ends_with("class")
        || trimmed.ends_with("const")
        || trimmed.ends_with("let")
        || trimmed.ends_with("var")
        || trimmed.ends_with("type")
        || trimmed.ends_with("interface")
        || trimmed.ends_with("enum")
}

/// 文字列リテラルやコメント内かを判定（簡易版）
fn is_in_string_or_comment(line: &str, position: usize) -> bool {
    let before = &line[..position.min(line.len())];
    
    // コメントチェック
    if before.contains("//") {
        let comment_pos = before.rfind("//").unwrap();
        return position > comment_pos;
    }
    
    // 文字列チェック（簡易版）
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    
    for (_i, ch) in before.chars().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        
        match ch {
            '\\' => escaped = true,
            '"' if !in_char => in_string = !in_string,
            '\'' if !in_string => in_char = !in_char,
            _ => {}
        }
    }
    
    in_string || in_char
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_definition_context() {
        assert!(is_definition_context("fn main() {", 3));
        assert!(is_definition_context("struct User {", 7));
        assert!(is_definition_context("let count = 0;", 4));
        assert!(!is_definition_context("println!(main);", 9));
    }
    
    #[test]
    fn test_is_in_string_or_comment() {
        assert!(is_in_string_or_comment("// this is main", 11));
        assert!(is_in_string_or_comment("\"hello main\"", 7));
        assert!(!is_in_string_or_comment("main(); // comment", 2));
    }
    
    #[test]
    fn test_build_search_pattern() {
        let pattern = build_search_pattern("test", &SymbolKind::Function);
        assert!(pattern.contains("test"));
        assert!(pattern.contains(r"\("));
    }
}