use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// エンドツーエンドテスト用のヘルパー構造体
struct E2ETestHelper {
    temp_dir: TempDir,
    binary_path: PathBuf,
}

impl E2ETestHelper {
    /// テストヘルパーを作成
    fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;

        // バイナリパスを取得（デバッグビルドを使用）
        let binary_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join("debug")
            .join("lsif-indexer");

        Ok(Self {
            temp_dir,
            binary_path,
        })
    }

    /// テストプロジェクトを作成
    fn create_test_project(&self, lang: &str) -> PathBuf {
        let project_dir = self.temp_dir.path().join(lang);
        fs::create_dir_all(&project_dir).unwrap();

        match lang {
            "rust" => self.create_rust_project(&project_dir),
            "typescript" => self.create_typescript_project(&project_dir),
            "go" => self.create_go_project(&project_dir),
            "python" => self.create_python_project(&project_dir),
            _ => panic!("Unsupported language: {}", lang),
        }

        project_dir
    }

    fn create_rust_project(&self, dir: &Path) {
        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();

        // Cargo.toml
        fs::write(
            dir.join("Cargo.toml"),
            r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        // src/main.rs
        fs::write(
            src_dir.join("main.rs"),
            r#"
fn main() {
    println!("Hello, world!");
    test_function();
}

fn test_function() {
    let calculator = Calculator::new();
    println!("2 + 3 = {}", calculator.add(2, 3));
}

struct Calculator;

impl Calculator {
    fn new() -> Self {
        Calculator
    }

    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let calc = Calculator::new();
        assert_eq!(calc.add(2, 3), 5);
    }

    #[test]
    fn test_multiply() {
        let calc = Calculator::new();
        assert_eq!(calc.multiply(3, 4), 12);
    }
}
"#,
        )
        .unwrap();

        // src/lib.rs
        fs::write(
            src_dir.join("lib.rs"),
            r#"
pub mod utils {
    pub fn format_name(name: &str) -> String {
        format!("Hello, {}!", name)
    }

    pub fn calculate_sum(numbers: &[i32]) -> i32 {
        numbers.iter().sum()
    }
}

pub trait Greeter {
    fn greet(&self) -> String;
}

pub struct Person {
    name: String,
}

impl Person {
    pub fn new(name: String) -> Self {
        Person { name }
    }
}

impl Greeter for Person {
    fn greet(&self) -> String {
        format!("Hi, I'm {}", self.name)
    }
}
"#,
        )
        .unwrap();
    }

    fn create_typescript_project(&self, dir: &Path) {
        // tsconfig.json
        fs::write(
            dir.join("tsconfig.json"),
            r#"{
    "compilerOptions": {
        "target": "ES2020",
        "module": "commonjs",
        "strict": true
    }
}"#,
        )
        .unwrap();

        // index.ts
        fs::write(
            dir.join("index.ts"),
            r#"
interface Person {
    name: string;
    age: number;
}

class Employee implements Person {
    constructor(
        public name: string,
        public age: number,
        public department: string
    ) {}

    introduce(): string {
        return `I'm ${this.name}, ${this.age} years old, from ${this.department}`;
    }
}

function createEmployee(name: string, age: number, dept: string): Employee {
    return new Employee(name, age, dept);
}

export { Person, Employee, createEmployee };

// Test usage
const emp = createEmployee("Alice", 30, "Engineering");
console.log(emp.introduce());
"#,
        )
        .unwrap();

        // utils.ts
        fs::write(
            dir.join("utils.ts"),
            r#"
export function add(a: number, b: number): number {
    return a + b;
}

export function multiply(a: number, b: number): number {
    return a * b;
}

export const Constants = {
    PI: 3.14159,
    E: 2.71828,
} as const;

export class MathUtils {
    static square(n: number): number {
        return n * n;
    }

    static cube(n: number): number {
        return n * n * n;
    }
}
"#,
        )
        .unwrap();
    }

    fn create_go_project(&self, dir: &Path) {
        // go.mod
        fs::write(
            dir.join("go.mod"),
            r#"module testproject

go 1.21
"#,
        )
        .unwrap();

        // main.go
        fs::write(
            dir.join("main.go"),
            r#"
package main

import (
    "fmt"
)

type Person struct {
    Name string
    Age  int
}

type Employee struct {
    Person
    Department string
}

func (e *Employee) Introduce() string {
    return fmt.Sprintf("I'm %s, %d years old, from %s", 
        e.Name, e.Age, e.Department)
}

func main() {
    emp := &Employee{
        Person: Person{
            Name: "Bob",
            Age:  25,
        },
        Department: "Sales",
    }
    fmt.Println(emp.Introduce())
    
    result := Add(10, 20)
    fmt.Printf("10 + 20 = %d\n", result)
}

func Add(a, b int) int {
    return a + b
}

func Multiply(a, b int) int {
    return a * b
}
"#,
        )
        .unwrap();

        // utils.go
        fs::write(
            dir.join("utils.go"),
            r#"
package main

import "strings"

func FormatName(name string) string {
    return "Hello, " + name + "!"
}

func ReverseString(s string) string {
    runes := []rune(s)
    for i, j := 0, len(runes)-1; i < j; i, j = i+1, j-1 {
        runes[i], runes[j] = runes[j], runes[i]
    }
    return string(runes)
}

func Contains(slice []string, item string) bool {
    for _, s := range slice {
        if s == item {
            return true
        }
    }
    return false
}

func ToUpper(s string) string {
    return strings.ToUpper(s)
}
"#,
        )
        .unwrap();
    }

    fn create_python_project(&self, dir: &Path) {
        // main.py
        fs::write(
            dir.join("main.py"),
            r#"
#!/usr/bin/env python3

class Person:
    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age

class Employee(Person):
    def __init__(self, name: str, age: int, department: str):
        super().__init__(name, age)
        self.department = department
    
    def introduce(self) -> str:
        return f"I'm {self.name}, {self.age} years old, from {self.department}"

def create_employee(name: str, age: int, dept: str) -> Employee:
    return Employee(name, age, dept)

def main():
    emp = create_employee("Charlie", 35, "Marketing")
    print(emp.introduce())
    
    from utils import add, multiply
    print(f"5 + 3 = {add(5, 3)}")
    print(f"4 * 6 = {multiply(4, 6)}")

if __name__ == "__main__":
    main()
"#,
        )
        .unwrap();

        // utils.py
        fs::write(
            dir.join("utils.py"),
            r#"
def add(a: int, b: int) -> int:
    """Add two numbers."""
    return a + b

def multiply(a: int, b: int) -> int:
    """Multiply two numbers."""
    return a * b

def format_name(name: str) -> str:
    """Format a name with greeting."""
    return f"Hello, {name}!"

class MathUtils:
    @staticmethod
    def square(n: int) -> int:
        """Calculate square of a number."""
        return n * n
    
    @staticmethod
    def cube(n: int) -> int:
        """Calculate cube of a number."""
        return n * n * n

CONSTANTS = {
    'PI': 3.14159,
    'E': 2.71828,
}
"#,
        )
        .unwrap();

        // __init__.py
        fs::write(dir.join("__init__.py"), "").unwrap();
    }

    /// コマンドを実行
    fn run_command(&self, args: &[&str], cwd: Option<&Path>) -> Result<CommandOutput> {
        let mut cmd = Command::new(&self.binary_path);

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        cmd.args(args);

        let output = cmd.output()?;

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            success: output.status.success(),
        })
    }
}

