use lsif_core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};

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
    }
}

#[test]
fn test_serialize_empty_graph_json() {
    let graph = CodeGraph::new();
    let json = serde_json::to_string(&graph).unwrap();
    assert!(json.contains("\"symbols\":[]"));
    assert!(json.contains("\"edges\":[]"));
}

#[test]
fn test_deserialize_empty_graph_json() {
    let json = r#"{"symbols":[],"edges":[]}"#;
    let graph: CodeGraph = serde_json::from_str(json).unwrap();
    assert_eq!(graph.symbol_count(), 0);
}

#[test]
fn test_serialize_graph_with_symbols_json() {
    let mut graph = CodeGraph::new();

    let func1 = create_test_symbol("func1", "function1", SymbolKind::Function, "/src/lib.rs");
    let var1 = create_test_symbol("var1", "variable1", SymbolKind::Variable, "/src/main.rs");

    graph.add_symbol(func1);
    graph.add_symbol(var1);

    let json = serde_json::to_string(&graph).unwrap();
    assert!(json.contains("\"id\":\"func1\""));
    assert!(json.contains("\"id\":\"var1\""));
    assert!(json.contains("\"name\":\"function1\""));
    assert!(json.contains("\"name\":\"variable1\""));
}

#[test]
fn test_serialize_graph_with_edges_json() {
    let mut graph = CodeGraph::new();

    let func1 = create_test_symbol("func1", "function1", SymbolKind::Function, "/src/lib.rs");
    let var1 = create_test_symbol("var1", "variable1", SymbolKind::Variable, "/src/main.rs");

    let idx1 = graph.add_symbol(func1);
    let idx2 = graph.add_symbol(var1);

    graph.add_edge(idx1, idx2, EdgeKind::Reference);

    let json = serde_json::to_string(&graph).unwrap();
    assert!(json.contains("\"from_id\":\"func1\""));
    assert!(json.contains("\"to_id\":\"var1\""));
    assert!(json.contains("\"kind\":\"Reference\""));
}

#[test]
fn test_round_trip_json() {
    let mut original = CodeGraph::new();

    let func1 = create_test_symbol("func1", "function1", SymbolKind::Function, "/src/lib.rs");
    let func2 = create_test_symbol("func2", "function2", SymbolKind::Function, "/src/lib.rs");
    let var1 = create_test_symbol("var1", "variable1", SymbolKind::Variable, "/src/main.rs");

    let idx1 = original.add_symbol(func1);
    let idx2 = original.add_symbol(func2);
    let idx3 = original.add_symbol(var1);

    original.add_edge(idx1, idx2, EdgeKind::Reference);
    original.add_edge(idx2, idx3, EdgeKind::Definition);

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: CodeGraph = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.symbol_count(), 3);
    assert!(deserialized.find_symbol("func1").is_some());
    assert!(deserialized.find_symbol("func2").is_some());
    assert!(deserialized.find_symbol("var1").is_some());
}

#[test]
fn test_serialize_empty_graph_bincode() {
    let graph = CodeGraph::new();
    let bytes = bincode::serialize(&graph).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn test_deserialize_empty_graph_bincode() {
    let graph = CodeGraph::new();
    let bytes = bincode::serialize(&graph).unwrap();
    let deserialized: CodeGraph = bincode::deserialize(&bytes).unwrap();
    assert_eq!(deserialized.symbol_count(), 0);
}

#[test]
fn test_round_trip_bincode() {
    let mut original = CodeGraph::new();

    let symbols = vec![
        create_test_symbol("s1", "symbol1", SymbolKind::Function, "/file1.rs"),
        create_test_symbol("s2", "symbol2", SymbolKind::Class, "/file2.rs"),
        create_test_symbol("s3", "symbol3", SymbolKind::Variable, "/file3.rs"),
    ];

    let indices: Vec<_> = symbols
        .into_iter()
        .map(|s| original.add_symbol(s))
        .collect();

    original.add_edge(indices[0], indices[1], EdgeKind::TypeDefinition);
    original.add_edge(indices[1], indices[2], EdgeKind::Contains);

    let bytes = bincode::serialize(&original).unwrap();
    let deserialized: CodeGraph = bincode::deserialize(&bytes).unwrap();

    assert_eq!(deserialized.symbol_count(), 3);
    assert!(deserialized.find_symbol("s1").is_some());
    assert!(deserialized.find_symbol("s2").is_some());
    assert!(deserialized.find_symbol("s3").is_some());
}

#[test]
fn test_complex_graph_serialization() {
    let mut graph = CodeGraph::new();

    let interface = create_test_symbol("IFoo", "IFoo", SymbolKind::Interface, "/interfaces.rs");
    let class = create_test_symbol("Foo", "Foo", SymbolKind::Class, "/classes.rs");
    let method1 = create_test_symbol("Foo::method1", "method1", SymbolKind::Method, "/classes.rs");
    let method2 = create_test_symbol("Foo::method2", "method2", SymbolKind::Method, "/classes.rs");
    let field = create_test_symbol("Foo::field", "field", SymbolKind::Field, "/classes.rs");

    let iface_idx = graph.add_symbol(interface);
    let class_idx = graph.add_symbol(class);
    let m1_idx = graph.add_symbol(method1);
    let m2_idx = graph.add_symbol(method2);
    let field_idx = graph.add_symbol(field);

    graph.add_edge(class_idx, iface_idx, EdgeKind::Implementation);
    graph.add_edge(class_idx, m1_idx, EdgeKind::Contains);
    graph.add_edge(class_idx, m2_idx, EdgeKind::Contains);
    graph.add_edge(class_idx, field_idx, EdgeKind::Contains);
    graph.add_edge(m1_idx, m2_idx, EdgeKind::Reference);

    let json = serde_json::to_string_pretty(&graph).unwrap();
    let deserialized: CodeGraph = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.symbol_count(), 5);

    // Fooに含まれる要素を確認（Contains関係はfind_referencesでは取得できない）
    // エッジが正しくデシリアライズされているか確認
    let iface_refs = deserialized.find_references("IFoo");
    assert_eq!(iface_refs.len(), 0, "IFooへのReferenceエッジはない");

    let m2_refs = deserialized.find_references("Foo::method2");
    assert_eq!(m2_refs.len(), 1, "method2はmethod1から参照されている");
    assert_eq!(m2_refs[0].id, "Foo::method1");
}

