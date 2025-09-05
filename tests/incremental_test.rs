use lsif_core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
use lsif_core::{IncrementalIndex};
use lsif_core::incremental::{calculate_file_hash, FileUpdate, UpdateResult};
use std::path::{Path, PathBuf};

fn create_test_symbol(id: &str, name: &str, kind: SymbolKind, file_path: &str) -> Symbol {
    Symbol {
        id: id.to_string(),
        kind,
        name: name.to_string(),
        file_path: file_path.to_string(),
        range: Range {
            start: Position {
                line: 10,
                character: 5,
            },
            end: Position {
                line: 10,
                character: 15,
            },
        },
        documentation: None,
        detail: None,
    }
}

fn create_public_symbol(id: &str, file_path: &str) -> Symbol {
    Symbol {
        id: id.to_string(),
        kind: SymbolKind::Function,
        name: format!("pub {id}"),
        file_path: file_path.to_string(),
        range: Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 10,
            },
        },
        documentation: None,
        detail: None,
    }
}

#[test]
fn test_new_incremental_index() {
    let index = IncrementalIndex::new();
    assert_eq!(index.graph.symbol_count(), 0);
    assert!(index.file_metadata.is_empty());
    assert!(index.symbol_to_file.is_empty());
    assert!(index.dead_symbols.is_empty());
}

#[test]
fn test_from_existing_graph() {
    let mut graph = CodeGraph::new();

    let symbol1 = create_test_symbol("func1", "function1", SymbolKind::Function, "/src/lib.rs");
    let symbol2 = create_test_symbol("var1", "variable1", SymbolKind::Variable, "/src/main.rs");

    graph.add_symbol(symbol1);
    graph.add_symbol(symbol2);

    let index = IncrementalIndex::from_graph(graph);

    assert_eq!(index.graph.symbol_count(), 2);
    assert_eq!(index.symbol_to_file.len(), 2);
    assert_eq!(index.file_metadata.len(), 2);
    assert!(index.symbol_to_file.contains_key("func1"));
    assert!(index.symbol_to_file.contains_key("var1"));
}

#[test]
fn test_update_file_add_symbols() {
    let mut index = IncrementalIndex::new();

    let symbols = vec![
        create_test_symbol("func1", "function1", SymbolKind::Function, "/src/lib.rs"),
        create_test_symbol("func2", "function2", SymbolKind::Function, "/src/lib.rs"),
    ];

    let result = index
        .update_file(Path::new("/src/lib.rs"), symbols, "hash123".to_string())
        .unwrap();

    assert_eq!(result.added_symbols.len(), 2);
    assert_eq!(result.removed_symbols.len(), 0);
    assert_eq!(result.updated_symbols.len(), 0);
    assert!(result.added_symbols.contains("func1"));
    assert!(result.added_symbols.contains("func2"));

    assert_eq!(index.graph.symbol_count(), 2);
}

#[test]
fn test_update_file_modify_symbols() {
    let mut index = IncrementalIndex::new();

    let initial_symbols = vec![
        create_test_symbol("func1", "function1", SymbolKind::Function, "/src/lib.rs"),
        create_test_symbol("func2", "function2", SymbolKind::Function, "/src/lib.rs"),
    ];

    index
        .update_file(
            Path::new("/src/lib.rs"),
            initial_symbols,
            "hash123".to_string(),
        )
        .unwrap();

    let updated_symbols = vec![
        create_test_symbol(
            "func1",
            "function1_modified",
            SymbolKind::Function,
            "/src/lib.rs",
        ),
        create_test_symbol("func3", "function3", SymbolKind::Function, "/src/lib.rs"),
    ];

    let result = index
        .update_file(
            Path::new("/src/lib.rs"),
            updated_symbols,
            "hash456".to_string(),
        )
        .unwrap();

    assert_eq!(result.added_symbols.len(), 1);
    assert_eq!(result.removed_symbols.len(), 1);
    assert_eq!(result.updated_symbols.len(), 1);
    assert!(result.added_symbols.contains("func3"));
    assert!(result.removed_symbols.contains("func2"));
    assert!(result.updated_symbols.contains("func1"));
}

