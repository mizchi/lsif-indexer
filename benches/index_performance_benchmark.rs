use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
// use lsif_core::parallel_optimized::{OptimizedParallelGraph, OptimizedParallelIndex};
use lsif_core::incremental::FileUpdate;
use lsif_core::{CodeGraph, IncrementalIndex, Position, Range, Symbol, SymbolKind};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tempfile::TempDir;

/// 実際のRustプロジェクトをシミュレートしたテストデータを生成
fn generate_test_project(base_dir: &Path, num_files: usize, symbols_per_file: usize) {
    for file_idx in 0..num_files {
        let module_dir = base_dir.join(format!("module_{}", file_idx / 10));
        fs::create_dir_all(&module_dir).unwrap();

        let file_path = module_dir.join(format!("file_{file_idx}.rs"));
        let mut content = String::new();

        // ファイル内容を生成
        content.push_str(&format!(
            "// Module {} File {}\n\n",
            file_idx / 10,
            file_idx
        ));

        for sym_idx in 0..symbols_per_file {
            if sym_idx % 3 == 0 {
                content.push_str(&format!(
                    "pub fn function_{file_idx}_{sym_idx} (x: i32) -> i32 {{\n    x + {sym_idx}\n}}\n\n"
                ));
            } else if sym_idx % 3 == 1 {
                content.push_str(&format!(
                    "pub struct Struct{file_idx}_{sym_idx} {{\n    field: i32,\n}}\n\n"
                ));
            } else {
                content.push_str(&format!(
                    "pub const CONST_{}_{}:i32 = {};\n\n",
                    file_idx,
                    sym_idx,
                    sym_idx * 100
                ));
            }
        }

        fs::write(file_path, content).unwrap();
    }
}

/// ファイルからシンボルを抽出（実際のパーサーをシミュレート）
fn extract_symbols_from_file(file_path: &Path) -> Vec<Symbol> {
    let content = fs::read_to_string(file_path).unwrap_or_default();
    let file_path_str = file_path.to_string_lossy().to_string();
    let mut symbols = Vec::new();

    for (line_no, line) in content.lines().enumerate() {
        if line.starts_with("pub fn ") {
            if let Some(name_start) = line.find("fn ").map(|i| i + 3) {
                if let Some(name_end) = line[name_start..].find('(').map(|i| i + name_start) {
                    let name = &line[name_start..name_end].trim();
                    symbols.push(Symbol {
                        id: format!("{file_path_str}:{name}"),
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        file_path: file_path_str.clone(),
                        range: Range {
                            start: Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: Position {
                                line: line_no as u32 + 2,
                                character: 0,
                            },
                        },
                        documentation: Some(format!("Function {name}")),
                        detail: None,
                    });
                }
            }
        } else if line.starts_with("pub struct ") {
            if let Some(name_start) = line.find("struct ").map(|i| i + 7) {
                if let Some(name_end) = line[name_start..].find(' ').map(|i| i + name_start) {
                    let name = &line[name_start..name_end].trim();
                    symbols.push(Symbol {
                        id: format!("{file_path_str}:{name}"),
                        name: name.to_string(),
                        kind: SymbolKind::Class,
                        file_path: file_path_str.clone(),
                        range: Range {
                            start: Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: Position {
                                line: line_no as u32 + 2,
                                character: 0,
                            },
                        },
                        documentation: Some(format!("Struct {name}")),
                        detail: None,
                    });
                }
            }
        } else if line.starts_with("pub const ") {
            if let Some(name_start) = line.find("const ").map(|i| i + 6) {
                if let Some(name_end) = line[name_start..].find(':').map(|i| i + name_start) {
                    let name = &line[name_start..name_end].trim();
                    symbols.push(Symbol {
                        id: format!("{file_path_str}:{name}"),
                        name: name.to_string(),
                        kind: SymbolKind::Constant,
                        file_path: file_path_str.clone(),
                        range: Range {
                            start: Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: Position {
                                line: line_no as u32,
                                character: line.len() as u32,
                            },
                        },
                        documentation: Some(format!("Constant {name}")),
                        detail: None,
                    });
                }
            }
        }
    }

    symbols
}

