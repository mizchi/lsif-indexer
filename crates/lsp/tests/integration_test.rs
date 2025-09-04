use lsp::{
    hierarchical_cache::{CacheConfig, HierarchicalCache},
    language_optimization::{LanguageOptimization, OptimizationStrategy},
    lsp_metrics::{CacheLevel, LspMetricsCollector},
    lsp_pool::{LspClientPool, PoolConfig},
    timeout_predictor::{LspOperation, TimeoutPredictor},
    Language,
};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

/// テスト用のサンプルコードを生成
struct TestProjects;

impl TestProjects {
    /// TypeScriptのテストプロジェクトを作成
    fn create_typescript_project(dir: &TempDir) -> PathBuf {
        let project_dir = dir.path().join("typescript");
        fs::create_dir_all(&project_dir).unwrap();

        // test.ts
        let test_content = r#"
interface Person {
    name: string;
    age: number;
}

class Employee implements Person {
    constructor(public name: string, public age: number, public role: string) {}
    
    greet(): string {
        return `Hello, I'm ${this.name}`;
    }
}

function main() {
    const emp = new Employee("Alice", 30, "Developer");
    console.log(emp.greet());
}

export { Employee, Person, main };
"#;
        fs::write(project_dir.join("test.ts"), test_content).unwrap();

        // lib.ts
        let lib_content = r#"
export function add(a: number, b: number): number {
    return a + b;
}

export function multiply(a: number, b: number): number {
    return a * b;
}

export class Calculator {
    add(a: number, b: number): number {
        return add(a, b);
    }
    
    multiply(a: number, b: number): number {
        return multiply(a, b);
    }
}
"#;
        fs::write(project_dir.join("lib.ts"), lib_content).unwrap();

        // tsconfig.json
        let tsconfig = r#"{
    "compilerOptions": {
        "target": "ES2020",
        "module": "commonjs",
        "strict": true,
        "esModuleInterop": true,
        "skipLibCheck": true,
        "forceConsistentCasingInFileNames": true
    }
}"#;
        fs::write(project_dir.join("tsconfig.json"), tsconfig).unwrap();

