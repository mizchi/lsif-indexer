use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use lsif_core::memory_pool::SymbolPool;
use lsif_core::optimized_graph::OptimizedCodeGraph;
use lsif_core::{CodeGraph, Position, Range, Symbol, SymbolKind};

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
        detail: None,
    }
}

/// 標準的なSymbol割り当てベンチマーク
fn benchmark_standard_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("symbol_allocation");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("standard", size), size, |b, &size| {
            b.iter(|| {
                let mut symbols = Vec::with_capacity(size);
                for i in 0..size {
                    symbols.push(create_test_symbol(i));
                }
                black_box(symbols)
            })
        });

        group.bench_with_input(BenchmarkId::new("pooled", size), size, |b, &size| {
            b.iter_batched_ref(
                || SymbolPool::new(size),
                |pool| {
                    let mut symbols = Vec::with_capacity(size);
                    for i in 0..size {
                        let sym = create_test_symbol(i);
                        let pooled = pool.acquire(
                            sym.id,
                            sym.name,
                            sym.kind,
                            sym.file_path,
                            sym.range,
                            sym.documentation,
                        );
                        symbols.push(pooled);
                    }
                    black_box(symbols)
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

/// グラフ構築のベンチマーク
fn benchmark_graph_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_construction");

    for size in [500, 2000, 5000].iter() {
        // 標準のCodeGraph
        group.bench_with_input(
            BenchmarkId::new("standard_graph", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut graph = CodeGraph::new();

                    // Symbolを追加
                    for i in 0..size {
                        graph.add_symbol(create_test_symbol(i));
                    }

                    // エッジは追加しない（Symbol処理のみのベンチマーク）

                    black_box(graph)
                })
            },
        );

        // 最適化されたOptimizedCodeGraph
        group.bench_with_input(
            BenchmarkId::new("optimized_graph", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let graph = OptimizedCodeGraph::with_pool_size(size);

                    // Symbolをバッチで追加
                    let symbols: Vec<Symbol> = (0..size).map(create_test_symbol).collect();
                    graph.add_symbols_batch(symbols);

                    // エッジは追加しない（Symbol処理のみのベンチマーク）

                    black_box(graph)
                })
            },
        );
    }

    group.finish();
}

/// Symbol検索のベンチマーク
fn benchmark_symbol_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("symbol_operations");

    let size = 10000;

    // 標準グラフを準備
    let standard_graph = {
        let mut graph = CodeGraph::new();
        for i in 0..size {
            graph.add_symbol(create_test_symbol(i));
        }
        // エッジは追加しない（Symbol処理のみのベンチマーク）
        graph
    };

    // 最適化グラフを準備
    let optimized_graph = {
        let graph = OptimizedCodeGraph::with_pool_size(size);
        let symbols: Vec<Symbol> = (0..size).map(create_test_symbol).collect();
        graph.add_symbols_batch(symbols);
        // エッジは追加しない（Symbol処理のみのベンチマーク）
        graph
    };

    // Symbol取得のベンチマーク
    group.bench_function("standard_get_symbol", |b| {
        b.iter(|| {
            for i in (0..100).step_by(10) {
                black_box(standard_graph.find_symbol(&format!("symbol_{}", i)));
            }
        })
    });

    group.bench_function("optimized_get_symbol", |b| {
        b.iter(|| {
            for i in (0..100).step_by(10) {
                black_box(optimized_graph.get_symbol(&format!("symbol_{}", i)));
            }
        })
    });

    // 参照検索のベンチマーク
    group.bench_function("standard_find_references", |b| {
        b.iter(|| {
            black_box(standard_graph.find_references("symbol_5000"));
        })
    });

    group.bench_function("optimized_find_references", |b| {
        b.iter(|| {
            black_box(optimized_graph.find_references("symbol_5000"));
        })
    });

    group.finish();
}

/// メモリプールの再利用ベンチマーク
fn benchmark_pool_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool_reuse");

    group.bench_function("without_reuse", |b| {
        b.iter(|| {
            let mut symbols = Vec::new();

            // 1000個作成、破棄、再作成を5回繰り返す
            for round in 0..5 {
                for i in 0..1000 {
                    symbols.push(create_test_symbol(round * 1000 + i));
                }
                symbols.clear(); // 破棄
            }

            black_box(symbols)
        })
    });

    group.bench_function("with_pool_reuse", |b| {
        b.iter_batched_ref(
            || SymbolPool::new(1000),
            |pool| {
                let mut pooled_symbols = Vec::new();

                // 1000個作成、返却、再利用を5回繰り返す
                for round in 0..5 {
                    for i in 0..1000 {
                        let sym = create_test_symbol(round * 1000 + i);
                        pooled_symbols.push(pool.acquire(
                            sym.id,
                            sym.name,
                            sym.kind,
                            sym.file_path,
                            sym.range,
                            sym.documentation,
                        ));
                    }

                    // プールに返却
                    for pooled in pooled_symbols.drain(..) {
                        pool.release(pooled);
                    }
                }

                black_box(pool.stats())
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

/// 実際のインデックス処理シミュレーション
fn benchmark_realistic_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_indexing");

    // 小規模プロジェクト（100ファイル、各30シンボル）
    group.bench_function("small_project_standard", |b| {
        b.iter(|| {
            let mut graph = CodeGraph::new();

            for file_idx in 0..100 {
                for sym_idx in 0..30 {
                    let id = file_idx * 30 + sym_idx;
                    graph.add_symbol(create_test_symbol(id));
                }
            }

            black_box(graph.symbol_count())
        })
    });

    group.bench_function("small_project_optimized", |b| {
        b.iter(|| {
            let graph = OptimizedCodeGraph::with_pool_size(3000);

            let symbols: Vec<Symbol> = (0..3000).map(create_test_symbol).collect();
            graph.add_symbols_batch(symbols);

            black_box(graph.symbol_count())
        })
    });

    // 中規模プロジェクト（500ファイル、各50シンボル）
    group.bench_function("medium_project_standard", |b| {
        b.iter(|| {
            let mut graph = CodeGraph::new();

            for file_idx in 0..500 {
                for sym_idx in 0..50 {
                    let id = file_idx * 50 + sym_idx;
                    graph.add_symbol(create_test_symbol(id));
                }
            }

            black_box(graph.symbol_count())
        })
    });

    group.bench_function("medium_project_optimized", |b| {
        b.iter(|| {
            let graph = OptimizedCodeGraph::with_pool_size(25000);

            let symbols: Vec<Symbol> = (0..25000).map(create_test_symbol).collect();
            graph.add_symbols_batch(symbols);

            black_box(graph.symbol_count())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_standard_allocation,
    benchmark_graph_construction,
    benchmark_symbol_operations,
    benchmark_pool_reuse,
    benchmark_realistic_indexing
);

criterion_main!(benches);
