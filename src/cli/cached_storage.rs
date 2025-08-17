use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// LRUキャッシュエントリ
#[derive(Clone)]
struct CacheEntry<T> {
    value: T,
    last_accessed: Instant,
    access_count: usize,
}

/// キャッシュ付きストレージ
pub struct CachedIndexStorage {
    db: sled::Db,
    cache: Arc<RwLock<HashMap<String, CacheEntry<Vec<u8>>>>>,
    max_cache_size: usize,
    ttl: Duration,
}

impl CachedIndexStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::open_with_config(path, 1000, Duration::from_secs(300))
    }

    pub fn open_with_config<P: AsRef<Path>>(
        path: P,
        max_cache_size: usize,
        ttl: Duration,
    ) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self {
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_cache_size,
            ttl,
        })
    }

    /// キャッシュ付き保存
    pub fn save_data_cached<T: Serialize>(&self, key: &str, data: &T) -> Result<()> {
        let serialized = bincode::serialize(data)?;
        
        // DBに保存
        self.db.insert(key, serialized.as_slice())?;
        
        // キャッシュに追加
        {
            let mut cache = self.cache.write().unwrap();
            
            // キャッシュサイズ制限チェック
            if cache.len() >= self.max_cache_size {
                self.evict_lru(&mut cache);
            }
            
            cache.insert(
                key.to_string(),
                CacheEntry {
                    value: serialized,
                    last_accessed: Instant::now(),
                    access_count: 0,
                },
            );
        }
        
        self.db.flush()?;
        Ok(())
    }

    /// キャッシュ付き読み込み
    pub fn load_data_cached<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        // まずキャッシュをチェック
        {
            let mut cache = self.cache.write().unwrap();
            if let Some(entry) = cache.get_mut(key) {
                // TTLチェック
                if entry.last_accessed.elapsed() < self.ttl {
                    entry.last_accessed = Instant::now();
                    entry.access_count += 1;
                    
                    let deserialized = bincode::deserialize(&entry.value)?;
                    return Ok(Some(deserialized));
                } else {
                    // TTL切れのエントリを削除
                    cache.remove(key);
                }
            }
        }
        
        // キャッシュになければDBから読み込み
        if let Some(data) = self.db.get(key)? {
            let deserialized: T = bincode::deserialize(&data)?;
            
            // キャッシュに追加
            {
                let mut cache = self.cache.write().unwrap();
                if cache.len() >= self.max_cache_size {
                    self.evict_lru(&mut cache);
                }
                
                cache.insert(
                    key.to_string(),
                    CacheEntry {
                        value: data.to_vec(),
                        last_accessed: Instant::now(),
                        access_count: 1,
                    },
                );
            }
            
            Ok(Some(deserialized))
        } else {
            Ok(None)
        }
    }

    /// バッチ読み込み（キャッシュ最適化）
    pub fn load_batch_cached<T: for<'de> Deserialize<'de> + Clone>(
        &self,
        keys: &[String],
    ) -> Result<Vec<Option<T>>> {
        let mut results = Vec::with_capacity(keys.len());
        let mut cache_misses = Vec::new();
        
        // キャッシュから取得
        {
            let mut cache = self.cache.write().unwrap();
            for key in keys {
                if let Some(entry) = cache.get_mut(key) {
                    if entry.last_accessed.elapsed() < self.ttl {
                        entry.last_accessed = Instant::now();
                        entry.access_count += 1;
                        
                        let deserialized: T = bincode::deserialize(&entry.value)?;
                        results.push(Some(deserialized));
                    } else {
                        cache.remove(key);
                        cache_misses.push(key.clone());
                        results.push(None);
                    }
                } else {
                    cache_misses.push(key.clone());
                    results.push(None);
                }
            }
        }
        
        // キャッシュミスしたものをDBから読み込み
        if !cache_misses.is_empty() {
            let mut cache = self.cache.write().unwrap();
            
            for (i, key) in keys.iter().enumerate() {
                if results[i].is_none() {
                    if let Some(data) = self.db.get(key)? {
                        let deserialized: T = bincode::deserialize(&data)?;
                        results[i] = Some(deserialized.clone());
                        
                        // キャッシュに追加
                        if cache.len() >= self.max_cache_size {
                            self.evict_lru(&mut cache);
                        }
                        
                        cache.insert(
                            key.clone(),
                            CacheEntry {
                                value: data.to_vec(),
                                last_accessed: Instant::now(),
                                access_count: 1,
                            },
                        );
                    }
                }
            }
        }
        
        Ok(results)
    }

    /// プリフェッチ（先読み）
    pub fn prefetch(&self, keys: &[String]) -> Result<()> {
        let mut cache = self.cache.write().unwrap();
        
        for key in keys {
            if !cache.contains_key(key) {
                if let Some(data) = self.db.get(key)? {
                    if cache.len() >= self.max_cache_size {
                        self.evict_lru(&mut cache);
                    }
                    
                    cache.insert(
                        key.clone(),
                        CacheEntry {
                            value: data.to_vec(),
                            last_accessed: Instant::now(),
                            access_count: 0,
                        },
                    );
                }
            }
        }
        
        Ok(())
    }

    /// キャッシュをクリア
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }

    /// キャッシュ統計を取得
    pub fn get_cache_stats(&self) -> CacheStats {
        let cache = self.cache.read().unwrap();
        
        let total_size: usize = cache.values().map(|e| e.value.len()).sum();
        let avg_access_count = if cache.is_empty() {
            0.0
        } else {
            cache.values().map(|e| e.access_count as f64).sum::<f64>() / cache.len() as f64
        };
        
        CacheStats {
            entries: cache.len(),
            total_size_bytes: total_size,
            avg_access_count,
            max_cache_size: self.max_cache_size,
        }
    }

    /// ウォームアップ（頻繁にアクセスされるデータを事前ロード）
    pub fn warmup(&self, hot_keys: &[String]) -> Result<()> {
        self.prefetch(hot_keys)?;
        
        // アクセスカウントを増やして優先度を上げる
        let mut cache = self.cache.write().unwrap();
        for key in hot_keys {
            if let Some(entry) = cache.get_mut(key) {
                entry.access_count = 10; // 高い初期優先度
            }
        }
        
        Ok(())
    }

    /// LRU削除（最も使われていないエントリを削除）
    fn evict_lru(&self, cache: &mut HashMap<String, CacheEntry<Vec<u8>>>) {
        if let Some((key_to_remove, _)) = cache
            .iter()
            .min_by_key(|(_, entry)| (entry.access_count, entry.last_accessed))
            .map(|(k, v)| (k.clone(), v.clone()))
        {
            cache.remove(&key_to_remove);
        }
    }

    /// スマートプリフェッチ（関連データを予測して先読み）
    pub fn smart_prefetch(&self, accessed_key: &str) -> Result<()> {
        // シンボルIDから関連するキーを推測
        let related_keys = self.predict_related_keys(accessed_key)?;
        self.prefetch(&related_keys)?;
        Ok(())
    }

    fn predict_related_keys(&self, key: &str) -> Result<Vec<String>> {
        let mut related = Vec::new();
        
        // 同じファイルの他のシンボル
        if let Some(file_prefix) = key.split('#').next() {
            for item in self.db.scan_prefix(file_prefix.as_bytes()).take(10) {
                if let Ok((k, _)) = item {
                    if let Ok(key_str) = String::from_utf8(k.to_vec()) {
                        related.push(key_str);
                    }
                }
            }
        }
        
        Ok(related)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub entries: usize,
    pub total_size_bytes: usize,
    pub avg_access_count: f64,
    pub max_cache_size: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.avg_access_count > 0.0 {
            (self.avg_access_count - 1.0) / self.avg_access_count
        } else {
            0.0
        }
    }
    
    pub fn cache_usage(&self) -> f64 {
        self.entries as f64 / self.max_cache_size as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_hit() {
        let temp_dir = TempDir::new().unwrap();
        let storage = CachedIndexStorage::open(temp_dir.path()).unwrap();
        
        // 最初の保存
        storage.save_data_cached("test_key", &"test_value").unwrap();
        
        // キャッシュから読み込み（高速）
        let result: Option<String> = storage.load_data_cached("test_key").unwrap();
        assert_eq!(result, Some("test_value".to_string()));
        
        // 統計確認
        let stats = storage.get_cache_stats();
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn test_lru_eviction() {
        let temp_dir = TempDir::new().unwrap();
        let storage = CachedIndexStorage::open_with_config(
            temp_dir.path(),
            3,
            Duration::from_secs(60),
        ).unwrap();
        
        // キャッシュサイズを超える数のアイテムを保存
        for i in 0..5 {
            storage.save_data_cached(&format!("key_{}", i), &i).unwrap();
        }
        
        // キャッシュサイズが制限内であることを確認
        let stats = storage.get_cache_stats();
        assert!(stats.entries <= 3);
    }
}