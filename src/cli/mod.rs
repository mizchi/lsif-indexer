pub mod storage;
pub mod lsp_client;
pub mod lsp_indexer;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use crate::core::{
    CodeGraph, generate_lsif, parse_lsif
};
use self::storage::{IndexStorage, IndexMetadata, IndexFormat};
use self::lsp_client::LspClient;
use self::lsp_indexer::LspIndexer;
use tracing::info;

#[derive(Parser)]
#[command(name = "lsif-indexer")]
#[command(about = "Language-neutral code index tool supporting LSIF format")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Export index to LSIF format
    ExportLsif {
        /// Index database path
        #[arg(short, long)]
        index: String,
        
        /// Output LSIF file path
        #[arg(short, long)]
        output: String,
    },
    
    /// Import LSIF format to index
    ImportLsif {
        /// Input LSIF file path
        #[arg(short, long)]
        input: String,
        
        /// Output index database path
        #[arg(short, long)]
        output: String,
    },
    
    /// Generate index from source code (using LSP)
    Generate {
        /// Source file path
        #[arg(short, long)]
        source: String,
        
        /// Output index database path
        #[arg(short, long)]
        output: String,
    },
    
    /// Query the index
    Query {
        /// Index file path
        #[arg(short, long)]
        index: String,
        
        /// Query type (definition, references, hover)
        #[arg(short, long)]
        query_type: String,
        
        /// File path
        #[arg(short, long)]
        file: String,
        
        /// Line number
        #[arg(short, long)]
        line: u32,
        
        /// Column number
        #[arg(short, long)]
        column: u32,
    },
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        match self.command {
            Commands::ExportLsif { index, output } => {
                info!("Exporting index {} to LSIF format {}", index, output);
                export_lsif(&index, &output)?;
            }
            Commands::ImportLsif { input, output } => {
                info!("Importing LSIF {} to index {}", input, output);
                import_lsif(&input, &output)?;
            }
            Commands::Generate { source, output } => {
                info!("Generating index from {} to {}", source, output);
                generate_index(&source, &output)?;
            }
            Commands::Query { index, query_type, file, line, column } => {
                info!("Querying {} for {} at {}:{}:{}", index, query_type, file, line, column);
                query_index(&index, &query_type, &file, line, column)?;
            }
        }
        Ok(())
    }
}

fn export_lsif(index_path: &str, output_path: &str) -> Result<()> {
    // Load the index
    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage.load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;
    
    // Generate LSIF
    let lsif_content = generate_lsif(graph)?;
    
    // Write to file
    fs::write(output_path, &lsif_content)?;
    
    info!("LSIF exported to {}", output_path);
    Ok(())
}

fn import_lsif(input_path: &str, output_path: &str) -> Result<()> {
    // Read LSIF content
    let lsif_content = fs::read_to_string(input_path)?;
    
    // Parse LSIF to graph
    let graph = parse_lsif(&lsif_content)?;
    
    // Save to database
    let storage = IndexStorage::open(output_path)?;
    
    let metadata = IndexMetadata {
        format: IndexFormat::Lsif,
        version: "0.5.0".to_string(),
        created_at: chrono::Utc::now(),
        project_root: std::env::current_dir()?.to_string_lossy().to_string(),
        files_count: 1,
        symbols_count: graph.symbol_count(),
    };
    
    storage.save_metadata(&metadata)?;
    storage.save_data("graph", &graph)?;
    
    info!("LSIF imported to {}", output_path);
    Ok(())
}

fn generate_index(source_path: &str, output_path: &str) -> Result<()> {
    // Get symbols from rust-analyzer
    let mut lsp_client = LspClient::spawn_rust_analyzer()?;
    let abs_path = fs::canonicalize(source_path)?;
    let file_uri = format!("file://{}", abs_path.display());
    let symbols = lsp_client.get_document_symbols(&file_uri)?;
    lsp_client.shutdown()?;
    
    // Create index from symbols
    let mut indexer = LspIndexer::new(source_path.to_string());
    indexer.index_from_symbols(symbols)?;
    
    // Save index
    let graph = indexer.into_graph();
    let storage = IndexStorage::open(output_path)?;
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
    
    info!("Index generated at {} with {} symbols", output_path, graph.symbol_count());
    Ok(())
}

fn query_index(index_path: &str, query_type: &str, file: &str, line: u32, _column: u32) -> Result<()> {
    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage.load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;
    
    // Create a symbol ID based on file and line
    let symbol_id = format!("{file}#{line}:");
    
    match query_type {
        "definition" => {
            if let Some(def) = graph.find_definition(&symbol_id) {
                println!("Definition: {} at {}:{}", def.name, def.file_path, def.range.start.line);
            } else {
                println!("No definition found");
            }
        }
        "references" => {
            let refs = graph.find_references(&symbol_id);
            println!("Found {} references:", refs.len());
            for r in refs {
                println!("  - {} at {}:{}", r.name, r.file_path, r.range.start.line);
            }
        }
        "hover" => {
            if let Some(symbol) = graph.find_symbol(&symbol_id) {
                if let Some(doc) = &symbol.documentation {
                    println!("Documentation: {doc}");
                } else {
                    println!("No documentation available");
                }
            } else {
                println!("Symbol not found");
            }
        }
        _ => {
            anyhow::bail!("Unknown query type: {}", query_type);
        }
    }
    
    Ok(())
}