use cli::storage::IndexStorage;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lsif_core::call_hierarchy::CallHierarchyAnalyzer;
use lsif_core::incremental::{FileUpdate, UpdateResult};
use lsif_core::lsif::{generate_lsif, parse_lsif};
use lsif_core::{CodeGraph, EdgeKind, IncrementalIndex, Position, Range, Symbol, SymbolKind};
use std::collections::HashMap;

fn create_small_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();

    for i in 0..10 {
        let symbol = Symbol {
            id: format!("symbol_{i}"),
            name: format!("function_{i}"),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position {
                    line: i * 10,
                    character: 0,
                },
                end: Position {
                    line: i * 10 + 5,
                    character: 0,
                },
            },
            documentation: Some(format!("Documentation for function_{i}")),
            detail: None,
        };
        graph.add_symbol(symbol);
    }

    // Add some edges
    if let (Some(idx0), Some(idx1)) = (
        graph.get_node_index("symbol_0"),
        graph.get_node_index("symbol_1"),
    ) {
        graph.add_edge(idx0, idx1, EdgeKind::Reference);
    }
    if let (Some(idx1), Some(idx2)) = (
        graph.get_node_index("symbol_1"),
        graph.get_node_index("symbol_2"),
    ) {
        graph.add_edge(idx1, idx2, EdgeKind::Reference);
    }

    graph
}

fn create_medium_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();

    for i in 0..100 {
        let symbol = Symbol {
            id: format!("symbol_{i}"),
            name: format!("function_{i}"),
            kind: if i % 3 == 0 {
                SymbolKind::Function
            } else if i % 3 == 1 {
                SymbolKind::Class
            } else {
                SymbolKind::Variable
            },
            file_path: format!("file_{}.rs", i / 10),
            range: Range {
                start: Position {
                    line: i * 10,
                    character: 0,
                },
                end: Position {
                    line: i * 10 + 5,
                    character: 0,
                },
            },
            documentation: Some(format!("Documentation for symbol_{i}")),
            detail: None,
        };
        graph.add_symbol(symbol);
    }

    // Add edges to create a more complex graph
    for i in 0..99 {
        if let (Some(idx_from), Some(idx_to)) = (
            graph.get_node_index(&format!("symbol_{i}")),
            graph.get_node_index(&format!("symbol_{}", i + 1)),
        ) {
            graph.add_edge(idx_from, idx_to, EdgeKind::Reference);
        }

        // Add some cross-references
        if i % 5 == 0 && i + 5 < 100 {
            if let (Some(idx_from), Some(idx_to)) = (
                graph.get_node_index(&format!("symbol_{i}")),
                graph.get_node_index(&format!("symbol_{}", i + 5)),
            ) {
                graph.add_edge(idx_from, idx_to, EdgeKind::Definition);
            }
        }
    }

    graph
}

fn create_large_graph() -> CodeGraph {
    let mut graph = CodeGraph::new();
    let mut indices = HashMap::new();

    // Create 1000 symbols
    for i in 0..1000 {
        let symbol = Symbol {
            id: format!("symbol_{i}"),
            name: format!("entity_{i}"),
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
                    line: (i % 100) * 10,
                    character: 0,
                },
                end: Position {
                    line: (i % 100) * 10 + 5,
                    character: 0,
                },
            },
            documentation: if i % 2 == 0 {
                Some(format!(
                    "Detailed documentation for entity_{i} with various information"
                ))
            } else {
                None
            },
            detail: None,
        };
        let idx = graph.add_symbol(symbol);
        indices.insert(format!("symbol_{i}"), idx);
    }

    // Create a complex web of edges
    for i in 0..1000 {
        // Sequential references
        if i < 999 {
            graph.add_edge(
                indices[&format!("symbol_{i}")],
                indices[&format!("symbol_{}", i + 1)],
                EdgeKind::Reference,
            );
        }

        // Cross-module references
        if i % 10 == 0 && i + 100 < 1000 {
            graph.add_edge(
                indices[&format!("symbol_{i}")],
                indices[&format!("symbol_{}", i + 100)],
                EdgeKind::Definition,
            );
        }

        // Cyclic references
        if i > 0 && i % 50 == 0 {
            graph.add_edge(
                indices[&format!("symbol_{i}")],
                indices[&format!("symbol_{}", i - 50)],
                EdgeKind::Reference,
            );
        }
    }

    graph
}

