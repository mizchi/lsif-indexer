use anyhow::Result;
use serde::{Serialize, Deserialize};
use std::path::Path;
use std::time::{SystemTime, Instant};
use crate::core::{
    incremental::{IncrementalIndex, UpdateResult},
    Symbol,
};

/// Incremental storage that efficiently updates only changed parts
pub struct IncrementalStorage {
    db: sled::Db,
    index_tree: sled::Tree,
    file_tree: sled::Tree,
    symbol_tree: sled::Tree,
    edge_tree: sled::Tree,
}

impl IncrementalStorage {
    pub fn open(path: &str) -> Result<Self> {
        let db = sled::open(path)?;
        
        let index_tree = db.open_tree("incremental_index")?;
        let file_tree = db.open_tree("file_metadata")?;
        let symbol_tree = db.open_tree("symbols")?;
        let edge_tree = db.open_tree("edges")?;
        
        Ok(Self {
            db,
            index_tree,
            file_tree,
            symbol_tree,
            edge_tree,
        })
    }

    /// Load or create incremental index
    pub fn load_or_create_index(&self) -> Result<IncrementalIndex> {
        if let Some(data) = self.index_tree.get("index")? {
            Ok(bincode::deserialize(&data)?)
        } else {
            Ok(IncrementalIndex::new())
        }
    }

    /// Save only the changed parts of the index
    pub fn save_incremental(&self, index: &IncrementalIndex, result: &UpdateResult) -> Result<StorageMetrics> {
        let start = Instant::now();
        let mut metrics = StorageMetrics::default();
        
        // Start a batch for atomic updates
        let mut batch = sled::Batch::default();
        
        // Update only changed symbols
        for symbol_id in &result.added_symbols {
            if let Some(symbol) = index.graph.find_symbol(symbol_id) {
                let key = format!("symbol:{symbol_id}");
                let value = bincode::serialize(symbol)?;
                batch.insert(key.as_bytes(), value);
                metrics.symbols_written += 1;
            }
        }
        
        for symbol_id in &result.updated_symbols {
            if let Some(symbol) = index.graph.find_symbol(symbol_id) {
                let key = format!("symbol:{symbol_id}");
                let value = bincode::serialize(symbol)?;
                batch.insert(key.as_bytes(), value);
                metrics.symbols_updated += 1;
            }
        }
        
        for symbol_id in &result.removed_symbols {
            let key = format!("symbol:{symbol_id}");
            batch.remove(key.as_bytes());
            metrics.symbols_removed += 1;
        }
        
        // Apply batch atomically
        self.symbol_tree.apply_batch(batch)?;
        
        // Update file metadata for affected files
        let mut file_batch = sled::Batch::default();
        for (path, metadata) in &index.file_metadata {
            if self.is_file_affected(path, result, index) {
                let key = path.to_string_lossy();
                let value = bincode::serialize(metadata)?;
                file_batch.insert(key.as_bytes(), value);
                metrics.files_updated += 1;
            }
        }
        self.file_tree.apply_batch(file_batch)?;
        
        // Save index metadata
        let index_data = bincode::serialize(&IndexMetadata {
            last_update: SystemTime::now(),
            total_symbols: index.graph.symbol_count(),
            dead_symbols: index.dead_symbols.len(),
        })?;
        self.index_tree.insert("metadata", index_data)?;
        
        // Flush to disk
        self.db.flush()?;
        
        metrics.duration_ms = start.elapsed().as_millis() as u64;
        Ok(metrics)
    }

    /// Save entire index (for initial creation or full rebuild)
    pub fn save_full(&self, index: &IncrementalIndex) -> Result<StorageMetrics> {
        let start = Instant::now();
        let mut metrics = StorageMetrics::default();
        
        // Clear existing data
        self.symbol_tree.clear()?;
        self.file_tree.clear()?;
        self.edge_tree.clear()?;
        
        // Save all symbols
        let mut batch = sled::Batch::default();
        for symbol in index.graph.get_all_symbols() {
            let key = format!("symbol:{}", symbol.id);
            let value = bincode::serialize(&symbol)?;
            batch.insert(key.as_bytes(), value);
            metrics.symbols_written += 1;
        }
        self.symbol_tree.apply_batch(batch)?;
        
        // Save all file metadata
        let mut file_batch = sled::Batch::default();
        for (path, metadata) in &index.file_metadata {
            let key = path.to_string_lossy();
            let value = bincode::serialize(metadata)?;
            file_batch.insert(key.as_bytes(), value);
            metrics.files_updated += 1;
        }
        self.file_tree.apply_batch(file_batch)?;
        
        // Save full index
        let index_data = bincode::serialize(index)?;
        self.index_tree.insert("index", index_data)?;
        
        self.db.flush()?;
        
        metrics.duration_ms = start.elapsed().as_millis() as u64;
        metrics.is_full_save = true;
        Ok(metrics)
    }

