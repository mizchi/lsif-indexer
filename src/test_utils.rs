use crate::core::graph::{Symbol, SymbolKind, Range, Position, CodeGraph};
use crate::cli::storage::{IndexStorage, IndexMetadata, IndexFormat};
use tempfile::TempDir;
use std::path::Path;

/// テスト用のシンボルを作成
pub fn create_test_symbol(id: &str, name: &str, kind: SymbolKind) -> Symbol {
    Symbol {
        id: id.to_string(),
        kind,
        name: name.to_string(),
        file_path: "test.rs".to_string(),
        range: Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 10 },
        },
        documentation: None,
    }
}

/// テスト用のグラフを作成
pub fn create_test_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();
    
    // 関数シンボル
    let func = create_test_symbol("func1", "test_function", SymbolKind::Function);
    let func_idx = graph.add_symbol(func);
    
    // 変数シンボル
    let var = create_test_symbol("var1", "test_variable", SymbolKind::Variable);
    let var_idx = graph.add_symbol(var);
    
    // 参照を追加
    graph.add_edge(var_idx, func_idx, crate::core::graph::EdgeKind::Reference);
    
    graph
}

/// テスト用の一時ストレージを作成
pub fn create_test_storage() -> (TempDir, IndexStorage) {
    let temp_dir = TempDir::new().unwrap();
    let storage = IndexStorage::open(temp_dir.path()).unwrap();
    (temp_dir, storage)
}

/// テスト用のメタデータを作成
pub fn create_test_metadata() -> IndexMetadata {
    IndexMetadata {
        format: IndexFormat::Lsif,
        version: "1.0.0".to_string(),
        created_at: chrono::Utc::now(),
        project_root: "/test".to_string(),
        files_count: 1,
        symbols_count: 2,
        git_commit_hash: Some("abcd1234".to_string()),
        file_hashes: std::collections::HashMap::new(),
    }
}

/// アサーション用のマクロ
#[macro_export]
macro_rules! assert_symbol_eq {
    ($left:expr, $right:expr) => {{
        let left = $left;
        let right = $right;
        assert_eq!(left.id, right.id, "Symbol IDs don't match");
        assert_eq!(left.name, right.name, "Symbol names don't match");
        assert_eq!(left.kind, right.kind, "Symbol kinds don't match");
        assert_eq!(left.file_path, right.file_path, "File paths don't match");
    }};
}