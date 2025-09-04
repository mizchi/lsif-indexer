mod common;

use common::typescript_test_helpers::{SymbolTest, TypeScriptReferenceTest};
use lsif_core::SymbolKind;
use std::process::Command;

/// TypeScript LSPが利用可能かチェック
fn ensure_typescript_lsp() {
    // @typescript/native-previewが使用可能かチェック
    let native_preview_available = Command::new("npx")
        .args(["-y", "@typescript/native-preview", "--version"])
        .output()
        .is_ok();

    // フォールバック: typescript-language-serverをチェック
    let tsserver_available = Command::new("typescript-language-server")
        .arg("--version")
        .output()
        .is_ok();

    if !native_preview_available && !tsserver_available {
        println!("TypeScript LSP not available. Installing @typescript/native-preview...");

        // @typescript/native-previewをインストール試行
        let install_result = Command::new("npm")
            .args(["install", "-g", "@typescript/native-preview"])
            .output();

        match install_result {
            Ok(output) if output.status.success() => {
                println!("@typescript/native-preview installed successfully");
            }
            _ => {
                panic!(
                    "TypeScript LSP not available. Please install one of:\n\
                    - npm install -g @typescript/native-preview (recommended)\n\
                    - npm install -g typescript-language-server typescript"
                );
            }
        }
    }
}

#[test]
#[ignore] // Run with: cargo test typescript_references -- --ignored --nocapture
fn test_all_typescript_references() {
    ensure_typescript_lsp();

    let test_env = TypeScriptReferenceTest::new();

    // すべてのテストケースを定義
    let tests = vec![
        // Interface references
        SymbolTest::new("User", SymbolKind::Interface, 1, 10)
            .with_file_expectation("main.ts", 2)
            .with_file_expectation("user.service.ts", 3)
            .with_file_expectation("user.test.ts", 3),
        // Class references
        SymbolTest::new("UserService", SymbolKind::Class, 1, 8)
            .with_file_expectation("main.ts", 2)
            .with_file_expectation("user.service.ts", 1)
            .with_file_expectation("user.test.ts", 2),
        // Function references
        SymbolTest::new("getUser", SymbolKind::Function, 1, 5)
            .with_file_expectation("main.ts", 1)
            .with_file_expectation("user.test.ts", 2),
        // Enum references
        SymbolTest::new("Role", SymbolKind::Enum, 1, 5)
            .with_file_expectation("main.ts", 1)
            .with_file_expectation("user.service.ts", 2),
        // Method references (as Function)
        SymbolTest::new("getAllUsers", SymbolKind::Function, 1, 3)
            .with_file_expectation("main.ts", 1)
            .with_file_expectation("user.test.ts", 1),
    ];

    // すべてのテストを実行
    test_env.test_multiple_symbols(tests).unwrap();

    println!("✅ All TypeScript reference tests passed!");
}

#[test]
#[ignore]
fn test_typescript_import_export_references() {
    ensure_typescript_lsp();

    let test_env = TypeScriptReferenceTest::new();

    // Import/Export specific tests
    test_env
        .test_symbol_references(
            "UserService",
            &SymbolKind::Class,
            1, // 1 definition (export)
            3, // at least 3 imports
            vec![
                ("main.ts", 1),      // import
                ("user.test.ts", 1), // import
            ],
        )
        .unwrap();

    println!("✅ TypeScript import/export references test passed");
}

#[test]
#[ignore]
fn test_typescript_lsp_integration() {
    ensure_typescript_lsp();

    println!("🔧 Testing TypeScript LSP integration...");

    // LSPクライアントの動作確認
    let test_env = TypeScriptReferenceTest::new();

    // 基本的な統合テスト
    test_env
        .test_symbol_references(
            "User",
            &SymbolKind::Interface,
            1,
            5, // 最小限の使用
            vec![("main.ts", 1)],
        )
        .unwrap();

    println!("✅ TypeScript LSP integration test passed");
}
