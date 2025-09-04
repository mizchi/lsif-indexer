use lsif_core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};

fn create_test_symbol(id: &str, name: &str, kind: SymbolKind) -> Symbol {
    Symbol {
        id: id.to_string(),
        kind,
        name: name.to_string(),
        file_path: "/test/file.rs".to_string(),
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
    }
}

#[test]
fn test_new_graph() {
    let graph = CodeGraph::new();
    assert_eq!(graph.symbol_count(), 0);
}

#[test]
fn test_add_symbol() {
    let mut graph = CodeGraph::new();
    let symbol = create_test_symbol("func1", "test_function", SymbolKind::Function);

    let _node_idx = graph.add_symbol(symbol.clone());

    assert_eq!(graph.symbol_count(), 1);
    assert!(graph.find_symbol("func1").is_some());
    assert_eq!(graph.find_symbol("func1").unwrap().name, "test_function");
}

#[test]
fn test_add_multiple_symbols() {
    let mut graph = CodeGraph::new();

    let func_symbol = create_test_symbol("func1", "function1", SymbolKind::Function);
    let class_symbol = create_test_symbol("class1", "MyClass", SymbolKind::Class);
    let var_symbol = create_test_symbol("var1", "myVariable", SymbolKind::Variable);

    graph.add_symbol(func_symbol);
    graph.add_symbol(class_symbol);
    graph.add_symbol(var_symbol);

    assert_eq!(graph.symbol_count(), 3);
    assert!(graph.find_symbol("func1").is_some());
    assert!(graph.find_symbol("class1").is_some());
    assert!(graph.find_symbol("var1").is_some());
}

#[test]
fn test_find_nonexistent_symbol() {
    let graph = CodeGraph::new();
    assert!(graph.find_symbol("nonexistent").is_none());
}

#[test]
fn test_add_edge() {
    let mut graph = CodeGraph::new();

    let func = create_test_symbol("func1", "function1", SymbolKind::Function);
    let var = create_test_symbol("var1", "variable1", SymbolKind::Variable);

    let func_idx = graph.add_symbol(func);
    let var_idx = graph.add_symbol(var);

    // var1がfunc1を参照している
    graph.add_edge(var_idx, func_idx, EdgeKind::Reference);

    // func1への参照を探す
    let references = graph.find_references("func1");
    assert_eq!(references.len(), 1);
    assert_eq!(references[0].id, "var1");
}

#[test]
fn test_find_references() {
    let mut graph = CodeGraph::new();

    // targetシンボルを作成
    let target = create_test_symbol("target", "target_func", SymbolKind::Function);
    // targetを参照する3つのシンボルを作成
    let caller1 = create_test_symbol("caller1", "caller_func1", SymbolKind::Function);
    let caller2 = create_test_symbol("caller2", "caller_func2", SymbolKind::Function);
    let caller3 = create_test_symbol("caller3", "caller_func3", SymbolKind::Function);

    let target_idx = graph.add_symbol(target);
    let caller1_idx = graph.add_symbol(caller1);
    let caller2_idx = graph.add_symbol(caller2);
    let caller3_idx = graph.add_symbol(caller3);

    // 各callerがtargetを参照
    graph.add_edge(caller1_idx, target_idx, EdgeKind::Reference);
    graph.add_edge(caller2_idx, target_idx, EdgeKind::Reference);
    graph.add_edge(caller3_idx, target_idx, EdgeKind::Reference);

    // targetへの参照を探す
    let references = graph.find_references("target");
    assert_eq!(references.len(), 3);

    let ref_ids: Vec<&str> = references.iter().map(|s| s.id.as_str()).collect();
    assert!(ref_ids.contains(&"caller1"));
    assert!(ref_ids.contains(&"caller2"));
    assert!(ref_ids.contains(&"caller3"));
}

#[test]
fn test_find_references_nonexistent_symbol() {
    let graph = CodeGraph::new();
    let references = graph.find_references("nonexistent");
    assert_eq!(references.len(), 0);
}

#[test]
fn test_find_definition() {
    let mut graph = CodeGraph::new();

    let def = create_test_symbol("def1", "MyFunction", SymbolKind::Function);
    let ref1 = create_test_symbol("ref1", "call1", SymbolKind::Variable);

    let def_idx = graph.add_symbol(def);
    let ref_idx = graph.add_symbol(ref1);

    graph.add_edge(def_idx, ref_idx, EdgeKind::Definition);

    let found_def = graph.find_definition("ref1");
    assert!(found_def.is_some());
    assert_eq!(found_def.unwrap().id, "def1");
}

#[test]
fn test_find_definition_with_multiple_edges() {
    let mut graph = CodeGraph::new();

    let def = create_test_symbol("def1", "MyFunction", SymbolKind::Function);
    let ref1 = create_test_symbol("ref1", "call1", SymbolKind::Variable);
    let other = create_test_symbol("other1", "other", SymbolKind::Variable);

    let def_idx = graph.add_symbol(def);
    let ref_idx = graph.add_symbol(ref1);
    let other_idx = graph.add_symbol(other);

    graph.add_edge(def_idx, ref_idx, EdgeKind::Definition);
    graph.add_edge(other_idx, ref_idx, EdgeKind::Reference);

    let found_def = graph.find_definition("ref1");
    assert!(found_def.is_some());
    assert_eq!(found_def.unwrap().id, "def1");
}

#[test]
fn test_find_definition_no_definition_edge() {
    let mut graph = CodeGraph::new();

    let symbol1 = create_test_symbol("sym1", "symbol1", SymbolKind::Function);
    let symbol2 = create_test_symbol("sym2", "symbol2", SymbolKind::Variable);

    let idx1 = graph.add_symbol(symbol1);
    let idx2 = graph.add_symbol(symbol2);

    graph.add_edge(idx1, idx2, EdgeKind::Reference);

    let found_def = graph.find_definition("sym2");
    assert!(found_def.is_none());
}

