use lsif_core::{
    format_definition_chain, CodeGraph, DefinitionChainAnalyzer, EdgeKind, Position, Range, Symbol,
    SymbolKind,
};

/// Create a complex code graph simulating real code relationships
fn create_complex_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();

    // Simulate a type alias chain:
    // type MyString = String
    // type UserName = MyString
    // type AdminName = UserName

    let my_string = Symbol {
        id: "type:MyString".to_string(),
        name: "MyString".to_string(),
        kind: SymbolKind::Class,
        file_path: "types.rs".to_string(),
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
        documentation: Some("type MyString = String".to_string()),
    };

    let string_type = Symbol {
        id: "std:String".to_string(),
        name: "String".to_string(),
        kind: SymbolKind::Class,
        file_path: "std/string.rs".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 100,
                character: 0,
            },
        },
        documentation: Some("Standard String type".to_string()),
    };

    let user_name = Symbol {
        id: "type:UserName".to_string(),
        name: "UserName".to_string(),
        kind: SymbolKind::Class,
        file_path: "user.rs".to_string(),
        range: Range {
            start: Position {
                line: 5,
                character: 0,
            },
            end: Position {
                line: 5,
                character: 25,
            },
        },
        documentation: Some("type UserName = MyString".to_string()),
    };

    let admin_name = Symbol {
        id: "type:AdminName".to_string(),
        name: "AdminName".to_string(),
        kind: SymbolKind::Class,
        file_path: "admin.rs".to_string(),
        range: Range {
            start: Position {
                line: 10,
                character: 0,
            },
            end: Position {
                line: 10,
                character: 30,
            },
        },
        documentation: Some("type AdminName = UserName".to_string()),
    };

    // Add symbols to graph
    let my_string_idx = graph.add_symbol(my_string);
    let string_idx = graph.add_symbol(string_type);
    let user_name_idx = graph.add_symbol(user_name);
    let admin_name_idx = graph.add_symbol(admin_name);

    // Create definition chain: AdminName -> UserName -> MyString -> String
    graph.add_edge(admin_name_idx, user_name_idx, EdgeKind::Definition);
    graph.add_edge(user_name_idx, my_string_idx, EdgeKind::Definition);
    graph.add_edge(my_string_idx, string_idx, EdgeKind::Definition);

    // Add variable with type reference
    let admin_var = Symbol {
        id: "var:admin_user".to_string(),
        name: "admin_user".to_string(),
        kind: SymbolKind::Variable,
        file_path: "main.rs".to_string(),
        range: Range {
            start: Position {
                line: 20,
                character: 0,
            },
            end: Position {
                line: 20,
                character: 40,
            },
        },
        documentation: Some("let admin_user: AdminName".to_string()),
    };

    let admin_var_idx = graph.add_symbol(admin_var);
    graph.add_edge(admin_var_idx, admin_name_idx, EdgeKind::Definition);

    // Add a function that returns a type
    let get_admin = Symbol {
        id: "fn:get_admin".to_string(),
        name: "get_admin".to_string(),
        kind: SymbolKind::Function,
        file_path: "main.rs".to_string(),
        range: Range {
            start: Position {
                line: 30,
                character: 0,
            },
            end: Position {
                line: 35,
                character: 1,
            },
        },
        documentation: Some("fn get_admin() -> AdminName".to_string()),
    };

    let get_admin_idx = graph.add_symbol(get_admin);
    graph.add_edge(get_admin_idx, admin_name_idx, EdgeKind::Definition);

    graph
}

