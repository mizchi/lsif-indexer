use anyhow::Result;
use cli::differential_indexer::DifferentialIndexer;
use cli::git_diff::GitDiffDetector;
use cli::storage::IndexStorage;
use git2::{Oid, Repository, Signature};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Git巻き戻しシナリオのテスト用ヘルパー
struct GitTestProject {
    temp_dir: TempDir,
    repo: Repository,
    db_path: std::path::PathBuf,
}

impl GitTestProject {
    fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let repo = Repository::init(temp_dir.path())?;

        // Git設定
        let mut config = repo.config()?;
        config.set_str("user.name", "Test User")?;
        config.set_str("user.email", "test@example.com")?;

        let db_path = temp_dir.path().join("index.db");

        Ok(Self {
            temp_dir,
            repo,
            db_path,
        })
    }

    fn commit_file(&self, file_name: &str, content: &str, message: &str) -> Result<Oid> {
        let file_path = self.temp_dir.path().join(file_name);
        fs::write(&file_path, content)?;

        let mut index = self.repo.index()?;
        index.add_path(Path::new(file_name))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;
        let sig = Signature::now("Test User", "test@example.com")?;

        let parent_commit = if self.repo.is_empty()? {
            vec![]
        } else {
            vec![self.repo.head()?.peel_to_commit()?]
        };

        let parent_refs: Vec<&git2::Commit> = parent_commit.iter().collect();

        let oid = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;

        Ok(oid)
    }

    fn checkout(&self, commit: Oid) -> Result<()> {
        let commit_obj = self.repo.find_commit(commit)?;
        let tree = commit_obj.tree()?;

        self.repo.checkout_tree(
            tree.as_object(),
            Some(git2::build::CheckoutBuilder::new().force()),
        )?;

        self.repo.set_head_detached(commit)?;

        Ok(())
    }
}

#[test]
#[ignore] // DifferentialIndexerが必要
fn test_git_rollback_and_restore() -> Result<()> {
    let project = GitTestProject::new()?;

    // バージョン1のコード
    let v1_commit = project.commit_file(
        "main.rs",
        r#"
        fn main() {
            println!("Version 1");
        }
        
        fn feature_a() {
            // Feature A implementation
        }
    "#,
        "Version 1",
    )?;

    // 初回インデックス
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result1 = indexer.index_differential()?;
    assert_eq!(result1.files_added, 1);
    assert!(result1.symbols_added >= 2); // main, feature_a

    // バージョン2のコード（feature_aを変更、feature_bを追加）
    let v2_commit = project.commit_file(
        "main.rs",
        r#"
        fn main() {
            println!("Version 2");
        }
        
        fn feature_a() {
            // Feature A modified
            println!("Modified");
        }
        
        fn feature_b() {
            // New feature B
        }
    "#,
        "Version 2",
    )?;

    // 差分インデックス
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let _result2 = indexer.index_differential()?;

    // メタデータを確認（v2のハッシュが保存されている）
    let storage = IndexStorage::open(&project.db_path)?;
    let metadata_v2 = storage.load_metadata()?.unwrap();
    assert_eq!(metadata_v2.git_commit_hash, Some(v2_commit.to_string()));

    // バージョン1に巻き戻し
    project.checkout(v1_commit)?;

    // リストア（ハッシュに基づいて変更を検出）
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result3 = indexer.index_differential()?;

    // ファイルが変更されたことが検出される
    assert_eq!(result3.files_modified, 1);

    // メタデータが更新される
    let metadata_v1_restored = storage.load_metadata()?.unwrap();
    assert_eq!(
        metadata_v1_restored.git_commit_hash,
        Some(v1_commit.to_string())
    );

    Ok(())
}

#[test]
#[ignore] // DifferentialIndexerが必要
fn test_content_hash_based_detection() -> Result<()> {
    let project = GitTestProject::new()?;

    // 初期ファイルをコミット
    project.commit_file("file1.rs", "fn func1() {}", "Initial")?;
    project.commit_file("file2.rs", "fn func2() {}", "Add file2")?;

    // インデックス作成
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    indexer.index_differential()?;

    // メタデータを取得
    let storage = IndexStorage::open(&project.db_path)?;
    let metadata1 = storage.load_metadata()?.unwrap();
    let initial_hashes = metadata1.file_hashes.clone();

    // file1を変更（コミットなし）
    fs::write(
        project.temp_dir.path().join("file1.rs"),
        "fn func1_modified() {}",
    )?;

    // 差分検出
    let detector = GitDiffDetector::new(project.temp_dir.path())?;
    let file1_path = project.temp_dir.path().join("file1.rs");
    let new_hash = detector.calculate_file_hash(&file1_path)?;

    // ハッシュが変わったことを確認
    let old_hash = initial_hashes
        .iter()
        .find(|(k, _)| k.ends_with("file1.rs"))
        .map(|(_, v)| v);

    assert!(old_hash.is_some());
    assert_ne!(old_hash.unwrap(), &new_hash);

    // 再インデックスで変更が検出される
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result = indexer.index_differential()?;
    assert_eq!(result.files_modified, 1);

    Ok(())
}

