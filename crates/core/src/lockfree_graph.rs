use crossbeam_skiplist::SkipMap;
use crossbeam_queue::SegQueue;
use crossbeam_epoch::{self as epoch, Atomic, Owned};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use parking_lot::RwLock;

use crate::{
    graph::{Symbol, EdgeKind},
};

/// ロックフリーなSymbolエントリ
#[derive(Debug)]
struct LockFreeSymbol {
    pub symbol: Symbol,
    pub version: AtomicU64,
}

impl LockFreeSymbol {
    fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            version: AtomicU64::new(0),
        }
    }
}

/// ロックフリーなエッジ情報
#[derive(Debug, Clone)]
struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

/// ロックフリーデータ構造を使用したグラフ実装
pub struct LockFreeGraph {
    /// Symbol ID -> Symbol のロックフリーマップ (SkipList実装)
    symbols: Arc<SkipMap<String, Arc<LockFreeSymbol>>>,
    /// エッジ情報のロックフリーキュー
    edges: Arc<SegQueue<Edge>>,
    /// エッジインデックス（読み取り最適化のため一部ロック使用）
    edge_index: Arc<RwLock<Vec<Edge>>>,
    /// 統計情報
    stats: Arc<LockFreeStats>,
}

/// ロックフリーな統計情報
struct LockFreeStats {
    symbol_count: AtomicUsize,
    edge_count: AtomicUsize,
    read_ops: AtomicU64,
    write_ops: AtomicU64,
    cas_retries: AtomicU64,
}

impl LockFreeStats {
    fn new() -> Self {
        Self {
            symbol_count: AtomicUsize::new(0),
            edge_count: AtomicUsize::new(0),
            read_ops: AtomicU64::new(0),
            write_ops: AtomicU64::new(0),
            cas_retries: AtomicU64::new(0),
        }
    }
}

impl LockFreeGraph {
    /// 新しいロックフリーグラフを作成
    pub fn new() -> Self {
        Self {
            symbols: Arc::new(SkipMap::new()),
            edges: Arc::new(SegQueue::new()),
            edge_index: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(LockFreeStats::new()),
        }
    }

    /// Symbolを追加（ロックフリー）
    pub fn add_symbol(&self, symbol: Symbol) -> String {
        let id = symbol.id.clone();
        let lockfree_symbol = Arc::new(LockFreeSymbol::new(symbol));
        
        // SkipListへの挿入（ロックフリー）
        self.symbols.insert(id.clone(), lockfree_symbol);
        self.stats.symbol_count.fetch_add(1, Ordering::Relaxed);
        self.stats.write_ops.fetch_add(1, Ordering::Relaxed);
        
        id
    }

    /// バッチでSymbolを追加（並列処理可能）
    pub fn add_symbols_batch(&self, symbols: Vec<Symbol>) {
        use rayon::prelude::*;
        
        symbols.into_par_iter().for_each(|symbol| {
            self.add_symbol(symbol);
        });
    }

    /// エッジを追加（ロックフリー）
    pub fn add_edge(&self, from: String, to: String, kind: EdgeKind) {
        let edge = Edge { from, to, kind };
        
        // ロックフリーキューへの追加
        self.edges.push(edge.clone());
        self.stats.edge_count.fetch_add(1, Ordering::Relaxed);
        
        // インデックスの更新（定期的にバッチ更新）
        if self.stats.edge_count.load(Ordering::Relaxed) % 100 == 0 {
            self.update_edge_index();
        }
    }

    /// エッジインデックスを更新
    fn update_edge_index(&self) {
        let mut new_edges = Vec::new();
        while let Some(edge) = self.edges.pop() {
            new_edges.push(edge);
        }
        
        if !new_edges.is_empty() {
            let mut index = self.edge_index.write();
            index.extend(new_edges);
        }
    }

    /// Symbolを取得（ロックフリー読み取り）
    pub fn get_symbol(&self, id: &str) -> Option<Symbol> {
        self.stats.read_ops.fetch_add(1, Ordering::Relaxed);
        
        self.symbols.get(id).map(|entry| {
            entry.value().symbol.clone()
        })
    }

    /// Symbol数を取得（ロックフリー）
    pub fn symbol_count(&self) -> usize {
        self.stats.symbol_count.load(Ordering::Relaxed)
    }

    /// エッジ数を取得（ロックフリー）
    pub fn edge_count(&self) -> usize {
        self.stats.edge_count.load(Ordering::Relaxed)
    }