#[test]
fn test_get_node_index() {
    let mut graph = CodeGraph::new();

    let symbol = create_test_symbol("sym1", "symbol1", SymbolKind::Function);
    let idx = graph.add_symbol(symbol);

    let retrieved_idx = graph.get_node_index("sym1");
    assert!(retrieved_idx.is_some());
    assert_eq!(retrieved_idx.unwrap(), idx);
}

#[test]
fn test_get_node_index_nonexistent() {
    let graph = CodeGraph::new();
    assert!(graph.get_node_index("nonexistent").is_none());
}

#[test]
fn test_get_all_symbols() {
    let mut graph = CodeGraph::new();

    let func = create_test_symbol("func1", "function1", SymbolKind::Function);
    let class = create_test_symbol("class1", "MyClass", SymbolKind::Class);
    let var = create_test_symbol("var1", "myVariable", SymbolKind::Variable);

    graph.add_symbol(func);
    graph.add_symbol(class);
    graph.add_symbol(var);

    let all_symbols: Vec<&Symbol> = graph.get_all_symbols().collect();
    assert_eq!(all_symbols.len(), 3);

    let symbol_ids: Vec<&str> = all_symbols.iter().map(|s| s.id.as_str()).collect();
    assert!(symbol_ids.contains(&"func1"));
    assert!(symbol_ids.contains(&"class1"));
    assert!(symbol_ids.contains(&"var1"));
}

#[test]
fn test_empty_graph_get_all_symbols() {
    let graph = CodeGraph::new();
    let all_symbols: Vec<&Symbol> = graph.get_all_symbols().collect();
    assert_eq!(all_symbols.len(), 0);
}

#[test]
fn test_complex_graph_scenario() {
    let mut graph = CodeGraph::new();

    let main_func = create_test_symbol("main", "main", SymbolKind::Function);
    let helper_func = create_test_symbol("helper", "helper_function", SymbolKind::Function);
    let class = create_test_symbol("MyClass", "MyClass", SymbolKind::Class);
    let method = create_test_symbol("MyClass::method", "method", SymbolKind::Method);
    let var = create_test_symbol("global_var", "global_variable", SymbolKind::Variable);

    let main_idx = graph.add_symbol(main_func);
    let helper_idx = graph.add_symbol(helper_func);
    let class_idx = graph.add_symbol(class);
    let method_idx = graph.add_symbol(method);
    let var_idx = graph.add_symbol(var);

    // mainはhelperとvarを参照
    graph.add_edge(main_idx, helper_idx, EdgeKind::Reference);
    graph.add_edge(main_idx, var_idx, EdgeKind::Reference);
    // classはmethodを含む
    graph.add_edge(class_idx, method_idx, EdgeKind::Contains);
    // helperはmethodを参照
    graph.add_edge(helper_idx, method_idx, EdgeKind::Reference);
    // varはmainの定義
    graph.add_edge(var_idx, main_idx, EdgeKind::Definition);

    assert_eq!(graph.symbol_count(), 5);

    // helperへの参照を探す（mainから参照されている）
    let helper_refs = graph.find_references("helper");
    assert_eq!(helper_refs.len(), 1);
    assert_eq!(helper_refs[0].id, "main");

    // varへの参照を探す（mainから参照されている）
    let var_refs = graph.find_references("global_var");
    assert_eq!(var_refs.len(), 1);
    assert_eq!(var_refs[0].id, "main");

    // methodへの参照を探す（helperから参照され、classに含まれる）
    let method_refs = graph.find_references("MyClass::method");
    // Referenceエッジのみをカウント（Containsは含まない）
    assert_eq!(method_refs.len(), 1);
    assert_eq!(method_refs[0].id, "helper");

    // mainの定義を探す
    let var_def = graph.find_definition("main");
    assert!(var_def.is_some());
    assert_eq!(var_def.unwrap().id, "global_var");
}

#[test]
fn test_symbol_with_documentation() {
    let mut graph = CodeGraph::new();

    let mut symbol = create_test_symbol("doc_func", "documented_function", SymbolKind::Function);
    symbol.documentation = Some("This is a documented function".to_string());

    graph.add_symbol(symbol);

    let found = graph.find_symbol("doc_func");
    assert!(found.is_some());
    assert_eq!(
        found.unwrap().documentation,
        Some("This is a documented function".to_string())
    );
}

#[test]
fn test_different_edge_kinds() {
    let mut graph = CodeGraph::new();

    let interface = create_test_symbol("IFoo", "IFoo", SymbolKind::Interface);
    let class = create_test_symbol("Foo", "Foo", SymbolKind::Class);
    let base_class = create_test_symbol("Base", "Base", SymbolKind::Class);
    let override_method = create_test_symbol("Foo::method", "method", SymbolKind::Method);
    let base_method = create_test_symbol("Base::method", "method", SymbolKind::Method);

    let interface_idx = graph.add_symbol(interface);
    let class_idx = graph.add_symbol(class);
    let base_idx = graph.add_symbol(base_class);
    let override_idx = graph.add_symbol(override_method);
    let base_method_idx = graph.add_symbol(base_method);

    graph.add_edge(class_idx, interface_idx, EdgeKind::Implementation);
    graph.add_edge(class_idx, base_idx, EdgeKind::TypeDefinition);
    graph.add_edge(override_idx, base_method_idx, EdgeKind::Override);

    assert_eq!(graph.symbol_count(), 5);
}