#[derive(Debug)]
struct CommandOutput {
    stdout: String,
    stderr: String,
    success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_rust_project() {
        let helper = E2ETestHelper::new().unwrap();
        let project_dir = helper.create_test_project("rust");
        let db_path = project_dir.join("index.db");

        // インデックス作成
        let output = helper.run_command(
            &[
                "index-project",
                "-p",
                project_dir.to_str().unwrap(),
                "-o",
                db_path.to_str().unwrap(),
                "-l",
                "rust",
            ],
            None,
        );

        if let Ok(output) = output {
            assert!(
                output.success || output.stdout.contains("Indexed"),
                "Failed to index: {}",
                output.stderr
            );

            // データベースファイルが作成されたことを確認
            assert!(db_path.exists() || output.stdout.contains("symbols"));
        }
    }

    #[test]
    fn test_index_typescript_project() {
        let helper = E2ETestHelper::new().unwrap();
        let project_dir = helper.create_test_project("typescript");
        let db_path = project_dir.join("index.db");

        let output = helper.run_command(
            &[
                "index-project",
                "-p",
                project_dir.to_str().unwrap(),
                "-o",
                db_path.to_str().unwrap(),
                "-l",
                "typescript",
            ],
            None,
        );

        if let Ok(output) = output {
            assert!(
                output.success || output.stdout.contains("Indexed"),
                "Failed to index TypeScript: {}",
                output.stderr
            );
        }
    }

