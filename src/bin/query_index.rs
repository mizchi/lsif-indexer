use anyhow::Result;
use lsif_indexer::{IndexStorage, CodeGraph};
use std::env;
use std::path::PathBuf;
use tracing_subscriber;
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <index_path> [symbol_id]", args[0]);
        eprintln!("Example: {} index.db", args[0]);
        eprintln!("Example: {} index.db 'src/main.rs#66:main'", args[0]);
        std::process::exit(1);
    }
    
    let index_path = PathBuf::from(&args[1]);
    
    if !index_path.exists() {
        anyhow::bail!("Index file not found: {}", index_path.display());
    }
    
    // Load the index
    info!("Loading index from: {}", index_path.display());
    let storage = IndexStorage::open(&index_path)?;
    
    // Load metadata
    if let Some(metadata) = storage.load_metadata()? {
        println!("\n=== Index Metadata ===");
        println!("Format: {:?}", metadata.format);
        println!("Version: {}", metadata.version);
        println!("Created at: {}", metadata.created_at);
        println!("Project root: {}", metadata.project_root);
        println!("Files count: {}", metadata.files_count);
        println!("Symbols count: {}", metadata.symbols_count);
    }
    
    // Load graph
    let graph: CodeGraph = storage.load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;
    
    println!("\n=== Graph Statistics ===");
    println!("Total symbols: {}", graph.symbol_count());
    
    // If a symbol ID was provided, query it
    if args.len() > 2 {
        let symbol_id = &args[2];
        println!("\n=== Querying Symbol: {} ===", symbol_id);
        
        if let Some(symbol) = graph.find_symbol(symbol_id) {
            println!("Found symbol:");
            println!("  Name: {}", symbol.name);
            println!("  Kind: {:?}", symbol.kind);
            println!("  File: {}", symbol.file_path);
            println!("  Location: line {}, character {}", 
                symbol.range.start.line + 1, 
                symbol.range.start.character
            );
            
            if let Some(doc) = &symbol.documentation {
                println!("  Documentation: {}", doc);
            }
            
            // Find references
            let references = graph.find_references(symbol_id);
            if !references.is_empty() {
                println!("\n  References ({}):", references.len());
                for ref_symbol in references {
                    println!("    - {} at {}:{}", 
                        ref_symbol.name,
                        ref_symbol.file_path,
                        ref_symbol.range.start.line + 1
                    );
                }
            }
            
            // Find definition
            if let Some(def) = graph.find_definition(symbol_id) {
                println!("\n  Definition:");
                println!("    - {} at {}:{}", 
                    def.name,
                    def.file_path,
                    def.range.start.line + 1
                );
            }
        } else {
            println!("Symbol not found!");
            println!("\nAvailable symbols (first 10):");
            
            // TODO: Add method to list all symbols from the graph
            
            println!("\nTry querying with a symbol ID like: 'src/main.rs#66:main'");
        }
    } else {
        println!("\nTo query a specific symbol, provide a symbol ID as the second argument.");
        println!("Example: {} {} 'src/main.rs#66:main'", args[0], index_path.display());
    }
    
    Ok(())
}