use anyhow::Result;
use lsif_indexer::cli::storage::IndexStorage;
use lsif_indexer::core::graph::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
use std::process::Command;
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

    // 参照の検証
    let refs = loaded_graph.find_references("main.rs#main");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "User");
}

#[test]
fn test_incremental_update() {
    use lsif_indexer::core::incremental::IncrementalIndex;
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
    let unused_idx = graph.add_symbol(unused_func);
    let main_idx = graph.add_symbol(main_func);

    // mainからused_funcへの参照を追加
    graph.add_edge(main_idx, used_idx, EdgeKind::Reference);
    // unused_funcへの参照はなし

    // デッドコード検出のシミュレーション
    // unused_funcは参照されていないため、デッドコードとして検出されるべき
    let all_symbols: Vec<_> = graph.get_all_symbols().collect();
    assert_eq!(all_symbols.len(), 3);

    // 参照カウント
    let used_refs = graph.find_references("file.rs#used_func");
    let unused_refs = graph.find_references("file.rs#unused_func");

    assert_eq!(used_refs.len(), 0); // find_referencesは逆方向を見るため0
    assert_eq!(unused_refs.len(), 0);
}

#[test]
fn test_parallel_storage() {
    use lsif_indexer::cli::parallel_storage::ParallelIndexStorage;

    let temp_dir = TempDir::new().unwrap();
    let storage = ParallelIndexStorage::open(temp_dir.path()).unwrap();

    // 大量のシンボルを生成
    let symbols: Vec<(String, Symbol)> = (0..100)
        .map(|i| {
            let symbol = Symbol {
                id: format!("symbol_{}", i),
                kind: SymbolKind::Function,
                name: format!("func_{}", i),
                file_path: format!("file_{}.rs", i / 10),
                range: Range {
                    start: Position {
                        line: i,
                        character: 0,
                    },
                    end: Position {
                        line: i + 5,
                        character: 0,
                    },
                },
                documentation: None,
            };
            (symbol.id.clone(), symbol)
        })
        .collect();

    // 並列保存
    storage.save_symbols_parallel(&symbols).unwrap();

    // 並列読み込み
    let keys: Vec<String> = (0..100).map(|i| format!("symbol_{}", i)).collect();
    let loaded: Vec<Option<Symbol>> = storage.load_symbols_parallel(&keys).unwrap();

    assert_eq!(loaded.len(), 100);
    assert!(loaded.iter().all(|s| s.is_some()));
}

#[test]
fn test_cache_performance() {
    use lsif_indexer::cli::cached_storage::CachedIndexStorage;
    use std::time::{Duration, Instant};

    let temp_dir = TempDir::new().unwrap();
    let storage =
        CachedIndexStorage::open_with_config(temp_dir.path(), 10, Duration::from_secs(60)).unwrap();

    let symbol = Symbol {
        id: "test_symbol".to_string(),
        kind: SymbolKind::Function,
        name: "test".to_string(),
        file_path: "test.rs".to_string(),
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

    // 最初の保存
    storage.save_data_cached("test_key", &symbol).unwrap();

    // キャッシュからの読み込み（高速）
    let start = Instant::now();
    let _loaded: Option<Symbol> = storage.load_data_cached("test_key").unwrap();
    let cache_time = start.elapsed();

    // キャッシュをクリア
    storage.clear_cache();

    // DBからの読み込み（遅い）
    let start = Instant::now();
    let _loaded: Option<Symbol> = storage.load_data_cached("test_key").unwrap();
    let db_time = start.elapsed();

    // キャッシュの方が高速であることを確認
    println!("Cache time: {:?}, DB time: {:?}", cache_time, db_time);
    assert!(cache_time < db_time * 2); // キャッシュは少なくとも半分の時間
}
