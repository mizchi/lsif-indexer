use lsif_core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
use lsif_lsif_core::{generate_lsif, parse_lsif, write_lsif, LsifParser};
use std::io::Cursor;

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
fn test_generate_empty_lsif() {
    let graph = CodeGraph::new();
    let result = generate_lsif(graph);
    assert!(result.is_ok());

    let lsif = result.unwrap();
    assert!(lsif.contains("\"label\":\"metaData\""));
    assert!(lsif.contains("\"label\":\"project\""));
}

#[test]
fn test_generate_lsif_with_symbols() {
    let mut graph = CodeGraph::new();

    let func1 = create_test_symbol("func1", "my_function", SymbolKind::Function, "/src/main.rs");
    let var1 = create_test_symbol("var1", "my_variable", SymbolKind::Variable, "/src/lib.rs");

    graph.add_symbol(func1);
    graph.add_symbol(var1);

    let result = generate_lsif(graph);
    assert!(result.is_ok());

    let lsif = result.unwrap();
    assert!(lsif.contains("\"label\":\"document\""));
    assert!(lsif.contains("\"uri\":\"file:///src/main.rs\""));
    assert!(lsif.contains("\"uri\":\"file:///src/lib.rs\""));
    assert!(lsif.contains("\"label\":\"range\""));
}

#[test]
fn test_generate_lsif_with_documentation() {
    let mut graph = CodeGraph::new();

    let mut func1 = create_test_symbol(
        "func1",
        "documented_function",
        SymbolKind::Function,
        "/src/doc.rs",
    );
    func1.documentation = Some("This is a test function".to_string());

    graph.add_symbol(func1);

    let result = generate_lsif(graph);
    assert!(result.is_ok());

    let lsif = result.unwrap();
    assert!(lsif.contains("\"label\":\"hoverResult\""));
    assert!(lsif.contains("This is a test function"));
}

#[test]
fn test_lsif_element_structure() {
    let mut graph = CodeGraph::new();
    let func = create_test_symbol("test_func", "test", SymbolKind::Function, "/test.rs");
    graph.add_symbol(func);

    let result = generate_lsif(graph);
    assert!(result.is_ok());

    let lsif = result.unwrap();
    let lines: Vec<&str> = lsif.lines().collect();
    assert!(!lines.is_empty());

    for line in lines {
        if !line.is_empty() {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
            assert!(parsed.is_ok(), "Invalid JSON: {line}");

            let value = parsed.unwrap();
            assert!(value.get("id").is_some());
            assert!(value.get("type").is_some());
            assert!(value.get("label").is_some());
        }
    }
}

#[test]
fn test_lsif_metadata_format() {
    let graph = CodeGraph::new();
    let result = generate_lsif(graph);
    assert!(result.is_ok());

    let lsif = result.unwrap();
    let lines: Vec<&str> = lsif.lines().collect();

    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(lines[0]) {
        assert_eq!(meta["label"], "metaData");
        assert_eq!(meta["version"], "0.5.0");
        assert!(meta["toolInfo"].is_object());
    }
}

#[test]
fn test_parse_empty_lsif() {
    let content = "";
    let result = parse_lsif(content);
    assert!(result.is_ok());

    let graph = result.unwrap();
    assert_eq!(graph.symbol_count(), 0);
}

#[test]
fn test_parse_lsif_with_document() {
    let content = r#"
{"id":"1","type":"vertex","label":"metaData","version":"0.5.0"}
{"id":"2","type":"vertex","label":"project","kind":"rust"}
{"id":"3","type":"vertex","label":"document","uri":"file:///src/main.rs","languageId":"rust"}
{"id":"4","type":"edge","label":"contains","outV":"2","inV":"3"}
"#;

    let result = parse_lsif(content);
    assert!(result.is_ok());
}

#[test]
fn test_parse_lsif_with_range() {
    let content = r#"
{"id":"1","type":"vertex","label":"document","uri":"file:///src/main.rs"}
{"id":"2","type":"vertex","label":"range","start":{"line":0,"character":0},"end":{"line":0,"character":10}}
{"id":"3","type":"edge","label":"contains","outV":"1","inV":"2"}
"#;

    let result = parse_lsif(content);
    assert!(result.is_ok());
}

#[test]
fn test_write_lsif() {
    let mut graph = CodeGraph::new();
    let symbol = create_test_symbol(
        "write_test",
        "test_write",
        SymbolKind::Function,
        "/write.rs",
    );
    graph.add_symbol(symbol);

    let mut buffer = Cursor::new(Vec::new());
    let result = write_lsif(&mut buffer, graph);
    assert!(result.is_ok());

    let written_content = String::from_utf8(buffer.into_inner()).unwrap();
    assert!(!written_content.is_empty());
    assert!(written_content.contains("metaData"));
    assert!(written_content.contains("project"));
}

#[test]
fn test_lsif_round_trip() {
    let mut original_graph = CodeGraph::new();

    let func1 = create_test_symbol("func1", "function_one", SymbolKind::Function, "/src/one.rs");
    let func2 = create_test_symbol("func2", "function_two", SymbolKind::Function, "/src/two.rs");

    original_graph.add_symbol(func1);
    original_graph.add_symbol(func2);

    let lsif_content = generate_lsif(original_graph).unwrap();

    let parsed_graph = parse_lsif(&lsif_content);
    assert!(parsed_graph.is_ok());
}

#[test]
fn test_lsif_parser_default() {
    let parser = LsifParser::default();
    let graph = parser.into_graph();
    assert_eq!(graph.symbol_count(), 0);
}

#[test]
fn test_parse_invalid_json() {
    let content = "invalid json content";
    let result = parse_lsif(content);
    assert!(result.is_err());
}

#[test]
fn test_multiple_documents_in_lsif() {
    let mut graph = CodeGraph::new();

    let symbols = vec![
        create_test_symbol("s1", "symbol1", SymbolKind::Function, "/src/file1.rs"),
        create_test_symbol("s2", "symbol2", SymbolKind::Class, "/src/file2.rs"),
        create_test_symbol("s3", "symbol3", SymbolKind::Variable, "/src/file1.rs"),
        create_test_symbol("s4", "symbol4", SymbolKind::Method, "/src/file3.rs"),
    ];

    for symbol in symbols {
        graph.add_symbol(symbol);
    }

    let result = generate_lsif(graph);
    assert!(result.is_ok());

    let lsif = result.unwrap();
    assert!(lsif.contains("file:///src/file1.rs"));
    assert!(lsif.contains("file:///src/file2.rs"));
    assert!(lsif.contains("file:///src/file3.rs"));
}

#[test]
fn test_lsif_with_edges() {
    let mut graph = CodeGraph::new();

    let def = create_test_symbol("def1", "definition", SymbolKind::Function, "/src/def.rs");
    let ref1 = create_test_symbol("ref1", "reference", SymbolKind::Variable, "/src/ref.rs");

    let def_idx = graph.add_symbol(def);
    let ref_idx = graph.add_symbol(ref1);

    graph.add_edge(def_idx, ref_idx, EdgeKind::Definition);

    let result = generate_lsif(graph);
    assert!(result.is_ok());

    let lsif = result.unwrap();
    assert!(lsif.contains("\"type\":\"edge\""));
}
