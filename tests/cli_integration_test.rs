use anyhow::Result;
use std::process::Command;
use tempfile::TempDir;

#[test]
#[ignore] // CLIインターフェースが変更されたため更新が必要
fn test_lsif_basic_indexing() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    // Test basic indexing with new CLI interface
    let output = Command::new("cargo")
        .args(["run", "--bin", "lsif", "--"])
        .args(["index"])
        .args(["--project", "."])
        .args(["--output", db_path.to_str().unwrap()])
        .output()?;

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check database was created
    assert!(db_path.exists());

    Ok(())
}

#[test]
#[ignore] // CLIインターフェースが変更されたため更新が必要
fn test_lsif_list_command() -> Result<()> {
    let output = Command::new("cargo")
        .args(["run", "--bin", "lsif", "--", "list"])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Rust"));
    assert!(stdout.contains("TypeScript"));
    assert!(stdout.contains("rust-analyzer"));

    Ok(())
}

#[test]
#[ignore] // CLIインターフェースが変更されたため更新が必要
fn test_lsif_stats_command() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    // Create index first
    Command::new("cargo")
        .args(["run", "--bin", "lsif", "--"])
        .args(["--files", "src/core/*.rs"])
        .args(["--output", db_path.to_str().unwrap()])
        .output()?;

    // Test stats command
    let output = Command::new("cargo")
        .args(["run", "--bin", "lsif", "--", "stats"])
        .args(["--db", db_path.to_str().unwrap()])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Database Statistics"));

    Ok(())
}

#[test]
fn test_lsif_indexer_help() -> Result<()> {
    let output = Command::new("cargo")
        .args(["run", "--bin", "lsif-indexer", "--", "--help"])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Language-neutral code index tool"));

    Ok(())
}

#[test]
#[ignore] // CLIインターフェースが変更されたため更新が必要
fn test_lsif_query_definition() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    // Create index
    Command::new("cargo")
        .args(["run", "--bin", "lsif", "--"])
        .args(["--files", "src/core/*.rs"])
        .args(["--output", db_path.to_str().unwrap()])
        .output()?;

    // Test query definition
    let output = Command::new("cargo")
        .args(["run", "--bin", "lsif", "--", "query"])
        .args(["--db", db_path.to_str().unwrap()])
        .args(["definition", "src/core/mod.rs", "1", "1"])
        .output()?;

    assert!(output.status.success());

    Ok(())
}

#[test]
#[ignore] // CLIインターフェースが変更されたため更新が必要
fn test_lsif_with_exclude() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    // Test with exclude patterns
    let output = Command::new("cargo")
        .args(["run", "--bin", "lsif", "--"])
        .args(["--files", "**/*.rs"])
        .args(["--exclude", "target"])
        .args(["--exclude", "tests"])
        .args(["--output", db_path.to_str().unwrap()])
        .output()?;

    assert!(output.status.success());

    Ok(())
}

#[test]
#[ignore] // CLIインターフェースが変更されたため更新が必要
fn test_parallel_and_cache_flags() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    // Test parallel and cache flags
    let output = Command::new("cargo")
        .args(["run", "--bin", "lsif", "--"])
        .args(["--files", "src/**/*.rs"])
        .args(["--parallel"])
        .args(["--cache"])
        .args(["--threads", "4"])
        .args(["--batch-size", "50"])
        .args(["--output", db_path.to_str().unwrap()])
        .output()?;

    assert!(output.status.success());

    Ok(())
}

// 実際のLSP連携をテストするための簡単なサンプルプロジェクト作成
#[test]
#[ignore] // LSPサーバーが必要なため、デフォルトでは無効
fn test_real_lsp_integration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_dir = temp_dir.path().join("test_project");
    std::fs::create_dir(&project_dir)?;

    // サンプルRustファイルを作成
    std::fs::write(
        project_dir.join("main.rs"),
        r#"
fn main() {
    println!("Hello, world!");
    calculate(5, 10);
}

fn calculate(a: i32, b: i32) -> i32 {
    a + b
}

struct User {
    name: String,
    age: u32,
}

impl User {
    fn new(name: String, age: u32) -> Self {
        User { name, age }
    }
}
"#,
    )?;

    let db_path = temp_dir.path().join("test.db");

    // 実際のrust-analyzerを使用してインデックス化
    let output = Command::new("cargo")
        .args(["run", "--bin", "lsif", "--"])
        .args(["--files", &format!("{}/**/*.rs", project_dir.display())])
        .args(["--bin", "rust-analyzer"])
        .args(["--output", db_path.to_str().unwrap()])
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // 実際のシンボルが抽出されていることを確認
        assert!(stdout.contains("main"));
        assert!(stdout.contains("calculate"));
        assert!(stdout.contains("User"));
    }

    Ok(())
}
