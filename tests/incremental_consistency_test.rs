use lsif_core::{
    calculate_file_hash, EdgeKind, FileUpdate, IncrementalIndex, Position, Range, Symbol,
    SymbolKind,
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn create_test_symbol(id: &str, name: &str, file: &str) -> Symbol {
    Symbol {
        id: id.to_string(),
        name: name.to_string(),
        kind: SymbolKind::Function,
        file_path: file.to_string(),
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
    }
}

/// エッジの整合性をチェック: 削除されたシンボルへの参照がないことを確認
#[test]
fn test_edge_consistency_after_symbol_removal() {
    let mut index = IncrementalIndex::new();

    // A -> B -> C のチェーンを作成
    let symbol_a = create_test_symbol("sym_a", "function_a", "file1.rs");
    let symbol_b = create_test_symbol("sym_b", "function_b", "file2.rs");
    let symbol_c = create_test_symbol("sym_c", "function_c", "file3.rs");

    // ファイルごとに正しく追加
    index
        .update_file(Path::new("file1.rs"), vec![symbol_a], "hash1".to_string())
        .unwrap();
    index
        .update_file(Path::new("file2.rs"), vec![symbol_b], "hash2".to_string())
        .unwrap();
    index
        .update_file(Path::new("file3.rs"), vec![symbol_c], "hash3".to_string())
        .unwrap();

    // エッジを追加（シンボルIDで追加する方が安全）
    let idx_a = index.graph.get_node_index("sym_a").unwrap();
    let idx_b = index.graph.get_node_index("sym_b").unwrap();
    index.graph.add_edge(idx_a, idx_b, EdgeKind::Reference);

    let idx_b = index.graph.get_node_index("sym_b").unwrap();
    let idx_c = index.graph.get_node_index("sym_c").unwrap();
    index.graph.add_edge(idx_b, idx_c, EdgeKind::Reference);

    // Bを含むファイルを削除
    index.remove_file(Path::new("file2.rs")).unwrap();

    // Bが削除されていることを確認
    assert!(index.graph.find_symbol("sym_b").is_none());

    // AからBへのエッジ、BからCへのエッジが削除されていることを確認
    let a_refs = index.graph.find_references("sym_a");
    assert_eq!(
        a_refs.len(),
        0,
        "References from A should be empty after B is removed"
    );

    // Cはまだ存在するが、参照元がない
    assert!(index.graph.find_symbol("sym_c").is_some());

    // グラフの整合性チェック: 削除されたノードへの参照がないことを間接的に確認
    // 存在するすべてのシンボルから参照を辿れることを確認
    assert!(index.graph.find_symbol("sym_a").is_some());
    assert!(index.graph.find_symbol("sym_c").is_some());

    // グラフの基本的な整合性: シンボル数が正しい
    assert_eq!(index.graph.symbol_count(), 2);
}

/// 同じファイルの連続更新での整合性
#[test]
fn test_rapid_sequential_updates() {
    let mut index = IncrementalIndex::new();

    let file_path = Path::new("rapid_update.rs");

    // 10回連続で更新
    for i in 0..10 {
        let symbols = vec![
            create_test_symbol(
                &format!("func_{i}"),
                &format!("function_{i}"),
                "rapid_update.rs",
            ),
            create_test_symbol(
                &format!("var_{i}"),
                &format!("variable_{i}"),
                "rapid_update.rs",
            ),
        ];

        let result = index
            .update_file(file_path, symbols, format!("hash_{i}"))
            .unwrap();

        // 最初の更新以外は、前回のシンボルが削除され、新しいシンボルが追加される
        if i > 0 {
            assert_eq!(result.removed_symbols.len(), 2);
            assert_eq!(result.added_symbols.len(), 2);
        }

        // 常に2つのシンボルのみ存在
        assert_eq!(index.graph.symbol_count(), 2);

        // symbol_to_fileの整合性
        assert_eq!(index.symbol_to_file.len(), 2);
        for path in index.symbol_to_file.values() {
            assert_eq!(path, &PathBuf::from("rapid_update.rs"));
        }
    }
}

/// 循環参照がある場合の削除
#[test]
fn test_circular_reference_removal() {
    let mut index = IncrementalIndex::new();

    // A -> B -> C -> A の循環参照を作成
    let symbol_a = create_test_symbol("cycle_a", "func_a", "file_a.rs");
    let symbol_b = create_test_symbol("cycle_b", "func_b", "file_b.rs");
    let symbol_c = create_test_symbol("cycle_c", "func_c", "file_c.rs");

    // ファイルごとに正しく追加
    index
        .update_file(Path::new("file_a.rs"), vec![symbol_a], "hash_a".to_string())
        .unwrap();
    index
        .update_file(Path::new("file_b.rs"), vec![symbol_b], "hash_b".to_string())
        .unwrap();
    index
        .update_file(Path::new("file_c.rs"), vec![symbol_c], "hash_c".to_string())
        .unwrap();

    let idx_a = index.graph.get_node_index("cycle_a").unwrap();
    let idx_b = index.graph.get_node_index("cycle_b").unwrap();
    let idx_c = index.graph.get_node_index("cycle_c").unwrap();

    index.graph.add_edge(idx_a, idx_b, EdgeKind::Reference);
    index.graph.add_edge(idx_b, idx_c, EdgeKind::Reference);
    index.graph.add_edge(idx_c, idx_a, EdgeKind::Reference);

    // Bを削除
    index.remove_file(Path::new("file_b.rs")).unwrap();

    // AとCは残っているが、循環参照は切れている
    assert!(index.graph.find_symbol("cycle_a").is_some());
    assert!(index.graph.find_symbol("cycle_b").is_none());
    assert!(index.graph.find_symbol("cycle_c").is_some());

    // Bへの参照がないことを確認（Bは削除されている）
    // Cはまだ存在するので、Aへの参照はある（C->A）
    let a_refs = index.graph.find_references("cycle_a");
    assert_eq!(a_refs.len(), 1, "CからAへの参照は残っている");
    assert_eq!(a_refs[0].id, "cycle_c");
}

/// 大量のシンボル更新での整合性
#[test]
fn test_bulk_update_consistency() {
    let mut index = IncrementalIndex::new();

    // 1000個のシンボルを10ファイルに分散して追加
    let files_count = 10usize;
    let symbols_per_file = 100usize;

    for file_idx in 0..files_count {
        let mut symbols = Vec::new();
        for sym_idx in 0..symbols_per_file {
            symbols.push(Symbol {
                id: format!("f{file_idx}_s{sym_idx}"),
                name: format!("symbol_{file_idx}_{sym_idx}"),
                kind: SymbolKind::Function,
                file_path: format!("file_{file_idx}.rs"),
                range: Range {
                    start: Position {
                        line: (sym_idx * 10) as u32,
                        character: 0,
                    },
                    end: Position {
                        line: (sym_idx * 10 + 5) as u32,
                        character: 0,
                    },
                },
                documentation: None,
            });
        }

        index
            .update_file(
                Path::new(&format!("file_{file_idx}.rs")),
                symbols,
                format!("initial_hash_{file_idx}"),
            )
            .unwrap();
    }

    // 初期状態の検証
    assert_eq!(index.graph.symbol_count(), files_count * symbols_per_file);
    assert_eq!(index.file_metadata.len(), files_count);
    assert_eq!(index.symbol_to_file.len(), files_count * symbols_per_file);

    // すべてのファイルを同時に更新（バッチ更新）
    let mut updates = Vec::new();
    for file_idx in 0..files_count {
        let mut new_symbols = Vec::new();
        // 半分を保持、半分を新規
        for sym_idx in 0..symbols_per_file / 2 {
            new_symbols.push(Symbol {
                id: format!("f{file_idx}_s{sym_idx}"),
                name: format!("updated_symbol_{file_idx}_{sym_idx}"),
                kind: SymbolKind::Function,
                file_path: format!("file_{file_idx}.rs"),
                range: Range {
                    start: Position {
                        line: (sym_idx * 10) as u32,
                        character: 0,
                    },
                    end: Position {
                        line: (sym_idx * 10 + 5) as u32,
                        character: 0,
                    },
                },
                documentation: None,
            });
        }
        for sym_idx in 0..symbols_per_file / 2 {
            new_symbols.push(Symbol {
                id: format!("f{file_idx}_new_s{sym_idx}"),
                name: format!("new_symbol_{file_idx}_{sym_idx}"),
                kind: SymbolKind::Function,
                file_path: format!("file_{file_idx}.rs"),
                range: Range {
                    start: Position {
                        line: ((sym_idx + symbols_per_file / 2) * 10) as u32,
                        character: 0,
                    },
                    end: Position {
                        line: ((sym_idx + symbols_per_file / 2) * 10 + 5) as u32,
                        character: 0,
                    },
                },
                documentation: None,
            });
        }

        updates.push(FileUpdate::Modified {
            path: PathBuf::from(format!("file_{file_idx}.rs")),
            symbols: new_symbols,
            hash: format!("updated_hash_{file_idx}"),
        });
    }

    let result = index.batch_update(updates).unwrap();

    // バッチ更新の結果を検証
    assert_eq!(result.total_added, files_count * symbols_per_file / 2);
    assert_eq!(result.total_removed, files_count * symbols_per_file / 2);
    assert_eq!(result.total_updated, files_count * symbols_per_file / 2);
    assert_eq!(result.affected_files, files_count);

    // 最終状態の整合性を検証
    assert_eq!(index.graph.symbol_count(), files_count * symbols_per_file);
    assert_eq!(index.file_metadata.len(), files_count);
    assert_eq!(index.symbol_to_file.len(), files_count * symbols_per_file);

    // すべてのsymbol_to_fileマッピングが正しいことを確認
    for (symbol_id, file_path) in &index.symbol_to_file {
        let symbol = index.graph.find_symbol(symbol_id).unwrap();
        assert_eq!(symbol.file_path, file_path.to_string_lossy());
    }
}

/// 異なる種類の更新を混在させた場合の整合性
#[test]
fn test_mixed_update_operations() {
    let mut index = IncrementalIndex::new();

    // 初期ファイルを設定
    for i in 0..5 {
        let symbols = vec![
            create_test_symbol(
                &format!("init_s{i}_1"),
                &format!("symbol_{i}_1"),
                &format!("file_{i}.rs"),
            ),
            create_test_symbol(
                &format!("init_s{i}_2"),
                &format!("symbol_{i}_2"),
                &format!("file_{i}.rs"),
            ),
        ];
        index
            .update_file(
                Path::new(&format!("file_{i}.rs")),
                symbols,
                format!("hash_{i}"),
            )
            .unwrap();
    }

    // 複雑な更新パターン
    let mixed_updates = vec![
        // file_0: 変更なし（スキップ）
        // file_1: 修正
        FileUpdate::Modified {
            path: PathBuf::from("file_1.rs"),
            symbols: vec![
                create_test_symbol("init_s1_1", "modified_symbol_1_1", "file_1.rs"),
                create_test_symbol("new_s1_3", "new_symbol_1_3", "file_1.rs"),
            ],
            hash: "hash_1_modified".to_string(),
        },
        // file_2: 削除
        FileUpdate::Removed {
            path: PathBuf::from("file_2.rs"),
        },
        // file_3: 変更なし（スキップ）
        // file_4: 削除して再追加
        FileUpdate::Removed {
            path: PathBuf::from("file_4.rs"),
        },
        FileUpdate::Added {
            path: PathBuf::from("file_4.rs"),
            symbols: vec![create_test_symbol(
                "new_s4_1",
                "brand_new_symbol_4_1",
                "file_4.rs",
            )],
            hash: "hash_4_new".to_string(),
        },
        // 新規ファイル追加
        FileUpdate::Added {
            path: PathBuf::from("file_5.rs"),
            symbols: vec![create_test_symbol(
                "new_s5_1",
                "new_file_symbol_5_1",
                "file_5.rs",
            )],
            hash: "hash_5".to_string(),
        },
    ];

    let _result = index.batch_update(mixed_updates).unwrap();

    // 期待される結果
    // - file_0, file_3: 変更なし（各2シンボル）
    // - file_1: 1つ更新、1つ削除、1つ追加（計2シンボル）
    // - file_2: 削除（0シンボル）
    // - file_4: 2つ削除、1つ追加（計1シンボル）
    // - file_5: 新規（1シンボル）

    let expected_symbol_count = (2 + 2 + 2) + 1 + 1;
    assert_eq!(index.graph.symbol_count(), expected_symbol_count);

    // ファイルメタデータの確認
    assert!(index
        .file_metadata
        .contains_key(&PathBuf::from("file_0.rs")));
    assert!(index
        .file_metadata
        .contains_key(&PathBuf::from("file_1.rs")));
    assert!(!index
        .file_metadata
        .contains_key(&PathBuf::from("file_2.rs")));
    assert!(index
        .file_metadata
        .contains_key(&PathBuf::from("file_3.rs")));
    assert!(index
        .file_metadata
        .contains_key(&PathBuf::from("file_4.rs")));
    assert!(index
        .file_metadata
        .contains_key(&PathBuf::from("file_5.rs")));

    // 各ファイルのシンボル数を確認
    assert_eq!(
        index.file_metadata[&PathBuf::from("file_0.rs")]
            .symbols
            .len(),
        2
    );
    assert_eq!(
        index.file_metadata[&PathBuf::from("file_1.rs")]
            .symbols
            .len(),
        2
    );
    assert_eq!(
        index.file_metadata[&PathBuf::from("file_3.rs")]
            .symbols
            .len(),
        2
    );
    assert_eq!(
        index.file_metadata[&PathBuf::from("file_4.rs")]
            .symbols
            .len(),
        1
    );
    assert_eq!(
        index.file_metadata[&PathBuf::from("file_5.rs")]
            .symbols
            .len(),
        1
    );
}

/// symbol_to_fileマッピングの整合性を詳細にチェック
#[test]
fn test_symbol_to_file_mapping_consistency() {
    let mut index = IncrementalIndex::new();

    // シンボルを追加
    let symbol1 = create_test_symbol("map_test_1", "symbol1", "original.rs");
    index.add_symbol(symbol1).unwrap();

    assert_eq!(
        index.symbol_to_file["map_test_1"],
        PathBuf::from("original.rs")
    );

    // 同じIDで異なるファイルのシンボルに更新
    let symbol1_moved = Symbol {
        id: "map_test_1".to_string(),
        name: "symbol1".to_string(),
        kind: SymbolKind::Function,
        file_path: "moved.rs".to_string(),
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

    // 古いファイルから削除
    index
        .update_file(Path::new("original.rs"), vec![], "hash_empty".to_string())
        .unwrap();

    // 新しいファイルに追加
    index
        .update_file(
            Path::new("moved.rs"),
            vec![symbol1_moved],
            "hash_moved".to_string(),
        )
        .unwrap();

    // マッピングが更新されていることを確認
    assert_eq!(
        index.symbol_to_file["map_test_1"],
        PathBuf::from("moved.rs")
    );
    assert_eq!(
        index.graph.find_symbol("map_test_1").unwrap().file_path,
        "moved.rs"
    );
}

/// デッドコード検出の整合性
#[test]
fn test_dead_code_detection_consistency() {
    let mut index = IncrementalIndex::new();

    // エントリーポイントと複雑な依存関係を構築
    let main = Symbol {
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
                line: 10,
                character: 0,
            },
        },
        documentation: None,
    };

    let lib_pub = Symbol {
        id: "lib_pub".to_string(),
        name: "pub exported_function".to_string(),
        kind: SymbolKind::Function,
        file_path: "lib.rs".to_string(),
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

    let util1 = create_test_symbol("util1", "utility1", "utils.rs");
    let util2 = create_test_symbol("util2", "utility2", "utils.rs");
    let internal = create_test_symbol("internal", "internal_func", "internal.rs");
    let orphan = create_test_symbol("orphan", "orphan_func", "orphan.rs");

    // シンボルを追加（ファイルごとに）
    index
        .update_file(Path::new("main.rs"), vec![main], "hash_main".to_string())
        .unwrap();
    index
        .update_file(Path::new("lib.rs"), vec![lib_pub], "hash_lib".to_string())
        .unwrap();
    index
        .update_file(
            Path::new("utils.rs"),
            vec![util1, util2],
            "hash_utils".to_string(),
        )
        .unwrap();
    index
        .update_file(
            Path::new("internal.rs"),
            vec![internal],
            "hash_internal".to_string(),
        )
        .unwrap();
    index
        .update_file(
            Path::new("orphan.rs"),
            vec![orphan],
            "hash_orphan".to_string(),
        )
        .unwrap();

    let main_idx = index.graph.get_node_index("main").unwrap();
    let pub_idx = index.graph.get_node_index("lib_pub").unwrap();
    let util1_idx = index.graph.get_node_index("util1").unwrap();
    let util2_idx = index.graph.get_node_index("util2").unwrap();
    let internal_idx = index.graph.get_node_index("internal").unwrap();

    // 依存関係を構築
    index
        .graph
        .add_edge(main_idx, util1_idx, EdgeKind::Reference);
    index
        .graph
        .add_edge(pub_idx, util2_idx, EdgeKind::Reference);
    index
        .graph
        .add_edge(util1_idx, internal_idx, EdgeKind::Reference);
    index
        .graph
        .add_edge(util2_idx, internal_idx, EdgeKind::Reference);

    // デッドコード検出
    let mut result = lsif_core::UpdateResult::default();
    index.detect_dead_code(&mut result);

    // orphanのみがデッドコード
    assert_eq!(result.dead_symbols.len(), 1);
    assert!(result.dead_symbols.contains("orphan"));

    // util2を削除して、更新として処理
    let empty_symbols = vec![];
    index
        .update_file(
            Path::new("utils.rs"),
            empty_symbols,
            "hash_empty".to_string(),
        )
        .unwrap();

    // util1だけを再追加
    let util1_only = vec![create_test_symbol("util1", "utility1", "utils.rs")];
    index
        .update_file(
            Path::new("utils.rs"),
            util1_only,
            "hash_util1_only".to_string(),
        )
        .unwrap();

    // エッジを再作成（main -> util1, util1 -> internal）
    let main_idx = index.graph.get_node_index("main").unwrap();
    let util1_idx = index.graph.get_node_index("util1").unwrap();
    let internal_idx = index.graph.get_node_index("internal").unwrap();
    index
        .graph
        .add_edge(main_idx, util1_idx, EdgeKind::Reference);
    index
        .graph
        .add_edge(util1_idx, internal_idx, EdgeKind::Reference);

    // 再度デッドコード検出
    let mut result2 = lsif_core::UpdateResult::default();
    index.detect_dead_code(&mut result2);

    // orphanのみがデッドコード（util1が残っており、internalへの参照も残っている）
    assert_eq!(result2.dead_symbols.len(), 1);
    assert!(result2.dead_symbols.contains("orphan"));
    // internalはutil1から参照されているため、デッドコードではない
}

/// 実際のファイルシステムとの統合テスト
#[test]
fn test_real_filesystem_consistency() {
    let temp_dir = TempDir::new().unwrap();
    let mut index = IncrementalIndex::new();

    // テストプロジェクトを作成
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();

    // main.rs
    let main_content = r#"
mod utils;
use utils::helper;

fn main() {
    helper();
}
"#;
    let main_path = src_dir.join("main.rs");
    fs::write(&main_path, main_content).unwrap();

    // utils.rs
    let utils_content = r#"
pub fn helper() {
    println!("Helper function");
}

fn unused_internal() {
    println!("This is not used");
}
"#;
    let utils_path = src_dir.join("utils.rs");
    fs::write(&utils_path, utils_content).unwrap();

    // ファイルをインデックスに追加
    let main_symbols = vec![Symbol {
        id: format!("{}:main", main_path.display()),
        name: "main".to_string(),
        kind: SymbolKind::Function,
        file_path: main_path.to_string_lossy().to_string(),
        range: Range {
            start: Position {
                line: 4,
                character: 0,
            },
            end: Position {
                line: 6,
                character: 1,
            },
        },
        documentation: None,
    }];

    let utils_symbols = vec![
        Symbol {
            id: format!("{}:helper", utils_path.display()),
            name: "pub helper".to_string(),
            kind: SymbolKind::Function,
            file_path: utils_path.to_string_lossy().to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 3,
                    character: 1,
                },
            },
            documentation: None,
        },
        Symbol {
            id: format!("{}:unused_internal", utils_path.display()),
            name: "unused_internal".to_string(),
            kind: SymbolKind::Function,
            file_path: utils_path.to_string_lossy().to_string(),
            range: Range {
                start: Position {
                    line: 5,
                    character: 0,
                },
                end: Position {
                    line: 7,
                    character: 1,
                },
            },
            documentation: None,
        },
    ];

    // インデックスに追加
    let main_hash = calculate_file_hash(main_content);
    let utils_hash = calculate_file_hash(utils_content);

    index
        .update_file(&main_path, main_symbols, main_hash.clone())
        .unwrap();
    index
        .update_file(&utils_path, utils_symbols, utils_hash.clone())
        .unwrap();

    // エッジを追加（main -> helper）
    let main_idx = index
        .graph
        .get_node_index(&format!("{}:main", main_path.display()))
        .unwrap();
    let helper_idx = index
        .graph
        .get_node_index(&format!("{}:helper", utils_path.display()))
        .unwrap();
    index
        .graph
        .add_edge(main_idx, helper_idx, EdgeKind::Reference);

    // デッドコード検出
    let mut result = lsif_core::UpdateResult::default();
    index.detect_dead_code(&mut result);

    // unused_internalのみがデッドコード
    assert_eq!(result.dead_symbols.len(), 1);
    assert!(result
        .dead_symbols
        .contains(&format!("{}:unused_internal", utils_path.display())));

    // ファイルを更新
    let utils_content_v2 = r#"
