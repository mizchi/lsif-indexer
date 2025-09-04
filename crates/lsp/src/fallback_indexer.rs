/// LSP接続が失敗した場合のフォールバックインデクサー
///
/// 正規表現ベースのシンプルな解析を行い、基本的なシンボル情報を抽出する
use anyhow::Result;
use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

// Rust用の正規表現を静的に初期化
static RUST_FN_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(pub\s+)?(async\s+)?fn\s+(\w+)").unwrap());
static RUST_STRUCT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(pub\s+)?struct\s+(\w+)").unwrap());
static RUST_ENUM_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(pub\s+)?enum\s+(\w+)").unwrap());
static RUST_IMPL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*impl(?:\s+<[^>]+>)?\s+(\w+)").unwrap());
static RUST_TRAIT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(pub\s+)?trait\s+(\w+)").unwrap());

// TypeScript/JavaScript用の正規表現を静的に初期化
static TS_FN_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(export\s+)?(async\s+)?function\s+(\w+)").unwrap());
static TS_CLASS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(export\s+)?class\s+(\w+)").unwrap());
static TS_INTERFACE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(export\s+)?interface\s+(\w+)").unwrap());
static TS_VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(export\s+)?(const|let|var)\s+(\w+)").unwrap());
static TS_ARROW_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(export\s+)?const\s+(\w+)\s*=\s*(?:async\s+)?[(\[]").unwrap());

// Python用の正規表現を静的に初期化
static PY_FN_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?:async\s+)?def\s+(\w+)").unwrap());
static PY_CLASS_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*class\s+(\w+)").unwrap());

// Go用の正規表現を静的に初期化
static GO_FN_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*func\s+(?:\(\s*\w+\s+[^)]+\)\s+)?(\w+)").unwrap());
static GO_TYPE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*type\s+(\w+)\s+(struct|interface)").unwrap());

/// サポートされる言語
pub enum FallbackLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
}

pub struct FallbackIndexer {
    language: FallbackLanguage,
}

impl FallbackIndexer {
    pub fn new(language: FallbackLanguage) -> Self {
        Self { language }
    }

    /// Python用の最適化されたインデクサーを作成
    pub fn for_python() -> Result<Self> {
        Ok(Self {
            language: FallbackLanguage::Python,
        })
    }

    /// JavaScript用の最適化されたインデクサーを作成
    pub fn for_javascript() -> Result<Self> {
        Ok(Self {
            language: FallbackLanguage::JavaScript,
        })
    }

    /// 拡張子から言語を推測
    pub fn from_extension(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        let language = match ext {
            "rs" => FallbackLanguage::Rust,
            "ts" | "tsx" => FallbackLanguage::TypeScript,
            "js" | "jsx" => FallbackLanguage::JavaScript,
            "py" | "pyi" => FallbackLanguage::Python,
            "go" => FallbackLanguage::Go,
            _ => return None,
        };
        Some(Self { language })
    }

    /// ファイルから基本的なシンボル情報を抽出
    pub fn extract_symbols(&self, file_path: &Path) -> Result<Vec<DocumentSymbol>> {
        let content = std::fs::read_to_string(file_path)?;
        let lines: Vec<&str> = content.lines().collect();

        match self.language {
            FallbackLanguage::Rust => self.extract_rust_symbols(&lines),
            FallbackLanguage::TypeScript | FallbackLanguage::JavaScript => {
                self.extract_typescript_symbols(&lines)
            }
            FallbackLanguage::Python => self.extract_python_symbols(&lines),
            FallbackLanguage::Go => self.extract_go_symbols(&lines),
        }
    }

    /// Rustのシンボルを抽出
    fn extract_rust_symbols(&self, lines: &[&str]) -> Result<Vec<DocumentSymbol>> {
        let mut symbols = Vec::new();

        for (line_no, line) in lines.iter().enumerate() {
            if let Some(caps) = RUST_FN_REGEX.captures(line) {
                let name = caps.get(3).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::FUNCTION,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = RUST_STRUCT_REGEX.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::STRUCT,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = RUST_ENUM_REGEX.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::ENUM,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = RUST_IMPL_REGEX.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    format!("impl {}", name),
                    SymbolKind::CLASS,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = RUST_TRAIT_REGEX.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::INTERFACE,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            }
        }

        Ok(symbols)
    }

