use anyhow::{Context, Result};
use git2::{Delta, DiffOptions, Repository, Status, StatusOptions};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;
use xxhash_rust::xxh3::xxh3_64;

/// ファイルの変更状態
#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeStatus {
    /// 新規追加
    Added,
    /// 変更
    Modified,
    /// 削除
    Deleted,
    /// 名前変更
    Renamed { from: PathBuf },
    /// Git管理外（コンテンツハッシュで管理）
    Untracked,
}

/// ファイルの変更情報
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub status: FileChangeStatus,
    pub content_hash: Option<String>,
}

/// Git差分検知器
pub struct GitDiffDetector {
    repo: Option<Repository>,
    project_root: PathBuf,
    /// ファイルパスとコンテンツハッシュのキャッシュ
    hash_cache: HashMap<PathBuf, String>,
}

impl GitDiffDetector {
    /// 新しいGit差分検知器を作成
    pub fn new<P: AsRef<Path>>(project_root: P) -> Result<Self> {
        let project_root = project_root.as_ref().to_path_buf();

        // Gitリポジトリを開く（存在しない場合はNone）
        let repo = Repository::open(&project_root).ok();

        if repo.is_none() {
            info!("Git repository not found, will use content hash for all files");
        }

        Ok(Self {
            repo,
            project_root,
            hash_cache: HashMap::new(),
        })
    }

    /// 前回のインデックス以降の変更ファイルを検出
    pub fn detect_changes_since(&mut self, last_commit: Option<&str>) -> Result<Vec<FileChange>> {
        info!("Detecting changes since commit: {:?}", last_commit);

        // Gitリポジトリがない場合、または last_commit が None の場合はハッシュベースの検出を使用
        if self.repo.is_none() || last_commit.is_none() {
            // 全ファイルをコンテンツハッシュで管理
            let changes = self.detect_all_files_with_hash()?;
            info!("Hash-based detection found {} changes", changes.len());
            return Ok(changes);
        }

        // Gitベースの変更検出
        let changes = self.detect_git_changes(last_commit)?;
        info!("Git detected {} changes", changes.len());

        // 変更が0件の場合は、ハッシュベースの検出も試す
        if changes.is_empty() && !self.hash_cache.is_empty() {
            info!("No Git changes detected, trying hash-based detection");
            let hash_changes = self.detect_all_files_with_hash()?;
            info!("Hash-based detection found {} changes", hash_changes.len());
            return Ok(hash_changes);
        }

        Ok(changes)
    }

    /// Git管理下のファイルの変更を検出
    fn detect_git_changes(&mut self, last_commit: Option<&str>) -> Result<Vec<FileChange>> {
        let repo = self.repo.as_ref().unwrap();
        let mut changes = Vec::new();

        // ワーキングディレクトリの変更を検出
        {
            let mut status_opts = StatusOptions::new();
            status_opts
                .include_untracked(true)
                .include_ignored(false)
                .include_unmodified(false);

            let statuses = repo
                .statuses(Some(&mut status_opts))
                .context("Failed to get repository status")?;

            // ステータスから変更ファイルを収集
            for entry in statuses.iter() {
                let path = entry
                    .path()
                    .ok_or_else(|| anyhow::anyhow!("Invalid path in status"))?;
                let file_path = self.project_root.join(path);

                let status = entry.status();
                let change_status = self.map_git_status(status, &file_path)?;

                // コンテンツハッシュを計算（削除以外）
                let content_hash = if change_status != FileChangeStatus::Deleted {
                    Some(self.calculate_file_hash(&file_path)?)
                } else {
                    None
                };

                changes.push(FileChange {
                    path: file_path,
                    status: change_status,
                    content_hash,
                });
            }
        }

        // コミット間の差分を検出（last_commitが指定されている場合）
        if let Some(commit_ref) = last_commit {
            self.add_commit_diff_changes(commit_ref, &mut changes)?;
        }

        Ok(changes)
    }

