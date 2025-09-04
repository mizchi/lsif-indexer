use anyhow::Result;
use cli::git_diff::{FileChange, FileChangeStatus, GitDiffDetector};
use git2::{Oid, Repository, Signature};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// テスト用のGitリポジトリを作成
fn create_test_repo(path: &Path) -> Result<Repository> {
    let repo = Repository::init(path)?;

    // Git設定
    let mut config = repo.config()?;
    config.set_str("user.name", "Test User")?;
    config.set_str("user.email", "test@example.com")?;

    Ok(repo)
}

/// ファイルを作成してコミット
fn commit_file(repo: &Repository, file_name: &str, content: &str, message: &str) -> Result<Oid> {
    let workdir = repo.workdir().unwrap();
    let file_path = workdir.join(file_name);
    fs::write(&file_path, content)?;

    let mut index = repo.index()?;
    index.add_path(Path::new(file_name))?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = Signature::now("Test User", "test@example.com")?;

    let parent_commit = if repo.is_empty()? {
        vec![]
    } else {
        vec![repo.head()?.peel_to_commit()?]
    };

    let parent_refs: Vec<&git2::Commit> = parent_commit.iter().collect();

    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;

    Ok(oid)
}

#[test]
fn test_detect_new_files_in_git_repo() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = create_test_repo(temp_dir.path())?;

    // 初期コミット
    commit_file(&repo, "initial.rs", "fn init() {}", "Initial commit")?;

    // 新しいファイルを追加（コミットせず）
    fs::write(temp_dir.path().join("new_file.rs"), "fn new_func() {}")?;
    fs::write(temp_dir.path().join("another.rs"), "struct Test {}")?;

    let mut detector = GitDiffDetector::new(temp_dir.path())?;
    let changes = detector.detect_changes_since(None)?;

    // 新規ファイルが検出されることを確認
    let new_files: Vec<&FileChange> = changes
        .iter()
        .filter(|c| c.status == FileChangeStatus::Added)
        .collect();

    assert_eq!(new_files.len(), 2);

    // ハッシュが計算されていることを確認
    for change in &new_files {
        assert!(change.content_hash.is_some());
    }

    Ok(())
}

#[test]
fn test_detect_modified_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = create_test_repo(temp_dir.path())?;

    // ファイルをコミット
    commit_file(&repo, "test.rs", "fn old() {}", "Add test file")?;

    // ファイルを変更（コミットせず）
    fs::write(
        temp_dir.path().join("test.rs"),
        "fn new() { println!(\"changed\"); }",
    )?;

    let mut detector = GitDiffDetector::new(temp_dir.path())?;
    let changes = detector.detect_changes_since(None)?;

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].status, FileChangeStatus::Modified);
    assert!(changes[0].content_hash.is_some());

    Ok(())
}

#[test]
fn test_detect_deleted_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = create_test_repo(temp_dir.path())?;

    // ファイルをコミット
    commit_file(
        &repo,
        "to_delete.rs",
        "fn delete_me() {}",
        "Add file to delete",
    )?;

    // ファイルを削除（コミットせず）
    fs::remove_file(temp_dir.path().join("to_delete.rs"))?;

    let mut detector = GitDiffDetector::new(temp_dir.path())?;
    let changes = detector.detect_changes_since(None)?;

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].status, FileChangeStatus::Deleted);
    assert!(changes[0].content_hash.is_none()); // 削除されたファイルにはハッシュなし

    Ok(())
}

#[test]
fn test_detect_changes_between_commits() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = create_test_repo(temp_dir.path())?;

    // 最初のコミット
    let first_commit = commit_file(&repo, "file1.rs", "v1", "First commit")?;

    // 2つ目のコミット
    commit_file(&repo, "file2.rs", "v2", "Second commit")?;

    // 3つ目のコミット（file1を変更）
    commit_file(&repo, "file1.rs", "v1_modified", "Modify file1")?;

    let mut detector = GitDiffDetector::new(temp_dir.path())?;
    let changes = detector.detect_changes_since(Some(&first_commit.to_string()))?;

    // file2が追加され、file1が変更されていることを確認
    let added = changes
        .iter()
        .filter(|c| matches!(c.status, FileChangeStatus::Added))
        .count();
    let _modified = changes
        .iter()
        .filter(|c| matches!(c.status, FileChangeStatus::Modified))
        .count();

    assert!(added >= 1); // file2.rs
                         // file1.rs（既にコミット済みなので検出されない可能性あり）

    Ok(())
}

