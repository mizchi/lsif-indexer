use core::lockfree_graph::{LockFreeGraph, WaitFreeReadGraph};
use core::{CodeGraph, Position, Range, Symbol, SymbolKind};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use std::sync::Arc;
use std::thread;

/// テスト用のSymbolを生成
fn create_test_symbol(id: usize) -> Symbol {
    Symbol {
        id: format!("symbol_{}", id),
        name: format!("function_{}", id),
        kind: match id % 5 {
            0 => SymbolKind::Function,
            1 => SymbolKind::Class,
            2 => SymbolKind::Method,
            3 => SymbolKind::Variable,
            _ => SymbolKind::Constant,
        },
        file_path: format!("src/module_{}/file_{}.rs", id / 100, id / 10),
        range: Range {
            start: Position {
                line: ((id % 100) * 10) as u32,
                character: 0,
            },
            end: Position {
                line: ((id % 100) * 10 + 5) as u32,
                character: 80,
            },
        },
        documentation: if id % 3 == 0 {
            Some(format!("Documentation for symbol {}", id))
        } else {
            None
        },
    }
}

/// シングルスレッドでのグラフ構築ベンチマーク
fn benchmark_single_thread_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_construction");

    for size in [1000, 5000, 10000].iter() {
        // 標準のCodeGraph
        group.bench_with_input(
            BenchmarkId::new("standard_graph", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut graph = CodeGraph::new();
                    for i in 0..size {
                        graph.add_symbol(create_test_symbol(i));
                    }
                    black_box(graph.symbol_count())
                })
            },
        );

        // ロックフリーグラフ
        group.bench_with_input(
            BenchmarkId::new("lockfree_graph", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let graph = LockFreeGraph::new();
                    for i in 0..size {
                        graph.add_symbol(create_test_symbol(i));
                    }
                    black_box(graph.symbol_count())
                })
            },
        );

        // Wait-Free読み取りグラフ
        group.bench_with_input(
            BenchmarkId::new("waitfree_read_graph", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let graph = WaitFreeReadGraph::new();
                    for i in 0..size {
                        graph.add_symbol(create_test_symbol(i));
                    }
                    graph.process_writes(); // バッチ処理を強制実行
                    black_box(graph.symbol_count())
                })
            },
        );
    }

    group.finish();
}

