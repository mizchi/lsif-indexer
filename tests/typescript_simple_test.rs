use cli::reference_finder::find_all_references;
use lsif_core::SymbolKind;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_typescript_simple_interface() {
    let temp_dir = TempDir::new().unwrap();

    // シンプルなTypeScriptコード
    let content = r#"interface User {
    id: string;
    name: string;
}

export interface User {
    email?: string;
}

const user: User = { id: "1", name: "Alice" };
function getUser(): User {
    return user;
}
"#;

    fs::write(temp_dir.path().join("test.ts"), content).unwrap();

    let references = find_all_references(temp_dir.path(), "User", &SymbolKind::Interface).unwrap();

    println!("Found {} references for User interface", references.len());
    for (i, r) in references.iter().enumerate() {
        println!(
            "  {}: line {} (definition: {})",
            i + 1,
            r.symbol.range.start.line + 1,
            r.is_definition
        );
    }

    // 少なくとも何か見つかるはず
    assert!(
        !references.is_empty(),
        "Should find at least one reference to User"
    );

    // 定義があるかチェック
    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    println!("Found {} definitions", definitions.len());

    // export interface User も定義として認識されるべき
    assert!(
        !definitions.is_empty(),
        "Should find at least one definition"
    );
}

#[test]
fn test_typescript_simple_function() {
    let temp_dir = TempDir::new().unwrap();

    let content = r#"function createUser(name: string) {
    return { name };
}

export function createUser(name: string, email: string) {
    return { name, email };
}

const user = createUser("Alice");
const user2 = createUser("Bob", "bob@example.com");
"#;

    fs::write(temp_dir.path().join("test.ts"), content).unwrap();

    let references =
        find_all_references(temp_dir.path(), "createUser", &SymbolKind::Function).unwrap();

    println!(
        "Found {} references for createUser function",
        references.len()
    );

    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

    println!(
        "Definitions: {}, Usages: {}",
        definitions.len(),
        usages.len()
    );

    assert!(
        !definitions.is_empty(),
        "Should find at least one definition"
    );
    assert_eq!(usages.len(), 2, "Should find exactly 2 function calls");
}

#[test]
fn test_typescript_simple_class() {
    let temp_dir = TempDir::new().unwrap();

    let content = r#"class UserService {
    constructor() {}
    getUser() { return null; }
}

export class UserService {
    private users: any[] = [];
}

const service = new UserService();
"#;

    fs::write(temp_dir.path().join("test.ts"), content).unwrap();

    let references =
        find_all_references(temp_dir.path(), "UserService", &SymbolKind::Class).unwrap();

    println!(
        "Found {} references for UserService class",
        references.len()
    );

    assert!(!references.is_empty(), "Should find UserService references");

    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    assert!(
        !definitions.is_empty(),
        "Should find at least one class definition"
    );
}
