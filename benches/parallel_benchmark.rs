use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use lsif_core::parallel::{
    parallel_lsif::ParallelLsifGenerator, ParallelCodeGraph, ParallelFileAnalyzer,
    ParallelIncrementalIndex,
};
// use lsif_core::parallel_optimized::{OptimizedDeadCodeDetector, OptimizedParallelGraph};
use lsif_core::incremental::UpdateResult;
use lsif_core::{CodeGraph, EdgeKind, IncrementalIndex, Position, Range, Symbol, SymbolKind};
use std::collections::HashMap;
use std::path::PathBuf;

fn create_symbols(count: usize) -> Vec<Symbol> {
    (0..count)
        .map(|i| Symbol {
            id: format!("symbol_{i}"),
            name: format!("function_{i}"),
            kind: match i % 5 {
                0 => SymbolKind::Function,
                1 => SymbolKind::Class,
                2 => SymbolKind::Method,
                3 => SymbolKind::Variable,
                _ => SymbolKind::Module,
            },
            file_path: format!("src/module_{}/file_{}.rs", i / 100, i / 10),
            range: Range {
                start: Position {
                    line: ((i % 100) * 10) as u32,
                    character: 0,
                },
                end: Position {
                    line: ((i % 100) * 10 + 5) as u32,
                    character: 0,
                },
            },
            documentation: if i % 2 == 0 {
                Some(format!("Documentation for symbol_{i}"))
            } else {
                None
            },
            detail: None,
        })
        .collect()
}

