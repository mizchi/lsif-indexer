use lsif_indexer::cli::lsp_adapter::{GenericLspClient, TypeScriptAdapter};
use lsif_indexer::cli::reference_finder::find_all_references;
use lsif_indexer::core::SymbolKind;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// TypeScript LSPが利用可能かチェック
fn ensure_typescript_lsp() {
    // @typescript/native-previewが使用可能かチェック
    let native_preview_available = Command::new("npx")
        .args(["-y", "@typescript/native-preview", "--version"])
        .output()
        .is_ok();

    // フォールバック: typescript-language-serverをチェック
    let tsserver_available = Command::new("typescript-language-server")
        .arg("--version")
        .output()
        .is_ok();

    if !native_preview_available && !tsserver_available {
        println!("TypeScript LSP not available. Installing @typescript/native-preview...");

        // @typescript/native-previewをインストール試行
        let install_result = Command::new("npm")
            .args(["install", "-g", "@typescript/native-preview"])
            .output();

        match install_result {
            Ok(output) if output.status.success() => {
                println!("@typescript/native-preview installed successfully");
            }
            _ => {
                panic!(
                    "TypeScript LSP not available. Please install one of:\n\
                    - npm install -g @typescript/native-preview (recommended)\n\
                    - npm install -g typescript-language-server typescript"
                );
            }
        }
    }
}

/// テスト用のTypeScriptプロジェクトを作成
fn create_typescript_test_project(temp_dir: &TempDir) -> PathBuf {
    let project_root = temp_dir.path();

    // tsconfig.json
    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true
  }
}"#;
    fs::write(project_root.join("tsconfig.json"), tsconfig).unwrap();

    // main.ts
    let main_content = r#"// main.ts
import { UserService } from './services/user.service';
import { User, UserRole } from './models/user.model';

function createUser(name: string, email: string): User {
    return {
        id: Math.random().toString(36),
        name,
        email,
        role: UserRole.User
    };
}

async function main() {
    const service = new UserService();
    const user = createUser('John Doe', 'john@example.com');
    
    await service.saveUser(user);
    const savedUser = await service.getUser(user.id);
    console.log('Saved user:', savedUser);
}

main().catch(console.error);
"#;
    fs::write(project_root.join("main.ts"), main_content).unwrap();

    // models/user.model.ts
    fs::create_dir_all(project_root.join("models")).unwrap();
    let user_model = r#"// models/user.model.ts
export interface User {
    id: string;
    name: string;
    email: string;
    role: UserRole;
}

export enum UserRole {
    Admin = 'admin',
    User = 'user',
    Guest = 'guest'
}

export function isValidUser(user: User): boolean {
    return user.name.length > 0 && user.email.includes('@');
}
"#;
    fs::write(project_root.join("models/user.model.ts"), user_model).unwrap();

    // services/user.service.ts
    fs::create_dir_all(project_root.join("services")).unwrap();
    let user_service = r#"// services/user.service.ts
import { User, isValidUser } from '../models/user.model';

export class UserService {
    private users: Map<string, User> = new Map();

    async saveUser(user: User): Promise<void> {
        if (!isValidUser(user)) {
            throw new Error('Invalid user');
        }
        this.users.set(user.id, user);
    }

    async getUser(id: string): Promise<User | undefined> {
        return this.users.get(id);
    }

    async getAllUsers(): Promise<User[]> {
        return Array.from(this.users.values());
    }
}
"#;
    fs::write(project_root.join("services/user.service.ts"), user_service).unwrap();

    // tests/user.test.ts
    fs::create_dir_all(project_root.join("tests")).unwrap();
    let test_content = r#"// tests/user.test.ts
import { User, UserRole, isValidUser } from '../models/user.model';
import { UserService } from '../services/user.service';

describe('UserService', () => {
    let service: UserService;

    beforeEach(() => {
        service = new UserService();
    });

    it('should save and retrieve user', async () => {
        const user: User = {
            id: '123',
            name: 'Test User',
            email: 'test@example.com',
            role: UserRole.User
        };

        await service.saveUser(user);
        const retrieved = await service.getUser(user.id);
        expect(retrieved).toEqual(user);
    });

    it('should validate user', () => {
        const validUser: User = {
            id: '456',
            name: 'Valid User',
            email: 'valid@example.com',
            role: UserRole.Admin
        };

        expect(isValidUser(validUser)).toBe(true);
    });
});
"#;
    fs::write(project_root.join("tests/user.test.ts"), test_content).unwrap();

    project_root.to_path_buf()
}

