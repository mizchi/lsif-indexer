use anyhow::Result;
use lsif_indexer::{IndexStorage, CodeGraph, generate_lsif};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing_subscriber;
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 {
        eprintln!("Usage: {} <index_db> <output_lsif>", args[0]);
        eprintln!("Example: {} index.db output.lsif", args[0]);
        std::process::exit(1);
    }
    
    let index_path = PathBuf::from(&args[1]);
    let output_path = PathBuf::from(&args[2]);
    
    if !index_path.exists() {
        anyhow::bail!("Index file not found: {}", index_path.display());
    }
    
    // Load the index
    info!("Loading index from: {}", index_path.display());
    let storage = IndexStorage::open(&index_path)?;
    
    // Load graph
    let graph: CodeGraph = storage.load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;
    
    info!("Loaded graph with {} symbols", graph.symbol_count());
    
    // Generate LSIF
    info!("Generating LSIF format...");
    let lsif_content = generate_lsif(graph)?;
    
    // Count lines for statistics
    let line_count = lsif_content.lines().count();
    
    // Write to file
    fs::write(&output_path, &lsif_content)?;
    
    info!("LSIF export successful!");
    println!("\n=== LSIF Export Summary ===");
    println!("Input index: {}", index_path.display());
    println!("Output LSIF: {}", output_path.display());
    println!("Total elements: {}", line_count);
    println!("File size: {} bytes", lsif_content.len());
    
    Ok(())
}