use super::graph::{CodeGraph, Symbol, EdgeKind};
use super::incremental::{IncrementalIndex, FileUpdate, UpdateResult, BatchUpdateResult};
use anyhow::Result;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

/// 並列処理用のCodeGraph実装
pub struct ParallelCodeGraph {
    inner: Arc<Mutex<CodeGraph>>,
}

impl Default for ParallelCodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ParallelCodeGraph {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CodeGraph::new())),
        }
    }
    
    pub fn from_graph(graph: CodeGraph) -> Self {
        Self {
            inner: Arc::new(Mutex::new(graph)),
        }
    }
    
    /// 複数のシンボルを並列で追加
    pub fn add_symbols_parallel(&self, symbols: Vec<Symbol>) -> Vec<NodeIndex> {
        let indices = Arc::new(Mutex::new(Vec::with_capacity(symbols.len())));
        
        symbols.into_par_iter().for_each(|symbol| {
            let mut graph = self.inner.lock().unwrap();
            let idx = graph.add_symbol(symbol);
            indices.lock().unwrap().push(idx);
        });
        
        Arc::try_unwrap(indices).unwrap().into_inner().unwrap()
    }
    
    /// 複数のエッジを並列で追加（バッチ処理）
    pub fn add_edges_parallel(&self, edges: Vec<(NodeIndex, NodeIndex, EdgeKind)>) {
        // エッジはバッチで追加する方が効率的
        let mut graph = self.inner.lock().unwrap();
        for (from, to, kind) in edges {
            graph.add_edge(from, to, kind);
        }
    }
    
    /// 複数のシンボル検索を並列実行
    pub fn find_symbols_parallel(&self, ids: Vec<&str>) -> HashMap<String, Option<Symbol>> {
        ids.par_iter()
            .map(|&id| {
                let graph = self.inner.lock().unwrap();
                let symbol = graph.find_symbol(id).cloned();
                (id.to_string(), symbol)
            })
            .collect()
    }
    
    /// グラフ全体のシンボルを並列処理
    pub fn process_symbols_parallel<F, R>(&self, processor: F) -> Vec<R>
    where
        F: Fn(&Symbol) -> R + Send + Sync,
        R: Send,
    {
        let graph = self.inner.lock().unwrap();
        let symbols: Vec<Symbol> = graph.get_all_symbols().cloned().collect();
        drop(graph); // ロックを解放
        
        symbols.par_iter().map(processor).collect()
    }
    
    pub fn into_inner(self) -> CodeGraph {
        Arc::try_unwrap(self.inner)
            .map(|mutex| mutex.into_inner().unwrap())
            .unwrap_or_else(|arc| arc.lock().unwrap().clone())
    }
}

/// 並列インクリメンタルインデックス
pub struct ParallelIncrementalIndex {
    inner: Arc<Mutex<IncrementalIndex>>,
}

