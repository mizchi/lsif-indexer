use anyhow::Result;
use lsif_core::{Position, Range, Symbol, SymbolKind};
use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Query, QueryCapture, QueryCursor};

/// Tree-sitterベースのパーサー
pub struct TreeSitterParser {
    parser: Parser,
    language: Language,
    queries: LanguageQueries,
}

/// 言語固有のクエリ
struct LanguageQueries {
    symbols: Query,
    definitions: Query,
    references: Query,
    visibility: Option<Query>,
}

impl TreeSitterParser {
    /// Rust用パーサーを作成
    pub fn rust() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::language();
        parser.set_language(language)?;

        let queries = LanguageQueries {
            symbols: Query::new(
                language,
                r#"
                (function_item name: (identifier) @function)
                (struct_item name: (type_identifier) @struct)
                (enum_item name: (type_identifier) @enum)
                (trait_item name: (type_identifier) @trait)
                (impl_item type: (type_identifier) @impl)
                (mod_item name: (identifier) @module)
                (const_item name: (identifier) @constant)
                (static_item name: (identifier) @static)
                (type_alias name: (type_identifier) @type_alias)
                "#,
            )?,
            definitions: Query::new(
                language,
                r#"
                (let_declaration pattern: (identifier) @definition)
                (parameter pattern: (identifier) @definition)
                "#,
            )?,
            references: Query::new(
                language,
                r#"
                (identifier) @reference
                (type_identifier) @type_reference
                "#,
            )?,
            visibility: Some(Query::new(
                language,
                r#"
                (visibility_modifier) @visibility
                "#,
            )?),
        };