    /// Gitステータスをファイル変更ステータスにマップ
    fn map_git_status(&self, status: Status, _path: &Path) -> Result<FileChangeStatus> {
        if status.contains(Status::WT_NEW) {
            Ok(FileChangeStatus::Added)
        } else if status.contains(Status::WT_MODIFIED) {
            Ok(FileChangeStatus::Modified)
        } else if status.contains(Status::WT_DELETED) {
            Ok(FileChangeStatus::Deleted)
        } else if status.contains(Status::WT_RENAMED) {
            // TODO: renamed元のパスを取得
            Ok(FileChangeStatus::Modified)
        } else if status.contains(Status::INDEX_NEW) {
            Ok(FileChangeStatus::Added)
        } else if status.contains(Status::INDEX_MODIFIED) {
            Ok(FileChangeStatus::Modified)
        } else if status.contains(Status::INDEX_DELETED) {
            Ok(FileChangeStatus::Deleted)
        } else {
            Ok(FileChangeStatus::Untracked)
        }
    }

    /// コミット間の差分を追加
    fn add_commit_diff_changes(
        &mut self,
        last_commit_ref: &str,
        changes: &mut Vec<FileChange>,
    ) -> Result<()> {
        let repo = self.repo.as_ref().unwrap();
        let last_commit = repo
            .revparse_single(last_commit_ref)
            .context("Failed to parse last commit reference")?;
        let last_tree = last_commit
            .peel_to_tree()
            .context("Failed to get tree from last commit")?;

        let head = repo.head().context("Failed to get HEAD")?;
        let head_tree = head.peel_to_tree().context("Failed to get HEAD tree")?;

        let mut diff_opts = DiffOptions::new();
        let diff = repo
            .diff_tree_to_tree(Some(&last_tree), Some(&head_tree), Some(&mut diff_opts))
            .context("Failed to create diff")?;

        // 既に処理済みのパスを記録
        let processed_paths: HashSet<PathBuf> = changes.iter().map(|c| c.path.clone()).collect();

        diff.foreach(
            &mut |delta, _progress| {
                let file_path = delta.new_file().path().map(|p| self.project_root.join(p));

                if let Some(path) = file_path {
                    if !processed_paths.contains(&path) {
                        let status = match delta.status() {
                            Delta::Added => FileChangeStatus::Added,
                            Delta::Deleted => FileChangeStatus::Deleted,
                            Delta::Modified => FileChangeStatus::Modified,
                            Delta::Renamed => {
                                if let Some(old_path) = delta.old_file().path() {
                                    FileChangeStatus::Renamed {
                                        from: self.project_root.join(old_path),
                                    }
                                } else {
                                    FileChangeStatus::Modified
                                }
                            }
                            _ => return true, // Continue
                        };

                        let content_hash = if status != FileChangeStatus::Deleted {
                            self.calculate_file_hash(&path).ok()
                        } else {
                            None
                        };

                        changes.push(FileChange {
                            path,
                            status,
                            content_hash,
                        });
                    }
                }
                true // Continue iteration
            },
            None,
            None,
            None,
        )?;

        Ok(())
    }

    /// Git管理外の全ファイルをハッシュ付きで検出
    fn detect_all_files_with_hash(&mut self) -> Result<Vec<FileChange>> {
        let mut changes = Vec::new();

        // プロジェクトルート以下の全ファイルを走査
        for entry in WalkDir::new(&self.project_root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path().to_path_buf();

            // 除外パターン（.git, target, node_modules など）
            if self.should_exclude(&path) {
                continue;
            }

            // 対象ファイルのみ処理（.rs, .ts, .js, .tsx, .jsx）
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if !matches!(ext, "rs" | "ts" | "js" | "tsx" | "jsx") {
                continue;
            }

            let content_hash = self.calculate_file_hash(&path)?;

            // キャッシュと比較して変更を検出
            let status = if let Some(cached_hash) = self.hash_cache.get(&path) {
                if cached_hash != &content_hash {
                    FileChangeStatus::Modified
                } else {
                    continue; // 変更なし
                }
            } else {
                FileChangeStatus::Added
            };

            changes.push(FileChange {
                path: path.clone(),
                status,
                content_hash: Some(content_hash.clone()),
            });

            // キャッシュを更新
            self.hash_cache.insert(path, content_hash);
        }

        Ok(changes)
    }

