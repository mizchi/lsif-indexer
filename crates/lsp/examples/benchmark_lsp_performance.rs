use lsp::lsp_performance_benchmark::{analyze_results, run_benchmark_suite};
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    println!("LSP Performance Benchmark\n");
    println!("========================\n");

    let mut all_results = HashMap::new();

    let rust_workspace = std::env::current_dir().unwrap();
    let rust_files = vec!["src/lib.rs", "src/adapter/mod.rs"];
    let rust_results = run_benchmark_suite(
        "rust-analyzer",
        vec!["rust-analyzer".to_string()],
        rust_workspace,
        rust_files,
    );
    all_results.insert("rust-analyzer".to_string(), rust_results);

    let ts_workspace = std::env::current_dir()
        .unwrap()
        .join("test_projects/typescript");
    std::fs::create_dir_all(&ts_workspace).ok();

    let ts_test_file = ts_workspace.join("test.ts");
    std::fs::write(
        &ts_test_file,
        r#"
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
"#,
    )
    .ok();

    let ts_lib_file = ts_workspace.join("lib.ts");
    std::fs::write(
        &ts_lib_file,
        r#"
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
"#,
    )
    .ok();

    let ts_files = vec!["test.ts", "lib.ts"];

    let tsgo_results = run_benchmark_suite(
        "tsgo",
        vec![
            "tsgo".to_string(),
            "--lsp".to_string(),
            "--stdio".to_string(),
        ],
        ts_workspace.clone(),
        ts_files.clone(),
    );
    all_results.insert("tsgo".to_string(), tsgo_results);

    let go_workspace = std::env::current_dir().unwrap().join("test_projects/go");
    std::fs::create_dir_all(&go_workspace).ok();

    let go_mod = go_workspace.join("go.mod");
    std::fs::write(
        &go_mod,
        r#"module test

go 1.21
"#,
    )
    .ok();

    let go_test_file = go_workspace.join("main.go");
    std::fs::write(
        &go_test_file,
        r#"
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
"#,
    )
    .ok();

    let go_lib_file = go_workspace.join("lib.go");
    std::fs::write(
        &go_lib_file,
        r#"
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
"#,
    )
    .ok();

    let go_files = vec!["main.go", "lib.go"];

    let gopls_results =
        run_benchmark_suite("gopls", vec!["gopls".to_string()], go_workspace, go_files);
    all_results.insert("gopls".to_string(), gopls_results);

    analyze_results(all_results);

    std::fs::remove_dir_all("test_projects").ok();
}
