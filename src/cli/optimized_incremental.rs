use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::cli::cached_storage::CachedIndexStorage;
use crate::cli::parallel_storage::ParallelIndexStorage;
use crate::core::{calculate_file_hash, Symbol};

/// 最適化された差分インデックス管理
pub struct OptimizedIncrementalIndexer {
    storage: Arc<ParallelIndexStorage>,
    cache: Arc<CachedIndexStorage>,
    file_index: HashMap<PathBuf, FileState>,
    dependency_graph: HashMap<String, HashSet<String>>,
    dirty_files: HashSet<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileState {
    hash: String,
    last_modified: std::time::SystemTime,
    symbols: Vec<String>,
    dependencies: HashSet<PathBuf>,
    imports: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalResult {
    pub files_checked: usize,
    pub files_updated: usize,
    pub symbols_added: usize,
    pub symbols_updated: usize,
    pub symbols_removed: usize,
    pub time_ms: u128,
    pub cache_hits: usize,
    pub affected_files: Vec<PathBuf>,
}

impl OptimizedIncrementalIndexer {
    pub fn new<P: AsRef<Path>>(storage_path: P) -> Result<Self> {
        let storage = Arc::new(ParallelIndexStorage::open(&storage_path)?);
        let cache = Arc::new(CachedIndexStorage::open(&storage_path)?);

        Ok(Self {
            storage,
            cache,
            file_index: HashMap::new(),
            dependency_graph: HashMap::new(),
            dirty_files: HashSet::new(),
        })
    }

    /// ファイルの変更を検出
    pub fn detect_changes(&mut self, project_path: &Path) -> Result<Vec<PathBuf>> {
        let start = Instant::now();
        let mut changed_files = Vec::new();

        // 並列でファイルをスキャン
        let file_paths: Vec<PathBuf> = self.collect_source_files(project_path)?;

        let changes: Vec<(PathBuf, bool)> = file_paths
            .par_iter()
            .map(|path| {
                let needs_update = self.check_file_needs_update(path).unwrap_or(true);
                (path.clone(), needs_update)
            })
            .collect();

        for (path, needs_update) in changes {
            if needs_update {
                changed_files.push(path.clone());
                self.dirty_files.insert(path);
            }
        }

        println!("Change detection took: {}ms", start.elapsed().as_millis());
        Ok(changed_files)
    }

    /// 差分更新を実行
    pub fn update_incremental(&mut self, changed_files: Vec<PathBuf>) -> Result<IncrementalResult> {
        let start = Instant::now();
        let mut result = IncrementalResult {
            files_checked: changed_files.len(),
            files_updated: 0,
            symbols_added: 0,
            symbols_updated: 0,
            symbols_removed: 0,
            time_ms: 0,
            cache_hits: 0,
            affected_files: Vec::new(),
        };

        // 影響を受けるファイルを計算（依存関係を考慮）
        let affected_files = self.calculate_affected_files(&changed_files);
        result.affected_files = affected_files.clone();

        // 変更されたファイルのみを並列処理
        let update_results: Vec<FileUpdateResult> = affected_files
            .par_iter()
            .map(|path| self.update_single_file(path))
            .collect::<Result<Vec<_>>>()?;

        // 結果を集計
        for update in update_results {
            result.files_updated += 1;
            result.symbols_added += update.added;
            result.symbols_updated += update.updated;
            result.symbols_removed += update.removed;
            result.cache_hits += update.cache_hits;
        }

        // 差分のみをストレージに保存
        self.save_incremental_changes(&result)?;

        result.time_ms = start.elapsed().as_millis();
        Ok(result)
    }

    /// 単一ファイルの更新
    fn update_single_file(&self, path: &Path) -> Result<FileUpdateResult> {
        let mut result = FileUpdateResult::default();

        // ファイル内容を読み込み
        let content = std::fs::read_to_string(path)?;
        let _file_hash = calculate_file_hash(&content);

        // 既存のシンボルをキャッシュから取得（高速）
        let old_symbols = self.get_cached_symbols(path)?;
        result.cache_hits = if old_symbols.is_some() { 1 } else { 0 };

        // 新しいシンボルを解析（ここは実際のパーサーに置き換え）
        let new_symbols = self.parse_file_symbols(path, &content)?;

        // 差分を計算
        if let Some(old) = old_symbols {
            let old_ids: HashSet<String> = old.iter().map(|s| s.id.clone()).collect();
            let new_ids: HashSet<String> = new_symbols.iter().map(|s| s.id.clone()).collect();

            result.removed = old_ids.difference(&new_ids).count();
            result.added = new_ids.difference(&old_ids).count();
            result.updated = old_ids.intersection(&new_ids).count();
        } else {
            result.added = new_symbols.len();
        }

        // キャッシュに保存
        self.cache_symbols(path, &new_symbols)?;

        Ok(result)
    }

    /// 依存関係を考慮して影響を受けるファイルを計算
    fn calculate_affected_files(&self, changed_files: &[PathBuf]) -> Vec<PathBuf> {
        let mut affected = HashSet::new();
        let mut to_process: Vec<PathBuf> = changed_files.to_vec();

        while let Some(file) = to_process.pop() {
            if affected.insert(file.clone()) {
                // このファイルに依存しているファイルを追加
                if let Some(dependents) = self.get_dependents(&file) {
                    for dep in dependents {
                        if !affected.contains(&dep) {
                            to_process.push(dep);
                        }
                    }
                }
            }
        }

        affected.into_iter().collect()
    }

    /// インテリジェントキャッシュプリフェッチ
    pub fn prefetch_likely_changes(&self, current_file: &Path) -> Result<()> {
        // 現在のファイルに関連する可能性の高いファイルを予測
        let mut related_files = Vec::new();

        // 同じディレクトリのファイル
        if let Some(parent) = current_file.parent() {
            for entry in std::fs::read_dir(parent)? {
                if let Ok(entry) = entry {
                    related_files.push(entry.path().to_string_lossy().to_string());
                }
            }
        }

        // インポートされているファイル
        if let Some(state) = self.file_index.get(current_file) {
            for import in &state.imports {
                related_files.push(import.clone());
            }
        }

        // キャッシュにプリロード
        self.cache.prefetch(&related_files)?;
        Ok(())
    }

    /// バッチ差分更新（複数ファイルを効率的に処理）
    pub fn batch_update(&mut self, files: Vec<PathBuf>) -> Result<IncrementalResult> {
        let chunk_size = 50; // 最適なチャンクサイズ
        let mut total_result = IncrementalResult {
            files_checked: files.len(),
            files_updated: 0,
            symbols_added: 0,
            symbols_updated: 0,
            symbols_removed: 0,
            time_ms: 0,
            cache_hits: 0,
            affected_files: Vec::new(),
        };

        // チャンクごとに処理
        for chunk in files.chunks(chunk_size) {
            let result = self.update_incremental(chunk.to_vec())?;
            total_result.files_updated += result.files_updated;
            total_result.symbols_added += result.symbols_added;
            total_result.symbols_updated += result.symbols_updated;
            total_result.symbols_removed += result.symbols_removed;
            total_result.cache_hits += result.cache_hits;
            total_result.affected_files.extend(result.affected_files);
        }

        Ok(total_result)
    }

    /// ファイルが更新必要かチェック（高速化）
    fn check_file_needs_update(&self, path: &Path) -> Result<bool> {
        // キャッシュから最終更新時刻を取得
        if let Some(state) = self.file_index.get(path) {
            let metadata = std::fs::metadata(path)?;
            let modified = metadata.modified()?;

            // タイムスタンプベースの高速チェック
            if modified <= state.last_modified {
                return Ok(false);
            }

            // ハッシュベースの正確なチェック（必要な場合のみ）
            let content = std::fs::read_to_string(path)?;
            let current_hash = calculate_file_hash(&content);
            Ok(current_hash != state.hash)
        } else {
            Ok(true) // 新規ファイル
        }
    }

    /// 差分のみを保存
    fn save_incremental_changes(&self, result: &IncrementalResult) -> Result<()> {
        // 変更されたシンボルのみを保存
        let changed_symbols: Vec<(String, Symbol)> = Vec::new(); // 実際の実装では収集

        // 並列バッチ保存
        self.storage.save_symbols_chunked(&changed_symbols, 100)?;

        println!(
            "Saved {} symbols incrementally (added: {}, updated: {}, removed: {})",
            result.symbols_added + result.symbols_updated,
            result.symbols_added,
            result.symbols_updated,
            result.symbols_removed
        );

        Ok(())
    }

    // ヘルパーメソッド
    fn collect_source_files(&self, _project_path: &Path) -> Result<Vec<PathBuf>> {
        // 実装簡略化
        Ok(Vec::new())
    }

    fn get_cached_symbols(&self, _path: &Path) -> Result<Option<Vec<Symbol>>> {
        // 実装簡略化
        Ok(None)
    }

    fn parse_file_symbols(&self, _path: &Path, _content: &str) -> Result<Vec<Symbol>> {
        // 実装簡略化
        Ok(Vec::new())
    }

    fn cache_symbols(&self, _path: &Path, _symbols: &[Symbol]) -> Result<()> {
        // 実装簡略化
        Ok(())
    }

    fn get_dependents(&self, _file: &Path) -> Option<Vec<PathBuf>> {
        // 実装簡略化
        None
    }
}

#[derive(Default)]
struct FileUpdateResult {
    added: usize,
    updated: usize,
    removed: usize,
    cache_hits: usize,
}

/// ウォッチモード用の自動差分更新
pub struct IncrementalWatcher {
    indexer: OptimizedIncrementalIndexer,
    project_path: PathBuf,
}

impl IncrementalWatcher {
    pub fn new(indexer: OptimizedIncrementalIndexer, project_path: PathBuf) -> Self {
        Self {
            indexer,
            project_path,
        }
    }

    /// ファイル変更を監視して自動更新
    pub fn watch(&mut self) -> Result<()> {
        loop {
            // ファイル変更を検出
            let changed = self.indexer.detect_changes(&self.project_path)?;

            if !changed.is_empty() {
                println!("Detected {} file changes", changed.len());

                // 差分更新を実行
                let result = self.indexer.update_incremental(changed)?;

                println!(
                    "Incremental update completed in {}ms: {} files, {} symbols",
                    result.time_ms,
                    result.files_updated,
                    result.symbols_added + result.symbols_updated
                );

                // キャッシュヒット率を表示
                if result.files_checked > 0 {
                    let hit_rate = (result.cache_hits as f64 / result.files_checked as f64) * 100.0;
                    println!("Cache hit rate: {hit_rate:.1}%");
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}
