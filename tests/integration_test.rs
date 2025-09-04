use cli::storage::IndexStorage;
use lsif_core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
use tempfile::TempDir;

#[test]
fn test_symbol_storage_and_retrieval() {
    let temp_dir = TempDir::new().unwrap();
    let storage = IndexStorage::open(temp_dir.path()).unwrap();

    // テストシンボルを作成
    let mut graph = CodeGraph::new();

    let user_symbol = Symbol {
        id: "user.rs#User".to_string(),
        kind: SymbolKind::Class,
        name: "User".to_string(),
        file_path: "user.rs".to_string(),
        range: Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 10,
                character: 0,
            },
        },
        documentation: Some("User struct".to_string()),
    };

    let main_symbol = Symbol {
        id: "main.rs#main".to_string(),
        kind: SymbolKind::Function,
        name: "main".to_string(),
        file_path: "main.rs".to_string(),
        range: Range {
            start: Position {
                line: 5,
                character: 0,
            },
            end: Position {
                line: 15,
                character: 0,
            },
        },
        documentation: Some("Main function".to_string()),
    };

    // シンボルをグラフに追加
    let user_idx = graph.add_symbol(user_symbol.clone());
    let main_idx = graph.add_symbol(main_symbol.clone());

    // エッジを追加（mainからUserへの参照）
    graph.add_edge(main_idx, user_idx, EdgeKind::Reference);

    // ストレージに保存
    storage.save_data("test_graph", &graph).unwrap();

    // ストレージから読み込み
    let loaded_graph: CodeGraph = storage.load_data("test_graph").unwrap().unwrap();

    // 検証
    assert_eq!(loaded_graph.symbol_count(), 2);
    assert!(loaded_graph.find_symbol("user.rs#User").is_some());
    assert!(loaded_graph.find_symbol("main.rs#main").is_some());

    // 参照の検証（Userを参照しているのはmain）
    let refs = loaded_graph.find_references("user.rs#User");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "main");
}

#[test]
fn test_incremental_update() {
    use lsif_lsif_core::IncrementalIndex;
    use std::path::Path;

    let mut index = IncrementalIndex::new();

    // 初回のシンボル追加
    let symbols_v1 = vec![
        Symbol {
            id: "file1.rs#func1".to_string(),
            kind: SymbolKind::Function,
            name: "func1".to_string(),
            file_path: "file1.rs".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 0,
                },
            },
            documentation: None,
        },
        Symbol {
            id: "file1.rs#func2".to_string(),
            kind: SymbolKind::Function,
            name: "func2".to_string(),
            file_path: "file1.rs".to_string(),
            range: Range {
                start: Position {
                    line: 10,
                    character: 0,
                },
                end: Position {
                    line: 15,
                    character: 0,
                },
            },
            documentation: None,
        },
    ];

    let result = index
        .update_file(Path::new("file1.rs"), symbols_v1, "hash1".to_string())
        .unwrap();

    assert_eq!(result.added_symbols.len(), 2);
    assert_eq!(result.updated_symbols.len(), 0);
    assert_eq!(result.removed_symbols.len(), 0);

    // 更新（1つ変更、1つ削除、1つ追加）
    let symbols_v2 = vec![
        Symbol {
            id: "file1.rs#func1".to_string(),
            kind: SymbolKind::Function,
            name: "func1_renamed".to_string(), // 名前変更
            file_path: "file1.rs".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 0,
                },
            },
            documentation: None,
        },
        Symbol {
            id: "file1.rs#func3".to_string(), // 新規
            kind: SymbolKind::Function,
            name: "func3".to_string(),
            file_path: "file1.rs".to_string(),
            range: Range {
                start: Position {
                    line: 20,
                    character: 0,
                },
                end: Position {
                    line: 25,
                    character: 0,
                },
            },
            documentation: None,
        },
    ];

    let result = index
        .update_file(Path::new("file1.rs"), symbols_v2, "hash2".to_string())
        .unwrap();

    assert_eq!(result.added_symbols.len(), 1); // func3
    assert_eq!(result.updated_symbols.len(), 1); // func1
    assert_eq!(result.removed_symbols.len(), 1); // func2
}

#[test]
fn test_dead_code_detection() {
    let mut graph = CodeGraph::new();

    // 使用されている関数
    let used_func = Symbol {
        id: "file.rs#used_func".to_string(),
        kind: SymbolKind::Function,
        name: "used_func".to_string(),
        file_path: "file.rs".to_string(),
        range: Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 5,
                character: 0,
            },
        },
        documentation: None,
    };

    // 使用されていない関数
    let unused_func = Symbol {
        id: "file.rs#unused_func".to_string(),
        kind: SymbolKind::Function,
        name: "unused_func".to_string(),
        file_path: "file.rs".to_string(),
        range: Range {
            start: Position {
                line: 10,
                character: 0,
            },
            end: Position {
                line: 15,
                character: 0,
            },
        },
        documentation: None,
    };

    // main関数
    let main_func = Symbol {
        id: "file.rs#main".to_string(),
        kind: SymbolKind::Function,
        name: "main".to_string(),
        file_path: "file.rs".to_string(),
        range: Range {
            start: Position {
                line: 20,
                character: 0,
            },
            end: Position {
                line: 30,
                character: 0,
            },
        },
        documentation: None,
    };

    let used_idx = graph.add_symbol(used_func);
    let _unused_idx = graph.add_symbol(unused_func);
    let main_idx = graph.add_symbol(main_func);

    // mainからused_funcへの参照を追加
    graph.add_edge(main_idx, used_idx, EdgeKind::Reference);
    // unused_funcへの参照はなし

    // デッドコード検出のシミュレーション
    // unused_funcは参照されていないため、デッドコードとして検出されるべき
    let all_symbols: Vec<_> = graph.get_all_symbols().collect();
    assert_eq!(all_symbols.len(), 3);

    // 参照カウント（誰がこのシンボルを参照しているか）
    let used_refs = graph.find_references("file.rs#used_func");
    let unused_refs = graph.find_references("file.rs#unused_func");

    assert_eq!(used_refs.len(), 1); // mainから参照されている
    assert_eq!(used_refs[0].id, "file.rs#main");
    assert_eq!(unused_refs.len(), 0); // 誰からも参照されていない（デッドコード）
}

// Parallel storage test removed - functionality merged into IndexStorage

// Cache performance test removed - functionality merged into IndexStorage