#[test]
#[ignore] // DifferentialIndexerが必要
fn test_mixed_git_and_untracked_files() -> Result<()> {
    let project = GitTestProject::new()?;

    // Gitで管理されるファイル
    project.commit_file("tracked.rs", "fn tracked() {}", "Add tracked file")?;

    // インデックス作成
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    indexer.index_differential()?;

    // Git管理外のファイルを追加
    fs::write(
        project.temp_dir.path().join("untracked.rs"),
        "fn untracked() {}",
    )?;

    // 差分インデックス
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result = indexer.index_differential()?;

    // 新規ファイルとして検出
    assert_eq!(result.files_added, 1);

    // メタデータにハッシュが保存される
    let storage = IndexStorage::open(&project.db_path)?;
    let metadata = storage.load_metadata()?.unwrap();
    assert!(metadata
        .file_hashes
        .iter()
        .any(|(k, _)| k.ends_with("untracked.rs")));

    // untrackedファイルを変更
    fs::write(
        project.temp_dir.path().join("untracked.rs"),
        "fn untracked_modified() {}",
    )?;

    // 再度差分インデックス
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result = indexer.index_differential()?;

    // 変更として検出される
    assert_eq!(result.files_modified, 1);

    Ok(())
}

#[test]
#[ignore] // DifferentialIndexerが必要
fn test_restore_after_branch_switch() -> Result<()> {
    let project = GitTestProject::new()?;

    // mainブランチでファイルを作成
    project.commit_file(
        "main_feature.rs",
        r#"
        fn main_feature() {
            println!("Main branch feature");
        }
    "#,
        "Main branch feature",
    )?;

    // インデックス作成
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    indexer.index_differential()?;

    // 新しいブランチを作成
    let main_commit = project.repo.head()?.target().unwrap();
    project
        .repo
        .branch("feature", &project.repo.find_commit(main_commit)?, false)?;

    // featureブランチにチェックアウト
    project.repo.set_head("refs/heads/feature")?;
    project
        .repo
        .checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;

    // featureブランチで異なるファイルを作成
    project.commit_file(
        "feature_specific.rs",
        r#"
        fn feature_specific() {
            println!("Feature branch");
        }
    "#,
        "Feature branch file",
    )?;

    // 差分インデックス
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result = indexer.index_differential()?;
    assert_eq!(result.files_added, 1);

    // mainブランチに戻る
    project.repo.set_head("refs/heads/main")?;
    project
        .repo
        .checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;

    // リストア（feature_specific.rsが削除されたことを検出）
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result = indexer.index_differential()?;
    assert_eq!(result.files_deleted, 1);

    Ok(())
}

#[test]
#[ignore] // DifferentialIndexerが必要
fn test_performance_with_many_files() -> Result<()> {
    let project = GitTestProject::new()?;

    // 多数のファイルを作成
    for i in 0..50 {
        let content = format!("fn func_{i}() {{ /* content */ }}");
        fs::write(
            project.temp_dir.path().join(format!("file_{i}.rs")),
            content,
        )?;
    }

    // 初回インデックス時間を測定
    let start = std::time::Instant::now();
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result = indexer.index_differential()?;
    let initial_duration = start.elapsed();

    assert_eq!(result.files_added, 50);
    println!("Initial indexing of 50 files: {initial_duration:?}");

    // 1ファイルだけ変更
    fs::write(
        project.temp_dir.path().join("file_0.rs"),
        "fn func_0_modified() { /* modified */ }",
    )?;

    // 差分インデックス時間を測定
    let start = std::time::Instant::now();
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result = indexer.index_differential()?;
    let diff_duration = start.elapsed();

    assert_eq!(result.files_modified, 1);
    println!("Differential indexing of 1 changed file: {diff_duration:?}");

    // 差分インデックスの方が圧倒的に速いことを確認
    assert!(diff_duration < initial_duration / 10);

    Ok(())
}

#[test]
#[ignore] // DifferentialIndexerが必要
fn test_hash_collision_resistance() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let detector = GitDiffDetector::new(temp_dir.path())?;

    // 似たような内容でもハッシュが異なることを確認
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");

    fs::write(&file1, "Hello, World!")?;
    fs::write(&file2, "Hello, World!!")?; // 1文字追加

    let hash1 = detector.calculate_file_hash(&file1)?;
    let hash2 = detector.calculate_file_hash(&file2)?;

    assert_ne!(hash1, hash2);

    // 同じ内容なら同じハッシュ
    let file3 = temp_dir.path().join("file3.txt");
    fs::write(&file3, "Hello, World!")?;
    let hash3 = detector.calculate_file_hash(&file3)?;

    assert_eq!(hash1, hash3);

    Ok(())
}

#[test]
#[ignore] // DifferentialIndexerが必要
fn test_metadata_compatibility() -> Result<()> {
    let project = GitTestProject::new()?;

    // ファイルを作成
    project.commit_file("test.rs", "fn test() {}", "Initial")?;

    // インデックス作成
    let mut indexer = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    indexer.index_differential()?;

    // メタデータを読み込み
    let storage = IndexStorage::open(&project.db_path)?;
    let metadata = storage.load_metadata()?.unwrap();

    // 必須フィールドが存在することを確認
    assert!(metadata.git_commit_hash.is_some());
    assert!(!metadata.file_hashes.is_empty());
    assert_eq!(metadata.files_count, 1);
    assert!(metadata.symbols_count > 0);

    // メタデータを手動で更新（互換性テスト）
    let mut modified_metadata = metadata.clone();
    modified_metadata.git_commit_hash = Some("dummy_hash".to_string());
    storage.save_metadata(&modified_metadata)?;

    // 新しいインデクサーで読み込めることを確認
    let mut indexer2 = DifferentialIndexer::new(&project.db_path, project.temp_dir.path())?;
    let result = indexer2.index_differential()?;

    // Git hashが変わったので再インデックスされる
    assert!(result.files_modified > 0 || result.files_added > 0);

    Ok(())
}
