use anyhow::Result;
use cli::differential_indexer::DifferentialIndexer;
use cli::storage::IndexStorage;
use git2::{Repository, Signature};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// テスト用のプロジェクト構造を作成
fn create_test_project(dir: &Path) -> Result<()> {
    // Rustファイルを作成
    fs::write(
        dir.join("main.rs"),
        r#"
        fn main() {
            println!("Hello, world!");
            helper();
        }
        
        fn helper() {
            // Helper function
        }
    "#,
    )?;

    fs::write(
        dir.join("lib.rs"),
        r#"
        pub struct Config {
            pub name: String,
            pub value: i32,
        }
        
        impl Config {
            pub fn new(name: String, value: i32) -> Self {
                Config { name, value }
            }
            
            pub fn get_name(&self) -> &str {
                &self.name
            }
        }
    "#,
    )?;

    // TypeScriptファイルも作成
    fs::write(
        dir.join("index.ts"),
        r#"
        export class Service {
            private name: string;
            
            constructor(name: string) {
                this.name = name;
            }
            
            public getName(): string {
                return this.name;
            }
        }
        
        export function createService(name: string): Service {
            return new Service(name);
        }
    "#,
    )?;

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_initial_indexing() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    // テストプロジェクトを作成
    create_test_project(temp_dir.path())?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer.index_differential()?;

    // 新規ファイルがすべて追加されたことを確認
    assert_eq!(result.files_added, 3); // main.rs, lib.rs, index.ts
    assert!(result.symbols_added > 0);
    assert_eq!(result.files_modified, 0);
    assert_eq!(result.files_deleted, 0);

    // メタデータが保存されたことを確認
    let storage = IndexStorage::open(&db_path)?;
    let metadata = storage.load_metadata()?;
    assert!(metadata.is_some());

    let metadata = metadata.unwrap();
    assert_eq!(metadata.files_count, 3);
    assert!(metadata.symbols_count > 0);
    assert!(!metadata.file_hashes.is_empty());

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_incremental_indexing_no_changes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    create_test_project(temp_dir.path())?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    indexer.index_differential()?;

    // 変更なしで再インデックス
    let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer2.index_differential()?;

    // 変更がないことを確認
    assert_eq!(result.files_added, 0);
    assert_eq!(result.files_modified, 0);
    assert_eq!(result.files_deleted, 0);
    assert_eq!(result.symbols_added, 0);

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_incremental_indexing_with_modifications() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    create_test_project(temp_dir.path())?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    indexer.index_differential()?;

    // ファイルを変更
    fs::write(
        temp_dir.path().join("main.rs"),
        r#"
        fn main() {
            println!("Modified!");
            new_helper();
        }
        
        fn new_helper() {
            // New helper
        }
        
        fn another_function() {
            // Added function
        }
    "#,
    )?;

    // 再インデックス
    let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer2.index_differential()?;

    // 変更が検出されたことを確認
    assert_eq!(result.files_modified, 1);
    assert!(result.symbols_updated > 0);

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_file_deletion_handling() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    create_test_project(temp_dir.path())?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    indexer.index_differential()?;

    // ファイルを削除
    fs::remove_file(temp_dir.path().join("index.ts"))?;

    // 再インデックス
    let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer2.index_differential()?;

    // 削除が検出されたことを確認
    assert_eq!(result.files_deleted, 1);
    assert!(result.symbols_deleted > 0);

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_new_file_addition() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    create_test_project(temp_dir.path())?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    indexer.index_differential()?;

    // 新しいファイルを追加
    fs::write(
        temp_dir.path().join("new_module.rs"),
        r#"
        pub mod new_module {
            pub fn new_function() {
                println!("New module");
            }
            
            pub struct NewStruct {
                field: String,
            }
            
            impl NewStruct {
                pub fn new() -> Self {
                    NewStruct { field: String::new() }
                }
            }
        }
    "#,
    )?;

    // 再インデックス
    let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer2.index_differential()?;

    // 新規ファイルが追加されたことを確認
    assert_eq!(result.files_added, 1);
    assert!(result.symbols_added >= 3); // new_function, NewStruct, impl NewStruct

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_full_reindex() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    create_test_project(temp_dir.path())?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result1 = indexer.index_differential()?;

    // フルリインデックス
    let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result2 = indexer2.full_reindex()?;

    // フルリインデックスですべてのファイルが再処理されたことを確認
    assert_eq!(result2.files_added, result1.files_added);
    assert_eq!(result2.symbols_added, result1.symbols_added);

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_git_integration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    // Gitリポジトリを初期化
    let repo = Repository::init(temp_dir.path())?;
    let mut config = repo.config()?;
    config.set_str("user.name", "Test")?;
    config.set_str("user.email", "test@test.com")?;

    create_test_project(temp_dir.path())?;

    // ファイルをコミット
    let mut index = repo.index()?;
    index.add_path(Path::new("main.rs"))?;
    index.add_path(Path::new("lib.rs"))?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = Signature::now("Test", "test@test.com")?;

    let initial_commit = repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    indexer.index_differential()?;

    // メタデータにGitコミットハッシュが保存されたことを確認
    let storage = IndexStorage::open(&db_path)?;
    let metadata = storage.load_metadata()?.unwrap();
    assert_eq!(metadata.git_commit_hash, Some(initial_commit.to_string()));

    // ファイルを変更してコミット
    fs::write(
        temp_dir.path().join("main.rs"),
        "fn main() { /* changed */ }",
    )?;
    index.add_path(Path::new("main.rs"))?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let parent = repo.find_commit(initial_commit)?;

    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Update main.rs",
        &tree,
        &[&parent],
    )?;

    // 差分インデックス
    let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer2.index_differential()?;

    // 変更が検出されない（既にコミット済みのため）
    assert_eq!(result.files_modified, 0);

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_metadata_persistence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    create_test_project(temp_dir.path())?;

    // 初回インデックス
    {
        let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
        indexer.index_differential()?;
    }

    // メタデータを読み込み
    let storage = IndexStorage::open(&db_path)?;
    let metadata = storage.load_metadata()?.unwrap();

    // メタデータの内容を検証
    assert!(!metadata.file_hashes.is_empty());
    assert_eq!(metadata.files_count, 3);
    assert!(metadata.symbols_count > 0);

    // ファイルハッシュが正しく保存されていることを確認
    for file_path in metadata.file_hashes.keys() {
        let path = Path::new(file_path);
        let file_name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            file_name == "main.rs"
                || file_name == "lib.rs"
                || file_name == "index.ts"
                || file_name.ends_with(".rs")
                || file_name.ends_with(".ts")
        );
    }

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_symbol_extraction_rust() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    fs::write(
        temp_dir.path().join("advanced.rs"),
        r#"
        pub struct MyStruct {
            field: String,
        }
        
        impl MyStruct {
            pub fn new() -> Self {
                MyStruct { field: String::new() }
            }
            
            fn private_method(&self) {
                println!("{}", self.field);
            }
        }
        
        pub fn public_function() {
            let s = MyStruct::new();
        }
        
        fn private_function() {
            // Private
        }
        
        pub trait MyTrait {
            fn trait_method(&self);
        }
        
        impl MyTrait for MyStruct {
            fn trait_method(&self) {
                // Implementation
            }
        }
    "#,
    )?;

    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer.index_differential()?;

    // 複数のシンボルが抽出されたことを確認
    // MyStruct, impl MyStruct, new, private_method, public_function,
    // private_function, MyTrait, impl MyTrait for MyStruct
    assert!(result.symbols_added >= 7);

    Ok(())
}

#[test]
#[ignore] // Differentialインデックサーの実装が必要
fn test_symbol_extraction_typescript() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    fs::write(
        temp_dir.path().join("advanced.ts"),
        r#"
        export class BaseClass {
            protected value: number;
            
            constructor(value: number) {
                this.value = value;
            }
        }
        
        export class DerivedClass extends BaseClass {
            private name: string;
            
            constructor(value: number, name: string) {
                super(value);
                this.name = name;
            }
            
            public getName(): string {
                return this.name;
            }
        }
        
        export function helperFunction(): void {
            console.log("Helper");
        }
        
        function privateFunction() {
            // Not exported
        }
        
        export interface MyInterface {
            method(): void;
        }
    "#,
    )?;

    let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
    let result = indexer.index_differential()?;

    // 複数のクラスと関数が抽出されたことを確認
    // BaseClass, DerivedClass, helperFunction, privateFunction
    assert!(result.symbols_added >= 4);

    Ok(())
}