        Ok(Self {
            parser,
            language,
            queries,
        })
    }

    /// TypeScript/JavaScript用パーサーを作成
    pub fn typescript() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_typescript::language_typescript();
        parser.set_language(language)?;

        let queries = LanguageQueries {
            symbols: Query::new(
                language,
                r#"
                (function_declaration name: (identifier) @function)
                (class_declaration name: (type_identifier) @class)
                (interface_declaration name: (type_identifier) @interface)
                (enum_declaration name: (identifier) @enum)
                (type_alias_declaration name: (type_identifier) @type_alias)
                (variable_declarator name: (identifier) @variable)
                (method_definition name: (property_identifier) @method)
                "#,
            )?,
            definitions: Query::new(
                language,
                r#"
                (variable_declarator name: (identifier) @definition)
                (formal_parameters (required_parameter pattern: (identifier) @parameter))
                "#,
            )?,
            references: Query::new(
                language,
                r#"
                (identifier) @reference
                (type_identifier) @type_reference
                "#,
            )?,
            visibility: Some(Query::new(
                language,
                r#"
                (export_statement) @export
                (accessibility_modifier) @access
                "#,
            )?),
        };

        Ok(Self {
            parser,
            language,
            queries,
        })
    }

    /// Python用パーサーを作成
    pub fn python() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_python::language();
        parser.set_language(language)?;

        let queries = LanguageQueries {
            symbols: Query::new(
                language,
                r#"
                (function_definition name: (identifier) @function)
                (class_definition name: (identifier) @class)
                (assignment left: (identifier) @variable)
                (decorated_definition) @decorated
                "#,
            )?,
            definitions: Query::new(
                language,
                r#"
                (assignment left: (identifier) @definition)
                (parameter (identifier) @parameter)
                "#,
            )?,
            references: Query::new(
                language,
                r#"
                (identifier) @reference
                (attribute) @attribute_access
                "#,
            )?,
            visibility: None, // Pythonは命名規則で判断
        };

        Ok(Self {
            parser,
            language,
            queries,
        })
    }

    /// Go用パーサーを作成
    pub fn go() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_go::language();
        parser.set_language(language)?;

        let queries = LanguageQueries {
            symbols: Query::new(
                language,
                r#"
                (function_declaration name: (identifier) @function)
                (method_declaration name: (field_identifier) @method)
                (type_declaration (type_spec name: (type_identifier) @type))
                (const_declaration (const_spec name: (identifier) @constant))
                (var_declaration (var_spec name: (identifier) @variable))
                "#,
            )?,
            definitions: Query::new(
                language,
                r#"
                (short_var_declaration left: (identifier_list (identifier) @definition))
                (var_spec name: (identifier) @definition)
                "#,
            )?,
            references: Query::new(
                language,
                r#"
                (identifier) @reference
                (type_identifier) @type_reference
                "#,
            )?,
            visibility: None, // Goは大文字小文字で判断
        };

        Ok(Self {
            parser,
            language,
            queries,
        })
    }

    /// ソースコードをパース
    pub fn parse(&mut self, source: &str) -> Result<tree_sitter::Tree> {
        self.parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse source"))
    }

    /// シンボルを抽出
    pub fn extract_symbols(&mut self, source: &str, file_path: &str) -> Result<Vec<Symbol>> {
        let tree = self.parse(source)?;
        let mut symbols = Vec::new();
        let mut cursor = QueryCursor::new();

        let matches = cursor.matches(&self.queries.symbols, tree.root_node(), source.as_bytes());

        for m in matches {
            for capture in m.captures {
                let node = capture.node;
                let kind = self.determine_symbol_kind(&capture);
                let name = self.get_node_text(&node, source);
                
                symbols.push(Symbol {
                    id: format!("{}:{}:{}", file_path, node.start_position().row, node.start_position().column),
                    kind,
                    name,
                    file_path: file_path.to_string(),
                    range: self.node_to_range(&node),
                    documentation: None,
                    detail: self.extract_detail(&node, source),
                });
            }
        }

        Ok(symbols)
    }

    /// 公開APIを抽出
    pub fn extract_public_apis(&mut self, source: &str, file_path: &str) -> Result<Vec<Symbol>> {
        let symbols = self.extract_symbols(source, file_path)?;
        
        // 可視性クエリがある場合は使用
        if self.queries.visibility.is_some() {
            let tree = self.parse(source)?;
            let mut cursor = QueryCursor::new();
            let mut public_positions = Vec::new();

            // visibility_queryを取得してマッチ
            let visibility_query = self.queries.visibility.as_ref().unwrap();
            let matches = cursor.matches(visibility_query, tree.root_node(), source.as_bytes());
            for m in matches {
                for capture in m.captures {
                    public_positions.push(capture.node.start_position());
                }
            }

            // 公開位置にあるシンボルをフィルタ
            Ok(symbols
                .into_iter()
                .filter(|s| {
                    public_positions.iter().any(|&pos| {
                        s.range.start.line == pos.row as u32
                    })
                })
                .collect())
        } else {
            // 言語固有のルールで判定
            Ok(symbols
                .into_iter()
                .filter(|s| self.is_public_by_naming(s))
                .collect())
        }
    }

    /// 定義と参照の関係を抽出
    pub fn extract_relationships(
        &mut self,
        source: &str,
    ) -> Result<Vec<(String, String, String)>> {
        let tree = self.parse(source)?;
        let mut relationships = Vec::new();
        let mut cursor = QueryCursor::new();

        // 定義を収集
        let mut definitions: HashMap<String, Node> = HashMap::new();
        let def_matches = cursor.matches(&self.queries.definitions, tree.root_node(), source.as_bytes());
        for m in def_matches {
            for capture in m.captures {
                let name = self.get_node_text(&capture.node, source);
                definitions.insert(name.clone(), capture.node);
            }
        }

        // 参照を収集して関係を構築
        cursor = QueryCursor::new();
        let ref_matches = cursor.matches(&self.queries.references, tree.root_node(), source.as_bytes());
        for m in ref_matches {
            for capture in m.captures {
                let name = self.get_node_text(&capture.node, source);
                if let Some(def_node) = definitions.get(&name) {
                    relationships.push((
                        format!("ref_{}", capture.node.id()),
                        "references".to_string(),
                        format!("def_{}", def_node.id()),
                    ));
                }
            }
        }

        Ok(relationships)
    }

    /// ノードからシンボル種別を判定
    fn determine_symbol_kind(&self, capture: &QueryCapture) -> SymbolKind {
        let capture_name = &self.queries.symbols.capture_names()[capture.index as usize];
        
        match capture_name.as_str() {
            "function" => SymbolKind::Function,
            "struct" => SymbolKind::Struct,
            "enum" => SymbolKind::Enum,
            "trait" => SymbolKind::Trait,
            "class" => SymbolKind::Class,
            "interface" => SymbolKind::Interface,
            "module" => SymbolKind::Module,
            "constant" => SymbolKind::Constant,
            "variable" => SymbolKind::Variable,
            "method" => SymbolKind::Method,
            "type_alias" => SymbolKind::TypeAlias,
            "parameter" => SymbolKind::Parameter,
            _ => SymbolKind::Unknown,
        }
    }

    /// ノードのテキストを取得
    fn get_node_text(&self, node: &Node, source: &str) -> String {
        source[node.byte_range()].to_string()
    }

    /// ノードから範囲を作成
    fn node_to_range(&self, node: &Node) -> Range {
        Range {
            start: Position {
                line: node.start_position().row as u32,
                character: node.start_position().column as u32,
            },
            end: Position {
                line: node.end_position().row as u32,
                character: node.end_position().column as u32,
            },
        }
    }

    /// 詳細情報を抽出
    fn extract_detail(&self, node: &Node, source: &str) -> Option<String> {
        // 親ノードから署名などを取得
        if let Some(parent) = node.parent() {
            let parent_text = self.get_node_text(&parent, source);
            if parent_text.len() < 200 {
                return Some(parent_text);
            }
        }
        None
    }

    /// 命名規則で公開APIか判定
    fn is_public_by_naming(&self, symbol: &Symbol) -> bool {
        // Go: 大文字始まり
        // Python: アンダースコアなし
        // その他: デフォルトで公開
        !symbol.name.starts_with('_')
    }

    /// 複雑度を計算するためのASTノード数をカウント
    pub fn calculate_complexity(&mut self, source: &str) -> Result<usize> {
        let tree = self.parse(source)?;
        let complexity_query = Query::new(
            self.language,
            r#"
            (if_statement) @branch
            (while_statement) @loop
            (for_statement) @loop
            (match_expression) @branch
            (case_statement) @branch
            "#,
        )?;

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&complexity_query, tree.root_node(), source.as_bytes());
        
        Ok(matches.count() + 1)  // 基本複雑度1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_parser() {
        let mut parser = TreeSitterParser::rust().unwrap();
        let source = r#"
