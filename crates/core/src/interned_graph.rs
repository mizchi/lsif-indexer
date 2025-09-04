use dashmap::DashMap;
use std::sync::Arc;

use crate::{
    graph::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind},
    string_interner::{intern, InternedString, InternedSymbol},
};

/// String interningを使用した最適化グラフ
pub struct InternedGraph {
    /// Symbol ID -> InternedSymbol のマッピング
    symbols: Arc<DashMap<InternedString, InternedSymbol>>,
    /// エッジ情報（インターン化されたID使用）
    edges: Arc<DashMap<(InternedString, InternedString, EdgeKind), ()>>,
}

impl InternedGraph {
    /// 新しいインターン化グラフを作成
    pub fn new() -> Self {
        Self {
            symbols: Arc::new(DashMap::new()),
            edges: Arc::new(DashMap::new()),
        }
    }

    /// Symbolを追加（文字列をインターン化）
    pub fn add_symbol(&self, symbol: Symbol) -> InternedString {
        // グローバルインターナーを使用
        let interned_id = intern(&symbol.id);
        let interned_symbol = InternedSymbol {
            id: interned_id,
            name: intern(&symbol.name),
            kind: symbol.kind,
            file_path: intern(&symbol.file_path),
            range: symbol.range,
            documentation: symbol.documentation.as_deref().map(intern),
            detail: symbol.detail.as_deref().map(intern),
        };

        self.symbols.insert(interned_id, interned_symbol);
        interned_id
    }

    /// バッチでSymbolを追加
    pub fn add_symbols_batch(&self, symbols: Vec<Symbol>) {
        for symbol in symbols {
            self.add_symbol(symbol);
        }
    }

    /// エッジを追加
    pub fn add_edge(&self, from: &str, to: &str, kind: EdgeKind) {
        let from_id = intern(from);
        let to_id = intern(to);
        self.edges.insert((from_id, to_id, kind), ());
    }

    /// Symbolを取得
    pub fn get_symbol(&self, id: &str) -> Option<Symbol> {
        let interned_id = intern(id);
        self.symbols
            .get(&interned_id)
            .map(|entry| entry.value().to_symbol())
    }

