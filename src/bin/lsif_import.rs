use anyhow::Result;
use lsif_indexer::{IndexStorage, IndexMetadata, IndexFormat, parse_lsif};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing_subscriber;
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 {
        eprintln!("Usage: {} <input_lsif> <output_db>", args[0]);
        eprintln!("Example: {} input.lsif index.db", args[0]);
        std::process::exit(1);
    }
    
    let input_path = PathBuf::from(&args[1]);
    let output_path = PathBuf::from(&args[2]);
    
    if !input_path.exists() {
        anyhow::bail!("LSIF file not found: {}", input_path.display());
    }
    
    // Read LSIF content
    info!("Reading LSIF from: {}", input_path.display());
    let lsif_content = fs::read_to_string(&input_path)?;
    let line_count = lsif_content.lines().count();
    
    info!("Parsing {} LSIF elements...", line_count);
    
    // Parse LSIF to graph
    let graph = parse_lsif(&lsif_content)?;
    let symbol_count = graph.symbol_count();
    
    info!("Parsed {} symbols from LSIF", symbol_count);
    
    // Save to database
    info!("Saving to database: {}", output_path.display());
    let storage = IndexStorage::open(&output_path)?;
    
    // Save metadata
    let metadata = IndexMetadata {
        format: IndexFormat::Lsif,
        version: "0.5.0".to_string(),
        created_at: chrono::Utc::now(),
        project_root: std::env::current_dir()?.to_string_lossy().to_string(),
        files_count: 1, // This would need to be calculated from the graph
        symbols_count: symbol_count,
    };
    
    storage.save_metadata(&metadata)?;
    storage.save_data("graph", &graph)?;
    
    info!("LSIF import successful!");
    println!("\n=== LSIF Import Summary ===");
    println!("Input LSIF: {}", input_path.display());
    println!("Output index: {}", output_path.display());
    println!("Total elements: {}", line_count);
    println!("Symbols imported: {}", symbol_count);
    
    Ok(())
}