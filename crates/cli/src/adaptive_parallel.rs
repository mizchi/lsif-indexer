use anyhow::Result;
use rayon::prelude::*;
use std::path::PathBuf;

/// 適応的並列処理の設定
#[derive(Debug, Clone)]
pub struct AdaptiveParallelConfig {
    /// 並列処理を開始する閾値（ファイル数）
    pub parallel_threshold: usize,
    /// 最大スレッド数（0 = 自動）
    pub max_threads: usize,
    /// チャンクサイズ（バッチ処理時）
    pub chunk_size: usize,
}

impl Default for AdaptiveParallelConfig {
    fn default() -> Self {
        Self {
            parallel_threshold: 30, // 30ファイル以上で並列処理
            max_threads: 0,         // 自動（CPU数に基づく）
            chunk_size: 10,         // より小さなチャンクで並列度を向上
        }
    }
}

/// 適応的並列処理エグゼキュータ
pub struct AdaptiveParallelExecutor {
    config: AdaptiveParallelConfig,
    thread_pool: Option<rayon::ThreadPool>,
}

impl AdaptiveParallelExecutor {
    /// 新しいエグゼキュータを作成
    pub fn new(config: AdaptiveParallelConfig) -> Result<Self> {
        let thread_pool = if config.max_threads > 0 {
            Some(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(config.max_threads)
                    .build()?,
            )
        } else {
            None
        };

        Ok(Self {
            config,
            thread_pool,
        })
    }

    /// デフォルト設定でエグゼキュータを作成
    pub fn with_defaults() -> Result<Self> {
        Self::new(AdaptiveParallelConfig::default())
    }

    /// ファイル処理を適応的に実行
    pub fn process_files<F, R>(&self, files: Vec<PathBuf>, processor: F) -> Vec<R>
    where
        F: Fn(&PathBuf) -> R + Sync + Send,
        R: Send,
    {
        if files.len() < self.config.parallel_threshold {
            // シーケンシャル処理（小規模データセット）
            files.iter().map(processor).collect()
        } else if let Some(pool) = &self.thread_pool {
            // カスタムスレッドプールで並列処理
            pool.install(|| files.par_iter().map(processor).collect())
        } else {
            // デフォルトの並列処理
            files.par_iter().map(processor).collect()
        }
    }

    /// チャンクベースの処理（大規模データセット用）
    pub fn process_chunked<T, F, R>(&self, items: Vec<T>, processor: F) -> Vec<R>
    where
        T: Send + Sync,
        F: Fn(&[T]) -> Vec<R> + Sync + Send,
        R: Send,
    {
        if items.len() < self.config.parallel_threshold {
            // シーケンシャル処理
            processor(&items)
        } else {
            // チャンク分割して並列処理
            items
                .par_chunks(self.config.chunk_size)
                .flat_map(processor)
                .collect()
        }
    }

    /// 条件付き並列マップ
    pub fn map_conditional<T, F, R>(&self, items: Vec<T>, mapper: F) -> Vec<R>
    where
        T: Send + Sync,
        F: Fn(T) -> R + Sync + Send,
        R: Send,
    {
        let len = items.len();

        if len < self.config.parallel_threshold {
            // シーケンシャル処理
            items.into_iter().map(mapper).collect()
        } else {
            // 並列処理
            items.into_par_iter().map(mapper).collect()
        }
    }

    /// 適応的フィルタと変換
    pub fn filter_map<T, F, R>(&self, items: Vec<T>, filter_mapper: F) -> Vec<R>
    where
        T: Send + Sync,
        F: Fn(T) -> Option<R> + Sync + Send,
        R: Send,
    {
        if items.len() < self.config.parallel_threshold {
            items.into_iter().filter_map(filter_mapper).collect()
        } else {
            items.into_par_iter().filter_map(filter_mapper).collect()
        }
    }

    /// 統計情報を取得
    pub fn get_stats(&self) -> ParallelExecutionStats {
        ParallelExecutionStats {
            parallel_threshold: self.config.parallel_threshold,
            max_threads: self.config.max_threads,
            chunk_size: self.config.chunk_size,
            thread_pool_size: self
                .thread_pool
                .as_ref()
                .map(|p| p.current_num_threads())
                .unwrap_or_else(rayon::current_num_threads),
        }
    }
}

/// 実行統計情報
#[derive(Debug)]
pub struct ParallelExecutionStats {
    pub parallel_threshold: usize,
    pub max_threads: usize,
    pub chunk_size: usize,
    pub thread_pool_size: usize,
}

/// インクリメンタル更新の適応的処理
pub struct AdaptiveIncrementalProcessor {
    executor: AdaptiveParallelExecutor,
    pub config: AdaptiveParallelConfig,
}

impl AdaptiveIncrementalProcessor {
    pub fn new(config: AdaptiveParallelConfig) -> Result<Self> {
        let config_clone = config.clone();
        Ok(Self {
            executor: AdaptiveParallelExecutor::new(config)?,
            config: config_clone,
        })
    }

