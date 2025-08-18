use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sled::transaction::ConflictableTransactionError;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub struct ParallelIndexStorage {
    db: Arc<sled::Db>,
}

impl ParallelIndexStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self { db: Arc::new(db) })
    }

    /// 複数のシンボルを並列で保存
    pub fn save_symbols_parallel<T: Serialize + Send + Sync>(
        &self,
        symbols: &[(String, T)],
    ) -> Result<()> {
        let results: Vec<Result<()>> = symbols
            .par_iter()
            .map(|(key, data)| {
                let serialized = bincode::serialize(data)?;
                self.db.insert(key.as_bytes(), serialized)?;
                Ok(())
            })
            .collect();

        // エラーチェック
        for result in results {
            result?;
        }

        self.db.flush()?;
        Ok(())
    }

    /// チャンクサイズを指定した並列保存
    pub fn save_symbols_chunked<T: Serialize + Send + Sync>(
        &self,
        symbols: &[(String, T)],
        chunk_size: usize,
    ) -> Result<()> {
        symbols
            .par_chunks(chunk_size)
            .try_for_each(|chunk| -> Result<()> {
                let batch = self.db.transaction(|tx| {
                    for (key, data) in chunk {
                        let serialized = bincode::serialize(data).map_err(|e| {
                            ConflictableTransactionError::Abort(anyhow::anyhow!("{}", e))
                        })?;
                        tx.insert(key.as_bytes(), serialized)?;
                    }
                    Ok(())
                });
                batch.map_err(|e| anyhow::anyhow!("Transaction failed: {:?}", e))
            })?;

        self.db.flush()?;
        Ok(())
    }

    /// バッチトランザクションを使用した保存
    pub fn save_batch<T: Serialize>(&self, data: HashMap<String, T>) -> Result<()> {
        let mut batch = sled::Batch::default();

        for (key, value) in data {
            let serialized = bincode::serialize(&value)?;
            batch.insert(key.as_bytes(), serialized);
        }

        self.db.apply_batch(batch)?;
        self.db.flush()?;
        Ok(())
    }

    /// 複数のキーを並列で読み込み
    pub fn load_symbols_parallel<T: for<'de> Deserialize<'de> + Send>(
        &self,
        keys: &[String],
    ) -> Result<Vec<Option<T>>> {
        let results: Vec<Option<T>> = keys
            .par_iter()
            .map(|key| {
                if let Some(data) = self.db.get(key).ok()? {
                    bincode::deserialize(&data).ok()
                } else {
                    None
                }
            })
            .collect();

        Ok(results)
    }

    /// 並列でプレフィックス検索
    pub fn scan_parallel<T: for<'de> Deserialize<'de> + Send>(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, T)>> {
        let items: Vec<_> = self
            .db
            .scan_prefix(prefix.as_bytes())
            .filter_map(|item| item.ok())
            .collect();

        let results: Vec<(String, T)> = items
            .par_iter()
            .filter_map(|(key, value)| {
                let key_str = String::from_utf8(key.to_vec()).ok()?;
                let data: T = bincode::deserialize(value).ok()?;
                Some((key_str, data))
            })
            .collect();

        Ok(results)
    }

    /// 非同期バックグラウンド保存
    pub fn save_async<T: Serialize + Send + 'static>(&self, key: String, data: T) -> Result<()> {
        let db = Arc::clone(&self.db);

        std::thread::spawn(move || {
            if let Ok(serialized) = bincode::serialize(&data) {
                let _ = db.insert(key.as_bytes(), serialized);
                let _ = db.flush();
            }
        });

        Ok(())
    }

    /// 統計情報の取得
    pub fn get_stats(&self) -> Result<StorageStats> {
        let size = self.db.size_on_disk()?;
        let len = self.db.len();

        Ok(StorageStats {
            total_size_bytes: size,
            total_entries: len,
            compression_ratio: self.calculate_compression_ratio()?,
        })
    }

    fn calculate_compression_ratio(&self) -> Result<f64> {
        // 簡易的な圧縮率計算
        let mut uncompressed_size = 0usize;
        let mut compressed_size = 0usize;

        for item in self.db.iter().take(100) {
            if let Ok((key, value)) = item {
                uncompressed_size += key.len() + value.len();
                compressed_size += value.len();
            }
        }

        if uncompressed_size > 0 {
            Ok(compressed_size as f64 / uncompressed_size as f64)
        } else {
            Ok(1.0)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_size_bytes: u64,
    pub total_entries: usize,
    pub compression_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parallel_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let storage = ParallelIndexStorage::open(temp_dir.path()).unwrap();

        let symbols: Vec<(String, String)> = (0..1000)
            .map(|i| (format!("key_{i}"), format!("value_{i}")))
            .collect();

        storage.save_symbols_parallel(&symbols).unwrap();

        let keys: Vec<String> = (0..1000).map(|i| format!("key_{i}")).collect();
        let loaded: Vec<Option<String>> = storage.load_symbols_parallel(&keys).unwrap();

        assert_eq!(loaded.len(), 1000);
        assert!(loaded.iter().all(|v| v.is_some()));
    }

    #[test]
    fn test_chunked_save() {
        let temp_dir = TempDir::new().unwrap();
        let storage = ParallelIndexStorage::open(temp_dir.path()).unwrap();

        let symbols: Vec<(String, i32)> = (0..10000).map(|i| (format!("chunk_{i}"), i)).collect();

        storage.save_symbols_chunked(&symbols, 100).unwrap();

        let keys: Vec<String> = vec!["chunk_0".to_string(), "chunk_9999".to_string()];
        let loaded: Vec<Option<i32>> = storage.load_symbols_parallel(&keys).unwrap();

        assert_eq!(loaded[0], Some(0));
        assert_eq!(loaded[1], Some(9999));
    }
}
