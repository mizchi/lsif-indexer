use anyhow::Result;
use cli::reference_finder::find_all_references;
use lsif_core::SymbolKind;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// テスト用のTypeScriptプロジェクトを作成
fn create_typescript_test_project(temp_dir: &TempDir) -> PathBuf {
    let project_root = temp_dir.path();

    // tsconfig.json
    fs::write(
        project_root.join("tsconfig.json"),
        r#"{
    "compilerOptions": {
        "target": "ES2020",
        "module": "commonjs",
        "lib": ["ES2020"],
        "strict": true,
        "esModuleInterop": true,
        "skipLibCheck": true
    },
    "include": ["src/**/*"]
}"#,
    )
    .unwrap();

    // src/types/user.ts
    let types_dir = project_root.join("src/types");
    fs::create_dir_all(&types_dir).unwrap();
    fs::write(
        types_dir.join("user.ts"),
        r#"export interface User {
    id: number;
    name: string;
    email: string;
    role: Role;
}

export enum Role {
    Admin = "admin",
    User = "user",
    Guest = "guest"
}

export type UserId = number;
"#,
    )
    .unwrap();

    // src/services/user.service.ts
    let services_dir = project_root.join("src/services");
    fs::create_dir_all(&services_dir).unwrap();
    fs::write(
        services_dir.join("user.service.ts"),
        r#"import { User, Role, UserId } from '../types/user';

export class UserService {
    private users: Map<UserId, User> = new Map();

    constructor() {
        this.initializeTestData();
    }

    private initializeTestData(): void {
        this.users.set(1, {
            id: 1,
            name: "Admin User",
            email: "admin@example.com",
            role: Role.Admin
        });
    }

    getUser(id: UserId): User | undefined {
        return this.users.get(id);
    }

    getAllUsers(): User[] {
        return Array.from(this.users.values());
    }

    createUser(user: User): User {
        this.users.set(user.id, user);
        return user;
    }

    updateUser(id: UserId, updates: Partial<User>): User | undefined {
        const user = this.users.get(id);
        if (user) {
            const updated = { ...user, ...updates };
            this.users.set(id, updated);
            return updated;
        }
        return undefined;
    }

    deleteUser(id: UserId): boolean {
        return this.users.delete(id);
    }

    getUsersByRole(role: Role): User[] {
        return this.getAllUsers().filter(user => user.role === role);
    }
}

export function getUser(id: UserId): User | undefined {
    const service = new UserService();
    return service.getUser(id);
}
"#,
    )
    .unwrap();

    // src/main.ts
    fs::write(
        project_root.join("src/main.ts"),
        r#"import { UserService } from './services/user.service';
import { User, Role } from './types/user';
import { getUser } from './services/user.service';

const service = new UserService();

// Create a new user
const newUser: User = {
    id: 2,
    name: "Test User",
    email: "test@example.com",
    role: Role.User
};

service.createUser(newUser);

// Get all users
const allUsers = service.getAllUsers();
console.log('All users:', allUsers);

// Get user by ID
const user = getUser(1);
if (user) {
    console.log('Found user:', user.name);
}

// Get users by role
const admins = service.getUsersByRole(Role.Admin);
console.log('Admins:', admins);
"#,
    )
    .unwrap();

    // src/tests/user.test.ts
    let tests_dir = project_root.join("src/tests");
    fs::create_dir_all(&tests_dir).unwrap();
    fs::write(
        tests_dir.join("user.test.ts"),
        r#"import { UserService } from '../services/user.service';
import { User, Role } from '../types/user';
import { getUser } from '../services/user.service';

describe('UserService', () => {
    let service: UserService;

    beforeEach(() => {
        service = new UserService();
    });

    test('should create a user', () => {
        const user: User = {
            id: 100,
            name: "Test User",
            email: "test@test.com",
            role: Role.User
        };

        const created = service.createUser(user);
        expect(created).toEqual(user);
    });

    test('should get user by id', () => {
        const user = service.getUser(1);
        expect(user).toBeDefined();
        expect(user?.role).toBe(Role.Admin);
    });

    test('should get all users', () => {
        const users = service.getAllUsers();
        expect(users.length).toBeGreaterThan(0);
    });

    test('getUser function should work', () => {
        const user = getUser(1);
        expect(user).toBeDefined();
    });
});
"#,
    )
    .unwrap();

    project_root.to_path_buf()
}

/// TypeScript参照テストの共通ヘルパー
pub struct TypeScriptReferenceTest {
    pub temp_dir: TempDir,
    pub project_root: PathBuf,
}

impl TypeScriptReferenceTest {
    /// 新しいテスト環境をセットアップ
    pub fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let project_root = create_typescript_test_project(&temp_dir);
        Self {
            temp_dir,
            project_root,
        }
    }

    /// シンボルの参照をテスト
    pub fn test_symbol_references(
        &self,
        symbol_name: &str,
        symbol_kind: &SymbolKind,
        expected_definitions: usize,
        min_usages: usize,
        file_expectations: Vec<(&str, usize)>,
    ) -> Result<()> {
        println!(
            "🔍 Testing TypeScript {} references for '{}'...",
            format!("{:?}", symbol_kind).to_lowercase(),
            symbol_name
        );

        // 参照を検索
        let references = find_all_references(&self.project_root, symbol_name, symbol_kind)?;
        println!(
            "Found {} references for '{}'",
            references.len(),
            symbol_name
        );

        // 参照を分類
        let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
        let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

        // 期待される結果を検証
        assert_eq!(
            definitions.len(),
            expected_definitions,
            "Expected {} definition(s) of {} '{}'",
            expected_definitions,
            format!("{:?}", symbol_kind).to_lowercase(),
            symbol_name
        );

        assert!(
            usages.len() >= min_usages,
            "Expected at least {} usages of {} '{}', got {}",
            min_usages,
            format!("{:?}", symbol_kind).to_lowercase(),
            symbol_name,
            usages.len()
        );

        // 各ファイルでの使用を確認
        for (file_suffix, expected_count) in file_expectations {
            let file_refs: Vec<_> = references
                .iter()
                .filter(|r| r.symbol.file_path.ends_with(file_suffix))
                .collect();

            assert!(
                file_refs.len() >= expected_count,
                "Expected at least {} references in {}, got {}",
                expected_count,
                file_suffix,
                file_refs.len()
            );
        }

        println!(
            "✅ {} references test passed",
            format!("{:?}", symbol_kind).to_lowercase()
        );
        Ok(())
    }

    /// 複数のシンボルをテスト
    pub fn test_multiple_symbols(&self, tests: Vec<SymbolTest>) -> Result<()> {
        for test in tests {
            self.test_symbol_references(
                &test.name,
                &test.kind,
                test.expected_definitions,
                test.min_usages,
                test.file_expectations,
            )?;
        }
        Ok(())
    }
}

/// シンボルテストの設定
pub struct SymbolTest {
    pub name: String,
    pub kind: SymbolKind,
    pub expected_definitions: usize,
    pub min_usages: usize,
    pub file_expectations: Vec<(&'static str, usize)>,
}

impl SymbolTest {
    pub fn new(
        name: &str,
        kind: SymbolKind,
        expected_definitions: usize,
        min_usages: usize,
    ) -> Self {
        Self {
            name: name.to_string(),
            kind,
            expected_definitions,
            min_usages,
            file_expectations: vec![],
        }
    }

    pub fn with_file_expectation(mut self, file: &'static str, count: usize) -> Self {
        self.file_expectations.push((file, count));
        self
    }
}
