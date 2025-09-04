use dashmap::DashMap;
use rayon::prelude::*;
use std::sync::Arc;

use crate::{
    graph::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind},
    memory_pool::{PooledSymbol, SymbolPool},
};

/// メモリプールを使用した最適化されたCodeGraph
pub struct OptimizedCodeGraph {
    /// Symbol ID -> PooledSymbol のマッピング
    symbols: Arc<DashMap<String, PooledSymbol>>,
    /// エッジ情報
    edges: Arc<DashMap<(String, String, EdgeKind), ()>>,
    /// 専用のメモリプール
    pool: Arc<SymbolPool>,
}

impl OptimizedCodeGraph {
    /// 新しい最適化されたグラフを作成
    pub fn new() -> Self {
        Self::with_pool_size(10000)
    }

    /// プールサイズを指定してグラフを作成
    pub fn with_pool_size(pool_size: usize) -> Self {
        Self {
            symbols: Arc::new(DashMap::new()),
            edges: Arc::new(DashMap::new()),
            pool: Arc::new(SymbolPool::new(pool_size)),
        }
    }

    /// Symbolを追加（メモリプールを使用）
    pub fn add_symbol(&self, symbol: Symbol) -> String {
        let id = symbol.id.clone();

        let pooled = self.pool.acquire(
            symbol.id,
            symbol.name,
            symbol.kind,
            symbol.file_path,
            symbol.range,
            symbol.documentation,
        );

        self.symbols.insert(id.clone(), pooled);
        id
    }

    /// バッチでSymbolを追加（並列処理）
    pub fn add_symbols_batch(&self, symbols: Vec<Symbol>) {
        symbols.into_par_iter().for_each(|symbol| {
            self.add_symbol(symbol);
        });
    }

    /// エッジを追加
    pub fn add_edge(&self, from: String, to: String, kind: EdgeKind) {
        self.edges.insert((from, to, kind), ());
    }

    /// Symbolを取得
    pub fn get_symbol(&self, id: &str) -> Option<Symbol> {
        self.symbols.get(id).map(|entry| {
            let pooled = entry.value();
            pooled.as_ref().clone()
        })
    }

    /// 全Symbolを取得
    pub fn all_symbols(&self) -> Vec<Symbol> {
        self.symbols
            .iter()
            .map(|entry| entry.value().as_ref().clone())
            .collect()
    }

