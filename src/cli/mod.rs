pub mod cached_storage;
pub mod call_hierarchy_cmd;
pub mod differential_indexer;
pub mod fuzzy_search;
pub mod generic_helpers;
pub mod git_diff;
pub mod incremental_storage;
pub mod indexer;
pub mod language_adapter;
pub mod minimal_language_adapter;
pub mod lsp_adapter;
pub mod lsp_client;
pub mod lsp_commands;
pub mod lsp_features;
pub mod lsp_indexer;
pub mod lsp_integration;
pub mod optimized_incremental;
pub mod parallel_storage;
pub mod reference_finder;
pub mod simple_cli;
pub mod storage;
pub mod ultra_fast_storage;

// Re-export commonly used types
pub use ultra_fast_storage::{MemoryPoolStorage, UltraFastStorage};

use self::lsp_adapter::{GenericLspClient, LspAdapter, RustAnalyzerAdapter};
use self::lsp_indexer::LspIndexer;
use self::storage::{IndexFormat, IndexMetadata, IndexStorage};
use crate::core::{generate_lsif, parse_lsif, CodeGraph};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use tracing::{debug, info};

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

    /// Trace definition chain for a symbol
    DefinitionChain {
        /// Index database path
        #[arg(short, long)]
        index: String,

        /// Symbol ID to trace
        #[arg(short, long)]
        symbol: String,

        /// Show all possible chains (for multiple definitions)
        #[arg(short, long)]
        all: bool,
    },

    /// Collect type-related symbols recursively
    TypeRelations {
        /// Index database path
        #[arg(short, long)]
        index: String,

        /// Type symbol ID to analyze
        #[arg(short, long)]
        type_symbol: String,

        /// Maximum recursion depth
        #[arg(short = 'd', long, default_value = "3")]
        max_depth: usize,

        /// Show type hierarchy (parents, children, siblings)
        #[arg(short = 'h', long)]
        hierarchy: bool,

        /// Group relations by type
        #[arg(short, long)]
        group: bool,
    },

    /// Execute a graph query pattern
    QueryPattern {
        /// Index database path
        #[arg(short, long)]
        index: String,

        /// Query pattern (Cypher-like syntax)
        #[arg(short, long)]
        pattern: String,

        /// Maximum results to show
        #[arg(short = 'l', long, default_value = "10")]
        limit: usize,
    },

    /// Index an entire project with enhanced reference tracking
    IndexProject {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        project: String,

        /// Output index database path
        #[arg(short, long)]
        output: String,

        /// Language (rust, typescript, python)
        #[arg(short, long, default_value = "rust")]
        language: String,
    },

    /// Differential index with Git diff detection
    DifferentialIndex {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        project: String,

        /// Output index database path
        #[arg(short, long)]
        output: String,

        /// Force full reindex
        #[arg(short, long)]
        force: bool,
    },

    /// Enhanced LSP commands
    #[command(subcommand)]
    Lsp(LspCommands),
}

