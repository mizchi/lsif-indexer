use anyhow::Result;
use lsif_indexer::{LspClient, LspIndexer, IndexStorage, IndexMetadata, IndexFormat};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing_subscriber;
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let args: Vec<String> = env::args().collect();
    
    let (target_file, output_path) = if args.len() > 2 {
        (PathBuf::from(&args[1]), PathBuf::from(&args[2]))
    } else if args.len() > 1 {
        (PathBuf::from(&args[1]), PathBuf::from("index.db"))
    } else {
        (PathBuf::from("src/main.rs"), PathBuf::from("index.db"))
    };
    
    if !target_file.exists() {
        anyhow::bail!("File not found: {}", target_file.display());
    }
    
    info!("Generating index for: {}", target_file.display());
    info!("Output path: {}", output_path.display());
    
    // Step 1: Get symbols from rust-analyzer
    info!("Starting rust-analyzer...");
    let mut lsp_client = LspClient::spawn_rust_analyzer()?;
    
    let abs_path = fs::canonicalize(&target_file)?;
    let file_uri = format!("file://{}", abs_path.display());
    
    info!("Requesting document symbols...");
    let symbols = lsp_client.get_document_symbols(&file_uri)?;
    info!("Received {} symbols", symbols.len());
    
    // Shutdown LSP client
    lsp_client.shutdown()?;
    
    // Step 2: Create index from symbols
    info!("Creating index from symbols...");
    let mut indexer = LspIndexer::new(target_file.to_string_lossy().to_string());
    indexer.index_from_symbols(symbols)?;
    
    // Step 3: Save index to database
    info!("Saving index to database...");
    let graph = indexer.into_graph();
    let storage = IndexStorage::open(&output_path)?;
    let metadata = IndexMetadata {
        format: IndexFormat::Lsif,
        version: "1.0.0".to_string(),
        created_at: chrono::Utc::now(),
        project_root: std::env::current_dir()?.to_string_lossy().to_string(),
        files_count: 1,
        symbols_count: graph.symbol_count(),
    };
    
    storage.save_metadata(&metadata)?;
    storage.save_data("graph", &graph)?;
    info!("Index saved with {} symbols", graph.symbol_count());
    
    info!("Index successfully generated at: {}", output_path.display());
    
    // Print summary
    println!("\n=== Index Generation Summary ===");
    println!("Source file: {}", target_file.display());
    println!("Index file: {}", output_path.display());
    println!("Status: SUCCESS");
    
    Ok(())
}