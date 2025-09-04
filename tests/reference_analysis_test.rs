use anyhow::Result;
use lsif_core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
use tempfile::TempDir;

#[cfg(test)]
mod rust_reference_tests {
    use super::*;
    use std::fs;

    fn setup_test_rust_project() -> Result<(TempDir, String)> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("lib.rs");

        let code = r#"
// テスト用のRustコード
pub struct Calculator {
    value: i32,
}

impl Calculator {
    pub fn new() -> Self {
        Calculator { value: 0 }
    }
    
    pub fn add(&mut self, x: i32) -> i32 {
        self.value += x;
        self.value
    }
    
    pub fn get_value(&self) -> i32 {
        self.value
    }
}

pub fn create_calculator() -> Calculator {
    Calculator::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculator() {
        let mut calc = Calculator::new();
        assert_eq!(calc.add(5), 5);
        assert_eq!(calc.get_value(), 5);
    }
    
    #[test]
    fn test_create() {
        let calc = create_calculator();
        assert_eq!(calc.get_value(), 0);
    }
}
"#;

        fs::write(&test_file, code)?;
        Ok((temp_dir, test_file.to_string_lossy().to_string()))
    }

    #[test]
    fn test_find_struct_references() -> Result<()> {
        let (_temp_dir, test_file) = setup_test_rust_project()?;

        // TODO: LSPサーバーからシンボル情報を取得してインデックス作成
        // ここでは仮のテストケースを作成

        let mut graph = CodeGraph::new();

        // Calculator構造体を追加
        let calc_symbol = Symbol {
            id: format!("{test_file}#2:Calculator"),
            kind: SymbolKind::Class,
            name: "Calculator".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 2,
                    character: 11,
                },
                end: Position {
                    line: 2,
                    character: 21,
                },
            },
            documentation: Some("Calculator struct".to_string()),
        };
        let _calc_idx = graph.add_symbol(calc_symbol);

        // Calculator::new()の定義を追加
        let new_symbol = Symbol {
            id: format!("{test_file}#7:new"),
            kind: SymbolKind::Method,
            name: "new".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 7,
                    character: 11,
                },
                end: Position {
                    line: 7,
                    character: 14,
                },
            },
            documentation: Some("Constructor".to_string()),
        };
        let new_idx = graph.add_symbol(new_symbol);

        // Calculator::new()への参照を追加（create_calculator関数内）
        let ref1_symbol = Symbol {
            id: format!("{test_file}#22:Calculator::new"),
            kind: SymbolKind::Function,
            name: "Calculator::new".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 22,
                    character: 4,
                },
                end: Position {
                    line: 22,
                    character: 19,
                },
            },
            documentation: None,
        };
        let ref1_idx = graph.add_symbol(ref1_symbol);

        // Calculator::new()への参照を追加（test関数内）
        let ref2_symbol = Symbol {
            id: format!("{test_file}#31:Calculator::new"),
            kind: SymbolKind::Function,
            name: "Calculator::new".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 31,
                    character: 24,
                },
                end: Position {
                    line: 31,
                    character: 39,
                },
            },
            documentation: None,
        };
        let ref2_idx = graph.add_symbol(ref2_symbol);

        // エッジを追加（参照関係）
        graph.add_edge(ref1_idx, new_idx, EdgeKind::Reference);
        graph.add_edge(ref2_idx, new_idx, EdgeKind::Reference);

        // 参照を検索
        let references = graph.find_references(&format!("{test_file}#7:new"));
        assert_eq!(
            references.len(),
            2,
            "Calculator::new()への参照が2つ見つかるべき"
        );

        // 参照元の位置を確認
        let ref_positions: Vec<(u32, u32)> = references
            .iter()
            .map(|s| (s.range.start.line, s.range.start.character))
            .collect();

        assert!(
            ref_positions.contains(&(22, 4)),
            "create_calculator内の参照が見つかるべき"
        );
        assert!(
            ref_positions.contains(&(31, 24)),
            "test関数内の参照が見つかるべき"
        );

        Ok(())
    }

    #[test]
    fn test_find_method_references() -> Result<()> {
        let (_temp_dir, test_file) = setup_test_rust_project()?;

        let mut graph = CodeGraph::new();

        // add()メソッドの定義
        let add_method = Symbol {
            id: format!("{test_file}#11:add"),
            kind: SymbolKind::Method,
            name: "add".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 11,
                    character: 11,
                },
                end: Position {
                    line: 11,
                    character: 14,
                },
            },
            documentation: Some("Add method".to_string()),
        };
        let add_idx = graph.add_symbol(add_method);

        // get_value()メソッドの定義
        let get_value_method = Symbol {
            id: format!("{test_file}#16:get_value"),
            kind: SymbolKind::Method,
            name: "get_value".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 16,
                    character: 11,
                },
                end: Position {
                    line: 16,
                    character: 20,
                },
            },
            documentation: Some("Get value method".to_string()),
        };
        let get_value_idx = graph.add_symbol(get_value_method);

        // test内でのadd()呼び出し
        let add_call = Symbol {
            id: format!("{test_file}#32:calc.add"),
            kind: SymbolKind::Function,
            name: "calc.add".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 32,
                    character: 20,
                },
                end: Position {
                    line: 32,
                    character: 28,
                },
            },
            documentation: None,
        };
        let add_call_idx = graph.add_symbol(add_call);
        graph.add_edge(add_call_idx, add_idx, EdgeKind::Reference);

        // test内でのget_value()呼び出し（2箇所）
        let get_value_call1 = Symbol {
            id: format!("{test_file}#33:calc.get_value"),
            kind: SymbolKind::Function,
            name: "calc.get_value".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 33,
                    character: 20,
                },
                end: Position {
                    line: 33,
                    character: 34,
                },
            },
            documentation: None,
        };
        let get_value_call1_idx = graph.add_symbol(get_value_call1);
        graph.add_edge(get_value_call1_idx, get_value_idx, EdgeKind::Reference);

        let get_value_call2 = Symbol {
            id: format!("{test_file}#39:calc.get_value"),
            kind: SymbolKind::Function,
            name: "calc.get_value".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 39,
                    character: 20,
                },
                end: Position {
                    line: 39,
                    character: 34,
                },
            },
            documentation: None,
        };
        let get_value_call2_idx = graph.add_symbol(get_value_call2);
        graph.add_edge(get_value_call2_idx, get_value_idx, EdgeKind::Reference);

        // add()メソッドへの参照を検索
        let add_refs = graph.find_references(&format!("{test_file}#11:add"));
        assert_eq!(add_refs.len(), 1, "add()メソッドへの参照が1つ見つかるべき");

        // get_value()メソッドへの参照を検索
        let get_value_refs = graph.find_references(&format!("{test_file}#16:get_value"));
        assert_eq!(
            get_value_refs.len(),
            2,
            "get_value()メソッドへの参照が2つ見つかるべき"
        );

        Ok(())
    }

    #[test]
    fn test_find_definition_from_reference() -> Result<()> {
        let (_temp_dir, test_file) = setup_test_rust_project()?;

        let mut graph = CodeGraph::new();

        // 定義を追加
        let definition = Symbol {
            id: format!("{test_file}#21:create_calculator"),
            kind: SymbolKind::Function,
            name: "create_calculator".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 21,
                    character: 7,
                },
                end: Position {
                    line: 21,
                    character: 24,
                },
            },
            documentation: Some("Factory function".to_string()),
        };
        let def_idx = graph.add_symbol(definition.clone());

        // 参照を追加
        let reference = Symbol {
            id: format!("{test_file}#38:create_calculator"),
            kind: SymbolKind::Function,
            name: "create_calculator".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 38,
                    character: 19,
                },
                end: Position {
                    line: 38,
                    character: 36,
                },
            },
            documentation: None,
        };
        let ref_idx = graph.add_symbol(reference.clone());

        // 参照から定義へのエッジを追加
        graph.add_edge(ref_idx, def_idx, EdgeKind::Reference);

        // 参照位置から定義を検索
        let found_def = graph.find_definition_at(
            &test_file,
            Position {
                line: 38,
                character: 25,
            },
        );

        assert!(found_def.is_some(), "定義が見つかるべき");
        let def = found_def.unwrap();
        assert_eq!(def.name, "create_calculator");
        assert_eq!(def.range.start.line, 21);

        Ok(())
    }
}

