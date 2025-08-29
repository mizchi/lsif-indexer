use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct IndexStorage {
    pub(crate) db: sled::Db,
}

impl IndexStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = sled::Config::new()
            .path(path)
            .cache_capacity(128 * 1024 * 1024) // 128MB cache
            .flush_every_ms(Some(1000)) // Flush every second
            .mode(sled::Mode::HighThroughput); // Optimized for throughput

        let db = config.open()?;
        Ok(Self { db })
    }

    /// Open for read-only operations (same as open but with smaller cache)
    pub fn open_read_only<P: AsRef<Path>>(path: P) -> Result<Self> {
        // sledのread_onlyモードは削除されたので、小さいキャッシュで代用
        let config = sled::Config::new()
            .path(path)
            .cache_capacity(64 * 1024 * 1024) // 64MB cache for read operations
            .flush_every_ms(Some(5000)); // Less frequent flushes for read-heavy workloads

        let db = config.open()?;
        Ok(Self { db })
    }

    pub fn save_data<T: Serialize>(&self, key: &str, data: &T) -> Result<()> {
        let serialized = bincode::serialize(data)?;
        self.db.insert(key, serialized)?;
        self.db.flush()?;
        Ok(())
    }

    pub fn load_data<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        if let Some(data) = self.db.get(key)? {
            let deserialized = bincode::deserialize(&data)?;
            Ok(Some(deserialized))
        } else {
            Ok(None)
        }
    }

    pub fn list_keys(&self) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        for k in self.db.iter().keys().flatten() {
            if let Ok(s) = String::from_utf8(k.to_vec()) {
                keys.push(s);
            }
        }
        Ok(keys)
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        self.db.remove(key)?;
        self.db.flush()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub format: IndexFormat,
    pub version: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub project_root: String,
    pub files_count: usize,
    pub symbols_count: usize,
    /// Gitコミットハッシュ（インデックス時点）
    pub git_commit_hash: Option<String>,
    /// ファイルごとのコンテンツハッシュ
    pub file_hashes: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexFormat {
    Lsif,
    Scip,
}

impl IndexStorage {
    pub fn save_metadata(&self, metadata: &IndexMetadata) -> Result<()> {
        let serialized = bincode::serialize(metadata)?;
        self.db.insert("__metadata__", serialized)?;
        self.db.flush()?;
        Ok(())
    }

    pub fn load_metadata(&self) -> Result<Option<IndexMetadata>> {
        if let Some(data) = self.db.get("__metadata__")? {
            let metadata = bincode::deserialize(&data)?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }
}
