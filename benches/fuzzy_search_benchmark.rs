use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use lsif_core::{fuzzy_search::FuzzySearchIndex, CodeGraph, Position, Range, Symbol, SymbolKind};
use std::collections::HashSet;

/// テスト用のシンボルを生成
fn create_test_symbols(count: usize) -> Vec<Symbol> {
    let mut symbols = Vec::with_capacity(count);

    // 実際のコードベースに存在しそうな名前パターン
    let prefixes = [
        "get",
        "set",
        "create",
        "update",
        "delete",
        "find",
        "search",
        "calculate",
        "process",
        "handle",
    ];
    let middles = [
        "User",
        "Item",
        "Order",
        "Product",
        "Customer",
        "Transaction",
        "Session",
        "Request",
        "Response",
        "Cache",
    ];
    let suffixes = [
        "ById", "ByName", "List", "Details", "Info", "Data", "Config", "Status", "Count", "Total",
    ];

    let mut index = 0;
    for prefix in &prefixes {
        for middle in &middles {
            for suffix in &suffixes {
                if index >= count {
                    return symbols;
                }

                let name = format!("{}{}{}", prefix, middle, suffix);
                symbols.push(Symbol {
                    id: format!("symbol_{}", index),
                    name: name.clone(),
                    kind: match index % 5 {
                        0 => SymbolKind::Function,
                        1 => SymbolKind::Class,
                        2 => SymbolKind::Method,
                        3 => SymbolKind::Variable,
                        _ => SymbolKind::Constant,
                    },
                    file_path: format!("src/module_{}/{}.rs", index / 100, name.to_lowercase()),
                    range: Range {
                        start: Position {
                            line: (index % 1000) as u32,
                            character: 0,
                        },
                        end: Position {
                            line: ((index % 1000) + 5) as u32,
                            character: 80,
                        },
                    },
                    documentation: if index % 3 == 0 {
                        Some(format!("Documentation for {}", name))
                    } else {
                        None
                    },
                    detail: None,
                });
                index += 1;
            }
        }
    }

    // 追加のランダムな名前
    while symbols.len() < count {
        let name = format!("symbol_random_{}", symbols.len());
        symbols.push(Symbol {
            id: format!("symbol_{}", symbols.len()),
            name: name.clone(),
            kind: SymbolKind::Function,
            file_path: format!("src/random/{}.rs", name),
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
        });
    }

    symbols
}

/// インデックス構築のベンチマーク
fn benchmark_index_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_building");

    for size in [100, 500, 1000, 5000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let symbols = create_test_symbols(size);
            let mut graph = CodeGraph::new();
            for symbol in &symbols {
                graph.add_symbol(symbol.clone());
            }

            b.iter(|| {
                let index = FuzzySearchIndex::build_from_graph(&graph);
                black_box(index.stats())
            });
        });
    }

    group.finish();
}

/// 検索パフォーマンスのベンチマーク
fn benchmark_search_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_performance");

    // 10000シンボルのインデックスを準備
    let symbols = create_test_symbols(10000);
    let mut graph = CodeGraph::new();
    for symbol in &symbols {
        graph.add_symbol(symbol.clone());
    }
    let index = FuzzySearchIndex::build_from_graph(&graph);

    // 様々なクエリパターン
    let queries = vec![
        ("exact", "getUserById"), // 完全一致
        ("prefix", "getUser"),    // プレフィックスマッチ
        ("typo", "getUserByld"),  // タイポ（Id -> ld）
        ("fuzzy", "gtUsrBId"),    // 大幅な省略
        ("abbreviation", "gubn"), // 略語
        ("partial", "UserBy"),    // 部分文字列
        ("case", "getuserbyid"),  // 大文字小文字の違い
        ("swap", "getByIdUser"),  // 単語の順序入れ替え
    ];

    for (query_type, query) in queries {
        group.bench_with_input(
            BenchmarkId::new("query_type", query_type),
            &query,
            |b, query| {
                b.iter(|| {
                    let results = index.search(query, 10);
                    black_box(results.len())
                });
            },
        );
    }

    group.finish();
}

