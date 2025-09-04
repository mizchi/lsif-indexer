use lsif_core::{CodeGraph, Symbol};
use tracing::info;

/// バッチグラフ更新のためのヘルパー構造体
pub struct BatchGraphUpdater {
    symbols_to_add: Vec<Symbol>,
    symbols_to_remove: Vec<String>, // symbol IDs
    files_to_clear: Vec<String>,    // file paths
}

impl Default for BatchGraphUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl BatchGraphUpdater {
    pub fn new() -> Self {
        Self {
            symbols_to_add: Vec::new(),
            symbols_to_remove: Vec::new(),
            files_to_clear: Vec::new(),
        }
    }

    /// 追加するシンボルを蓄積
    pub fn queue_symbol_addition(&mut self, symbol: Symbol) {
        self.symbols_to_add.push(symbol);
    }

    /// 削除するシンボルを蓄積
    pub fn queue_symbol_removal(&mut self, symbol_id: String) {
        self.symbols_to_remove.push(symbol_id);
    }

    /// ファイル内の全シンボルを削除するようマーク
    pub fn queue_file_clear(&mut self, file_path: String) {
        self.files_to_clear.push(file_path);
    }

    /// 蓄積した変更をグラフに一括適用
    pub fn apply_to_graph(&self, graph: &mut CodeGraph) {
        let start = std::time::Instant::now();

        // まず削除を実行
        if !self.files_to_clear.is_empty() {
            info!("Clearing symbols from {} files", self.files_to_clear.len());
            for file_path in &self.files_to_clear {
                let symbols_to_remove: Vec<_> = graph
                    .get_all_symbols()
                    .filter(|s| s.file_path == *file_path)
                    .map(|s| s.id.clone())
                    .collect();

                for symbol_id in symbols_to_remove {
                    graph.remove_symbol(&symbol_id);
                }
            }
        }

        if !self.symbols_to_remove.is_empty() {
            info!(
                "Removing {} individual symbols",
                self.symbols_to_remove.len()
            );
            for symbol_id in &self.symbols_to_remove {
                graph.remove_symbol(symbol_id);
            }
        }

        // 次に追加を実行（バッチで）
        if !self.symbols_to_add.is_empty() {
            info!("Adding {} symbols in batch", self.symbols_to_add.len());

            // バッチサイズを調整（メモリ使用量とパフォーマンスのバランス）
            const BATCH_SIZE: usize = 100;

            for chunk in self.symbols_to_add.chunks(BATCH_SIZE) {
                for symbol in chunk {
                    graph.add_symbol(symbol.clone());
                }
            }
        }

        let elapsed = start.elapsed();
        info!(
            "Batch graph update completed in {:.3}s",
            elapsed.as_secs_f64()
        );
    }

    /// 統計情報を取得
    pub fn stats(&self) -> BatchUpdateStats {
        BatchUpdateStats {
            symbols_to_add: self.symbols_to_add.len(),
            symbols_to_remove: self.symbols_to_remove.len(),
            files_to_clear: self.files_to_clear.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchUpdateStats {
    pub symbols_to_add: usize,
    pub symbols_to_remove: usize,
    pub files_to_clear: usize,
}

impl BatchUpdateStats {
    pub fn is_empty(&self) -> bool {
        self.symbols_to_add == 0 && self.symbols_to_remove == 0 && self.files_to_clear == 0
    }
}