        project_dir
    }

    /// Goのテストプロジェクトを作成
    fn create_go_project(dir: &TempDir) -> PathBuf {
        let project_dir = dir.path().join("go");
        fs::create_dir_all(&project_dir).unwrap();

        // go.mod
        let go_mod = r#"module test

go 1.21
"#;
        fs::write(project_dir.join("go.mod"), go_mod).unwrap();

        // main.go
        let main_content = r#"
package main

import "fmt"

type Person struct {
    Name string
    Age  int
}

type Employee struct {
    Person
    Role string
}

func (e Employee) Greet() string {
    return fmt.Sprintf("Hello, I'm %s", e.Name)
}

func main() {
    emp := Employee{
        Person: Person{Name: "Bob", Age: 25},
        Role:   "Engineer",
    }
    fmt.Println(emp.Greet())
}
"#;
        fs::write(project_dir.join("main.go"), main_content).unwrap();

        // lib.go
        let lib_content = r#"
package main

func Add(a, b int) int {
    return a + b
}

func Multiply(a, b int) int {
    return a * b
}

type Calculator struct{}

func (c Calculator) Add(a, b int) int {
    return Add(a, b)
}

func (c Calculator) Multiply(a, b int) int {
    return Multiply(a, b)
}
"#;
        fs::write(project_dir.join("lib.go"), lib_content).unwrap();

        project_dir
    }

    /// Rustのテストプロジェクトを作成
    fn create_rust_project(dir: &TempDir) -> PathBuf {
        let project_dir = dir.path().join("rust");
        fs::create_dir_all(&project_dir).unwrap();
        let src_dir = project_dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();

        // Cargo.toml
        let cargo_toml = r#"[package]
name = "test_project"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
        fs::write(project_dir.join("Cargo.toml"), cargo_toml).unwrap();

        // src/main.rs
        let main_content = r#"
mod lib;

use lib::{Calculator, Person, Employee};

fn main() {
    let emp = Employee::new("Charlie", 28, "Designer");
    println!("{}", emp.greet());
    
    let calc = Calculator::new();
    println!("2 + 3 = {}", calc.add(2, 3));
}
"#;
        fs::write(src_dir.join("main.rs"), main_content).unwrap();

        // src/lib.rs
        let lib_content = r#"
pub trait Person {
    fn name(&self) -> &str;
    fn age(&self) -> u32;
}

pub struct Employee {
    name: String,
    age: u32,
    role: String,
}

impl Employee {
    pub fn new(name: &str, age: u32, role: &str) -> Self {
        Self {
            name: name.to_string(),
            age,
            role: role.to_string(),
        }
    }
    
    pub fn greet(&self) -> String {
        format!("Hello, I'm {}", self.name)
    }
}

impl Person for Employee {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn age(&self) -> u32 {
        self.age
    }
}

pub struct Calculator;

impl Calculator {
    pub fn new() -> Self {
        Self
    }
    
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
    
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
"#;
        fs::write(src_dir.join("lib.rs"), lib_content).unwrap();

        project_dir
    }

    /// Pythonのテストプロジェクトを作成
    fn create_python_project(dir: &TempDir) -> PathBuf {
        let project_dir = dir.path().join("python");
        fs::create_dir_all(&project_dir).unwrap();

        // main.py
        let main_content = r#"
from lib import Calculator, Employee

def main():
    emp = Employee("Diana", 35, "Manager")
    print(emp.greet())
    
    calc = Calculator()
    print(f"3 + 4 = {calc.add(3, 4)}")
    print(f"5 * 6 = {calc.multiply(5, 6)}")

if __name__ == "__main__":
    main()
"#;
        fs::write(project_dir.join("main.py"), main_content).unwrap();

        // lib.py
        let lib_content = r#"
class Person:
    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age

class Employee(Person):
    def __init__(self, name: str, age: int, role: str):
        super().__init__(name, age)
        self.role = role
    
    def greet(self) -> str:
        return f"Hello, I'm {self.name}"

class Calculator:
    def add(self, a: int, b: int) -> int:
        return a + b
    
    def multiply(self, a: int, b: int) -> int:
        return a * b
"#;
        fs::write(project_dir.join("lib.py"), lib_content).unwrap();

        // __init__.py
        fs::write(project_dir.join("__init__.py"), "").unwrap();

        project_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchical_cache() {
        let config = CacheConfig::default();
        let cache = HierarchicalCache::new(config).unwrap();

        // L1キャッシュテスト
        let file_path = PathBuf::from("test.rs");
        let symbols = lsp_types::DocumentSymbolResponse::Nested(vec![]);

        cache
            .cache_document_symbols(&file_path, symbols.clone())
            .unwrap();
        let cached = cache.get_document_symbols(&file_path);
        assert!(cached.is_some());

        // メトリクス確認
        let metrics = cache.get_metrics();
        assert_eq!(metrics.l1_hits, 1);
    }

    #[test]
    fn test_lsp_pool_management() {
        let config = PoolConfig {
            max_instances_per_language: 2,
            max_idle_time: Duration::from_secs(60),
            init_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(2),
            max_retries: 1,
        };

        let pool = LspClientPool::new(config);
        let stats = pool.get_stats();

        assert_eq!(stats.total_clients, 0);
        assert_eq!(stats.active_clients, 0);
    }

    #[test]
    fn test_timeout_predictor() {
        let mut predictor = TimeoutPredictor::new();

        // 初期タイムアウト取得
        let timeout = predictor.get_timeout(LspOperation::Initialize);
        assert_eq!(timeout, Duration::from_secs(5));

        // 成功を記録
        for _ in 0..15 {
            predictor.record_operation(
                LspOperation::Initialize,
                10_000,
                500,
                Duration::from_millis(100),
                true,
            );
        }

        // タイムアウトが短縮されることを確認
        let new_timeout = predictor.get_timeout(LspOperation::Initialize);
        assert!(new_timeout <= Duration::from_secs(2));
    }

    #[test]
    fn test_language_optimization() {
        let strategy = OptimizationStrategy::new();

        // TypeScript最適化戦略を確認
        let ts_opt = strategy.get_optimization(&Language::TypeScript);
        assert!(ts_opt.is_some());

        if let Some(opt) = ts_opt {
            assert!(opt.should_parallelize());
            assert_eq!(opt.optimal_chunk_size(), 15);
            assert!(opt.prefer_lsp());
            assert_eq!(opt.preferred_lsp_server(), Some("tsgo"));
        }

        // Rust最適化戦略を確認
        let rust_opt = strategy.get_optimization(&Language::Rust);
        assert!(rust_opt.is_some());

        if let Some(opt) = rust_opt {
            assert!(opt.should_parallelize());
            assert_eq!(opt.optimal_chunk_size(), 20);
            assert_eq!(opt.preferred_lsp_server(), Some("rust-analyzer"));
        }
    }

    #[test]
    fn test_metrics_collection() {
        let collector = LspMetricsCollector::new();

        // 操作を記録
        collector.record_operation_complete("initialize", Duration::from_millis(100), true);

        collector.record_operation_complete("workspace/symbol", Duration::from_millis(50), true);

        // キャッシュ統計を記録
        collector.record_cache_hit(CacheLevel::L1);
        collector.record_cache_miss(CacheLevel::L2);

        let summary = collector.get_summary();
        assert_eq!(summary.total_requests, 2);
        assert!(summary.cache_hit_rate > 0.0);
    }

    #[test]
    fn test_project_creation() {
        let temp_dir = TempDir::new().unwrap();

        // TypeScriptプロジェクト作成
        let ts_dir = TestProjects::create_typescript_project(&temp_dir);
        assert!(ts_dir.join("test.ts").exists());
        assert!(ts_dir.join("lib.ts").exists());
        assert!(ts_dir.join("tsconfig.json").exists());

        // Goプロジェクト作成
        let go_dir = TestProjects::create_go_project(&temp_dir);
        assert!(go_dir.join("main.go").exists());
        assert!(go_dir.join("lib.go").exists());
        assert!(go_dir.join("go.mod").exists());

        // Rustプロジェクト作成
        let rust_dir = TestProjects::create_rust_project(&temp_dir);
        assert!(rust_dir.join("Cargo.toml").exists());
        assert!(rust_dir.join("src/main.rs").exists());
        assert!(rust_dir.join("src/lib.rs").exists());

        // Pythonプロジェクト作成
        let py_dir = TestProjects::create_python_project(&temp_dir);
        assert!(py_dir.join("main.py").exists());
        assert!(py_dir.join("lib.py").exists());
    }

    #[test]
    fn test_adaptive_timeout() {
        let mut predictor = TimeoutPredictor::new();

        // 各操作のデフォルトタイムアウトを確認
        let operations = [
            (LspOperation::Initialize, Duration::from_secs(5)),
            (LspOperation::WorkspaceSymbol, Duration::from_secs(2)),
            (LspOperation::DocumentSymbol, Duration::from_secs(1)),
            (LspOperation::Definition, Duration::from_millis(1500)),
        ];

        for (op, expected_initial) in operations {
            let timeout = predictor.get_timeout(op);
            assert_eq!(timeout, expected_initial);
        }

        // ファイルサイズベースの予測
        let file_timeout = predictor.predict_timeout_for_operation(
            LspOperation::DocumentSymbol,
            50_000, // 50KB
            2_000,  // 2000行
        );
        assert!(file_timeout > Duration::from_secs(1));
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = HierarchicalCache::new(CacheConfig::default()).unwrap();
        let file_path = PathBuf::from("test.rs");

        // データをキャッシュ
        cache
            .cache_document_symbols(
                &file_path,
                lsp_types::DocumentSymbolResponse::Nested(vec![]),
            )
            .unwrap();

        cache.cache_workspace_symbols("test", vec![]).unwrap();
        cache.cache_definitions(&file_path, 10, 5, vec![]).unwrap();

        // ファイル無効化
        cache.invalidate_file(&file_path);

        // キャッシュがクリアされていることを確認
        assert!(cache.get_document_symbols(&file_path).is_none());
        assert!(cache.get_definitions(&file_path, 10, 5).is_none());
        assert!(cache.get_workspace_symbols("test").is_none());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    #[ignore] // LSPサーバーが必要なため、通常はスキップ
    fn test_full_lsp_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let ts_dir = TestProjects::create_typescript_project(&temp_dir);

        // LSPプールを作成
        let pool = LspClientPool::with_defaults();

        // TypeScriptファイルでクライアントを取得
        let test_file = ts_dir.join("test.ts");
        let result = pool.get_or_create_client(&test_file, &ts_dir);

        if result.is_ok() {
            let stats = pool.get_stats();
            assert_eq!(stats.total_clients, 1);

            // クリーンアップ
            pool.shutdown_all();
        }
    }

    #[test]
    fn test_performance_benchmarks() {
        let collector = LspMetricsCollector::new();
        let mut predictor = TimeoutPredictor::new();

        // 様々な操作をシミュレート
        let operations = vec![
            ("initialize", 100, true),
            ("workspace/symbol", 50, true),
            ("documentSymbol", 30, true),
            ("definition", 25, true),
            ("references", 35, false), // 失敗ケース
        ];

        for (op_name, duration_ms, success) in operations {
            let duration = Duration::from_millis(duration_ms);
            collector.record_operation_complete(op_name, duration, success);

            // 適応的タイムアウトの学習
            if let Some(op) = match op_name {
                "initialize" => Some(LspOperation::Initialize),
                "workspace/symbol" => Some(LspOperation::WorkspaceSymbol),
                "documentSymbol" => Some(LspOperation::DocumentSymbol),
                _ => None,
            } {
                predictor.record_operation(op, 10_000, 500, duration, success);
            }
        }

        let summary = collector.get_summary();
        assert_eq!(summary.total_requests, 5);
        assert!(summary.error_rate > 0.0); // 1つ失敗があるため
    }
}
