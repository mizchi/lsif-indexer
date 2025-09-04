use lsif_core::{
    format_type_relations, CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind,
    TypeRelationsAnalyzer,
};

/// Create a complex type hierarchy for testing
fn create_type_hierarchy_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();

    // Base interface
    let i_serializable = Symbol {
        id: "interface:ISerializable".to_string(),
        name: "ISerializable".to_string(),
        kind: SymbolKind::Interface,
        file_path: "serializable.rs".to_string(),
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
        documentation: Some("Base serialization interface".to_string()),
    };

    // Base class implementing interface
    let base_model = Symbol {
        id: "class:BaseModel".to_string(),
        name: "BaseModel".to_string(),
        kind: SymbolKind::Class,
        file_path: "models/base.rs".to_string(),
        range: Range {
            start: Position {
                line: 10,
                character: 0,
            },
            end: Position {
                line: 50,
                character: 1,
            },
        },
        documentation: Some("Base model class".to_string()),
    };

    // Derived classes
    let user_model = Symbol {
        id: "class:UserModel".to_string(),
        name: "UserModel".to_string(),
        kind: SymbolKind::Class,
        file_path: "models/user.rs".to_string(),
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
        documentation: Some("User model extending BaseModel".to_string()),
    };

    let admin_model = Symbol {
        id: "class:AdminModel".to_string(),
        name: "AdminModel".to_string(),
        kind: SymbolKind::Class,
        file_path: "models/admin.rs".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 40,
                character: 1,
            },
        },
        documentation: Some("Admin model extending UserModel".to_string()),
    };

    // Variables using the types
    let current_user = Symbol {
        id: "var:currentUser".to_string(),
        name: "currentUser".to_string(),
        kind: SymbolKind::Variable,
        file_path: "main.rs".to_string(),
        range: Range {
            start: Position {
                line: 20,
                character: 0,
            },
            end: Position {
                line: 20,
                character: 30,
            },
        },
        documentation: Some("let currentUser: UserModel".to_string()),
    };

    let admin_user = Symbol {
        id: "var:adminUser".to_string(),
        name: "adminUser".to_string(),
        kind: SymbolKind::Variable,
        file_path: "main.rs".to_string(),
        range: Range {
            start: Position {
                line: 25,
                character: 0,
            },
            end: Position {
                line: 25,
                character: 30,
            },
        },
        documentation: Some("let adminUser: AdminModel".to_string()),
    };

    // Methods
    let get_user = Symbol {
        id: "fn:getUser".to_string(),
        name: "getUser".to_string(),
        kind: SymbolKind::Function,
        file_path: "services/user_service.rs".to_string(),
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
        documentation: Some("fn getUser() -> UserModel".to_string()),
    };

    let save_method = Symbol {
        id: "method:save".to_string(),
        name: "save".to_string(),
        kind: SymbolKind::Method,
        file_path: "models/base.rs".to_string(),
        range: Range {
            start: Position {
                line: 30,
                character: 4,
            },
            end: Position {
                line: 35,
                character: 5,
            },
        },
        documentation: Some("Method of BaseModel".to_string()),
    };

    let validate_method = Symbol {
        id: "method:validate".to_string(),
        name: "validate".to_string(),
        kind: SymbolKind::Method,
        file_path: "models/user.rs".to_string(),
        range: Range {
            start: Position {
                line: 15,
                character: 4,
            },
            end: Position {
                line: 20,
                character: 5,
            },
        },
        documentation: Some("Method of UserModel".to_string()),
    };

    // Fields
    let id_field = Symbol {
        id: "field:id".to_string(),
        name: "id".to_string(),
        kind: SymbolKind::Field,
        file_path: "models/base.rs".to_string(),
        range: Range {
            start: Position {
                line: 12,
                character: 4,
            },
            end: Position {
                line: 12,
                character: 20,
            },
        },
        documentation: Some("id: String".to_string()),
    };

    let username_field = Symbol {
        id: "field:username".to_string(),
        name: "username".to_string(),
        kind: SymbolKind::Field,
        file_path: "models/user.rs".to_string(),
        range: Range {
            start: Position {
                line: 5,
                character: 4,
            },
            end: Position {
                line: 5,
                character: 25,
            },
        },
        documentation: Some("username: String".to_string()),
    };

    // Add all symbols to graph
    let i_serializable_idx = graph.add_symbol(i_serializable);
    let base_model_idx = graph.add_symbol(base_model);
    let user_model_idx = graph.add_symbol(user_model);
    let admin_model_idx = graph.add_symbol(admin_model);
    let current_user_idx = graph.add_symbol(current_user);
    let admin_user_idx = graph.add_symbol(admin_user);
    let get_user_idx = graph.add_symbol(get_user);
    let save_method_idx = graph.add_symbol(save_method);
    let validate_method_idx = graph.add_symbol(validate_method);
    let id_field_idx = graph.add_symbol(id_field);
    let username_field_idx = graph.add_symbol(username_field);

    // Create relationships
    // BaseModel implements ISerializable
    graph.add_edge(base_model_idx, i_serializable_idx, EdgeKind::Definition);

    // UserModel extends BaseModel
    graph.add_edge(user_model_idx, base_model_idx, EdgeKind::Definition);

    // AdminModel extends UserModel
    graph.add_edge(admin_model_idx, user_model_idx, EdgeKind::Definition);

    // Variables reference their types
    graph.add_edge(current_user_idx, user_model_idx, EdgeKind::Reference);
    graph.add_edge(admin_user_idx, admin_model_idx, EdgeKind::Reference);

    // Function returns UserModel
    graph.add_edge(get_user_idx, user_model_idx, EdgeKind::Reference);

    // Methods belong to their classes
    graph.add_edge(save_method_idx, base_model_idx, EdgeKind::Reference);
    graph.add_edge(validate_method_idx, user_model_idx, EdgeKind::Reference);

    // Fields belong to their classes
    graph.add_edge(id_field_idx, base_model_idx, EdgeKind::Reference);
    graph.add_edge(username_field_idx, user_model_idx, EdgeKind::Reference);

    graph
}