/// ディレクトリ内のすべてのRustファイルを取得
fn collect_rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_rust_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }

    files
}

fn benchmark_initial_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("initial_indexing");
    group.sample_size(10); // 初回インデックスは時間がかかるのでサンプル数を減らす

    // テストケース: 小規模、中規模、大規模プロジェクト
    let test_cases = vec![
        ("small_project", 10, 20),   // 10ファイル、各20シンボル = 200シンボル
        ("medium_project", 100, 30), // 100ファイル、各30シンボル = 3,000シンボル
        ("large_project", 500, 40),  // 500ファイル、各40シンボル = 20,000シンボル
    ];

    for (name, num_files, symbols_per_file) in test_cases {
        // 逐次版
        group.bench_function(BenchmarkId::new("sequential", name), |b| {
            b.iter_batched_ref(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    generate_test_project(temp_dir.path(), num_files, symbols_per_file);
                    temp_dir
                },
                |temp_dir| {
                    let start = Instant::now();
                    let mut graph = CodeGraph::new();
                    let files = collect_rust_files(temp_dir.path());

                    for file in &files {
                        let symbols = extract_symbols_from_file(file);
                        for symbol in symbols {
                            graph.add_symbol(symbol);
                        }
                    }

                    let elapsed = start.elapsed();
                    (graph.symbol_count(), elapsed)
                },
                BatchSize::LargeInput,
            )
        });

        // 最適化並列版
        group.bench_function(BenchmarkId::new("parallel_optimized", name), |b| {
            b.iter_batched_ref(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    generate_test_project(temp_dir.path(), num_files, symbols_per_file);
                    temp_dir
                },
                |temp_dir| {
                    let start = Instant::now();
                    let mut graph = CodeGraph::new();
                    let files = collect_rust_files(temp_dir.path());

                    // ファイルを並列で処理してシンボルを抽出
                    use rayon::prelude::*;
                    let all_symbols: Vec<Symbol> = files
                        .par_iter()
                        .flat_map(|file| extract_symbols_from_file(file))
                        .collect();

                    // バッチでシンボルを追加
                    // OptimizedParallelGraph::add_symbols_batch(&mut graph, all_symbols);
                    for symbol in all_symbols {
                        graph.add_symbol(symbol);
                    }

                    let elapsed = start.elapsed();
                    (graph.symbol_count(), elapsed)
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

fn benchmark_incremental_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_indexing");

    // 中規模プロジェクトでのインクリメンタル更新をテスト
    let num_files = 100;
    let symbols_per_file = 30;
    let files_to_update = 10; // 10%のファイルを更新

    group.bench_function("sequential_update", |b| {
        b.iter_batched_ref(
            || {
                let temp_dir = TempDir::new().unwrap();
                generate_test_project(temp_dir.path(), num_files, symbols_per_file);

                // 初期インデックスを作成
                let mut index = IncrementalIndex::new();
                let files = collect_rust_files(temp_dir.path());

                for file in &files {
                    let symbols = extract_symbols_from_file(file);
                    let hash = format!("hash_{}", file.to_string_lossy());
                    index.update_file(file, symbols, hash).unwrap();
                }

                (temp_dir, index, files)
            },
            |(_temp_dir, index, files)| {
                let start = Instant::now();

                // 一部のファイルを更新
                for (i, file) in files.iter().take(files_to_update).enumerate() {
                    // ファイルにシンボルを追加
                    let mut content = fs::read_to_string(file).unwrap();
                    content.push_str(&format!("\npub fn new_function_{i}() {{}}\n"));
                    fs::write(file, content).unwrap();

                    let symbols = extract_symbols_from_file(file);
                    let hash = format!("new_hash_{}", file.to_string_lossy());
                    index.update_file(file, symbols, hash).unwrap();
                }

                start.elapsed()
            },
            BatchSize::LargeInput,
        )
    });

    group.bench_function("parallel_optimized_update", |b| {
        b.iter_batched_ref(
            || {
                let temp_dir = TempDir::new().unwrap();
                generate_test_project(temp_dir.path(), num_files, symbols_per_file);

                // 初期インデックスを作成
                let index = IncrementalIndex::new();
                let files = collect_rust_files(temp_dir.path());

                // let parallel_index = OptimizedParallelIndex::from_index(index);

                // 初期インデックス作成（並列）
                use rayon::prelude::*;
                let _file_updates: Vec<_> = files
                    .par_iter()
                    .map(|file| {
                        let symbols = extract_symbols_from_file(file);
                        let hash = format!("hash_{}", file.to_string_lossy());
                        FileUpdate::Added {
                            path: file.clone(),
                            symbols,
                            hash,
                        }
                    })
                    .collect();

                // parallel_index.batch_update_files(file_updates).unwrap();

                (temp_dir, index, files)
            },
            |(_temp_dir, index, files)| {
                let start = Instant::now();

                // 更新するファイルを準備
                use rayon::prelude::*;
                let file_updates: Vec<_> = (0..files_to_update)
                    .into_par_iter()
                    .map(|i| {
                        let file = &files[i];
                        // ファイルにシンボルを追加
                        let mut content = fs::read_to_string(file).unwrap();
                        content.push_str(&format!("\npub fn new_function_{i}() {{}}\n"));
                        fs::write(file, content).unwrap();

                        let symbols = extract_symbols_from_file(file);
                        let hash = format!("new_hash_{}", file.to_string_lossy());

                        FileUpdate::Modified {
                            path: file.clone(),
                            symbols,
                            hash,
                        }
                    })
                    .collect();

                // parallel_index.batch_update_files(file_updates).unwrap();
                index.batch_update(file_updates).unwrap();

                start.elapsed()
            },
            BatchSize::LargeInput,
        )
    });

    group.finish();
}

/// 実際のファイルI/Oを含む総合的なベンチマーク
fn benchmark_end_to_end_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end_indexing");
    group.sample_size(10);

    // 実プロジェクトサイズ: 1000ファイル、平均50シンボル/ファイル = 50,000シンボル
    group.bench_function("sequential_full_project", |b| {
        b.iter_batched_ref(
            || {
                let temp_dir = TempDir::new().unwrap();
                generate_test_project(temp_dir.path(), 1000, 50);
                temp_dir
            },
            |temp_dir| {
                let start = Instant::now();

                let mut graph = CodeGraph::new();
                let mut file_count = 0;
                let mut symbol_count = 0;

                let files = collect_rust_files(temp_dir.path());
                for file in &files {
                    let symbols = extract_symbols_from_file(file);
                    symbol_count += symbols.len();
                    for symbol in symbols {
                        graph.add_symbol(symbol);
                    }
                    file_count += 1;
                }

                let elapsed = start.elapsed();
                let throughput = symbol_count as f64 / elapsed.as_secs_f64();

                (file_count, symbol_count, elapsed, throughput)
            },
            BatchSize::LargeInput,
        )
    });

    group.bench_function("parallel_optimized_full_project", |b| {
        b.iter_batched_ref(
            || {
                let temp_dir = TempDir::new().unwrap();
                generate_test_project(temp_dir.path(), 1000, 50);
                temp_dir
            },
            |temp_dir| {
                let start = Instant::now();

                let mut graph = CodeGraph::new();
                let files = collect_rust_files(temp_dir.path());
                let file_count = files.len();

                // 並列処理
                use rayon::prelude::*;
                let all_symbols: Vec<Symbol> = files
                    .par_iter()
                    .flat_map(|file| extract_symbols_from_file(file))
                    .collect();

                let symbol_count = all_symbols.len();
                // OptimizedParallelGraph::add_symbols_batch(&mut graph, all_symbols);
                for symbol in all_symbols {
                    graph.add_symbol(symbol);
                }

                let elapsed = start.elapsed();
                let throughput = symbol_count as f64 / elapsed.as_secs_f64();

                (file_count, symbol_count, elapsed, throughput)
            },
            BatchSize::LargeInput,
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_initial_indexing,
    benchmark_incremental_indexing,
    benchmark_end_to_end_performance
);
criterion_main!(benches);
