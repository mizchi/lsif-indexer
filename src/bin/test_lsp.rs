use anyhow::Result;
use lsif_indexer::LspClient;
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing_subscriber;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    // Get target file from args or use default
    let args: Vec<String> = env::args().collect();
    let target_file = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("src/main.rs")
    };
    
    if !target_file.exists() {
        anyhow::bail!("File not found: {}", target_file.display());
    }
    
    println!("Testing LSP with rust-analyzer on: {}", target_file.display());
    
    // Spawn rust-analyzer
    let mut client = LspClient::spawn_rust_analyzer()?;
    
    // Get absolute path and convert to file URI
    let abs_path = fs::canonicalize(&target_file)?;
    let file_uri = format!("file://{}", abs_path.display());
    
    println!("Requesting document symbols for: {}", file_uri);
    
    // Request document symbols
    let symbols = client.get_document_symbols(&file_uri)?;
    
    // Save results to JSON
    let output_file = "lsp_symbols.json";
    let json_output = serde_json::to_string_pretty(&symbols)?;
    fs::write(output_file, &json_output)?;
    
    println!("Found {} symbols", symbols.len());
    println!("Results saved to: {}", output_file);
    
    // Print summary
    for symbol in &symbols {
        println!(
            "  - {} '{}' at line {}",
            format!("{:?}", symbol.kind),
            symbol.name,
            symbol.range.start.line + 1
        );
        
        // Print children if any
        if let Some(children) = &symbol.children {
            for child in children {
                println!(
                    "    - {} '{}' at line {}",
                    format!("{:?}", child.kind),
                    child.name,
                    child.range.start.line + 1
                );
            }
        }
    }
    
    // Shutdown
    client.shutdown()?;
    
    Ok(())
}