fn benchmark_graph_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_construction");

    group.bench_function("small_graph", |b| b.iter(create_small_graph));

    group.bench_function("medium_graph", |b| b.iter(create_medium_graph));

    group.bench_function("large_graph", |b| b.iter(create_large_graph));

    group.finish();
}

fn benchmark_symbol_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("symbol_operations");

    let small_graph = create_small_graph();
    let medium_graph = create_medium_graph();
    let large_graph = create_large_graph();

    group.bench_function("find_symbol_small", |b| {
        b.iter(|| small_graph.find_symbol(black_box("symbol_5")))
    });

    group.bench_function("find_symbol_medium", |b| {
        b.iter(|| medium_graph.find_symbol(black_box("symbol_50")))
    });

    group.bench_function("find_symbol_large", |b| {
        b.iter(|| large_graph.find_symbol(black_box("symbol_500")))
    });

    group.bench_function("find_references_medium", |b| {
        b.iter(|| medium_graph.find_references(black_box("symbol_50")))
    });

    group.bench_function("find_references_large", |b| {
        b.iter(|| large_graph.find_references(black_box("symbol_500")))
    });

    group.finish();
}

fn benchmark_lsif_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsif_operations");

    let small_graph = create_small_graph();
    let medium_graph = create_medium_graph();
    let large_graph = create_large_graph();

    group.bench_function("generate_lsif_small", |b| {
        b.iter(|| generate_lsif(black_box(small_graph.clone())).unwrap())
    });

    group.bench_function("generate_lsif_medium", |b| {
        b.iter(|| generate_lsif(black_box(medium_graph.clone())).unwrap())
    });

    group.bench_function("generate_lsif_large", |b| {
        b.iter(|| generate_lsif(black_box(large_graph.clone())).unwrap())
    });

    // Benchmark parsing
    let small_lsif = generate_lsif(small_graph.clone()).unwrap();
    let medium_lsif = generate_lsif(medium_graph.clone()).unwrap();
    let large_lsif = generate_lsif(large_graph.clone()).unwrap();

    group.bench_function("parse_lsif_small", |b| {
        b.iter(|| parse_lsif(black_box(&small_lsif)).unwrap())
    });

    group.bench_function("parse_lsif_medium", |b| {
        b.iter(|| parse_lsif(black_box(&medium_lsif)).unwrap())
    });

    group.bench_function("parse_lsif_large", |b| {
        b.iter(|| parse_lsif(black_box(&large_lsif)).unwrap())
    });

    group.finish();
}

fn benchmark_call_hierarchy(c: &mut Criterion) {
    let mut group = c.benchmark_group("call_hierarchy");

    let medium_graph = create_medium_graph();
    let large_graph = create_large_graph();

    group.bench_function("outgoing_calls_medium", |b| {
        let analyzer = CallHierarchyAnalyzer::new(&medium_graph);
        b.iter(|| analyzer.get_outgoing_calls(black_box("symbol_0"), black_box(3)))
    });

    group.bench_function("outgoing_calls_large", |b| {
        let analyzer = CallHierarchyAnalyzer::new(&large_graph);
        b.iter(|| analyzer.get_outgoing_calls(black_box("symbol_0"), black_box(3)))
    });

    group.bench_function("incoming_calls_medium", |b| {
        let analyzer = CallHierarchyAnalyzer::new(&medium_graph);
        b.iter(|| analyzer.get_incoming_calls(black_box("symbol_50"), black_box(2)))
    });

    group.bench_function("incoming_calls_large", |b| {
        let analyzer = CallHierarchyAnalyzer::new(&large_graph);
        b.iter(|| analyzer.get_incoming_calls(black_box("symbol_500"), black_box(2)))
    });

    group.bench_function("find_paths_medium", |b| {
        let analyzer = CallHierarchyAnalyzer::new(&medium_graph);
        b.iter(|| {
            analyzer.find_call_paths(black_box("symbol_0"), black_box("symbol_10"), black_box(5))
        })
    });

    group.bench_function("find_paths_large", |b| {
        let analyzer = CallHierarchyAnalyzer::new(&large_graph);
        b.iter(|| {
            analyzer.find_call_paths(black_box("symbol_0"), black_box("symbol_100"), black_box(5))
        })
    });

    group.finish();
}