#[test]
#[ignore] // Run with: cargo test typescript_references -- --ignored --nocapture
fn test_typescript_interface_references() {
    ensure_typescript_lsp();

    let temp_dir = TempDir::new().unwrap();
    let project_root = create_typescript_test_project(&temp_dir);

    println!("🔍 Testing TypeScript interface references...");

    // User インターフェースの参照を検索
    let references = find_all_references(&project_root, "User", &SymbolKind::Interface).unwrap();

    println!("Found {} references for 'User' interface", references.len());

    // 参照を分類
    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

    // 期待される結果を検証
    assert_eq!(
        definitions.len(),
        1,
        "Expected 1 definition of User interface"
    );
    assert!(
        usages.len() >= 10,
        "Expected at least 10 usages of User interface, got {}",
        usages.len()
    );

    // 各ファイルでの使用を確認
    let main_refs: Vec<_> = references
        .iter()
        .filter(|r| r.symbol.file_path.ends_with("main.ts"))
        .collect();
    let service_refs: Vec<_> = references
        .iter()
        .filter(|r| r.symbol.file_path.ends_with("user.service.ts"))
        .collect();
    let test_refs: Vec<_> = references
        .iter()
        .filter(|r| r.symbol.file_path.ends_with("user.test.ts"))
        .collect();

    assert!(main_refs.len() >= 2, "Expected User references in main.ts");
    assert!(
        service_refs.len() >= 3,
        "Expected User references in user.service.ts"
    );
    assert!(
        test_refs.len() >= 3,
        "Expected User references in user.test.ts"
    );

    println!("✅ Interface references test passed");
}

#[test]
#[ignore]
fn test_typescript_class_references() {
    ensure_typescript_lsp();

    let temp_dir = TempDir::new().unwrap();
    let project_root = create_typescript_test_project(&temp_dir);

    println!("🔍 Testing TypeScript class references...");

    // UserService クラスの参照を検索
    let references = find_all_references(&project_root, "UserService", &SymbolKind::Class).unwrap();

    println!(
        "Found {} references for 'UserService' class",
        references.len()
    );

    // デバッグ用：各参照を出力
    for (i, ref_item) in references.iter().enumerate() {
        println!(
            "  {} {} at {}:{}:{} (definition: {})",
            i + 1,
            ref_item.symbol.name,
            ref_item.symbol.file_path,
            ref_item.symbol.range.start.line + 1,
            ref_item.symbol.range.start.character + 1,
            ref_item.is_definition
        );
    }

    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

    assert_eq!(
        definitions.len(),
        1,
        "Expected 1 definition of UserService class"
    );
    assert!(
        usages.len() >= 4,
        "Expected at least 4 usages of UserService class"
    );

    // import文での使用を確認
    let import_refs: Vec<_> = references
        .iter()
        .filter(|r| {
            let line = r.symbol.range.start.line as usize;
            r.symbol.file_path.ends_with(".ts") && line < 5 // 通常import文は最初の5行以内
        })
        .collect();

    assert!(
        import_refs.len() >= 2,
        "Expected UserService imports in multiple files"
    );

    println!("✅ Class references test passed");
}

#[test]
#[ignore]
fn test_typescript_function_references() {
    ensure_typescript_lsp();

    let temp_dir = TempDir::new().unwrap();
    let project_root = create_typescript_test_project(&temp_dir);

    println!("🔍 Testing TypeScript function references...");

    // isValidUser 関数の参照を検索
    let references =
        find_all_references(&project_root, "isValidUser", &SymbolKind::Function).unwrap();

    println!(
        "Found {} references for 'isValidUser' function",
        references.len()
    );

    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

    assert_eq!(
        definitions.len(),
        1,
        "Expected 1 definition of isValidUser function"
    );
    assert!(
        usages.len() >= 3,
        "Expected at least 3 usages of isValidUser function"
    );

    // service内での使用を確認
    let service_usage = usages
        .iter()
        .any(|r| r.symbol.file_path.ends_with("user.service.ts"));
    assert!(
        service_usage,
        "Expected isValidUser to be used in user.service.ts"
    );

    println!("✅ Function references test passed");
}

