use lsif_core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
use petgraph::visit::EdgeRef;
use std::fs;
use tempfile::TempDir;

/// テスト用のコードサンプルを生成
struct TestCodeSamples;

impl TestCodeSamples {
    /// Rustのコードサンプルを生成
    fn rust_samples() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "main.rs",
                r#"
fn main() {
    println!("Hello, world!");
    let calc = Calculator::new();
    println!("2 + 3 = {}", calc.add(2, 3));
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
}
"#,
            ),
            (
                "lib.rs",
                r#"
pub mod utils {
    pub fn format_name(name: &str) -> String {
        format!("Hello, {}!", name)
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
            ),
        ]
    }

    /// TypeScriptのコードサンプルを生成
    fn typescript_samples() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "index.ts",
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
        return `I'm ${this.name}, ${this.age} years old`;
    }
}

function createEmployee(name: string, age: number): Employee {
    return new Employee(name, age, "Engineering");
}

export { Person, Employee, createEmployee };
"#,
            ),
            (
                "utils.ts",
                r#"
export function add(a: number, b: number): number {
    return a + b;
}

export function multiply(a: number, b: number): number {
    return a * b;
}

export class MathUtils {
    static square(n: number): number {
        return n * n;
    }
}

export const PI = 3.14159;
"#,
            ),
        ]
    }

    /// Goのコードサンプルを生成
    fn go_samples() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "main.go",
                r#"
package main

import "fmt"

type Person struct {
    Name string
    Age  int
}

type Employee struct {
    Person
    Department string
}

func (e *Employee) Introduce() string {
    return fmt.Sprintf("I'm %s, %d years old", e.Name, e.Age)
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
}

func Add(a, b int) int {
    return a + b
}
"#,
            ),
            (
                "utils.go",
                r#"
package main

import "strings"

func FormatName(name string) string {
    return "Hello, " + name + "!"
}

func ToUpper(s string) string {
    return strings.ToUpper(s)
}

type Calculator struct{}

func (c *Calculator) Add(a, b int) int {
    return a + b
}
"#,
            ),
        ]
    }

    /// Pythonのコードサンプルを生成
    fn python_samples() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "main.py",
                r#"
class Person:
    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age

class Employee(Person):
    def __init__(self, name: str, age: int, department: str):
        super().__init__(name, age)
        self.department = department
    
    def introduce(self) -> str:
        return f"I'm {self.name}, {self.age} years old"

def create_employee(name: str, age: int) -> Employee:
    return Employee(name, age, "Marketing")

def main():
    emp = create_employee("Charlie", 35)
    print(emp.introduce())

if __name__ == "__main__":
    main()
"#,
            ),
            (
                "utils.py",
                r#"
def add(a: int, b: int) -> int:
    return a + b

def multiply(a: int, b: int) -> int:
    return a * b

class MathUtils:
    @staticmethod
    def square(n: int) -> int:
        return n * n

PI = 3.14159
E = 2.71828
"#,
            ),
        ]
    }
}

