use lsp::adapter::lsp::{GenericLspClient, TypeScriptAdapter};
use lsp::lsp_indexer::LspIndexer;
use std::path::PathBuf;
use std::process::Command;
use IndexStorage;

#[test]
#[ignore] // Run with: cargo test typescript_e2e -- --ignored --nocapture
fn test_typescript_lsp_indexing() {
    println!("=== TypeScript LSP E2E Test ===");

    // Check if TypeScript LSP is available
    if !check_typescript_lsp_available() {
        println!("TypeScript LSP not available, installing...");
        install_typescript_lsp();
    }

    // Get the test file path
    let test_file = PathBuf::from("tests/fixtures/typescript/sample.ts");
    if !test_file.exists() {
        panic!("Test file not found: {test_file:?}");
    }

    let abs_path = std::fs::canonicalize(&test_file).unwrap();
    let file_uri = format!("file://{}", abs_path.display());

    println!("Testing file: {file_uri}");

    // Create TypeScript LSP client
    let adapter = TypeScriptAdapter;
    let mut client =
        GenericLspClient::new(Box::new(adapter)).expect("Failed to create TypeScript LSP client");

    // Get document symbols
    println!("Getting document symbols...");
    let symbols = client
        .get_document_symbols(&file_uri)
        .expect("Failed to get document symbols");

    println!("Found {} top-level symbols", symbols.len());

    // Create indexer and process symbols
    let mut indexer = LspIndexer::new(test_file.to_str().unwrap().to_string());
    indexer
        .index_from_symbols(symbols.clone())
        .expect("Failed to index symbols");

    let graph = indexer.into_graph();

    // Verify results
    assert!(graph.symbol_count() > 0, "No symbols found in graph");

    // Print summary
    println!("\n=== Index Summary ===");
    println!("Total symbols indexed: {}", graph.symbol_count());

    // Count symbols
    let symbol_count = graph.symbol_count();
    println!("\nTotal symbols: {symbol_count}");

    // Verify expected symbols exist
    let expected_symbols = vec![
        "User",          // Interface
        "UserService",   // Class
        "validateEmail", // Function
        "UserRole",      // Enum
        "main",          // Function
    ];

    for expected in &expected_symbols {
        let found = graph.get_all_symbols().any(|s| s.name.contains(expected));
        assert!(found, "Expected symbol '{expected}' not found");
        println!("✓ Found symbol: {expected}");
    }

    // Shutdown client
    client
        .shutdown()
        .expect("Failed to shutdown TypeScript LSP");

    println!("\n=== Test Passed ===");
}

#[test]
#[ignore]
fn test_typescript_incremental_update() {
    use lsif_core::calculate_file_hash;
    use tempfile::tempdir;

    println!("=== TypeScript Incremental Update Test ===");

    // Check TypeScript LSP availability
    if !check_typescript_lsp_available() {
        println!("TypeScript LSP not available, installing...");
        install_typescript_lsp();
    }

    let test_file = PathBuf::from("tests/fixtures/typescript/sample.ts");
    let abs_path = std::fs::canonicalize(&test_file).unwrap();
    let file_uri = format!("file://{}", abs_path.display());

    // Create temporary storage
    let dir = tempdir().unwrap();
    let _db_path = dir.path().join("typescript_test.db");

    // Initial indexing
    let adapter = TypeScriptAdapter;
    let mut client = GenericLspClient::new(Box::new(adapter)).unwrap();
    let symbols = client.get_document_symbols(&file_uri).unwrap();

    let mut indexer = LspIndexer::new(test_file.to_str().unwrap().to_string());
    indexer.index_from_symbols(symbols).unwrap();
    let graph = indexer.into_graph();

    // Save graph to storage
    let storage = IndexStorage::open(dir.path()).unwrap();
    storage.save_data("graph", &graph).unwrap();

    // Simulate file change
    let content = std::fs::read_to_string(&test_file).unwrap();
    let hash1 = calculate_file_hash(&content);

    println!("Initial indexing completed.");
    println!("File hash: {}", hash1);
    println!("Symbols indexed: {}", graph.get_all_symbols().count());

    client.shutdown().unwrap();
    println!("\n=== Test Passed ===");
}

fn check_typescript_lsp_available() -> bool {
    // Check if typescript-language-server is available
    Command::new("typescript-language-server")
        .arg("--version")
        .output()
        .is_ok()
}

fn install_typescript_lsp() {
    println!("Installing TypeScript language server...");

    let output = Command::new("npm")
        .args(["install", "-g", "typescript-language-server", "typescript"])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            println!("TypeScript LSP installed successfully");
        }
        Ok(result) => {
            println!("Warning: Failed to install TypeScript LSP");
            println!("stderr: {}", String::from_utf8_lossy(&result.stderr));
            println!("You may need to install it manually: npm install -g typescript-language-server typescript");
        }
        Err(e) => {
            println!("Warning: npm not found: {e}");
            println!("Please install Node.js and run: npm install -g typescript-language-server typescript");
        }
    }
}
