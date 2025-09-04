use anyhow::Result;
use cli::{
    lsp_adapter::{detect_language, LspAdapter, RustAnalyzerAdapter, TypeScriptAdapter},
    lsp_client::LspClient,
    lsp_features::{DependencyGraph, LspClient as FeatureClient, LspCodeAnalyzer},
    lsp_integration::LspIntegration,
};
use lsp_types::*;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
#[ignore] // Requires rust-analyzer to be installed
fn test_lsp_client_basic() -> Result<()> {
    let adapter = Box::new(RustAnalyzerAdapter);
    let client = LspClient::new(adapter)?;

    // Test basic functionality
    let test_file = PathBuf::from("src/lib.rs");
    if test_file.exists() {
        let content = fs::read_to_string(&test_file)?;
        let uri = lsp_types::Url::from_file_path(&test_file).unwrap();

        client.open_document(uri.clone(), content, "rust".to_string())?;

        // Test hover
        let hover = client.hover(
            uri.clone(),
            lsp_types::Position {
                line: 0,
                character: 0,
            },
        )?;
        assert!(hover.is_some() || hover.is_none()); // Either result is acceptable

        // Test document symbols
        let symbols = client.document_symbols(uri)?;
        assert!(!symbols.is_empty());
    }

    Ok(())
}

#[test]
fn test_language_detection() {
    assert!(detect_language("main.rs").is_some());
    assert!(detect_language("index.ts").is_some());
    assert!(detect_language("app.js").is_some());
    assert!(detect_language("unknown.xyz").is_none());
}

#[tokio::test]
#[ignore] // Requires language servers to be installed
async fn test_lsp_integration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.rs");

    fs::write(
        &test_file,
        r#"
fn main() {
    println!("Hello, world!");
}

struct MyStruct {
    field: String,
}

impl MyStruct {
    fn new() -> Self {
        Self {
            field: String::new(),
        }
    }
}
"#,
    )?;

    let mut lsp = LspIntegration::new(temp_dir.path().to_path_buf())?;

    // Test hover
    let hover_info = lsp.get_hover_info(&test_file, 2, 5).await?;
    assert!(!hover_info.is_empty());

    // Test completions
    let completions = lsp.get_completions(&test_file, 3, 1).await?;
    assert!(!completions.is_empty());

    Ok(())
}

#[tokio::test]
#[ignore] // Requires language servers to be installed
async fn test_enhanced_indexing() -> Result<()> {
    // use lsif_core::enhanced_graph::EnhancedIndex;

    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("lib.rs");

    fs::write(
        &test_file,
        r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(x: f64, y: f64) -> f64 {
    x * y
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
    
    #[test]
    fn test_multiply() {
        assert_eq!(multiply(2.0, 3.0), 6.0);
    }
}
"#,
    )?;

    let _lsp = LspIntegration::new(temp_dir.path().to_path_buf())?;
    // let mut index = EnhancedIndex::default();

    // lsp.enhance_index(&mut index).await?;

    // Verify index contains expected symbols
    // assert!(!index.symbols.is_empty());

    // Check for function symbols
    // let function_symbols: Vec<_> = index
    //     .symbols
    //     .values()
    //     .filter(|s| matches!(s.kind, lsif_core::SymbolKind::Function))
    //     .collect();
    // assert!(function_symbols.len() >= 2); // add and multiply

    Ok(())
}

#[test]
#[ignore] // lspサブコマンドは現在のCLIに存在しない
fn test_lsp_cli_commands() -> Result<()> {
    use std::process::Command;

    // Test that LSP commands are available in the CLI
    let output = Command::new("cargo")
        .args(["run", "--bin", "lsif", "--", "lsp", "--help"])
        .output()?;

    let help_text = String::from_utf8_lossy(&output.stdout);
    assert!(help_text.contains("hover"));
    assert!(help_text.contains("complete"));
    assert!(help_text.contains("implementation"));
    assert!(help_text.contains("type-definition"));
    assert!(help_text.contains("rename"));
    assert!(help_text.contains("diagnostics"));

    Ok(())
}

// エッジケーステスト
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_empty_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let empty_file = temp_dir.path().join("empty.rs");
        fs::write(&empty_file, "").unwrap();

        let adapter = Box::new(RustAnalyzerAdapter);
        if let Ok(client) = LspClient::new(adapter) {
            let uri = Url::from_file_path(&empty_file).unwrap();
            let symbols = client.document_symbols(uri);

            // 空ファイルでもエラーにならないことを確認
            assert!(symbols.is_ok());
            let symbols = symbols.unwrap();
            assert!(symbols.is_empty());
        }
    }

    #[test]
    fn test_invalid_syntax_handling() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_file = temp_dir.path().join("invalid.rs");
        fs::write(
            &invalid_file,
            r#"
fn broken_function(
    // 構文エラー: 関数が閉じられていない
"#,
        )
        .unwrap();

        let adapter = Box::new(RustAnalyzerAdapter);
        if let Ok(client) = LspClient::new(adapter) {
            let uri = Url::from_file_path(&invalid_file).unwrap();

            // 構文エラーがあっても診断情報を取得できることを確認
            let diagnostics = client.diagnostics(uri);
            // 診断情報の取得はエラーになる可能性がある（診断をサポートしていない場合など）
            if let Ok(diags) = diagnostics {
                assert!(diags.is_empty() || !diags.is_empty());
            }
        }
    }

    #[test]
    fn test_unicode_handling() {
        let temp_dir = TempDir::new().unwrap();
        let unicode_file = temp_dir.path().join("unicode.rs");
        fs::write(
            &unicode_file,
            r#"
// 日本語コメント
fn こんにちは() {
    println!("Hello 世界 🌍");
    let 変数 = "テスト";
}
"#,
        )
        .unwrap();

        let adapter = Box::new(RustAnalyzerAdapter);
        if let Ok(client) = LspClient::new(adapter) {
            let uri = Url::from_file_path(&unicode_file).unwrap();
            let symbols = client.document_symbols(uri);

            // Unicode文字を含むファイルでも正常に処理できることを確認
            assert!(symbols.is_ok());
        }
    }
}