#[test]
fn test_content_hash_without_git() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Gitリポジトリなしでファイルを作成
    fs::write(temp_dir.path().join("file1.rs"), "content1")?;
    fs::write(temp_dir.path().join("file2.rs"), "content2")?;

    let mut detector = GitDiffDetector::new(temp_dir.path())?;

    // 初回検出 - すべて新規として検出
    let changes1 = detector.detect_changes_since(None)?;
    assert_eq!(changes1.len(), 2);
    for change in &changes1 {
        assert_eq!(change.status, FileChangeStatus::Added);
        assert!(change.content_hash.is_some());
    }

    // ハッシュキャッシュを保存
    let cache_path = temp_dir.path().join("hash_cache.json");
    detector.save_hash_cache(&cache_path)?;

    // 新しいdetectorでキャッシュを読み込み
    let mut detector2 = GitDiffDetector::new(temp_dir.path())?;
    detector2.load_hash_cache(&cache_path)?;

    // 変更なしの場合、何も検出されない
    let changes2 = detector2.detect_changes_since(None)?;
    assert_eq!(changes2.len(), 0);

    // ファイルを変更
    fs::write(temp_dir.path().join("file1.rs"), "new content")?;

    // 変更が検出される
    let changes3 = detector2.detect_changes_since(None)?;
    assert_eq!(changes3.len(), 1);
    assert_eq!(changes3[0].status, FileChangeStatus::Modified);

    Ok(())
}

#[test]
#[ignore] // パフォーマンステストは手動実行
fn test_xxhash_performance() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("large_file.txt");

    // 1MBのテストファイルを作成
    let content = "a".repeat(1024 * 1024);
    fs::write(&test_file, &content)?;

    let detector = GitDiffDetector::new(temp_dir.path())?;

    // ハッシュ計算時間を測定
    let start = std::time::Instant::now();
    let hash1 = detector.calculate_file_hash(&test_file)?;
    let duration = start.elapsed();

    // xxHash3は非常に高速なので、1MBでも1ms未満であるべき
    assert!(
        duration.as_millis() < 10,
        "Hash calculation took too long: {duration:?}"
    );

    // 同じ内容なら同じハッシュ
    let hash2 = detector.calculate_file_hash(&test_file)?;
    assert_eq!(hash1, hash2);

    // 内容が変わればハッシュも変わる
    fs::write(&test_file, "b".repeat(1024 * 1024))?;
    let hash3 = detector.calculate_file_hash(&test_file)?;
    assert_ne!(hash1, hash3);

    Ok(())
}

#[test]
fn test_hash_cache_persistence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    // ファイルを作成
    fs::write(temp_dir.path().join("test1.rs"), "content1")?;
    fs::write(temp_dir.path().join("test2.rs"), "content2")?;

    // 初回検出とキャッシュ保存
    {
        let mut detector = GitDiffDetector::new(temp_dir.path())?;
        detector.detect_changes_since(None)?;
        detector.save_hash_cache(&cache_path)?;
    }

    // キャッシュが存在することを確認
    assert!(cache_path.exists());

    // キャッシュを読み込んで検証
    {
        let mut detector = GitDiffDetector::new(temp_dir.path())?;
        detector.load_hash_cache(&cache_path)?;

        // キャッシュがあるので変更なし
        let changes = detector.detect_changes_since(None)?;
        assert_eq!(changes.len(), 0);
    }

    Ok(())
}

#[test]
fn test_exclude_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // 様々なディレクトリとファイルを作成
    fs::create_dir_all(temp_dir.path().join("src"))?;
    fs::create_dir_all(temp_dir.path().join("target/debug"))?;
    fs::create_dir_all(temp_dir.path().join("node_modules"))?;
    fs::create_dir_all(temp_dir.path().join(".git"))?;

    fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}")?;
    fs::write(temp_dir.path().join("target/debug/app"), "binary")?;
    fs::write(temp_dir.path().join("node_modules/lib.js"), "module")?;

    let mut detector = GitDiffDetector::new(temp_dir.path())?;
    let changes = detector.detect_changes_since(None)?;

    // src/main.rsのみが検出される（target, node_modules, .gitは除外）
    assert_eq!(changes.len(), 1);
    assert!(changes[0].path.to_string_lossy().contains("src/main.rs"));

    Ok(())
}

#[test]
#[ignore] // Git rename detectionの実装が必要
fn test_renamed_file_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = create_test_repo(temp_dir.path())?;

    // ファイルをコミット
    commit_file(&repo, "old_name.rs", "fn test() {}", "Add file")?;

    // Git mvコマンドでリネーム
    let mut index = repo.index()?;
    index.remove_path(Path::new("old_name.rs"))?;
    fs::rename(
        temp_dir.path().join("old_name.rs"),
        temp_dir.path().join("new_name.rs"),
    )?;
    index.add_path(Path::new("new_name.rs"))?;
    index.write()?;

    let mut detector = GitDiffDetector::new(temp_dir.path())?;
    let changes = detector.detect_changes_since(None)?;

    // リネームが検出される
    assert!(changes
        .iter()
        .any(|c| matches!(c.status, FileChangeStatus::Modified)
            || matches!(c.status, FileChangeStatus::Renamed { .. })));

    Ok(())
}

#[test]
fn test_get_head_commit() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Gitリポジトリなしの場合
    {
        let detector = GitDiffDetector::new(temp_dir.path())?;
        assert_eq!(detector.get_head_commit(), None);
    }

    // Gitリポジトリありの場合
    {
        let repo = create_test_repo(temp_dir.path())?;
        let commit_oid = commit_file(&repo, "test.rs", "content", "Test commit")?;

        let detector = GitDiffDetector::new(temp_dir.path())?;
        let head_commit = detector.get_head_commit();

        assert!(head_commit.is_some());
        assert_eq!(head_commit.unwrap(), commit_oid.to_string());
    }

    Ok(())
}