/// Create a graph with complex type relationships
fn create_complex_type_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();

    // Generic container type
    let container_type = Symbol {
        id: "type:Container<T>".to_string(),
        name: "Container<T>".to_string(),
        kind: SymbolKind::Class,
        file_path: "container.rs".to_string(),
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
        documentation: Some("Generic container type".to_string()),
    };

    // Specific instantiation
    let string_container = Symbol {
        id: "type:StringContainer".to_string(),
        name: "StringContainer".to_string(),
        kind: SymbolKind::Class,
        file_path: "container.rs".to_string(),
        range: Range {
            start: Position {
                line: 25,
                character: 0,
            },
            end: Position {
                line: 30,
                character: 1,
            },
        },
        documentation: Some("type StringContainer = Container<String>".to_string()),
    };

    // Multiple variables using the container
    let data1 = Symbol {
        id: "var:data1".to_string(),
        name: "data1".to_string(),
        kind: SymbolKind::Variable,
        file_path: "app.rs".to_string(),
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
        documentation: None,
    };

    let data2 = Symbol {
        id: "var:data2".to_string(),
        name: "data2".to_string(),
        kind: SymbolKind::Variable,
        file_path: "app.rs".to_string(),
        range: Range {
            start: Position {
                line: 15,
                character: 0,
            },
            end: Position {
                line: 15,
                character: 30,
            },
        },
        documentation: None,
    };

    let data3 = Symbol {
        id: "var:data3".to_string(),
        name: "data3".to_string(),
        kind: SymbolKind::Variable,
        file_path: "app.rs".to_string(),
        range: Range {
            start: Position {
                line: 20,
                character: 0,
            },
            end: Position {
                line: 20,
                character: 30,
            },
        },
        documentation: None,
    };

    // Functions working with the type
    let create_container = Symbol {
        id: "fn:createContainer".to_string(),
        name: "createContainer".to_string(),
        kind: SymbolKind::Function,
        file_path: "factory.rs".to_string(),
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
        documentation: Some("fn createContainer() -> StringContainer".to_string()),
    };

    let process_container = Symbol {
        id: "fn:processContainer".to_string(),
        name: "processContainer".to_string(),
        kind: SymbolKind::Function,
        file_path: "processor.rs".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 15,
                character: 1,
            },
        },
        documentation: Some("fn processContainer(c: StringContainer)".to_string()),
    };

    // Add to graph
    let container_idx = graph.add_symbol(container_type);
    let string_container_idx = graph.add_symbol(string_container);
    let data1_idx = graph.add_symbol(data1);
    let data2_idx = graph.add_symbol(data2);
    let data3_idx = graph.add_symbol(data3);
    let create_idx = graph.add_symbol(create_container);
    let process_idx = graph.add_symbol(process_container);

    // StringContainer is Container<String>
    graph.add_edge(string_container_idx, container_idx, EdgeKind::Definition);

    // Variables use StringContainer
    graph.add_edge(data1_idx, string_container_idx, EdgeKind::Reference);
    graph.add_edge(data2_idx, string_container_idx, EdgeKind::Reference);
    graph.add_edge(data3_idx, string_container_idx, EdgeKind::Reference);

    // Functions use StringContainer
    graph.add_edge(create_idx, string_container_idx, EdgeKind::Reference);
    graph.add_edge(process_idx, string_container_idx, EdgeKind::Reference);

    graph
}