#[test]
#[ignore]
fn test_typescript_enum_references() {
    ensure_typescript_lsp();

    let temp_dir = TempDir::new().unwrap();
    let project_root = create_typescript_test_project(&temp_dir);

    println!("🔍 Testing TypeScript enum references...");

    // UserRole enumの参照を検索
    let references = find_all_references(&project_root, "UserRole", &SymbolKind::Enum).unwrap();

    println!("Found {} references for 'UserRole' enum", references.len());

    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

    assert_eq!(
        definitions.len(),
        1,
        "Expected 1 definition of UserRole enum"
    );
    assert!(
        usages.len() >= 5,
        "Expected at least 5 usages of UserRole enum"
    );

    // enum値の使用も確認
    let enum_member_refs =
        find_all_references(&project_root, "Admin", &SymbolKind::EnumMember).unwrap();

    assert!(
        !enum_member_refs.is_empty(),
        "Expected references to UserRole.Admin"
    );

    println!("✅ Enum references test passed");
}

#[test]
#[ignore]
fn test_typescript_method_references() {
    ensure_typescript_lsp();

    let temp_dir = TempDir::new().unwrap();
    let project_root = create_typescript_test_project(&temp_dir);

    println!("🔍 Testing TypeScript method references...");

    // saveUser メソッドの参照を検索
    let references = find_all_references(&project_root, "saveUser", &SymbolKind::Method).unwrap();

    println!(
        "Found {} references for 'saveUser' method",
        references.len()
    );

    let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
    let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

    assert_eq!(
        definitions.len(),
        1,
        "Expected 1 definition of saveUser method"
    );
    assert!(
        usages.len() >= 2,
        "Expected at least 2 usages of saveUser method"
    );

    // 各ファイルでの使用を確認
    let main_usage = usages
        .iter()
        .any(|r| r.symbol.file_path.ends_with("main.ts"));
    let test_usage = usages
        .iter()
        .any(|r| r.symbol.file_path.ends_with("user.test.ts"));

    assert!(main_usage, "Expected saveUser to be called in main.ts");
    assert!(test_usage, "Expected saveUser to be called in tests");

    println!("✅ Method references test passed");
}

#[test]
#[ignore]
fn test_typescript_import_export_references() {
    ensure_typescript_lsp();

    let temp_dir = TempDir::new().unwrap();
    let project_root = create_typescript_test_project(&temp_dir);

    println!("🔍 Testing TypeScript import/export references...");

    // createUser関数（main.ts内で定義され、export されていない）
    let create_user_refs =
        find_all_references(&project_root, "createUser", &SymbolKind::Function).unwrap();

    println!(
        "Found {} references for 'createUser' function",
        create_user_refs.len()
    );

    // main.ts内でのみ使用されているはず
    let all_in_main = create_user_refs
        .iter()
        .all(|r| r.symbol.file_path.ends_with("main.ts"));

    assert!(
        all_in_main,
        "createUser should only be referenced in main.ts"
    );
    assert_eq!(
        create_user_refs.len(),
        2,
        "Expected exactly 2 references (1 definition + 1 usage)"
    );

    println!("✅ Import/export references test passed");
}

#[test]
#[ignore]
fn test_typescript_lsp_integration() {
    ensure_typescript_lsp();

    let temp_dir = TempDir::new().unwrap();
    let project_root = create_typescript_test_project(&temp_dir);

    println!("🔍 Testing TypeScript LSP integration for references...");

    // LSPクライアントを使用してシンボルを取得
    let adapter = TypeScriptAdapter;
    let mut client =
        GenericLspClient::new(Box::new(adapter)).expect("Failed to create TypeScript LSP client");

    // main.tsファイルのURIを構築
    let main_file = project_root.join("main.ts");
    let abs_path = std::fs::canonicalize(&main_file).unwrap();
    let file_uri = format!("file://{}", abs_path.display());

    // ドキュメントシンボルを取得
    let symbols = client
        .get_document_symbols(&file_uri)
        .expect("Failed to get document symbols");

    println!("LSP found {} symbols in main.ts", symbols.len());

    // 参照検索と比較
    let references =
        find_all_references(&project_root, "createUser", &SymbolKind::Function).unwrap();

    // LSPで見つかったcreateUserシンボルがあることを確認
    let create_user_symbol = symbols.iter().any(|s| s.name == "createUser");

    assert!(create_user_symbol, "LSP should find createUser symbol");
    assert!(
        !references.is_empty(),
        "Reference finder should find createUser references"
    );

    client.shutdown().expect("Failed to shutdown LSP");

    println!("✅ LSP integration test passed");
}
