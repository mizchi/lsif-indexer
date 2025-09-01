use crate::differential_indexer::DifferentialIndexer;
use crate::git_diff::GitDiffDetector;
use crate::storage::IndexStorage;
use crate::reference_finder::ReferenceFinder;
use lsif_core::{CodeGraph, Location, Symbol, SymbolKind};
use serde_json;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;
use std::time::Instant;
use tracing::{debug, info};

const DEFAULT_INDEX_PATH: &str = ".lsif-index.db";
const MAX_CHANGES_DISPLAY: usize = 15;

#[derive(Parser)]
#[command(name = "lsif")]
#[command(about = "Fast code indexer and search tool")]
#[command(version)]
pub struct Cli {
    /// Database path (default: .lsif-index.db)
    #[arg(short = 'D', long = "db", global = true, env = "LSIF_DB")]
    pub database: Option<String>,

    /// Project root (default: current directory)
    #[arg(short = 'P', long = "project", global = true, env = "LSIF_PROJECT")]
    pub project_root: Option<String>,

    /// Disable automatic indexing
    #[arg(short = 'n', long = "no-index", global = true)]
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
    Calls {
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

    /// Search symbols [aliases: search, s]
    #[command(visible_alias = "search", visible_alias = "s")]
    Find {
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
        
        /// Use fallback indexer only (faster but less accurate)
        #[arg(short = 'F', long = "fallback")]
        fallback_only: bool,
        
        /// Number of parallel threads (0 = auto)
        #[arg(short = 'j', long = "jobs", default_value = "0")]
        threads: usize,
        
        /// Show progress
        #[arg(short = 'p', long = "progress")]
        show_progress: bool,
    },

    /// Show unused code [aliases: unused, u]
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
    Stats {
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

    /// Watch for changes and auto-index [aliases: watch, w]
    #[command(visible_alias = "watch", visible_alias = "w")]
    Watch {
        /// Polling interval in seconds
        #[arg(short = 'i', long = "interval", default_value = "2")]
        interval: u64,
        
        /// Run command on change
        #[arg(short = 'c', long = "command")]
        command: Option<String>,
    },

    /// Show type hierarchy [aliases: types, t]
    #[command(visible_alias = "types", visible_alias = "t")]
    Types {
        /// Type name or location
        #[arg(value_name = "TYPE")]
        type_name: String,
        
        /// Show implementations
        #[arg(short = 'i', long = "impls")]
        show_implementations: bool,
        
        /// Show hierarchy tree
        #[arg(short = 't', long = "tree")]
        show_tree: bool,
    },

    /// Clear and rebuild index [aliases: rebuild]
    #[command(visible_alias = "rebuild")]
    Rebuild {
        /// Confirm without prompting
        #[arg(short = 'y', long = "yes")]
        confirm: bool,
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
            Commands::Calls { symbol, incoming, outgoing, max_depth } => {
                let direction = if incoming {
                    "incoming"
                } else if outgoing {
                    "outgoing"
                } else {
                    "both"
                };
                handle_calls(&db_path, &symbol, direction, max_depth)?;
            }
            Commands::Find { query, fuzzy, symbol_type, path_pattern, max_results } => {
                handle_find(&db_path, &query, fuzzy, symbol_type, path_pattern, max_results)?;
            }
            Commands::Index { force, fallback_only, threads, show_progress } => {
                handle_index(&db_path, &project_root, force, fallback_only, threads, show_progress)?;
            }
            Commands::Unused { public_only, file_filter, json_output } => {
                handle_unused(&db_path, public_only, file_filter, json_output)?;
            }
            Commands::Stats { detailed, by_file, by_type } => {
                handle_stats(&db_path, detailed, by_file, by_type)?;
            }
            Commands::Export { output, format, include_refs } => {
                handle_export(&db_path, &output, &format, include_refs)?;
            }
            Commands::Watch { interval, command } => {
                handle_watch(&db_path, &project_root, interval, command)?;
            }
            Commands::Types { type_name, show_implementations, show_tree } => {
                handle_types(&db_path, &type_name, show_implementations, show_tree)?;
            }
            Commands::Rebuild { confirm } => {
                handle_rebuild(&db_path, &project_root, confirm)?;
            }
        }

        Ok(())
    }
}

// Helper functions

/// Parse location format: file.rs:10:5 or file.rs
fn parse_location(location: &str) -> Result<(String, usize, usize)> {
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
    let detector = GitDiffDetector::new(Path::new(project_root));
    if let Ok(changed_files) = detector.get_changed_files() {
        return Ok(!changed_files.is_empty());
    }
    
    Ok(false)
}

/// Quick incremental index
fn quick_index(db_path: &str, project_root: &str) -> Result<()> {
    let start = Instant::now();
    println!("⚡ Quick indexing...");
    
    let mut indexer = DifferentialIndexer::new(db_path, Path::new(project_root))?;
    let result = indexer.index_differential()?;
    
    if result.files_added + result.files_modified + result.files_removed > 0 {
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
fn handle_definition(db_path: &str, location: &str, show_all: bool) -> Result<()> {
    let (file, line, column) = parse_location(location)?;
    
    println!("🔍 Finding definition at {}:{}:{}", file, line, column);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_graph()?;
    
    let target_location = Location {
        path: file.clone(),
        start_line: line,
        start_column: column,
        end_line: line,
        end_column: column,
    };
    
    if let Some(definitions) = graph.find_definition(&target_location) {
        if definitions.is_empty() {
            println!("❌ No definition found");
        } else if !show_all && definitions.len() > 1 {
            println!("📍 {} (multiple definitions, use -a to see all)", format_location(&definitions[0]));
        } else {
            for def in definitions {
                println!("📍 {}", format_location(&def));
            }
        }
    } else {
        println!("❌ No definition found at this location");
    }
    
    Ok(())
}

fn format_location(loc: &Location) -> String {
    format!("{}:{}:{}", loc.path, loc.start_line, loc.start_column)
}

fn handle_references(db_path: &str, location: &str, include_defs: bool, group: bool) -> Result<()> {
    let (file, line, column) = parse_location(location)?;
    
    println!("🔗 Finding references for {}:{}:{}", file, line, column);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_graph()?;
    
    let target_location = Location {
        path: file.clone(),
        start_line: line,
        start_column: column,
        end_line: line,
        end_column: column,
    };
    
    let finder = ReferenceFinder::new(&graph);
    let refs = finder.find_references(&target_location)?;
    
    if refs.is_empty() {
        println!("❌ No references found");
        return Ok(());
    }
    
    println!("Found {} references", refs.len());
    
    if group {
        // Group by file
        let mut by_file: std::collections::HashMap<String, Vec<Location>> = std::collections::HashMap::new();
        for r in refs {
            by_file.entry(r.path.clone()).or_default().push(r);
        }
        
        for (file, locs) in by_file {
            println!("\n📁 {}:", file);
            for loc in locs {
                println!("  └─ line {}, col {}", loc.start_line, loc.start_column);
            }
        }
    } else {
        for r in refs {
            println!("  📌 {}", format_location(&r));
        }
    }
    
    Ok(())
}

fn handle_calls(db_path: &str, symbol: &str, direction: &str, depth: usize) -> Result<()> {
    use crate::call_hierarchy_cmd::CallHierarchyCommand;
    
    println!("📞 Analyzing call hierarchy for {} ({})", symbol, direction);
    
    let cmd = CallHierarchyCommand::new(db_path, symbol, direction, depth);
    cmd.execute()?;
    
    Ok(())
}

fn handle_find(db_path: &str, query: &str, fuzzy: bool, symbol_type: Option<String>, 
               path_pattern: Option<String>, max_results: usize) -> Result<()> {
    let mode = if fuzzy { "fuzzy" } else { "exact" };
    println!("🔍 Searching for '{}' ({})", query, mode);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_graph()?;
    
    let mut results = Vec::new();
    
    for (_, symbol) in graph.symbols() {
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
            if !symbol.location.path.contains(pattern) {
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
            println!("  🔹 {} ({}) - {}", symbol.name, kind, format_location(&symbol.location));
        }
    }
    
    Ok(())
}

fn handle_index(db_path: &str, project_root: &str, force: bool, fallback_only: bool,
                threads: usize, show_progress: bool) -> Result<()> {
    use crate::indexer::Indexer;
    use std::time::Instant;
    
    if force {
        println!("🔄 Force reindexing project...");
        if Path::new(db_path).exists() {
            std::fs::remove_file(db_path)?;
        }
    } else {
        println!("📇 Indexing project...");
    }
    
    let start = Instant::now();
    
    if !force && Path::new(db_path).exists() {
        // Try differential indexing first
        let mut indexer = DifferentialIndexer::new(db_path, Path::new(project_root))?;
        let result = indexer.index_differential()?;
        
        if result.files_added + result.files_modified + result.files_removed > 0 {
            println!(
                "✅ Incremental index completed in {:.2}s (+{} ~{} -{} files)",
                start.elapsed().as_secs_f64(),
                result.files_added,
                result.files_modified,
                result.files_deleted
            );
        } else {
            println!("✅ Index is up to date");
        }
    } else {
        // Full index using differential indexer
        let mut diff_indexer = DifferentialIndexer::new(db_path, Path::new(project_root))?;
        
        // TODO: Add support for fallback_only and threads options
        
        let result = diff_indexer.full_reindex()?;
        
        println!(
            "✅ Indexed {} symbols from {} files in {:.2}s",
            result.symbols_added,
            result.files_added,
            start.elapsed().as_secs_f64()
        );
    }
    
    Ok(())
}

fn handle_unused(db_path: &str, public_only: bool, file_filter: Option<String>, 
                 json_output: bool) -> Result<()> {
    println!("🗑️  Finding unused code...");
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_graph()?;
    
    let unused = graph.find_unused_symbols();
    
    let mut filtered: Vec<Symbol> = unused.into_iter()
        .filter(|s| {
            if public_only && !s.is_public {
                return false;
            }
            if let Some(ref filter) = file_filter {
                return s.location.path.contains(filter);
            }
            true
        })
        .collect();
    
    filtered.sort_by(|a, b| a.location.path.cmp(&b.location.path));
    
    if json_output {
        let paths: Vec<String> = filtered.iter().map(|s| format_location(&s.location)).collect();
        println!("{}", serde_json::to_string_pretty(&paths)?);
    } else if filtered.is_empty() {
        println!("✅ No unused code found");
    } else {
        println!("Found {} unused symbols:", filtered.len());
        for symbol in filtered {
            let visibility = if symbol.is_public { "public" } else { "private" };
            println!("  ⚠️  {} ({}, {}) - {}", 
                symbol.name, 
                format!("{:?}", symbol.kind).to_lowercase(),
                visibility,
                format_location(&symbol.location)
            );
        }
    }
    
    Ok(())
}

fn handle_stats(db_path: &str, detailed: bool, by_file: bool, by_type: bool) -> Result<()> {
    println!("📊 Project statistics:");
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_graph()?;
    
    let total_symbols = graph.symbols().count();
    let total_files = graph.files().count();
    
    println!("  Total files: {}", total_files);
    println!("  Total symbols: {}", total_symbols);
    
    if by_type {
        let mut by_kind: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (_, symbol) in graph.symbols() {
            *by_kind.entry(format!("{:?}", symbol.kind)).or_default() += 1;
        }
        
        println!("\n📈 By type:");
        let mut sorted: Vec<_> = by_kind.into_iter().collect();
        sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        
        for (kind, count) in sorted {
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
        for (_, symbol) in graph.symbols() {
            *by_file.entry(symbol.location.path.clone()).or_default() += 1;
        }
        
        println!("\n📁 Top files by symbol count:");
        let mut sorted: Vec<_> = by_file.into_iter().collect();
        sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        
        for (file, count) in sorted.iter().take(10) {
            println!("  {} - {} symbols", file, count);
        }
    }
    
    if detailed {
        let refs_count = graph.references().count();
        let unused = graph.find_unused_symbols().len();
        
        println!("\n📌 Detailed stats:");
        println!("  Total references: {}", refs_count);
        println!("  Unused symbols: {}", unused);
        println!("  Average symbols per file: {:.1}", total_symbols as f64 / total_files as f64);
    }
    
    Ok(())
}

fn handle_export(db_path: &str, output: &str, format: &str, include_refs: bool) -> Result<()> {
    use std::fs::File;
    use std::io::Write;
    
    println!("📤 Exporting to {} (format: {})", output, format);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_graph()?;
    
    match format {
        "json" => {
            let mut data = serde_json::json!({
                "symbols": graph.symbols().map(|(_, s)| s).collect::<Vec<_>>(),
                "files": graph.files().collect::<Vec<_>>(),
            });
            
            if include_refs {
                data["references"] = serde_json::json!(graph.references().collect::<Vec<_>>());
            }
            
            let mut file = File::create(output)?;
            file.write_all(serde_json::to_string_pretty(&data)?.as_bytes())?;
            
            println!("✅ Exported {} symbols to {}", graph.symbols().count(), output);
        }
        "lsif" => {
            // TODO: Implement LSIF export
            println!("❌ LSIF export not yet implemented");
        }
        "dot" => {
            // TODO: Implement Graphviz DOT export
            println!("❌ DOT export not yet implemented");
        }
        _ => {
            println!("❌ Unknown format: {}. Supported: json, lsif, dot", format);
        }
    }
    
    Ok(())
}

fn handle_watch(db_path: &str, project_root: &str, interval: u64, command: Option<String>) -> Result<()> {
    use std::thread;
    use std::time::Duration;
    
    println!("👁️  Watching for changes (interval: {}s)", interval);
    println!("Press Ctrl+C to stop watching");
    
    let mut last_check = std::time::SystemTime::now();
    
    loop {
        thread::sleep(Duration::from_secs(interval));
        
        if should_auto_index(db_path, project_root)? {
            println!("\n🔄 Changes detected, reindexing...");
            quick_index(db_path, project_root)?;
            
            if let Some(ref cmd) = command {
                println!("▶️  Running command: {}", cmd);
                let output = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .output()?;
                
                if !output.status.success() {
                    eprintln!("⚠️  Command failed with status: {}", output.status);
                }
            }
        }
        
        last_check = std::time::SystemTime::now();
    }
}

fn handle_types(db_path: &str, type_name: &str, show_impls: bool, show_tree: bool) -> Result<()> {
    println!("🏷️  Type hierarchy for {}", type_name);
    
    let storage = IndexStorage::open(db_path)?;
    let graph = storage.load_graph()?;
    
    // Find the type symbol
    let type_symbol = graph.symbols()
        .find(|(_, s)| s.name == type_name && matches!(s.kind, SymbolKind::Class | SymbolKind::Interface))
        .map(|(_, s)| s.clone());
    
    if let Some(symbol) = type_symbol {
        println!("Found {} '{}' at {}", 
            format!("{:?}", symbol.kind).to_lowercase(),
            symbol.name,
            format_location(&symbol.location)
        );
        
        if show_impls {
            let impls = graph.find_implementations(&symbol.name);
            if !impls.is_empty() {
                println!("\n📦 Implementations:");
                for impl_sym in impls {
                    println!("  └─ {} - {}", impl_sym.name, format_location(&impl_sym.location));
                }
            } else {
                println!("\n  No implementations found");
            }
        }
        
        if show_tree {
            println!("\n🌳 Type hierarchy:");
            println!("  {} (root)", symbol.name);
            // TODO: Implement full hierarchy traversal
            println!("  └─ (hierarchy traversal not yet implemented)");
        }
    } else {
        println!("❌ Type '{}' not found", type_name);
    }
    
    Ok(())
}

fn handle_rebuild(db_path: &str, project_root: &str, confirm: bool) -> Result<()> {
    use std::io::{self, Write};
    
    if !confirm {
        print!("⚠️  This will delete the existing index. Continue? (y/N): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    
    println!("🔨 Rebuilding index from scratch...");
    
    if Path::new(db_path).exists() {
        std::fs::remove_file(db_path)?;
    }
    
    handle_index(db_path, project_root, true, false, 0, true)?;
    
    Ok(())
}