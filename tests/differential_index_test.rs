use anyhow::Result;
use cli::differential_indexer::DifferentialIndexer;
use cli::git_diff::{FileChangeStatus, GitDiffDetector};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[cfg(test)]
mod git_diff_tests {
    use super::*;
    use git2::{Repository, Signature};

    fn create_test_repo(path: &Path) -> Result<Repository> {
        let repo = Repository::init(path)?;

        // Configure git user
        let sig = Signature::now("Test User", "test@example.com")?;

        // Create initial commit
        let tree_id = {
            let mut index = repo.index()?;
            index.write_tree()?
        };

        {
            let tree = repo.find_tree(tree_id)?;
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;
        }

        Ok(repo)
    }

    #[test]
    fn test_detect_new_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let _repo = create_test_repo(temp_dir.path())?;

        // Add a new file
        let file_path = temp_dir.path().join("new_file.rs");
        fs::write(&file_path, "fn main() {}")?;

        let mut detector = GitDiffDetector::new(temp_dir.path())?;
        let changes = detector.detect_changes_since(None)?;

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, FileChangeStatus::Added);
        assert!(changes[0].content_hash.is_some());

        Ok(())
    }

    #[test]
    #[ignore] // Differentialインデックサーの実装が必要
    fn test_detect_modified_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let repo = create_test_repo(temp_dir.path())?;

        // Create and commit a file
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn old() {}")?;

        let sig = Signature::now("Test User", "test@example.com")?;
        let mut index = repo.index()?;
        index.add_path(Path::new("test.rs"))?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let parent = repo.head()?.peel_to_commit()?;

        repo.commit(Some("HEAD"), &sig, &sig, "Add test file", &tree, &[&parent])?;

        // Modify the file
        fs::write(&file_path, "fn new() {}")?;

        let mut detector = GitDiffDetector::new(temp_dir.path())?;
        let changes = detector.detect_changes_since(None)?;

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, FileChangeStatus::Modified);

        Ok(())
    }

    #[test]
    fn test_content_hash_detection() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create files without git
        fs::write(temp_dir.path().join("file1.rs"), "content1")?;
        fs::write(temp_dir.path().join("file2.rs"), "content2")?;

        let mut detector = GitDiffDetector::new(temp_dir.path())?;

        // First detection - all files are new
        let changes1 = detector.detect_changes_since(None)?;
        assert_eq!(changes1.len(), 2);

        // Save and reload hash cache
        let cache_path = temp_dir.path().join("hash_cache.json");
        detector.save_hash_cache(&cache_path)?;

        let mut detector2 = GitDiffDetector::new(temp_dir.path())?;
        detector2.load_hash_cache(&cache_path)?;

        // Second detection - no changes
        let changes2 = detector2.detect_changes_since(None)?;
        assert_eq!(changes2.len(), 0);

        // Modify a file
        fs::write(temp_dir.path().join("file1.rs"), "new content")?;

        let changes3 = detector2.detect_changes_since(None)?;
        assert_eq!(changes3.len(), 1);
        assert_eq!(changes3[0].status, FileChangeStatus::Modified);

        Ok(())
    }
}

#[cfg(test)]
mod differential_indexer_tests {
    use super::*;

    #[test]
    #[ignore] // Differentialインデックサーの実装が必要
    fn test_differential_index_new_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Create test files
        fs::write(
            temp_dir.path().join("main.rs"),
            r#"
            fn main() {
                println!("Hello");
            }
            
            fn helper() {
                // Helper function
            }
        "#,
        )?;

        fs::write(
            temp_dir.path().join("lib.rs"),
            r#"
            pub struct Config {
                name: String,
            }
            
            impl Config {
                pub fn new(name: String) -> Self {
                    Config { name }
                }
            }
        "#,
        )?;

        // Create indexer
        let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;

        // Perform initial index
        let result = indexer.index_differential()?;

        assert_eq!(result.files_added, 2);
        assert!(result.symbols_added > 0);
        assert_eq!(result.files_modified, 0);
        assert_eq!(result.files_deleted, 0);

        Ok(())
    }

    #[test]
    #[ignore] // Differentialインデックサーの実装が必要
    fn test_differential_index_modified_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Create initial file
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn old() {}")?;

        // Initial index
        let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
        let result1 = indexer.index_differential()?;
        assert_eq!(result1.files_added, 1);
        assert_eq!(result1.symbols_added, 1);

        // Modify file
        fs::write(&file_path, "fn new() {}\nfn another() {}")?;

        // Re-index
        let mut indexer2 = DifferentialIndexer::new(&db_path, temp_dir.path())?;
        let result2 = indexer2.index_differential()?;

        assert_eq!(result2.files_modified, 1);
        assert!(result2.symbols_updated > 0);

        Ok(())
    }

    #[test]
    #[ignore] // Differentialインデックサーの実装が必要
    fn test_full_reindex() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Create files
        fs::write(temp_dir.path().join("file1.rs"), "fn func1() {}")?;
        fs::write(temp_dir.path().join("file2.rs"), "fn func2() {}")?;

        let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;

        // Initial index
        let result1 = indexer.index_differential()?;
        assert_eq!(result1.files_added, 2);

        // Full reindex (should process all files again)
        let result2 = indexer.full_reindex()?;
        assert_eq!(result2.files_added, 2);

        Ok(())
    }

    #[test]
    #[ignore] // Differentialインデックサーの実装が必要
    fn test_rust_symbol_extraction() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        let rust_code = r#"
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
"#;

        fs::write(temp_dir.path().join("test.rs"), rust_code)?;

        let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
        let result = indexer.index_differential()?;

        // Should find: MyStruct, impl MyStruct, new, private_method, public_function, private_function
        assert!(result.symbols_added >= 6);

        Ok(())
    }

    #[test]
    #[ignore] // Differentialインデックサーの実装が必要
    fn test_typescript_symbol_extraction() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        let ts_code = r#"
export class MyClass {
    private field: string;
    
    constructor() {
        this.field = "";
    }
    
    public method(): void {
        console.log(this.field);
    }
}

export function myFunction(): void {
    const instance = new MyClass();
}

function privateFunction() {
    // Private
}
"#;

        fs::write(temp_dir.path().join("test.ts"), ts_code)?;

        let mut indexer = DifferentialIndexer::new(&db_path, temp_dir.path())?;
        let result = indexer.index_differential()?;

        // Should find: MyClass, myFunction, privateFunction
        assert!(result.symbols_added >= 3);

        Ok(())
    }
}