#[cfg(test)]
mod typescript_reference_tests {
    use super::*;
    use std::fs;

    fn setup_test_typescript_project() -> Result<(TempDir, String)> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("calculator.ts");

        let code = r#"
// テスト用のTypeScriptコード
export class Calculator {
    private value: number;
    
    constructor() {
        this.value = 0;
    }
    
    public add(x: number): number {
        this.value += x;
        return this.value;
    }
    
    public getValue(): number {
        return this.value;
    }
}

export function createCalculator(): Calculator {
    return new Calculator();
}

// 使用例
const calc1 = new Calculator();
calc1.add(10);
const result1 = calc1.getValue();

const calc2 = createCalculator();
calc2.add(20);
const result2 = calc2.getValue();

// インターフェースの例
interface CalculatorInterface {
    add(x: number): number;
    getValue(): number;
}

class AdvancedCalculator implements CalculatorInterface {
    private value = 0;
    
    add(x: number): number {
        this.value += x * 2;
        return this.value;
    }
    
    getValue(): number {
        return this.value;
    }
}
"#;

        fs::write(&test_file, code)?;
        Ok((temp_dir, test_file.to_string_lossy().to_string()))
    }

    #[test]
    fn test_find_class_references_typescript() -> Result<()> {
        let (_temp_dir, test_file) = setup_test_typescript_project()?;

        let mut graph = CodeGraph::new();

        // Calculatorクラスの定義
        let calc_class = Symbol {
            id: format!("{test_file}#2:Calculator"),
            kind: SymbolKind::Class,
            name: "Calculator".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 2,
                    character: 13,
                },
                end: Position {
                    line: 2,
                    character: 23,
                },
            },
            documentation: Some("Calculator class".to_string()),
        };
        let calc_idx = graph.add_symbol(calc_class);

        // new Calculator()の参照（2箇所）
        let new_calc1 = Symbol {
            id: format!("{test_file}#24:new Calculator"),
            kind: SymbolKind::Class,
            name: "new Calculator".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 24,
                    character: 14,
                },
                end: Position {
                    line: 24,
                    character: 29,
                },
            },
            documentation: None,
        };
        let new_calc1_idx = graph.add_symbol(new_calc1);
        graph.add_edge(new_calc1_idx, calc_idx, EdgeKind::Reference);

        let new_calc2 = Symbol {
            id: format!("{test_file}#20:new Calculator"),
            kind: SymbolKind::Class,
            name: "new Calculator".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 20,
                    character: 11,
                },
                end: Position {
                    line: 20,
                    character: 26,
                },
            },
            documentation: None,
        };
        let new_calc2_idx = graph.add_symbol(new_calc2);
        graph.add_edge(new_calc2_idx, calc_idx, EdgeKind::Reference);

        // Calculatorクラスへの参照を検索
        let refs = graph.find_references(&format!("{test_file}#2:Calculator"));
        assert_eq!(refs.len(), 2, "Calculatorクラスへの参照が2つ見つかるべき");

        Ok(())
    }

    #[test]
    fn test_find_interface_implementations() -> Result<()> {
        let (_temp_dir, test_file) = setup_test_typescript_project()?;

        let mut graph = CodeGraph::new();

        // インターフェースの定義
        let interface = Symbol {
            id: format!("{test_file}#33:CalculatorInterface"),
            kind: SymbolKind::Interface,
            name: "CalculatorInterface".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 33,
                    character: 10,
                },
                end: Position {
                    line: 33,
                    character: 29,
                },
            },
            documentation: Some("Calculator interface".to_string()),
        };
        let interface_idx = graph.add_symbol(interface);

        // 実装クラス
        let impl_class = Symbol {
            id: format!("{test_file}#38:AdvancedCalculator"),
            kind: SymbolKind::Class,
            name: "AdvancedCalculator".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 38,
                    character: 6,
                },
                end: Position {
                    line: 38,
                    character: 24,
                },
            },
            documentation: None,
        };
        let impl_idx = graph.add_symbol(impl_class);

        // implements関係を追加
        graph.add_edge(impl_idx, interface_idx, EdgeKind::Implementation);

        // インターフェースの実装を検索
        let implementations =
            graph.find_implementations(&format!("{test_file}#33:CalculatorInterface"));
        assert_eq!(
            implementations.len(),
            1,
            "インターフェースの実装が1つ見つかるべき"
        );
        assert_eq!(implementations[0].name, "AdvancedCalculator");

        Ok(())
    }

    #[test]
    fn test_find_method_overrides() -> Result<()> {
        let (_temp_dir, test_file) = setup_test_typescript_project()?;

        let mut graph = CodeGraph::new();

        // インターフェースのメソッド定義
        let interface_add = Symbol {
            id: format!("{test_file}#34:add"),
            kind: SymbolKind::Method,
            name: "add".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 34,
                    character: 4,
                },
                end: Position {
                    line: 34,
                    character: 7,
                },
            },
            documentation: Some("Interface add method".to_string()),
        };
        let interface_add_idx = graph.add_symbol(interface_add);

        // 実装クラスのメソッド
        let impl_add = Symbol {
            id: format!("{test_file}#41:add"),
            kind: SymbolKind::Method,
            name: "add".to_string(),
            file_path: test_file.clone(),
            range: Range {
                start: Position {
                    line: 41,
                    character: 4,
                },
                end: Position {
                    line: 41,
                    character: 7,
                },
            },
            documentation: None,
        };
        let impl_add_idx = graph.add_symbol(impl_add);

        // オーバーライド関係を追加
        graph.add_edge(impl_add_idx, interface_add_idx, EdgeKind::Override);

        // メソッドのオーバーライドを検索
        let overrides = graph.find_overrides(&format!("{test_file}#34:add"));
        assert_eq!(
            overrides.len(),
            1,
            "メソッドのオーバーライドが1つ見つかるべき"
        );
        assert_eq!(overrides[0].range.start.line, 41);

        Ok(())
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    #[ignore] // LSPサーバーが必要
    fn test_cross_language_references() -> Result<()> {
        // RustからTypeScriptのAPIを呼び出すケース（WASMバインディングなど）
        // TODO: 実装
        Ok(())
    }
}
