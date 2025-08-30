/// LSP接続が失敗した場合のフォールバックインデクサー
///
/// 正規表現ベースのシンプルな解析を行い、基本的なシンボル情報を抽出する
use anyhow::Result;
use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};
use regex::Regex;
use std::path::Path;

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

        // 関数定義
        let fn_regex = Regex::new(r"^\s*(pub\s+)?(async\s+)?fn\s+(\w+)")?;
        // 構造体定義
        let struct_regex = Regex::new(r"^\s*(pub\s+)?struct\s+(\w+)")?;
        // enum定義
        let enum_regex = Regex::new(r"^\s*(pub\s+)?enum\s+(\w+)")?;
        // impl定義
        let impl_regex = Regex::new(r"^\s*impl(?:\s+<[^>]+>)?\s+(\w+)")?;
        // trait定義
        let trait_regex = Regex::new(r"^\s*(pub\s+)?trait\s+(\w+)")?;

        for (line_no, line) in lines.iter().enumerate() {
            if let Some(caps) = fn_regex.captures(line) {
                let name = caps.get(3).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::FUNCTION,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = struct_regex.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::STRUCT,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = enum_regex.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::ENUM,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = impl_regex.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    format!("impl {}", name),
                    SymbolKind::CLASS,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = trait_regex.captures(line) {
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

        // 関数定義
        let fn_regex = Regex::new(r"^\s*(export\s+)?(async\s+)?function\s+(\w+)")?;
        // クラス定義
        let class_regex = Regex::new(r"^\s*(export\s+)?class\s+(\w+)")?;
        // インターフェース定義
        let interface_regex = Regex::new(r"^\s*(export\s+)?interface\s+(\w+)")?;
        // const/let/var定義
        let var_regex = Regex::new(r"^\s*(export\s+)?(const|let|var)\s+(\w+)")?;
        // アロー関数
        let arrow_regex = Regex::new(r"^\s*(export\s+)?const\s+(\w+)\s*=\s*(?:async\s+)?[(\[]")?;

        for (line_no, line) in lines.iter().enumerate() {
            if let Some(caps) = fn_regex.captures(line) {
                let name = caps.get(3).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::FUNCTION,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = class_regex.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::CLASS,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = interface_regex.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::INTERFACE,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = arrow_regex.captures(line) {
                let name = caps.get(2).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::FUNCTION,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = var_regex.captures(line) {
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

        // クラス定義
        let class_regex = Regex::new(r"^class\s+(\w+)")?;
        // 関数定義
        let fn_regex = Regex::new(r"^(?:async\s+)?def\s+(\w+)")?;
        // メソッド定義（インデントあり）
        let method_regex = Regex::new(r"^\s+(?:async\s+)?def\s+(\w+)")?;

        for (line_no, line) in lines.iter().enumerate() {
            if let Some(caps) = class_regex.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::CLASS,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = fn_regex.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::FUNCTION,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = method_regex.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::METHOD,
                    line_no as u32,
                    4, // インデント考慮
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

        // 関数定義
        let fn_regex = Regex::new(r"^func\s+(?:\([^)]+\)\s+)?(\w+)")?;
        // 構造体定義
        let struct_regex = Regex::new(r"^type\s+(\w+)\s+struct")?;
        // インターフェース定義
        let interface_regex = Regex::new(r"^type\s+(\w+)\s+interface")?;
        // type定義
        let type_regex = Regex::new(r"^type\s+(\w+)\s+")?;
        // var/const定義
        let var_regex = Regex::new(r"^(?:var|const)\s+(\w+)")?;

        for (line_no, line) in lines.iter().enumerate() {
            if let Some(caps) = fn_regex.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::FUNCTION,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = struct_regex.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::STRUCT,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = interface_regex.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::INTERFACE,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = type_regex.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                symbols.push(self.create_symbol(
                    name,
                    SymbolKind::CLASS,
                    line_no as u32,
                    0,
                    line_no as u32,
                    line.len() as u32,
                ));
            } else if let Some(caps) = var_regex.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
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
        DocumentSymbol {
            name,
            detail: None,
            kind,
            tags: None,
            deprecated: None,
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
    use tempfile::TempDir;

    #[test]
    fn test_rust_fallback() -> Result<()> {
        let dir = TempDir::new()?;
        let file = dir.path().join("test.rs");
        std::fs::write(
            &file,
            r#"
pub struct User {
    name: String,
}

impl User {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

pub trait Greet {
    fn greet(&self);
}
"#,
        )?;

        let indexer = FallbackIndexer::new(FallbackLanguage::Rust);
        let symbols = indexer.extract_symbols(&file)?;

        assert!(symbols.len() >= 3);
        assert!(symbols.iter().any(|s| s.name == "User"));
        assert!(symbols.iter().any(|s| s.name.contains("impl")));
        assert!(symbols.iter().any(|s| s.name == "Greet"));

        Ok(())
    }

    #[test]
    fn test_python_fallback() -> Result<()> {
        let dir = TempDir::new()?;
        let file = dir.path().join("test.py");
        std::fs::write(
            &file,
            r#"
class User:
    def __init__(self, name):
        self.name = name
    
    def greet(self):
        print(f"Hello, {self.name}")

def main():
    user = User("Alice")
    user.greet()
"#,
        )?;

        let indexer = FallbackIndexer::new(FallbackLanguage::Python);
        let symbols = indexer.extract_symbols(&file)?;

        assert!(symbols.len() >= 4);
        assert!(symbols.iter().any(|s| s.name == "User"));
        assert!(symbols.iter().any(|s| s.name == "__init__"));
        assert!(symbols.iter().any(|s| s.name == "greet"));
        assert!(symbols.iter().any(|s| s.name == "main"));

        Ok(())
    }

    #[test]
    fn test_typescript_fallback() -> Result<()> {
        let dir = TempDir::new()?;
        let file = dir.path().join("test.ts");
        std::fs::write(
            &file,
            r#"
interface User {
    name: string;
    age: number;
}

class UserImpl implements User {
    constructor(public name: string, public age: number) {}
    
    greet(): void {
        console.log(`Hello, ${this.name}`);
    }
}

export function createUser(name: string, age: number): User {
    return new UserImpl(name, age);
}

const greetUser = (user: User) => {
    console.log(`Hi ${user.name}`);
};
"#,
        )?;

        let indexer = FallbackIndexer::new(FallbackLanguage::TypeScript);
        let symbols = indexer.extract_symbols(&file)?;

        assert!(symbols.len() >= 4);
        assert!(symbols.iter().any(|s| s.name == "User"));
        assert!(symbols.iter().any(|s| s.name == "UserImpl"));
        assert!(symbols.iter().any(|s| s.name == "createUser"));
        assert!(symbols.iter().any(|s| s.name == "greetUser"));

        Ok(())
    }

    #[test]
    fn test_go_fallback() -> Result<()> {
        let dir = TempDir::new()?;
        let file = dir.path().join("test.go");
        std::fs::write(
            &file,
            r#"
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
"#,
        )?;

        let indexer = FallbackIndexer::new(FallbackLanguage::Go);
        let symbols = indexer.extract_symbols(&file)?;

        assert!(symbols.len() >= 3);
        assert!(symbols.iter().any(|s| s.name == "User"));
        assert!(symbols.iter().any(|s| s.name == "Greet"));
        assert!(symbols.iter().any(|s| s.name == "CreateUser"));

        Ok(())
    }
}
