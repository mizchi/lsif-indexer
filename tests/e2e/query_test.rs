use super::E2eContext;
/// E2E tests for query operations
use anyhow::Result;
use std::fs;

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_query_definitions() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("query_def")?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "query_def.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Query for definitions at specific location
    let output = ctx.run_command(&[
        "query",
        "-i",
        "query_def.db",
        "--query-type",
        "definition",
        "-f",
        &format!("{}/src/main.rs", project_dir.to_str().unwrap()),
        "-l",
        "4", // Line where greet is called
        "-c",
        "5",
    ])?;

    output.assert_success()?;
    output.assert_stdout_contains("greet")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_query_references() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("query_ref")?;

    // Create file with multiple references
    let src_dir = project_dir.join("src");
    fs::write(
        src_dir.join("lib2.rs"),
        r#"
use crate::utils::{add, multiply};

pub fn calculate() {
    let sum = add(5, 3);
    let product = multiply(4, 2);
    let another_sum = add(10, 20);
    println!("Results: {} {}", sum, product);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::add;
    
    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
"#,
    )?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "query_ref.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Query for references to 'add' function
    let output = ctx.run_command(&[
        "references",
        "-p",
        project_dir.to_str().unwrap(),
        "-n",
        "add",
        "-k",
        "function",
    ])?;

    output.assert_success()?;
    // Should find multiple references
    output.assert_stdout_contains("lib.rs")?; // Definition
    output.assert_stdout_contains("lib2.rs")?; // Usage

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_query_symbols() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("query_symbols")?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "query_symbols.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Query all symbols
    let output =
        ctx.run_command(&["query", "-i", "query_symbols.db", "--query-type", "symbols"])?;

    output.assert_success()?;
    output.assert_stdout_contains("main")?;
    output.assert_stdout_contains("greet")?;
    output.assert_stdout_contains("Config")?;
    output.assert_stdout_contains("add")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_query_hover() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("query_hover")?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "query_hover.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Query hover information
    let output = ctx.run_command(&[
        "query",
        "-i",
        "query_hover.db",
        "--query-type",
        "hover",
        "-f",
        &format!("{}/src/config.rs", project_dir.to_str().unwrap()),
        "-l",
        "2", // Config struct line
        "-c",
        "12", // Position on "Config"
    ])?;

    output.assert_success()?;
    // Should show information about Config struct
    output.assert_stdout_contains("Config")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_call_hierarchy() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("call_hierarchy");
    fs::create_dir_all(&project_dir)?;

    // Create complex call hierarchy
    fs::write(
        project_dir.join("main.rs"),
        r#"
fn main() {
    top_level();
}

fn top_level() {
    middle_level_a();
    middle_level_b();
}

fn middle_level_a() {
    bottom_level();
}

fn middle_level_b() {
    bottom_level();
}

fn bottom_level() {
    println!("Bottom");
}
"#,
    )?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "hierarchy.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Get call hierarchy for bottom_level
    let output = ctx.run_command(&[
        "call-hierarchy",
        "-i",
        "hierarchy.db",
        "-s",
        "bottom_level",
        "-d",
        "incoming",
    ])?;

    output.assert_success()?;
    output.assert_stdout_contains("middle_level_a")?;
    output.assert_stdout_contains("middle_level_b")?;

    // Get outgoing calls for top_level
    let output = ctx.run_command(&[
        "call-hierarchy",
        "-i",
        "hierarchy.db",
        "-s",
        "top_level",
        "-d",
        "outgoing",
    ])?;

    output.assert_success()?;
    output.assert_stdout_contains("middle_level_a")?;
    output.assert_stdout_contains("middle_level_b")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_find_dead_code() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("dead_code");
    fs::create_dir_all(&project_dir)?;

    // Create file with unused functions
    fs::write(
        project_dir.join("main.rs"),
        r#"
fn main() {
    used_function();
}

fn used_function() {
    println!("Used");
}

fn unused_function() {
    println!("Unused");
}

fn another_unused() {
    println!("Also unused");
}

// Private functions that are never called
fn dead_code_1() {
    dead_code_2();
}

fn dead_code_2() {
    println!("Dead");
}
"#,
    )?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "dead_code.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Find dead code
    let output = ctx.run_command(&["show-dead-code", "-i", "dead_code.db"])?;

    output.assert_success()?;
    output.assert_stdout_contains("unused_function")?;
    output.assert_stdout_contains("another_unused")?;
    output.assert_stdout_contains("dead_code_1")?;

    // Should not list used functions
    assert!(!output.stdout.contains("main"));
    assert!(!output.stdout.contains("used_function"));

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_type_hierarchy() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("type_hierarchy");
    fs::create_dir_all(&project_dir)?;

    // Create file with type hierarchy
    fs::write(
        project_dir.join("types.rs"),
        r#"
trait Animal {
    fn make_sound(&self);
}

trait Mammal: Animal {
    fn feed_young(&self);
}

struct Dog;

impl Animal for Dog {
    fn make_sound(&self) {
        println!("Woof!");
    }
}

impl Mammal for Dog {
    fn feed_young(&self) {
        println!("Feeding puppies");
    }
}

struct Cat;

impl Animal for Cat {
    fn make_sound(&self) {
        println!("Meow!");
    }
}
"#,
    )?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "types.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Query type hierarchy
    let output = ctx.run_command(&["type-hierarchy", "-i", "types.db", "-t", "Dog"])?;

    output.assert_success()?;
    output.assert_stdout_contains("Animal")?;
    output.assert_stdout_contains("Mammal")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_symbol_search_patterns() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("search_patterns");
    fs::create_dir_all(&project_dir)?;

    // Create file with various symbols
    fs::write(
        project_dir.join("symbols.rs"),
        r#"
fn test_function_one() {}
fn test_function_two() {}
fn another_test_function() {}
fn completely_different() {}

struct TestStruct;
struct AnotherTestStruct;
struct NonTestStruct;

impl TestStruct {
    fn test_method(&self) {}
}
"#,
    )?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "patterns.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Search with pattern matching
    let output =
        ctx.run_command(&["search-symbols", "-i", "patterns.db", "-p", "test_function"])?;

    output.assert_success()?;
    output.assert_stdout_contains("test_function_one")?;
    output.assert_stdout_contains("test_function_two")?;
    output.assert_stdout_contains("another_test_function")?;
    assert!(!output.stdout.contains("completely_different"));

    // Search for structs
    let output = ctx.run_command(&[
        "search-symbols",
        "-i",
        "patterns.db",
        "-p",
        "Test",
        "-t",
        "struct",
    ])?;

    output.assert_success()?;
    output.assert_stdout_contains("TestStruct")?;
    output.assert_stdout_contains("AnotherTestStruct")?;
    assert!(!output.stdout.contains("NonTestStruct"));

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_workspace_symbols() -> Result<()> {
    let ctx = E2eContext::new()?;
    let workspace_dir = ctx.temp_dir.path().join("workspace");

    // Create workspace structure
    let crate1_dir = workspace_dir.join("crate1");
    fs::create_dir_all(&crate1_dir)?;
    fs::write(
        crate1_dir.join("lib.rs"),
        r#"
pub fn crate1_function() {
    println!("Crate 1");
}
"#,
    )?;

    let crate2_dir = workspace_dir.join("crate2");
    fs::create_dir_all(&crate2_dir)?;
    fs::write(
        crate2_dir.join("lib.rs"),
        r#"
pub fn crate2_function() {
    println!("Crate 2");
}
"#,
    )?;

    // Index the entire workspace
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        workspace_dir.to_str().unwrap(),
        "-o",
        "workspace.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Query workspace symbols
    let output = ctx.run_command(&["workspace-symbols", "-i", "workspace.db", "-q", "function"])?;

    output.assert_success()?;
    output.assert_stdout_contains("crate1_function")?;
    output.assert_stdout_contains("crate2_function")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_semantic_tokens() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("semantic");
    fs::create_dir_all(&project_dir)?;

    // Create file with various token types
    fs::write(
        project_dir.join("semantic.rs"),
        r#"
const CONSTANT: i32 = 42;

#[derive(Debug)]
struct MyStruct {
    field: String,
}

impl MyStruct {
    fn new() -> Self {
        MyStruct {
            field: String::from("value"),
        }
    }
}

fn main() {
    let instance = MyStruct::new();
    println!("{:?}", instance);
}
"#,
    )?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "semantic.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Get semantic tokens
    let output = ctx.run_command(&[
        "semantic-tokens",
        "-i",
        "semantic.db",
        "-f",
        &format!("{}/semantic.rs", project_dir.to_str().unwrap()),
    ])?;

    output.assert_success()?;
    // Should identify different token types
    output.assert_stdout_contains("constant")?;
    output.assert_stdout_contains("struct")?;
    output.assert_stdout_contains("function")?;
    output.assert_stdout_contains("variable")?;

    Ok(())
}