    /// TypeScript/JavaScriptのシンボルを抽出
    fn extract_typescript_symbols(&self, lines: &[&str]) -> Result<Vec<DocumentSymbol>> {
        let mut symbols = Vec::new();

        for (line_no, line) in lines.iter().enumerate() {
            if let Some(caps) = TS_FN_REGEX.captures(line) {
                let name = caps.get(3).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::FUNCTION,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = TS_CLASS_REGEX.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::CLASS,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = TS_INTERFACE_REGEX.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::INTERFACE,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = TS_ARROW_REGEX.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::FUNCTION,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = TS_VAR_REGEX.captures(line) {
                let name = caps.get(3).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::VARIABLE,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            }
        }

        Ok(symbols)
    }

    /// Pythonのシンボルを抽出
    fn extract_python_symbols(&self, lines: &[&str]) -> Result<Vec<DocumentSymbol>> {
        let mut symbols = Vec::new();

        for (line_no, line) in lines.iter().enumerate() {
            if let Some(caps) = PY_CLASS_REGEX.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::CLASS,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = PY_FN_REGEX.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                // インデントされている場合はメソッド、そうでなければ関数
                let kind = if line.trim_start() != *line {
                    SymbolKind::METHOD
                } else {
                    SymbolKind::FUNCTION
                };
                let column = line.len() - line.trim_start().len();
                symbols.push(self.create_symbol(
                    name,
                    kind,
                    line_no as u32,
                    column as u32,
                    line_no as u32,
                    line.len() as u32,
                ));
            }
        }

        Ok(symbols)
    }