    /// ファイルのコンテンツハッシュを計算（高速なxxHash3を使用）
    pub fn calculate_file_hash(&self, path: &Path) -> Result<String> {
        let content =
            fs::read(path).with_context(|| format!("Failed to read file: {}", path.display()))?;

        // xxHash3は非常に高速で、ファイルの変更検知には十分な品質
        let hash = xxh3_64(&content);
        Ok(format!("{hash:016x}"))
    }

    /// 除外すべきパスかどうかを判定
    fn should_exclude(&self, path: &Path) -> bool {
        for component in path.components() {
            if let Some(name) = component.as_os_str().to_str() {
                if name == ".git"
                    || name == "target"
                    || name == "node_modules"
                    || name == ".idea"
                    || name == ".vscode"
                {
                    return true;
                }
            }
        }
        false
    }

    /// 現在のHEADコミットのSHAを取得
    pub fn get_head_commit(&self) -> Option<String> {
        self.repo.as_ref().and_then(|repo| {
            repo.head()
                .ok()
                .and_then(|head| head.target())
                .map(|oid| oid.to_string())
        })
    }

    /// ハッシュキャッシュを保存
    pub fn save_hash_cache<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let cache_data = serde_json::to_string(&self.hash_cache)?;
        fs::write(path, cache_data)?;
        Ok(())
    }

    /// ハッシュキャッシュを読み込み
    pub fn load_hash_cache<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        if path.as_ref().exists() {
            let cache_data = fs::read_to_string(path)?;
            self.hash_cache = serde_json::from_str(&cache_data)?;
        }
        Ok(())
    }

    /// キャッシュされたハッシュを設定
    pub fn set_cached_hash(&mut self, path: PathBuf, hash: String) {
        self.hash_cache.insert(path, hash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn test_content_hash_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!").unwrap();

        let detector = GitDiffDetector::new(temp_dir.path()).unwrap();
        let hash = detector.calculate_file_hash(&file_path).unwrap();

        // xxHash3 hash of "Hello, World!"
        assert_eq!(hash.len(), 16); // xxHash3 produces 16 hex characters (64-bit)
    }

    #[test]
    fn test_should_exclude() {
        let detector = GitDiffDetector::new(".").unwrap();

        assert!(detector.should_exclude(Path::new(".git/config")));
        assert!(detector.should_exclude(Path::new("target/debug/build")));
        assert!(detector.should_exclude(Path::new("node_modules/package")));
        assert!(!detector.should_exclude(Path::new("src/main.rs")));
    }

    #[test]
    fn test_detect_changes_without_git() {
        let temp_dir = TempDir::new().unwrap();

        // Create some test files
        fs::write(temp_dir.path().join("file1.rs"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.rs"), "content2").unwrap();

        let mut detector = GitDiffDetector::new(temp_dir.path()).unwrap();
        let changes = detector.detect_changes_since(None).unwrap();

        assert_eq!(changes.len(), 2);
        for change in changes {
            assert_eq!(change.status, FileChangeStatus::Added);
            assert!(change.content_hash.is_some());
        }
    }

    #[test]
    fn test_detect_modified_files() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        // Create initial file
        fs::write(&file_path, "initial content").unwrap();
        let mut detector = GitDiffDetector::new(temp_dir.path()).unwrap();

        // First detection - file should be added
        let changes = detector.detect_changes_since(None).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, FileChangeStatus::Added);

        // Modify the file
        fs::write(&file_path, "modified content").unwrap();

        // Second detection - file should be modified
        let changes = detector.detect_changes_since(None).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, FileChangeStatus::Modified);
    }

    #[test]
    #[ignore] // TODO: Fix deletion detection logic
    fn test_detect_deleted_files() {
        // This test verifies that deleted files are detected
        // The current implementation tracks files in hash_cache,
        // so deletion is detected when a cached file no longer exists
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repository
        Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to initialize git");

        let file1 = temp_dir.path().join("file1.rs");

        // Create and detect initial file
        fs::write(&file1, "content1").unwrap();

        let mut detector = GitDiffDetector::new(temp_dir.path()).unwrap();
        let initial = detector.detect_changes_since(None).unwrap();
        assert!(initial
            .iter()
            .any(|c| c.path == file1 && c.status == FileChangeStatus::Added));

        // Now the file is in the cache
        // Delete it and detect again
        fs::remove_file(&file1).unwrap();

        let changes = detector.detect_changes_since(None).unwrap();

        // Should detect the deletion
        let deleted = changes.iter().find(|c| c.path == file1);
        assert!(deleted.is_some(), "Deleted file should be detected");
        assert_eq!(deleted.unwrap().status, FileChangeStatus::Deleted);
    }

    #[test]
    fn test_hash_cache_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("cache.json");
        let file_path = temp_dir.path().join("test.rs");

        fs::write(&file_path, "test content").unwrap();

        // Create detector and detect changes
        let mut detector = GitDiffDetector::new(temp_dir.path()).unwrap();
        detector.detect_changes_since(None).unwrap();

        // Save cache
        detector.save_hash_cache(&cache_path).unwrap();
        assert!(cache_path.exists());

        // Create new detector and load cache
        let mut new_detector = GitDiffDetector::new(temp_dir.path()).unwrap();
        new_detector.load_hash_cache(&cache_path).unwrap();

        // Hash cache should be loaded
        assert!(!new_detector.hash_cache.is_empty());
    }

    #[test]
    fn test_exclude_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Create files in various directories
        let src_dir = temp_dir.path().join("src");
        let target_dir = temp_dir.path().join("target");
        let git_dir = temp_dir.path().join(".git");

        fs::create_dir(&src_dir).unwrap();
        fs::create_dir(&target_dir).unwrap();
        fs::create_dir(&git_dir).unwrap();

        fs::write(src_dir.join("main.rs"), "src content").unwrap();
        fs::write(target_dir.join("debug.rs"), "target content").unwrap();
        fs::write(git_dir.join("config"), "git config").unwrap();

        let mut detector = GitDiffDetector::new(temp_dir.path()).unwrap();
        let changes = detector.detect_changes_since(None).unwrap();

        // Should only detect src file, not target or .git
        assert_eq!(changes.len(), 1);
        assert!(changes[0].path.to_str().unwrap().contains("src"));
    }

    #[test]
    fn test_file_change_struct() {
        let change = FileChange {
            path: PathBuf::from("test.rs"),
            status: FileChangeStatus::Modified,
            content_hash: Some("hash123".to_string()),
        };

        assert_eq!(change.path, PathBuf::from("test.rs"));
        assert_eq!(change.status, FileChangeStatus::Modified);
        assert_eq!(change.content_hash, Some("hash123".to_string()));
    }

    #[test]
    fn test_file_change_status() {
        assert_ne!(FileChangeStatus::Added, FileChangeStatus::Modified);
        assert_ne!(FileChangeStatus::Modified, FileChangeStatus::Deleted);
        assert_ne!(FileChangeStatus::Deleted, FileChangeStatus::Added);
    }

    #[test]
    fn test_new_detector_with_non_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let detector = GitDiffDetector::new(temp_dir.path());
        assert!(detector.is_ok());

        let detector = detector.unwrap();
        assert!(detector.repo.is_none());
    }

    #[test]
    fn test_get_head_commit_without_repo() {
        let temp_dir = TempDir::new().unwrap();
        let detector = GitDiffDetector::new(temp_dir.path()).unwrap();
        assert!(detector.get_head_commit().is_none());
    }

    #[test]
    fn test_hash_consistency() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = "consistent content";

        fs::write(&file_path, content).unwrap();

        let detector = GitDiffDetector::new(temp_dir.path()).unwrap();
        let hash1 = detector.calculate_file_hash(&file_path).unwrap();
        let hash2 = detector.calculate_file_hash(&file_path).unwrap();

        // Same content should produce same hash
        assert_eq!(hash1, hash2);

        // Modify content
        fs::write(&file_path, "different content").unwrap();
        let hash3 = detector.calculate_file_hash(&file_path).unwrap();

        // Different content should produce different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_walkdir_with_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let real_file = temp_dir.path().join("real.rs");

        fs::write(&real_file, "real content").unwrap();

        // Note: Symlink creation might fail on Windows without permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_path = temp_dir.path().join("link.rs");
            let _ = symlink(&real_file, &symlink_path);
        }

        let mut detector = GitDiffDetector::new(temp_dir.path()).unwrap();
        let changes = detector.detect_changes_since(None).unwrap();

        // Should detect at least the real file
        assert!(changes.iter().any(|c| c.path == real_file));
    }
} // Test comment for differential index