#[test]
fn test_remove_file() {
    let mut index = IncrementalIndex::new();

    let symbols = vec![
        create_test_symbol("func1", "function1", SymbolKind::Function, "/src/lib.rs"),
        create_test_symbol("func2", "function2", SymbolKind::Function, "/src/lib.rs"),
    ];

    index
        .update_file(Path::new("/src/lib.rs"), symbols, "hash123".to_string())
        .unwrap();

    let result = index.remove_file(Path::new("/src/lib.rs")).unwrap();

    assert_eq!(result.removed_symbols.len(), 2);
    assert!(result.removed_symbols.contains("func1"));
    assert!(result.removed_symbols.contains("func2"));
    assert_eq!(index.graph.symbol_count(), 0);
    assert!(!index
        .file_metadata
        .contains_key(&PathBuf::from("/src/lib.rs")));
}

#[test]
fn test_needs_update() {
    let mut index = IncrementalIndex::new();

    let symbols = vec![create_test_symbol(
        "func1",
        "function1",
        SymbolKind::Function,
        "/src/lib.rs",
    )];

    index
        .update_file(Path::new("/src/lib.rs"), symbols, "hash123".to_string())
        .unwrap();

    assert!(!index.needs_update(Path::new("/src/lib.rs"), "hash123"));
    assert!(index.needs_update(Path::new("/src/lib.rs"), "hash456"));
    assert!(index.needs_update(Path::new("/src/main.rs"), "anyhash"));
}

#[test]
fn test_dead_code_detection_simple() {
    let mut index = IncrementalIndex::new();

    let main_symbol = create_test_symbol("main", "main", SymbolKind::Function, "/src/main.rs");
    let used_func = create_test_symbol(
        "used_func",
        "used_function",
        SymbolKind::Function,
        "/src/lib.rs",
    );
    let unused_func = create_test_symbol(
        "unused_func",
        "unused_function",
        SymbolKind::Function,
        "/src/lib.rs",
    );

    let main_idx = index.graph.add_symbol(main_symbol);
    let used_idx = index.graph.add_symbol(used_func);
    let _unused_idx = index.graph.add_symbol(unused_func);

    index
        .graph
        .add_edge(main_idx, used_idx, EdgeKind::Reference);

    let mut result = UpdateResult::default();
    index.detect_dead_code(&mut result);

    assert!(result.dead_symbols.contains("unused_func"));
    assert!(!result.dead_symbols.contains("main"));
    assert!(!result.dead_symbols.contains("used_func"));
}

#[test]
fn test_dead_code_detection_with_public_api() {
    let mut index = IncrementalIndex::new();

    let pub_func = create_public_symbol("public_func", "/src/lib.rs");
    let internal_func = create_test_symbol(
        "internal_func",
        "internal",
        SymbolKind::Function,
        "/src/lib.rs",
    );
    let unused_func =
        create_test_symbol("unused_func", "unused", SymbolKind::Function, "/src/lib.rs");

    let pub_idx = index.graph.add_symbol(pub_func);
    let internal_idx = index.graph.add_symbol(internal_func);
    let _unused_idx = index.graph.add_symbol(unused_func);

    index
        .graph
        .add_edge(pub_idx, internal_idx, EdgeKind::Reference);

    let mut result = UpdateResult::default();
    index.detect_dead_code(&mut result);

    assert!(!result.dead_symbols.contains("public_func"));
    assert!(!result.dead_symbols.contains("internal_func"));
    assert!(result.dead_symbols.contains("unused_func"));
}