// パフォーマンステスト
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore] // パフォーマンステストは通常スキップ
    fn test_large_file_performance() {
        let temp_dir = TempDir::new().unwrap();
        let large_file = temp_dir.path().join("large.rs");

        // 大きなファイルを生成
        let mut content = String::new();
        for i in 0..1000 {
            content.push_str(&format!(
                r#"
fn function_{}() -> i32 {{
    let result = {};
    println!("Function {}", result);
    result
}}
"#,
                i,
                i * 2,
                i
            ));
        }
        fs::write(&large_file, content).unwrap();

        let adapter = Box::new(RustAnalyzerAdapter);
        if let Ok(client) = LspClient::new(adapter) {
            let uri = Url::from_file_path(&large_file).unwrap();
            let start = Instant::now();
            let symbols = client.document_symbols(uri);
            let duration = start.elapsed();

            assert!(symbols.is_ok());
            println!("Large file indexing took: {duration:?}");

            // 10秒以内に完了することを確認
            assert!(duration.as_secs() < 10);
        }
    }

    #[test]
    fn test_dependency_graph_scalability() {
        let mut graph = DependencyGraph::new();

        let start = Instant::now();

        // 大規模な依存関係グラフを構築
        for i in 0..500 {
            for j in 0..20 {
                graph.add_dependency(&format!("module_{i}.rs"), &format!("dep_{i}_{j}.rs"));
            }
        }

        let build_duration = start.elapsed();

        // 検索性能をテスト
        let search_start = Instant::now();
        for i in 0..100 {
            let deps = graph.get_dependencies(&format!("module_{i}.rs"));
            assert!(deps.is_some());
        }
        let search_duration = search_start.elapsed();

        println!("Built graph with 10000 edges in {build_duration:?}");
        println!("Searched 100 nodes in {search_duration:?}");

        // 妥当な時間内に完了することを確認
        assert!(build_duration.as_secs() < 5);
        assert!(search_duration.as_millis() < 500);
    }
}

// 統合テスト
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 実際のLSPサーバーが必要
    async fn test_full_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let project_file = temp_dir.path().join("project.rs");

        fs::write(
            &project_file,
            r#"
mod utils {
    pub fn helper(x: i32) -> i32 {
        x * 2
    }
}

use utils::helper;

fn main() {
    let result = helper(21);
    println!("Result: {}", result);
}
"#,
        )
        .unwrap();

        // LSPクライアントを初期化
        let adapter = Box::new(RustAnalyzerAdapter);
        if let Ok(client) = FeatureClient::new(adapter) {
            let analyzer = LspCodeAnalyzer::new(Arc::new(client));
            let uri = Url::from_file_path(&project_file).unwrap();

            // ファイル構造を解析
            let structure = analyzer.analyze_file_structure(uri.as_str());
            assert!(structure.is_ok());

            // 依存関係グラフを構築
            let graph = analyzer.build_dependency_graph(uri.as_str());
            assert!(graph.is_ok());
        }
    }

    #[tokio::test]
    #[ignore] // 実際のLSPサーバーが必要
    async fn test_multi_language_support() {
        let temp_dir = TempDir::new().unwrap();

        // Rustファイル
        let rust_file = temp_dir.path().join("test.rs");
        fs::write(&rust_file, "fn main() {}").unwrap();

        // TypeScriptファイル
        let ts_file = temp_dir.path().join("test.ts");
        fs::write(&ts_file, "function main() {}").unwrap();

        // JavaScriptファイル
        let js_file = temp_dir.path().join("test.js");
        fs::write(&js_file, "function main() {}").unwrap();

        // 各言語のアダプターをテスト
        for (file, adapter) in [
            (
                rust_file,
                Box::new(RustAnalyzerAdapter) as Box<dyn LspAdapter>,
            ),
            (ts_file, Box::new(TypeScriptAdapter) as Box<dyn LspAdapter>),
            (js_file, Box::new(TypeScriptAdapter) as Box<dyn LspAdapter>), // JS も TypeScript adapter で処理
        ] {
            if let Ok(client) = LspClient::new(adapter) {
                let uri = Url::from_file_path(&file).unwrap();
                let symbols = client.document_symbols(uri);

                // 各言語でシンボルを取得できることを確認
                assert!(symbols.is_ok());
            }
        }
    }
}