fn benchmark_symbol_addition(c: &mut Criterion) {
    let mut group = c.benchmark_group("symbol_addition");

    for size in [100, 1000, 10000].iter() {
        let symbols = create_symbols(*size);

        // Sequential version
        group.bench_with_input(
            BenchmarkId::new("sequential", size),
            &symbols,
            |b, symbols| {
                b.iter_batched(
                    CodeGraph::new,
                    |mut graph| {
                        for symbol in symbols {
                            graph.add_symbol(symbol.clone());
                        }
                        graph
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        // Parallel version (with Mutex)
        group.bench_with_input(
            BenchmarkId::new("parallel_mutex", size),
            &symbols,
            |b, symbols| {
                b.iter_batched(
                    ParallelCodeGraph::new,
                    |graph| {
                        graph.add_symbols_parallel(symbols.clone());
                        graph
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        // Optimized parallel version (batch processing)
        group.bench_with_input(
            BenchmarkId::new("parallel_optimized", size),
            &symbols,
            |b, symbols| {
                b.iter_batched(
                    CodeGraph::new,
                    |mut graph| {
                        // OptimizedParallelGraph::add_symbols_batch(&mut graph, symbols.clone());
                        for symbol in symbols.clone() {
                            graph.add_symbol(symbol);
                        }
                        graph
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn benchmark_symbol_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("symbol_search");

    for size in [1000, 10000].iter() {
        let mut graph = CodeGraph::new();
        let symbols = create_symbols(*size);
        for symbol in &symbols {
            graph.add_symbol(symbol.clone());
        }

        let search_ids: Vec<String> = (0..*size)
            .step_by(10)
            .map(|i| format!("symbol_{i}"))
            .collect();

        let parallel_graph = ParallelCodeGraph::from_graph(graph.clone());

        // Sequential search
        group.bench_with_input(
            BenchmarkId::new("sequential", size),
            &search_ids,
            |b, ids| {
                b.iter(|| {
                    let mut results = HashMap::new();
                    for id in ids {
                        results.insert(id.clone(), graph.find_symbol(id).cloned());
                    }
                    results
                })
            },
        );

        // Parallel search
        group.bench_with_input(BenchmarkId::new("parallel", size), &search_ids, |b, ids| {
            b.iter(|| {
                let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
                parallel_graph.find_symbols_parallel(id_refs)
            })
        });
    }

    group.finish();
}

fn benchmark_file_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_updates");

    for num_files in [10, 50, 100].iter() {
        let updates: Vec<(PathBuf, Vec<Symbol>, String)> = (0..*num_files)
            .map(|i| {
                let path = PathBuf::from(format!("file_{i}.rs"));
                let symbols = vec![
                    Symbol {
                        id: format!("file{i}_sym1"),
                        name: format!("function1_{i}"),
                        kind: SymbolKind::Function,
                        file_path: format!("file_{i}.rs"),
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 5,
                                character: 0,
                            },
                        },
                        documentation: None,
                        detail: None,
                    },
                    Symbol {
                        id: format!("file{i}_sym2"),
                        name: format!("function2_{i}"),
                        kind: SymbolKind::Function,
                        file_path: format!("file_{i}.rs"),
                        range: Range {
                            start: Position {
                                line: 10,
                                character: 0,
                            },
                            end: Position {
                                line: 15,
                                character: 0,
                            },
                        },
                        documentation: None,
                        detail: None,
                    },
                ];
                let hash = format!("hash_{i}");
                (path, symbols, hash)
            })
            .collect();

        // Sequential update
        group.bench_with_input(
            BenchmarkId::new("sequential", num_files),
            &updates,
            |b, updates| {
                b.iter_batched(
                    IncrementalIndex::new,
                    |mut index| {
                        for (path, symbols, hash) in updates {
                            index
                                .update_file(path, symbols.clone(), hash.clone())
                                .unwrap();
                        }
                        index
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        // Parallel update
        group.bench_with_input(
            BenchmarkId::new("parallel", num_files),
            &updates,
            |b, updates| {
                b.iter_batched(
                    ParallelIncrementalIndex::new,
                    |index| {
                        index.update_files_parallel(updates.clone()).unwrap();
                        index
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn benchmark_dead_code_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("dead_code_detection");

    for size in [100, 500, 1000].iter() {
        // Create index with mix of live and dead symbols
        let mut index = IncrementalIndex::new();

        // Add main function (entry point)
        index
            .add_symbol(Symbol {
                id: "main".to_string(),
                name: "main".to_string(),
                kind: SymbolKind::Function,
                file_path: "main.rs".to_string(),
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 5,
                        character: 0,
                    },
                },
                documentation: None,
                detail: None,
            })
            .unwrap();

        let main_idx = index.graph.get_node_index("main").unwrap();

        // Add connected symbols (live)
        for i in 0..(*size / 2) {
            let symbol = Symbol {
                id: format!("live_{i}"),
                name: format!("live_function_{i}"),
                kind: SymbolKind::Function,
                file_path: format!("file_{}.rs", i / 10),
                range: Range {
                    start: Position {
                        line: i as u32 * 10,
                        character: 0,
                    },
                    end: Position {
                        line: i as u32 * 10 + 5,
                        character: 0,
                    },
                },
                documentation: None,
                detail: None,
            };
            let idx = index.graph.add_symbol(symbol.clone());
            index.symbol_to_file.insert(
                format!("live_{i}"),
                PathBuf::from(format!("file_{}.rs", i / 10)),
            );

            // Connect to main or previous symbol
            if i == 0 {
                index.graph.add_edge(main_idx, idx, EdgeKind::Reference);
            } else if i > 0 {
                let prev_idx = index
                    .graph
                    .get_node_index(&format!("live_{}", i - 1))
                    .unwrap();
                index.graph.add_edge(prev_idx, idx, EdgeKind::Reference);
            }
        }

        // Add unconnected symbols (dead)
        for i in 0..(*size / 2) {
            let symbol = Symbol {
                id: format!("dead_{i}"),
                name: format!("dead_function_{i}"),
                kind: SymbolKind::Function,
                file_path: format!("dead_file_{}.rs", i / 10),
                range: Range {
                    start: Position {
                        line: i as u32 * 10,
                        character: 0,
                    },
                    end: Position {
                        line: i as u32 * 10 + 5,
                        character: 0,
                    },
                },
                documentation: None,
                detail: None,
            };
            index.add_symbol(symbol).unwrap();
        }

        let parallel_index = ParallelIncrementalIndex::from_index(index.clone());

        // Sequential dead code detection
        group.bench_with_input(BenchmarkId::new("sequential", size), &size, |b, _| {
            b.iter_batched(
                || index.clone(),
                |mut idx| {
                    let mut result = UpdateResult::default();
                    idx.detect_dead_code(&mut result);
                    result.dead_symbols
                },
                BatchSize::SmallInput,
            )
        });

        // Parallel dead code detection (Mutex version)
        group.bench_with_input(BenchmarkId::new("parallel_mutex", size), &size, |b, _| {
            b.iter(|| parallel_index.detect_dead_code_parallel().unwrap())
        });

        // Optimized parallel dead code detection - commented out as module doesn't exist
        // group.bench_with_input(
        //     BenchmarkId::new("parallel_optimized", size),
        //     &size,
        //     |b, _| {
        //         b.iter_batched(
        //             || index.graph.clone(),
        //             |graph| OptimizedDeadCodeDetector::detect_parallel(&graph),
        //             BatchSize::SmallInput,
        //         )
        //     },
        // );
    }

    group.finish();
}

fn benchmark_lsif_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsif_generation");

    for size in [100, 500, 1000].iter() {
        let mut graph = CodeGraph::new();
        let symbols = create_symbols(*size);

        for symbol in &symbols {
            graph.add_symbol(symbol.clone());
        }

        // Add some edges
        for i in 0..(*size - 1) {
            if let (Some(from), Some(to)) = (
                graph.get_node_index(&format!("symbol_{i}")),
                graph.get_node_index(&format!("symbol_{}", i + 1)),
            ) {
                graph.add_edge(from, to, EdgeKind::Reference);
            }
        }

        // Sequential LSIF generation
        group.bench_with_input(BenchmarkId::new("sequential", size), &graph, |b, graph| {
            b.iter(|| core::LsifGenerator::new(graph.clone()).generate().unwrap())
        });

        // Parallel LSIF generation
        group.bench_with_input(BenchmarkId::new("parallel", size), &graph, |b, graph| {
            b.iter(|| {
                let generator = ParallelLsifGenerator::new(graph.clone());
                let elements = generator.generate_parallel().unwrap();
                // Convert to JSON lines format
                elements
                    .into_iter()
                    .map(|elem| serde_json::to_string(&elem).unwrap())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        });
    }

    group.finish();
}

fn benchmark_file_hash_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_hash_calculation");

    for num_files in [10, 50, 100].iter() {
        let files: Vec<(PathBuf, String)> = (0..*num_files)
            .map(|i| {
                let path = PathBuf::from(format!("file_{i}.rs"));
                let content = format!(
                    "// File {i}\n\
                    fn function_{i}() {{\n\
                    \tprintln!(\"Hello from file {i}\");\n\
                    \t// Some more content to make it realistic\n\
                    \tlet x = {i};\n\
                    \tlet y = x * 2;\n\
                    \tfor i in 0..10 {{\n\
                    \t\tprintln!(\"{{}}\", i + y);\n\
                    \t}}\n\
                    }}\n"
                );
                (path, content)
            })
            .collect();

        let file_refs: Vec<(&PathBuf, String)> =
            files.iter().map(|(p, c)| (p, c.clone())).collect();

        // Sequential hash calculation
        group.bench_with_input(
            BenchmarkId::new("sequential", num_files),
            &files,
            |b, files| {
                b.iter(|| {
                    let mut hashes = HashMap::new();
                    for (path, content) in files {
                        let hash = core::calculate_file_hash(content);
                        hashes.insert(path.clone(), hash);
                    }
                    hashes
                })
            },
        );

        // Parallel hash calculation
        group.bench_with_input(
            BenchmarkId::new("parallel", num_files),
            &file_refs,
            |b, files| b.iter(|| ParallelFileAnalyzer::calculate_hashes_parallel(files.clone())),
        );
    }

    group.finish();
}

criterion_group!(
    parallel_benches,
    benchmark_symbol_addition,
    benchmark_symbol_search,
    benchmark_file_updates,
    benchmark_dead_code_detection,
    benchmark_lsif_generation,
    benchmark_file_hash_calculation
);
criterion_main!(parallel_benches);