pub fn helper() {
    internal_helper();
}

fn internal_helper() {
    println!("Now used internally");
}

fn unused_internal() {
    println!("Still not used");
}
"#;
    fs::write(&utils_path, utils_content_v2).unwrap();

    let utils_symbols_v2 = vec![
        Symbol {
            id: format!("{}:helper", utils_path.display()),
            name: "pub helper".to_string(),
            kind: SymbolKind::Function,
            file_path: utils_path.to_string_lossy().to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 3,
                    character: 1,
                },
            },
            documentation: None,
        },
        Symbol {
            id: format!("{}:internal_helper", utils_path.display()),
            name: "internal_helper".to_string(),
            kind: SymbolKind::Function,
            file_path: utils_path.to_string_lossy().to_string(),
            range: Range {
                start: Position {
                    line: 5,
                    character: 0,
                },
                end: Position {
                    line: 7,
                    character: 1,
                },
            },
            documentation: None,
        },
        Symbol {
            id: format!("{}:unused_internal", utils_path.display()),
            name: "unused_internal".to_string(),
            kind: SymbolKind::Function,
            file_path: utils_path.to_string_lossy().to_string(),
            range: Range {
                start: Position {
                    line: 9,
                    character: 0,
                },
                end: Position {
                    line: 11,
                    character: 1,
                },
            },
            documentation: None,
        },
    ];

    let utils_hash_v2 = calculate_file_hash(utils_content_v2);

    // ハッシュの変更を確認
    assert!(index.needs_update(&utils_path, &utils_hash_v2));

    // 更新を実行
    let update_result = index
        .update_file(&utils_path, utils_symbols_v2, utils_hash_v2)
        .unwrap();

    assert_eq!(update_result.added_symbols.len(), 1); // internal_helper
    assert_eq!(update_result.updated_symbols.len(), 2); // helper, unused_internal

    // helper -> internal_helperのエッジを追加
    let helper_idx = index
        .graph
        .get_node_index(&format!("{}:helper", utils_path.display()))
        .unwrap();
    let internal_helper_idx = index
        .graph
        .get_node_index(&format!("{}:internal_helper", utils_path.display()))
        .unwrap();
    index
        .graph
        .add_edge(helper_idx, internal_helper_idx, EdgeKind::Reference);

    // 再度デッドコード検出
    let mut result2 = lsif_core::UpdateResult::default();
    index.detect_dead_code(&mut result2);

    // unused_internalのみがデッドコード
    assert_eq!(result2.dead_symbols.len(), 1);
    assert!(result2
        .dead_symbols
        .contains(&format!("{}:unused_internal", utils_path.display())));
    assert!(!result2
        .dead_symbols
        .contains(&format!("{}:internal_helper", utils_path.display())));
}