/// テスト用のシンボルを作成
fn create_test_symbols() -> Vec<Symbol> {
    vec![
        Symbol {
            id: "main".to_string(),
            name: "main".to_string(),
            kind: SymbolKind::Function,
            file_path: "main.rs".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 4,
                },
            },
            documentation: None,
            detail: None,
        },
        Symbol {
            id: "Calculator".to_string(),
            name: "Calculator".to_string(),
            kind: SymbolKind::Struct,
            file_path: "main.rs".to_string(),
            range: Range {
                start: Position {
                    line: 7,
                    character: 0,
                },
                end: Position {
                    line: 7,
                    character: 10,
                },
            },
            documentation: None,
            detail: None,
        },
        Symbol {
            id: "Calculator::add".to_string(),
            name: "add".to_string(),
            kind: SymbolKind::Method,
            file_path: "main.rs".to_string(),
            range: Range {
                start: Position {
                    line: 13,
                    character: 4,
                },
                end: Position {
                    line: 13,
                    character: 7,
                },
            },
            documentation: None,
            detail: None,
        },
        Symbol {
            id: "Person".to_string(),
            name: "Person".to_string(),
            kind: SymbolKind::Struct,
            file_path: "lib.rs".to_string(),
            range: Range {
                start: Position {
                    line: 11,
                    character: 0,
                },
                end: Position {
                    line: 11,
                    character: 6,
                },
            },
            documentation: None,
            detail: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_building() {
        let mut graph = CodeGraph::new();
        let symbols = create_test_symbols();

        // シンボルを追加
        for symbol in &symbols {
            graph.add_symbol(symbol.clone());
        }

        // 参照を追加
        // 参照を追加（エッジとして）
        if let (Some(&from), Some(&to)) = (
            graph.symbol_index.get("main"),
            graph.symbol_index.get("Calculator"),
        ) {
            graph.graph.add_edge(from, to, EdgeKind::Reference);
        }
        if let (Some(&from), Some(&to)) = (
            graph.symbol_index.get("main"),
            graph.symbol_index.get("Calculator::add"),
        ) {
            graph.graph.add_edge(from, to, EdgeKind::Reference);
        }

        // グラフの検証
        assert_eq!(graph.symbol_index.len(), 4);

        // mainから出ていくエッジの数をカウント
        if let Some(&node) = graph.symbol_index.get("main") {
            let edge_count = graph.graph.edges(node).count();
            assert_eq!(edge_count, 2);
        }
    }

    #[test]
    fn test_graph_query() {
        let mut graph = CodeGraph::new();
        let symbols = create_test_symbols();

        for symbol in &symbols {
            graph.add_symbol(symbol.clone());
        }

        // 名前でシンボルを検索
        let calculator_count = graph
            .graph
            .node_indices()
            .filter(|&idx| graph.graph[idx].name == "Calculator")
            .count();
        assert_eq!(calculator_count, 1);

        // 種類でシンボルを検索
        let method_count = graph
            .graph
            .node_indices()
            .filter(|&idx| graph.graph[idx].kind == SymbolKind::Method)
            .count();
        assert_eq!(method_count, 1);

        // ファイルでシンボルを検索
        let main_symbols_count = graph
            .graph
            .node_indices()
            .filter(|&idx| graph.graph[idx].file_path == "main.rs")
            .count();
        assert_eq!(main_symbols_count, 3);
    }

    #[test]
    fn test_test_code_samples() {
        let temp_dir = TempDir::new().unwrap();

        // Rustサンプル
        let rust_samples = TestCodeSamples::rust_samples();
        for (filename, content) in rust_samples {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, content).unwrap();
            assert!(file_path.exists());

            let read_content = fs::read_to_string(&file_path).unwrap();
            // main.rsはCalculatorを含む、lib.rsはutilsを含む
            if filename == "main.rs" {
                assert!(read_content.contains("Calculator"));
            } else if filename == "lib.rs" {
                assert!(read_content.contains("utils"));
            }
        }

        // TypeScriptサンプル
        let ts_samples = TestCodeSamples::typescript_samples();
        for (filename, content) in ts_samples {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, content).unwrap();
            assert!(file_path.exists());

            let read_content = fs::read_to_string(&file_path).unwrap();
            assert!(read_content.contains("Employee") || read_content.contains("export"));
        }

        // Goサンプル
        let go_samples = TestCodeSamples::go_samples();
        for (filename, content) in go_samples {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, content).unwrap();
            assert!(file_path.exists());

            let read_content = fs::read_to_string(&file_path).unwrap();
            assert!(read_content.contains("package main"));
        }

        // Pythonサンプル
        let py_samples = TestCodeSamples::python_samples();
        for (filename, content) in py_samples {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, content).unwrap();
            assert!(file_path.exists());

            let read_content = fs::read_to_string(&file_path).unwrap();
            assert!(read_content.contains("def") || read_content.contains("class"));
        }
    }

    #[test]
    fn test_symbol_relationships() {
        let mut graph = CodeGraph::new();

        // 継承関係のテスト
        graph.add_symbol(Symbol {
            id: "Person".to_string(),
            name: "Person".to_string(),
            kind: SymbolKind::Class,
            file_path: "test.py".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 6,
                },
            },
            documentation: None,
            detail: None,
        });

        graph.add_symbol(Symbol {
            id: "Employee".to_string(),
            name: "Employee".to_string(),
            kind: SymbolKind::Class,
            file_path: "test.py".to_string(),
            range: Range {
                start: Position {
                    line: 5,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 8,
                },
            },
            documentation: None,
            detail: None,
        });

        // Employee extends Person - グラフに継承関係を追加
        if let (Some(&person_idx), Some(&employee_idx)) = (
            graph.symbol_index.get("Person"),
            graph.symbol_index.get("Employee"),
        ) {
            // 継承関係を参照で表現（EdgeKind::Inheritanceが存在しないため）
            graph
                .graph
                .add_edge(employee_idx, person_idx, EdgeKind::Reference);
        }

        // インターフェース実装のテスト
        graph.add_symbol(Symbol {
            id: "Greeter".to_string(),
            name: "Greeter".to_string(),
            kind: SymbolKind::Interface,
            file_path: "test.ts".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 7,
                },
            },
            documentation: None,
            detail: None,
        });

        // Person implements Greeter
        if let (Some(&person_idx), Some(&greeter_idx)) = (
            graph.symbol_index.get("Person"),
            graph.symbol_index.get("Greeter"),
        ) {
            graph
                .graph
                .add_edge(person_idx, greeter_idx, EdgeKind::Implementation);
        }

        // 関係の検証
        // Greeterの実装を探す
        let mut implementations = Vec::new();
        if let Some(&greeter_idx) = graph.symbol_index.get("Greeter") {
            for edge in graph
                .graph
                .edges_directed(greeter_idx, petgraph::Direction::Incoming)
            {
                if matches!(edge.weight(), EdgeKind::Implementation) {
                    let source = edge.source();
                    if let Some(symbol) = graph.graph.node_weight(source) {
                        implementations.push(symbol.id.clone());
                    }
                }
            }
        }
        assert!(implementations.contains(&"Person".to_string()));

        // Employeeの基底クラスを探す
        let mut base_classes = Vec::new();
        if let Some(&employee_idx) = graph.symbol_index.get("Employee") {
            for edge in graph.graph.edges(employee_idx) {
                // 継承関係を参照で判定（テスト用簡略化）
                if matches!(edge.weight(), EdgeKind::Reference) {
                    let target = edge.target();
                    if let Some(symbol) = graph.graph.node_weight(target) {
                        base_classes.push(symbol.id.clone());
                    }
                }
            }
        }
        assert!(base_classes.contains(&"Person".to_string()));
    }

    #[test]
    fn test_dead_code_detection() {
        let mut graph = CodeGraph::new();
        let symbols = create_test_symbols();

        for symbol in &symbols {
            graph.add_symbol(symbol.clone());
        }

        // mainから参照を追加
        if let (Some(&main_idx), Some(&calc_idx)) = (
            graph.symbol_index.get("main"),
            graph.symbol_index.get("Calculator"),
        ) {
            graph
                .graph
                .add_edge(main_idx, calc_idx, EdgeKind::Reference);
        }
        if let (Some(&main_idx), Some(&add_idx)) = (
            graph.symbol_index.get("main"),
            graph.symbol_index.get("Calculator::add"),
        ) {
            graph.graph.add_edge(main_idx, add_idx, EdgeKind::Reference);
        }
        // Personは参照されない（デッドコード）

        // 未参照シンボルを見つける
        let mut unreferenced = Vec::new();
        for &node_idx in graph.symbol_index.values() {
            let has_incoming = graph
                .graph
                .edges_directed(node_idx, petgraph::Direction::Incoming)
                .count()
                > 0;
            if !has_incoming {
                if let Some(symbol) = graph.graph.node_weight(node_idx) {
                    unreferenced.push(symbol.clone());
                }
            }
        }

        // Personが未参照として検出されるはず
        assert!(unreferenced.iter().any(|s| s.name == "Person"));
        // mainは参照されない（エントリーポイント）が未参照リストに含まれる
        assert!(unreferenced.iter().any(|s| s.name == "main"));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut graph = CodeGraph::new();

        // 循環参照を作成
        graph.add_symbol(Symbol {
            id: "A".to_string(),
            name: "A".to_string(),
            kind: SymbolKind::Module,
            file_path: "a.rs".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 1,
                },
            },
            documentation: None,
            detail: None,
        });

        graph.add_symbol(Symbol {
            id: "B".to_string(),
            name: "B".to_string(),
            kind: SymbolKind::Module,
            file_path: "b.rs".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 1,
                },
            },
            documentation: None,
            detail: None,
        });

        graph.add_symbol(Symbol {
            id: "C".to_string(),
            name: "C".to_string(),
            kind: SymbolKind::Module,
            file_path: "c.rs".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 1,
                },
            },
            documentation: None,
            detail: None,
        });

        // A -> B -> C -> A の循環参照
        if let (Some(&a_idx), Some(&b_idx)) =
            (graph.symbol_index.get("A"), graph.symbol_index.get("B"))
        {
            graph.graph.add_edge(a_idx, b_idx, EdgeKind::Reference);
        }
        if let (Some(&b_idx), Some(&c_idx)) =
            (graph.symbol_index.get("B"), graph.symbol_index.get("C"))
        {
            graph.graph.add_edge(b_idx, c_idx, EdgeKind::Reference);
        }
        if let (Some(&c_idx), Some(&a_idx)) =
            (graph.symbol_index.get("C"), graph.symbol_index.get("A"))
        {
            graph.graph.add_edge(c_idx, a_idx, EdgeKind::Reference);
        }

        // petgraphのscc（強連結成分）を使用して循環を検出
        use petgraph::algo::kosaraju_scc;
        let sccs = kosaraju_scc(&graph.graph);

        // 循環参照が検出されるはず（サイズ > 1のsccがある）
        let has_cycle = sccs.iter().any(|scc| scc.len() > 1);
        assert!(has_cycle);
    }
}
