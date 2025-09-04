use anyhow::Result;
use lsp_types::{DocumentSymbolResponse, Location, WorkspaceSymbol};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

/// 階層的キャッシュシステム
/// L1: メモリ内キャッシュ（高速、小容量、短TTL）
/// L2: ディスクキャッシュ（中速、中容量、中TTL）
/// L3: 永続化DB（低速、大容量、長期保存）
pub struct HierarchicalCache {
    l1_memory: Arc<RwLock<L1MemoryCache>>,
    l2_disk: Arc<RwLock<L2DiskCache>>,
    l3_db: Arc<RwLock<L3PersistentDb>>,
    config: CacheConfig,
    metrics: Arc<RwLock<CacheMetrics>>,
}

#[derive(Clone, Debug)]
pub struct CacheConfig {
    /// L1キャッシュのTTL（推奨: 100ms）
    pub l1_ttl: Duration,
    /// L2キャッシュのTTL（推奨: 1秒）
    pub l2_ttl: Duration,
    /// L3キャッシュのTTL（推奨: 無期限）
    pub l3_ttl: Option<Duration>,
    /// L1の最大エントリ数
    pub l1_max_entries: usize,
    /// L2の最大サイズ（バイト）
    pub l2_max_size_bytes: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1_ttl: Duration::from_millis(100), // パフォーマンス分析に基づく
            l2_ttl: Duration::from_secs(1),     // パフォーマンス分析に基づく
            l3_ttl: None,                       // 永続化
            l1_max_entries: 1000,
            l2_max_size_bytes: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// キャッシュメトリクス
#[derive(Debug, Default, Clone)]
pub struct CacheMetrics {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l3_hits: u64,
    pub l3_misses: u64,
    pub total_requests: u64,
}

impl CacheMetrics {
    pub fn hit_rate(&self, level: CacheLevel) -> f64 {
        match level {
            CacheLevel::L1 => {
                let total = self.l1_hits + self.l1_misses;
                if total == 0 {
                    0.0
                } else {
                    self.l1_hits as f64 / total as f64
                }
            }
            CacheLevel::L2 => {
                let total = self.l2_hits + self.l2_misses;
                if total == 0 {
                    0.0
                } else {
                    self.l2_hits as f64 / total as f64
                }
            }
            CacheLevel::L3 => {
                let total = self.l3_hits + self.l3_misses;
                if total == 0 {
                    0.0
                } else {
                    self.l3_hits as f64 / total as f64
                }
            }
        }
    }

