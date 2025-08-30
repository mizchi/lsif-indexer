use lsif_indexer::cli::go_adapter::GoAdapter;
use lsif_indexer::cli::lsp_minimal_client::MinimalLspClient;
use lsif_indexer::cli::minimal_language_adapter::MinimalLanguageAdapter;
use lsif_indexer::cli::python_adapter::PythonAdapter;
use lsif_indexer::cli::typescript_adapter::TypeScriptAdapter;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 性能測定用の統合テスト

#[test]
#[ignore] // cargo test -- --ignored performance
fn test_performance_metrics() {
    println!("\n=== LSP Indexing Performance Test ===\n");

    // 各言語のテスト
    measure_language_performance("Go", Box::new(GoAdapter), "test-go-project");
    measure_language_performance(
        "Python",
        Box::new(PythonAdapter::new()),
        "test-python-project",
    );
    measure_language_performance(
        "TypeScript",
        Box::new(TypeScriptAdapter::new()),
        "test-typescript-project",
    );

    println!("\n=== Performance Summary ===\n");
    println!("All tests completed. Check above for detailed metrics.");
}

fn measure_language_performance(
    language: &str,
    adapter: Box<dyn MinimalLanguageAdapter>,
    project_dir: &str,
) {
    println!("\n📊 {} Performance Metrics:", language);
    println!("{}", "=".repeat(50));

    let project_path = PathBuf::from(project_dir);
    if !project_path.exists() {
        println!("  ⚠️  Project not found, skipping");
        return;
    }

    // LSPクライアント作成と初期化の時間測定
    let start = Instant::now();
    let mut client = match MinimalLspClient::new(adapter) {
        Ok(c) => c,
        Err(e) => {
            println!("  ❌ Failed to create client: {}", e);
            return;
        }
    };
    let client_creation_time = start.elapsed();

    // 初期化時間の測定
    let start = Instant::now();
    if let Err(e) = client.initialize(&project_path) {
        println!("  ❌ Failed to initialize: {}", e);
        return;
    }
    let init_time = start.elapsed();

    // ファイル一覧の取得
    let mut source_files = Vec::new();
    if let Ok(entries) = fs::read_dir(&project_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_source_file(&path, language) {
                source_files.push(path);
            }
        }
    }

    // 各ファイルのシンボル抽出時間を測定
    let mut total_symbols = 0;
    let mut total_extraction_time = std::time::Duration::ZERO;
    let mut file_metrics = Vec::new();

    for file_path in &source_files {
        let file_name = file_path.file_name().unwrap().to_string_lossy();
        let file_size = fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);

        let start = Instant::now();
        match client.get_document_symbols(file_path) {
            Ok(symbols) => {
                let extraction_time = start.elapsed();
                total_extraction_time += extraction_time;
                total_symbols += symbols.len();

                file_metrics.push((
                    file_name.to_string(),
                    file_size,
                    symbols.len(),
                    extraction_time,
                ));
            }
            Err(e) => {
                println!("  ⚠️  Failed to extract symbols from {}: {}", file_name, e);
            }
        }
    }

    // シャットダウン時間の測定
    let start = Instant::now();
    let _ = client.shutdown();
    let shutdown_time = start.elapsed();

    // メトリクスの表示
    println!("\n  📈 Timing Metrics:");
    println!("    • Client creation:  {:>8.2?}", client_creation_time);
    println!("    • Initialization:   {:>8.2?}", init_time);
    println!(
        "    • Symbol extraction:{:>8.2?} (total)",
        total_extraction_time
    );
    println!("    • Shutdown:         {:>8.2?}", shutdown_time);
    println!(
        "    • Total time:       {:>8.2?}",
        client_creation_time + init_time + total_extraction_time + shutdown_time
    );

    println!("\n  📁 File Metrics:");
    for (name, size, symbols, time) in &file_metrics {
        let symbols_per_sec = if time.as_secs_f64() > 0.0 {
            *symbols as f64 / time.as_secs_f64()
        } else {
            0.0
        };
        println!(
            "    • {:<20} {:>6} bytes, {:>3} symbols, {:>6.2?} ({:.0} sym/s)",
            name, size, symbols, time, symbols_per_sec
        );
    }

    // 統計サマリー
    let avg_extraction_time = if !source_files.is_empty() {
        total_extraction_time / source_files.len() as u32
    } else {
        std::time::Duration::ZERO
    };

    let total_bytes: u64 = file_metrics.iter().map(|(_, size, _, _)| size).sum();
    let bytes_per_sec = if total_extraction_time.as_secs_f64() > 0.0 {
        total_bytes as f64 / total_extraction_time.as_secs_f64()
    } else {
        0.0
    };

    println!("\n  📊 Statistics:");
    println!("    • Files processed:    {}", source_files.len());
    println!("    • Total symbols:      {}", total_symbols);
    println!(
        "    • Avg symbols/file:   {:.1}",
        total_symbols as f64 / source_files.len().max(1) as f64
    );
    println!("    • Avg time/file:      {:>6.2?}", avg_extraction_time);
    println!(
        "    • Throughput:         {:.2} KB/s",
        bytes_per_sec / 1024.0
    );

    // パフォーマンス評価
    let total_time = client_creation_time + init_time + total_extraction_time + shutdown_time;
    let rating = if total_time.as_millis() < 500 {
        "⚡ Excellent"
    } else if total_time.as_millis() < 1000 {
        "✅ Good"
    } else if total_time.as_millis() < 2000 {
        "🔶 Acceptable"
    } else {
        "⚠️  Needs optimization"
    };

    println!("\n  🎯 Performance Rating: {}", rating);
}

