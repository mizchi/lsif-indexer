/// 参照検索の実装
/// 
/// ファイル内容を実際に検索して使用箇所を見つける

use crate::core::{Symbol, SymbolKind, Position, Range};
use anyhow::Result;
use regex::Regex;
use std::path::Path;
use walkdir::WalkDir;
use std::collections::HashSet;

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
    let mut seen_locations = HashSet::new();
    
    // 検索パターンを構築
    let pattern = build_search_pattern(target_name, target_kind);
    let regex = Regex::new(&pattern)?;
    
    // 除外するディレクトリ
    let exclude_dirs = vec!["target", ".git", "node_modules", ".vscode"];
    
    // 対象ファイルを走査
    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            // 除外ディレクトリをスキップ
            if let Some(name) = e.file_name().to_str() {
                !exclude_dirs.contains(&name)
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_source_file(e.path()))
    {
        let path = entry.path();
        if let Ok(content) = std::fs::read_to_string(path) {
            let refs = find_references_in_file(path, &content, target_name, target_kind, &regex)?;
            
            // 重複を除外
            for reference in refs {
                let location_key = format!(
                    "{}:{}:{}", 
                    reference.symbol.file_path,
                    reference.symbol.range.start.line,
                    reference.symbol.range.start.character
                );
                
                if seen_locations.insert(location_key) {
                    references.push(reference);
                }
            }
        }
    }
    
    // 結果をソート（ファイル名、行番号順）
    references.sort_by(|a, b| {
        a.symbol.file_path.cmp(&b.symbol.file_path)
            .then(a.symbol.range.start.line.cmp(&b.symbol.range.start.line))
            .then(a.symbol.range.start.character.cmp(&b.symbol.range.start.character))
    });
    
    Ok(references)
}

/// ファイル内の参照を検索
fn find_references_in_file(
    path: &Path,
    content: &str,
    target_name: &str,
    target_kind: &SymbolKind,
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
            let is_definition = is_definition_context(line, start_col) && 
                               matches!(target_kind, SymbolKind::Function | SymbolKind::Method | 
                                                    SymbolKind::Struct | SymbolKind::Class | 
                                                    SymbolKind::Variable | SymbolKind::Constant);
            
            // 文字列リテラルやコメント内は除外
            if is_in_string_or_comment(line, start_col) {
                continue;
            }
            
            let symbol = Symbol {
                id: format!("{}#{}:{}:{}", path_str, line_no + 1, start_col, target_name),
                kind: if is_definition { 
                    target_kind.clone()
                } else { 
                    SymbolKind::Reference
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
            // 関数名の後に空白と括弧、またはジェネリクスが来る
            format!(r"\b{}\b", escaped)  // シンプルに単語境界のみでマッチ
        }
        SymbolKind::Class | SymbolKind::Struct => {
            // 型参照または定義
            // 構造体名は単体で使われるか、::でメソッドアクセスされる
            format!(r"\b{}(?:\b|::)", escaped)
        }
        SymbolKind::Variable | SymbolKind::Constant => {
            // 変数参照
            format!(r"\b{}\b", escaped)
        }
        SymbolKind::Module => {
            // モジュール参照
            format!(r"\b{}(?:\b|::)", escaped)
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
    
    // 定義パターン（より正確に）
    let definition_patterns = [
        ("fn ", true),          // 関数定義
        ("function ", true),    // JavaScriptなど
        ("def ", true),         // Python
        ("struct ", true),      // 構造体定義
        ("class ", true),       // クラス定義
        ("interface ", true),   // インターフェース
        ("let ", true),         // 変数定義
        ("const ", true),       // 定数定義
        ("var ", true),         // 変数定義
        ("type ", true),        // 型エイリアス
        ("enum ", true),        // 列挙型
        ("impl ", false),       // implブロックはそれ自体が定義ではない
    ];
    
    // キーワードの直前にあるかチェック
    for (keyword, is_definition_keyword) in definition_patterns.iter() {
        if let Some(keyword_pos) = trimmed.rfind(keyword) {
            // キーワードの後の部分をチェック
            let after_keyword = &trimmed[keyword_pos + keyword.len()..];
            
            // キーワードの直後か、またはスペースのみの場合
            if after_keyword.is_empty() || after_keyword.chars().all(char::is_whitespace) {
                return *is_definition_keyword;
            }
        }
    }
    
    false
}

/// 文字列リテラルやコメント内かを判定（改良版）
fn is_in_string_or_comment(line: &str, position: usize) -> bool {
    let before = &line[..position.min(line.len())];
    
    // 単一行コメントチェック
    if let Some(comment_pos) = before.find("//") {
        // 文字列内の // でない場合のみ
        let before_comment = &before[..comment_pos];
        if !is_in_string_literal(before_comment, comment_pos) {
            return position > comment_pos;
        }
    }
    
    // 文字列リテラル内かチェック
    is_in_string_literal(before, position)
}

/// 文字列リテラル内かを判定
fn is_in_string_literal(text: &str, _position: usize) -> bool {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut in_raw_string = false;
    
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        
        match chars[i] {
            '\\' if !in_raw_string => escaped = true,
            'r' if i + 1 < chars.len() && chars[i + 1] == '"' && !in_string && !in_char => {
                in_raw_string = true;
                i += 1; // Skip the next character
            }
            '"' if !in_char => {
                if in_raw_string {
                    in_raw_string = false;
                } else {
                    in_string = !in_string;
                }
            }
            '\'' if !in_string && !in_raw_string => in_char = !in_char,
            _ => {}
        }
        i += 1;
    }
    
    in_string || in_char || in_raw_string
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