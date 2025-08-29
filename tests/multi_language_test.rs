use lsif_indexer::cli::go_adapter::GoAdapter;
use lsif_indexer::cli::lsp_minimal_client::MinimalLspClient;
use lsif_indexer::cli::minimal_language_adapter::MinimalLanguageAdapter;
use lsif_indexer::cli::python_adapter::PythonAdapter;
use lsif_indexer::cli::typescript_adapter::TypeScriptAdapter;
use std::path::PathBuf;

/// 複数言語のLSPサーバー統合テスト

#[test]
#[ignore] // cargo test -- --ignored multi_language
fn test_go_lsp_integration() {
    let project_path = PathBuf::from("test-go-project");
    if !project_path.exists() {
        eprintln!("Skipping Go test: project not found");
        return;
    }

    let adapter = Box::new(GoAdapter);
    test_language_adapter(adapter, &project_path, "Go");
}

#[test]
#[ignore]
fn test_python_lsp_integration() {
    let project_path = PathBuf::from("test-python-project");
    if !project_path.exists() {
        eprintln!("Skipping Python test: project not found");
        return;
    }

    // Pythonアダプタの作成（利用可能なLSPサーバーを自動選択）
    let adapter = Box::new(PythonAdapter::new());

    // LSPサーバーが利用可能かチェック
    match adapter.spawn_lsp_command() {
        Ok(mut child) => {
            println!("✓ Python LSP server available");
            let _ = child.kill();
        }
        Err(_) => {
            eprintln!("Python LSP server not available. Install with:");
            eprintln!("  pip install python-lsp-server");
            eprintln!("  or");
            eprintln!("  npm install -g pyright");
            return;
        }
    }

    test_language_adapter(adapter, &project_path, "Python");
}

#[test]
#[ignore]
fn test_typescript_lsp_integration() {
    let project_path = PathBuf::from("test-typescript-project");
    if !project_path.exists() {
        eprintln!("Skipping TypeScript test: project not found");
        return;
    }

    let adapter = Box::new(TypeScriptAdapter::new());

    // LSPサーバーが利用可能かチェック
    match adapter.spawn_lsp_command() {
        Ok(mut child) => {
            println!("✓ TypeScript LSP server available");
            let _ = child.kill();
        }
        Err(_) => {
            eprintln!("TypeScript LSP server not available. Install with:");
            eprintln!("  npm install -g typescript-language-server typescript");
            return;
        }
    }

    test_language_adapter(adapter, &project_path, "TypeScript");
}

/// 共通のテストロジック
fn test_language_adapter(
    adapter: Box<dyn MinimalLanguageAdapter>,
    project_path: &PathBuf,
    language_name: &str,
) {
    println!("\n=== Testing {} Language Adapter ===", language_name);

    // LSPクライアントの作成
    let mut client = match MinimalLspClient::new(adapter) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create {} LSP client: {}", language_name, e);
            return;
        }
    };

    // 初期化
    match client.initialize(project_path) {
        Ok(result) => {
            println!("✓ {} LSP server initialized", language_name);
            if let Some(info) = result.server_info {
                println!(
                    "  Server: {} {}",
                    info.name,
                    info.version.unwrap_or_default()
                );
            }
        }
        Err(e) => {
            eprintln!("Failed to initialize {} LSP: {}", language_name, e);
            return;
        }
    }

    // プロジェクト内のソースファイルを探す
    let source_files = find_source_files(project_path, language_name);

    if source_files.is_empty() {
        eprintln!("No source files found for {}", language_name);
        return;
    }

    let mut total_symbols = 0;

    // 各ファイルのシンボルを取得
    for file_path in &source_files {
        match client.get_document_symbols(file_path) {
            Ok(symbols) => {
                let file_name = file_path.file_name().unwrap().to_string_lossy();
                println!("✓ {} - {} symbols", file_name, symbols.len());

                // 主要なシンボルを表示
                for symbol in symbols.iter().take(5) {
                    println!("    - {} ({:?})", symbol.name, symbol.kind);
                }
                if symbols.len() > 5 {
                    println!("    ... and {} more", symbols.len() - 5);
                }

                total_symbols += symbols.len();
            }
            Err(e) => {
                eprintln!("Failed to get symbols from {:?}: {}", file_path, e);
            }
        }
    }

    println!("  Total symbols: {}", total_symbols);
    assert!(total_symbols > 0, "Should find at least one symbol");

    // シャットダウン
    match client.shutdown() {
        Ok(_) => println!("✓ {} LSP server shut down cleanly", language_name),
        Err(e) => eprintln!("Failed to shutdown {} LSP: {}", language_name, e),
    }
}

/// 言語に応じたソースファイルを探す
fn find_source_files(project_path: &PathBuf, language_name: &str) -> Vec<PathBuf> {
    let extensions = match language_name {
        "Go" => vec!["go"],
        "Python" => vec!["py"],
        "TypeScript" => vec!["ts", "tsx"],
        "JavaScript" => vec!["js", "jsx"],
        _ => vec![],
    };

    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(project_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if extensions.contains(&ext.to_str().unwrap_or("")) {
                        files.push(path);
                    }
                }
            }
        }
    }

    files.sort();
    files
}

#[test]
#[ignore]
fn test_all_languages_summary() {
    println!("\n=== Multi-Language LSP Integration Test Summary ===\n");

    let languages = vec![
        ("Go", "test-go-project", "gopls"),
        ("Python", "test-python-project", "pylsp/pyright"),
        (
            "TypeScript",
            "test-typescript-project",
            "typescript-language-server",
        ),
    ];

    let mut successful = 0;
    let mut failed = 0;

    for (lang, project_dir, lsp_server) in languages {
        let project_path = PathBuf::from(project_dir);

        print!("{:<12} ", format!("{}:", lang));

        if !project_path.exists() {
            println!("❌ Project not found");
            failed += 1;
            continue;
        }

        let adapter: Box<dyn MinimalLanguageAdapter> = match lang {
            "Go" => Box::new(GoAdapter),
            "Python" => Box::new(PythonAdapter::new()),
            "TypeScript" => Box::new(TypeScriptAdapter::new()),
            _ => continue,
        };

        // LSPサーバーの起動テスト
        match adapter.spawn_lsp_command() {
            Ok(mut child) => {
                println!("✅ LSP server available ({})", lsp_server);
                successful += 1;
                let _ = child.kill();
            }
            Err(_) => {
                println!("❌ LSP server not found (install: {})", lsp_server);
                failed += 1;
            }
        }
    }

    println!("\n📊 Results: {} successful, {} failed", successful, failed);

    if failed > 0 {
        println!("\n📝 Installation instructions:");
        println!("  Go:         go install golang.org/x/tools/gopls@latest");
        println!("  Python:     pip install python-lsp-server");
        println!("  TypeScript: npm install -g typescript-language-server typescript");
    }
}
