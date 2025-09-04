use lsp::adapter::go::GoAdapter;
use lsp::lsp_minimal_client::MinimalLspClient;
use lsp_types::Position;
use std::path::PathBuf;

#[test]
#[ignore] // cargo test -- --ignored lsp_minimal_integration
fn test_gopls_with_minimal_client() {
    let project_path = PathBuf::from("test-go-project");
    if !project_path.exists() {
        eprintln!("Test Go project not found. Please create test-go-project first.");
        return;
    }

    // GoアダプタでLSPクライアントを作成
    let adapter = Box::new(GoAdapter);
    let mut client = match MinimalLspClient::new(adapter) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create LSP client: {}", e);
            return;
        }
    };

    // 初期化
    match client.initialize(&project_path) {
        Ok(result) => {
            println!("✓ LSP server initialized");
            println!(
                "  Server name: {:?}",
                result.server_info.as_ref().map(|i| &i.name)
            );
            println!(
                "  Server version: {:?}",
                result.server_info.as_ref().and_then(|i| i.version.as_ref())
            );
        }
        Err(e) => {
            eprintln!("Failed to initialize: {}", e);
            return;
        }
    }

    // main.goのシンボル取得
    let main_file = project_path.join("main.go");
    match client.get_document_symbols(&main_file) {
        Ok(symbols) => {
            println!("✓ Found {} symbols in main.go", symbols.len());

            // シンボルの詳細を表示
            for symbol in &symbols {
                println!("  - {} ({:?})", symbol.name, symbol.kind);

                // 子シンボル（メソッドなど）も表示
                if let Some(children) = &symbol.children {
                    for child in children {
                        println!("    - {} ({:?})", child.name, child.kind);
                    }
                }
            }

            // 特定のシンボルの存在確認
            assert!(
                symbols.iter().any(|s| s.name == "Calculator"),
                "Calculator type should exist"
            );
            assert!(
                symbols.iter().any(|s| s.name == "main"),
                "main function should exist"
            );
            assert!(
                symbols.iter().any(|s| s.name == "NewCalculator"),
                "NewCalculator function should exist"
            );
        }
        Err(e) => {
            eprintln!("Failed to get symbols: {}", e);
        }
    }

    // utils.goのシンボル取得
    let utils_file = project_path.join("utils.go");
    match client.get_document_symbols(&utils_file) {
        Ok(symbols) => {
            println!("✓ Found {} symbols in utils.go", symbols.len());

            for symbol in &symbols {
                println!("  - {} ({:?})", symbol.name, symbol.kind);
            }

            assert!(
                symbols.iter().any(|s| s.name == "StringUtils"),
                "StringUtils type should exist"
            );
            assert!(
                symbols.iter().any(|s| s.name == "CountWords"),
                "CountWords function should exist"
            );
        }
        Err(e) => {
            eprintln!("Failed to get utils.go symbols: {}", e);
        }
    }

    // 参照検索テスト
    // Calculator型の参照を検索（定義位置: line 8, col 5）
    match client.find_references(
        &main_file,
        Position {
            line: 7,
            character: 5,
        },
    ) {
        Ok(refs) => {
            println!("✓ Found {} references to Calculator", refs.len());
            for reference in &refs {
                println!(
                    "  - {}:{}:{}",
                    reference.uri.path(),
                    reference.range.start.line + 1,
                    reference.range.start.character + 1
                );
            }
        }
        Err(e) => {
            eprintln!("Failed to find references: {}", e);
        }
    }

    // シャットダウン
    match client.shutdown() {
        Ok(_) => println!("✓ LSP server shut down cleanly"),
        Err(e) => eprintln!("Failed to shutdown: {}", e),
    }
}

#[test]
#[ignore]
fn test_multiple_file_analysis() {
    let project_path = PathBuf::from("test-go-project");
    if !project_path.exists() {
        eprintln!("Test Go project not found.");
        return;
    }

    let adapter = Box::new(GoAdapter);
    let mut client = match MinimalLspClient::new(adapter) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {}", e);
            return;
        }
    };

    if let Err(e) = client.initialize(&project_path) {
        eprintln!("Failed to initialize: {}", e);
        return;
    }

    // 複数ファイルのシンボルを収集
    let mut total_symbols = 0;
    let files = vec!["main.go", "utils.go"];

    for file_name in files {
        let file_path = project_path.join(file_name);
        if let Ok(symbols) = client.get_document_symbols(&file_path) {
            total_symbols += symbols.len();
            println!("{}: {} symbols", file_name, symbols.len());
        }
    }

    println!("Total symbols across all files: {}", total_symbols);
    assert!(
        total_symbols > 10,
        "Should find multiple symbols across files"
    );

    let _ = client.shutdown();
}