#[test]
fn test_preserve_documentation() {
    let mut graph = CodeGraph::new();

    let mut symbol = create_test_symbol("doc_func", "documented", SymbolKind::Function, "/doc.rs");
    symbol.documentation = Some("This is important documentation".to_string());

    graph.add_symbol(symbol);

    let json = serde_json::to_string(&graph).unwrap();
    let deserialized: CodeGraph = serde_json::from_str(&json).unwrap();

    let found = deserialized.find_symbol("doc_func").unwrap();
    assert_eq!(
        found.documentation,
        Some("This is important documentation".to_string())
    );
}

#[test]
fn test_all_edge_kinds() {
    let mut graph = CodeGraph::new();

    let symbols: Vec<_> = (0..9)
        .map(|i| {
            create_test_symbol(
                &format!("s{i}"),
                &format!("symbol{i}"),
                SymbolKind::Function,
                "/test.rs",
            )
        })
        .map(|s| graph.add_symbol(s))
        .collect();

    let edge_kinds = vec![
        EdgeKind::Definition,
        EdgeKind::Reference,
        EdgeKind::TypeDefinition,
        EdgeKind::Implementation,
        EdgeKind::Override,
        EdgeKind::Import,
        EdgeKind::Export,
        EdgeKind::Contains,
    ];

    for (i, kind) in edge_kinds.iter().enumerate() {
        graph.add_edge(symbols[i], symbols[i + 1], kind.clone());
    }

    let json = serde_json::to_string(&graph).unwrap();
    let deserialized: CodeGraph = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.symbol_count(), 9);

    for kind in edge_kinds {
        let kind_str = format!("{kind:?}");
        assert!(json.contains(&kind_str));
    }
}

#[test]
fn test_large_graph_performance() {
    let mut graph = CodeGraph::new();

    let symbols: Vec<_> = (0..100)
        .map(|i| {
            create_test_symbol(
                &format!("sym{i}"),
                &format!("symbol{i}"),
                SymbolKind::Function,
                "/large.rs",
            )
        })
        .map(|s| graph.add_symbol(s))
        .collect();

    for i in 0..99 {
        graph.add_edge(symbols[i], symbols[i + 1], EdgeKind::Reference);
    }

    let start = std::time::Instant::now();
    let json = serde_json::to_string(&graph).unwrap();
    let serialize_time = start.elapsed();

    let start = std::time::Instant::now();
    let _deserialized: CodeGraph = serde_json::from_str(&json).unwrap();
    let deserialize_time = start.elapsed();

    println!("Serialize time: {serialize_time:?}, Deserialize time: {deserialize_time:?}");
    assert!(serialize_time.as_millis() < 100);
    assert!(deserialize_time.as_millis() < 100);
}

#[test]
fn test_pretty_json_format() {
    let mut graph = CodeGraph::new();

    let func = create_test_symbol(
        "test_func",
        "test_function",
        SymbolKind::Function,
        "/test.rs",
    );
    graph.add_symbol(func);

    let pretty_json = serde_json::to_string_pretty(&graph).unwrap();
    assert!(pretty_json.contains("\n"));
    assert!(pretty_json.contains("  "));
}
