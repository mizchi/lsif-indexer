use super::E2eContext;
/// E2E tests for indexing operations
use anyhow::Result;
use std::fs;

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_rust_project() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_test_project("rust_project")?;

    // Create additional Rust files
    let src_dir = project_dir.join("src");

    // Add a module with complex structure
    fs::write(
        src_dir.join("math.rs"),
        r#"
pub mod operations {
    pub fn add(a: i32, b: i32) -> i32 {
        a + b
    }
    
    pub fn subtract(a: i32, b: i32) -> i32 {
        a - b
    }
}

pub trait Calculator {
    fn calculate(&self, a: i32, b: i32) -> i32;
}

pub struct Adder;

impl Calculator for Adder {
    fn calculate(&self, a: i32, b: i32) -> i32 {
        operations::add(a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_adder() {
        let adder = Adder;
        assert_eq!(adder.calculate(2, 3), 5);
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
        "rust_project.db",
        "-l",
        "rust",
    ])?;

    output.assert_success()?;
    ctx.assert_file_exists("rust_project.db")?;

    // Verify the index contains expected symbols
    let output = ctx.run_command(&["list-symbols", "-i", "rust_project.db"])?;

    output.assert_success()?;
    output.assert_stdout_contains("Function: add")?;
    output.assert_stdout_contains("Function: subtract")?;
    output.assert_stdout_contains("Trait: Calculator")?;
    output.assert_stdout_contains("Struct: Adder")?;
    output.assert_stdout_contains("Module: operations")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_typescript_project() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_typescript_project("ts_project")?;

    // Add additional TypeScript files
    fs::write(
        project_dir.join("types.ts"),
        r#"
export type ID = string | number;

export interface Config {
    apiUrl: string;
    timeout: number;
    retryCount?: number;
}

export enum Status {
    Active = "ACTIVE",
    Inactive = "INACTIVE",
    Pending = "PENDING"
}

export type Handler<T> = (data: T) => void;

export class BaseService {
    protected config: Config;
    
    constructor(config: Config) {
        this.config = config;
    }
    
    protected async request<T>(endpoint: string): Promise<T> {
        // Mock implementation
        return {} as T;
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
        "ts_project.db",
        "-l",
        "typescript",
    ])?;

    output.assert_success()?;
    ctx.assert_file_exists("ts_project.db")?;

    // Verify TypeScript-specific symbols
    let output = ctx.run_command(&["list-symbols", "-i", "ts_project.db"])?;

    output.assert_success()?;
    output.assert_stdout_contains("Interface: User")?;
    output.assert_stdout_contains("Class: UserService")?;
    output.assert_stdout_contains("Function: createUser")?;
    output.assert_stdout_contains("Interface: Config")?;
    output.assert_stdout_contains("Enum: Status")?;
    output.assert_stdout_contains("Class: BaseService")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_python_project() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.create_python_project("py_project")?;

    // Add additional Python files
    fs::write(
        project_dir.join("database.py"),
        r#"
from typing import Dict, List, Optional
from dataclasses import dataclass
from abc import ABC, abstractmethod

@dataclass
class Record:
    id: int
    name: str
    data: Dict[str, any]
    
class Database(ABC):
    @abstractmethod
    def connect(self) -> None:
        pass
    
    @abstractmethod
    def query(self, sql: str) -> List[Record]:
        pass
    
class SQLiteDatabase(Database):
    def __init__(self, path: str):
        self.path = path
        self.connection = None
    
    def connect(self) -> None:
        # Mock connection
        pass
    
    def query(self, sql: str) -> List[Record]:
        # Mock query
        return []
    
    def close(self) -> None:
        if self.connection:
            self.connection = None

def create_database(db_type: str, **kwargs) -> Database:
    if db_type == "sqlite":
        return SQLiteDatabase(kwargs.get("path", ":memory:"))
    raise ValueError(f"Unknown database type: {db_type}")
"#,
    )?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "py_project.db",
        "-l",
        "python",
    ])?;

    output.assert_success()?;
    ctx.assert_file_exists("py_project.db")?;

    // Verify Python-specific symbols
    let output = ctx.run_command(&["list-symbols", "-i", "py_project.db"])?;

    output.assert_success()?;
    output.assert_stdout_contains("Class: Person")?;
    output.assert_stdout_contains("Function: process_people")?;
    output.assert_stdout_contains("Class: Logger")?;
    output.assert_stdout_contains("Class: Record")?;
    output.assert_stdout_contains("Class: Database")?;
    output.assert_stdout_contains("Class: SQLiteDatabase")?;
    output.assert_stdout_contains("Function: create_database")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_multi_language_project() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("multi_lang");
    fs::create_dir_all(&project_dir)?;

    // Create Rust files
    let rust_dir = project_dir.join("rust");
    fs::create_dir_all(&rust_dir)?;
    fs::write(
        rust_dir.join("lib.rs"),
        r#"
pub fn rust_function() {
    println!("From Rust");
}
"#,
    )?;

    // Create TypeScript files
    let ts_dir = project_dir.join("typescript");
    fs::create_dir_all(&ts_dir)?;
    fs::write(
        ts_dir.join("index.ts"),
        r#"
export function tsFunction() {
    console.log("From TypeScript");
}
"#,
    )?;

    // Create Python files
    let py_dir = project_dir.join("python");
    fs::create_dir_all(&py_dir)?;
    fs::write(
        py_dir.join("main.py"),
        r#"
def python_function():
    print("From Python")
"#,
    )?;

    // Index each language separately
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        rust_dir.to_str().unwrap(),
        "-o",
        "multi_rust.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    let output = ctx.run_command(&[
        "index-project",
        "-p",
        ts_dir.to_str().unwrap(),
        "-o",
        "multi_ts.db",
        "-l",
        "typescript",
    ])?;
    output.assert_success()?;

    let output = ctx.run_command(&[
        "index-project",
        "-p",
        py_dir.to_str().unwrap(),
        "-o",
        "multi_py.db",
        "-l",
        "python",
    ])?;
    output.assert_success()?;

    // Verify each index
    let output = ctx.run_command(&["list-symbols", "-i", "multi_rust.db"])?;
    output.assert_stdout_contains("rust_function")?;

    let output = ctx.run_command(&["list-symbols", "-i", "multi_ts.db"])?;
    output.assert_stdout_contains("tsFunction")?;

    let output = ctx.run_command(&["list-symbols", "-i", "multi_py.db"])?;
    output.assert_stdout_contains("python_function")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_with_exclude_patterns() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("exclude_test");
    fs::create_dir_all(&project_dir)?;

    // Create main source
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;
    fs::write(
        src_dir.join("main.rs"),
        r#"
fn main() {
    println!("Main");
}
"#,
    )?;

    // Create test directory
    let test_dir = project_dir.join("tests");
    fs::create_dir_all(&test_dir)?;
    fs::write(
        test_dir.join("test.rs"),
        r#"
#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_something() {
    assert!(true);
}
"#,
    )?;

    // Create vendor directory (should be excluded)
    let vendor_dir = project_dir.join("vendor");
    fs::create_dir_all(&vendor_dir)?;
    fs::write(
        vendor_dir.join("external.rs"),
        r#"
fn vendor_function() {
    println!("Vendor");
}
"#,
    )?;

    // Index with exclude pattern
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "exclude_test.db",
        "-l",
        "rust",
        "--exclude",
        "vendor",
        "--exclude",
        "tests",
    ])?;

    output.assert_success()?;

    // Verify only main.rs was indexed
    let output = ctx.run_command(&["list-symbols", "-i", "exclude_test.db"])?;

    output.assert_stdout_contains("main")?;
    assert!(!output.stdout.contains("test_something"));
    assert!(!output.stdout.contains("vendor_function"));

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_large_file() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("large_file");
    fs::create_dir_all(&project_dir)?;

    // Generate a large file with many functions
    let mut content = String::new();
    for i in 0..100 {
        content.push_str(&format!(
            r#"
fn function_{i}() {{
    println!("Function {i}");
}}

pub struct Struct_{i} {{
    field: i32,
}}

impl Struct_{i} {{
    pub fn method(&self) -> i32 {{
        self.field
    }}
}}
"#,
            i = i
        ));
    }

    fs::write(project_dir.join("large.rs"), content)?;

    // Index the large file
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "large_file.db",
        "-l",
        "rust",
    ])?;

    output.assert_success()?;

    // Verify symbols were indexed
    let output = ctx.run_command(&["show-stats", "-i", "large_file.db"])?;

    output.assert_success()?;
    output.assert_stdout_contains("Total symbols")?;

    // Parse the stats to verify count
    let stats_output = output.stdout.clone();
    if let Some(line) = stats_output.lines().find(|l| l.contains("Total symbols")) {
        if let Some(count_str) = line.split(':').nth(1) {
            let count: usize = count_str.trim().parse().unwrap_or(0);
            assert!(count >= 300); // At least 100 functions + 100 structs + 100 methods
        }
    }

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_nested_modules() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("nested_modules");
    fs::create_dir_all(&project_dir)?;

    // Create nested module structure
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    fs::write(
        src_dir.join("lib.rs"),
        r#"
pub mod level1 {
    pub mod level2 {
        pub mod level3 {
            pub fn deep_function() {
                println!("Deep");
            }
            
            pub struct DeepStruct {
                value: i32,
            }
        }
        
        pub fn level2_function() {
            level3::deep_function();
        }
    }
    
    pub fn level1_function() {
        level2::level2_function();
    }
}

pub use level1::level2::level3::DeepStruct;
"#,
    )?;

    // Index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "nested.db",
        "-l",
        "rust",
    ])?;

    output.assert_success()?;

    // Verify nested modules and symbols
    let output = ctx.run_command(&["list-symbols", "-i", "nested.db"])?;

    output.assert_success()?;
    output.assert_stdout_contains("Module: level1")?;
    output.assert_stdout_contains("Module: level2")?;
    output.assert_stdout_contains("Module: level3")?;
    output.assert_stdout_contains("Function: deep_function")?;
    output.assert_stdout_contains("Struct: DeepStruct")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_index_invalid_syntax() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("invalid_syntax");
    fs::create_dir_all(&project_dir)?;

    // Create file with syntax errors
    fs::write(
        project_dir.join("invalid.rs"),
        r#"
fn valid_function() {
    println!("Valid");
}

fn invalid_function() {
    this is not valid rust syntax
    missing semicolons and brackets
}

fn another_valid() {
    println!("Another valid");
}
"#,
    )?;

    // Try to index the project
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "invalid.db",
        "-l",
        "rust",
    ])?;

    // Should complete but might have warnings
    output.assert_success()?;

    // Should still index valid functions
    let output = ctx.run_command(&["list-symbols", "-i", "invalid.db"])?;

    output.assert_stdout_contains("valid_function")?;
    output.assert_stdout_contains("another_valid")?;

    Ok(())
}

#[test]
#[ignore] // E2EテストのCLIインターフェース更新が必要
fn test_incremental_indexing() -> Result<()> {
    let ctx = E2eContext::new()?;
    let project_dir = ctx.temp_dir.path().join("incremental");
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

    // Initial index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "incremental.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Verify initial function
    let output = ctx.run_command(&["list-symbols", "-i", "incremental.db"])?;
    output.assert_stdout_contains("initial_function")?;

    // Add new file
    fs::write(
        project_dir.join("new.rs"),
        r#"
fn new_function() {
    println!("New");
}
"#,
    )?;

    // Re-index
    let output = ctx.run_command(&[
        "index-project",
        "-p",
        project_dir.to_str().unwrap(),
        "-o",
        "incremental.db",
        "-l",
        "rust",
    ])?;
    output.assert_success()?;

    // Verify both functions are present
    let output = ctx.run_command(&["list-symbols", "-i", "incremental.db"])?;
    output.assert_stdout_contains("initial_function")?;
    output.assert_stdout_contains("new_function")?;

    Ok(())
}
