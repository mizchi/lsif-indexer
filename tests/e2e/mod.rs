/// E2E test helpers and utilities
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// E2E test context that manages temporary directories and binary execution
pub struct E2eContext {
    /// Temporary directory for test files
    pub temp_dir: TempDir,
    /// Path to the compiled binary
    pub binary_path: PathBuf,
}

impl E2eContext {
    /// Create a new E2E test context
    pub fn new() -> Result<Self> {
        // Build the binary in release mode for better performance
        let output = Command::new("cargo")
            .args(["build", "--release", "--bin", "lsif"])
            .output()
            .context("Failed to build binary")?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to build binary: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let binary_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("release")
            .join("lsif");

        let temp_dir = TempDir::new().context("Failed to create temp directory")?;

        Ok(Self {
            temp_dir,
            binary_path,
        })
    }

    /// Execute a command with the lsif-indexer binary
    pub fn run_command(&self, args: &[&str]) -> Result<TestOutput> {
        let output = Command::new(&self.binary_path)
            .args(args)
            .current_dir(&self.temp_dir)
            .output()
            .context("Failed to execute command")?;

        Ok(TestOutput::from(output))
    }

    /// Create a test project with sample files
    pub fn create_test_project(&self, name: &str) -> Result<PathBuf> {
        let project_dir = self.temp_dir.path().join(name);
        fs::create_dir_all(&project_dir)?;

        // Create a simple Rust project structure
        let src_dir = project_dir.join("src");
        fs::create_dir_all(&src_dir)?;

        // Create main.rs
        fs::write(
            src_dir.join("main.rs"),
            r#"
fn main() {
    println!("Hello, world!");
    greet("E2E Test");
}

fn greet(name: &str) {
    println!("Hello, {}!", name);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_greet() {
        greet("Test");
    }
}
"#,
        )?;

        // Create lib.rs
        fs::write(
            src_dir.join("lib.rs"),
            r#"
pub mod utils {
    pub fn add(a: i32, b: i32) -> i32 {
        a + b
    }
    
    pub fn multiply(a: i32, b: i32) -> i32 {
        a * b
    }
}

pub use utils::{add, multiply};
"#,
        )?;

        // Create a module file
        fs::write(
            src_dir.join("config.rs"),
            r#"
pub struct Config {
    pub debug: bool,
    pub port: u16,
}

impl Config {
    pub fn new() -> Self {
        Config {
            debug: false,
            port: 8080,
        }
    }
    
    pub fn with_debug(mut self) -> Self {
        self.debug = true;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
"#,
        )?;

        Ok(project_dir)
    }

    /// Create a TypeScript test project
    pub fn create_typescript_project(&self, name: &str) -> Result<PathBuf> {
        let project_dir = self.temp_dir.path().join(name);
        fs::create_dir_all(&project_dir)?;

        // Create index.ts
        fs::write(
            project_dir.join("index.ts"),
            r#"
export interface User {
    id: number;
    name: string;
    email: string;
}

export class UserService {
    private users: User[] = [];
    
    addUser(user: User): void {
        this.users.push(user);
    }
    
    getUser(id: number): User | undefined {
        return this.users.find(u => u.id === id);
    }
    
    getAllUsers(): User[] {
        return [...this.users];
    }
}

export function createUser(name: string, email: string): User {
    return {
        id: Date.now(),
        name,
        email
    };
}
"#,
        )?;

        // Create utils.ts
        fs::write(
            project_dir.join("utils.ts"),
            r#"
export function formatDate(date: Date): string {
    return date.toISOString();
}

export function parseJson<T>(json: string): T {
    return JSON.parse(json);
}

export const VERSION = "1.0.0";
"#,
        )?;

        Ok(project_dir)
    }

    /// Create a Python test project
    pub fn create_python_project(&self, name: &str) -> Result<PathBuf> {
        let project_dir = self.temp_dir.path().join(name);
        fs::create_dir_all(&project_dir)?;

        // Create main.py
        fs::write(
            project_dir.join("main.py"),
            r#"
from typing import List, Optional

class Person:
    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age
    
    def greet(self) -> str:
        return f"Hello, I'm {self.name}"
    
    def is_adult(self) -> bool:
        return self.age >= 18

def process_people(people: List[Person]) -> List[str]:
    return [p.greet() for p in people if p.is_adult()]

def main():
    people = [
        Person("Alice", 25),
        Person("Bob", 17),
        Person("Charlie", 30)
    ]
    greetings = process_people(people)
    for greeting in greetings:
        print(greeting)

if __name__ == "__main__":
    main()
"#,
        )?;

        // Create utils.py
        fs::write(
            project_dir.join("utils.py"),
            r#"
import json
from typing import Any, Dict

def load_config(path: str) -> Dict[str, Any]:
    with open(path, 'r') as f:
        return json.load(f)

def save_config(path: str, config: Dict[str, Any]) -> None:
    with open(path, 'w') as f:
        json.dump(config, f, indent=2)

class Logger:
    def __init__(self, name: str):
        self.name = name
    
    def log(self, message: str) -> None:
        print(f"[{self.name}] {message}")
"#,
        )?;

        Ok(project_dir)
    }

    /// Assert that a file exists
    pub fn assert_file_exists(&self, path: impl AsRef<Path>) -> Result<()> {
        let full_path = if path.as_ref().is_absolute() {
            path.as_ref().to_path_buf()
        } else {
            self.temp_dir.path().join(path)
        };

        if !full_path.exists() {
            anyhow::bail!("File does not exist: {}", full_path.display());
        }

        Ok(())
    }

    /// Read a file from the temp directory
    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<String> {
        let full_path = if path.as_ref().is_absolute() {
            path.as_ref().to_path_buf()
        } else {
            self.temp_dir.path().join(path)
        };

        fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read file: {}", full_path.display()))
    }
}

/// Test output wrapper
pub struct TestOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub exit_code: Option<i32>,
}

impl From<Output> for TestOutput {
    fn from(output: Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            success: output.status.success(),
            exit_code: output.status.code(),
        }
    }
}

impl TestOutput {
    /// Assert that the command succeeded
    pub fn assert_success(&self) -> Result<()> {
        if !self.success {
            anyhow::bail!(
                "Command failed with exit code {:?}\nstderr: {}",
                self.exit_code,
                self.stderr
            );
        }
        Ok(())
    }

    /// Assert that the stdout contains a string
    pub fn assert_stdout_contains(&self, text: &str) -> Result<()> {
        if !self.stdout.contains(text) {
            anyhow::bail!(
                "stdout does not contain '{}'\nActual stdout:\n{}",
                text,
                self.stdout
            );
        }
        Ok(())
    }
}
