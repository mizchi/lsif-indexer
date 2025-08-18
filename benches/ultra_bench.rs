use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use lsif_indexer::cli::parallel_storage::ParallelIndexStorage;
use lsif_indexer::cli::ultra_fast_storage::{MemoryPoolStorage, UltraFastStorage};
use lsif_indexer::core::graph::{Position, Range, Symbol, SymbolKind};
use rand::prelude::*;
use std::sync::Arc;
use tempfile::TempDir;

fn generate_test_symbols(count: usize) -> Vec<Symbol> {
    (0..count)
        .map(|i| Symbol {
            id: format!("symbol_{i}"),
            kind: match i % 5 {
                0 => SymbolKind::Function,
                1 => SymbolKind::Class,
                2 => SymbolKind::Method,
                3 => SymbolKind::Variable,
                _ => SymbolKind::Constant,
            },
            name: format!("test_symbol_{i}"),
            file_path: format!("src/test/file_{}.rs", i % 10),
            range: Range {
                start: Position {
                    line: (i % 1000) as u32,
                    character: 0,
                },
                end: Position {
                    line: (i % 1000) as u32 + 1,
                    character: 80,
                },
            },
            documentation: if i % 3 == 0 {
                Some(format!("Documentation for symbol {i}"))
            } else {
                None
            },
        })
        .collect()
}

