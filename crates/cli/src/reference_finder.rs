use anyhow::Result;
/// 参照検索の実装
///
/// ファイル内容を実際に検索して使用箇所を見つける
use lsif_core::{Position, Range, Symbol, SymbolKind};
use lsp::adapter::language::{LanguageAdapter, RustLanguageAdapter, TypeScriptLanguageAdapter};
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
    let exclude_dirs = ["target", ".git", "node_modules", ".vscode"];

    // 対象ファイルを走査
    for entry in WalkDir::new(project_root)
        .follow_links(false)
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
        // 言語に応じたアダプタを取得
        let language_adapter = match path.extension().and_then(|s| s.to_str()) {
            Some("rs") => Some(Box::new(RustLanguageAdapter) as Box<dyn LanguageAdapter>),
            Some("ts") | Some("tsx") | Some("js") | Some("jsx") => {
                Some(Box::new(TypeScriptLanguageAdapter) as Box<dyn LanguageAdapter>)
            }
            _ => None,
        };

        if let Some(adapter) = language_adapter {
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
                    *target_kind
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
                detail: None,
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
    use std::fs;
    use tempfile::TempDir;

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

    #[test]
    fn test_find_references_in_file_rust() {
        let content = r#"
fn main() {
    println!("Hello");
    helper();
}

fn helper() {
    main(); // recursive call
}
"#;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, content).unwrap();

        let adapter = RustLanguageAdapter;
        let refs =
            find_references_in_file(&file_path, content, "main", &SymbolKind::Function, &adapter)
                .unwrap();

        // Should find definition and usage
        assert!(refs.len() >= 2);

        // Check definition
        let definitions: Vec<_> = refs.iter().filter(|r| r.is_definition).collect();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].symbol.range.start.line, 1);

        // Check reference
        let references: Vec<_> = refs.iter().filter(|r| !r.is_definition).collect();
        assert!(!references.is_empty());
    }

    #[test]
    fn test_find_references_in_file_typescript() {
        let content = r#"
function greet(name: string) {
    console.log(`Hello, ${name}`);
}

const result = greet("World");
greet("TypeScript");
"#;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.ts");
        fs::write(&file_path, content).unwrap();

        let adapter = TypeScriptLanguageAdapter;
        let refs = find_references_in_file(
            &file_path,
            content,
            "greet",
            &SymbolKind::Function,
            &adapter,
        )
        .unwrap();

        // Should find definition and 2 usages
        assert!(refs.len() >= 3);

        // Check definition
        let definitions: Vec<_> = refs.iter().filter(|r| r.is_definition).collect();
        assert_eq!(definitions.len(), 1);

        // Check references
        let references: Vec<_> = refs.iter().filter(|r| !r.is_definition).collect();
        assert_eq!(references.len(), 2);
    }

    #[test]
    fn test_find_references_ignores_strings_and_comments() {
        let content = r#"
fn test_func() {
    // test_func is mentioned in comment
    println!("test_func in string");
}

fn other() {
    test_func(); // actual usage
}
"#;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, content).unwrap();

        let adapter = RustLanguageAdapter;
        let refs = find_references_in_file(
            &file_path,
            content,
            "test_func",
            &SymbolKind::Function,
            &adapter,
        )
        .unwrap();

        // Should only find definition and actual usage, not in comments/strings
        assert_eq!(refs.len(), 2);

        let definitions: Vec<_> = refs.iter().filter(|r| r.is_definition).collect();
        assert_eq!(definitions.len(), 1);

        let references: Vec<_> = refs.iter().filter(|r| !r.is_definition).collect();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].symbol.range.start.line, 7);
    }

    #[test]
    fn test_find_all_references_in_project() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        // Create multiple files with references
        let file1_content = r#"
pub fn shared_func() {
    println!("Shared function");
}
"#;

        let file2_content = r#"
use crate::shared_func;

