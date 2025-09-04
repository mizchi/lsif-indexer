use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;

use crate::{Position, Range, Symbol, SymbolKind};

/// メモリプールで管理されるSymbol
#[derive(Clone)]
pub struct PooledSymbol {
    inner: Arc<Symbol>,
}

impl PooledSymbol {
    pub fn get_symbol(&self) -> &Symbol {
        &self.inner
    }

    pub fn into_inner(self) -> Arc<Symbol> {
        self.inner
    }
}

impl AsRef<Symbol> for PooledSymbol {
    fn as_ref(&self) -> &Symbol {
        &self.inner
    }
}

/// Symbol構造体用のメモリプール
pub struct SymbolPool {
    /// 再利用可能なSymbolのプール
    pool: Arc<RwLock<VecDeque<Arc<Symbol>>>>,
    /// プールの最大サイズ
    max_size: usize,
    /// 統計情報
    stats: Arc<RwLock<PoolStats>>,
}

#[derive(Default, Debug, Clone)]
pub struct PoolStats {
    pub allocations: usize,
    pub reuses: usize,
    pub returns: usize,
    pub pool_hits: usize,
    pub pool_misses: usize,
}

impl SymbolPool {
    /// 新しいSymbolプールを作成
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            max_size,
            stats: Arc::new(RwLock::new(PoolStats::default())),
        }
    }

    /// プールからSymbolを取得または新規作成
    pub fn acquire(
        &self,
        id: String,
        name: String,
        kind: SymbolKind,
        file_path: String,
        range: Range,
        documentation: Option<String>,
    ) -> PooledSymbol {
        let mut pool = self.pool.write();
        let mut stats = self.stats.write();

        // プールから再利用可能なSymbolを探す
        if let Some(mut symbol_arc) = pool.pop_front() {
            // Arc::get_mutで既存のSymbolを変更
            if let Some(symbol) = Arc::get_mut(&mut symbol_arc) {
                symbol.id = id;
                symbol.name = name;
                symbol.kind = kind;
                symbol.file_path = file_path;
                symbol.range = range;
                symbol.documentation = documentation;

                stats.reuses += 1;
                stats.pool_hits += 1;

                return PooledSymbol { inner: symbol_arc };
            }
        }

        // プールが空の場合、新規作成
        stats.allocations += 1;
        stats.pool_misses += 1;

        PooledSymbol {
            inner: Arc::new(Symbol {
                id,
                name,
                kind,
                file_path,
                range,
                documentation,
                detail: None,
            }),
        }
    }

    /// Symbolをプールに返却
    pub fn release(&self, symbol: PooledSymbol) {
        let mut pool = self.pool.write();
        let mut stats = self.stats.write();

        // プールが最大サイズに達していない場合のみ返却
        if pool.len() < self.max_size {
            pool.push_back(symbol.inner);
            stats.returns += 1;
        }
    }

    /// バッチでSymbolを作成
    pub fn acquire_batch(&self, count: usize) -> Vec<PooledSymbol> {
        let mut symbols = Vec::with_capacity(count);

        for i in 0..count {
            symbols.push(self.acquire(
                format!("temp_{}", i),
                String::new(),
                SymbolKind::Function,
                String::new(),
                Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
                None,
            ));
        }

        symbols
    }

    /// プールの統計情報を取得
    pub fn stats(&self) -> PoolStats {
        self.stats.read().clone()
    }

    /// プールをクリア
    pub fn clear(&self) {
        self.pool.write().clear();
    }

    /// プールの現在のサイズを取得
    pub fn pool_size(&self) -> usize {
        self.pool.read().len()
    }
}

/// グローバルなSymbolプール
static GLOBAL_POOL: once_cell::sync::Lazy<SymbolPool> =
    once_cell::sync::Lazy::new(|| SymbolPool::new(10000));

/// グローバルプールからSymbolを取得
pub fn acquire_symbol(
    id: String,
    name: String,
    kind: SymbolKind,
    file_path: String,
    range: Range,
    documentation: Option<String>,
) -> PooledSymbol {
    GLOBAL_POOL.acquire(id, name, kind, file_path, range, documentation)
}

/// グローバルプールにSymbolを返却
pub fn release_symbol(symbol: PooledSymbol) {
    GLOBAL_POOL.release(symbol)
}

/// グローバルプールの統計情報を取得
pub fn pool_stats() -> PoolStats {
    GLOBAL_POOL.stats()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_pool_basic() {
        let pool = SymbolPool::new(10);

        // 新規作成
        let symbol1 = pool.acquire(
            "id1".to_string(),
            "name1".to_string(),
            SymbolKind::Function,
            "file1.rs".to_string(),
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 0,
                },
            },
            None,
        );

        assert_eq!(symbol1.as_ref().id, "id1");
        assert_eq!(pool.stats().allocations, 1);

        // プールに返却
        pool.release(symbol1);
        assert_eq!(pool.pool_size(), 1);
        assert_eq!(pool.stats().returns, 1);

        // 再利用
        let symbol2 = pool.acquire(
            "id2".to_string(),
            "name2".to_string(),
            SymbolKind::Class,
            "file2.rs".to_string(),
            Range {
                start: Position {
                    line: 2,
                    character: 0,
                },
                end: Position {
                    line: 3,
                    character: 0,
                },
            },
            Some("doc".to_string()),
        );

        assert_eq!(symbol2.as_ref().id, "id2");
        assert_eq!(symbol2.as_ref().name, "name2");
        assert_eq!(pool.stats().reuses, 1);
        assert_eq!(pool.stats().pool_hits, 1);
    }

    #[test]
    fn test_pool_max_size() {
        let pool = SymbolPool::new(2);

        let symbols: Vec<_> = (0..3)
            .map(|i| {
                pool.acquire(
                    format!("id{}", i),
                    format!("name{}", i),
                    SymbolKind::Function,
                    "file.rs".to_string(),
                    Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 1,
                            character: 0,
                        },
                    },
                    None,
                )
            })
            .collect();

        // 3つ全て返却するが、プールには2つまでしか保持されない
        for symbol in symbols {
            pool.release(symbol);
        }

        assert_eq!(pool.pool_size(), 2);
        assert_eq!(pool.stats().returns, 2);
    }

    #[test]
    fn test_batch_acquire() {
        let pool = SymbolPool::new(100);

        let batch = pool.acquire_batch(50);
        assert_eq!(batch.len(), 50);
        assert_eq!(pool.stats().allocations, 50);

        // バッチを返却
        for symbol in batch {
            pool.release(symbol);
        }

        assert_eq!(pool.pool_size(), 50);

        // 次のバッチは再利用される
        let batch2 = pool.acquire_batch(30);
        assert_eq!(batch2.len(), 30);
        assert_eq!(pool.stats().reuses, 30);
    }

    #[test]
    fn test_global_pool() {
        let symbol = acquire_symbol(
            "global_id".to_string(),
            "global_name".to_string(),
            SymbolKind::Variable,
            "global.rs".to_string(),
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 0,
                },
            },
            None,
        );

        assert_eq!(symbol.as_ref().id, "global_id");

        release_symbol(symbol);

        let stats = pool_stats();
        assert!(stats.allocations > 0 || stats.reuses > 0);
    }
}