impl Default for ParallelIncrementalIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ParallelIncrementalIndex {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(IncrementalIndex::new())),
        }
    }
    
    pub fn from_index(index: IncrementalIndex) -> Self {
        Self {
            inner: Arc::new(Mutex::new(index)),
        }
    }
    
    /// 複数ファイルの並列更新
    pub fn update_files_parallel(&self, updates: Vec<(PathBuf, Vec<Symbol>, String)>) -> Result<Vec<UpdateResult>> {
        updates
            .into_par_iter()
            .map(|(path, symbols, hash)| {
                let mut index = self.inner.lock().unwrap();
                index.update_file(&path, symbols, hash)
            })
            .collect()
    }
    
    /// 並列バッチ更新（最適化版）
    pub fn batch_update_parallel(&self, updates: Vec<FileUpdate>) -> Result<BatchUpdateResult> {
        // ファイルごとにグループ化して並列処理
        let results: Vec<UpdateResult> = updates
            .into_par_iter()
            .map(|update| {
                let mut index = self.inner.lock().unwrap();
                match update {
                    FileUpdate::Modified { path, symbols, hash } => {
                        index.update_file(&path, symbols, hash)
                    }
                    FileUpdate::Removed { path } => {
                        index.remove_file(&path)
                    }
                    FileUpdate::Added { path, symbols, hash } => {
                        index.update_file(&path, symbols, hash)
                    }
                }
            })
            .collect::<Result<Vec<_>>>()?;
        
        // 結果を集計
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
    
    /// 並列デッドコード検出
    pub fn detect_dead_code_parallel(&self) -> Result<HashSet<String>> {
        let index = self.inner.lock().unwrap();
        let graph = &index.graph;
        
        // エントリーポイントを並列で収集
        let entry_points: HashSet<String> = graph
            .get_all_symbols()
            .par_bridge()
            .filter(|symbol| Self::is_entry_point(symbol))
            .map(|symbol| symbol.id.clone())
            .collect();
        
        // グラフトラバーサル（これは逐次的に行う必要がある）
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
        
        // デッドシンボルを並列で検出
        let all_symbols: Vec<Symbol> = graph.get_all_symbols().cloned().collect();
        let dead_symbols: HashSet<String> = all_symbols
            .par_iter()
            .filter(|symbol| !live_symbols.contains(&symbol.id))
            .map(|symbol| symbol.id.clone())
            .collect();
        
        Ok(dead_symbols)
    }
    
    fn is_entry_point(symbol: &Symbol) -> bool {
        symbol.name == "main" || 
        symbol.name.ends_with("::main") ||
        symbol.name.starts_with("pub ") ||
        symbol.name.contains("test") ||
        symbol.name.contains("bench") ||
        (symbol.file_path.contains("lib.rs") && symbol.name.starts_with("pub"))
    }
    
    pub fn into_inner(self) -> IncrementalIndex {
        Arc::try_unwrap(self.inner)
            .map(|mutex| mutex.into_inner().unwrap())
            .unwrap_or_else(|arc| arc.lock().unwrap().clone())
    }
}

/// ファイル解析の並列処理
pub struct ParallelFileAnalyzer;

impl ParallelFileAnalyzer {
    /// 複数ファイルを並列で解析
    pub fn analyze_files_parallel<F, R>(files: Vec<PathBuf>, analyzer: F) -> Vec<Result<R>>
    where
        F: Fn(&Path) -> Result<R> + Send + Sync,
        R: Send,
    {
        files.par_iter()
            .map(|path| analyzer(path))
            .collect()
    }
    
    /// ファイルハッシュを並列計算
    pub fn calculate_hashes_parallel(contents: Vec<(&PathBuf, String)>) -> HashMap<PathBuf, String> {
        use super::incremental::calculate_file_hash;
        
        contents
            .into_par_iter()
            .map(|(path, content)| {
                let hash = calculate_file_hash(&content);
                (path.clone(), hash)
            })
            .collect()
    }
}

/// LSIF生成の並列処理
pub mod parallel_lsif {
    use super::*;
    use crate::core::lsif::{LsifElement, Vertex, Edge, labels};
    use serde_json::json;
    
    pub struct ParallelLsifGenerator {
        graph: CodeGraph,
        vertex_counter: Arc<Mutex<usize>>,
    }
    
    impl ParallelLsifGenerator {
        pub fn new(graph: CodeGraph) -> Self {
            Self {
                graph,
                vertex_counter: Arc::new(Mutex::new(0)),
            }
        }
        
        fn next_id(&self) -> String {
            let mut counter = self.vertex_counter.lock().unwrap();
            *counter += 1;
            counter.to_string()
        }
        