#[derive(Subcommand)]
pub enum LspCommands {
    /// Get hover information at a specific location
    Hover {
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

    /// Get completions at a specific location
    Complete {
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

    /// Find implementations of a symbol
    Implementations {
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

    /// Find type definition of a symbol
    TypeDefinition {
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

    /// Rename a symbol
    Rename {
        /// File path
        #[arg(short, long)]
        file: String,

        /// Line number
        #[arg(short, long)]
        line: u32,

        /// Column number
        #[arg(short, long)]
        column: u32,

        /// New name
        #[arg(short, long)]
        new_name: String,
    },

    /// Get diagnostics for a file
    Diagnostics {
        /// File path
        #[arg(short, long)]
        file: String,
    },

    /// Index project with LSP integration
    IndexWithLsp {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        project: String,

        /// Output index database path
        #[arg(short, long)]
        output: String,
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
            Commands::Generate {
                source,
                output,
                language,
            } => {
                info!("Generating index from {} to {}", source, output);
                generate_index(&source, &output, language.as_deref())?;
            }
            Commands::Query {
                index,
                query_type,
                file,
                line,
                column,
            } => {
                info!(
                    "Querying {} for {} at {}:{}:{}",
                    index, query_type, file, line, column
                );
                query_index(&index, &query_type, &file, line, column)?;
            }
            Commands::CallHierarchy {
                index,
                symbol,
                direction,
                max_depth,
            } => {
                info!(
                    "Showing {} call hierarchy for {} (depth: {})",
                    direction, symbol, max_depth
                );
                // 自動差分インデックス実行（シンボル特化）
                ensure_index_updated_for_symbol(&index, &symbol)?;
                call_hierarchy_cmd::show_call_hierarchy(&index, &symbol, &direction, max_depth)?;
            }
            Commands::CallPaths {
                index,
                from,
                to,
                max_depth,
            } => {
                info!(
                    "Finding paths from {} to {} (max depth: {})",
                    from, to, max_depth
                );
                // 自動差分インデックス実行
                ensure_index_updated(&index)?;
                call_hierarchy_cmd::find_paths(&index, &from, &to, max_depth)?;
            }
            Commands::UpdateIncremental {
                index,
                source,
                detect_dead,
            } => {
                info!("Updating index {} incrementally with {}", index, source);
                update_incremental(&index, &source, detect_dead)?;
            }
            Commands::ShowDeadCode { index } => {
                info!("Showing dead code in index {}", index);
                // 自動差分インデックス実行
                ensure_index_updated(&index)?;
                show_dead_code(&index)?;
            }
            Commands::DefinitionChain { index, symbol, all } => {
                info!("Tracing definition chain for {} in index {}", symbol, index);
                // 自動差分インデックス実行（シンボル特化）
                ensure_index_updated_for_symbol(&index, &symbol)?;
                show_definition_chain(&index, &symbol, all)?;
            }
            Commands::TypeRelations {
                index,
                type_symbol,
                max_depth,
                hierarchy,
                group,
            } => {
                info!(
                    "Collecting type relations for {} in index {} (depth: {})",
                    type_symbol, index, max_depth
                );
                // 自動差分インデックス実行（シンボル特化）
                ensure_index_updated_for_symbol(&index, &type_symbol)?;
                show_type_relations(&index, &type_symbol, max_depth, hierarchy, group)?;
            }
            Commands::QueryPattern {
                index,
                pattern,
                limit,
            } => {
                info!("Executing query pattern: {}", pattern);
                // 自動差分インデックス実行
                ensure_index_updated(&index)?;
                execute_query_pattern(&index, &pattern, limit)?;
            }
            Commands::IndexProject {
                project,
                output,
                language,
            } => {
                info!("Indexing project {} with language {}", project, language);
                index_project(&project, &output, &language)?;
            }
            Commands::DifferentialIndex {
                project,
                output,
                force,
            } => {
                info!("Running differential index for project {}", project);
                run_differential_index(&project, &output, force)?;
            }
            Commands::Lsp(lsp_cmd) => {
                execute_lsp_command(lsp_cmd)?;
            }
        }
        Ok(())
    }
}

fn export_lsif(index_path: &str, output_path: &str) -> Result<()> {
    // Load the index
    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
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
        git_commit_hash: None,
        file_hashes: std::collections::HashMap::new(),
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
            let mut client = GenericLspClient::new(Box::new(RustAnalyzerAdapter))?;
            let syms = client.get_document_symbols(&file_uri)?;
            client.shutdown()?;
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
        git_commit_hash: None,
        file_hashes: std::collections::HashMap::new(),
    };

    storage.save_metadata(&metadata)?;
    storage.save_data("graph", &graph)?;

    info!(
        "Index generated at {} with {} symbols",
        output_path,
        graph.symbol_count()
    );
    Ok(())
}

fn query_index(
    index_path: &str,
    query_type: &str,
    file: &str,
    line: u32,
    _column: u32,
) -> Result<()> {
    // 自動差分インデックス実行
    ensure_index_updated(index_path)?;
    
    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;

    // Create a symbol ID based on file and line
    let symbol_id = format!("{file}#{line}:");

    match query_type {
        "definition" => {
            if let Some(def) = graph.find_definition(&symbol_id) {
                println!(
                    "Definition: {} at {}:{}",
                    def.name, def.file_path, def.range.start.line
                );
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
    use self::incremental_storage::IncrementalStorage;
    use crate::core::calculate_file_hash;

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
        println!("File {source_path} is up to date");
        return Ok(());
    }

    // Get symbols from LSP
    let mut client = GenericLspClient::new(Box::new(RustAnalyzerAdapter))?;
    let abs_path = fs::canonicalize(source_path)?;
    let file_uri = format!("file://{}", abs_path.display());
    let lsp_symbols = client.get_document_symbols(&file_uri)?;
    client.shutdown()?;

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
        println!(
            "\nDead code detected: {} symbols",
            result.dead_symbols.len()
        );
        for symbol_id in result.dead_symbols.iter().take(10) {
            println!("  - {symbol_id}");
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
        let mut by_file: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for symbol_id in dead_symbols {
            if let Some(path) = index.symbol_to_file.get(symbol_id) {
                by_file
                    .entry(path.to_string_lossy().to_string())
                    .or_default()
                    .push(symbol_id.clone());
            }
        }

        for (file, symbols) in by_file {
            println!("\n{file}:");
            for symbol in symbols.iter().take(5) {
                println!("  - {symbol}");
            }
            if symbols.len() > 5 {
                println!("  ... and {} more", symbols.len() - 5);
            }
        }
    }

    Ok(())
}

fn show_definition_chain(index_path: &str, symbol_id: &str, show_all: bool) -> Result<()> {
    use crate::core::{format_definition_chain, DefinitionChainAnalyzer};

    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;

    let analyzer = DefinitionChainAnalyzer::new(&graph);

    if show_all {
        // Show all possible definition chains
        let chains = analyzer.get_all_definition_chains(symbol_id);

        if chains.is_empty() {
            println!("No definition chains found for symbol: {symbol_id}");
        } else {
            println!("All definition chains for {symbol_id}:");
            for (i, chain) in chains.iter().enumerate() {
                println!("\n  Chain {}:", i + 1);
                println!("    {}", format_definition_chain(chain));
            }
            println!("\nTotal chains found: {}", chains.len());
        }
    } else {
        // Show single definition chain
        match analyzer.get_definition_chain(symbol_id) {
            Some(chain) => {
                println!("Definition chain for {symbol_id}:");
                println!("  {}", format_definition_chain(&chain));

                if chain.has_cycle {
                    println!("\n⚠️  Circular dependency detected!");
                }

                // Show ultimate source
                if let Some(ultimate) = analyzer.find_ultimate_source(symbol_id) {
                    println!("\nUltimate source:");
                    println!(
                        "  {} ({}:{})",
                        ultimate.name,
                        ultimate.file_path,
                        ultimate.range.start.line + 1
                    );

                    if let Some(doc) = &ultimate.documentation {
                        println!("  Documentation: {doc}");
                    }
                }
            }
            None => {
                println!("Symbol not found: {symbol_id}");
            }
        }
    }

    Ok(())
}

fn show_type_relations(
    index_path: &str,
    type_symbol_id: &str,
    max_depth: usize,
    show_hierarchy: bool,
    group_by_type: bool,
) -> Result<()> {
    use crate::core::{format_type_relations, TypeRelationsAnalyzer};

    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;

    let analyzer = TypeRelationsAnalyzer::new(&graph);

    // Collect type relations
    if let Some(relations) = analyzer.collect_type_relations(type_symbol_id, max_depth) {
        println!("{}", format_type_relations(&relations));

        // Show hierarchy if requested
        if show_hierarchy {
            let hierarchy = analyzer.find_type_hierarchy(type_symbol_id);

            println!("\nType Hierarchy:");
            if !hierarchy.parents.is_empty() {
                println!("  Parents ({}):", hierarchy.parents.len());
                for parent in hierarchy.parents.iter().take(5) {
                    println!("    - {} ({})", parent.name, parent.file_path);
                }
            }

            if !hierarchy.children.is_empty() {
                println!("  Children ({}):", hierarchy.children.len());
                for child in hierarchy.children.iter().take(5) {
                    println!("    - {} ({})", child.name, child.file_path);
                }
            }

            if !hierarchy.siblings.is_empty() {
                println!("  Siblings ({}):", hierarchy.siblings.len());
                for sibling in hierarchy.siblings.iter().take(5) {
                    println!("    - {} ({})", sibling.name, sibling.file_path);
                }
            }
        }

        // Group by relation type if requested
        if group_by_type {
            let groups = analyzer.group_relations_by_type(type_symbol_id);

            println!("\nRelations grouped by type:");

            if !groups.definitions.is_empty() {
                println!("  Definitions ({}):", groups.definitions.len());
                for def in groups.definitions.iter().take(3) {
                    println!("    - {} ({})", def.name, def.file_path);
                }
            }

            if !groups.references.is_empty() {
                println!("  References ({}):", groups.references.len());
                for reference in groups.references.iter().take(3) {
                    println!("    - {} ({})", reference.name, reference.file_path);
                }
            }

            if !groups.variables_of_type.is_empty() {
                println!(
                    "  Variables of this type ({}):",
                    groups.variables_of_type.len()
                );
                for var in groups.variables_of_type.iter().take(3) {
                    println!("    - {} ({})", var.name, var.file_path);
                }
            }

            if !groups.functions_returning_type.is_empty() {
                println!(
                    "  Functions returning this type ({}):",
                    groups.functions_returning_type.len()
                );
                for func in groups.functions_returning_type.iter().take(3) {
                    println!("    - {} ({})", func.name, func.file_path);
                }
            }

            if !groups.fields_of_type.is_empty() {
                println!("  Fields of this type ({}):", groups.fields_of_type.len());
                for field in groups.fields_of_type.iter().take(3) {
                    println!("    - {} ({})", field.name, field.file_path);
                }
            }
        }

        // Find all references recursively
        let all_refs = analyzer.find_all_type_references(type_symbol_id, max_depth);
        println!("\nTotal references found (recursive): {}", all_refs.len());
    } else {
        println!("Symbol not found or not a type: {type_symbol_id}");
    }

    Ok(())
}

fn execute_query_pattern(index_path: &str, pattern: &str, limit: usize) -> Result<()> {
    use crate::core::{format_query_results, QueryEngine, QueryParser};

    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;

    // Parse the query pattern
    let query_pattern =
        QueryParser::parse(pattern).map_err(|e| anyhow::anyhow!("Failed to parse query: {}", e))?;

    // Execute the query
    let engine = QueryEngine::new(&graph);
    let results = engine.execute(&query_pattern);

    // Format and display results
    if results.matches.is_empty() {
        println!("No matches found for pattern: {pattern}");
    } else {
        let total_matches = results.matches.len();
        let mut limited_results = results;
        if limited_results.matches.len() > limit {
            limited_results.matches.truncate(limit);
            println!("Showing first {limit} of {total_matches} matches\n");
        }

        println!("{}", format_query_results(&limited_results));
    }

    Ok(())
}

fn index_project(project_path: &str, output_path: &str, language: &str) -> Result<()> {
    use self::indexer::Indexer;
    use self::lsp_adapter::{RustAnalyzerAdapter, TypeScriptAdapter};
    use std::path::Path;

    let project_root = Path::new(project_path);

    // Create appropriate language adapter
    let adapter: Box<dyn LspAdapter> = match language {
        "rust" => Box::new(RustAnalyzerAdapter),
        "typescript" | "ts" | "javascript" | "js" => Box::new(TypeScriptAdapter),
        _ => {
            return Err(anyhow::anyhow!("Unsupported language: {}", language));
        }
    };

    // Create enhanced indexer and index the project
    let mut indexer = Indexer::new();
    indexer.index_project(project_root, adapter)?;

    // Save the graph
    let graph = indexer.into_graph();
    let storage = IndexStorage::open(output_path)?;

    let metadata = IndexMetadata {
        format: IndexFormat::Lsif,
        version: "2.0.0".to_string(), // Version 2 with references
        created_at: chrono::Utc::now(),
        project_root: project_root.canonicalize()?.to_string_lossy().to_string(),
        files_count: 0, // TODO: Track actual file count
        symbols_count: graph.symbol_count(),
        git_commit_hash: None,
        file_hashes: std::collections::HashMap::new(),
    };

    storage.save_metadata(&metadata)?;
    storage.save_data("graph", &graph)?;

    info!(
        "Project indexed at {} with {} symbols",
        output_path,
        graph.symbol_count()
    );

    Ok(())
}

fn execute_lsp_command(command: LspCommands) -> Result<()> {
    use self::lsp_integration::LspIntegration;
    use std::path::PathBuf;

    match command {
        LspCommands::Hover { file, line, column } => {
            let mut lsp = LspIntegration::new(PathBuf::from("."))?;
            let runtime = tokio::runtime::Runtime::new()?;
            let hover_info =
                runtime.block_on(lsp.get_hover_info(&PathBuf::from(&file), line, column))?;
            println!("{hover_info}");
        }
        LspCommands::Complete { file, line, column } => {
            let mut lsp = LspIntegration::new(PathBuf::from("."))?;
            let runtime = tokio::runtime::Runtime::new()?;
            let completions =
                runtime.block_on(lsp.get_completions(&PathBuf::from(&file), line, column))?;

            println!("Completions:");
            for (i, item) in completions.iter().take(20).enumerate() {
                println!(
                    "  {}. {} - {}",
                    i + 1,
                    item.label,
                    item.detail.as_deref().unwrap_or("")
                );
            }
            if completions.len() > 20 {
                println!("  ... and {} more", completions.len() - 20);
            }
        }
        LspCommands::Implementations { file, line, column } => {
            let mut lsp = LspIntegration::new(PathBuf::from("."))?;
            let runtime = tokio::runtime::Runtime::new()?;
            let implementations =
                runtime.block_on(lsp.find_implementations(&PathBuf::from(&file), line, column))?;

            println!("Implementations:");
            for impl_loc in implementations {
                println!(
                    "  - {}:{}:{}",
                    impl_loc.uri.path(),
                    impl_loc.range.start.line + 1,
                    impl_loc.range.start.character + 1
                );
            }
        }
        LspCommands::TypeDefinition { file, line, column } => {
            let mut lsp = LspIntegration::new(PathBuf::from("."))?;
            let runtime = tokio::runtime::Runtime::new()?;
            let type_defs =
                runtime.block_on(lsp.find_type_definition(&PathBuf::from(&file), line, column))?;

            println!("Type Definitions:");
            for type_def in type_defs {
                println!(
                    "  - {}:{}:{}",
                    type_def.uri.path(),
                    type_def.range.start.line + 1,
                    type_def.range.start.character + 1
                );
            }
        }
        LspCommands::Rename {
            file,
            line,
            column,
            new_name,
        } => {
            let mut lsp = LspIntegration::new(PathBuf::from("."))?;
            let runtime = tokio::runtime::Runtime::new()?;
            let workspace_edit = runtime.block_on(lsp.rename_symbol(
                &PathBuf::from(&file),
                line,
                column,
                new_name,
            ))?;

            println!("Rename edits:");
            if let Some(changes) = workspace_edit.changes {
                for (uri, edits) in changes {
                    println!("  File: {}", uri.path());
                    for edit in edits {
                        println!(
                            "    - Line {}: ... -> {}",
                            edit.range.start.line + 1,
                            edit.new_text
                        );
                    }
                }
            }
        }
        LspCommands::Diagnostics { file } => {
            let mut lsp = LspIntegration::new(PathBuf::from("."))?;
            let runtime = tokio::runtime::Runtime::new()?;
            let diagnostics = runtime.block_on(lsp.get_diagnostics(&PathBuf::from(&file)))?;

            if diagnostics.is_empty() {
                println!("No diagnostics found.");
            } else {
                println!("Diagnostics:");
                for diag in diagnostics {
                    println!(
                        "  - [{}] Line {}: {}",
                        format!(
                            "{:?}",
                            diag.severity
                                .unwrap_or(lsp_types::DiagnosticSeverity::INFORMATION)
                        )
                        .to_lowercase(),
                        diag.range.start.line + 1,
                        diag.message
                    );
                }
            }
        }
        LspCommands::IndexWithLsp { project, output } => {
            let mut lsp = LspIntegration::new(PathBuf::from(&project))?;
            let runtime = tokio::runtime::Runtime::new()?;

            // Create enhanced index
            let mut enhanced_index = crate::core::enhanced_graph::EnhancedIndex::default();

            runtime.block_on(lsp.enhance_index(&mut enhanced_index))?;

            // Save to storage
            let storage = IndexStorage::open(&output)?;
            let metadata = IndexMetadata {
                format: IndexFormat::Lsif,
                version: "3.0.0".to_string(), // Version 3 with LSP integration
                created_at: chrono::Utc::now(),
                project_root: std::fs::canonicalize(&project)?
                    .to_string_lossy()
                    .to_string(),
                files_count: 0, // TODO: Track actual file count
                symbols_count: enhanced_index.symbols.len(),
                git_commit_hash: None,
                file_hashes: std::collections::HashMap::new(),
            };

            storage.save_metadata(&metadata)?;
            storage.save_data("enhanced_index", &enhanced_index)?;

            println!("Project indexed with LSP integration:");
            println!("  Symbols: {}", enhanced_index.symbols.len());
            println!("  References: {}", enhanced_index.references.len());
            println!("  Call graph edges: {}", enhanced_index.call_graph.len());
            println!("  Type relations: {}", enhanced_index.type_relations.len());
        }
    }

    Ok(())
}

/// インデックスが最新かチェックし、必要に応じて差分インデックスを実行
fn ensure_index_updated(index_path: &str) -> Result<()> {
    use self::differential_indexer::DifferentialIndexer;
    use self::git_diff::GitDiffDetector;
    use std::path::Path;
    use std::time::Instant;
    
    // インデックスファイルが存在しない場合はスキップ
    if !Path::new(index_path).exists() {
        info!("Index file does not exist, skipping auto-update");
        return Ok(());
    }
    
    // ストレージからメタデータを読み込み
    let storage = IndexStorage::open(index_path)?;
    let metadata = storage.load_metadata()?;
    
    if metadata.is_none() {
        info!("No metadata found in index, skipping auto-update");
        return Ok(());
    }
    
    let metadata = metadata.unwrap();
    
    // プロジェクトルートを取得
    let project_root = Path::new(&metadata.project_root);
    if !project_root.exists() {
        // 相対パスの可能性があるので、現在のディレクトリも試す
        let current_dir = std::env::current_dir()?;
        if !current_dir.exists() {
            info!("Project root not found, skipping auto-update");
            return Ok(());
        }
    }
    
    // Git差分検知を使用して変更をチェック
    let mut detector = GitDiffDetector::new(project_root)?;
    
    // 前回のGitコミットハッシュと比較
    let current_commit = detector.get_head_commit();
    let needs_update = if let Some(ref saved_commit) = metadata.git_commit_hash {
        current_commit.as_ref() != Some(saved_commit)
    } else {
        // Gitコミットハッシュがない場合はファイルハッシュで比較
        let changes = detector.detect_changes_since(None)?;
        !changes.is_empty()
    };
    
    if needs_update {
        info!("Changes detected, running differential index...");
        let start = Instant::now();
        
        // 差分インデックスを実行
        let mut indexer = DifferentialIndexer::new(index_path, project_root)?;
        let result = indexer.index_differential()?;
        
        let elapsed = start.elapsed();
        info!(
            "Differential index completed in {:.2}s: {} files modified, {} symbols updated",
            elapsed.as_secs_f64(),
            result.files_modified,
            result.symbols_updated
        );
        
        // 変更があった場合は警告を表示
        if result.files_modified > 0 || result.symbols_updated > 0 {
            println!(
                "⚡ Auto-updated index: {} files changed, {} symbols updated ({:.2}s)",
                result.files_modified,
                result.symbols_updated,
                elapsed.as_secs_f64()
            );
        }
    } else {
        debug!("No changes detected, using existing index");
    }
    
    Ok(())
}

/// コールヒエラルキー表示前の自動更新チェック
fn ensure_index_updated_for_symbol(index_path: &str, symbol: &str) -> Result<()> {
    // まず通常の差分インデックスを実行
    ensure_index_updated(index_path)?;
    
    // シンボルが存在するファイルの特定と追加チェック（オプション）
    let storage = IndexStorage::open(index_path)?;
    if let Ok(Some(graph)) = storage.load_data::<CodeGraph>("graph") {
        if let Some(symbol_info) = graph.find_symbol(symbol) {
            debug!(
                "Symbol '{}' found in file: {}",
                symbol, symbol_info.file_path
            );
            
            // そのファイルが最近変更されていれば、関連ファイルも含めて再インデックス
            // （ここは将来的な拡張ポイント）
        }
    }
    
    Ok(())
}

/// 差分インデックスコマンドの実行
fn run_differential_index(project_path: &str, output_path: &str, force: bool) -> Result<()> {
    use self::differential_indexer::DifferentialIndexer;
    use std::path::Path;
    use std::time::Instant;
    
    let project = Path::new(project_path);
    let start = Instant::now();
    
    let mut indexer = DifferentialIndexer::new(output_path, project)?;
    
    let result = if force {
        info!("Forcing full reindex...");
        indexer.full_reindex()?
    } else {
        info!("Running differential index...");
        indexer.index_differential()?
    };
    
    let elapsed = start.elapsed();
    
    println!("Differential index completed in {:.2}s:", elapsed.as_secs_f64());
    println!("  Files added: {}", result.files_added);
    println!("  Files modified: {}", result.files_modified);
    println!("  Files deleted: {}", result.files_deleted);
    println!("  Symbols added: {}", result.symbols_added);
    println!("  Symbols updated: {}", result.symbols_updated);
    println!("  Symbols deleted: {}", result.symbols_deleted);
    
    if result.files_added == 0 && result.files_modified == 0 && result.files_deleted == 0 {
        println!("No changes detected - index is up to date!");
    }
    
    Ok(())
}
// Test 1755525843
