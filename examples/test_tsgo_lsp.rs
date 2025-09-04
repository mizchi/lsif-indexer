/// tsgo LSPテストプログラム
///
/// workspace/symbolとdocumentSymbolの自動切り替えをテスト
use anyhow::Result;
use lsp::adapter::tsgo::TsgoAdapter;
use lsp::auto_switching_client::{AutoSwitchingLspClient, SymbolInfo};
use std::path::Path;

fn main() -> Result<()> {
    // ログ設定
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== tsgo LSP Auto-Switching Test ===\n");

    // tsgoアダプタを作成
    println!("Creating tsgo adapter...");
    let adapter = Box::new(TsgoAdapter);

    // 自動切り替えクライアントを作成
    println!("Initializing auto-switching LSP client...");
    let client = match AutoSwitchingLspClient::new(adapter) {
        Ok(client) => {
            println!("✅ Client initialized successfully");
            client
        }
        Err(e) => {
            println!("❌ Failed to initialize client: {}", e);
            println!("\nMake sure tsgo is installed:");
            println!("  npm install -g @typescript/native-preview");
            return Err(e);
        }
    };

    // 機能を確認
    println!("\n=== Server Capabilities ===");
    println!(
        "workspace/symbol: {}",
        if client.has_workspace_symbol() {
            "✅ Supported"
        } else {
            "❌ Not supported"
        }
    );
    println!(
        "documentSymbol: {}",
        if client.has_document_symbol() {
            "✅ Supported"
        } else {
            "❌ Not supported"
        }
    );

    // テストプロジェクトのパス
    let test_project = Path::new("tmp/ts-test-project");
    if !test_project.exists() {
        println!(
            "\n⚠️  Test project not found at: {}",
            test_project.display()
        );
        println!("Please create it first with TypeScript files.");
        return Ok(());
    }

    // プロジェクト全体のシンボルを取得（自動切り替え）
    println!("\n=== Getting All Symbols (Auto-Switch) ===");
    match client.get_all_symbols(test_project.to_str().unwrap()) {
        Ok(symbols) => {
            println!("✅ Found {} symbols", symbols.len());
            print_symbols(&symbols, 10);
        }
        Err(e) => {
            println!("❌ Failed to get symbols: {}", e);
        }
    }

    // 特定ファイルのシンボルを取得
    let test_file = test_project.join("index.ts");
    if test_file.exists() {
        let file_uri = format!("file://{}", test_file.canonicalize()?.display());
        println!("\n=== Getting File Symbols: {} ===", test_file.display());

        match client.get_file_symbols(&file_uri) {
            Ok(symbols) => {
                println!("✅ Found {} symbols in file", symbols.len());
                print_symbols(&symbols, 20);
            }
            Err(e) => {
                println!("❌ Failed to get file symbols: {}", e);
            }
        }
    }

    // 機能の詳細を表示
    println!("\n=== Detailed Capabilities ===");
    let caps = client.get_capabilities();

    if let Some(ref ws_provider) = caps.workspace_symbol_provider {
        println!("workspace_symbol_provider: {:?}", ws_provider);
    }

    if let Some(ref doc_provider) = caps.document_symbol_provider {
        println!("document_symbol_provider: {:?}", doc_provider);
    }

    println!("\n✅ Test completed successfully!");
    Ok(())
}

fn print_symbols(symbols: &[SymbolInfo], limit: usize) {
    for (i, symbol) in symbols.iter().take(limit).enumerate() {
        let kind_str = format!("{:?}", symbol.kind);
        let container = symbol.container_name.as_deref().unwrap_or("-");
        println!(
            "  {}. {} [{}] in {} at {}:{}",
            i + 1,
            symbol.name,
            kind_str,
            container,
            symbol.location.uri.path(),
            symbol.location.range.start.line + 1
        );

        if let Some(ref detail) = symbol.detail {
            println!("     Detail: {}", detail);
        }
    }

    if symbols.len() > limit {
        println!("  ... and {} more symbols", symbols.len() - limit);
    }
}
