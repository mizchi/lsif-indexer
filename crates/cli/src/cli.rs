#[path = "commands/mod.rs"]
mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;
use std::time::Instant;

use crate::differential_indexer::DifferentialIndexer;
use crate::git_diff::GitDiffDetector;
use commands::{
    definition::handle_definition,
    references::handle_references,
    search::handle_search,
    index::handle_index,
    utils::print_success,
};

const DEFAULT_INDEX_PATH: &str = ".lsif-index.db";

#[derive(Parser)]
#[command(name = "lsif")]
#[command(about = "Fast code indexer and search tool with smart auto-indexing")]
#[command(version)]
pub struct Cli {
    /// Database path (default: .lsif-index.db)
    #[arg(short = 'D', long = "db", global = true)]
    pub database: Option<String>,

    /// Project root (default: current directory)
    #[arg(short = 'P', long = "project", global = true)]
    pub project_root: Option<String>,

    /// Disable automatic indexing
    #[arg(short = 'n', long = "no-auto-index", global = true)]
    pub no_auto_index: bool,

    /// Verbose output
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    /// Output format (human, quickfix, lsp, grep, json, tsv, null)
    #[arg(short = 'f', long = "format", global = true, default_value = "human")]
    pub format: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Go to definition [aliases: def, d]
    #[command(visible_alias = "def", visible_alias = "d")]
    Definition {
        /// Location: file.rs:10:5 or just file.rs
        #[arg(value_name = "LOCATION")]
        location: String,
        
        /// Show all definitions if multiple exist
        #[arg(short = 'a', long = "all")]
        show_all: bool,
    },

    /// Find references [aliases: ref, r]
    #[command(visible_alias = "ref", visible_alias = "r")]
    References {
        /// Location: file.rs:10:5 or just file.rs
        #[arg(value_name = "LOCATION")]
        location: String,
        
        /// Include definitions in results
        #[arg(short = 'd', long = "include-defs")]
        include_definitions: bool,
        
        /// Group by file
        #[arg(short = 'g', long = "group")]
        group_by_file: bool,
    },

    /// Show call hierarchy [aliases: calls, c]
    #[command(visible_alias = "calls", visible_alias = "c")]
    CallHierarchy {
        /// Symbol name or location
        #[arg(value_name = "SYMBOL")]
        symbol: String,
        
        /// Show incoming calls (who calls this)
        #[arg(short = 'i', long = "incoming", conflicts_with = "outgoing")]
        incoming: bool,
        
        /// Show outgoing calls (what this calls)
        #[arg(short = 'o', long = "outgoing", conflicts_with = "incoming")]
        outgoing: bool,
        
        /// Maximum depth (default: 3)
        #[arg(short = 'l', long = "level", default_value = "3")]
        max_depth: usize,
    },

    /// Search symbols [aliases: search, s, find]
    #[command(visible_alias = "search", visible_alias = "s", visible_alias = "find")]
    WorkspaceSymbols {
        /// Search query
        query: String,
        
        /// Use fuzzy matching
        #[arg(short = 'f', long = "fuzzy")]
        fuzzy: bool,
        
        /// Filter by type (function|class|variable|interface|enum)
        #[arg(short = 't', long = "type")]
        symbol_type: Option<String>,
        
        /// Filter by file pattern
        #[arg(short = 'p', long = "path")]
        path_pattern: Option<String>,
        
        /// Filter by return type
        #[arg(long = "returns")]
        returns: Option<String>,
        
        /// Filter by parameter type
        #[arg(long = "takes")]
        takes: Option<String>,
        
        /// Filter by implementation/trait
        #[arg(long = "implements")]
        implements: Option<String>,
        
        /// Filter by field type
        #[arg(long = "has-field")]
        has_field: Option<String>,
        
        /// Maximum results (default: 50)
        #[arg(short = 'm', long = "max", default_value = "50")]
        max_results: usize,
    },

