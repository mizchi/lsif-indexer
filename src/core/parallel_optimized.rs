use super::graph::{CodeGraph, Symbol, EdgeKind};
use super::incremental::{IncrementalIndex, FileUpdate, UpdateResult, BatchUpdateResult};
use anyhow::Result;
use rayon::prelude::*;
use std::sync::{Arc, RwLock};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

/// ロックフリーな並列シンボル追加の最適化版
pub struct OptimizedParallelGraph;

impl OptimizedParallelGraph {
    /// バッチでシンボルを追加（チャンク単位で並列処理）
    pub fn add_symbols_batch(graph: &mut CodeGraph, symbols: Vec<Symbol>) -> Vec<NodeIndex> {
        // 小さなバッチはオーバーヘッドの方が大きいので逐次処理
        if symbols.len() < 100 {
            return symbols.into_iter()
                .map(|symbol| graph.add_symbol(symbol))
                .collect();
        }
        
        // チャンクごとに処理してからマージ
        let chunk_size = 1000;
        let chunks: Vec<_> = symbols.chunks(chunk_size).collect();
        
        if chunks.len() == 1 {
            // 単一チャンクの場合は直接処理
            return symbols.into_iter()
                .map(|symbol| graph.add_symbol(symbol))
                .collect();
        }
        
        // 各チャンクを並列で処理してサブグラフを作成
        let sub_graphs: Vec<(CodeGraph, Vec<NodeIndex>)> = chunks
            .into_par_iter()
            .map(|chunk| {
                let mut sub_graph = CodeGraph::new();
                let indices: Vec<_> = chunk.iter()
                    .map(|symbol| sub_graph.add_symbol(symbol.clone()))
                    .collect();
                (sub_graph, indices)
            })
            .collect();
        
        // サブグラフをメイングラフにマージ
        let mut all_indices = Vec::with_capacity(symbols.len());
        
        for (sub_graph, indices) in sub_graphs {
            let index_mapping = merge_subgraph(graph, sub_graph);
            for idx in indices {
                if let Some(&new_idx) = index_mapping.get(&idx) {
                    all_indices.push(new_idx);
                }
            }
        }
        
        all_indices
    }
    
    /// 複数ファイルの並列解析（RwLock使用）
    pub fn analyze_files_parallel<F, R>(
        files: Vec<PathBuf>,
        analyzer: F,
    ) -> Vec<Result<(PathBuf, R)>>
    where
        F: Fn(&Path) -> Result<R> + Send + Sync,
        R: Send,
    {
        files
            .into_par_iter()
            .map(|path| {
                let result = analyzer(&path)?;
                Ok((path, result))
            })
            .collect()
    }
}

/// サブグラフをメイングラフにマージ
fn merge_subgraph(main_graph: &mut CodeGraph, sub_graph: CodeGraph) -> HashMap<NodeIndex, NodeIndex> {
    let mut index_mapping = HashMap::new();
    
    // すべてのシンボルを追加
    for symbol in sub_graph.get_all_symbols() {
        let old_idx = sub_graph.get_node_index(&symbol.id).unwrap();
        let new_idx = main_graph.add_symbol(symbol.clone());
        index_mapping.insert(old_idx, new_idx);
    }
    
    // すべてのエッジを追加
    for edge in sub_graph.graph.edge_indices() {
        if let Some((from, to)) = sub_graph.graph.edge_endpoints(edge) {
            if let (Some(&new_from), Some(&new_to)) = (
                index_mapping.get(&from),
                index_mapping.get(&to)
            ) {
                if let Some(edge_kind) = sub_graph.graph.edge_weight(edge) {
                    main_graph.add_edge(new_from, new_to, edge_kind.clone());
                }
            }
        }
    }
    
    index_mapping
}

/// 最適化された並列インクリメンタルインデックス
pub struct OptimizedParallelIndex {
    index: Arc<RwLock<IncrementalIndex>>,
}

impl OptimizedParallelIndex {
    pub fn new() -> Self {
        Self {
            index: Arc::new(RwLock::new(IncrementalIndex::new())),
        }
    }
    
    pub fn from_index(index: IncrementalIndex) -> Self {
        Self {
            index: Arc::new(RwLock::new(index)),
        }
    }
    
