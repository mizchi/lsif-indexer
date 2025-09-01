use crate::differential_indexer::DifferentialIndexer;
use crate::git_diff::GitDiffDetector;
use crate::storage::IndexStorage;
use lsif_core::{CodeGraph, SymbolKind};
use serde_json;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;
use std::time::Instant;

const DEFAULT_INDEX_PATH: &str = ".lsif-index.db";
const MAX_CHANGES_DISPLAY: usize = 15;

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

        match self.command {
            Commands::Definition { location, show_all } => {
                handle_definition(&db_path, &location, show_all)?;
            }
            Commands::References { location, include_definitions, group_by_file } => {
                handle_references(&db_path, &location, include_definitions, group_by_file)?;
            }
            Commands::CallHierarchy { symbol, incoming, outgoing, max_depth } => {
                let direction = if incoming {
                    "incoming"
                } else if outgoing {
                    "outgoing"
                } else {
                    "both"
                };
                handle_calls(&db_path, &symbol, direction, max_depth)?;
            }
            Commands::WorkspaceSymbols { query, fuzzy, symbol_type, path_pattern, max_results } => {
                handle_find(&db_path, &query, fuzzy, symbol_type, path_pattern, max_results)?;
            }
            Commands::Index { force, show_progress } => {
                handle_index(&db_path, &project_root, force, show_progress)?;
            }
            Commands::Unused { public_only, file_filter, json_output } => {
                handle_unused(&db_path, public_only, file_filter, json_output)?;
            }
            Commands::Status { detailed, by_file, by_type } => {
                handle_stats(&db_path, detailed, by_file, by_type)?;
            }
            Commands::Export { output, format, include_refs } => {
                handle_export(&db_path, &output, &format, include_refs)?;
            }
        }

        Ok(())
    }
}

// Helper functions

/// Parse location format: file.rs:10:5 or file.rs
fn parse_location(location: &str) -> Result<(String, u32, u32)> {
    let parts: Vec<&str> = location.split(':').collect();
    let file = parts[0].to_string();
    let line = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let column = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    Ok((file, line, column))
}

/// Check if auto-indexing is needed
fn should_auto_index(db_path: &str, project_root: &str) -> Result<bool> {
    if !Path::new(db_path).exists() {
        return Ok(true);
    }

    // Quick check using git HEAD
    match GitDiffDetector::new(Path::new(project_root)) {
        Ok(detector) => {
            // Check if there are any changes
            Ok(true) // Simplified - always check for now
        }
        Err(_) => Ok(false)
    }
}

/// Quick incremental index
fn quick_index(db_path: &str, project_root: &str) -> Result<()> {
    let start = Instant::now();
    println!("⚡ Quick indexing...");
    
    let mut indexer = DifferentialIndexer::new(db_path, Path::new(project_root))?;
    let result = indexer.index_differential()?;
    
    if result.files_added + result.files_modified + result.files_deleted > 0 {
        println!(
            "✅ Indexed in {:.2}s (+{} ~{} -{} files)",
            start.elapsed().as_secs_f64(),
            result.files_added,
            result.files_modified,
            result.files_deleted
        );
    }
    
    Ok(())
}

// Command handlers

fn handle_definition(db_path: &str, location: &str, _show_all: bool) -> Result<()> {
    let (file, line, column) = parse_location(location)?;
    
    println!("🔍 Finding definition at {}:{}:{}", file, line, column);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_data::<CodeGraph>("graph")?.unwrap_or_default();
    
    // Find symbol at location (simplified)
    let symbol = graph.get_all_symbols()
        .find(|s| s.file_path == file && 
              s.range.start.line == line &&
              s.range.start.character >= column.saturating_sub(5) &&
              s.range.start.character <= column + 5);
    
    if let Some(sym) = symbol {
        println!("📍 {} at {}:{}:{}", 
            sym.name, 
            sym.file_path,
            sym.range.start.line,
            sym.range.start.character
        );
    } else {
        println!("❌ No definition found at this location");
    }
    
    Ok(())
}

fn handle_references(db_path: &str, location: &str, _include_defs: bool, _group: bool) -> Result<()> {
    let (file, line, column) = parse_location(location)?;
    
    println!("🔗 Finding references for {}:{}:{}", file, line, column);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_data::<CodeGraph>("graph")?.unwrap_or_default();
    
    // Find symbol at location
    let symbol = graph.get_all_symbols()
        .find(|s| s.file_path == file && 
              s.range.start.line == line &&
              s.range.start.character >= column.saturating_sub(5) &&
              s.range.start.character <= column + 5);
    
    if let Some(sym) = symbol {
        println!("Found symbol: {}", sym.name);
        // TODO: Implement actual reference finding
        println!("Reference finding not yet implemented in simplified version");
    } else {
        println!("❌ No symbol found at this location");
    }
    
    Ok(())
}

fn handle_calls(db_path: &str, symbol: &str, direction: &str, _depth: usize) -> Result<()> {
    println!("📞 Analyzing call hierarchy for {} ({})", symbol, direction);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_data::<CodeGraph>("graph")?.unwrap_or_default();
    
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
        
        if direction == "incoming" || direction == "both" {
            println!("\n⬇️  Incoming calls: (not yet implemented)");
        }
        
        if direction == "outgoing" || direction == "both" {
            println!("\n⬆️  Outgoing calls: (not yet implemented)");
        }
    } else {
        println!("❌ Symbol '{}' not found", symbol);
    }
    
    Ok(())
}