    /// Index the project [aliases: idx, i]
    #[command(visible_alias = "idx", visible_alias = "i")]
    Index {
        /// Force full reindex
        #[arg(short = 'f', long = "force")]
        force: bool,
        
        /// Show progress
        #[arg(short = 'p', long = "progress")]
        show_progress: bool,
        
        /// Use fallback indexer only (faster but less accurate)
        #[arg(long = "fallback-only")]
        fallback_only: bool,
    },

    /// Find unused code [aliases: unused, u]
    #[command(visible_alias = "unused", visible_alias = "u")]
    Unused {
        /// Show only public unused symbols
        #[arg(short = 'p', long = "public")]
        public_only: bool,
        
        /// Filter by file pattern
        #[arg(short = 'f', long = "filter")]
        file_filter: Option<String>,
        
        /// Export as JSON
        #[arg(short = 'j', long = "json")]
        json_output: bool,
    },

    /// Show project statistics [aliases: stats, st]
    #[command(visible_alias = "stats", visible_alias = "st")]
    Status {
        /// Show detailed statistics
        #[arg(short = 'd', long = "detailed")]
        detailed: bool,
        
        /// Group by file
        #[arg(short = 'f', long = "by-file")]
        by_file: bool,
        
        /// Group by symbol type
        #[arg(short = 't', long = "by-type")]
        by_type: bool,
    },

    /// Export index data [aliases: export, e]
    #[command(visible_alias = "export", visible_alias = "e")]
    Export {
        /// Output file
        output: String,
        
        /// Export format (json|lsif|dot)
        #[arg(short = 'f', long = "format", default_value = "json")]
        format: String,
        
        /// Include references
        #[arg(short = 'r', long = "refs")]
        include_refs: bool,
    },
}

impl Cli {
    pub fn run(self) -> Result<()> {
        // Initialize tracing based on verbose flag
        if self.verbose {
            tracing_subscriber::fmt()
                .with_env_filter("debug")
                .init();
        }

        let db_path = self.database.unwrap_or_else(|| DEFAULT_INDEX_PATH.to_string());
        let project_root = self.project_root.unwrap_or_else(|| ".".to_string());

        // Smart auto-indexing: only if DB doesn't exist or is stale
        if !self.no_auto_index && should_auto_index(&db_path, &project_root)? {
            quick_index(&db_path, &project_root)?;
        }

        let format = crate::output_format::OutputFormat::from_str(&self.format)?;
        
        match self.command {
            Commands::Definition { location, show_all } => {
                handle_definition(&db_path, &location, show_all, format)?;
            }
            Commands::References { location, include_definitions, group_by_file } => {
                handle_references(&db_path, &location, include_definitions, group_by_file, format)?;
            }
            Commands::CallHierarchy { symbol, incoming, outgoing, max_depth } => {
                handle_call_hierarchy(&db_path, &symbol, incoming, outgoing, max_depth)?;
            }
            Commands::WorkspaceSymbols { query, fuzzy, symbol_type, path_pattern, max_results, returns, takes, implements, has_field } => {
                handle_search(&db_path, &query, fuzzy, symbol_type, path_pattern, max_results, format, returns, takes, implements, has_field)?;
            }
            Commands::Index { force, show_progress, fallback_only } => {
                handle_index(&db_path, &project_root, force, show_progress, fallback_only)?;
            }
            Commands::Unused { public_only, file_filter, json_output } => {
                handle_unused(&db_path, public_only, file_filter, json_output)?;
            }
            Commands::Status { detailed, by_file, by_type } => {
                commands::stats::handle_stats(&db_path, detailed, by_file, by_type)?;
            }
            Commands::Export { output, format, include_refs } => {
                handle_export(&db_path, &output, &format, include_refs)?;
            }
        }

        Ok(())
    }
}

// Helper functions