fn benchmark_edge_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("edge_operations");

    group.bench_function("add_edges_sequential", |b| {
        b.iter(|| {
            let mut graph = CodeGraph::new();
            let mut indices = Vec::new();

            // Add symbols first
            for i in 0..100 {
                let symbol = Symbol {
                    id: format!("s{i}"),
                    name: format!("symbol_{i}"),
                    kind: SymbolKind::Function,
                    file_path: "test.rs".to_string(),
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
                    detail: None,
                };
                indices.push(graph.add_symbol(symbol));
            }

            // Add edges
            for i in 0..99 {
                graph.add_edge(indices[i], indices[i + 1], EdgeKind::Reference);
            }

            graph
        })
    });

    group.bench_function("add_edges_complex", |b| {
        b.iter(|| {
            let mut graph = CodeGraph::new();
            let mut indices = Vec::new();

            // Add symbols
            for i in 0..100 {
                let symbol = Symbol {
                    id: format!("s{i}"),
                    name: format!("symbol_{i}"),
                    kind: SymbolKind::Function,
                    file_path: "test.rs".to_string(),
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
                    detail: None,
                };
                indices.push(graph.add_symbol(symbol));
            }

            // Add complex edge patterns
            for i in 0..100 {
                for j in 0..5 {
                    let target = (i + j * 20) % 100;
                    if i != target {
                        graph.add_edge(indices[i], indices[target], EdgeKind::Reference);
                    }
                }
            }

            graph
        })
    });

    group.finish();
}