    /// 変更ファイルを処理
    pub fn process_changes<F>(
        &self,
        added: Vec<PathBuf>,
        modified: Vec<PathBuf>,
        deleted: Vec<PathBuf>,
        processor: F,
    ) -> Result<()>
    where
        F: Fn(&PathBuf, ChangeType) -> Result<()> + Sync + Send,
    {
        // 追加と変更は並列処理の恩恵を受けやすい
        let total_changes = added.len() + modified.len();

        if total_changes >= self.executor.config.parallel_threshold {
            // 並列処理
            rayon::scope(|s| {
                s.spawn(|_| {
                    added.par_iter().for_each(|path| {
                        let _ = processor(path, ChangeType::Added);
                    });
                });

                s.spawn(|_| {
                    modified.par_iter().for_each(|path| {
                        let _ = processor(path, ChangeType::Modified);
                    });
                });
            });
        } else {
            // シーケンシャル処理
            for path in &added {
                processor(path, ChangeType::Added)?;
            }
            for path in &modified {
                processor(path, ChangeType::Modified)?;
            }
        }

        // 削除は常にシーケンシャル（依存関係の問題を避けるため）
        for path in &deleted {
            processor(path, ChangeType::Deleted)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_adaptive_threshold() {
        let config = AdaptiveParallelConfig {
            parallel_threshold: 10,
            max_threads: 2,
            chunk_size: 5,
        };

        let executor = AdaptiveParallelExecutor::new(config).unwrap();

        // 閾値未満：シーケンシャル処理
        let small_items = vec![1, 2, 3, 4, 5];
        let small_result = executor.map_conditional(small_items, |x| x * 2);
        assert_eq!(small_result, vec![2, 4, 6, 8, 10]);

        // 閾値以上：並列処理
        let large_items: Vec<i32> = (1..=20).collect();
        let large_result = executor.map_conditional(large_items, |x| x * 2);
        let expected: Vec<i32> = (2..=40).step_by(2).collect();
        assert_eq!(large_result, expected);
    }

    #[test]
    fn test_chunked_processing() {
        let config = AdaptiveParallelConfig {
            parallel_threshold: 5,
            max_threads: 2,
            chunk_size: 3,
        };

        let executor = AdaptiveParallelExecutor::new(config).unwrap();

        let items: Vec<i32> = (1..=10).collect();
        let result = executor.process_chunked(items, |chunk| chunk.iter().map(|x| x * 2).collect());

        let expected: Vec<i32> = (2..=20).step_by(2).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_incremental_processor() {
        let config = AdaptiveParallelConfig::default();
        let processor = AdaptiveIncrementalProcessor::new(config).unwrap();

        let added = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
        let modified = vec![PathBuf::from("c.rs")];
        let deleted = vec![PathBuf::from("d.rs")];

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        processor
            .process_changes(added, modified, deleted, move |_path, _change_type| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn test_process_files() {
        let config = AdaptiveParallelConfig {
            parallel_threshold: 3,
            max_threads: 2,
            chunk_size: 5,
        };

        let executor = AdaptiveParallelExecutor::new(config).unwrap();

        // 小さいファイルセット（シーケンシャル）
        let small_files = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
        let small_result = executor.process_files(small_files, |path| {
            path.file_name().unwrap().to_str().unwrap().to_string()
        });
        assert_eq!(small_result, vec!["a.rs", "b.rs"]);

        // 大きいファイルセット（並列）
        let large_files: Vec<PathBuf> = (0..10)
            .map(|i| PathBuf::from(format!("file{}.rs", i)))
            .collect();
        let large_result = executor.process_files(large_files.clone(), |path| {
            path.file_name().unwrap().to_str().unwrap().to_string()
        });
        assert_eq!(large_result.len(), 10);
    }

    #[test]
    fn test_filter_map() {
        let config = AdaptiveParallelConfig {
            parallel_threshold: 3,
            max_threads: 2,
            chunk_size: 5,
        };

        let executor = AdaptiveParallelExecutor::new(config).unwrap();

        // 小さいデータセット
        let small_items = vec![1, 2, 3];
        let small_result =
            executor.filter_map(small_items, |x| if x % 2 == 0 { Some(x * 2) } else { None });
        assert_eq!(small_result, vec![4]);

        // 大きいデータセット
        let large_items: Vec<i32> = (1..=10).collect();
        let large_result =
            executor.filter_map(large_items, |x| if x % 2 == 0 { Some(x * 2) } else { None });
        assert_eq!(large_result, vec![4, 8, 12, 16, 20]);
    }

    #[test]
    fn test_get_stats() {
        let config = AdaptiveParallelConfig {
            parallel_threshold: 100,
            max_threads: 4,
            chunk_size: 25,
        };

        let executor = AdaptiveParallelExecutor::new(config).unwrap();
        let stats = executor.get_stats();

        assert_eq!(stats.parallel_threshold, 100);
        assert_eq!(stats.max_threads, 4);
        assert_eq!(stats.chunk_size, 25);
        assert_eq!(stats.thread_pool_size, 4);
    }

    #[test]
    fn test_with_defaults() {
        let executor = AdaptiveParallelExecutor::with_defaults().unwrap();
        let stats = executor.get_stats();

        assert_eq!(stats.parallel_threshold, 30);
        assert_eq!(stats.max_threads, 0);
        assert_eq!(stats.chunk_size, 10);
    }

    #[test]
    fn test_incremental_processor_parallel() {
        let config = AdaptiveParallelConfig {
            parallel_threshold: 2,
            max_threads: 2,
            chunk_size: 5,
        };
        let processor = AdaptiveIncrementalProcessor::new(config).unwrap();

        // 並列処理される十分な数のファイル
        let added: Vec<PathBuf> = (0..10)
            .map(|i| PathBuf::from(format!("a{}.rs", i)))
            .collect();
        let modified: Vec<PathBuf> = (0..10)
            .map(|i| PathBuf::from(format!("m{}.rs", i)))
            .collect();
        let deleted = vec![PathBuf::from("d.rs")];

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        processor
            .process_changes(added, modified, deleted, move |_path, _change_type| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 21);
    }
}
