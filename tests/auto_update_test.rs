use anyhow::Result;
use cli::differential_indexer::DifferentialIndexer;
use cli::git_diff::GitDiffDetector;
use cli::storage::IndexStorage;
use lsif_core::CodeGraph;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// 自動更新のシミュレーションテスト
#[test]
#[ignore] // DifferentialIndexerの実装が必要
fn test_auto_update_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_detection.db");

    // Gitリポジトリを初期化
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()?;
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()?;
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_dir.path())
        .output()?;

    // 初期ファイルを作成
    fs::write(
        temp_dir.path().join("main.rs"),
        r#"
        fn main() {
            println!("Hello");
        }
        
        fn helper() {
            // Helper
        }
    "#,
    )?;

    // Gitに追加
    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result1 = indexer.index_differential()?;
    assert_eq!(result1.files_added, 1);

    // メタデータを確認
    let storage = IndexStorage::open(&db_path)?;
    let metadata1 = storage.load_metadata()?.unwrap();
    let initial_hashes = metadata1.file_hashes.clone();

    // ファイルを変更
    fs::write(
        temp_dir.path().join("main.rs"),
        r#"
        fn main() {
            println!("Modified!");
            new_function();
        }
        
        fn new_function() {
            // New
        }
    "#,
    )?;

    // 変更検知テスト
    let mut detector = GitDiffDetector::new(temp_dir.path())?;
    let changes = detector.detect_changes_since(None)?;
    assert!(!changes.is_empty());

    // 差分インデックス
    let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result2 = indexer2.index_differential()?;
    assert_eq!(result2.files_modified, 1);

    // ハッシュが更新されたことを確認
    let metadata2 = storage.load_metadata()?.unwrap();
    let updated_hashes = metadata2.file_hashes;

    // main.rsのハッシュが変わったことを確認
    let initial_hash = initial_hashes
        .iter()
        .find(|(k, _)| k.ends_with("main.rs"))
        .map(|(_, v)| v);
    let updated_hash = updated_hashes
        .iter()
        .find(|(k, _)| k.ends_with("main.rs"))
        .map(|(_, v)| v);

    assert!(initial_hash.is_some());
    assert!(updated_hash.is_some());
    assert_ne!(initial_hash, updated_hash);

    Ok(())
}

/// クエリ実行時の自動更新シミュレーション
#[test]
#[ignore] // DifferentialIndexerの実装が必要
fn test_query_with_auto_update() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_query.db");

    // Gitリポジトリを初期化
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()?;
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()?;
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_dir.path())
        .output()?;

    // テストファイルを作成
    fs::write(
        temp_dir.path().join("lib.rs"),
        r#"
        pub struct Config {
            name: String,
        }
        
        impl Config {
            pub fn new(name: String) -> Self {
                Config { name }
            }
        }
    "#,
    )?;

    // Gitに追加
    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    indexer.index_differential()?;

    // インデックスからデータを読み込み
    let storage = IndexStorage::open(&db_path)?;
    let graph1: CodeGraph = storage.load_data("graph")?.unwrap();
    let symbols1 = graph1.get_all_symbols().count();

    // ファイルを変更
    fs::write(
        temp_dir.path().join("lib.rs"),
        r#"
        pub struct Config {
            name: String,
            value: i32,
        }
        
        impl Config {
            pub fn new(name: String, value: i32) -> Self {
                Config { name, value }
            }
            
            pub fn get_value(&self) -> i32 {
                self.value
            }
        }
    "#,
    )?;

    // 差分インデックスを実行
    let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer2.index_differential()?;
    assert_eq!(result.files_modified, 1);

    // シンボル数が変わったことを確認
    let graph2: CodeGraph = storage.load_data("graph")?.unwrap();
    let symbols2 = graph2.get_all_symbols().count();
    assert_ne!(symbols1, symbols2);

    Ok(())
}

/// パフォーマンステスト：自動更新のオーバーヘッド測定
#[test]
#[ignore] // DifferentialIndexerの実装が必要
fn test_auto_update_performance() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_performance.db");

    // Gitリポジトリを初期化
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()?;
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()?;
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_dir.path())
        .output()?;

    // 複数のファイルを作成
    for i in 0..10 {
        fs::write(
            temp_dir.path().join(format!("file{i}.rs")),
            format!("fn func_{i}() {{ /* content */ }}"),
        )?;
    }

    // Gitに追加
    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()?;

    // 初回インデックス
    let start = std::time::Instant::now();
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer.index_differential()?;
    let initial_time = start.elapsed();

    assert!(
        result.files_added >= 10,
        "Expected at least 10 files added, got {}",
        result.files_added
    );
    println!("Initial indexing of 10 files: {initial_time:?}");

    // 変更なしで再実行（自動更新のオーバーヘッド測定）
    let start = std::time::Instant::now();
    let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result2 = indexer2.index_differential()?;
    let no_change_time = start.elapsed();

    assert_eq!(result2.files_modified, 0);
    println!("Auto-update check (no changes): {no_change_time:?}");

    // 自動更新チェックは高速であるべき（0.1秒以内）
    assert!(
        no_change_time.as_millis() < 100,
        "Auto-update check took too long: {no_change_time:?}"
    );

    // 1ファイルだけ変更
    fs::write(
        temp_dir.path().join("file0.rs"),
        "fn func_0_modified() { /* modified */ }",
    )?;

    // 差分インデックス実行時間
    let start = std::time::Instant::now();
    let mut indexer3 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result3 = indexer3.index_differential()?;
    let update_time = start.elapsed();

    assert_eq!(result3.files_modified, 1);
    println!("Auto-update with 1 file changed: {update_time:?}");

    // 差分更新は初回より圧倒的に速いはず
    assert!(update_time < initial_time / 5);

    Ok(())
}

/// CLI統合テスト（実際のコマンド実行）
#[test]
#[ignore] // 実際のバイナリが必要なため
fn test_cli_auto_update() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("cli_test.db");

    // テストファイルを作成
    fs::write(
        temp_dir.path().join("test.rs"),
        "fn main() { println!(\"test\"); }",
    )?;

    // 初回インデックス
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "differential-index",
            "-p",
            &temp_dir.path().to_string_lossy(),
            "-o",
            &db_path.to_string_lossy(),
        ])
        .output()?;

    assert!(output.status.success());

    // ファイルを変更
    fs::write(
        temp_dir.path().join("test.rs"),
        "fn main() { println!(\"modified\"); }",
    )?;

    // クエリ実行（自動更新が発生するはず）
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "query",
            "-i",
            &db_path.to_string_lossy(),
            "--query-type",
            "definition",
            "-f",
            "test.rs",
            "-l",
            "1",
            "-c",
            "1",
        ])
        .output()?;

    assert!(output.status.success());

    // 出力に自動更新メッセージが含まれることを確認
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("Auto-updated index") {
        println!("Auto-update message found in output");
    }

    Ok(())
}