fn benchmark_extreme_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("extreme_scale");
    group.sample_size(10); // サンプルサイズを減らして高速化

    // 大規模データでのベンチマーク
    for size in [1000, 10000, 50000].iter() {
        // 100000を削除してテスト時間短縮
        group.throughput(Throughput::Elements(*size as u64));

        // 既存の並列ストレージ
        group.bench_with_input(
            BenchmarkId::new("parallel_save_optimized", size),
            size,
            |b, &symbol_count| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = ParallelIndexStorage::open(temp_dir.path()).unwrap();
                        let symbols: Vec<(String, Symbol)> = generate_test_symbols(symbol_count)
                            .into_iter()
                            .map(|s| (s.id.clone(), s))
                            .collect();
                        (storage, symbols, temp_dir)
                    },
                    |(storage, symbols, _temp_dir)| {
                        let optimal_chunk_size = calculate_optimal_chunk_size(symbols.len());
                        storage
                            .save_symbols_chunked(&symbols, optimal_chunk_size)
                            .unwrap();
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        // 新しい超高速ストレージ
        group.bench_with_input(
            BenchmarkId::new("ultra_fast_save", size),
            size,
            |b, &symbol_count| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = UltraFastStorage::open(temp_dir.path()).unwrap();
                        let symbols = generate_test_symbols(symbol_count);
                        let data: Vec<(String, Symbol)> =
                            symbols.into_iter().map(|s| (s.id.clone(), s)).collect();
                        (storage, data, temp_dir)
                    },
                    |(storage, data, _temp_dir)| {
                        storage.pipeline_batch_save(data).unwrap();
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        // メモリプール最適化版
        group.bench_with_input(
            BenchmarkId::new("memory_pool_save", size),
            size,
            |b, &symbol_count| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let base_storage = UltraFastStorage::open(temp_dir.path()).unwrap();
                        let storage = MemoryPoolStorage::new(base_storage);
                        let symbols = generate_test_symbols(symbol_count);
                        (storage, symbols, temp_dir)
                    },
                    |(storage, symbols, _temp_dir)| {
                        for symbol in symbols {
                            storage.save_with_pool(&symbol.id, &symbol).unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn benchmark_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");

    // メモリ効率的な保存
    group.bench_function("streaming_save", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let storage = ParallelIndexStorage::open(temp_dir.path()).unwrap();
                (storage, temp_dir)
            },
            |(storage, _temp_dir)| {
                // ストリーミング保存（メモリ効率重視）
                for i in 0..1000 {
                    let symbol = Symbol {
                        id: format!("stream_{i}"),
                        kind: SymbolKind::Function,
                        name: format!("func_{i}"),
                        file_path: "stream.rs".to_string(),
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
                    };

                    storage
                        .save_symbols_parallel(&[(symbol.id.clone(), symbol)])
                        .unwrap();
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn benchmark_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");
    group.sample_size(10);

    // 並行読み書きベンチマーク - 既存実装
    group.bench_function("concurrent_read_write_parallel", |b| {
        let temp_dir = TempDir::new().unwrap();
        let storage = ParallelIndexStorage::open(temp_dir.path()).unwrap();

        // 事前にデータを保存
        let symbols: Vec<(String, Symbol)> = generate_test_symbols(1000)
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();
        storage.save_symbols_parallel(&symbols).unwrap();

        b.iter(|| {
            use rayon::prelude::*;

            // 並行読み書き
            (0..100).into_par_iter().for_each(|i| {
                if i % 2 == 0 {
                    // 読み込み
                    let _: Option<Symbol> = storage
                        .load_symbols_parallel(&[format!("symbol_{i}")])
                        .unwrap()
                        .into_iter()
                        .next()
                        .flatten();
                } else {
                    // 書き込み
                    let symbol = Symbol {
                        id: format!("concurrent_{i}"),
                        kind: SymbolKind::Variable,
                        name: format!("var_{i}"),
                        file_path: "concurrent.rs".to_string(),
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
                    };
                    storage
                        .save_symbols_parallel(&[(symbol.id.clone(), symbol)])
                        .unwrap();
                }
            });
        });
    });

    // 並行読み書きベンチマーク - 超高速実装
    group.bench_function("concurrent_read_write_ultra", |b| {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(UltraFastStorage::open(temp_dir.path()).unwrap());

        // 事前にデータを保存
        let symbols = generate_test_symbols(1000);
        let data: Vec<(String, Symbol)> = symbols.into_iter().map(|s| (s.id.clone(), s)).collect();
        storage.pipeline_batch_save(data).unwrap();

        b.iter(|| {
            use rayon::prelude::*;

            // 並行読み書き
            (0..100).into_par_iter().for_each(|i| {
                let storage = Arc::clone(&storage);
                if i % 2 == 0 {
                    // 読み込み
                    let key = format!("symbol_{i}");
                    let _: Option<Symbol> = storage.load_mmap(key.as_bytes()).unwrap();
                } else {
                    // 書き込み
                    let symbol = Symbol {
                        id: format!("concurrent_{i}"),
                        kind: SymbolKind::Variable,
                        name: format!("var_{i}"),
                        file_path: "concurrent.rs".to_string(),
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
                    };
                    storage
                        .save_zero_copy(symbol.id.as_bytes(), &symbol)
                        .unwrap();
                }
            });
        });
    });

    group.finish();
}

fn benchmark_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    // 圧縮効果のベンチマーク
    for compression in ["none", "lz4", "zstd"].iter() {
        group.bench_with_input(
            BenchmarkId::new("save_with_compression", compression),
            compression,
            |b, &compression_type| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = ParallelIndexStorage::open(temp_dir.path()).unwrap();
                        let symbols = generate_test_symbols(1000);
                        (storage, symbols, temp_dir)
                    },
                    |(storage, symbols, _temp_dir)| {
                        // 圧縮タイプに応じた保存
                        match compression_type {
                            "none" => {
                                let data: Vec<(String, Symbol)> =
                                    symbols.into_iter().map(|s| (s.id.clone(), s)).collect();
                                storage.save_symbols_parallel(&data).unwrap();
                            }
                            "lz4" => {
                                // LZ4圧縮（シミュレーション）
                                let data: Vec<(String, Symbol)> =
                                    symbols.into_iter().map(|s| (s.id.clone(), s)).collect();
                                storage.save_symbols_chunked(&data, 100).unwrap();
                            }
                            "zstd" => {
                                // Zstd圧縮（シミュレーション）
                                let data: Vec<(String, Symbol)> =
                                    symbols.into_iter().map(|s| (s.id.clone(), s)).collect();
                                storage.save_symbols_chunked(&data, 50).unwrap();
                            }
                            _ => {}
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// 最適なチャンクサイズを計算
fn calculate_optimal_chunk_size(total_items: usize) -> usize {
    let num_threads = rayon::current_num_threads();
    let base_chunk_size = 100;

    // スレッド数とデータサイズに基づいて最適化
    let optimal = (total_items / num_threads).max(base_chunk_size);

    // 上限を設定（メモリ効率のため）
    optimal.min(1000)
}

// キャッシュ効率のベンチマーク
fn benchmark_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_efficiency");

    // キャッシュヒット率テスト
    group.bench_function("cache_hit_rate", |b| {
        let temp_dir = TempDir::new().unwrap();
        let base_storage = UltraFastStorage::open(temp_dir.path()).unwrap();
        let storage = MemoryPoolStorage::with_cache_size(base_storage, 1000);

        // データを事前に保存
        let symbols = generate_test_symbols(500);
        for symbol in &symbols {
            storage.save_with_pool(&symbol.id, symbol).unwrap();
        }

        b.iter(|| {
            // 80%は既存データ、20%は新規データをアクセス
            for i in 0..100 {
                let id = if i % 5 == 0 {
                    format!("symbol_{}", 500 + i) // キャッシュミス
                } else {
                    format!("symbol_{}", i % 500) // キャッシュヒット
                };
                let _: Option<Symbol> = storage.load_data(&id).unwrap();
            }
        });

        // キャッシュ統計を表示
        let stats = storage.get_cache_stats();
        println!(
            "Cache hit rate: {:.2}% (hits: {}, misses: {})",
            stats.hit_rate * 100.0,
            stats.hits,
            stats.misses
        );
    });

    // キャッシュサイズ別のパフォーマンス比較
    for cache_size in [100, 1000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("with_cache_size", cache_size),
            cache_size,
            |b, &cache_size| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let base_storage = UltraFastStorage::open(temp_dir.path()).unwrap();
                        let storage = MemoryPoolStorage::with_cache_size(base_storage, cache_size);
                        let symbols = generate_test_symbols(10000);
                        (storage, symbols, temp_dir)
                    },
                    |(storage, symbols, _temp_dir)| {
                        // ランダムアクセスパターン
                        let mut rng = thread_rng();

                        for _ in 0..1000 {
                            let idx = rng.gen_range(0..symbols.len());
                            let symbol = &symbols[idx];

                            if rng.gen_bool(0.3) {
                                // 30%は書き込み
                                storage.save_with_pool(&symbol.id, symbol).unwrap();
                            } else {
                                // 70%は読み込み
                                let _: Option<Symbol> = storage.load_data(&symbol.id).unwrap();
                            }
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_extreme_scale,
    benchmark_cache_efficiency,
    benchmark_memory_efficiency,
    benchmark_concurrent_access,
    benchmark_compression
);
criterion_main!(benches);
