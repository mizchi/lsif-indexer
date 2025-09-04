use cli::python_adapter::PythonAdapter;
use cli::typescript_adapter::TypeScriptAdapter;
use lsp::adapter::go::GoAdapter;
use lsp::lsp_minimal_client::MinimalLspClient;
use std::path::PathBuf;
use std::time::Instant;

/// 簡易性能測定テスト

#[test]
#[ignore]
fn quick_performance_check() {
    println!("\n=== Quick Performance Check ===\n");

    // Go言語の性能測定
    let go_project = PathBuf::from("test-go-project");
    if go_project.exists() {
        println!("📊 Go Language:");
        let adapter = Box::new(GoAdapter);
        let start = Instant::now();

        if let Ok(mut client) = MinimalLspClient::new(adapter) {
            if client.initialize(&go_project).is_ok() {
                let main_file = go_project.join("main.go");
                if let Ok(symbols) = client.get_document_symbols(&main_file) {
                    let elapsed = start.elapsed();
                    println!("  • Time: {:?}", elapsed);
                    println!("  • Symbols found: {}", symbols.len());
                    println!(
                        "  • Speed: {:.0} symbols/sec",
                        symbols.len() as f64 / elapsed.as_secs_f64()
                    );
                }
                let _ = client.shutdown();
            }
        }
    }

    // Python言語の性能測定
    let python_project = PathBuf::from("test-python-project");
    if python_project.exists() {
        println!("\n📊 Python Language:");
        let adapter = Box::new(PythonAdapter::new());
        let start = Instant::now();

        if let Ok(mut client) = MinimalLspClient::new(adapter) {
            if client.initialize(&python_project).is_ok() {
                let calc_file = python_project.join("calculator.py");
                if let Ok(symbols) = client.get_document_symbols(&calc_file) {
                    let elapsed = start.elapsed();
                    println!("  • Time: {:?}", elapsed);
                    println!("  • Symbols found: {}", symbols.len());
                    println!(
                        "  • Speed: {:.0} symbols/sec",
                        symbols.len() as f64 / elapsed.as_secs_f64()
                    );
                }
                let _ = client.shutdown();
            }
        }
    }

    // TypeScript言語の性能測定
    let ts_project = PathBuf::from("test-typescript-project");
    if ts_project.exists() {
        println!("\n📊 TypeScript Language:");
        let adapter = Box::new(TypeScriptAdapter::new());
        let start = Instant::now();

        if let Ok(mut client) = MinimalLspClient::new(adapter) {
            if client.initialize(&ts_project).is_ok() {
                let calc_file = ts_project.join("calculator.ts");
                if let Ok(symbols) = client.get_document_symbols(&calc_file) {
                    let elapsed = start.elapsed();
                    println!("  • Time: {:?}", elapsed);
                    println!("  • Symbols found: {}", symbols.len());
                    println!(
                        "  • Speed: {:.0} symbols/sec",
                        symbols.len() as f64 / elapsed.as_secs_f64()
                    );
                }
                let _ = client.shutdown();
            }
        }
    }

    println!("\n✅ Quick performance check completed");
}