fn handle_find(db_path: &str, query: &str, fuzzy: bool, symbol_type: Option<String>, 
               path_pattern: Option<String>, max_results: usize) -> Result<()> {
    let mode = if fuzzy { "fuzzy" } else { "exact" };
    println!("🔍 Searching for '{}' ({})", query, mode);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_data::<CodeGraph>("graph")?.unwrap_or_default();
    
    let mut results = Vec::new();
    
    for symbol in graph.get_all_symbols() {
        // Type filter
        if let Some(ref st) = symbol_type {
            let matches = match st.as_str() {
                "function" => matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method),
                "class" => matches!(symbol.kind, SymbolKind::Class),
                "variable" => matches!(symbol.kind, SymbolKind::Variable | SymbolKind::Field),
                "interface" => matches!(symbol.kind, SymbolKind::Interface),
                "enum" => matches!(symbol.kind, SymbolKind::Enum),
                _ => false,
            };
            if !matches {
                continue;
            }
        }
        
        // Path filter
        if let Some(ref pattern) = path_pattern {
            if !symbol.file_path.contains(pattern) {
                continue;
            }
        }
        
        // Name matching
        let matches = if fuzzy {
            symbol.name.to_lowercase().contains(&query.to_lowercase())
        } else {
            symbol.name == query
        };
        
        if matches {
            results.push(symbol.clone());
            if results.len() >= max_results {
                break;
            }
        }
    }
    
    if results.is_empty() {
        println!("❌ No symbols found");
    } else {
        println!("Found {} symbols (max: {})", results.len(), max_results);
        for symbol in results {
            let kind = format!("{:?}", symbol.kind).to_lowercase();
            println!("  🔹 {} ({}) - {}:{}:{}", 
                symbol.name, 
                kind,
                symbol.file_path,
                symbol.range.start.line,
                symbol.range.start.character
            );
        }
    }
    
    Ok(())
}

fn handle_index(db_path: &str, project_root: &str, force: bool, _show_progress: bool) -> Result<()> {
    let start = Instant::now();
    
    if force {
        println!("🔄 Force reindexing project...");
        if Path::new(db_path).exists() {
            std::fs::remove_file(db_path)?;
        }
    } else {
        println!("📇 Indexing project...");
    }
    
    let mut indexer = DifferentialIndexer::new(db_path, Path::new(project_root))?;
    
    let result = if force || !Path::new(db_path).exists() {
        indexer.full_reindex()?
    } else {
        indexer.index_differential()?
    };
    
    println!(
        "✅ Indexed {} symbols in {:.2}s (+{} ~{} -{} files)",
        result.symbols_added,
        start.elapsed().as_secs_f64(),
        result.files_added,
        result.files_modified,
        result.files_deleted
    );
    
    Ok(())
}

fn handle_unused(db_path: &str, _public_only: bool, _file_filter: Option<String>, 
                 _json_output: bool) -> Result<()> {
    println!("🗑️  Finding unused code...");
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_data::<CodeGraph>("graph")?.unwrap_or_default();
    
    // TODO: Implement actual unused code detection
    println!("Unused code detection not yet implemented");
    println!("Total symbols in index: {}", graph.get_all_symbols().count());
    
    Ok(())
}

fn handle_stats(db_path: &str, _detailed: bool, by_file: bool, by_type: bool) -> Result<()> {
    println!("📊 Project statistics:");
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_data::<CodeGraph>("graph")?.unwrap_or_default();
    
    let total_symbols = graph.get_all_symbols().count();
    println!("  Total symbols: {}", total_symbols);
    
    if by_type {
        let mut by_kind: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for symbol in graph.get_all_symbols() {
            *by_kind.entry(format!("{:?}", symbol.kind)).or_default() += 1;
        }
        
        println!("\n📈 By type:");
        let mut sorted: Vec<_> = by_kind.into_iter().collect();
        sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        
        for (kind, count) in sorted.iter().take(10) {
            println!("  {} {}: {}", 
                match kind.as_str() {
                    "Function" | "Method" => "🔧",
                    "Class" => "📦",
                    "Variable" | "Field" => "📝",
                    "Interface" => "🔌",
                    "Enum" => "📋",
                    _ => "❓",
                },
                kind, count
            );
        }
    }
    
    if by_file {
        let mut by_file: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for symbol in graph.get_all_symbols() {
            *by_file.entry(symbol.file_path.clone()).or_default() += 1;
        }
        
        println!("\n📁 Top files by symbol count:");
        let mut sorted: Vec<_> = by_file.into_iter().collect();
        sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        
        for (file, count) in sorted.iter().take(10) {
            println!("  {} - {} symbols", file, count);
        }
    }
    
    Ok(())
}

fn handle_export(db_path: &str, output: &str, format: &str, _include_refs: bool) -> Result<()> {
    use std::fs::File;
    use std::io::Write;
    
    println!("📤 Exporting to {} (format: {})", output, format);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_data::<CodeGraph>("graph")?.unwrap_or_default();
    
    match format {
        "json" => {
            let symbols: Vec<_> = graph.get_all_symbols().cloned().collect();
            let data = serde_json::json!({
                "symbols": symbols,
                "total": symbols.len(),
            });
            
            let mut file = File::create(output)?;
            file.write_all(serde_json::to_string_pretty(&data)?.as_bytes())?;
            
            println!("✅ Exported {} symbols to {}", symbols.len(), output);
        }
        _ => {
            println!("❌ Format '{}' not yet implemented. Supported: json", format);
        }
    }
    
    Ok(())
}