    /// ファイル単位でグループ化して並列更新
    pub fn batch_update_files(&self, updates: Vec<FileUpdate>) -> Result<BatchUpdateResult> {
        // ファイルパスでグループ化
        let mut grouped_updates: HashMap<PathBuf, Vec<FileUpdate>> = HashMap::new();
        
        for update in updates {
            let path = match &update {
                FileUpdate::Modified { path, .. } => path.clone(),
                FileUpdate::Removed { path } => path.clone(),
                FileUpdate::Added { path, .. } => path.clone(),
            };
            grouped_updates.entry(path).or_insert_with(Vec::new).push(update);
        }
        
        // 各ファイルグループを並列処理
        let results: Vec<UpdateResult> = grouped_updates
            .into_par_iter()
            .map(|(_, file_updates)| {
                let mut local_result = UpdateResult::default();
                
                for update in file_updates {
                    let mut index = self.index.write().unwrap();
                    let result = match update {
                        FileUpdate::Modified { path, symbols, hash } => {
                            index.update_file(&path, symbols, hash)
                        }
                        FileUpdate::Removed { path } => {
                            index.remove_file(&path)
                        }
                        FileUpdate::Added { path, symbols, hash } => {
                            index.update_file(&path, symbols, hash)
                        }
                    }?;
                    
                    // 結果をマージ
                    local_result.added_symbols.extend(result.added_symbols);
                    local_result.removed_symbols.extend(result.removed_symbols);
                    local_result.updated_symbols.extend(result.updated_symbols);
                    local_result.dead_symbols.extend(result.dead_symbols);
                }
                
                Ok(local_result)
            })
            .collect::<Result<Vec<_>>>()?;
        
        // 全結果を集計
        let mut batch_result = BatchUpdateResult::default();
        for result in results {
            batch_result.total_added += result.added_symbols.len();
            batch_result.total_removed += result.removed_symbols.len();
            batch_result.total_updated += result.updated_symbols.len();
            batch_result.total_dead = result.dead_symbols.len();
            batch_result.affected_files += 1;
        }
        
        Ok(batch_result)
    }
    
    /// ファイルハッシュの並列計算（最適化版）
    pub fn calculate_file_hashes_parallel(
        files: Vec<(PathBuf, String)>
    ) -> HashMap<PathBuf, String> {
        use super::incremental::calculate_file_hash;
        
        files
            .into_par_iter()
            .map(|(path, content)| {
                let hash = calculate_file_hash(&content);
                (path, hash)
            })
            .collect()
    }
    
    pub fn into_inner(self) -> IncrementalIndex {
        Arc::try_unwrap(self.index)
            .map(|rwlock| rwlock.into_inner().unwrap())
            .unwrap_or_else(|arc| arc.read().unwrap().clone())
    }
}

/// 並列デッドコード検出の最適化版
pub struct OptimizedDeadCodeDetector;

impl OptimizedDeadCodeDetector {
    pub fn detect_parallel(graph: &CodeGraph) -> HashSet<String> {
        // エントリーポイントを並列で収集
        let entry_points: HashSet<String> = graph
            .get_all_symbols()
            .par_bridge()
            .filter(|symbol| Self::is_entry_point(symbol))
            .map(|symbol| symbol.id.clone())
            .collect();
        
        // グラフトラバーサル（BFS）- これは逐次的に行う必要がある
        let mut live_symbols = entry_points.clone();
        let mut to_visit: Vec<String> = entry_points.into_iter().collect();
        
        while let Some(symbol_id) = to_visit.pop() {
            if let Some(node_idx) = graph.get_node_index(&symbol_id) {
                for edge in graph.graph.edges(node_idx) {
                    if matches!(edge.weight(), EdgeKind::Reference | EdgeKind::Definition) {
                        if let Some(target_symbol) = graph.graph.node_weight(edge.target()) {
                            if live_symbols.insert(target_symbol.id.clone()) {
                                to_visit.push(target_symbol.id.clone());
                            }
                        }
                    }
                }
            }
        }
        
        // デッドシンボルを並列で検出（大規模グラフで効果的）
        if graph.symbol_count() > 1000 {
            graph.get_all_symbols()
                .par_bridge()
                .filter(|symbol| !live_symbols.contains(&symbol.id))
                .map(|symbol| symbol.id.clone())
                .collect()
        } else {
            // 小規模グラフでは逐次処理の方が効率的
            graph.get_all_symbols()
                .filter(|symbol| !live_symbols.contains(&symbol.id))
                .map(|symbol| symbol.id.clone())
                .collect()
        }
    }
    
    fn is_entry_point(symbol: &Symbol) -> bool {
        symbol.name == "main" || 
        symbol.name.ends_with("::main") ||
        symbol.name.starts_with("pub ") ||
        symbol.name.contains("test") ||
        symbol.name.contains("bench") ||
        (symbol.file_path.contains("lib.rs") && symbol.name.starts_with("pub"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{SymbolKind, Range, Position};
    
    fn create_test_symbol(id: &str) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: id.to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 1, character: 0 },
            },
            documentation: None,
        }
    }
    
    #[test]
    fn test_optimized_batch_add() {
        let mut graph = CodeGraph::new();
        
        let symbols: Vec<Symbol> = (0..2000)
            .map(|i| create_test_symbol(&format!("sym_{}", i)))
            .collect();
        
        let indices = OptimizedParallelGraph::add_symbols_batch(&mut graph, symbols);
        
        assert_eq!(indices.len(), 2000);
        assert_eq!(graph.symbol_count(), 2000);
    }
    
    #[test]
    fn test_small_batch_sequential() {
        let mut graph = CodeGraph::new();
        
        let symbols: Vec<Symbol> = (0..50)
            .map(|i| create_test_symbol(&format!("small_{}", i)))
            .collect();
        
        let indices = OptimizedParallelGraph::add_symbols_batch(&mut graph, symbols);
        
        assert_eq!(indices.len(), 50);
        assert_eq!(graph.symbol_count(), 50);
    }
}