pub fn hello_world() {
    println!("Hello, world!");
}

struct MyStruct {
    field: String,
}
        "#;

        let symbols = parser.extract_symbols(source, "test.rs").unwrap();
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.name == "hello_world"));
        assert!(symbols.iter().any(|s| s.name == "MyStruct"));
    }

    #[test]
    fn test_typescript_parser() {
        let mut parser = TreeSitterParser::typescript().unwrap();
        let source = r#"
export function greet(name: string): void {
    console.log(`Hello, ${name}!`);
}

interface Person {
    name: string;
    age: number;
}
        "#;

        let symbols = parser.extract_symbols(source, "test.ts").unwrap();
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.name == "greet"));
        assert!(symbols.iter().any(|s| s.name == "Person"));
    }

    #[test]
    fn test_python_parser() {
        let mut parser = TreeSitterParser::python().unwrap();
        let source = r#"
def main():
    print("Hello, world!")

class MyClass:
    def __init__(self):
        self.value = 42
        "#;

        let symbols = parser.extract_symbols(source, "test.py").unwrap();
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.name == "main"));
        assert!(symbols.iter().any(|s| s.name == "MyClass"));
    }

    #[test]
    fn test_go_parser() {
        let mut parser = TreeSitterParser::go().unwrap();
        let source = r#"
package main

func main() {
    fmt.Println("Hello, world!")
}

type Person struct {
    Name string
    Age  int
}
        "#;

        let symbols = parser.extract_symbols(source, "test.go").unwrap();
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.name == "main"));
        assert!(symbols.iter().any(|s| s.name == "Person"));
    }

    #[test]
    fn test_complexity_calculation() {
        let mut parser = TreeSitterParser::rust().unwrap();
        let source = r#"
fn complex_function(x: i32) {
    if x > 0 {
        for i in 0..x {
            if i % 2 == 0 {
                println!("even");
            } else {
                println!("odd");
            }
        }
    }
}
        "#;

        let complexity = parser.calculate_complexity(source).unwrap();
        assert!(complexity > 1);  // 複数の分岐があるため
    }

    #[test]
    fn test_public_api_extraction() {
        let mut parser = TreeSitterParser::rust().unwrap();
        let source = r#"
pub fn public_function() {}
fn private_function() {}
pub struct PublicStruct;
struct PrivateStruct;
        "#;

        let public_apis = parser.extract_public_apis(source, "test.rs").unwrap();
        // Tree-sitterのクエリでpub修飾子を検出
        assert!(public_apis.iter().any(|s| s.name == "public_function"));
        assert!(!public_apis.iter().any(|s| s.name == "private_function"));
    }
}