fn is_source_file(path: &Path, language: &str) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match language {
        "Go" => ext == "go",
        "Python" => ext == "py" || ext == "pyi",
        "TypeScript" => ext == "ts" || ext == "tsx",
        "JavaScript" => ext == "js" || ext == "jsx",
        _ => false,
    }
}

#[test]
#[ignore]
fn test_memory_usage() {
    use std::process::Command;

    println!("\n=== Memory Usage Test ===\n");

    // 現在のプロセスIDを取得
    let pid = std::process::id();

    // メモリ使用量を測定する関数
    let get_memory_usage = || -> Option<u64> {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;

        String::from_utf8(output.stdout)
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()
    };

    // 初期メモリ使用量
    let initial_memory = get_memory_usage().unwrap_or(0);
    println!("Initial memory: {} KB", initial_memory);

    // 各言語でのメモリ使用量を測定
    let languages = vec![
        (
            "Go",
            Box::new(GoAdapter) as Box<dyn MinimalLanguageAdapter>,
            "test-go-project",
        ),
        (
            "Python",
            Box::new(PythonAdapter::new()),
            "test-python-project",
        ),
        (
            "TypeScript",
            Box::new(TypeScriptAdapter::new()),
            "test-typescript-project",
        ),
    ];

    for (lang, adapter, project_dir) in languages {
        let project_path = PathBuf::from(project_dir);
        if !project_path.exists() {
            continue;
        }

        // LSP操作を実行
        if let Ok(mut client) = MinimalLspClient::new(adapter) {
            let _ = client.initialize(&project_path);

            // すべてのファイルを処理
            if let Ok(entries) = fs::read_dir(&project_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let _ = client.get_document_symbols(&path);
                    }
                }
            }

            // メモリ使用量を測定
            let current_memory = get_memory_usage().unwrap_or(0);
            let memory_increase = current_memory.saturating_sub(initial_memory);

            println!("{} LSP operations:", lang);
            println!("  Current memory: {} KB", current_memory);
            println!("  Memory increase: {} KB", memory_increase);

            let _ = client.shutdown();
        }
    }

    // 最終メモリ使用量
    let final_memory = get_memory_usage().unwrap_or(0);
    let total_increase = final_memory.saturating_sub(initial_memory);

    println!("\nFinal memory: {} KB", final_memory);
    println!("Total memory increase: {} KB", total_increase);

    // メモリ使用量の評価
    let rating = if total_increase < 10_000 {
        "⚡ Excellent (< 10 MB)"
    } else if total_increase < 50_000 {
        "✅ Good (< 50 MB)"
    } else if total_increase < 100_000 {
        "🔶 Acceptable (< 100 MB)"
    } else {
        "⚠️  High memory usage"
    };

    println!("\n🎯 Memory Usage Rating: {}", rating);
}

#[test]
#[ignore]
fn test_scalability() {
    println!("\n=== Scalability Test ===\n");

    // 異なるサイズのプロジェクトでテスト
    let test_sizes = vec![
        ("Small", 10, 5),   // 10 files, 5 symbols each
        ("Medium", 50, 10), // 50 files, 10 symbols each
        ("Large", 100, 20), // 100 files, 20 symbols each
    ];

    for (size_name, file_count, symbols_per_file) in test_sizes {
        println!("Testing {} project ({} files):", size_name, file_count);

        // テスト用のPythonファイルを生成
        let temp_dir = PathBuf::from(format!("/tmp/scale_test_{}", size_name.to_lowercase()));
        fs::create_dir_all(&temp_dir).unwrap();

        for i in 0..file_count {
            let content = generate_python_file(symbols_per_file);
            let file_path = temp_dir.join(format!("test_{}.py", i));
            fs::write(&file_path, content).unwrap();
        }

        // パフォーマンス測定
        let adapter = Box::new(PythonAdapter::new());
        let start = Instant::now();

        if let Ok(mut client) = MinimalLspClient::new(adapter) {
            if client.initialize(&temp_dir).is_ok() {
                let mut total_symbols = 0;

                for i in 0..file_count {
                    let file_path = temp_dir.join(format!("test_{}.py", i));
                    if let Ok(symbols) = client.get_document_symbols(&file_path) {
                        total_symbols += symbols.len();
                    }
                }

                let elapsed = start.elapsed();
                let files_per_sec = file_count as f64 / elapsed.as_secs_f64();
                let symbols_per_sec = total_symbols as f64 / elapsed.as_secs_f64();

                println!("  • Time: {:?}", elapsed);
                println!("  • Files/sec: {:.2}", files_per_sec);
                println!("  • Symbols/sec: {:.2}", symbols_per_sec);
                println!("  • Total symbols: {}", total_symbols);

                let _ = client.shutdown();
            }
        }

        // クリーンアップ
        let _ = fs::remove_dir_all(&temp_dir);
    }
}

fn generate_python_file(symbol_count: usize) -> String {
    let mut content = String::new();
    content.push_str("# Generated test file\n\n");

    for i in 0..symbol_count {
        if i % 3 == 0 {
            content.push_str(&format!("def function_{}(x):\n    return x * 2\n\n", i));
        } else if i % 3 == 1 {
            content.push_str(&format!("class Class{}:\n    pass\n\n", i));
        } else {
            content.push_str(&format!("CONSTANT_{} = {}\n\n", i, i));
        }
    }

    content
}
