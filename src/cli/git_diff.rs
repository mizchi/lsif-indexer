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
        if self.repo.is_some() {
            let changes = self.detect_git_changes(last_commit)?;
            info!("Git detected {} changes", changes.len());
            Ok(changes)
        } else {
            // Gitリポジトリがない場合は全ファイルをコンテンツハッシュで管理
            let changes = self.detect_all_files_with_hash()?;
            info!("Hash-based detection found {} changes", changes.len());
            Ok(changes)
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
} // Test comment for differential index
