use super::E2eContext;
/// E2E tests for basic CLI commands
use anyhow::Result;

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_help_command() -> Result<()> {
    let ctx = E2eContext::new()?;

    // Test --help flag
    let output = ctx.run_command(&["--help"])?;
    output.assert_success()?;
    output.assert_stdout_contains("AI-optimized code indexer")?;
    output.assert_stdout_contains("Usage:")?;
    output.assert_stdout_contains("Commands:")?;

    // Test help subcommand
    let output = ctx.run_command(&["help"])?;
    output.assert_success()?;
    output.assert_stdout_contains("AI-optimized code indexer")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_version_command() -> Result<()> {
    let ctx = E2eContext::new()?;

    let output = ctx.run_command(&["--version"])?;
    output.assert_success()?;
    output.assert_stdout_contains("lsif")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_invalid_command() -> Result<()> {
    let ctx = E2eContext::new()?;

    let output = ctx.run_command(&["invalid-command"])?;
    assert!(!output.success);

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_project_basic() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("test_project")?;

    // Run index command
    let output = ctx.run_command(&[
        "index",
        "-p",
        project_dir.to_str().unwrap(),
        "-d",
        "index.db",
        "-f",
    ])?;

    output.assert_success()?;
    ctx.assert_file_exists("index.db")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_and_query() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("query_test")?;

    // First, create an index
    let output = ctx.run_command(&[
        "index",
        "-p",
        project_dir.to_str().unwrap(),
        "-d",
        "query_test.db",
        "-f",
    ])?;
    output.assert_success()?;

    // Then query symbols
    let output = ctx.run_command(&[
        "symbols",
        "-p",
        project_dir.to_str().unwrap(),
        "-d",
        "query_test.db",
    ])?;
    output.assert_success()?;
    output.assert_stdout_contains("main")?;
    output.assert_stdout_contains("greet")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_show_stats() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("stats_test")?;

    // Create an index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "stats_test.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Show stats
    let output = ctx.run_command(&["show-stats", "-i", "stats_test.db"])?;
    output.assert_success()?;
    output.assert_stdout_contains("Total symbols")?;
    output.assert_stdout_contains("Files indexed")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_list_symbols() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("list_test")?;

    // Create an index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "list_test.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // List symbols
    let output = ctx.run_command(&["list-symbols", "-i", "list_test.db"])?;
    output.assert_success()?;
    output.assert_stdout_contains("Function: main")?;
    output.assert_stdout_contains("Function: greet")?;
    output.assert_stdout_contains("Function: add")?;
    output.assert_stdout_contains("Struct: Config")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_fuzzy_search() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("fuzzy_test")?;

    // Create an index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "fuzzy_test.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Fuzzy search for "grt" (should find "greet")
    let output = ctx.run_command(&["fuzzy-search", "-i", "fuzzy_test.db", "-q", "grt"])?;
    output.assert_success()?;
    output.assert_stdout_contains("greet")?;

    // Fuzzy search for "cfg" (should find "Config")
    let output = ctx.run_command(&["fuzzy-search", "-i", "fuzzy_test.db", "-q", "cfg"])?;
    output.assert_success()?;
    output.assert_stdout_contains("Config")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_export_lsif() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("export_test")?;

    // Create an index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "export_test.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Export to LSIF
    let output = ctx.run_command(&["export-lsif", "-i", "export_test.db", "-o", "export.lsif"])?;
    output.assert_success()?;
    ctx.assert_file_exists("export.lsif")?;

    // Verify LSIF content
    let lsif_content = ctx.read_file("export.lsif")?;
    assert!(lsif_content.contains("metaData"));
    assert!(lsif_content.contains("document"));
    assert!(lsif_content.contains("range"));

    Ok(())
}
