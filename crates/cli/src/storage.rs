use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub struct IndexStorage {
    pub(crate) db: sled::Db,
    db_path: PathBuf,
}

impl IndexStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        let config = sled::Config::new()
            .path(&db_path)
            .cache_capacity(128 * 1024 * 1024) // 128MB cache
            .flush_every_ms(Some(1000)) // Flush every second
            .mode(sled::Mode::HighThroughput); // Optimized for throughput

        let db = config.open()?;
        Ok(Self { db, db_path })
    }

    /// Open for read-only operations (same as open but with smaller cache)
    pub fn open_read_only<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        // sledのread_onlyモードは削除されたので、小さいキャッシュで代用
        let config = sled::Config::new()
            .path(&db_path)
            .cache_capacity(64 * 1024 * 1024) // 64MB cache for read operations
            .flush_every_ms(Some(5000)); // Less frequent flushes for read-heavy workloads

        let db = config.open()?;
        Ok(Self { db, db_path })
    }

    /// Get database path
    pub fn get_db_path(&self) -> Result<PathBuf> {
        Ok(self.db_path.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_storage_open() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test.db");

        let storage = IndexStorage::open(&storage_path);
        assert!(storage.is_ok());
    }

    #[test]
    fn test_storage_open_read_only() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_ro.db");

        // 最初に通常モードで作成
        let storage = IndexStorage::open(&storage_path).unwrap();
        storage.save_data("test_key", &"test_value").unwrap();
        drop(storage);

        // 読み取り専用モードで開く
        let ro_storage = IndexStorage::open_read_only(&storage_path);
        assert!(ro_storage.is_ok());

        let value: Option<String> = ro_storage.unwrap().load_data("test_key").unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[test]
    fn test_save_and_load_data() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_data.db");
        let storage = IndexStorage::open(&storage_path).unwrap();

        // 文字列データの保存と読み込み
        storage.save_data("string_key", &"test_string").unwrap();
        let loaded: Option<String> = storage.load_data("string_key").unwrap();
        assert_eq!(loaded, Some("test_string".to_string()));

        // 数値データの保存と読み込み
        storage.save_data("number_key", &42i32).unwrap();
        let loaded: Option<i32> = storage.load_data("number_key").unwrap();
        assert_eq!(loaded, Some(42));

        // 複雑な構造体の保存と読み込み
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestStruct {
            name: String,
            value: i32,
        }

        let test_struct = TestStruct {
            name: "test".to_string(),
            value: 100,
        };

        storage.save_data("struct_key", &test_struct).unwrap();
        let loaded: Option<TestStruct> = storage.load_data("struct_key").unwrap();
        assert_eq!(loaded, Some(test_struct));
    }

    #[test]
    fn test_load_nonexistent_key() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_empty.db");
        let storage = IndexStorage::open(&storage_path).unwrap();

        let result: Option<String> = storage.load_data("nonexistent").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_list_keys() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_keys.db");
        let storage = IndexStorage::open(&storage_path).unwrap();

        storage.save_data("key1", &"value1").unwrap();
        storage.save_data("key2", &"value2").unwrap();
        storage.save_data("key3", &"value3").unwrap();

        let keys = storage.list_keys().unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[test]
    fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_delete.db");
        let storage = IndexStorage::open(&storage_path).unwrap();

        storage.save_data("delete_me", &"value").unwrap();

        // データが存在することを確認
        let loaded: Option<String> = storage.load_data("delete_me").unwrap();
        assert_eq!(loaded, Some("value".to_string()));

        // 削除
        storage.delete("delete_me").unwrap();

        // 削除されたことを確認
        let loaded: Option<String> = storage.load_data("delete_me").unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn test_metadata_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_metadata.db");
        let storage = IndexStorage::open(&storage_path).unwrap();

        let metadata = IndexMetadata {
            format: IndexFormat::Lsif,
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now(),
            project_root: "/path/to/project".to_string(),
            files_count: 100,
            symbols_count: 500,
            git_commit_hash: Some("abc123".to_string()),
            file_hashes: {
                let mut hashes = HashMap::new();
                hashes.insert("file1.rs".to_string(), "hash1".to_string());
                hashes.insert("file2.rs".to_string(), "hash2".to_string());
                hashes
            },
        };

        storage.save_metadata(&metadata).unwrap();
        let loaded = storage.load_metadata().unwrap();

        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.version, metadata.version);
        assert_eq!(loaded.project_root, metadata.project_root);
        assert_eq!(loaded.files_count, metadata.files_count);
        assert_eq!(loaded.symbols_count, metadata.symbols_count);
        assert_eq!(loaded.git_commit_hash, metadata.git_commit_hash);
        assert_eq!(loaded.file_hashes.len(), 2);
    }

    #[test]
    fn test_metadata_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_no_metadata.db");
        let storage = IndexStorage::open(&storage_path).unwrap();

        let loaded = storage.load_metadata().unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_index_format() {
        // IndexFormatのシリアライズ/デシリアライズをテスト
        let lsif_format = IndexFormat::Lsif;
        let scip_format = IndexFormat::Scip;

        let lsif_serialized = bincode::serialize(&lsif_format).unwrap();
        let lsif_deserialized: IndexFormat = bincode::deserialize(&lsif_serialized).unwrap();
        assert!(matches!(lsif_deserialized, IndexFormat::Lsif));

        let scip_serialized = bincode::serialize(&scip_format).unwrap();
        let scip_deserialized: IndexFormat = bincode::deserialize(&scip_serialized).unwrap();
        assert!(matches!(scip_deserialized, IndexFormat::Scip));
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_concurrent.db");
        let storage = Arc::new(IndexStorage::open(&storage_path).unwrap());

        let mut handles = vec![];

        // 複数のスレッドから同時にアクセス
        for i in 0..5 {
            let storage_clone = storage.clone();
            let handle = thread::spawn(move || {
                let key = format!("key_{}", i);
                let value = format!("value_{}", i);
                storage_clone.save_data(&key, &value).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // 全てのデータが正しく保存されたか確認
        for i in 0..5 {
            let key = format!("key_{}", i);
            let expected_value = format!("value_{}", i);
            let loaded: Option<String> = storage.load_data(&key).unwrap();
            assert_eq!(loaded, Some(expected_value));
        }
    }
}
// Test differential