    /// 参照を検索
    pub fn find_references(&self, symbol_id: &str) -> Vec<String> {
        self.stats.read_ops.fetch_add(1, Ordering::Relaxed);
        
        // 最新のエッジインデックスを確保
        self.update_edge_index();
        
        let index = self.edge_index.read();
        index
            .iter()
            .filter(|edge| edge.to == symbol_id && edge.kind == EdgeKind::Reference)
            .map(|edge| edge.from.clone())
            .collect()
    }

    /// 全Symbolを取得（スナップショット）
    pub fn all_symbols(&self) -> Vec<Symbol> {
        self.symbols
            .iter()
            .map(|entry| entry.value().symbol.clone())
            .collect()
    }

    /// 統計情報を取得
    pub fn stats(&self) -> LockFreeGraphStats {
        LockFreeGraphStats {
            symbol_count: self.stats.symbol_count.load(Ordering::Relaxed),
            edge_count: self.stats.edge_count.load(Ordering::Relaxed),
            read_ops: self.stats.read_ops.load(Ordering::Relaxed),
            write_ops: self.stats.write_ops.load(Ordering::Relaxed),
            cas_retries: self.stats.cas_retries.load(Ordering::Relaxed),
        }
    }

    /// Compare-And-Swap操作でSymbolを更新
    pub fn update_symbol<F>(&self, id: &str, updater: F) -> bool 
    where
        F: Fn(&Symbol) -> Symbol
    {
        if let Some(entry) = self.symbols.get(id) {
            let lockfree_symbol = entry.value();
            let current_version = lockfree_symbol.version.load(Ordering::SeqCst);
            
            // 新しいSymbolを作成
            let new_symbol = updater(&lockfree_symbol.symbol);
            let new_lockfree = Arc::new(LockFreeSymbol::new(new_symbol));
            
            // CAS操作で更新を試みる
            let mut retries = 0;
            loop {
                if lockfree_symbol.version.compare_exchange(
                    current_version,
                    current_version + 1,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ).is_ok() {
                    // 成功したら新しいSymbolを挿入
                    self.symbols.insert(id.to_string(), new_lockfree);
                    self.stats.write_ops.fetch_add(1, Ordering::Relaxed);
                    return true;
                }
                
                retries += 1;
                self.stats.cas_retries.fetch_add(1, Ordering::Relaxed);
                
                if retries > 3 {
                    // リトライ制限に達した
                    return false;
                }
            }
        }
        false
    }
}

impl Default for LockFreeGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// 統計情報
#[derive(Debug, Clone)]
pub struct LockFreeGraphStats {
    pub symbol_count: usize,
    pub edge_count: usize,
    pub read_ops: u64,
    pub write_ops: u64,
    pub cas_retries: u64,
}

/// Wait-Freeな読み取り最適化グラフ
pub struct WaitFreeReadGraph {
    /// 読み取り専用のSymbolマップ（RCU風実装）
    symbols: Arc<Atomic<Arc<SkipMap<String, Symbol>>>>,
    /// 書き込み用のキュー
    write_queue: Arc<SegQueue<WriteOp>>,
    /// バージョン番号
    version: Arc<AtomicU64>,
    /// 統計
    stats: Arc<LockFreeStats>,
}

enum WriteOp {
    AddSymbol(Symbol),
    #[allow(dead_code)]
    AddEdge(String, String, EdgeKind),
}

impl WaitFreeReadGraph {
    /// 新しいWait-Free読み取りグラフを作成
    pub fn new() -> Self {
        let initial_map = Arc::new(SkipMap::new());
        Self {
            symbols: Arc::new(Atomic::new(initial_map)),
            write_queue: Arc::new(SegQueue::new()),
            version: Arc::new(AtomicU64::new(0)),
            stats: Arc::new(LockFreeStats::new()),
        }
    }

    /// Wait-Free読み取り
    pub fn get_symbol(&self, id: &str) -> Option<Symbol> {
        let guard = epoch::pin();
        let symbols = unsafe { self.symbols.load(Ordering::Acquire, &guard).as_ref() }.unwrap();
        
        self.stats.read_ops.fetch_add(1, Ordering::Relaxed);
        symbols.get(id).map(|entry| entry.value().clone())
    }

    /// 書き込み操作をキューに追加
    pub fn add_symbol(&self, symbol: Symbol) {
        self.write_queue.push(WriteOp::AddSymbol(symbol));
        self.stats.write_ops.fetch_add(1, Ordering::Relaxed);
        
        // 定期的にバッチ処理
        if self.stats.write_ops.load(Ordering::Relaxed) % 100 == 0 {
            self.process_writes();
        }
    }

