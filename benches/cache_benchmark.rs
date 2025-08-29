use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use lsif_indexer::cli::cached_storage::CachedIndexStorage;
use lsif_indexer::cli::storage::IndexStorage;
use lsif_indexer::core::graph::{Position, Range, Symbol, SymbolKind};
use std::time::Duration;
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

fn benchmark_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_performance");

    // キャッシュヒット率のベンチマーク
    group.bench_function("cache_hit_rate", |b| {
        let temp_dir = TempDir::new().unwrap();
        let storage =
            CachedIndexStorage::open_with_config(temp_dir.path(), 100, Duration::from_secs(60))
                .unwrap();

        // データを事前に保存
        let symbols = generate_test_symbols(100);
        for symbol in &symbols {
            storage.save_data_cached(&symbol.id, symbol).unwrap();
        }

        b.iter(|| {
            // 同じデータを繰り返し読み込み（キャッシュヒット）
            for i in 0..10 {
                let _: Option<Symbol> = storage.load_data_cached(&format!("symbol_{i}")).unwrap();
            }
        });
    });

    // キャッシュなしとの比較
    group.bench_function("no_cache_baseline", |b| {
        let temp_dir = TempDir::new().unwrap();
        let storage = IndexStorage::open(temp_dir.path()).unwrap();

        // データを事前に保存
        let symbols = generate_test_symbols(100);
        for symbol in &symbols {
            storage.save_data(&symbol.id, symbol).unwrap();
        }

        b.iter(|| {
            for i in 0..10 {
                let _: Option<Symbol> = storage.load_data(&format!("symbol_{i}")).unwrap();
            }
        });
    });

    // バッチ読み込みの性能
    for size in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_load_cached", size),
            size,
            |b, &batch_size| {
                let temp_dir = TempDir::new().unwrap();
                let storage = CachedIndexStorage::open_with_config(
                    temp_dir.path(),
                    200,
                    Duration::from_secs(60),
                )
                .unwrap();

                // データを事前に保存
                let symbols = generate_test_symbols(200);
                for symbol in &symbols {
                    storage.save_data_cached(&symbol.id, symbol).unwrap();
                }

                let keys: Vec<String> = (0..batch_size).map(|i| format!("symbol_{i}")).collect();

                b.iter(|| {
                    let _: Vec<Option<Symbol>> = storage.load_batch_cached(&keys).unwrap();
                });
            },
        );
    }

    // プリフェッチの効果
    group.bench_function("prefetch_effectiveness", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let storage = CachedIndexStorage::open_with_config(
                    temp_dir.path(),
                    50,
                    Duration::from_secs(60),
                )
                .unwrap();

                // データを事前に保存
                let symbols = generate_test_symbols(100);
                for symbol in &symbols {
                    storage.save_data_cached(&symbol.id, symbol).unwrap();
                }
                storage.clear_cache(); // キャッシュをクリア

                (storage, temp_dir)
            },
            |(storage, _temp_dir)| {
                // プリフェッチ
                let prefetch_keys: Vec<String> = (0..20).map(|i| format!("symbol_{i}")).collect();
                storage.prefetch(&prefetch_keys).unwrap();

                // プリフェッチしたデータを読み込み
                for i in 0..20 {
                    let _: Option<Symbol> =
                        storage.load_data_cached(&format!("symbol_{i}")).unwrap();
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn benchmark_cache_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_memory");

    // キャッシュサイズによる性能変化
    for cache_size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("cache_size_impact", cache_size),
            cache_size,
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let storage = CachedIndexStorage::open_with_config(
                    temp_dir.path(),
                    size,
                    Duration::from_secs(300),
                )
                .unwrap();

                // キャッシュサイズを超えるデータを保存
                let symbols = generate_test_symbols(size * 2);
                for symbol in &symbols {
                    storage.save_data_cached(&symbol.id, symbol).unwrap();
                }

                b.iter(|| {
                    // ランダムアクセスパターン
                    for _ in 0..100 {
                        let idx = (rand::random::<usize>() % (size * 2)) as usize;
                        let _: Option<Symbol> =
                            storage.load_data_cached(&format!("symbol_{idx}")).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn benchmark_warmup(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_warmup");

    group.bench_function("cold_start", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let storage = CachedIndexStorage::open(temp_dir.path()).unwrap();

                // データを保存
                let symbols = generate_test_symbols(100);
                for symbol in &symbols {
                    storage.save_data_cached(&symbol.id, symbol).unwrap();
                }
                storage.clear_cache();

                (storage, temp_dir)
            },
            |(storage, _temp_dir)| {
                // コールドスタート（キャッシュなし）
                for i in 0..50 {
                    let _: Option<Symbol> =
                        storage.load_data_cached(&format!("symbol_{i}")).unwrap();
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("warm_start", |b| {
        let temp_dir = TempDir::new().unwrap();
        let storage = CachedIndexStorage::open(temp_dir.path()).unwrap();

        // データを保存
        let symbols = generate_test_symbols(100);
        for symbol in &symbols {
            storage.save_data_cached(&symbol.id, symbol).unwrap();
        }

        // ウォームアップ
        let hot_keys: Vec<String> = (0..50).map(|i| format!("symbol_{i}")).collect();
        storage.warmup(&hot_keys).unwrap();

        b.iter(|| {
            // ウォームスタート（プリロード済み）
            for i in 0..50 {
                let _: Option<Symbol> = storage.load_data_cached(&format!("symbol_{i}")).unwrap();
            }
        });
    });

    group.finish();
}

// 外部クレートを使わない簡易ランダム生成
mod rand {
    use std::cell::Cell;
    use std::time::{SystemTime, UNIX_EPOCH};

    thread_local! {
        static SEED: Cell<u64> = Cell::new({
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        });
    }

    pub fn random<T>() -> T
    where
        Standard: Distribution<T>,
    {
        SEED.with(|seed| {
            let mut s = seed.get();
            s = s.wrapping_mul(1103515245).wrapping_add(12345);
            seed.set(s);
            Standard.sample(s)
        })
    }

    trait Distribution<T> {
        fn sample(&self, seed: u64) -> T;
    }

    struct Standard;

    impl Distribution<usize> for Standard {
        fn sample(&self, seed: u64) -> usize {
            (seed >> 16) as usize
        }
    }
}

criterion_group!(
    benches,
    benchmark_cache_performance,
    benchmark_cache_memory,
    benchmark_warmup
);
criterion_main!(benches);