/// Create a graph with circular dependencies
fn create_cyclic_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();

    // Simulate recursive types:
    // struct Node { next: Option<Box<Node>> }
    // type NodeRef = &Node
    // impl Node { fn get_ref(&self) -> NodeRef }

    let node_struct = Symbol {
        id: "struct:Node".to_string(),
        name: "Node".to_string(),
        kind: SymbolKind::Class,
        file_path: "node.rs".to_string(),
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
        documentation: Some("Recursive node structure".to_string()),
    };

    let node_ref = Symbol {
        id: "type:NodeRef".to_string(),
        name: "NodeRef".to_string(),
        kind: SymbolKind::Class,
        file_path: "node.rs".to_string(),
        range: Range {
            start: Position {
                line: 7,
                character: 0,
            },
            end: Position {
                line: 7,
                character: 20,
            },
        },
        documentation: Some("type NodeRef = &Node".to_string()),
    };

    let node_idx = graph.add_symbol(node_struct.clone());
    let ref_idx = graph.add_symbol(node_ref);

    // Create cycle: Node -> NodeRef -> Node
    graph.add_edge(node_idx, ref_idx, EdgeKind::Definition);
    graph.add_edge(ref_idx, node_idx, EdgeKind::Definition);

    // Add another type that references the cycle
    let list_node = Symbol {
        id: "struct:ListNode".to_string(),
        name: "ListNode".to_string(),
        kind: SymbolKind::Class,
        file_path: "list.rs".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 10,
                character: 1,
            },
        },
        documentation: Some("List node using Node".to_string()),
    };

    let list_idx = graph.add_symbol(list_node);
    graph.add_edge(list_idx, node_idx, EdgeKind::Definition);

    graph
}

#[test]
fn test_recursive_definition_tracing() {
    let graph = create_complex_graph();
    let analyzer = DefinitionChainAnalyzer::new(&graph);

    // Test tracing from variable to ultimate type
    let chain = analyzer.get_definition_chain("var:admin_user").unwrap();

    println!("Definition chain for admin_user:");
    println!("{}", format_definition_chain(&chain));

    assert!(!chain.has_cycle);
    assert_eq!(chain.chain.len(), 5);
    assert_eq!(chain.chain[0].id, "var:admin_user");
    assert_eq!(chain.chain[1].id, "type:AdminName");
    assert_eq!(chain.chain[2].id, "type:UserName");
    assert_eq!(chain.chain[3].id, "type:MyString");
    assert_eq!(chain.chain[4].id, "std:String");

    // Test finding ultimate source
    let ultimate = analyzer.find_ultimate_source("var:admin_user").unwrap();
    assert_eq!(ultimate.id, "std:String");
    assert_eq!(ultimate.name, "String");

    // Test from function return type
    let fn_chain = analyzer.get_definition_chain("fn:get_admin").unwrap();
    assert_eq!(fn_chain.chain.len(), 5);
    assert_eq!(fn_chain.chain.last().unwrap().id, "std:String");
}

#[test]
fn test_cyclic_definition_detection() {
    let graph = create_cyclic_graph();
    let analyzer = DefinitionChainAnalyzer::new(&graph);

    // Test cycle detection
    let chain = analyzer.get_definition_chain("struct:Node").unwrap();

    println!("Cyclic chain for Node:");
    println!("{}", format_definition_chain(&chain));

    assert!(chain.has_cycle);

    // Test tracing through cycle
    let list_chain = analyzer.get_definition_chain("struct:ListNode").unwrap();
    println!("Chain for ListNode (references cycle):");
    println!("{}", format_definition_chain(&list_chain));

    // Should stop at cycle
    assert!(list_chain.has_cycle);
}