    /// 書き込みキューを処理
    pub fn process_writes(&self) {
        let guard = epoch::pin();
        let current = unsafe { self.symbols.load(Ordering::Acquire, &guard).as_ref() }.unwrap();
        
        // 新しいマップを作成
        let new_map = SkipMap::new();
        
        // 既存のデータをコピー
        for entry in current.iter() {
            new_map.insert(entry.key().clone(), entry.value().clone());
        }
        
        // 新しい書き込みを適用
        while let Some(op) = self.write_queue.pop() {
            match op {
                WriteOp::AddSymbol(symbol) => {
                    new_map.insert(symbol.id.clone(), symbol);
                    self.stats.symbol_count.fetch_add(1, Ordering::Relaxed);
                }
                WriteOp::AddEdge(_, _, _) => {
                    // エッジ処理は省略
                }
            }
        }
        
        // 新しいマップをアトミックに設定
        let new_arc = Arc::new(new_map);
        self.symbols.store(Owned::new(new_arc), Ordering::Release);
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Symbol数を取得
    pub fn symbol_count(&self) -> usize {
        self.stats.symbol_count.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{SymbolKind, Position, Range};
    use std::thread;
    use std::sync::Arc;

    #[test]
    fn test_lockfree_graph_basic() {
        let graph = LockFreeGraph::new();
        
        let symbol = Symbol {
            id: "test_id".to_string(),
            name: "test_func".to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 1, character: 0 },
            },
            documentation: None,
        };
        
        let id = graph.add_symbol(symbol.clone());
        assert_eq!(id, "test_id");
        
        let retrieved = graph.get_symbol("test_id").unwrap();
        assert_eq!(retrieved.name, "test_func");
        assert_eq!(graph.symbol_count(), 1);
    }

    #[test]
    fn test_concurrent_writes() {
        let graph = Arc::new(LockFreeGraph::new());
        let mut handles = vec![];
        
        // 10スレッドで同時に書き込み
        for i in 0..10 {
            let g = graph.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let symbol = Symbol {
                        id: format!("symbol_{}_{}", i, j),
                        name: format!("name_{}_{}", i, j),
                        kind: SymbolKind::Function,
                        file_path: "test.rs".to_string(),
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 1, character: 0 },
                        },
                        documentation: None,
                    };
                    g.add_symbol(symbol);
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        assert_eq!(graph.symbol_count(), 1000);
    }

    #[test]
    fn test_cas_update() {
        let graph = LockFreeGraph::new();
        
        let symbol = Symbol {
            id: "mutable".to_string(),
            name: "original".to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 1, character: 0 },
            },
            documentation: None,
        };
        
        graph.add_symbol(symbol);
        
        // CAS更新
        let success = graph.update_symbol("mutable", |s| {
            let mut new_symbol = s.clone();
            new_symbol.name = "updated".to_string();
            new_symbol
        });
        
        assert!(success);
        
        let updated = graph.get_symbol("mutable").unwrap();
        assert_eq!(updated.name, "updated");
    }

    #[test]
    fn test_waitfree_read() {
        let graph = WaitFreeReadGraph::new();
        
        // 複数のSymbolを追加
        for i in 0..100 {
            let symbol = Symbol {
                id: format!("symbol_{}", i),
                name: format!("name_{}", i),
                kind: SymbolKind::Function,
                file_path: "test.rs".to_string(),
                range: Range {
                    start: Position { line: i, character: 0 },
                    end: Position { line: i + 1, character: 0 },
                },
                documentation: None,
            };
            graph.add_symbol(symbol);
        }
        
        // バッチ処理を強制実行
        graph.process_writes();
        
        // 読み取り確認
        assert_eq!(graph.symbol_count(), 100);
        let symbol = graph.get_symbol("symbol_50");
        assert!(symbol.is_some());
        assert_eq!(symbol.unwrap().name, "name_50");
    }

    #[test]
    fn test_concurrent_read_write() {
        let graph = Arc::new(LockFreeGraph::new());
        let mut handles = vec![];
        
        // ライタースレッド
        for i in 0..5 {
            let g = graph.clone();
            let handle = thread::spawn(move || {
                for j in 0..200 {
                    let symbol = Symbol {
                        id: format!("w_{}_{}", i, j),
                        name: format!("writer_{}_{}", i, j),
                        kind: SymbolKind::Function,
                        file_path: "test.rs".to_string(),
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 1, character: 0 },
                        },
                        documentation: None,
                    };
                    g.add_symbol(symbol);
                }
            });
            handles.push(handle);
        }
        
        // リーダースレッド
        for _ in 0..10 {
            let g = graph.clone();
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    let count = g.symbol_count();
                    assert!(count <= 1000);
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        let stats = graph.stats();
        println!("Stats: {:?}", stats);
        assert_eq!(graph.symbol_count(), 1000);
        assert!(stats.read_ops > 0);
        assert!(stats.write_ops > 0);
    }
}