use lsif_indexer::cli::go_adapter::GoAdapter;
use lsif_indexer::cli::minimal_language_adapter::MinimalLanguageAdapter;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
#[ignore] // 実行時は cargo test -- --ignored gopls_integration_test
fn test_gopls_connection() {
    // Goアダプタの作成
    let adapter = GoAdapter;

    // 基本情報の確認
    assert_eq!(adapter.language_id(), "go");
    assert_eq!(adapter.supported_extensions(), vec!["go"]);

    // LSPコマンドの起動テスト
    match adapter.spawn_lsp_command() {
        Ok(mut child) => {
            println!("✓ gopls process started successfully");
            // プロセスを終了
            let _ = child.kill();
        }
        Err(e) => {
            eprintln!("Failed to start gopls: {}", e);
            panic!("gopls not available");
        }
    }

    println!("✓ Go adapter created successfully");
}

#[test]
#[ignore]
fn test_gopls_lsp_communication() {
    // テスト用プロジェクトのパス
    let project_path = PathBuf::from("test-go-project");
    if !project_path.exists() {
        eprintln!("Test Go project not found. Skipping test.");
        return;
    }

    // Goアダプタで LSP プロセスを起動
    let adapter = GoAdapter;
    match adapter.spawn_lsp_command() {
        Ok(mut child) => {
            println!("✓ Successfully started gopls process");

            // TODO: 実際のLSPクライアント実装後にシンボル取得テストを追加
            // 現在は基本的な起動確認のみ

            println!("  Process ID: {:?}", child.id());

            // プロセスを終了
            let _ = child.kill();
            println!("✓ gopls process terminated");
        }
        Err(e) => {
            eprintln!("Failed to start gopls: {}", e);
            eprintln!("Make sure gopls is installed: go install golang.org/x/tools/gopls@latest");
        }
    }
}

// ヘルパー関数: テスト環境のセットアップ