        pub fn generate_parallel(&self) -> Result<Vec<LsifElement>> {
            let mut elements = Vec::new();
            
            // メタデータとプロジェクトは逐次的に生成
            elements.push(self.generate_metadata()?);
            let project_id = self.next_id();
            elements.push(self.generate_project(&project_id)?);
            
            // ドキュメントごとにグループ化
            let mut documents_map: HashMap<String, Vec<Symbol>> = HashMap::new();
            for symbol in self.graph.get_all_symbols() {
                documents_map
                    .entry(symbol.file_path.clone())
                    .or_default()
                    .push(symbol.clone());
            }
            
            // ドキュメントを並列で処理
            let doc_elements: Vec<Vec<LsifElement>> = documents_map
                .into_par_iter()
                .map(|(file_path, symbols)| {
                    let mut doc_elements = Vec::new();
                    let doc_id = self.next_id();
                    
                    // ドキュメントvertex
                    doc_elements.push(self.generate_document(&doc_id, &file_path));
                    
                    // プロジェクトへのcontainsエッジ
                    doc_elements.push(self.generate_contains_edge(
                        &self.next_id(),
                        &project_id,
                        &doc_id
                    ));
                    
                    // シンボルのrange vertexとエッジ
                    for symbol in symbols {
                        let range_id = self.next_id();
                        doc_elements.push(self.generate_range(&range_id, &symbol));
                        doc_elements.push(self.generate_contains_edge(
                            &self.next_id(),
                            &doc_id,
                            &range_id
                        ));
                        
                        // Result setとhover
                        let result_set_id = self.next_id();
                        doc_elements.push(self.generate_result_set(&result_set_id));
                        doc_elements.push(self.generate_next_edge(
                            &self.next_id(),
                            &range_id,
                            &result_set_id
                        ));
                        
                        if let Some(doc) = &symbol.documentation {
                            let hover_id = self.next_id();
                            doc_elements.push(self.generate_hover(&hover_id, doc));
                            doc_elements.push(self.generate_hover_edge(
                                &self.next_id(),
                                &result_set_id,
                                &hover_id
                            ));
                        }
                    }
                    
                    doc_elements
                })
                .collect();
            
            // 結果をフラット化
            for mut doc_elem in doc_elements {
                elements.append(&mut doc_elem);
            }
            
            Ok(elements)
        }
        
        fn generate_metadata(&self) -> Result<LsifElement> {
            let mut data = HashMap::new();
            data.insert("version".to_string(), json!("0.5.0"));
            data.insert("projectRoot".to_string(), json!("file:///"));
            data.insert("positionEncoding".to_string(), json!("utf-16"));
            data.insert("toolInfo".to_string(), json!({
                "name": "lsif-indexer-parallel",
                "version": "1.0.0"
            }));
            
            Ok(LsifElement::Vertex(Vertex {
                id: self.next_id(),
                element_type: "vertex".to_string(),
                label: labels::METADATA.to_string(),
                data,
            }))
        }
        
        fn generate_project(&self, id: &str) -> Result<LsifElement> {
            let mut data = HashMap::new();
            data.insert("kind".to_string(), json!("rust"));
            
            Ok(LsifElement::Vertex(Vertex {
                id: id.to_string(),
                element_type: "vertex".to_string(),
                label: labels::PROJECT.to_string(),
                data,
            }))
        }
        
        fn generate_document(&self, id: &str, file_path: &str) -> LsifElement {
            let mut data = HashMap::new();
            data.insert("uri".to_string(), json!(format!("file://{}", file_path)));
            data.insert("languageId".to_string(), json!("rust"));
            
            LsifElement::Vertex(Vertex {
                id: id.to_string(),
                element_type: "vertex".to_string(),
                label: labels::DOCUMENT.to_string(),
                data,
            })
        }
        
        fn generate_range(&self, id: &str, symbol: &Symbol) -> LsifElement {
            let mut data = HashMap::new();
            data.insert("start".to_string(), json!({
                "line": symbol.range.start.line,
                "character": symbol.range.start.character
            }));
            data.insert("end".to_string(), json!({
                "line": symbol.range.end.line,
                "character": symbol.range.end.character
            }));
            
            LsifElement::Vertex(Vertex {
                id: id.to_string(),
                element_type: "vertex".to_string(),
                label: labels::RANGE.to_string(),
                data,
            })
        }
        
        fn generate_result_set(&self, id: &str) -> LsifElement {
            LsifElement::Vertex(Vertex {
                id: id.to_string(),
                element_type: "vertex".to_string(),
                label: labels::RESULT_SET.to_string(),
                data: HashMap::new(),
            })
        }
        
