use cli::reference_finder::find_all_references;
use lsif_core::SymbolKind;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// テスト用のファイルを作成
fn create_test_files(temp_dir: &Path) -> anyhow::Result<()> {
    // メインファイル
    let main_content = r#"// main.rs
fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {
    let result = calculate_sum(10, 20);
    println!("Result: {}", result);
    
    let helper = Helper::new();
    helper.process();
}

struct Helper {
    value: i32,
}

impl Helper {
    fn new() -> Self {
        Helper { value: 0 }
    }
    
    fn process(&self) {
        let sum = calculate_sum(self.value, 5);
        println!("Sum: {}", sum);
    }
}
"#;
    fs::write(temp_dir.join("main.rs"), main_content)?;

    // 別のファイル
    let utils_content = r#"// utils.rs
use crate::Helper;

fn use_helper() {
    let helper = Helper::new();
    helper.process();
}

fn another_function() {
    // calculate_sumは使わない
    let x = 10;
    let y = 20;
    let z = x + y;
}
"#;
    fs::write(temp_dir.join("utils.rs"), utils_content)?;

    // テストファイル
    let test_content = r#"// test.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_sum() {
        assert_eq!(calculate_sum(2, 3), 5);
        assert_eq!(calculate_sum(-1, 1), 0);
    }
}
"#;
    fs::write(temp_dir.join("test.rs"), test_content)?;

    Ok(())
}

#[test]
fn test_find_function_references() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(temp_dir.path()).unwrap();

    // calculate_sum関数の参照を検索
    let references =
        find_all_references(temp_dir.path(), "calculate_sum", &SymbolKind::Function).unwrap();

    // 期待される参照数をチェック
    assert!(
        references.len() >= 3,
        "Expected at least 3 references, got {}",
        references.len()
    );

    // 定義と使用を分ける
    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

    // 定義は1つ
    assert_eq!(definitions.len(), 1, "Expected 1 definition");

    // 使用箇所は少なくとも2つ（main内とprocess内）
    assert!(
        usages.len() >= 2,
        "Expected at least 2 usages, got {}",
        usages.len()
    );

    // main.rsファイル内の参照をチェック
    let main_refs: Vec<_> = references
        .iter()
        .filter(|r| r.symbol.file_path.ends_with("main.rs"))
        .collect();
    assert!(
        main_refs.len() >= 3,
        "Expected at least 3 references in main.rs"
    );
}

#[test]
fn test_find_struct_references() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(temp_dir.path()).unwrap();

    // Helper構造体の参照を検索
    let references = find_all_references(temp_dir.path(), "Helper", &SymbolKind::Struct).unwrap();

    // 期待される参照数をチェック
    assert!(
        references.len() >= 4,
        "Expected at least 4 references, got {}",
        references.len()
    );

    // 定義と使用を分ける
    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

    // 定義は1つ
    assert_eq!(definitions.len(), 1, "Expected 1 definition");

    // 使用箇所をチェック
    assert!(usages.len() >= 3, "Expected at least 3 usages");
}

#[test]
fn test_find_method_references() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(temp_dir.path()).unwrap();

    // processメソッドの参照を検索
    let references = find_all_references(temp_dir.path(), "process", &SymbolKind::Method).unwrap();

    // 少なくとも定義と2つの呼び出しがあるはず
    assert!(
        references.len() >= 3,
        "Expected at least 3 references, got {}",
        references.len()
    );

    // 呼び出し箇所の確認
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();
    assert!(usages.len() >= 2, "Expected at least 2 method calls");
}

#[test]
fn test_no_references_found() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(temp_dir.path()).unwrap();

    // 存在しない関数の参照を検索
    let references = find_all_references(
        temp_dir.path(),
        "non_existent_function",
        &SymbolKind::Function,
    )
    .unwrap();

    assert!(
        references.is_empty(),
        "Expected no references for non-existent function"
    );
}

#[test]
fn test_reference_locations() {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(temp_dir.path()).unwrap();

    // calculate_sum関数の参照を検索
    let references =
        find_all_references(temp_dir.path(), "calculate_sum", &SymbolKind::Function).unwrap();

    // 各参照の位置情報が正しいかチェック
    for reference in &references {
        assert!(!reference.symbol.file_path.is_empty());
        assert!(reference.symbol.range.start.line < 100); // 妥当な行番号
        assert!(reference.symbol.range.start.character < 200); // 妥当な列番号
        assert_eq!(reference.symbol.name, "calculate_sum");
    }

    // 定義の位置をチェック
    let definition = references.iter().find(|r| r.is_definition);
    assert!(definition.is_some(), "Definition not found");

    if let Some(def) = definition {
        assert_eq!(def.symbol.range.start.line, 1); // fn calculate_sumは2行目（0-indexed）
    }
}

#[test]
fn test_pattern_matching_accuracy() {
    let temp_dir = TempDir::new().unwrap();

    // 特殊なケースを含むファイルを作成
    let content = r#"
fn test() {
    // コメント内のtest_functionは無視されるべき
    let str = "test_function in string"; // 文字列内も無視
    
    test_function(); // これは検出されるべき
    another_test_function(); // これは検出されない
}

fn test_function() {
    println!("Test");
}
"#;
    fs::write(temp_dir.path().join("special.rs"), content).unwrap();

    let references =
        find_all_references(temp_dir.path(), "test_function", &SymbolKind::Function).unwrap();

    // 定義と有効な呼び出しのみが検出される
    assert_eq!(
        references.len(),
        2,
        "Expected exactly 2 references (1 definition + 1 usage)"
    );

    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

    assert_eq!(definitions.len(), 1);
    assert_eq!(usages.len(), 1);
}
