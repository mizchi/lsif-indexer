use super::graph::{CodeGraph, EdgeKind, Symbol};
use anyhow::Result;
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub last_modified: SystemTime,
    pub symbols: HashSet<String>,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IncrementalIndex {
    pub graph: CodeGraph,
    pub file_metadata: HashMap<PathBuf, FileMetadata>,
    pub symbol_to_file: HashMap<String, PathBuf>,
    pub dead_symbols: HashSet<String>,
}

impl IncrementalIndex {
    pub fn new() -> Self {
        Self {
            graph: CodeGraph::new(),
            file_metadata: HashMap::new(),
            symbol_to_file: HashMap::new(),
            dead_symbols: HashSet::new(),
        }
    }

    pub fn from_graph(graph: CodeGraph) -> Self {
        let mut symbol_to_file = HashMap::new();
        let mut file_metadata = HashMap::new();

        // Build symbol to file mapping
        for symbol in graph.get_all_symbols() {
            let path = PathBuf::from(&symbol.file_path);
            symbol_to_file.insert(symbol.id.clone(), path.clone());

            file_metadata
                .entry(path.clone())
                .or_insert_with(|| FileMetadata {
                    path: path.clone(),
                    last_modified: SystemTime::now(),
                    symbols: HashSet::new(),
                    hash: String::new(),
                })
                .symbols
                .insert(symbol.id.clone());
        }

        Self {
            graph,
            file_metadata,
            symbol_to_file,
            dead_symbols: HashSet::new(),
        }
    }

    /// Update the index with changes from a specific file
    pub fn update_file(
        &mut self,
        file_path: &Path,
        new_symbols: Vec<Symbol>,
        file_hash: String,
    ) -> Result<UpdateResult> {
        let mut result = UpdateResult::default();

        // Get old symbols for this file
        let old_symbols = self
            .file_metadata
            .get(file_path)
            .map(|meta| meta.symbols.clone())
            .unwrap_or_default();

        // Find removed symbols (potential dead code)
        for old_symbol_id in &old_symbols {
            if !new_symbols.iter().any(|s| &s.id == old_symbol_id) {
                result.removed_symbols.insert(old_symbol_id.clone());
                self.remove_symbol(old_symbol_id)?;
                self.symbol_to_file.remove(old_symbol_id);
                self.mark_as_potentially_dead(old_symbol_id);
            }
        }

        // Process new and updated symbols
        let mut new_symbol_ids = HashSet::new();
        for symbol in new_symbols {
            let symbol_id = symbol.id.clone();
            new_symbol_ids.insert(symbol_id.clone());

            if old_symbols.contains(&symbol_id) {
                // Update existing symbol
                self.update_symbol(symbol)?;
                result.updated_symbols.insert(symbol_id);
            } else {
                // Add new symbol
                self.add_symbol(symbol)?;
                result.added_symbols.insert(symbol_id);
            }
        }

        // Update file metadata
        self.file_metadata.insert(
            file_path.to_path_buf(),
            FileMetadata {
                path: file_path.to_path_buf(),
                last_modified: SystemTime::now(),
                symbols: new_symbol_ids,
                hash: file_hash,
            },
        );

        // Check for dead code
        self.detect_dead_code(&mut result);

        Ok(result)
    }

    /// Remove a file from the index
    pub fn remove_file(&mut self, file_path: &Path) -> Result<UpdateResult> {
        let mut result = UpdateResult::default();

        if let Some(metadata) = self.file_metadata.remove(file_path) {
            for symbol_id in metadata.symbols {
                result.removed_symbols.insert(symbol_id.clone());
                self.remove_symbol(&symbol_id)?;
                self.symbol_to_file.remove(&symbol_id);
            }
        }

        // Detect newly dead code after removal
        self.detect_dead_code(&mut result);

        Ok(result)
    }

    pub fn add_symbol(&mut self, symbol: Symbol) -> Result<()> {
        let path = PathBuf::from(&symbol.file_path);
        self.symbol_to_file.insert(symbol.id.clone(), path);
        self.graph.add_symbol(symbol);
        Ok(())
    }

