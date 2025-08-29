use crate::cli::language_adapter::{detect_language_adapter, LanguageAdapter};
/// 参照検索の実装
///
/// ファイル内容を実際に検索して使用箇所を見つける
use crate::core::{Position, Range, Symbol, SymbolKind};
use anyhow::Result;
use regex::Regex;
use std::collections::HashSet;
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
    let mut seen_locations = HashSet::new();

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
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        // 言語アダプターを検出
        if let Some(adapter) = detect_language_adapter(&path.to_string_lossy()) {
            if adapter.is_source_file(path) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let refs = find_references_in_file(
                        path,
                        &content,
                        target_name,
                        target_kind,
                        adapter.as_ref(),
                    )?;

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
        }
    }

    // 結果をソート（ファイル名、行番号順）
    references.sort_by(|a, b| {
        a.symbol
            .file_path
            .cmp(&b.symbol.file_path)
            .then(a.symbol.range.start.line.cmp(&b.symbol.range.start.line))
            .then(
                a.symbol
                    .range
                    .start
                    .character
                    .cmp(&b.symbol.range.start.character),
            )
    });

    Ok(references)
}

/// ファイル内の参照を検索
fn find_references_in_file(
    path: &Path,
    content: &str,
    target_name: &str,
    target_kind: &SymbolKind,
    adapter: &dyn LanguageAdapter,
) -> Result<Vec<Reference>> {
    // 検索パターンを構築
    let pattern = adapter.build_reference_pattern(target_name, target_kind);
    let regex = Regex::new(&pattern)?;
    let mut references = Vec::new();
    let path_str = path.to_string_lossy().to_string();

    for (line_no, line) in content.lines().enumerate() {
        // 正規表現でマッチを検索
        for mat in regex.find_iter(line) {
            let start_col = mat.start();
            let end_col = mat.end();

            // コンテキストから定義か使用かを判定
            let is_definition = adapter.is_definition_context(line, start_col)
                && matches!(
                    target_kind,
                    SymbolKind::Function
                        | SymbolKind::Method
                        | SymbolKind::Struct
                        | SymbolKind::Class
                        | SymbolKind::Interface
                        | SymbolKind::Enum
                        | SymbolKind::Variable
                        | SymbolKind::Constant
                );

            // 文字列リテラルやコメント内は除外
            if adapter.is_in_string_or_comment(line, start_col) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::language_adapter::{RustLanguageAdapter, TypeScriptLanguageAdapter};

    #[test]
    fn test_is_definition_context() {
        let rust_adapter = RustLanguageAdapter;
        assert!(rust_adapter.is_definition_context("fn main() {", 3));
        assert!(rust_adapter.is_definition_context("struct User {", 7));

        let ts_adapter = TypeScriptLanguageAdapter;
        assert!(ts_adapter.is_definition_context("let count = 0;", 4));
        assert!(!ts_adapter.is_definition_context("println!(main);", 9));
    }

    #[test]
    fn test_is_in_string_or_comment() {
        let adapter = RustLanguageAdapter;
        assert!(adapter.is_in_string_or_comment("// this is main", 11));
        assert!(adapter.is_in_string_or_comment("\"hello main\"", 7));
        assert!(!adapter.is_in_string_or_comment("main(); // comment", 2));
    }

    #[test]
    fn test_build_search_pattern() {
        let adapter = RustLanguageAdapter;
        let pattern = adapter.build_reference_pattern("test", &SymbolKind::Function);
        assert!(pattern.contains("test"));
        assert!(pattern.contains(r"\b")); // 単語境界のチェック
    }
}
