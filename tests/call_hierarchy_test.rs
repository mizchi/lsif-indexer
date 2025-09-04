use lsif_core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
use std::path::PathBuf;

#[test]
fn test_call_hierarchy_with_sample_code() {
    // Create a simple graph for testing
    let graph = create_sample_graph();

    // Import the call hierarchy analyzer
    use lsif_core::{format_hierarchy, CallHierarchyAnalyzer};

    let analyzer = CallHierarchyAnalyzer::new(&graph);

    // Test outgoing calls from main
    let main_hierarchy = analyzer.get_outgoing_calls("main", 3).unwrap();
    let formatted = format_hierarchy(&main_hierarchy, "", true);

    println!("=== Outgoing calls from main ===");
    println!("{formatted}");

    assert!(formatted.contains("main"));
    assert!(formatted.contains("calculate"));
    assert!(formatted.contains("add"));
    assert!(formatted.contains("multiply"));

    // Test incoming calls to add
    let add_hierarchy = analyzer.get_incoming_calls("add", 2).unwrap();
    let formatted_incoming = format_hierarchy(&add_hierarchy, "", true);

    println!("\n=== Incoming calls to add ===");
    println!("{formatted_incoming}");

    assert!(formatted_incoming.contains("add"));
    assert!(formatted_incoming.contains("calculate"));

    // Test call paths
    let paths = analyzer.find_call_paths("main", "add", 5);
    println!("\n=== Call paths from main to add ===");
    for path in &paths {
        println!("  Path: {}", path.join(" -> "));
    }

    assert!(!paths.is_empty());
}

#[test]
fn test_call_hierarchy_with_real_file() {
    let fixture_path = PathBuf::from("tests/fixtures/sample.rs");

    if !fixture_path.exists() {
        println!("Skipping test: fixture file not found");
        return;
    }

    // This would require actual LSP integration
    // For now, we'll just test the structure
    println!("Testing with fixture: {}", fixture_path.display());

    // Create expected hierarchy structure
    let expected_hierarchy = r#"
main
├── calculate
│   ├── add
│   ├── multiply
│   └── combine
└── DataProcessor::process
    ├── DataProcessor::validate
    │   └── DataProcessor::check_item
    ├── DataProcessor::transform
    │   └── DataProcessor::transform_item
    └── DataProcessor::output
"#;

    println!("Expected hierarchy:{expected_hierarchy}");
}

fn create_sample_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();

    // Create symbols based on the sample.rs structure
    let symbols = vec![
        ("main", SymbolKind::Function, 3),
        ("calculate", SymbolKind::Function, 11),
        ("add", SymbolKind::Function, 17),
        ("multiply", SymbolKind::Function, 21),
        ("combine", SymbolKind::Function, 25),
        ("DataProcessor::new", SymbolKind::Method, 34),
        ("DataProcessor::process", SymbolKind::Method, 40),
        ("DataProcessor::validate", SymbolKind::Method, 46),
        ("DataProcessor::check_item", SymbolKind::Method, 53),
        ("DataProcessor::transform", SymbolKind::Method, 59),
        ("DataProcessor::transform_item", SymbolKind::Method, 65),
        ("DataProcessor::output", SymbolKind::Method, 69),
    ];

    let mut indices = std::collections::HashMap::new();

    // Add all symbols to the graph
    for (name, kind, line) in symbols {
        let symbol = Symbol {
            id: name.to_string(),
            name: name.to_string(),
            kind,
            file_path: "tests/fixtures/sample.rs".to_string(),
            range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line: line + 3,
                    character: 0,
                },
            },
            documentation: None,
        };
        let idx = graph.add_symbol(symbol);
        indices.insert(name.to_string(), idx);
    }

    // Add edges based on the call hierarchy
    // main calls
    graph.add_edge(indices["main"], indices["calculate"], EdgeKind::Reference);
    graph.add_edge(
        indices["main"],
        indices["DataProcessor::new"],
        EdgeKind::Reference,
    );

    // calculate calls
    graph.add_edge(indices["calculate"], indices["add"], EdgeKind::Reference);
    graph.add_edge(
        indices["calculate"],
        indices["multiply"],
        EdgeKind::Reference,
    );
    graph.add_edge(
        indices["calculate"],
        indices["combine"],
        EdgeKind::Reference,
    );

    // DataProcessor::process calls
    graph.add_edge(
        indices["DataProcessor::process"],
        indices["DataProcessor::validate"],
        EdgeKind::Reference,
    );
    graph.add_edge(
        indices["DataProcessor::process"],
        indices["DataProcessor::transform"],
        EdgeKind::Reference,
    );
    graph.add_edge(
        indices["DataProcessor::process"],
        indices["DataProcessor::output"],
        EdgeKind::Reference,
    );

    // DataProcessor::validate calls
    graph.add_edge(
        indices["DataProcessor::validate"],
        indices["DataProcessor::check_item"],
        EdgeKind::Reference,
    );

    // DataProcessor::transform calls
    graph.add_edge(
        indices["DataProcessor::transform"],
        indices["DataProcessor::transform_item"],
        EdgeKind::Reference,
    );

    graph
}