#[test]
fn test_multiple_definition_paths() {
    let mut graph = CodeGraph::new();

    // Create diamond dependency:
    // UserInput -> Validated OR Raw
    // Validated -> Sanitized
    // Raw -> Sanitized
    // Sanitized -> String

    let user_input = Symbol {
        id: "type:UserInput".to_string(),
        name: "UserInput".to_string(),
        kind: SymbolKind::Class,
        file_path: "input.rs".to_string(),
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

    let validated = Symbol {
        id: "type:Validated".to_string(),
        name: "Validated".to_string(),
        kind: SymbolKind::Class,
        file_path: "input.rs".to_string(),
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
        documentation: None,
    };

    let raw = Symbol {
        id: "type:Raw".to_string(),
        name: "Raw".to_string(),
        kind: SymbolKind::Class,
        file_path: "input.rs".to_string(),
        range: Range {
            start: Position {
                line: 10,
                character: 0,
            },
            end: Position {
                line: 10,
                character: 20,
            },
        },
        documentation: None,
    };

    let sanitized = Symbol {
        id: "type:Sanitized".to_string(),
        name: "Sanitized".to_string(),
        kind: SymbolKind::Class,
        file_path: "input.rs".to_string(),
        range: Range {
            start: Position {
                line: 15,
                character: 0,
            },
            end: Position {
                line: 15,
                character: 20,
            },
        },
        documentation: None,
    };

    let string = Symbol {
        id: "std:String".to_string(),
        name: "String".to_string(),
        kind: SymbolKind::Class,
        file_path: "std/string.rs".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 100,
                character: 0,
            },
        },
        documentation: None,
    };

    let input_idx = graph.add_symbol(user_input);
    let validated_idx = graph.add_symbol(validated);
    let raw_idx = graph.add_symbol(raw);
    let sanitized_idx = graph.add_symbol(sanitized);
    let string_idx = graph.add_symbol(string);

    // UserInput can be either Validated or Raw
    graph.add_edge(input_idx, validated_idx, EdgeKind::Definition);
    graph.add_edge(input_idx, raw_idx, EdgeKind::Definition);

    // Both Validated and Raw lead to Sanitized
    graph.add_edge(validated_idx, sanitized_idx, EdgeKind::Definition);
    graph.add_edge(raw_idx, sanitized_idx, EdgeKind::Definition);

    // Sanitized leads to String
    graph.add_edge(sanitized_idx, string_idx, EdgeKind::Definition);

    let analyzer = DefinitionChainAnalyzer::new(&graph);

    // Get all possible chains
    let all_chains = analyzer.get_all_definition_chains("type:UserInput");

    println!("All definition chains for UserInput:");
    for (i, chain) in all_chains.iter().enumerate() {
        println!("  Path {}: {}", i + 1, format_definition_chain(chain));
    }

    // Should have 2 different paths
    assert_eq!(all_chains.len(), 2);

    // Both should end at String
    for chain in &all_chains {
        assert_eq!(chain.chain.last().unwrap().id, "std:String");
        assert!(!chain.has_cycle);
    }

    // Test path checking
    assert!(analyzer.has_definition_path("type:UserInput", "std:String"));
    assert!(analyzer.has_definition_path("type:Validated", "std:String"));
    assert!(!analyzer.has_definition_path("std:String", "type:UserInput"));
}

#[test]
fn test_shortest_definition_path() {
    let mut graph = CodeGraph::new();

    // Create a graph with multiple paths of different lengths
    let symbols: Vec<Symbol> = (0..6)
        .map(|i| Symbol {
            id: format!("type:T{i}"),
            name: format!("Type{i}"),
            kind: SymbolKind::Class,
            file_path: "types.rs".to_string(),
            range: Range {
                start: Position {
                    line: i * 5,
                    character: 0,
                },
                end: Position {
                    line: i * 5,
                    character: 20,
                },
            },
            documentation: None,
        })
        .collect();

    let indices: Vec<_> = symbols.into_iter().map(|s| graph.add_symbol(s)).collect();

    // Create paths:
    // T0 -> T1 -> T2 -> T3 -> T4 -> T5 (length 5)
    // T0 -> T2 -> T5 (length 2)
    // T0 -> T5 (direct, length 1)

    // Long path
    for i in 0..5 {
        graph.add_edge(indices[i], indices[i + 1], EdgeKind::Definition);
    }

    // Medium path
    graph.add_edge(indices[0], indices[2], EdgeKind::Definition);
    graph.add_edge(indices[2], indices[5], EdgeKind::Definition);

    // Short path
    graph.add_edge(indices[0], indices[5], EdgeKind::Definition);

    let analyzer = DefinitionChainAnalyzer::new(&graph);

    // Should find the shortest path
    let shortest = analyzer
        .get_shortest_definition_path("type:T0", "type:T5")
        .unwrap();

    println!("Shortest path from T0 to T5:");
    for symbol in &shortest {
        print!("{} ", symbol.name);
    }
    println!();

    assert_eq!(shortest.len(), 2);
    assert_eq!(shortest[0].id, "type:T0");
    assert_eq!(shortest[1].id, "type:T5");
}