#[test]
fn test_batch_update() {
    let mut index = IncrementalIndex::new();

    let updates = vec![
        FileUpdate::Added {
            path: PathBuf::from("/src/file1.rs"),
            symbols: vec![
                create_test_symbol("func1", "function1", SymbolKind::Function, "/src/file1.rs"),
                create_test_symbol("func2", "function2", SymbolKind::Function, "/src/file1.rs"),
            ],
            hash: "hash1".to_string(),
        },
        FileUpdate::Added {
            path: PathBuf::from("/src/file2.rs"),
            symbols: vec![create_test_symbol(
                "func3",
                "function3",
                SymbolKind::Function,
                "/src/file2.rs",
            )],
            hash: "hash2".to_string(),
        },
        FileUpdate::Modified {
            path: PathBuf::from("/src/file1.rs"),
            symbols: vec![create_test_symbol(
                "func1",
                "function1_modified",
                SymbolKind::Function,
                "/src/file1.rs",
            )],
            hash: "hash1_modified".to_string(),
        },
    ];

    let result = index.batch_update(updates).unwrap();

    assert_eq!(result.total_added, 3);
    assert_eq!(result.total_removed, 1);
    assert_eq!(result.total_updated, 1);
    assert_eq!(result.affected_files, 3);
}

#[test]
fn test_calculate_file_hash() {
    let content1 = "fn main() { println!(\"Hello\"); }";
    let content2 = "fn main() { println!(\"World\"); }";

    let hash1 = calculate_file_hash(content1);
    let hash2 = calculate_file_hash(content2);

    assert_ne!(hash1, hash2);
    assert_eq!(hash1, calculate_file_hash(content1));
}

#[test]
fn test_get_dead_symbols() {
    let mut index = IncrementalIndex::new();

    let unused_func = create_test_symbol("unused", "unused", SymbolKind::Function, "/src/lib.rs");
    index.graph.add_symbol(unused_func);

    let mut result = UpdateResult::default();
    index.detect_dead_code(&mut result);

    let dead_symbols = index.get_dead_symbols();
    assert!(dead_symbols.contains("unused"));
}

#[test]
fn test_transitive_dead_code_detection() {
    let mut index = IncrementalIndex::new();

    let main_symbol = create_test_symbol("main", "main", SymbolKind::Function, "/src/main.rs");
    let func_a = create_test_symbol("func_a", "function_a", SymbolKind::Function, "/src/lib.rs");
    let func_b = create_test_symbol("func_b", "function_b", SymbolKind::Function, "/src/lib.rs");
    let func_c = create_test_symbol("func_c", "function_c", SymbolKind::Function, "/src/lib.rs");
    let unused = create_test_symbol("unused", "unused", SymbolKind::Function, "/src/lib.rs");

    let main_idx = index.graph.add_symbol(main_symbol);
    let a_idx = index.graph.add_symbol(func_a);
    let b_idx = index.graph.add_symbol(func_b);
    let c_idx = index.graph.add_symbol(func_c);
    let _unused_idx = index.graph.add_symbol(unused);

    index.graph.add_edge(main_idx, a_idx, EdgeKind::Reference);
    index.graph.add_edge(a_idx, b_idx, EdgeKind::Reference);
    index.graph.add_edge(b_idx, c_idx, EdgeKind::Reference);

    let mut result = UpdateResult::default();
    index.detect_dead_code(&mut result);

    assert!(!result.dead_symbols.contains("main"));
    assert!(!result.dead_symbols.contains("func_a"));
    assert!(!result.dead_symbols.contains("func_b"));
    assert!(!result.dead_symbols.contains("func_c"));
    assert!(result.dead_symbols.contains("unused"));
}

#[test]
fn test_file_metadata() {
    let mut index = IncrementalIndex::new();

    let symbols = vec![create_test_symbol(
        "func1",
        "function1",
        SymbolKind::Function,
        "/src/lib.rs",
    )];

    index
        .update_file(Path::new("/src/lib.rs"), symbols, "hash123".to_string())
        .unwrap();

    let metadata = index
        .file_metadata
        .get(&PathBuf::from("/src/lib.rs"))
        .unwrap();
    assert_eq!(metadata.path, PathBuf::from("/src/lib.rs"));
    assert_eq!(metadata.hash, "hash123");
    assert!(metadata.symbols.contains("func1"));
}
