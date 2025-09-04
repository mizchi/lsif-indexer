use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use lsp::adapter::go::GoAdapter;
use lsp::adapter::language::LanguageAdapter;
use lsp::adapter::python::PythonAdapter;
use lsp::adapter::typescript::TypeScriptAdapter;
use lsp::lsp_client::LspClient;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// 言語別のLSPインデックス性能測定
fn benchmark_language_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("language_indexing");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(10);

    // Go言語のベンチマーク
    let go_project = PathBuf::from("test-go-project");
    if go_project.exists() {
        group.bench_function("go_symbols_extraction", |b| {
            b.iter(|| {
                let adapter = Box::new(GoAdapter);
                extract_symbols_from_project(adapter, &go_project)
            });
        });
    }

    // Python言語のベンチマーク
    let python_project = PathBuf::from("test-python-project");
    if python_project.exists() {
        group.bench_function("python_symbols_extraction", |b| {
            b.iter(|| {
                let adapter = Box::new(PythonAdapter::new());
                extract_symbols_from_project(adapter, &python_project)
            });
        });
    }

    // TypeScript言語のベンチマーク
    let ts_project = PathBuf::from("test-typescript-project");
    if ts_project.exists() {
        group.bench_function("typescript_symbols_extraction", |b| {
            b.iter(|| {
                let adapter = Box::new(TypeScriptAdapter::new());
                extract_symbols_from_project(adapter, &ts_project)
            });
        });
    }

    group.finish();
}

fn benchmark_file_size_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_size_scaling");

    // 異なるサイズのファイルでのパフォーマンス測定
    let sizes = vec![100, 500, 1000, 5000];

    for size in sizes {
        let file_content = generate_test_code(size);
        let file_path = format!("/tmp/test_{}_lines.py", size);
        fs::write(&file_path, &file_content).unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _size| {
            b.iter(|| {
                let adapter = Box::new(PythonAdapter::new());
                if let Ok(mut client) = LspClient::new(adapter) {
                    let project_path = PathBuf::from("/tmp");
                    let _ = client.initialize(&project_path);
                    let file = PathBuf::from(&file_path);
                    let _ = client.get_document_symbols(&file);
                    let _ = client.shutdown();
                }
            });
        });
    }

    group.finish();
}

fn benchmark_concurrent_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_processing");

    // 並列処理のベンチマーク
    let thread_counts = vec![1, 2, 4, 8];

    for threads in thread_counts {
        group.bench_with_input(
            BenchmarkId::from_parameter(threads),
            &threads,
            |b, &thread_count| {
                b.iter(|| {
                    process_files_concurrently(thread_count);
                });
            },
        );
    }

    group.finish();
}

// ヘルパー関数

fn extract_symbols_from_project(
    adapter: Box<dyn LanguageAdapter>,
    project_path: &PathBuf,
) -> usize {
    let mut total_symbols = 0;

    if let Ok(mut client) = LspClient::new(adapter) {
        if client.initialize(project_path).is_ok() {
            // プロジェクト内のファイルを処理
            if let Ok(entries) = fs::read_dir(project_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Ok(symbols) = client.get_document_symbols(&path) {
                            total_symbols += symbols.len();
                        }
                    }
                }
            }
            let _ = client.shutdown();
        }
    }

    black_box(total_symbols)
}

fn generate_test_code(lines: usize) -> String {
    let mut code = String::new();

    // ヘッダー
    code.push_str("# Auto-generated test file\n\n");

    // クラス定義
    for i in 0..lines / 20 {
        code.push_str(&format!("class TestClass{}:\n", i));
        code.push_str("    def __init__(self):\n");
        code.push_str(&format!("        self.value = {}\n\n", i));

        // メソッド
        for j in 0..5 {
            code.push_str(&format!("    def method_{}(self, x):\n", j));
            code.push_str(&format!("        return x * {}\n\n", j + 1));
        }
    }

    // 関数定義
    for i in 0..lines / 10 {
        code.push_str(&format!("def function_{}(a, b):\n", i));
        code.push_str(&format!("    return a + b + {}\n\n", i));
    }

    code
}

fn process_files_concurrently(thread_count: usize) {
    use std::thread;

    let files: Vec<PathBuf> = vec![
        PathBuf::from("test-go-project/main.go"),
        PathBuf::from("test-go-project/utils.go"),
        PathBuf::from("test-python-project/calculator.py"),
        PathBuf::from("test-python-project/utils.py"),
        PathBuf::from("test-typescript-project/calculator.ts"),
        PathBuf::from("test-typescript-project/utils.ts"),
    ];

    let chunk_size = files.len().div_ceil(thread_count);
    let chunks: Vec<_> = files.chunks(chunk_size).map(|c| c.to_vec()).collect();

    let handles: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            thread::spawn(move || {
                for file in chunk {
                    if file.exists() {
                        // ファイルごとに適切なアダプタを選択
                        let adapter: Box<dyn LanguageAdapter> =
                            if file.extension().is_some_and(|e| e == "go") {
                                Box::new(GoAdapter)
                            } else if file.extension().is_some_and(|e| e == "py") {
                                Box::new(PythonAdapter::new())
                            } else {
                                Box::new(TypeScriptAdapter::new())
                            };

                        if let Ok(mut client) = LspClient::new(adapter) {
                            if let Some(parent) = file.parent() {
                                let _ = client.initialize(parent);
                                let _ = client.get_document_symbols(&file);
                                let _ = client.shutdown();
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        let _ = handle.join();
    }
}

criterion_group!(
    benches,
    benchmark_language_indexing,
    benchmark_file_size_scaling,
    benchmark_concurrent_processing
);
criterion_main!(benches);