#[test]
fn test_collect_type_relations_with_hierarchy() {
    let graph = create_type_hierarchy_graph();
    let analyzer = TypeRelationsAnalyzer::new(&graph);

    // Test collecting relations for UserModel
    let relations = analyzer
        .collect_type_relations("class:UserModel", 3)
        .unwrap();

    println!("Type relations for UserModel:");
    println!("{}", format_type_relations(&relations));

    assert_eq!(relations.root_type.name, "UserModel");

    // Should find currentUser as a user
    assert!(relations.users.iter().any(|u| u.id == "var:currentUser"));

    // Should find AdminModel as an extension
    assert!(relations
        .extensions
        .iter()
        .any(|e| e.id == "class:AdminModel"));

    // Should find validate method
    assert!(relations.methods.iter().any(|m| m.id == "method:validate"));

    // Should find username field
    assert!(relations.members.iter().any(|f| f.id == "field:username"));

    assert!(relations.total_relations > 0);
}

#[test]
fn test_type_hierarchy_analysis() {
    let graph = create_type_hierarchy_graph();
    let analyzer = TypeRelationsAnalyzer::new(&graph);

    // Test hierarchy for UserModel
    let hierarchy = analyzer.find_type_hierarchy("class:UserModel");

    assert!(hierarchy.root.is_some());
    assert_eq!(hierarchy.root.unwrap().name, "UserModel");

    // Should have BaseModel as parent (plus ISerializable through recursion)
    assert!(!hierarchy.parents.is_empty());
    assert!(hierarchy.parents.iter().any(|p| p.name == "BaseModel"));

    // Should have AdminModel as child
    assert_eq!(hierarchy.children.len(), 1);
    assert_eq!(hierarchy.children[0].name, "AdminModel");

    // AdminModel should also be found as sibling (shares BaseModel parent)
    // Note: In this case, AdminModel extends UserModel directly, so no siblings
}

#[test]
fn test_recursive_type_references() {
    let graph = create_complex_type_graph();
    let analyzer = TypeRelationsAnalyzer::new(&graph);

    // Find all references to Container<T> through StringContainer
    let refs = analyzer.find_all_type_references("type:Container<T>", 2);

    println!("All references to Container<T> (recursive):");
    for reference in &refs {
        println!("  - {} ({:?})", reference.name, reference.kind);
    }

    // Should find StringContainer and all its users
    assert!(refs.iter().any(|r| r.id == "type:StringContainer"));

    // Through recursion at depth 2, should also find variables
    // that reference StringContainer
    assert!(refs.len() >= 4); // StringContainer + 3 variables minimum
}

#[test]
fn test_group_relations_by_type() {
    let graph = create_complex_type_graph();
    let analyzer = TypeRelationsAnalyzer::new(&graph);

    let groups = analyzer.group_relations_by_type("type:StringContainer");

    println!("Relations grouped by type for StringContainer:");
    println!("  Variables: {}", groups.variables_of_type.len());
    println!(
        "  Functions returning: {}",
        groups.functions_returning_type.len()
    );
    println!("  Referenced by: {}", groups.referenced_by.len());

    // Should categorize correctly
    assert_eq!(groups.variables_of_type.len(), 3); // data1, data2, data3
    assert_eq!(groups.functions_returning_type.len(), 2); // createContainer and processContainer

    // processContainer uses it as parameter, should be in referenced_by
    assert!(groups
        .referenced_by
        .iter()
        .any(|r| r.id == "fn:processContainer"));
}

#[test]
fn test_type_relations_for_interface() {
    let graph = create_type_hierarchy_graph();
    let analyzer = TypeRelationsAnalyzer::new(&graph);

    // Test collecting relations for interface
    let relations = analyzer.collect_type_relations("interface:ISerializable", 3);

    assert!(relations.is_some());
    let relations = relations.unwrap();

    println!("Type relations for ISerializable:");
    println!("{}", format_type_relations(&relations));

    // Should find classes extending from it (through recursive collection)
    assert!(relations
        .extensions
        .iter()
        .any(|i| i.id == "class:BaseModel"));

    // Through recursion, should find derived classes
    assert!(relations.total_relations > 0);
}

#[test]
fn test_non_type_symbol_returns_none() {
    let graph = create_type_hierarchy_graph();
    let analyzer = TypeRelationsAnalyzer::new(&graph);

    // Variable is not a type, should return None
    let relations = analyzer.collect_type_relations("var:currentUser", 2);
    assert!(relations.is_none());

    // Method is not a type, should return None
    let relations = analyzer.collect_type_relations("method:save", 2);
    assert!(relations.is_none());
}

#[test]
fn test_max_depth_limiting() {
    let graph = create_type_hierarchy_graph();
    let analyzer = TypeRelationsAnalyzer::new(&graph);

    // Test with depth 0 (only direct relations)
    let relations_d0 = analyzer
        .collect_type_relations("class:BaseModel", 0)
        .unwrap();

    // Test with depth 2 (recursive)
    let relations_d2 = analyzer
        .collect_type_relations("class:BaseModel", 2)
        .unwrap();

    println!("Relations at depth 0: {}", relations_d0.total_relations);
    println!("Relations at depth 2: {}", relations_d2.total_relations);

    // Deeper recursion should find more relations
    assert!(relations_d2.total_relations >= relations_d0.total_relations);
}