/// 結果数による検索パフォーマンス
fn benchmark_search_by_result_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_by_result_count");

    let symbols = create_test_symbols(10000);
    let mut graph = CodeGraph::new();
    for symbol in &symbols {
        graph.add_symbol(symbol.clone());
    }
    let index = FuzzySearchIndex::build_from_graph(&graph);

    // 汎用的なクエリ（多くの結果を返す）
    let query = "get";

    for max_results in [1, 10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(max_results),
            max_results,
            |b, &max_results| {
                b.iter(|| {
                    let results = index.search(query, max_results);
                    black_box(results.len())
                });
            },
        );
    }

    group.finish();
}

/// インデックスサイズによる検索パフォーマンス
fn benchmark_search_by_index_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_by_index_size");

    let query = "getUserById";

    for size in [100, 1000, 5000, 10000, 20000].iter() {
        let symbols = create_test_symbols(*size);
        let mut graph = CodeGraph::new();
        for symbol in &symbols {
            graph.add_symbol(symbol.clone());
        }
        let index = FuzzySearchIndex::build_from_graph(&graph);

        group.bench_with_input(BenchmarkId::from_parameter(size), &query, |b, query| {
            b.iter(|| {
                let results = index.search(query, 10);
                black_box(results.len())
            });
        });
    }

    group.finish();
}

/// トライグラム生成のベンチマーク
fn benchmark_trigram_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("trigram_generation");

    let test_strings = vec![
        ("short", "api"),
        ("medium", "getUserById"),
        ("long", "calculateTotalTransactionAmountForCustomer"),
        ("snake_case", "get_user_by_id_and_name"),
        ("camelCase", "getUserByIdAndName"),
        ("PascalCase", "GetUserByIdAndName"),
        ("with_numbers", "calculateSum123ForUser456"),
    ];

    for (name, text) in test_strings {
        group.bench_with_input(BenchmarkId::new("text_type", name), &text, |b, text| {
            b.iter(|| {
                // generate_trigramsはprivateなので、インデックスに追加することでテスト
                let index = FuzzySearchIndex::new();
                let symbol = Symbol {
                    id: "test".to_string(),
                    name: text.to_string(),
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
                index.add_symbol(symbol);
                black_box(index.stats())
            });
        });
    }

    group.finish();
}

/// メモリ使用量のベンチマーク（統計情報を通じて）
fn benchmark_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");

    group.bench_function("stats_calculation_10k", |b| {
        let symbols = create_test_symbols(10000);
        let mut graph = CodeGraph::new();
        for symbol in &symbols {
            graph.add_symbol(symbol.clone());
        }
        let index = FuzzySearchIndex::build_from_graph(&graph);

        b.iter(|| {
            let stats = index.stats();
            black_box((
                stats.total_symbols,
                stats.total_trigrams,
                stats.avg_symbols_per_trigram,
            ))
        });
    });

    group.finish();
}

/// 並行アクセスのベンチマーク
fn benchmark_concurrent_access(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("concurrent_access");

    let symbols = create_test_symbols(10000);
    let mut graph = CodeGraph::new();
    for symbol in &symbols {
        graph.add_symbol(symbol.clone());
    }
    let index = Arc::new(FuzzySearchIndex::build_from_graph(&graph));

    group.bench_function("concurrent_searches", |b| {
        b.iter(|| {
            let mut handles = vec![];
            let queries = vec!["get", "set", "user", "item", "data"];

            for query in queries {
                let index_clone = index.clone();
                let handle = thread::spawn(move || {
                    let results = index_clone.search(query, 10);
                    black_box(results.len())
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_index_building,
    benchmark_search_performance,
    benchmark_search_by_result_count,
    benchmark_search_by_index_size,
    benchmark_trigram_generation,
    benchmark_memory_efficiency,
    benchmark_concurrent_access
);

criterion_main!(benches);