    fn update_symbol(&mut self, symbol: Symbol) -> Result<()> {
        // Remove old symbol
        if let Some(node_idx) = self.graph.get_node_index(&symbol.id) {
            self.graph.graph.remove_node(node_idx);
            self.graph.symbol_index.remove(&symbol.id);
        }

        // Add updated symbol
        self.add_symbol(symbol)?;
        Ok(())
    }

    fn remove_symbol(&mut self, symbol_id: &str) -> Result<()> {
        if let Some(node_idx) = self.graph.get_node_index(symbol_id) {
            self.graph.graph.remove_node(node_idx);
            self.graph.symbol_index.remove(symbol_id);
        }
        Ok(())
    }

    fn mark_as_potentially_dead(&mut self, symbol_id: &str) {
        self.dead_symbols.insert(symbol_id.to_string());
    }

    pub fn detect_dead_code(&mut self, result: &mut UpdateResult) {
        let mut live_symbols = HashSet::new();
        let mut to_visit = Vec::new();

        // Start from entry points (main functions, exported symbols, tests)
        for symbol in self.graph.get_all_symbols() {
            if self.is_entry_point(symbol) {
                live_symbols.insert(symbol.id.clone());
                to_visit.push(symbol.id.clone());
            }
        }

        // Traverse the graph to find all reachable symbols
        while let Some(symbol_id) = to_visit.pop() {
            if let Some(node_idx) = self.graph.get_node_index(&symbol_id) {
                // Find all symbols referenced by this symbol
                for edge in self.graph.graph.edges(node_idx) {
                    if matches!(edge.weight(), EdgeKind::Reference | EdgeKind::Definition) {
                        if let Some(target_symbol) = self.graph.graph.node_weight(edge.target()) {
                            if live_symbols.insert(target_symbol.id.clone()) {
                                to_visit.push(target_symbol.id.clone());
                            }
                        }
                    }
                }
            }
        }

        // Find dead symbols
        for symbol in self.graph.get_all_symbols() {
            if !live_symbols.contains(&symbol.id) {
                result.dead_symbols.insert(symbol.id.clone());
                self.dead_symbols.insert(symbol.id.clone());
            }
        }
    }

    fn is_entry_point(&self, symbol: &Symbol) -> bool {
        // Main functions
        if symbol.name == "main" || symbol.name.ends_with("::main") {
            return true;
        }

        // Public API (pub functions/types)
        if symbol.name.starts_with("pub ") {
            return true;
        }

        // Test functions
        if symbol.name.contains("test") || symbol.name.contains("bench") {
            return true;
        }

        // Library entry points
        if symbol.file_path.contains("lib.rs") && symbol.name.starts_with("pub") {
            return true;
        }

        false
    }

    /// Get all currently dead symbols
    pub fn get_dead_symbols(&self) -> &HashSet<String> {
        &self.dead_symbols
    }

    /// Check if a file needs updating
    pub fn needs_update(&self, file_path: &Path, current_hash: &str) -> bool {
        self.file_metadata
            .get(file_path)
            .map(|meta| meta.hash != current_hash)
            .unwrap_or(true)
    }

    /// Batch update multiple files
    pub fn batch_update(&mut self, updates: Vec<FileUpdate>) -> Result<BatchUpdateResult> {
        let mut batch_result = BatchUpdateResult::default();

        for update in updates {
            let result = match update {
                FileUpdate::Modified {
                    path,
                    symbols,
                    hash,
                } => self.update_file(&path, symbols, hash)?,
                FileUpdate::Removed { path } => self.remove_file(&path)?,
                FileUpdate::Added {
                    path,
                    symbols,
                    hash,
                } => self.update_file(&path, symbols, hash)?,
            };

            batch_result.merge(result);
        }

        Ok(batch_result)
    }
}

