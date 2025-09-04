use cli::reference_finder::find_all_references;
use lsif_core::SymbolKind;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_typescript_definition_patterns() {
    let temp_dir = TempDir::new().unwrap();

    // 様々な定義パターンをテスト
    let content = r#"interface User {
    id: string;
}

export interface ExportedUser {
    id: string;
}

class MyClass {
    method() {}
}

export class ExportedClass {
    method() {}
}

function myFunction() {}

export function exportedFunction() {}

const myConst = 1;
export const exportedConst = 2;

type MyType = string;
export type ExportedType = number;

enum MyEnum { A, B }
export enum ExportedEnum { X, Y }
"#;

    fs::write(temp_dir.path().join("test.ts"), content).unwrap();

    // 各種定義パターンをテスト
    let test_cases = vec![
        ("User", SymbolKind::Interface, "interface"),
        ("ExportedUser", SymbolKind::Interface, "export interface"),
        ("MyClass", SymbolKind::Class, "class"),
        ("ExportedClass", SymbolKind::Class, "export class"),
        ("myFunction", SymbolKind::Function, "function"),
        ("exportedFunction", SymbolKind::Function, "export function"),
        ("MyEnum", SymbolKind::Enum, "enum"),
        ("ExportedEnum", SymbolKind::Enum, "export enum"),
    ];

    for (name, kind, pattern_type) in test_cases {
        println!("\n--- Testing {} ({}) ---", name, pattern_type);

        let references = find_all_references(temp_dir.path(), name, &kind).unwrap();

        println!("Found {} references", references.len());

        let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
        println!("Definitions: {}", definitions.len());

        for (i, r) in references.iter().enumerate() {
            let line_num = r.symbol.range.start.line as usize;
            let line = content.lines().nth(line_num).unwrap_or("");
            println!(
                "  {}: line {} (def: {}) - '{}'",
                i + 1,
                line_num + 1,
                r.is_definition,
                line.trim()
            );
        }

        assert!(
            !references.is_empty(),
            "{} should have at least one reference",
            name
        );
        assert!(
            !definitions.is_empty(),
            "{} should have at least one definition",
            name
        );
    }
}