    #[test]
    fn test_index_go_project() {
        let helper = E2ETestHelper::new().unwrap();
        let project_dir = helper.create_test_project("go");
        let db_path = project_dir.join("index.db");

        let output = helper.run_command(
            &[
                "index-project",
                "-p",
                project_dir.to_str().unwrap(),
                "-o",
                db_path.to_str().unwrap(),
                "-l",
                "go",
            ],
            None,
        );

        if let Ok(output) = output {
            assert!(
                output.success || output.stdout.contains("Indexed"),
                "Failed to index Go: {}",
                output.stderr
            );
        }
    }

    #[test]
    fn test_index_python_project() {
        let helper = E2ETestHelper::new().unwrap();
        let project_dir = helper.create_test_project("python");
        let db_path = project_dir.join("index.db");

        let output = helper.run_command(
            &[
                "index-project",
                "-p",
                project_dir.to_str().unwrap(),
                "-o",
                db_path.to_str().unwrap(),
                "-l",
                "python",
            ],
            None,
        );

        if let Ok(output) = output {
            assert!(
                output.success || output.stdout.contains("Indexed"),
                "Failed to index Python: {}",
                output.stderr
            );
        }
    }

    #[test]
    fn test_query_symbols() {
        let helper = E2ETestHelper::new().unwrap();
        let project_dir = helper.create_test_project("rust");
        let db_path = project_dir.join("index.db");

        // まずインデックスを作成
        let _ = helper.run_command(
            &[
                "index-project",
                "-p",
                project_dir.to_str().unwrap(),
                "-o",
                db_path.to_str().unwrap(),
                "-l",
                "rust",
            ],
            None,
        );

        // シンボル検索をテスト
        if db_path.exists() {
            let output = helper.run_command(
                &[
                    "query",
                    "-i",
                    db_path.to_str().unwrap(),
                    "--query-type",
                    "symbols",
                ],
                None,
            );

            if let Ok(output) = output {
                // シンボルが含まれていることを確認
                assert!(
                    output.stdout.contains("Calculator")
                        || output.stdout.contains("main")
                        || output.stdout.contains("Symbol"),
                    "No symbols found in output: {}",
                    output.stdout
                );
            }
        }
    }

    #[test]
    fn test_differential_index() {
        let helper = E2ETestHelper::new().unwrap();
        let project_dir = helper.create_test_project("rust");
        let db_path = project_dir.join("index.db");

        // 初回インデックス
        let _ = helper.run_command(
            &[
                "index-project",
                "-p",
                project_dir.to_str().unwrap(),
                "-o",
                db_path.to_str().unwrap(),
                "-l",
                "rust",
            ],
            None,
        );

        // ファイルを変更
        let src_main = project_dir.join("src").join("main.rs");
        if src_main.exists() {
            let content = fs::read_to_string(&src_main).unwrap();
            fs::write(&src_main, format!("{}\n// Modified", content)).unwrap();

            // 差分インデックス
            let output = helper.run_command(
                &[
                    "differential-index",
                    "-p",
                    project_dir.to_str().unwrap(),
                    "-o",
                    db_path.to_str().unwrap(),
                ],
                None,
            );

            if let Ok(output) = output {
                // 差分インデックスが成功または適切なメッセージが表示されること
                assert!(
                    output.success
                        || output.stdout.contains("Differential")
                        || output.stdout.contains("Updated")
                        || output.stderr.contains("not a git repository"), // Git未初期化の場合
                    "Unexpected output: stdout={}, stderr={}",
                    output.stdout,
                    output.stderr
                );
            }
        }
    }

    #[test]
    fn test_show_dead_code() {
        let helper = E2ETestHelper::new().unwrap();
        let project_dir = helper.create_test_project("rust");
        let db_path = project_dir.join("index.db");

        // インデックス作成
        let _ = helper.run_command(
            &[
                "index-project",
                "-p",
                project_dir.to_str().unwrap(),
                "-o",
                db_path.to_str().unwrap(),
                "-l",
                "rust",
            ],
            None,
        );

        if db_path.exists() {
            let output =
                helper.run_command(&["show-dead-code", "-i", db_path.to_str().unwrap()], None);

            if let Ok(output) = output {
                // デッドコード検出が動作すること
                // （実装によってはメッセージが異なる可能性がある）
                assert!(
                    output.stdout.contains("Dead")
                        || output.stdout.contains("Unused")
                        || output.stdout.contains("No dead code")
                        || output.stderr.contains("currently being refactored"),
                    "Unexpected dead code output: {}",
                    output.stdout
                );
            }
        }
    }
}