/// Check if auto-indexing is needed
fn should_auto_index(db_path: &str, project_root: &str) -> Result<bool> {
    if !Path::new(db_path).exists() {
        return Ok(true);
    }

    // Quick check using git HEAD
    match GitDiffDetector::new(Path::new(project_root)) {
        Ok(_detector) => {
            // Simplified - always check for now
            Ok(true)
        }
        Err(_) => Ok(false)
    }
}

/// Quick incremental index
fn quick_index(db_path: &str, project_root: &str) -> Result<()> {
    let start = Instant::now();
    println!("⚡ Quick indexing...");
    
    let mut indexer = DifferentialIndexer::new(db_path, Path::new(project_root))?;
    
    // 環境変数でフォールバックオンリーモードを制御
    if std::env::var("LSIF_FALLBACK_ONLY").is_ok() {
        indexer.set_fallback_only(true);
    }
    
    let result = indexer.index_differential()?;
    
    if result.files_added + result.files_modified + result.files_deleted > 0 {
        print_success(&format!(
            "Indexed in {:.2}s (+{} ~{} -{} files)",
            start.elapsed().as_secs_f64(),
            result.files_added,
            result.files_modified,
            result.files_deleted
        ));
    }
    
    Ok(())
}

// Stub handlers for unimplemented commands

fn handle_call_hierarchy(
    db_path: &str,
    symbol: &str,
    incoming: bool,
    outgoing: bool,
    _max_depth: usize,
) -> Result<()> {
    use commands::utils::{load_graph, print_info, print_error};
    
    let direction = if incoming { "incoming" } else if outgoing { "outgoing" } else { "both" };
    print_info(&format!("Analyzing call hierarchy for {} ({})", symbol, direction), "📞");
    
    let graph = load_graph(db_path)?;
    
    // Find the symbol
    let target_symbol = graph.get_all_symbols()
        .find(|s| s.name == symbol)
        .cloned();
    
    if let Some(sym) = target_symbol {
        println!("Symbol: {} at {}:{}:{}", 
            sym.name, 
            sym.file_path,
            sym.range.start.line,
            sym.range.start.character
        );
        
        if incoming || !outgoing {
            println!("\n⬇️  Incoming calls: (not yet implemented)");
        }
        
        if outgoing || !incoming {
            println!("\n⬆️  Outgoing calls: (not yet implemented)");
        }
    } else {
        print_error(&format!("Symbol '{}' not found", symbol));
    }
    
    Ok(())
}

fn handle_unused(
    db_path: &str,
    _public_only: bool,
    _file_filter: Option<String>,
    _json_output: bool,
) -> Result<()> {
    use commands::utils::{load_graph, print_info};
    
    print_info("Finding unused code...", "🗑️");
    
    let graph = load_graph(db_path)?;
    
    // TODO: Implement actual unused code detection
    println!("Unused code detection not yet implemented");
    println!("Total symbols in index: {}", graph.get_all_symbols().count());
    
    Ok(())
}

fn handle_export(
    db_path: &str,
    output: &str,
    format: &str,
    _include_refs: bool,
) -> Result<()> {
    use std::fs::File;
    use std::io::Write;
    use commands::utils::{load_graph, print_info, print_success, print_error};
    
    print_info(&format!("Exporting to {} (format: {})", output, format), "📤");
    
    let graph = load_graph(db_path)?;
    
    match format {
        "json" => {
            let symbols: Vec<_> = graph.get_all_symbols().cloned().collect();
            let data = serde_json::json!({
                "symbols": symbols,
                "total": symbols.len(),
            });
            
            let mut file = File::create(output)?;
            file.write_all(serde_json::to_string_pretty(&data)?.as_bytes())?;
            
            print_success(&format!("Exported {} symbols to {}", symbols.len(), output));
        }
        _ => {
            print_error(&format!("Format '{}' not yet implemented. Supported: json", format));
        }
    }
    
    Ok(())
}