    pub fn overall_hit_rate(&self) -> f64 {
        let hits = self.l1_hits + self.l2_hits + self.l3_hits;
        if self.total_requests == 0 {
            0.0
        } else {
            hits as f64 / self.total_requests as f64
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CacheLevel {
    L1,
    L2,
    L3,
}

/// L1メモリキャッシュ（documentSymbol結果用）
struct L1MemoryCache {
    document_symbols: HashMap<PathBuf, CachedEntry<DocumentSymbolResponse>>,
    max_entries: usize,
}

/// L2ディスクキャッシュ（workspace/symbol結果用）
struct L2DiskCache {
    workspace_symbols: HashMap<String, CachedEntry<Vec<WorkspaceSymbol>>>,
    cache_dir: PathBuf,
    current_size_bytes: usize,
    max_size_bytes: usize,
}

/// L3永続化DB（定義・参照情報用）
struct L3PersistentDb {
    definitions: HashMap<(PathBuf, u32, u32), Vec<Location>>,
    references: HashMap<(PathBuf, u32, u32), Vec<Location>>,
    db_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CachedEntry<T> {
    data: T,
    timestamp_ms: u64, // UNIXタイムスタンプミリ秒
    #[serde(skip)]
    instant: Option<Instant>, // 内部用
    access_count: u32,
}

impl<T> CachedEntry<T> {
    fn new(data: T) -> Self {
        Self {
            data,
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            instant: Some(Instant::now()),
            access_count: 1,
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.instant.map(|i| i.elapsed() > ttl).unwrap_or(true)
    }

    fn touch(&mut self) {
        self.access_count += 1;
    }
}

impl HierarchicalCache {
    pub fn new(config: CacheConfig) -> Result<Self> {
        let cache_dir = PathBuf::from("tmp/lsp_cache");
        std::fs::create_dir_all(&cache_dir)?;

        let db_path = cache_dir.join("persistent.db");

        Ok(Self {
            l1_memory: Arc::new(RwLock::new(L1MemoryCache {
                document_symbols: HashMap::new(),
                max_entries: config.l1_max_entries,
            })),
            l2_disk: Arc::new(RwLock::new(L2DiskCache {
                workspace_symbols: HashMap::new(),
                cache_dir: cache_dir.clone(),
                current_size_bytes: 0,
                max_size_bytes: config.l2_max_size_bytes,
            })),
            l3_db: Arc::new(RwLock::new(L3PersistentDb {
                definitions: HashMap::new(),
                references: HashMap::new(),
                db_path,
            })),
            config,
            metrics: Arc::new(RwLock::new(CacheMetrics::default())),
        })
    }

    /// documentSymbol結果をキャッシュ（L1）
    pub fn cache_document_symbols(
        &self,
        file_path: &Path,
        symbols: DocumentSymbolResponse,
    ) -> Result<()> {
        let mut l1 = self.l1_memory.write().unwrap();

        // LRU実装: 最大エントリ数を超えたら最も古いものを削除
        if l1.document_symbols.len() >= l1.max_entries {
            let oldest = l1
                .document_symbols
                .iter()
                .min_by_key(|(_, entry)| entry.timestamp_ms)
                .map(|(path, _)| path.clone());

            if let Some(path) = oldest {
                l1.document_symbols.remove(&path);
                debug!("Evicted oldest document symbol cache for: {:?}", path);
            }
        }

        l1.document_symbols
            .insert(file_path.to_path_buf(), CachedEntry::new(symbols));

        debug!("Cached document symbols for: {:?}", file_path);
        Ok(())
    }

    /// documentSymbol結果を取得（L1）
    pub fn get_document_symbols(&self, file_path: &Path) -> Option<DocumentSymbolResponse> {
        let mut metrics = self.metrics.write().unwrap();
        metrics.total_requests += 1;

        let mut l1 = self.l1_memory.write().unwrap();

        if let Some(entry) = l1.document_symbols.get_mut(file_path) {
            if !entry.is_expired(self.config.l1_ttl) {
                entry.touch();
                metrics.l1_hits += 1;
                debug!("L1 cache hit for document symbols: {:?}", file_path);
                return Some(entry.data.clone());
            } else {
                // 期限切れエントリを削除
                l1.document_symbols.remove(file_path);
            }
        }

        metrics.l1_misses += 1;
        None
    }

    /// workspace/symbol結果をキャッシュ（L2）
    pub fn cache_workspace_symbols(
        &self,
        query: &str,
        symbols: Vec<WorkspaceSymbol>,
    ) -> Result<()> {
        let mut l2 = self.l2_disk.write().unwrap();

        // サイズチェック（簡易的な推定）
        let estimated_size = symbols.len() * std::mem::size_of::<WorkspaceSymbol>();

        // サイズ制限を超える場合は最も古いエントリを削除
        while l2.current_size_bytes + estimated_size > l2.max_size_bytes {
            let oldest = l2
                .workspace_symbols
                .iter()
                .min_by_key(|(_, entry)| entry.timestamp_ms)
                .map(|(query, _)| query.clone());

            if let Some(query) = oldest {
                l2.workspace_symbols.remove(&query);
                l2.current_size_bytes = l2.current_size_bytes.saturating_sub(estimated_size);
                debug!("Evicted oldest workspace symbol cache for query: {}", query);
            } else {
                break;
            }
        }

        l2.workspace_symbols
            .insert(query.to_string(), CachedEntry::new(symbols));
        l2.current_size_bytes += estimated_size;

        // ディスクに非同期で書き込み（実装簡略化のため省略）
        debug!("Cached workspace symbols for query: {}", query);
        Ok(())
    }

    /// workspace/symbol結果を取得（L2）
    pub fn get_workspace_symbols(&self, query: &str) -> Option<Vec<WorkspaceSymbol>> {
        let mut metrics = self.metrics.write().unwrap();

        let mut l2 = self.l2_disk.write().unwrap();

        if let Some(entry) = l2.workspace_symbols.get_mut(query) {
            if !entry.is_expired(self.config.l2_ttl) {
                entry.touch();
                metrics.l2_hits += 1;
                debug!("L2 cache hit for workspace symbols: {}", query);
                return Some(entry.data.clone());
            } else {
                l2.workspace_symbols.remove(query);
            }
        }

        metrics.l2_misses += 1;
        None
    }

    /// 定義情報をキャッシュ（L3）
    pub fn cache_definitions(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        definitions: Vec<Location>,
    ) -> Result<()> {
        let mut l3 = self.l3_db.write().unwrap();
        l3.definitions
            .insert((file_path.to_path_buf(), line, column), definitions);

        // 永続化（実装簡略化のため省略）
        debug!("Cached definitions for {:?}:{}:{}", file_path, line, column);
        Ok(())
    }

    /// 定義情報を取得（L3）
    pub fn get_definitions(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
    ) -> Option<Vec<Location>> {
        let mut metrics = self.metrics.write().unwrap();

        let l3 = self.l3_db.read().unwrap();

        if let Some(definitions) = l3.definitions.get(&(file_path.to_path_buf(), line, column)) {
            metrics.l3_hits += 1;
            debug!(
                "L3 cache hit for definitions: {:?}:{}:{}",
                file_path, line, column
            );
            return Some(definitions.clone());
        }

        metrics.l3_misses += 1;
        None
    }

    /// 参照情報をキャッシュ（L3）
    pub fn cache_references(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        references: Vec<Location>,
    ) -> Result<()> {
        let mut l3 = self.l3_db.write().unwrap();
        l3.references
            .insert((file_path.to_path_buf(), line, column), references);

        debug!("Cached references for {:?}:{}:{}", file_path, line, column);
        Ok(())
    }

    /// 参照情報を取得（L3）
    pub fn get_references(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
    ) -> Option<Vec<Location>> {
        let l3 = self.l3_db.read().unwrap();
        l3.references
            .get(&(file_path.to_path_buf(), line, column))
            .cloned()
    }

    /// キャッシュをクリア
    pub fn clear_all(&self) {
        {
            let mut l1 = self.l1_memory.write().unwrap();
            l1.document_symbols.clear();
        }
        {
            let mut l2 = self.l2_disk.write().unwrap();
            l2.workspace_symbols.clear();
            l2.current_size_bytes = 0;
        }
        {
            let mut l3 = self.l3_db.write().unwrap();
            l3.definitions.clear();
            l3.references.clear();
        }
        info!("Cleared all cache levels");
    }

    /// ファイル変更時のキャッシュ無効化
    pub fn invalidate_file(&self, file_path: &Path) {
        // L1から該当ファイルのキャッシュを削除
        {
            let mut l1 = self.l1_memory.write().unwrap();
            if l1.document_symbols.remove(file_path).is_some() {
                debug!("Invalidated L1 cache for: {:?}", file_path);
            }
        }

        // L3から該当ファイルの定義・参照を削除
        {
            let mut l3 = self.l3_db.write().unwrap();
            l3.definitions.retain(|(path, _, _), _| path != file_path);
            l3.references.retain(|(path, _, _), _| path != file_path);
            debug!("Invalidated L3 cache for: {:?}", file_path);
        }

        // L2のworkspace/symbolは全体に影響するため全削除
        {
            let mut l2 = self.l2_disk.write().unwrap();
            l2.workspace_symbols.clear();
            l2.current_size_bytes = 0;
            debug!("Cleared L2 workspace symbol cache due to file change");
        }
    }

    /// キャッシュメトリクスを取得
    pub fn get_metrics(&self) -> CacheMetrics {
        self.metrics.read().unwrap().clone()
    }

    /// キャッシュ統計を表示
    pub fn print_stats(&self) {
        let metrics = self.get_metrics();
        println!("\n=== Cache Statistics ===");
        println!("Total requests: {}", metrics.total_requests);
        println!(
            "Overall hit rate: {:.2}%",
            metrics.overall_hit_rate() * 100.0
        );
        println!(
            "L1 hit rate: {:.2}% (hits: {}, misses: {})",
            metrics.hit_rate(CacheLevel::L1) * 100.0,
            metrics.l1_hits,
            metrics.l1_misses
        );
        println!(
            "L2 hit rate: {:.2}% (hits: {}, misses: {})",
            metrics.hit_rate(CacheLevel::L2) * 100.0,
            metrics.l2_hits,
            metrics.l2_misses
        );
        println!(
            "L3 hit rate: {:.2}% (hits: {}, misses: {})",
            metrics.hit_rate(CacheLevel::L3) * 100.0,
            metrics.l3_hits,
            metrics.l3_misses
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{DocumentSymbol, SymbolKind};

    #[test]
    fn test_l1_cache() {
        let cache = HierarchicalCache::new(CacheConfig::default()).unwrap();

        let file_path = PathBuf::from("test.rs");
        let symbols = DocumentSymbolResponse::Nested(vec![DocumentSymbol {
            name: "test_function".to_string(),
            kind: SymbolKind::FUNCTION,
            range: Default::default(),
            selection_range: Default::default(),
            detail: None,
            tags: None,
            deprecated: None,
            children: None,
        }]);

        // キャッシュに保存
        cache
            .cache_document_symbols(&file_path, symbols.clone())
            .unwrap();

        // キャッシュから取得
        let cached = cache.get_document_symbols(&file_path);
        assert!(cached.is_some());

        // メトリクスを確認
        let metrics = cache.get_metrics();
        assert_eq!(metrics.l1_hits, 1);
        assert_eq!(metrics.l1_misses, 0);
    }

    #[test]
    fn test_cache_expiration() {
        let mut config = CacheConfig::default();
        config.l1_ttl = Duration::from_millis(10);

        let cache = HierarchicalCache::new(config).unwrap();
        let file_path = PathBuf::from("test.rs");
        let symbols = DocumentSymbolResponse::Nested(vec![]);

        cache.cache_document_symbols(&file_path, symbols).unwrap();

        // TTL内は取得可能
        assert!(cache.get_document_symbols(&file_path).is_some());

        // TTL経過後は取得不可
        std::thread::sleep(Duration::from_millis(15));
        assert!(cache.get_document_symbols(&file_path).is_none());
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = HierarchicalCache::new(CacheConfig::default()).unwrap();
        let file_path = PathBuf::from("test.rs");

        // 各レベルにデータを追加
        cache
            .cache_document_symbols(&file_path, DocumentSymbolResponse::Nested(vec![]))
            .unwrap();
        cache.cache_workspace_symbols("test", vec![]).unwrap();
        cache.cache_definitions(&file_path, 10, 5, vec![]).unwrap();

        // ファイルを無効化
        cache.invalidate_file(&file_path);

        // L1とL3は削除される
        assert!(cache.get_document_symbols(&file_path).is_none());
        assert!(cache.get_definitions(&file_path, 10, 5).is_none());

        // L2も削除される（ワークスペース全体に影響するため）
        assert!(cache.get_workspace_symbols("test").is_none());
    }
}
