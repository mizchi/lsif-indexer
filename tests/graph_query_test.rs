use lsif_core::{
    format_query_results, CodeGraph, EdgeKind, Position, QueryEngine, QueryParser, Range, Symbol,
    SymbolKind,
};

fn create_test_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();

    // Create a simple type hierarchy
    let interface = Symbol {
        id: "interface:ILogger".to_string(),
        name: "ILogger".to_string(),
        kind: SymbolKind::Interface,
        file_path: "logger.rs".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 5,
                character: 1,
            },
        },
        documentation: Some("Logger interface".to_string()),
    };

    let console_logger = Symbol {
        id: "class:ConsoleLogger".to_string(),
        name: "ConsoleLogger".to_string(),
        kind: SymbolKind::Class,
        file_path: "console_logger.rs".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 20,
                character: 1,
            },
        },
        documentation: Some("Console logger implementation".to_string()),
    };

    let file_logger = Symbol {
        id: "class:FileLogger".to_string(),
        name: "FileLogger".to_string(),
        kind: SymbolKind::Class,
        file_path: "file_logger.rs".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 30,
                character: 1,
            },
        },
        documentation: Some("File logger implementation".to_string()),
    };

    let log_function = Symbol {
        id: "fn:log".to_string(),
        name: "log".to_string(),
        kind: SymbolKind::Function,
        file_path: "main.rs".to_string(),
        range: Range {
            start: Position {
                line: 10,
                character: 0,
            },
            end: Position {
                line: 15,
                character: 1,
            },
        },
        documentation: Some("Log function".to_string()),
    };

    let logger_var = Symbol {
        id: "var:logger".to_string(),
        name: "logger".to_string(),
        kind: SymbolKind::Variable,
        file_path: "main.rs".to_string(),
        range: Range {
            start: Position {
                line: 5,
                character: 0,
            },
            end: Position {
                line: 5,
                character: 20,
            },
        },
        documentation: Some("Logger instance".to_string()),
    };

    let config = Symbol {
        id: "var:config".to_string(),
        name: "config".to_string(),
        kind: SymbolKind::Variable,
        file_path: "config.rs".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 20,
            },
        },
        documentation: None,
    };

    // Add symbols to graph
    let interface_idx = graph.add_symbol(interface);
    let console_idx = graph.add_symbol(console_logger);
    let file_idx = graph.add_symbol(file_logger);
    let log_fn_idx = graph.add_symbol(log_function);
    let logger_var_idx = graph.add_symbol(logger_var);
    let config_idx = graph.add_symbol(config);

    // Create relationships
    // ConsoleLogger implements ILogger
    graph.add_edge(console_idx, interface_idx, EdgeKind::Definition);

    // FileLogger implements ILogger
    graph.add_edge(file_idx, interface_idx, EdgeKind::Definition);

    // logger variable references ConsoleLogger
    graph.add_edge(logger_var_idx, console_idx, EdgeKind::Reference);

    // log function references logger variable
    graph.add_edge(log_fn_idx, logger_var_idx, EdgeKind::Reference);

    // config references FileLogger
    graph.add_edge(config_idx, file_idx, EdgeKind::Reference);

    graph
}

#[test]
fn test_parse_simple_patterns() {
    // Test single node
    let pattern = QueryParser::parse("(fn:Function)").unwrap();
    assert_eq!(pattern.nodes.len(), 1);
    assert_eq!(pattern.nodes[0].variable, Some("fn".to_string()));
    assert_eq!(pattern.nodes[0].label, Some("Function".to_string()));

    // Test relationship
    let pattern = QueryParser::parse("(a)-[:Reference]->(b)").unwrap();
    assert_eq!(pattern.nodes.len(), 2);
    assert_eq!(pattern.relationships.len(), 1);
    assert_eq!(
        pattern.relationships[0].edge_type,
        Some(EdgeKind::Reference)
    );

    // Test node with only label
    let pattern = QueryParser::parse("(:Class)").unwrap();
    assert_eq!(pattern.nodes[0].variable, None);
    assert_eq!(pattern.nodes[0].label, Some("Class".to_string()));
}

#[test]
fn test_parse_depth_patterns() {
    // Fixed depth
    let pattern = QueryParser::parse("(a)-[:Reference*2]->(b)").unwrap();
    assert_eq!(pattern.relationships[0].min_depth, 2);
    assert_eq!(pattern.relationships[0].max_depth, Some(2));

    // Range depth
    let pattern = QueryParser::parse("(a)-[:Reference*1..3]->(b)").unwrap();
    assert_eq!(pattern.relationships[0].min_depth, 1);
    assert_eq!(pattern.relationships[0].max_depth, Some(3));

    // Unlimited depth
    let pattern = QueryParser::parse("(a)-[:Reference*]->(b)").unwrap();
    assert_eq!(pattern.relationships[0].min_depth, 1);
    assert_eq!(pattern.relationships[0].max_depth, None);

    // Unlimited range
    let pattern = QueryParser::parse("(a)-[:Reference*2..]->(b)").unwrap();
    assert_eq!(pattern.relationships[0].min_depth, 2);
    assert_eq!(pattern.relationships[0].max_depth, None);
}