        fn generate_hover(&self, id: &str, content: &str) -> LsifElement {
            let mut data = HashMap::new();
            data.insert("result".to_string(), json!({
                "contents": {
                    "kind": "markdown",
                    "value": content
                }
            }));
            
            LsifElement::Vertex(Vertex {
                id: id.to_string(),
                element_type: "vertex".to_string(),
                label: labels::HOVER_RESULT.to_string(),
                data,
            })
        }
        
        fn generate_contains_edge(&self, id: &str, from: &str, to: &str) -> LsifElement {
            LsifElement::Edge(Edge {
                id: id.to_string(),
                element_type: "edge".to_string(),
                label: labels::CONTAINS.to_string(),
                out_v: from.to_string(),
                in_v: to.to_string(),
                data: HashMap::new(),
            })
        }
        
        fn generate_next_edge(&self, id: &str, from: &str, to: &str) -> LsifElement {
            LsifElement::Edge(Edge {
                id: id.to_string(),
                element_type: "edge".to_string(),
                label: labels::NEXT.to_string(),
                out_v: from.to_string(),
                in_v: to.to_string(),
                data: HashMap::new(),
            })
        }
        
        fn generate_hover_edge(&self, id: &str, from: &str, to: &str) -> LsifElement {
            LsifElement::Edge(Edge {
                id: id.to_string(),
                element_type: "edge".to_string(),
                label: labels::TEXTDOCUMENT_HOVER.to_string(),
                out_v: from.to_string(),
                in_v: to.to_string(),
                data: HashMap::new(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Range, Position, SymbolKind};
    
    fn create_test_symbol(id: &str, file: &str) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: id.to_string(),
            kind: SymbolKind::Function,
            file_path: file.to_string(),
            range: Range {
                start: Position { line: 1, character: 0 },
                end: Position { line: 5, character: 0 },
            },
            documentation: None,
        }
    }
    
    #[test]
    fn test_parallel_symbol_addition() {
        let graph = ParallelCodeGraph::new();
        
        let symbols: Vec<Symbol> = (0..100)
            .map(|i| create_test_symbol(&format!("sym_{}", i), "test.rs"))
            .collect();
        
        let indices = graph.add_symbols_parallel(symbols);
        assert_eq!(indices.len(), 100);
        
        let inner = graph.into_inner();
        assert_eq!(inner.symbol_count(), 100);
    }
    
    #[test]
    fn test_parallel_symbol_search() {
        let mut base_graph = CodeGraph::new();
        for i in 0..50 {
            base_graph.add_symbol(create_test_symbol(&format!("sym_{}", i), "test.rs"));
        }
        
        let graph = ParallelCodeGraph::from_graph(base_graph);
        let ids: Vec<&str> = (0..50).map(|i| Box::leak(format!("sym_{}", i).into_boxed_str()) as &str).collect();
        
        let results = graph.find_symbols_parallel(ids);
        assert_eq!(results.len(), 50);
        
        for i in 0..50 {
            assert!(results.get(&format!("sym_{}", i)).unwrap().is_some());
        }
    }
    
    #[test]
    fn test_parallel_file_update() {
        let index = ParallelIncrementalIndex::new();
        
        let updates: Vec<(PathBuf, Vec<Symbol>, String)> = (0..10)
            .map(|i| {
                let path = PathBuf::from(format!("file_{}.rs", i));
                let symbols = vec![
                    create_test_symbol(&format!("file{}_sym1", i), &format!("file_{}.rs", i)),
                    create_test_symbol(&format!("file{}_sym2", i), &format!("file_{}.rs", i)),
                ];
                let hash = format!("hash_{}", i);
                (path, symbols, hash)
            })
            .collect();
        
        let results = index.update_files_parallel(updates).unwrap();
        assert_eq!(results.len(), 10);
        
        for result in results {
            assert_eq!(result.added_symbols.len(), 2);
        }
    }
}