#[derive(Debug, Clone, Default)]
pub struct UpdateResult {
    pub added_symbols: HashSet<String>,
    pub removed_symbols: HashSet<String>,
    pub updated_symbols: HashSet<String>,
    pub dead_symbols: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BatchUpdateResult {
    pub total_added: usize,
    pub total_removed: usize,
    pub total_updated: usize,
    pub total_dead: usize,
    pub affected_files: usize,
}

impl BatchUpdateResult {
    fn merge(&mut self, result: UpdateResult) {
        self.total_added += result.added_symbols.len();
        self.total_removed += result.removed_symbols.len();
        self.total_updated += result.updated_symbols.len();
        self.total_dead = result.dead_symbols.len(); // Use latest dead count
        self.affected_files += 1;
    }
}

#[derive(Debug, Clone)]
pub enum FileUpdate {
    Modified {
        path: PathBuf,
        symbols: Vec<Symbol>,
        hash: String,
    },
    Removed {
        path: PathBuf,
    },
    Added {
        path: PathBuf,
        symbols: Vec<Symbol>,
        hash: String,
    },
}

/// Calculate file hash for change detection
pub fn calculate_file_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range, SymbolKind};

    fn create_test_symbol(id: &str, file: &str) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: id.to_string(),
            kind: SymbolKind::Function,
            file_path: file.to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 0,
                },
            },
            documentation: None,
            detail: None,
        }
    }

    #[test]
    fn test_incremental_update() {
        let mut index = IncrementalIndex::new();

        // Initial file
        let symbols = vec![
            create_test_symbol("func1", "file1.rs"),
            create_test_symbol("func2", "file1.rs"),
        ];

        let result = index
            .update_file(Path::new("file1.rs"), symbols, "hash1".to_string())
            .unwrap();

        assert_eq!(result.added_symbols.len(), 2);
        assert_eq!(result.removed_symbols.len(), 0);

        // Update file - remove func1, add func3
        let updated_symbols = vec![
            create_test_symbol("func2", "file1.rs"),
            create_test_symbol("func3", "file1.rs"),
        ];

        let result = index
            .update_file(Path::new("file1.rs"), updated_symbols, "hash2".to_string())
            .unwrap();

        assert_eq!(result.added_symbols.len(), 1);
        assert_eq!(result.removed_symbols.len(), 1);
        assert!(result.removed_symbols.contains("func1"));
    }

    #[test]
    fn test_dead_code_detection() {
        let mut index = IncrementalIndex::new();

        // Add main and helper functions
        let symbols = vec![
            create_test_symbol("main", "main.rs"),
            create_test_symbol("used_func", "lib.rs"),
            create_test_symbol("unused_func", "lib.rs"),
        ];

        for symbol in symbols {
            index.add_symbol(symbol).unwrap();
        }

        // Add edge from main to used_func
        let main_idx = index.graph.get_node_index("main").unwrap();
        let used_idx = index.graph.get_node_index("used_func").unwrap();
        index
            .graph
            .add_edge(main_idx, used_idx, EdgeKind::Reference);

        // Detect dead code
        let mut result = UpdateResult::default();
        index.detect_dead_code(&mut result);

        assert!(result.dead_symbols.contains("unused_func"));
        assert!(!result.dead_symbols.contains("main"));
        assert!(!result.dead_symbols.contains("used_func"));
    }

    #[test]
    fn test_file_removal() {
        let mut index = IncrementalIndex::new();

        // Add file with symbols
        let symbols = vec![
            create_test_symbol("func1", "file1.rs"),
            create_test_symbol("func2", "file1.rs"),
        ];

        index
            .update_file(Path::new("file1.rs"), symbols, "hash1".to_string())
            .unwrap();

        // Remove file
        let result = index.remove_file(Path::new("file1.rs")).unwrap();

        assert_eq!(result.removed_symbols.len(), 2);
        assert!(result.removed_symbols.contains("func1"));
        assert!(result.removed_symbols.contains("func2"));
        assert_eq!(index.graph.symbol_count(), 0);
    }
}