#[test]
fn test_query_find_implementations() {
    let graph = create_test_graph();
    let engine = QueryEngine::new(&graph);

    // Find all classes that implement the interface
    let pattern = QueryParser::parse("(impl:Class)-[:Definition]->(interface:Interface)").unwrap();
    let results = engine.execute(&pattern);

    assert_eq!(results.matches.len(), 2);

    // Check that we found both implementations
    let impl_names: Vec<String> = results
        .matches
        .iter()
        .flat_map(|m| &m.bindings)
        .filter(|(var, _)| var == "impl")
        .map(|(_, sym)| sym.name.clone())
        .collect();

    assert!(impl_names.contains(&"ConsoleLogger".to_string()));
    assert!(impl_names.contains(&"FileLogger".to_string()));
}

#[test]
fn test_query_find_references() {
    let graph = create_test_graph();
    let engine = QueryEngine::new(&graph);

    // Find all variables that reference classes
    let pattern = QueryParser::parse("(var:Variable)-[:Reference]->(cls:Class)").unwrap();
    let results = engine.execute(&pattern);

    assert_eq!(results.matches.len(), 2); // logger -> ConsoleLogger, config -> FileLogger

    for match_result in &results.matches {
        let var_binding = match_result
            .bindings
            .iter()
            .find(|(v, _)| v == "var")
            .map(|(_, s)| &s.name);
        let cls_binding = match_result
            .bindings
            .iter()
            .find(|(v, _)| v == "cls")
            .map(|(_, s)| &s.name);

        match (var_binding, cls_binding) {
            (Some(var), Some(cls)) => {
                assert!(
                    (var == "logger" && cls == "ConsoleLogger")
                        || (var == "config" && cls == "FileLogger")
                );
            }
            _ => panic!("Missing bindings"),
        }
    }
}

#[test]
fn test_query_transitive_references() {
    let graph = create_test_graph();
    let engine = QueryEngine::new(&graph);

    // Find functions that indirectly reference classes through variables
    let pattern = QueryParser::parse("(fn:Function)-[:Reference*1..2]->(cls:Class)").unwrap();
    let results = engine.execute(&pattern);

    // Should find: log -> logger -> ConsoleLogger
    assert!(!results.matches.is_empty());

    let has_log_to_console = results.matches.iter().any(|m| {
        let fn_name = m
            .bindings
            .iter()
            .find(|(v, _)| v == "fn")
            .map(|(_, s)| &s.name);
        let cls_name = m
            .bindings
            .iter()
            .find(|(v, _)| v == "cls")
            .map(|(_, s)| &s.name);

        fn_name == Some(&"log".to_string()) && cls_name == Some(&"ConsoleLogger".to_string())
    });

    assert!(has_log_to_console);
}

#[test]
fn test_query_any_relationship() {
    let graph = create_test_graph();
    let engine = QueryEngine::new(&graph);

    // Find all nodes connected to interfaces (any relationship type)
    let pattern = QueryParser::parse("(node)-[]->(interface:Interface)").unwrap();
    let results = engine.execute(&pattern);

    // Should find ConsoleLogger and FileLogger
    assert_eq!(results.matches.len(), 2);
}

#[test]
fn test_query_with_paths() {
    let graph = create_test_graph();
    let engine = QueryEngine::new(&graph);

    // Find paths from functions to interfaces
    let pattern =
        QueryParser::parse("(fn:Function)-[:Reference*1..3]->(interface:Interface)").unwrap();
    let results = engine.execute(&pattern);

    // Check that paths are populated
    if !results.matches.is_empty() {
        for match_result in &results.matches {
            assert!(!match_result.paths.is_empty());

            // Each path should start with a function and end with an interface
            for path in &match_result.paths {
                if let (Some(first), Some(last)) = (path.first(), path.last()) {
                    assert!(matches!(first.kind, SymbolKind::Function));
                    assert!(matches!(last.kind, SymbolKind::Interface));
                }
            }
        }
    }
}

#[test]
fn test_format_query_results() {
    let graph = create_test_graph();
    let engine = QueryEngine::new(&graph);

    let pattern = QueryParser::parse("(impl:Class)-[:Definition]->(interface:Interface)").unwrap();
    let results = engine.execute(&pattern);

    let formatted = format_query_results(&results);

    // Check that the formatted output contains expected information
    assert!(formatted.contains("Found 2 matches"));
    assert!(formatted.contains("ConsoleLogger"));
    assert!(formatted.contains("FileLogger"));
    assert!(formatted.contains("ILogger"));
    assert!(formatted.contains("Bindings:"));
}
