use cli::storage::{IndexFormat, IndexMetadata, IndexStorage};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use lsif_core::graph::{CodeGraph, Position, Range, Symbol, SymbolKind};
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
            detail: None,
        })
        .collect()
}

fn generate_test_graph(symbol_count: usize) -> CodeGraph {
    let mut graph = CodeGraph::new();
    let symbols = generate_test_symbols(symbol_count);

    for symbol in symbols {
        graph.add_symbol(symbol);
    }

    graph
}

fn benchmark_symbol_save(c: &mut Criterion) {
    let mut group = c.benchmark_group("symbol_storage");

    for size in [100, 1000, 10000].iter() {
        // Individual symbol saving
        group.bench_with_input(
            BenchmarkId::new("save_individual_symbols", size),
            size,
            |b, &symbol_count| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = IndexStorage::open(temp_dir.path()).unwrap();
                        let symbols = generate_test_symbols(symbol_count);
                        (storage, symbols, temp_dir)
                    },
                    |(storage, symbols, _temp_dir)| {
                        for symbol in &symbols {
                            storage.save_data(&symbol.id, symbol).unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("save_graph", size),
            size,
            |b, &symbol_count| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = IndexStorage::open(temp_dir.path()).unwrap();
                        let graph = generate_test_graph(symbol_count);
                        (storage, graph, temp_dir)
                    },
                    |(storage, graph, _temp_dir)| {
                        storage.save_data("code_graph", &graph).unwrap();
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("save_batch_symbols", size),
            size,
            |b, &symbol_count| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let storage = IndexStorage::open(temp_dir.path()).unwrap();
                        let symbols = generate_test_symbols(symbol_count);
                        (storage, symbols, temp_dir)
                    },
                    |(storage, symbols, _temp_dir)| {
                        storage.save_data("all_symbols", &symbols).unwrap();
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn benchmark_symbol_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("symbol_loading");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("load_individual_symbols", size),
            size,
            |b, &symbol_count| {
                let temp_dir = TempDir::new().unwrap();
                let storage = IndexStorage::open(temp_dir.path()).unwrap();
                let symbols = generate_test_symbols(symbol_count);

                for symbol in &symbols {
                    storage.save_data(&symbol.id, symbol).unwrap();
                }

                b.iter(|| {
                    for i in 0..symbol_count {
                        let _: Option<Symbol> = storage.load_data(&format!("symbol_{i}")).unwrap();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("load_graph", size),
            size,
            |b, &symbol_count| {
                let temp_dir = TempDir::new().unwrap();
                let storage = IndexStorage::open(temp_dir.path()).unwrap();
                let graph = generate_test_graph(symbol_count);
                storage.save_data("code_graph", &graph).unwrap();

                b.iter(|| {
                    let _: Option<CodeGraph> = storage.load_data("code_graph").unwrap();
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("load_batch_symbols", size),
            size,
            |b, &symbol_count| {
                let temp_dir = TempDir::new().unwrap();
                let storage = IndexStorage::open(temp_dir.path()).unwrap();
                let symbols = generate_test_symbols(symbol_count);
                storage.save_data("all_symbols", &symbols).unwrap();

                b.iter(|| {
                    let _: Option<Vec<Symbol>> = storage.load_data("all_symbols").unwrap();
                });
            },
        );
    }

    group.finish();
}

fn benchmark_metadata_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("metadata_operations");

    group.bench_function("save_metadata", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let storage = IndexStorage::open(temp_dir.path()).unwrap();
                let metadata = IndexMetadata {
                    format: IndexFormat::Lsif,
                    version: "1.0.0".to_string(),
                    created_at: chrono::Utc::now(),
                    project_root: "/test/project".to_string(),
                    files_count: 1000,
                    symbols_count: 10000,
                    git_commit_hash: None,
                    file_hashes: std::collections::HashMap::new(),
                };
                (storage, metadata, temp_dir)
            },
            |(storage, metadata, _temp_dir)| {
                storage.save_metadata(&metadata).unwrap();
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("load_metadata", |b| {
        let temp_dir = TempDir::new().unwrap();
        let storage = IndexStorage::open(temp_dir.path()).unwrap();
        let metadata = IndexMetadata {
            format: IndexFormat::Lsif,
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now(),
            project_root: "/test/project".to_string(),
            files_count: 1000,
            symbols_count: 10000,
            git_commit_hash: None,
            file_hashes: std::collections::HashMap::new(),
        };
        storage.save_metadata(&metadata).unwrap();

        b.iter(|| {
            let _ = black_box(storage.load_metadata().unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_symbol_save,
    benchmark_symbol_load,
    benchmark_metadata_operations
);
criterion_main!(benches);
