use super::E2eContext;
/// E2E tests for differential indexing
use anyhow::Result;
use std::fs;
use std::thread;
use std::time::Duration;

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_differential_index_basic() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("diff_basic");
    fs::create_dir_all(&project_dir)?;

    // Create initial file
    fs::write(
        project_dir.join("main.rs"),
        r#"
fn initial_function() {
    println!("Initial");
}
"#,
    )?;

    // Initial full index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_basic.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Verify initial state
    let output = ctx.run_command(&["list-symbols", "-i", "diff_basic.db"])?;
    output.assert_stdout_contains("initial_function")?;

    // Wait a moment to ensure file modification time changes
    thread::sleep(Duration::from_millis(100));

    // Modify the file
    fs::write(
        project_dir.join("main.rs"),
        r#"
fn initial_function() {
    println!("Modified");
}

fn new_function() {
    println!("New");
}
"#,
    )?;

    // Run differential index
    let output = ctx.run_command(&[
        "differential-index",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_basic.db",
    ])?;
    output.assert_success()?;

    // Verify both functions are present
    let output = ctx.run_command(&["list-symbols", "-i", "diff_basic.db"])?;
    output.assert_stdout_contains("initial_function")?;
    output.assert_stdout_contains("new_function")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_differential_index_new_file() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("diff_new_file");
    fs::create_dir_all(&project_dir)?;

    // Create initial file
    fs::write(
        project_dir.join("file1.rs"),
        r#"
fn file1_function() {
    println!("File 1");
}
"#,
    )?;

    // Initial index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_new.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Add new file
    fs::write(
        project_dir.join("file2.rs"),
        r#"
fn file2_function() {
    println!("File 2");
}
"#,
    )?;

    // Run differential index
    let output = ctx.run_command(&[
        "differential-index",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_new.db",
    ])?;
    output.assert_success()?;

    // Verify both files' symbols are present
    let output = ctx.run_command(&["list-symbols", "-i", "diff_new.db"])?;
    output.assert_stdout_contains("file1_function")?;
    output.assert_stdout_contains("file2_function")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_differential_index_deleted_file() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("diff_delete");
    fs::create_dir_all(&project_dir)?;

    // Create two files
    fs::write(
        project_dir.join("keep.rs"),
        r#"
fn keep_function() {
    println!("Keep");
}
"#,
    )?;

    fs::write(
        project_dir.join("delete.rs"),
        r#"
fn delete_function() {
    println!("Delete");
}
"#,
    )?;

    // Initial index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_delete.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Verify both functions exist
    let output = ctx.run_command(&["list-symbols", "-i", "diff_delete.db"])?;
    output.assert_stdout_contains("keep_function")?;
    output.assert_stdout_contains("delete_function")?;

    // Delete one file
    fs::remove_file(project_dir.join("delete.rs"))?;

    // Run differential index
    let output = ctx.run_command(&[
        "differential-index",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_delete.db",
    ])?;
    output.assert_success()?;

    // Verify only kept function remains
    let output = ctx.run_command(&["list-symbols", "-i", "diff_delete.db"])?;
    output.assert_stdout_contains("keep_function")?;
    assert!(!output.stdout.contains("delete_function"));

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_differential_index_multiple_changes() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("diff_multiple");
    fs::create_dir_all(&project_dir)?;

    // Create initial files
    fs::write(
        project_dir.join("unchanged.rs"),
        r#"
fn unchanged_function() {
    println!("Unchanged");
}
"#,
    )?;

    fs::write(
        project_dir.join("modify.rs"),
        r#"
fn old_function() {
    println!("Old");
}
"#,
    )?;

    fs::write(
        project_dir.join("delete.rs"),
        r#"
fn to_delete() {
    println!("Delete me");
}
"#,
    )?;

    // Initial index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_multiple.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Wait to ensure timestamp changes
    thread::sleep(Duration::from_millis(100));

    // Make multiple changes
    // 1. Modify existing file
    fs::write(
        project_dir.join("modify.rs"),
        r#"
fn new_function() {
    println!("New");
}

fn additional_function() {
    println!("Additional");
}
"#,
    )?;

    // 2. Delete a file
    fs::remove_file(project_dir.join("delete.rs"))?;

    // 3. Add new file
    fs::write(
        project_dir.join("new.rs"),
        r#"
fn brand_new_function() {
    println!("Brand new");
}
"#,
    )?;

    // Run differential index
    let output = ctx.run_command(&[
        "differential-index",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_multiple.db",
    ])?;
    output.assert_success()?;

    // Verify expected state
    let output = ctx.run_command(&["list-symbols", "-i", "diff_multiple.db"])?;

    // Unchanged file should still be there
    output.assert_stdout_contains("unchanged_function")?;

    // Modified file should have new functions
    output.assert_stdout_contains("new_function")?;
    output.assert_stdout_contains("additional_function")?;
    assert!(!output.stdout.contains("old_function"));

    // Deleted file's function should be gone
    assert!(!output.stdout.contains("to_delete"));

    // New file's function should be present
    output.assert_stdout_contains("brand_new_function")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_differential_index_performance() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("diff_perf");
    fs::create_dir_all(&project_dir)?;

    // Create many files
    for i in 0..20 {
        fs::write(
            project_dir.join(format!("file_{}.rs", i)),
            format!(
                r#"
fn function_{}() {{
    println!("Function {{}}", {});
}}
"#,
                i, i
            ),
        )?;
    }

    // Initial index
    let start = std::time::Instant::now();
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_perf.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;
    let initial_time = start.elapsed();

    // Wait to ensure timestamp changes
    thread::sleep(Duration::from_millis(100));

    // Modify just one file
    fs::write(
        project_dir.join("file_5.rs"),
        r#"
fn function_5_modified() {
    println!("Modified");
}

fn new_function() {
    println!("New");
}
"#,
    )?;

    // Run differential index
    let start = std::time::Instant::now();
    let output = ctx.run_command(&[
        "differential-index",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_perf.db",
    ])?;
    output.assert_success()?;
    let diff_time = start.elapsed();

    // Differential indexing should be much faster than initial
    assert!(
        diff_time < initial_time / 2,
        "Differential indexing should be faster than initial indexing"
    );

    // Verify the change was applied
    let output = ctx.run_command(&["list-symbols", "-i", "diff_perf.db"])?;
    output.assert_stdout_contains("function_5_modified")?;
    output.assert_stdout_contains("new_function")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_differential_index_with_errors() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("diff_errors");
    fs::create_dir_all(&project_dir)?;

    // Create initial valid file
    fs::write(
        project_dir.join("valid.rs"),
        r#"
fn valid_function() {
    println!("Valid");
}
"#,
    )?;

    // Initial index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_errors.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Wait to ensure timestamp changes
    thread::sleep(Duration::from_millis(100));

    // Add file with syntax errors
    fs::write(
        project_dir.join("invalid.rs"),
        r#"
fn invalid_syntax() {
    this is not valid rust
    missing semicolons
}

fn partially_valid() {
    println!("This part is valid");
}
"#,
    )?;

    // Differential index should still work
    let output = ctx.run_command(&[
        "differential-index",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_errors.db",
    ])?;
    output.assert_success()?;

    // Valid functions should still be indexed
    let output = ctx.run_command(&["list-symbols", "-i", "diff_errors.db"])?;
    output.assert_stdout_contains("valid_function")?;
    output.assert_stdout_contains("partially_valid")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_differential_index_nested_directories() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("diff_nested");
    let src_dir = project_dir.join("src");
    let module_dir = src_dir.join("module");
    fs::create_dir_all(&module_dir)?;

    // Create initial structure
    fs::write(
        src_dir.join("main.rs"),
        r#"
mod module;

fn main() {
    println!("Main");
}
"#,
    )?;

    fs::write(
        module_dir.join("mod.rs"),
        r#"
pub fn module_function() {
    println!("Module");
}
"#,
    )?;

    // Initial index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_nested.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Wait to ensure timestamp changes
    thread::sleep(Duration::from_millis(100));

    // Add new file in nested directory
    fs::write(
        module_dir.join("helper.rs"),
        r#"
pub fn helper_function() {
    println!("Helper");
}
"#,
    )?;

    // Modify module file
    fs::write(
        module_dir.join("mod.rs"),
        r#"
mod helper;

pub fn module_function() {
    helper::helper_function();
}

pub fn new_module_function() {
    println!("New module function");
}
"#,
    )?;

    // Run differential index
    let output = ctx.run_command(&[
        "differential-index",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_nested.db",
    ])?;
    output.assert_success()?;

    // Verify all functions are indexed
    let output = ctx.run_command(&["list-symbols", "-i", "diff_nested.db"])?;
    output.assert_stdout_contains("main")?;
    output.assert_stdout_contains("module_function")?;
    output.assert_stdout_contains("new_module_function")?;
    output.assert_stdout_contains("helper_function")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_differential_index_rename_detection() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("diff_rename");
    fs::create_dir_all(&project_dir)?;

    // Create initial file
    fs::write(
        project_dir.join("old_name.rs"),
        r#"
fn renamed_function() {
    println!("This function will move");
}
"#,
    )?;

    // Initial index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_rename.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Wait to ensure timestamp changes
    thread::sleep(Duration::from_millis(100));

    // Rename file (delete old, create new with same content)
    let content = fs::read_to_string(project_dir.join("old_name.rs"))?;
    fs::remove_file(project_dir.join("old_name.rs"))?;
    fs::write(project_dir.join("new_name.rs"), content)?;

    // Run differential index
    let output = ctx.run_command(&[
        "differential-index",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_rename.db",
    ])?;
    output.assert_success()?;

    // Function should still be indexed
    let output = ctx.run_command(&["list-symbols", "-i", "diff_rename.db"])?;
    output.assert_stdout_contains("renamed_function")?;

    // Verify it's now in the new file
    let output = ctx.run_command(&["query", "-i", "diff_rename.db", "--query-type", "symbols"])?;
    output.assert_stdout_contains("new_name.rs")?;
    assert!(!output.stdout.contains("old_name.rs"));

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_differential_index_concurrent_modifications() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("diff_concurrent");
    fs::create_dir_all(&project_dir)?;

    // Create initial files
    for i in 0..5 {
        fs::write(
            project_dir.join(format!("file_{}.rs", i)),
            format!(
                r#"
fn function_{}() {{
    println!("Initial {{}}", {});
}}
"#,
                i, i
            ),
        )?;
    }

    // Initial index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_concurrent.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Wait to ensure timestamp changes
    thread::sleep(Duration::from_millis(100));

    // Modify multiple files "concurrently"
    for i in 0..5 {
        fs::write(
            project_dir.join(format!("file_{}.rs", i)),
            format!(
                r#"
fn function_{}_modified() {{
    println!("Modified {{}}", {});
}}

fn additional_{}() {{
    println!("Additional {{}}", {});
}}
"#,
                i, i, i, i
            ),
        )?;
    }

    // Run differential index
    let output = ctx.run_command(&[
        "differential-index",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "diff_concurrent.db",
    ])?;
    output.assert_success()?;

    // Verify all modifications were indexed
    let output = ctx.run_command(&["list-symbols", "-i", "diff_concurrent.db"])?;

    for i in 0..5 {
        output.assert_stdout_contains(&format!("function_{}_modified", i))?;
        output.assert_stdout_contains(&format!("additional_{}", i))?;
        // Old functions should be gone
        assert!(!output.stdout.contains(&format!("function_{}\n", i)));
    }

    Ok(())
}