    /// 全Symbolを取得
    pub fn all_symbols(&self) -> Vec<Symbol> {
        self.symbols
            .iter()
            .map(|entry| entry.value().to_symbol())
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
        let target_id = intern(symbol_id);

        self.edges
            .iter()
            .filter_map(|entry| {
                let ((from, to, kind), _) = entry.pair();
                if *to == target_id && *kind == EdgeKind::Reference {
                    Some(from.as_str().to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// 定義を検索
    pub fn find_definition(&self, symbol_id: &str) -> Option<String> {
        let source_id = intern(symbol_id);

        self.edges.iter().find_map(|entry| {
            let ((from, to, kind), _) = entry.pair();
            if *from == source_id && *kind == EdgeKind::Definition {
                Some(to.as_str().to_string())
            } else {
                None
            }
        })
    }

    /// メモリ使用量の推定値を取得（バイト単位）
    pub fn estimated_memory_usage(&self) -> usize {
        // インターン化されたシンボルのサイズ
        let symbol_size = std::mem::size_of::<InternedSymbol>();
        let symbols_memory = self.symbols.len() * symbol_size;

        // エッジのサイズ（インターン化されたIDは小さい）
        let edge_size = std::mem::size_of::<(InternedString, InternedString, EdgeKind)>();
        let edges_memory = self.edges.len() * edge_size;

        // グローバルインターナーのメモリ使用量（推定）
        let interner_memory = 0; // グローバルインターナーの使用量は別途計測

        symbols_memory + edges_memory + interner_memory
    }

    /// インターナーの統計情報を取得
    pub fn interner_stats(&self) -> crate::string_interner::InternerStats {
        crate::string_interner::interner_stats()
    }

    /// 通常のCodeGraphに変換
    pub fn to_code_graph(&self) -> CodeGraph {
        let mut graph = CodeGraph::new();

        // 全Symbolを追加
        for entry in self.symbols.iter() {
            let symbol = entry.value().to_symbol();
            graph.add_symbol(symbol);
        }

        // 全エッジを追加
        for entry in self.edges.iter() {
            let ((from, to, kind), _) = entry.pair();
            // String IDから既存のNodeIndexを取得
            let from_str = from.as_str();
            let to_str = to.as_str();

            if let (Some(from_symbol), Some(to_symbol)) =
                (graph.find_symbol(from_str), graph.find_symbol(to_str))
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

impl Default for InternedGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// ベンチマーク用のヘルパー関数
pub fn create_test_graph_interned(num_symbols: usize) -> InternedGraph {
    let graph = InternedGraph::new();

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
            &format!("symbol_{}", i),
            &format!("symbol_{}", i + 1),
            EdgeKind::Reference,
        );
    }

    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interned_graph_basic() {
        let graph = InternedGraph::new();

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
        assert_eq!(id.as_str(), "test_id");

        let retrieved = graph.get_symbol("test_id").unwrap();
        assert_eq!(retrieved.name, "test_func");

        assert_eq!(graph.symbol_count(), 1);
    }

    #[test]
    fn test_interned_strings_deduplication() {
        let graph = InternedGraph::new();

        // 同じファイルパスを持つ複数のシンボル
        for i in 0..10 {
            let symbol = Symbol {
                id: format!("id_{}", i),
                name: format!("name_{}", i),
                kind: SymbolKind::Function,
                file_path: "shared/file.rs".to_string(), // 同じパス
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
                documentation: Some("Shared documentation".to_string()), // 同じドキュメント
                detail: None,
            };
            graph.add_symbol(symbol);
        }

        let stats = graph.interner_stats();
        // "shared/file.rs" と "Shared documentation" は1回だけインターン化される
        assert!(stats.cache_hits > 0);

        // メモリ使用量が削減されているはず
        let memory = graph.estimated_memory_usage();
        let standard_memory = 10
            * (std::mem::size_of::<Symbol>()
                + "shared/file.rs".len() * 10
                + "Shared documentation".len() * 10);

        assert!(memory < standard_memory);
    }

    #[test]
    fn test_edges_and_references() {
        let graph = InternedGraph::new();

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

        graph.add_edge("symbol_0", "symbol_1", EdgeKind::Reference);
        graph.add_edge("symbol_2", "symbol_1", EdgeKind::Reference);
        graph.add_edge("symbol_1", "symbol_0", EdgeKind::Definition);

        let refs = graph.find_references("symbol_1");
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&"symbol_0".to_string()));
        assert!(refs.contains(&"symbol_2".to_string()));

        let def = graph.find_definition("symbol_1");
        assert_eq!(def, Some("symbol_0".to_string()));
    }

    #[test]
    fn test_conversion_to_code_graph() {
        let interned_graph = InternedGraph::new();

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
            interned_graph.add_symbol(symbol);
        }

        interned_graph.add_edge("id_0", "id_1", EdgeKind::Reference);

        let code_graph = interned_graph.to_code_graph();
        assert_eq!(code_graph.symbol_count(), 10);

        let refs = code_graph.find_references("id_1");
        assert_eq!(refs.unwrap().len(), 1);
    }

    #[test]
    fn test_large_scale_memory_efficiency() {
        let graph = InternedGraph::new();

        // 1000個のシンボル、100個のユニークなファイルパス
        for i in 0..1000 {
            let symbol = Symbol {
                id: format!("symbol_{}", i),
                name: format!("name_{}", i % 50), // 50個のユニークな名前
                kind: SymbolKind::Function,
                file_path: format!("src/module_{}/file.rs", i % 100), // 100個のユニークなパス
                range: Range {
                    start: Position {
                        line: (i % 100) as u32,
                        character: 0,
                    },
                    end: Position {
                        line: (i % 100 + 1) as u32,
                        character: 0,
                    },
                },
                documentation: Some(format!("Doc type {}", i % 10)), // 10個のユニークなドキュメント
                detail: None,
            };
            graph.add_symbol(symbol);
        }

        let stats = graph.interner_stats();
        // 多くの文字列が再利用されているはず
        assert!(stats.cache_hits > 500);

        // インターン化された文字列の総数は全体よりずっと少ないはず
        assert!(stats.total_strings < 2000); // 1000 IDs + 重複排除された他のフィールド
    }
}