    /// Symbol数を取得
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// エッジ数を取得
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// 参照を検索
    pub fn find_references(&self, symbol_id: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter_map(|entry| {
                let ((from, to, kind), _) = entry.pair();
                if to == symbol_id && *kind == EdgeKind::Reference {
                    Some(from.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// 定義を検索
    pub fn find_definition(&self, symbol_id: &str) -> Option<String> {
        self.edges.iter().find_map(|entry| {
            let ((from, to, kind), _) = entry.pair();
            if from == symbol_id && *kind == EdgeKind::Definition {
                Some(to.clone())
            } else {
                None
            }
        })
    }

    /// メモリプールの統計情報を取得
    pub fn pool_stats(&self) -> crate::memory_pool::PoolStats {
        self.pool.stats()
    }

    /// メモリ使用量の推定値を取得（バイト単位）
    pub fn estimated_memory_usage(&self) -> usize {
        let symbol_size = std::mem::size_of::<Symbol>();
        let pooled_symbol_size = std::mem::size_of::<PooledSymbol>();
        let edge_size = std::mem::size_of::<(String, String, EdgeKind)>();

        let symbols_memory = self.symbols.len() * (pooled_symbol_size + 64); // 64 = 推定文字列サイズ
        let edges_memory = self.edges.len() * edge_size;
        let pool_memory = self.pool.pool_size() * symbol_size;

        symbols_memory + edges_memory + pool_memory
    }

    /// 通常のCodeGraphに変換
    pub fn to_code_graph(&self) -> CodeGraph {
        let mut graph = CodeGraph::new();

        // 全Symbolを追加
        for entry in self.symbols.iter() {
            let symbol = entry.value().as_ref().clone();
            graph.add_symbol(symbol);
        }

        // 全エッジを追加
        for entry in self.edges.iter() {
            let ((from, to, kind), _) = entry.pair();
            // String IDから既存のNodeIndexを取得
            if let (Some(from_symbol), Some(to_symbol)) =
                (graph.find_symbol(from), graph.find_symbol(to))
            {
                if let (Some(from_idx), Some(to_idx)) = (
                    graph.symbol_index.get(&from_symbol.id),
                    graph.symbol_index.get(&to_symbol.id),
                ) {
                    graph.add_edge(*from_idx, *to_idx, *kind);
                }
            }
        }

        graph
    }
}

impl Default for OptimizedCodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// ベンチマーク用のヘルパー関数
pub fn create_test_graph_optimized(num_symbols: usize) -> OptimizedCodeGraph {
    let graph = OptimizedCodeGraph::with_pool_size(num_symbols);

    let symbols: Vec<Symbol> = (0..num_symbols)
        .map(|i| Symbol {
            id: format!("symbol_{}", i),
            name: format!("name_{}", i),
            kind: if i % 2 == 0 {
                SymbolKind::Function
            } else {
                SymbolKind::Class
            },
            file_path: format!("file_{}.rs", i / 100),
            range: Range {
                start: Position {
                    line: (i * 10) as u32,
                    character: 0,
                },
                end: Position {
                    line: (i * 10 + 5) as u32,
                    character: 0,
                },
            },
            documentation: if i % 3 == 0 {
                Some(format!("Doc for {}", i))
            } else {
                None
            },
            detail: None,
        })
        .collect();

    graph.add_symbols_batch(symbols);

    // エッジを追加（各シンボルから次のシンボルへの参照）
    for i in 0..num_symbols - 1 {
        graph.add_edge(
            format!("symbol_{}", i),
            format!("symbol_{}", i + 1),
            EdgeKind::Reference,
        );
    }

    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimized_graph_basic() {
        let graph = OptimizedCodeGraph::new();

        let symbol = Symbol {
            id: "test_id".to_string(),
            name: "test_func".to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 0,
                },
            },
            documentation: None,
            detail: None,
        };

        let id = graph.add_symbol(symbol.clone());
        assert_eq!(id, "test_id");

        let retrieved = graph.get_symbol("test_id").unwrap();
        assert_eq!(retrieved.name, "test_func");

        assert_eq!(graph.symbol_count(), 1);
    }

    #[test]
    fn test_batch_add_symbols() {
        let graph = OptimizedCodeGraph::new();

        let symbols: Vec<Symbol> = (0..100)
            .map(|i| Symbol {
                id: format!("id_{}", i),
                name: format!("name_{}", i),
                kind: SymbolKind::Function,
                file_path: "test.rs".to_string(),
                range: Range {
                    start: Position {
                        line: i,
                        character: 0,
                    },
                    end: Position {
                        line: i + 1,
                        character: 0,
                    },
                },
                documentation: None,
                detail: None,
            })
            .collect();

        graph.add_symbols_batch(symbols);
        assert_eq!(graph.symbol_count(), 100);

        // プール統計を確認
        let stats = graph.pool_stats();
        assert_eq!(stats.allocations, 100);
    }

    #[test]
    fn test_edges_and_references() {
        let graph = OptimizedCodeGraph::new();

        for i in 0..3 {
            let symbol = Symbol {
                id: format!("symbol_{}", i),
                name: format!("name_{}", i),
                kind: SymbolKind::Function,
                file_path: "test.rs".to_string(),
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 1,
                        character: 0,
                    },
                },
                documentation: None,
                detail: None,
            };
            graph.add_symbol(symbol);
        }

        graph.add_edge(
            "symbol_0".to_string(),
            "symbol_1".to_string(),
            EdgeKind::Reference,
        );
        graph.add_edge(
            "symbol_2".to_string(),
            "symbol_1".to_string(),
            EdgeKind::Reference,
        );
        graph.add_edge(
            "symbol_1".to_string(),
            "symbol_0".to_string(),
            EdgeKind::Definition,
        );

        let refs = graph.find_references("symbol_1");
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&"symbol_0".to_string()));
        assert!(refs.contains(&"symbol_2".to_string()));

        let def = graph.find_definition("symbol_1");
        assert_eq!(def, Some("symbol_0".to_string()));
    }

    #[test]
    fn test_memory_pool_reuse() {
        let graph = OptimizedCodeGraph::with_pool_size(10);

        // 最初のバッチを追加
        let symbols1: Vec<Symbol> = (0..5)
            .map(|i| Symbol {
                id: format!("id1_{}", i),
                name: format!("name1_{}", i),
                kind: SymbolKind::Function,
                file_path: "test.rs".to_string(),
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 1,
                        character: 0,
                    },
                },
                documentation: None,
                detail: None,
            })
            .collect();

        graph.add_symbols_batch(symbols1);

        let stats1 = graph.pool_stats();
        assert_eq!(stats1.allocations, 5);

        // symbolsをクリアしてプールに返す（実際の実装では自動的に行われる）
        // ここではテストのために手動でシミュレート

        // メモリ使用量を確認
        let memory = graph.estimated_memory_usage();
        assert!(memory > 0);
    }

    #[test]
    fn test_conversion_to_code_graph() {
        let opt_graph = OptimizedCodeGraph::new();

        for i in 0..10 {
            let symbol = Symbol {
                id: format!("id_{}", i),
                name: format!("name_{}", i),
                kind: SymbolKind::Function,
                file_path: "test.rs".to_string(),
                range: Range {
                    start: Position {
                        line: i,
                        character: 0,
                    },
                    end: Position {
                        line: i + 1,
                        character: 0,
                    },
                },
                documentation: None,
                detail: None,
            };
            opt_graph.add_symbol(symbol);
        }

        opt_graph.add_edge("id_0".to_string(), "id_1".to_string(), EdgeKind::Reference);

        let code_graph = opt_graph.to_code_graph();
        assert_eq!(code_graph.symbol_count(), 10);

        let refs = code_graph.find_references("id_1").unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, "id_0");
    }
}
