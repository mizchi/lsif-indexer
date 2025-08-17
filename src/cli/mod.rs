pub mod storage;
pub mod lsp_client;
pub mod lsp_indexer;
pub mod call_hierarchy_cmd;
pub mod incremental_storage;
pub mod lsp_adapter;

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
        
        /// Language (auto-detect if not specified)
        #[arg(short, long)]
        language: Option<String>,
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
    
    /// Show call hierarchy for a function
    CallHierarchy {
        /// Index file path
        #[arg(short, long)]
        index: String,
        
        /// Symbol ID
        #[arg(short, long)]
        symbol: String,
        
        /// Direction (incoming, outgoing, full)
        #[arg(short, long, default_value = "full")]
        direction: String,
        
        /// Maximum depth
        #[arg(short = 'm', long, default_value = "3")]
        max_depth: usize,
    },
    
    /// Find call paths between two functions
    CallPaths {
        /// Index file path
        #[arg(short, long)]
        index: String,
        
        /// From symbol ID
        #[arg(short, long)]
        from: String,
        
        /// To symbol ID
        #[arg(short, long)]
        to: String,
        
        /// Maximum depth
        #[arg(short = 'm', long, default_value = "5")]
        max_depth: usize,
    },
    
    /// Update index incrementally
    UpdateIncremental {
        /// Index database path
        #[arg(short, long)]
        index: String,
        
        /// Source file path to update
        #[arg(short, long)]
        source: String,
        
        /// Show dead code detection results
        #[arg(short, long)]
        detect_dead: bool,
    },
    
    /// Show dead code in the index
    ShowDeadCode {
        /// Index database path
        #[arg(short, long)]
        index: String,
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
            Commands::Generate { source, output, language } => {
                info!("Generating index from {} to {}", source, output);
                generate_index(&source, &output, language.as_deref())?;
            }
            Commands::Query { index, query_type, file, line, column } => {
                info!("Querying {} for {} at {}:{}:{}", index, query_type, file, line, column);
                query_index(&index, &query_type, &file, line, column)?;
            }
            Commands::CallHierarchy { index, symbol, direction, max_depth } => {
                info!("Showing {} call hierarchy for {} (depth: {})", direction, symbol, max_depth);
                call_hierarchy_cmd::show_call_hierarchy(&index, &symbol, &direction, max_depth)?;
            }
            Commands::CallPaths { index, from, to, max_depth } => {
                info!("Finding paths from {} to {} (max depth: {})", from, to, max_depth);
                call_hierarchy_cmd::find_paths(&index, &from, &to, max_depth)?;
            }
            Commands::UpdateIncremental { index, source, detect_dead } => {
                info!("Updating index {} incrementally with {}", index, source);
                update_incremental(&index, &source, detect_dead)?;
            }
            Commands::ShowDeadCode { index } => {
                info!("Showing dead code in index {}", index);
                show_dead_code(&index)?;
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

fn generate_index(source_path: &str, output_path: &str, language: Option<&str>) -> Result<()> {
    use self::lsp_adapter::{detect_language, GenericLspClient, RustAnalyzerAdapter};
    
    let abs_path = fs::canonicalize(source_path)?;
    let file_uri = format!("file://{}", abs_path.display());
    
    // Detect or use specified language
    let symbols = if let Some(lang) = language {
        match lang {
            "rust" => {
                let mut client = GenericLspClient::new(Box::new(RustAnalyzerAdapter))?;
                let syms = client.get_document_symbols(&file_uri)?;
                client.shutdown()?;
                syms
            }
            "typescript" | "ts" => {
                use self::lsp_adapter::TypeScriptAdapter;
                let mut client = GenericLspClient::new(Box::new(TypeScriptAdapter))?;
                let syms = client.get_document_symbols(&file_uri)?;
                client.shutdown()?;
                syms
            }
            "python" | "py" => {
                use self::lsp_adapter::PythonAdapter;
                let mut client = GenericLspClient::new(Box::new(PythonAdapter))?;
                let syms = client.get_document_symbols(&file_uri)?;
                client.shutdown()?;
                syms
            }
            _ => {
                anyhow::bail!("Unsupported language: {}", lang);
            }
        }
    } else {
        // Auto-detect language from file extension
        if let Some(adapter) = detect_language(source_path) {
            let mut client = GenericLspClient::new(adapter)?;
            let syms = client.get_document_symbols(&file_uri)?;
            client.shutdown()?;
            syms
        } else {
            // Fallback to rust-analyzer for backward compatibility
            let mut lsp_client = LspClient::spawn_rust_analyzer()?;
            let syms = lsp_client.get_document_symbols(&file_uri)?;
            lsp_client.shutdown()?;
            syms
        }
    };
    
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

fn update_incremental(index_path: &str, source_path: &str, detect_dead: bool) -> Result<()> {
    use crate::core::calculate_file_hash;
    use self::incremental_storage::IncrementalStorage;
    
    // Open incremental storage
    let storage = IncrementalStorage::open(index_path)?;
    
    // Load or create index
    let mut index = storage.load_or_create_index()?;
    
    // Read source file
    let content = fs::read_to_string(source_path)?;
    let file_hash = calculate_file_hash(&content);
    
    // Check if update is needed
    let path = std::path::Path::new(source_path);
    if !index.needs_update(path, &file_hash) {
        println!("File {} is up to date", source_path);
        return Ok(());
    }
    
    // Get symbols from LSP
    let mut lsp_client = LspClient::spawn_rust_analyzer()?;
    let abs_path = fs::canonicalize(source_path)?;
    let file_uri = format!("file://{}", abs_path.display());
    let lsp_symbols = lsp_client.get_document_symbols(&file_uri)?;
    lsp_client.shutdown()?;
    
    // Convert LSP symbols to our Symbol format
    let mut indexer = LspIndexer::new(source_path.to_string());
    indexer.index_from_symbols(lsp_symbols)?;
    let graph = indexer.into_graph();
    let symbols: Vec<_> = graph.get_all_symbols().cloned().collect();
    
    // Update index
    let result = index.update_file(path, symbols, file_hash)?;
    
    // Save incremental changes
    let metrics = storage.save_incremental(&index, &result)?;
    
    println!("Update complete: {}", metrics.summary());
    println!("  Added: {} symbols", result.added_symbols.len());
    println!("  Updated: {} symbols", result.updated_symbols.len());
    println!("  Removed: {} symbols", result.removed_symbols.len());
    
    if detect_dead {
        println!("\nDead code detected: {} symbols", result.dead_symbols.len());
        for symbol_id in result.dead_symbols.iter().take(10) {
            println!("  - {}", symbol_id);
        }
        if result.dead_symbols.len() > 10 {
            println!("  ... and {} more", result.dead_symbols.len() - 10);
        }
    }
    
    // Show storage stats
    let stats = storage.get_stats()?;
    println!("\nStorage stats:");
    println!("  Total symbols: {}", stats.total_symbols);
    println!("  Total files: {}", stats.total_files);
    println!("  DB size: {} KB", stats.db_size_bytes / 1024);
    
    Ok(())
}

fn show_dead_code(index_path: &str) -> Result<()> {
    use self::incremental_storage::IncrementalStorage;
    
    let storage = IncrementalStorage::open(index_path)?;
    let index = storage.load_or_create_index()?;
    
    let dead_symbols = index.get_dead_symbols();
    
    if dead_symbols.is_empty() {
        println!("No dead code detected.");
    } else {
        println!("Dead code found: {} symbols", dead_symbols.len());
        
        // Group by file
        let mut by_file: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for symbol_id in dead_symbols {
            if let Some(path) = index.symbol_to_file.get(symbol_id) {
                by_file.entry(path.to_string_lossy().to_string())
                    .or_insert_with(Vec::new)
                    .push(symbol_id.clone());
            }
        }
        
        for (file, symbols) in by_file {
            println!("\n{}:", file);
            for symbol in symbols.iter().take(5) {
                println!("  - {}", symbol);
            }
            if symbols.len() > 5 {
                println!("  ... and {} more", symbols.len() - 5);
            }
        }
    }
    
    Ok(())
}