/// マルチスレッドでの並行書き込みベンチマーク
fn benchmark_concurrent_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_writes");

    let num_threads = 4;
    let symbols_per_thread = 250;

    // ロックフリーグラフ
    group.bench_function("lockfree_concurrent", |b| {
        b.iter(|| {
            let graph = Arc::new(LockFreeGraph::new());
            let mut handles = vec![];

            for thread_id in 0..num_threads {
                let g = graph.clone();
                let handle = thread::spawn(move || {
                    for i in 0..symbols_per_thread {
                        let id = thread_id * symbols_per_thread + i;
                        g.add_symbol(create_test_symbol(id));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(graph.symbol_count())
        })
    });

    // DashMapを使用した比較（coreのOptimizedCodeGraphから）
    group.bench_function("dashmap_concurrent", |b| {
        b.iter(|| {
            let graph = Arc::new(core::optimized_graph::OptimizedCodeGraph::with_pool_size(
                1000,
            ));
            let mut handles = vec![];

            for thread_id in 0..num_threads {
                let g = graph.clone();
                let handle = thread::spawn(move || {
                    for i in 0..symbols_per_thread {
                        let id = thread_id * symbols_per_thread + i;
                        g.add_symbol(create_test_symbol(id));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(graph.symbol_count())
        })
    });

    group.finish();
}

/// 読み取り性能のベンチマーク
fn benchmark_read_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_performance");

    let size = 10000;

    // 事前にデータを準備
    let standard_graph = {
        let mut graph = CodeGraph::new();
        for i in 0..size {
            graph.add_symbol(create_test_symbol(i));
        }
        graph
    };

    let lockfree_graph = {
        let graph = LockFreeGraph::new();
        for i in 0..size {
            graph.add_symbol(create_test_symbol(i));
        }
        graph
    };

    let waitfree_graph = {
        let graph = WaitFreeReadGraph::new();
        for i in 0..size {
            graph.add_symbol(create_test_symbol(i));
        }
        graph.process_writes();
        graph
    };

    // ランダムアクセスの読み取り
    let test_ids: Vec<String> = (0..100).map(|i| format!("symbol_{}", i * 100)).collect();

    group.bench_function("standard_read", |b| {
        b.iter(|| {
            for id in &test_ids {
                black_box(standard_graph.find_symbol(id));
            }
        })
    });

    group.bench_function("lockfree_read", |b| {
        b.iter(|| {
            for id in &test_ids {
                black_box(lockfree_graph.get_symbol(id));
            }
        })
    });

    group.bench_function("waitfree_read", |b| {
        b.iter(|| {
            for id in &test_ids {
                black_box(waitfree_graph.get_symbol(id));
            }
        })
    });

    group.finish();
}

/// 混合ワークロード（読み書き並行）のベンチマーク
fn benchmark_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    // 80%読み取り、20%書き込みのワークロード
    group.bench_function("lockfree_mixed_80_20", |b| {
        b.iter_batched_ref(
            || {
                let graph = Arc::new(LockFreeGraph::new());
                // 初期データを投入
                for i in 0..1000 {
                    graph.add_symbol(create_test_symbol(i));
                }
                graph
            },
            |graph| {
                let mut handles = vec![];

                // リーダースレッド（3つ）
                for _ in 0..3 {
                    let g = graph.clone();
                    let handle = thread::spawn(move || {
                        for _ in 0..800 {
                            let id = format!("symbol_{}", (rand::random::<u64>() as usize) % 1000);
                            black_box(g.get_symbol(&id));
                        }
                    });
                    handles.push(handle);
                }

                // ライタースレッド（1つ）
                let g = graph.clone();
                let handle = thread::spawn(move || {
                    for i in 0..200 {
                        g.add_symbol(create_test_symbol(1000 + i));
                    }
                });
                handles.push(handle);

                for handle in handles {
                    handle.join().unwrap();
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

/// CAS（Compare-And-Swap）操作のベンチマーク
fn benchmark_cas_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cas_operations");

    let graph = LockFreeGraph::new();

    // 初期データ投入
    for i in 0..100 {
        graph.add_symbol(create_test_symbol(i));
    }

    group.bench_function("cas_update", |b| {
        let mut counter = 0;
        b.iter(|| {
            let id = format!("symbol_{}", counter % 100);
            let success = graph.update_symbol(&id, |s| {
                let mut new_symbol = s.clone();
                new_symbol.name = format!("updated_{}", counter);
                new_symbol
            });
            counter += 1;
            black_box(success)
        })
    });

    group.finish();
}

/// バッチ処理のベンチマーク
fn benchmark_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");

    let batch_sizes = [100, 500, 1000];

    for batch_size in batch_sizes.iter() {
        // ロックフリーグラフのバッチ追加
        group.bench_with_input(
            BenchmarkId::new("lockfree_batch", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let graph = LockFreeGraph::new();
                    let symbols: Vec<Symbol> = (0..size).map(|i| create_test_symbol(i)).collect();
                    graph.add_symbols_batch(symbols);
                    black_box(graph.symbol_count())
                })
            },
        );

        // 標準グラフの逐次追加（比較用）
        group.bench_with_input(
            BenchmarkId::new("standard_sequential", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut graph = CodeGraph::new();
                    for i in 0..size {
                        graph.add_symbol(create_test_symbol(i));
                    }
                    black_box(graph.symbol_count())
                })
            },
        );
    }

    group.finish();
}

// rand crateの簡易実装
mod rand {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEED: AtomicU64 = AtomicU64::new(12345);

    pub fn random<T>() -> T
    where
        T: From<u64>,
    {
        let seed = SEED.fetch_add(1, Ordering::Relaxed);
        let val = seed.wrapping_mul(1103515245).wrapping_add(12345);
        T::from(val)
    }
}

criterion_group!(
    benches,
    benchmark_single_thread_construction,
    benchmark_concurrent_writes,
    benchmark_read_performance,
    benchmark_mixed_workload,
    benchmark_cas_operations,
    benchmark_batch_operations
);

criterion_main!(benches);
