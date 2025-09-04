use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use lsif_core::interned_graph::InternedGraph;
use lsif_core::optimized_graph::OptimizedCodeGraph;
use lsif_core::string_interner::StringInterner;
use lsif_core::{CodeGraph, Position, Range, Symbol, SymbolKind};

/// テスト用のSymbolを生成（重複する文字列を含む）
fn create_test_symbol_with_duplicates(id: usize) -> Symbol {
    Symbol {
        id: format!("symbol_{}", id),
        name: format!("function_{}", id % 100), // 100個のユニークな名前
        kind: match id % 5 {
            0 => SymbolKind::Function,
            1 => SymbolKind::Class,
            2 => SymbolKind::Method,
            3 => SymbolKind::Variable,
            _ => SymbolKind::Constant,
        },
        file_path: format!("src/module_{}/file.rs", id % 50), // 50個のユニークなパス
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
        documentation: Some(format!("Documentation type {}", id % 20)), // 20個のユニークなドキュメント
        detail: None,
    }
}

/// 文字列インターン化のベンチマーク
fn benchmark_string_interning(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_interning");

    for size in [1000, 5000, 10000].iter() {
        // 標準的な文字列処理
        group.bench_with_input(
            BenchmarkId::new("standard_strings", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut strings = Vec::with_capacity(size);
                    for i in 0..size {
                        strings.push(format!("string_{}", i % 100)); // 100個のユニークな文字列
                    }
                    black_box(strings)
                })
            },
        );

        // インターン化された文字列
        group.bench_with_input(
            BenchmarkId::new("interned_strings", size),
            size,
            |b, &size| {
                b.iter_batched_ref(
                    StringInterner::new,
                    |interner| {
                        let mut interned = Vec::with_capacity(size);
                        for i in 0..size {
                            let s = format!("string_{}", i % 100);
                            interned.push(interner.intern(&s));
                        }
                        black_box(interned)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

/// グラフ構築のベンチマーク（インターン化あり/なし）
fn benchmark_graph_with_interning(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_with_interning");

    for size in [1000, 5000, 10000].iter() {
        // 標準のCodeGraph
        group.bench_with_input(
            BenchmarkId::new("standard_graph", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut graph = CodeGraph::new();

                    for i in 0..size {
                        graph.add_symbol(create_test_symbol_with_duplicates(i));
                    }

                    black_box(graph.symbol_count())
                })
            },
        );

        // メモリプールを使用したOptimizedCodeGraph
        group.bench_with_input(BenchmarkId::new("pooled_graph", size), size, |b, &size| {
            b.iter(|| {
                let graph = OptimizedCodeGraph::with_pool_size(size);

                let symbols: Vec<Symbol> =
                    (0..size).map(create_test_symbol_with_duplicates).collect();
                graph.add_symbols_batch(symbols);

                black_box(graph.symbol_count())
            })
        });

        // インターン化されたInternedGraph
        group.bench_with_input(
            BenchmarkId::new("interned_graph", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let graph = InternedGraph::new();

                    let symbols: Vec<Symbol> =
                        (0..size).map(create_test_symbol_with_duplicates).collect();
                    graph.add_symbols_batch(symbols);

                    black_box(graph.symbol_count())
                })
            },
        );
    }

    group.finish();
}

/// メモリ使用量のベンチマーク
fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    let size = 10000;

    // 標準グラフのメモリ使用量を推定
    group.bench_function("standard_memory", |b| {
        b.iter(|| {
            let mut graph = CodeGraph::new();
            for i in 0..size {
                graph.add_symbol(create_test_symbol_with_duplicates(i));
            }

            // メモリ使用量の推定
            let memory = size
                * (
                    std::mem::size_of::<Symbol>() +
                50 + // 平均的な文字列サイズ
                20
                    // ハッシュマップのオーバーヘッド
                );
            black_box(memory)
        })
    });

    // インターン化グラフのメモリ使用量
    group.bench_function("interned_memory", |b| {
        b.iter(|| {
            let graph = InternedGraph::new();

            for i in 0..size {
                graph.add_symbol(create_test_symbol_with_duplicates(i));
            }

            let memory = graph.estimated_memory_usage();
            black_box(memory)
        })
    });

    group.finish();
}

/// 検索パフォーマンスのベンチマーク
fn benchmark_search_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_performance");

    let size = 5000;

    // 標準グラフを準備
    let standard_graph = {
        let mut graph = CodeGraph::new();
        for i in 0..size {
            graph.add_symbol(create_test_symbol_with_duplicates(i));
        }
        graph
    };

    // インターン化グラフを準備
    let interned_graph = {
        let graph = InternedGraph::new();
        for i in 0..size {
            graph.add_symbol(create_test_symbol_with_duplicates(i));
        }

        // エッジを追加
        for i in 0..size / 10 {
            graph.add_edge(
                &format!("symbol_{}", i),
                &format!("symbol_{}", (i + 1) % size),
                core::EdgeKind::Reference,
            );
        }
        graph
    };

    // Symbol検索のベンチマーク
    group.bench_function("standard_find_symbol", |b| {
        b.iter(|| {
            for i in (0..100).step_by(10) {
                black_box(standard_graph.find_symbol(&format!("symbol_{}", i)));
            }
        })
    });

    group.bench_function("interned_find_symbol", |b| {
        b.iter(|| {
            for i in (0..100).step_by(10) {
                black_box(interned_graph.get_symbol(&format!("symbol_{}", i)));
            }
        })
    });

    // 参照検索のベンチマーク
    group.bench_function("interned_find_references", |b| {
        b.iter(|| {
            black_box(interned_graph.find_references("symbol_250"));
        })
    });

    group.finish();
}

/// 実際のプロジェクトシミュレーション
fn benchmark_realistic_project(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_project");

    // 大規模プロジェクト（1000ファイル、各50シンボル）
    let total_symbols = 50000;

    group.bench_function("large_project_standard", |b| {
        b.iter(|| {
            let mut graph = CodeGraph::new();

            for file_idx in 0..1000 {
                for sym_idx in 0..50 {
                    let id = file_idx * 50 + sym_idx;
                    graph.add_symbol(create_test_symbol_with_duplicates(id));
                }
            }

            black_box(graph.symbol_count())
        })
    });

    group.bench_function("large_project_interned", |b| {
        b.iter(|| {
            let graph = InternedGraph::new();

            let symbols: Vec<Symbol> = (0..total_symbols)
                .map(create_test_symbol_with_duplicates)
                .collect();
            graph.add_symbols_batch(symbols);

            // 統計情報を取得
            let stats = graph.interner_stats();
            black_box((graph.symbol_count(), stats.cache_hits))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_string_interning,
    benchmark_graph_with_interning,
    benchmark_memory_usage,
    benchmark_search_performance,
    benchmark_realistic_project
);

criterion_main!(benches);