    fn is_file_affected(&self, path: &Path, result: &UpdateResult, index: &IncrementalIndex) -> bool {
        // Check if any symbols from this file were modified
        if let Some(metadata) = index.file_metadata.get(path) {
            for symbol_id in &metadata.symbols {
                if result.added_symbols.contains(symbol_id) ||
                   result.updated_symbols.contains(symbol_id) ||
                   result.removed_symbols.contains(symbol_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Load specific symbols by IDs
    pub fn load_symbols(&self, symbol_ids: &[String]) -> Result<Vec<Symbol>> {
        let mut symbols = Vec::new();
        
        for symbol_id in symbol_ids {
            let key = format!("symbol:{symbol_id}");
            if let Some(data) = self.symbol_tree.get(key)? {
                let symbol: Symbol = bincode::deserialize(&data)?;
                symbols.push(symbol);
            }
        }
        
        Ok(symbols)
    }

    /// Get storage statistics
    pub fn get_stats(&self) -> Result<StorageStats> {
        Ok(StorageStats {
            total_symbols: self.symbol_tree.len(),
            total_files: self.file_tree.len(),
            total_edges: self.edge_tree.len(),
            db_size_bytes: self.db.size_on_disk()?,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct StorageMetrics {
    pub duration_ms: u64,
    pub symbols_written: usize,
    pub symbols_updated: usize,
    pub symbols_removed: usize,
    pub files_updated: usize,
    pub is_full_save: bool,
}

impl StorageMetrics {
    pub fn summary(&self) -> String {
        if self.is_full_save {
            format!(
                "Full save: {} symbols, {} files in {}ms",
                self.symbols_written, self.files_updated, self.duration_ms
            )
        } else {
            format!(
                "Incremental save: +{} ~{} -{} symbols, {} files in {}ms",
                self.symbols_written, self.symbols_updated, self.symbols_removed,
                self.files_updated, self.duration_ms
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_symbols: usize,
    pub total_files: usize,
    pub total_edges: usize,
    pub db_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexMetadata {
    last_update: SystemTime,
    total_symbols: usize,
    dead_symbols: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{SymbolKind, Range, Position};
    use tempfile::tempdir;

    fn create_test_symbol(id: &str) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: id.to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position { line: 1, character: 0 },
                end: Position { line: 5, character: 0 },
            },
            documentation: None,
        }
    }

    #[test]
    fn test_incremental_save() {
        let dir = tempdir().unwrap();
        let storage = IncrementalStorage::open(dir.path().join("test.db").to_str().unwrap()).unwrap();
        
        let mut index = IncrementalIndex::new();
        
        // Initial save
        let symbols = vec![
            create_test_symbol("func1"),
            create_test_symbol("func2"),
        ];
        
        let result = index.update_file(
            Path::new("test.rs"),
            symbols,
            "hash1".to_string()
        ).unwrap();
        
        let metrics = storage.save_incremental(&index, &result).unwrap();
        assert_eq!(metrics.symbols_written, 2);
        
        // Incremental update
        let updated_symbols = vec![
            create_test_symbol("func2"),
            create_test_symbol("func3"),
        ];
        
        let result = index.update_file(
            Path::new("test.rs"),
            updated_symbols,
            "hash2".to_string()
        ).unwrap();
        
        let metrics = storage.save_incremental(&index, &result).unwrap();
        assert_eq!(metrics.symbols_written, 1); // Only func3 is new
        assert_eq!(metrics.symbols_removed, 1); // func1 was removed
    }

    #[test]
    fn test_full_vs_incremental_performance() {
        let dir = tempdir().unwrap();
        let storage = IncrementalStorage::open(dir.path().join("test.db").to_str().unwrap()).unwrap();
        
        let mut index = IncrementalIndex::new();
        
        // Create many symbols
        let mut symbols = Vec::new();
        for i in 0..100 {
            symbols.push(Symbol {
                id: format!("func{}", i),
                name: format!("function_{}", i),
                kind: SymbolKind::Function,
                file_path: format!("file{}.rs", i / 10),
                range: Range {
                    start: Position { line: i * 10, character: 0 },
                    end: Position { line: i * 10 + 5, character: 0 },
                },
                documentation: Some(format!("Doc for func{}", i)),
            });
        }
        
        // Initial full save
        for symbol in symbols {
            index.add_symbol(symbol).unwrap();
        }
        
        let full_metrics = storage.save_full(&index).unwrap();
        assert!(full_metrics.is_full_save);
        assert_eq!(full_metrics.symbols_written, 100);
        
        // Small incremental update (should be much faster)
        let small_update = vec![create_test_symbol("new_func")];
        let result = index.update_file(
            Path::new("new.rs"),
            small_update,
            "new_hash".to_string()
        ).unwrap();
        
        let incr_metrics = storage.save_incremental(&index, &result).unwrap();
        assert!(!incr_metrics.is_full_save);
        assert_eq!(incr_metrics.symbols_written, 1);
        
        // Incremental should be faster than full for small changes
        println!("Full save: {}ms, Incremental: {}ms", 
                 full_metrics.duration_ms, incr_metrics.duration_ms);
    }
}