fn benchmark_incremental_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_updates");

    // Setup: Create a large index
    let mut base_index = IncrementalIndex::new();
    let mut all_symbols = Vec::new();

    // Add 1000 symbols across 100 files
    for file_idx in 0..100 {
        let mut file_symbols = Vec::new();
        for sym_idx in 0..10 {
            let symbol = Symbol {
                id: format!("file{file_idx}_sym{sym_idx}"),
                name: format!("function_{file_idx}_{sym_idx}"),
                kind: SymbolKind::Function,
                file_path: format!("src/file_{file_idx}.rs"),
                range: Range {
                    start: Position {
                        line: sym_idx * 10,
                        character: 0,
                    },
                    end: Position {
                        line: sym_idx * 10 + 5,
                        character: 0,
                    },
                },
                documentation: Some(format!("Doc for function_{file_idx}_{sym_idx}")),
                detail: None,
            };
            file_symbols.push(symbol.clone());
            all_symbols.push(symbol);
        }
        base_index
            .update_file(
                std::path::Path::new(&format!("src/file_{file_idx}.rs")),
                file_symbols,
                format!("hash_{file_idx}"),
            )
            .unwrap();
    }

    // Benchmark small update (1 file, 2 symbol changes)
    group.bench_function("small_update", |b| {
        b.iter(|| {
            let mut index = base_index.clone();
            let mut updated_symbols = Vec::new();

            // Modify 2 symbols in one file
            for i in 0..2 {
                updated_symbols.push(Symbol {
                    id: format!("file0_sym{i}"),
                    name: format!("updated_function_0_{i}"),
                    kind: SymbolKind::Function,
                    file_path: "src/file_0.rs".to_string(),
                    range: Range {
                        start: Position {
                            line: i * 10,
                            character: 0,
                        },
                        end: Position {
                            line: i * 10 + 5,
                            character: 0,
                        },
                    },
                    documentation: Some(format!("Updated doc {i}")),
                    detail: None,
                });
            }

            index.update_file(
                std::path::Path::new("src/file_0.rs"),
                updated_symbols,
                "new_hash_0".to_string(),
            )
        })
    });

    // Benchmark medium update (10 files, 50 symbol changes)
    group.bench_function("medium_update", |b| {
        b.iter(|| {
            let mut index = base_index.clone();
            let mut updates = Vec::new();

            for file_idx in 0..10 {
                let mut file_symbols = Vec::new();
                for sym_idx in 0..5 {
                    file_symbols.push(Symbol {
                        id: format!("file{file_idx}_sym{sym_idx}"),
                        name: format!("updated_function_{file_idx}_{sym_idx}"),
                        kind: SymbolKind::Function,
                        file_path: format!("src/file_{file_idx}.rs"),
                        range: Range {
                            start: Position {
                                line: sym_idx * 10,
                                character: 0,
                            },
                            end: Position {
                                line: sym_idx * 10 + 5,
                                character: 0,
                            },
                        },
                        documentation: Some(format!("Updated doc {file_idx}_{sym_idx}")),
                        detail: None,
                    });
                }
                updates.push(FileUpdate::Modified {
                    path: std::path::PathBuf::from(format!("src/file_{file_idx}.rs")),
                    symbols: file_symbols,
                    hash: format!("new_hash_{file_idx}"),
                });
            }

            index.batch_update(updates)
        })
    });

    // Benchmark full rebuild vs incremental
    group.bench_function("full_rebuild", |b| {
        b.iter(|| {
            let mut new_index = IncrementalIndex::new();
            for symbol in &all_symbols {
                new_index.add_symbol(black_box(symbol.clone())).unwrap();
            }
            new_index
        })
    });

    // Benchmark dead code detection
    group.bench_function("dead_code_detection", |b| {
        let mut index_with_dead = base_index.clone();

        // Add some unreferenced symbols
        for i in 0..20 {
            index_with_dead
                .add_symbol(Symbol {
                    id: format!("dead_sym_{i}"),
                    name: format!("unused_function_{i}"),
                    kind: SymbolKind::Function,
                    file_path: "src/dead.rs".to_string(),
                    range: Range {
                        start: Position {
                            line: i * 10,
                            character: 0,
                        },
                        end: Position {
                            line: i * 10 + 5,
                            character: 0,
                        },
                    },
                    documentation: None,
                    detail: None,
                })
                .unwrap();
        }

        b.iter(|| {
            let mut index = index_with_dead.clone();
            let mut result = UpdateResult::default();
            index.detect_dead_code(&mut result);
            result.dead_symbols.len()
        })
    });

    group.finish();
}

fn benchmark_storage_operations(c: &mut Criterion) {
    use tempfile::tempdir;

    let mut group = c.benchmark_group("storage_operations");

    // Create test data
    let mut graph = CodeGraph::new();
    for i in 0..100 {
        graph.add_symbol(Symbol {
            id: format!("sym_{i}"),
            name: format!("function_{i}"),
            kind: SymbolKind::Function,
            file_path: format!("file_{}.rs", i / 10),
            range: Range {
                start: Position {
                    line: i * 10,
                    character: 0,
                },
                end: Position {
                    line: i * 10 + 5,
                    character: 0,
                },
            },
            documentation: Some(format!("Doc {i}")),
            detail: None,
        });
    }

    let dir = tempdir().unwrap();
    let db_path = dir.path();

    // Benchmark full save
    group.bench_function("storage_full_save", |b| {
        let storage = IndexStorage::open(db_path).unwrap();
        b.iter(|| storage.save_data("graph", black_box(&graph)).unwrap())
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_graph_construction,
    benchmark_symbol_operations,
    benchmark_lsif_operations,
    benchmark_call_hierarchy,
    benchmark_edge_operations,
    benchmark_incremental_updates,
    benchmark_storage_operations
);
criterion_main!(benches);