fn main() {
    shared_func();
    shared_func();
}
"#;

        fs::write(src_dir.join("lib.rs"), file1_content).unwrap();
        fs::write(src_dir.join("main.rs"), file2_content).unwrap();

        let refs =
            find_all_references(temp_dir.path(), "shared_func", &SymbolKind::Function).unwrap();

        // Should find definition in lib.rs and usages in main.rs
        assert!(refs.len() >= 3);

        // Check files are found
        let files: HashSet<_> = refs.iter().map(|r| &r.symbol.file_path).collect();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_find_references_with_different_symbol_kinds() {
        let content = r#"
struct User {
    name: String,
}

impl User {
    fn new(name: String) -> User {
        User { name }
    }
}

fn main() {
    let user = User::new("Alice".to_string());
}
"#;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, content).unwrap();

        let adapter = RustLanguageAdapter;

        // Find struct references
        let struct_refs =
            find_references_in_file(&file_path, content, "User", &SymbolKind::Struct, &adapter)
                .unwrap();

        assert!(!struct_refs.is_empty()); // At least the definition

        // Find method references
        let method_refs =
            find_references_in_file(&file_path, content, "new", &SymbolKind::Method, &adapter)
                .unwrap();

        assert!(!method_refs.is_empty()); // Definition and usage
    }

    #[test]
    fn test_reference_sorting() {
        let mut references = vec![
            Reference {
                symbol: Symbol {
                    id: "1".to_string(),
                    name: "test".to_string(),
                    kind: SymbolKind::Function,
                    file_path: "b.rs".to_string(),
                    range: Range {
                        start: Position {
                            line: 10,
                            character: 5,
                        },
                        end: Position {
                            line: 10,
                            character: 10,
                        },
                    },
                    documentation: None,
                    detail: None,
                },
                is_definition: false,
            },
            Reference {
                symbol: Symbol {
                    id: "2".to_string(),
                    name: "test".to_string(),
                    kind: SymbolKind::Function,
                    file_path: "a.rs".to_string(),
                    range: Range {
                        start: Position {
                            line: 5,
                            character: 0,
                        },
                        end: Position {
                            line: 5,
                            character: 5,
                        },
                    },
                    documentation: None,
                    detail: None,
                },
                is_definition: true,
            },
            Reference {
                symbol: Symbol {
                    id: "3".to_string(),
                    name: "test".to_string(),
                    kind: SymbolKind::Function,
                    file_path: "a.rs".to_string(),
                    range: Range {
                        start: Position {
                            line: 5,
                            character: 10,
                        },
                        end: Position {
                            line: 5,
                            character: 15,
                        },
                    },
                    documentation: None,
                    detail: None,
                },
                is_definition: false,
            },
        ];

        // Sort as in find_all_references
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

        // Check order: a.rs should come before b.rs
        assert_eq!(references[0].symbol.file_path, "a.rs");
        assert_eq!(references[1].symbol.file_path, "a.rs");
        assert_eq!(references[2].symbol.file_path, "b.rs");

        // Within a.rs, should be sorted by position
        assert_eq!(references[0].symbol.range.start.character, 0);
        assert_eq!(references[1].symbol.range.start.character, 10);
    }

    #[test]
    fn test_exclude_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Create source directory
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        // Create target directory (should be excluded)
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&target_dir).unwrap();

        // Create files in both
        let content = r#"
fn test_func() {
    println!("Test");
}
"#;

        fs::write(src_dir.join("main.rs"), content).unwrap();
        fs::write(target_dir.join("debug.rs"), content).unwrap();

        let refs =
            find_all_references(temp_dir.path(), "test_func", &SymbolKind::Function).unwrap();

        // Should only find reference in src, not in target
        for reference in &refs {
            assert!(!reference.symbol.file_path.contains("target"));
        }
    }

    #[test]
    fn test_reference_deduplication() {
        // This tests the deduplication logic in find_all_references
        let temp_dir = TempDir::new().unwrap();

        let content = r#"
fn duplicate() {
    duplicate(); // Same location shouldn't appear twice
}
"#;

        fs::write(temp_dir.path().join("test.rs"), content).unwrap();

        let refs =
            find_all_references(temp_dir.path(), "duplicate", &SymbolKind::Function).unwrap();

        // Check that each location appears only once
        let mut seen = HashSet::new();
        for reference in &refs {
            let key = format!(
                "{}:{}:{}",
                reference.symbol.file_path,
                reference.symbol.range.start.line,
                reference.symbol.range.start.character
            );
            assert!(seen.insert(key), "Duplicate reference found");
        }
    }
}