    /// Goのシンボルを抽出
    fn extract_go_symbols(&self, lines: &[&str]) -> Result<Vec<DocumentSymbol>> {
        let mut symbols = Vec::new();

        for (line_no, line) in lines.iter().enumerate() {
            if let Some(caps) = GO_FN_REGEX.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::FUNCTION,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = GO_TYPE_REGEX.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                let type_kind = caps.get(2).unwrap().as_str();
                let kind = match type_kind {
                    "struct" => SymbolKind::STRUCT,
                    "interface" => SymbolKind::INTERFACE,
                    _ => SymbolKind::CLASS,
                };
                symbols.push(self.create_symbol(
                    name,
                    kind,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            }
        }

        Ok(symbols)
    }

    /// DocumentSymbolを作成するヘルパー関数
    #[allow(deprecated)]
    fn create_symbol(
        &self,
        name: String,
        kind: SymbolKind,
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
    ) -> DocumentSymbol {
        #[allow(deprecated)]
        DocumentSymbol {
            name,
            kind,
            tags: None,
            deprecated: None,
            detail: None,
            range: Range {
                start: Position {
                    line: start_line,
                    character: start_char,
                },
                end: Position {
                    line: end_line,
                    character: end_char,
                },
            },
            selection_range: Range {
                start: Position {
                    line: start_line,
                    character: start_char,
                },
                end: Position {
                    line: end_line,
                    character: end_char,
                },
            },
            children: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_fallback_language_from_extension() {
        // Rust
        assert!(FallbackIndexer::from_extension(Path::new("test.rs")).is_some());

        // TypeScript/JavaScript
        assert!(FallbackIndexer::from_extension(Path::new("test.ts")).is_some());
        assert!(FallbackIndexer::from_extension(Path::new("test.tsx")).is_some());
        assert!(FallbackIndexer::from_extension(Path::new("test.js")).is_some());
        assert!(FallbackIndexer::from_extension(Path::new("test.jsx")).is_some());

        // Python
        assert!(FallbackIndexer::from_extension(Path::new("test.py")).is_some());
        assert!(FallbackIndexer::from_extension(Path::new("test.pyi")).is_some());

        // Go
        assert!(FallbackIndexer::from_extension(Path::new("test.go")).is_some());

        // 未対応の拡張子
        assert!(FallbackIndexer::from_extension(Path::new("test.xyz")).is_none());
        assert!(FallbackIndexer::from_extension(Path::new("test")).is_none());
    }

    #[test]
    fn test_extract_rust_symbols() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        let rust_code = r#"
fn main() {
    println!("Hello");
}

pub struct MyStruct {
    field: String,
}

impl MyStruct {
    pub fn new() -> Self {
        Self { field: String::new() }
    }
}

trait MyTrait {
    fn method(&self);
}
"#;

        fs::write(&file_path, rust_code).unwrap();

        let indexer = FallbackIndexer::from_extension(&file_path).unwrap();
        let symbols = indexer.extract_symbols(&file_path).unwrap();

        // 関数、構造体、トレイトが検出されることを確認
        assert!(symbols
            .iter()
            .any(|s| s.name == "main" && s.kind == SymbolKind::FUNCTION));
        assert!(symbols
            .iter()
            .any(|s| s.name == "MyStruct" && s.kind == SymbolKind::STRUCT));
        assert!(symbols
            .iter()
            .any(|s| s.name == "new" && s.kind == SymbolKind::FUNCTION));
        assert!(symbols
            .iter()
            .any(|s| s.name == "MyTrait" && s.kind == SymbolKind::INTERFACE));
    }

    #[test]
    fn test_extract_typescript_symbols() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.ts");

        let ts_code = r#"
function hello() {
    console.log("Hello");
}

class MyClass {
    constructor() {}
    
    method() {
        return 42;
    }
}

interface MyInterface {
    prop: string;
}

const myVar = 123;
export const exportedVar = "test";
"#;

        fs::write(&file_path, ts_code).unwrap();

        let indexer = FallbackIndexer::from_extension(&file_path).unwrap();
        let symbols = indexer.extract_symbols(&file_path).unwrap();

        // 関数、クラス、インターフェース、変数が検出されることを確認
        assert!(symbols
            .iter()
            .any(|s| s.name == "hello" && s.kind == SymbolKind::FUNCTION));
        assert!(symbols
            .iter()
            .any(|s| s.name == "MyClass" && s.kind == SymbolKind::CLASS));
        assert!(symbols
            .iter()
            .any(|s| s.name == "MyInterface" && s.kind == SymbolKind::INTERFACE));
        assert!(symbols
            .iter()
            .any(|s| s.name == "myVar" && s.kind == SymbolKind::VARIABLE));
        assert!(symbols
            .iter()
            .any(|s| s.name == "exportedVar" && s.kind == SymbolKind::VARIABLE));
    }

    #[test]
    fn test_extract_python_symbols() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");

        let py_code = r#"
def hello():
    print("Hello")

class MyClass:
    def __init__(self):
        self.value = 0
    
    def method(self):
        return self.value

async def async_function():
    await some_task()
"#;

        fs::write(&file_path, py_code).unwrap();

        let indexer = FallbackIndexer::from_extension(&file_path).unwrap();
        let symbols = indexer.extract_symbols(&file_path).unwrap();

        // 関数、クラス、メソッドが検出されることを確認
        assert!(symbols
            .iter()
            .any(|s| s.name == "hello" && s.kind == SymbolKind::FUNCTION));
        assert!(symbols
            .iter()
            .any(|s| s.name == "MyClass" && s.kind == SymbolKind::CLASS));
        assert!(symbols
            .iter()
            .any(|s| s.name == "__init__" && s.kind == SymbolKind::METHOD));
        assert!(symbols
            .iter()
            .any(|s| s.name == "method" && s.kind == SymbolKind::METHOD));
        assert!(symbols
            .iter()
            .any(|s| s.name == "async_function" && s.kind == SymbolKind::FUNCTION));
    }

    #[test]
    fn test_extract_go_symbols() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.go");

        let go_code = r#"
package main

type User struct {
    Name string
    Age  int
}

func (u *User) Greet() {
    fmt.Printf("Hello, I'm %s\n", u.Name)
}

func CreateUser(name string, age int) *User {
    return &User{Name: name, Age: age}
}
"#;

        fs::write(&file_path, go_code).unwrap();

        let indexer = FallbackIndexer::from_extension(&file_path).unwrap();
        let symbols = indexer.extract_symbols(&file_path).unwrap();

        // 構造体、メソッド、関数が検出されることを確認
        assert!(symbols
            .iter()
            .any(|s| s.name == "User" && s.kind == SymbolKind::STRUCT));
        assert!(symbols
            .iter()
            .any(|s| s.name == "Greet" && s.kind == SymbolKind::FUNCTION));
        assert!(symbols
            .iter()
            .any(|s| s.name == "CreateUser" && s.kind == SymbolKind::FUNCTION));
    }
}
