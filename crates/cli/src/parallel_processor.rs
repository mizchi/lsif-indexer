use anyhow::Result;
use lsif_core::Symbol;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// 並列処理用のバッチプロセッサ
pub struct ParallelProcessor {
    thread_count: usize,
    batch_size: usize,
}

impl Default for ParallelProcessor {
    fn default() -> Self {
        Self {
            thread_count: num_cpus::get().min(8), // 最大8スレッド
            batch_size: 10,
        }
    }
}

impl ParallelProcessor {
    pub fn new(thread_count: usize, batch_size: usize) -> Self {
        Self {
            thread_count: thread_count.max(1).min(16),
            batch_size: batch_size.max(1),
        }
    }

    /// ファイルのバッチ処理を並列実行
    pub fn process_files_parallel<F, T>(
        &self,
        files: Vec<T>,
        processor: F,
    ) -> Result<Vec<Vec<Symbol>>>
    where
        F: Fn(T) -> Result<Vec<Symbol>> + Send + Sync + 'static,
        T: Send + Clone + 'static,
    {
        if files.is_empty() {
            return Ok(Vec::new());
        }

        let total_files = files.len();
        let chunk_size = total_files.div_ceil(self.thread_count);

        // プログレストラッカー
        let progress = Arc::new(Mutex::new(ProgressTracker::new(total_files)));
        let processor = Arc::new(processor);

        // ファイルをチャンクに分割
        let chunks: Vec<Vec<T>> = files
            .into_iter()
            .collect::<Vec<_>>()
            .chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        // 各チャンクを並列処理
        let handles: Vec<_> = chunks
            .into_iter()
            .enumerate()
            .map(|(thread_id, chunk)| {
                let processor = Arc::clone(&processor);
                let progress = Arc::clone(&progress);

                thread::spawn(move || {
                    let mut results = Vec::new();

                    for item in chunk {
                        match processor(item) {
                            Ok(symbols) => {
                                results.push(symbols);

                                // プログレス更新
                                let mut tracker = progress.lock().unwrap();
                                tracker.increment();
                                if tracker.should_print() {
                                    eprintln!(
                                        "  🔄 Thread {}: Processed {}/{} files ({:.0}%)",
                                        thread_id + 1,
                                        tracker.processed,
                                        tracker.total,
                                        tracker.percentage()
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "  ⚠️  Thread {}: Error processing file: {}",
                                    thread_id + 1,
                                    e
                                );
                                results.push(Vec::new());
                            }
                        }
                    }

                    results
                })
            })
            .collect();

        // 結果を収集
        let mut all_results = Vec::new();
        for handle in handles {
            match handle.join() {
                Ok(results) => all_results.extend(results),
                Err(_) => {
                    eprintln!("Thread panicked during processing");
                }
            }
        }

        // 最終プログレス表示
        let tracker = progress.lock().unwrap();
        if tracker.total > 10 {
            eprintln!(
                "  ✅ Completed: {}/{} files (100%)",
                tracker.processed, tracker.total
            );
        }

        Ok(all_results)
    }

    /// バッチサイズを動的に調整
    pub fn adjust_batch_size(&mut self, file_count: usize) {
        if file_count < 50 {
            self.batch_size = 5;
        } else if file_count < 200 {
            self.batch_size = 10;
        } else {
            self.batch_size = 20;
        }
    }
}

/// プログレストラッカー
struct ProgressTracker {
    total: usize,
    processed: usize,
    last_print: Instant,
    print_interval: Duration,
}

impl ProgressTracker {
    fn new(total: usize) -> Self {
        Self {
            total,
            processed: 0,
            last_print: Instant::now(),
            print_interval: Duration::from_secs(2), // 2秒ごとに表示
        }
    }

    fn increment(&mut self) {
        self.processed += 1;
    }

    fn should_print(&mut self) -> bool {
        if self.last_print.elapsed() > self.print_interval {
            self.last_print = Instant::now();
            true
        } else {
            false
        }
    }

    fn percentage(&self) -> f64 {
        (self.processed as f64 / self.total as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_processor() {
        let processor = ParallelProcessor::new(4, 5);

        let files: Vec<usize> = (0..20).collect();

        let results = processor.process_files_parallel(files, |n| {
            // シンプルな処理をシミュレート
            thread::sleep(Duration::from_millis(10));
            Ok(vec![Symbol {
                id: format!("test_{}", n),
                name: format!("symbol_{}", n),
                kind: lsif_core::SymbolKind::Function,
                file_path: format!("file_{}.rs", n),
                range: lsif_core::Range {
                    start: lsif_core::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsif_core::Position {
                        line: 0,
                        character: 0,
                    },
                },
                documentation: None,
                detail: None,
            }])
        });

        assert!(results.is_ok());
        let results = results.unwrap();
        assert_eq!(results.len(), 20);
    }

    #[test]
    fn test_batch_size_adjustment() {
        let mut processor = ParallelProcessor::default();

        processor.adjust_batch_size(30);
        assert_eq!(processor.batch_size, 5);

        processor.adjust_batch_size(100);
        assert_eq!(processor.batch_size, 10);

        processor.adjust_batch_size(500);
        assert_eq!(processor.batch_size, 20);
    }
}
