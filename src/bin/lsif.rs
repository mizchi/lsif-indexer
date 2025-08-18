use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use glob::glob;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use lsif_indexer::cli::{
    cached_storage::CachedIndexStorage, parallel_storage::ParallelIndexStorage,
    storage::IndexStorage, MemoryPoolStorage,
};
use lsif_indexer::core::Symbol;

#[derive(Parser)]
#[command(name = "lsif-indexer")]
#[command(
    version,
    about = "High-performance code indexing tool with LSP support"
)]
#[command(long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Files to index (glob pattern)
    #[arg(short, long, default_value = "**/*.rs")]
    files: String,

    /// Output database path
    #[arg(short, long, default_value = "./index.db")]
    output: PathBuf,

    /// LSP binary to use (auto-detect if not specified)
    #[arg(short, long)]
    bin: Option<String>,

    /// Language (auto-detect from files if not specified)
    #[arg(short, long)]
    language: Option<Language>,

    /// Enable parallel processing (uses memory pool storage by default)
    #[arg(short, long, default_value_t = true)]
    parallel: bool,

    /// Enable caching (fallback if parallel is disabled)
    #[arg(short, long, default_value_t = false)]
    cache: bool,

    /// Verbose output
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Number of threads (0 = auto)
    #[arg(short = 't', long, default_value_t = 0)]
    threads: usize,

    /// Batch size for processing
    #[arg(short = 'B', long, default_value_t = 100)]
    batch_size: usize,

    /// Show progress bar
    #[arg(short = 'P', long, default_value_t = true)]
    progress: bool,

    /// Incremental update (only process changed files)
    #[arg(short, long, default_value_t = false)]
    incremental: bool,

    /// Exclude patterns (can be specified multiple times)
    #[arg(short = 'e', long)]
    exclude: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Index files and create database
    Index {
        /// Files to index (glob pattern)
        #[arg(default_value = "**/*.rs")]
        files: String,

        /// Output database path
        #[arg(short, long, default_value = "./index.db")]
        output: PathBuf,
    },

    /// Query the index
    Query {
        /// Database path
        #[arg(short, long, default_value = "./index.db")]
        db: PathBuf,

        /// Query type
        #[command(subcommand)]
        query: QueryType,
    },

    /// Show statistics about the index
    Stats {
        /// Database path
        #[arg(short, long, default_value = "./index.db")]
        db: PathBuf,
    },

    /// List supported languages and LSP servers
    List,

    /// Watch files and update index automatically
    Watch {
        /// Files to watch (glob pattern)
        #[arg(default_value = "**/*.rs")]
        files: String,

        /// Database path
        #[arg(short, long, default_value = "./index.db")]
        db: PathBuf,
    },

    /// Advanced LSP functionality
    #[command(subcommand)]
    Lsp(lsif_indexer::cli::lsp_commands::LspSubcommand),

    /// Search symbols in the index
    Search {
        /// Search query (symbol name or pattern)
        query: String,

        /// Database path
        #[arg(short, long, default_value = "./index.db")]
        db: PathBuf,

        /// Filter by symbol type (function, struct, enum, etc.)
        #[arg(short = 't', long)]
        symbol_type: Option<String>,

        /// Filter by file pattern
        #[arg(short = 'f', long)]
        file: Option<String>,

        /// Show detailed information
        #[arg(short = 'd', long)]
        detailed: bool,
    },

    /// Find symbol definition or references
    Find {
        /// What to find
        #[command(subcommand)]
        what: FindType,

        /// Database path
        #[arg(short, long, default_value = "./index.db")]
        db: PathBuf,
    },

    /// Interactive mode for exploring the index
    Interactive {
        /// Database path
        #[arg(short, long, default_value = "./index.db")]
        db: PathBuf,
    },

    /// Perform differential indexing (only index changed files)
    Diff {
        /// Database path
        #[arg(short, long, default_value = "./index.db")]
        db: PathBuf,

        /// Force full reindex
        #[arg(short, long)]
        full: bool,

        /// Show detailed progress
        #[arg(short, long)]
        verbose: bool,
    },

    /// Restore index from saved metadata (useful after git operations)
    Restore {
        /// Database path
        #[arg(short, long, default_value = "./index.db")]
        db: PathBuf,

        /// Target git commit to restore to
        #[arg(short, long)]
        commit: Option<String>,

        /// Show what would be done without actually doing it
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum FindType {
    /// Find symbol definition
    Definition {
        /// Symbol name
        symbol: String,
    },

    /// Find symbol references
    References {
        /// Symbol name
        symbol: String,
    },

    /// Find symbol by location
    At {
        /// File path
        file: PathBuf,
        /// Line number
        line: u32,
        /// Column number
        column: u32,
    },
}

#[derive(Subcommand)]
enum QueryType {
    /// Find definition
    Definition {
        /// File path
        file: PathBuf,
        /// Line number
        line: u32,
        /// Column number
        column: u32,
    },

    /// Find references
    References {
        /// Symbol ID or name
        symbol: String,
    },

    /// Show call hierarchy
    CallHierarchy {
        /// Function name
        function: String,
        /// Maximum depth
        #[arg(short, long, default_value_t = 3)]
        depth: usize,
    },

    /// Find dead code
    DeadCode,

    /// Show type relations
    TypeRelations {
        /// Type name
        type_name: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Java,
    Cpp,
}

impl Language {
    fn to_lsp_binary(&self) -> &str {
        match self {
            Language::Rust => "rust-analyzer",
            Language::TypeScript | Language::JavaScript => "typescript-language-server",
            Language::Python => "pylsp",
            Language::Go => "gopls",
            Language::Java => "jdtls",
            Language::Cpp => "clangd",
        }
    }

    fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Language::Rust),
            "ts" | "tsx" => Some(Language::TypeScript),
            "js" | "jsx" => Some(Language::JavaScript),
            "py" => Some(Language::Python),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            "cpp" | "cc" | "cxx" | "hpp" | "h" => Some(Language::Cpp),
            _ => None,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Set thread pool size
    if cli.threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(cli.threads)
            .build_global()
            .context("Failed to set thread pool size")?;
    }

    match cli.command {
        Some(Commands::Index {
            ref files,
            ref output,
        }) => {
            index_files(&files, &output, &cli)?;
        }
        Some(Commands::Query { db, query }) => {
            execute_query(&db, query)?;
        }
        Some(Commands::Stats { db }) => {
            show_stats(&db)?;
        }
        Some(Commands::List) => {
            list_languages();
        }
        Some(Commands::Watch { ref files, ref db }) => {
            watch_files(&files, &db, &cli)?;
        }
        Some(Commands::Lsp(lsp_cmd)) => {
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(async {
                let lsp_command = lsif_indexer::cli::lsp_commands::LspCommand { command: lsp_cmd };
                lsp_command.execute().await
            })?;
        }
        Some(Commands::Search {
            query,
            db,
            symbol_type,
            file,
            detailed,
        }) => {
            execute_search(&db, &query, symbol_type, file, detailed)?;
        }
        Some(Commands::Find { what, db }) => {
            execute_find(&db, what)?;
        }
        Some(Commands::Interactive { db }) => {
            run_interactive_mode(&db)?;
        }
        Some(Commands::Diff { db, full, verbose }) => {
            execute_differential_index(&db, full, verbose)?;
        }
        Some(Commands::Restore {
            db,
            commit,
            dry_run,
        }) => {
            #[path = "../lsif_restore.rs"]
            mod lsif_restore;
            lsif_restore::execute_restore(&db, commit, dry_run)?;
        }
        None => {
            // Default action: index files
            index_files(&cli.files, &cli.output, &cli)?;
        }
    }

    Ok(())
}

fn index_files(pattern: &str, output: &Path, cli: &Cli) -> Result<()> {
    let start = Instant::now();

    // Collect files
    let files = collect_files(pattern, &cli.exclude)?;

    if files.is_empty() {
        warn!("No files found matching pattern: {}", pattern);
        return Ok(());
    }

    info!("Found {} files to index", files.len());

    // Detect or use specified language
    let language = cli
        .language
        .or_else(|| {
            files
                .first()
                .and_then(|f| f.extension())
                .and_then(|ext| ext.to_str())
                .and_then(Language::from_extension)
        })
        .context("Could not detect language")?;

    // Get LSP binary
    let lsp_binary = cli
        .bin
        .as_deref()
        .unwrap_or_else(|| language.to_lsp_binary());

    info!("Using LSP: {} for language: {:?}", lsp_binary, language);

    // Create storage - Memory pool storage is now the default for best performance
    let storage = if cli.parallel {
        info!("Using memory pool storage (optimized)");
        Box::new(MemoryPoolStorage::open(output)?) as Box<dyn Storage>
    } else if cli.cache {
        info!("Using cached storage");
        Box::new(CachedIndexStorage::open(output)?) as Box<dyn Storage>
    } else {
        info!("Using basic storage");
        Box::new(IndexStorage::open(output)?) as Box<dyn Storage>
    };

    // Create progress bar
    let progress = if cli.progress {
        let pb = ProgressBar::new(files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
                .progress_chars("#>-")
        );
        Some(pb)
    } else {
        None
    };

    // Process files
    let mut total_symbols = 0;
    let mut errors = 0;

    for (i, file) in files.iter().enumerate() {
        if let Some(pb) = &progress {
            pb.set_message(format!("Processing: {}", file.display()));
        }

        match index_single_file(file, lsp_binary, &*storage) {
            Ok(symbol_count) => {
                total_symbols += symbol_count;
                info!("Indexed {} with {} symbols", file.display(), symbol_count);
            }
            Err(e) => {
                error!("Failed to index {}: {}", file.display(), e);
                errors += 1;
            }
        }

        if let Some(pb) = &progress {
            pb.set_position((i + 1) as u64);
        }

        // Batch processing
        if (i + 1) % cli.batch_size == 0 {
            storage.flush()?;
        }
    }

    // Final flush
    storage.flush()?;

    if let Some(pb) = progress {
        pb.finish_with_message("Indexing complete!");
    }

    let elapsed = start.elapsed();

    // Print summary
    println!("\n=== Indexing Summary ===");
    println!("Files processed: {}", files.len());
    println!("Total symbols: {}", total_symbols);
    println!("Errors: {}", errors);
    println!("Time: {:.2}s", elapsed.as_secs_f64());
    println!(
        "Speed: {:.0} files/sec",
        files.len() as f64 / elapsed.as_secs_f64()
    );
    println!("Database: {}", output.display());

    Ok(())
}

fn collect_files(pattern: &str, exclude: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in glob(pattern).context("Invalid glob pattern")? {
        match entry {
            Ok(path) => {
                if path.is_file() {
                    // Check exclude patterns
                    let should_exclude =
                        exclude.iter().any(|ex| path.to_string_lossy().contains(ex));

                    if !should_exclude {
                        files.push(path);
                    }
                }
            }
            Err(e) => warn!("Glob error: {}", e),
        }
    }

    files.sort();
    Ok(files)
}

fn index_single_file(file: &Path, _lsp_binary: &str, storage: &dyn Storage) -> Result<usize> {
    // 実際のファイルが存在するか確認
    if !file.exists() {
        return Ok(0);
    }

    // ファイル内容を読み込んで簡単な解析を行う
    let content = std::fs::read_to_string(file)?;
    let mut symbols = Vec::new();

    // 簡単なRustコード解析（実際のLSPが利用できない場合のフォールバック）
    // TODO: 実際のLSP連携を実装
    if file.extension().and_then(|s| s.to_str()) == Some("rs") {
        // 関数定義を検索
        for (line_no, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // 関数定義
            if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                if let Some(name) = extract_function_name(trimmed) {
                    symbols.push(Symbol {
                        id: format!("{}#{}:{}:{}", file.display(), line_no + 1, 0, name),
                        kind: lsif_indexer::core::SymbolKind::Function,
                        name: name.to_string(),
                        file_path: file.to_string_lossy().to_string(),
                        range: lsif_indexer::core::Range {
                            start: lsif_indexer::core::Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: lsif_indexer::core::Position {
                                line: line_no as u32 + 1,
                                character: 0,
                            },
                        },
                        documentation: None,
                    });
                }
            }

            // 構造体定義
            if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") {
                if let Some(name) = extract_struct_name(trimmed) {
                    symbols.push(Symbol {
                        id: format!("{}#{}:{}:{}", file.display(), line_no + 1, 0, name),
                        kind: lsif_indexer::core::SymbolKind::Class,
                        name: name.to_string(),
                        file_path: file.to_string_lossy().to_string(),
                        range: lsif_indexer::core::Range {
                            start: lsif_indexer::core::Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: lsif_indexer::core::Position {
                                line: line_no as u32 + 1,
                                character: 0,
                            },
                        },
                        documentation: None,
                    });
                }
            }

            // impl ブロック
            if trimmed.starts_with("impl ") {
                if let Some(name) = extract_impl_name(trimmed) {
                    symbols.push(Symbol {
                        id: format!("{}#{}:{}:impl_{}", file.display(), line_no + 1, 0, name),
                        kind: lsif_indexer::core::SymbolKind::Class,
                        name: format!("impl {}", name),
                        file_path: file.to_string_lossy().to_string(),
                        range: lsif_indexer::core::Range {
                            start: lsif_indexer::core::Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: lsif_indexer::core::Position {
                                line: line_no as u32 + 1,
                                character: 0,
                            },
                        },
                        documentation: None,
                    });
                }
            }
        }
    }

    // シンボルが見つからない場合は、ダミーシンボルを作成
    if symbols.is_empty() {
        symbols.push(Symbol {
            id: format!("{}#file", file.display()),
            kind: lsif_indexer::core::SymbolKind::Variable,
            name: file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            file_path: file.to_string_lossy().to_string(),
            range: lsif_indexer::core::Range {
                start: lsif_indexer::core::Position {
                    line: 0,
                    character: 0,
                },
                end: lsif_indexer::core::Position {
                    line: 1,
                    character: 0,
                },
            },
            documentation: None,
        });
    }

    let symbol_count = symbols.len();
    storage.save_symbols(&symbols)?;
    Ok(symbol_count)
}

fn extract_function_name(line: &str) -> Option<&str> {
    let line = line.trim_start_matches("pub ").trim_start_matches("fn ");
    line.split(&['(', '<'][..]).next()
}

fn extract_struct_name(line: &str) -> Option<&str> {
    let line = line
        .trim_start_matches("pub ")
        .trim_start_matches("struct ");
    line.split(&[' ', '<', '{'][..]).next()
}

fn extract_impl_name(line: &str) -> Option<&str> {
    let line = line.trim_start_matches("impl ");
    line.split(&[' ', '<', '{'][..]).next()
}

fn execute_query(db: &Path, query: QueryType) -> Result<()> {
    let storage = IndexStorage::open(db)?;

    match query {
        QueryType::Definition { file, line, column } => {
            println!(
                "Searching for definition at {}:{}:{}",
                file.display(),
                line,
                column
            );

            // ファイルからシンボルを検索（簡易版）
            let target_id = format!("{}#{}:{}:", file.display(), line, column);

            // データベースからシンボルを検索
            let mut found = false;
            for entry in std::fs::read_dir(db)? {
                if let Ok(entry) = entry {
                    let key = entry.file_name().to_string_lossy().to_string();
                    if key.starts_with(&target_id)
                        || key.contains(&file.to_string_lossy().to_string())
                    {
                        if let Ok(symbol) = storage.load_data::<Symbol>(&key) {
                            if let Some(sym) = symbol {
                                println!(
                                    "Found: {} at {}:{}",
                                    sym.name,
                                    sym.file_path,
                                    sym.range.start.line + 1
                                );
                                found = true;
                            }
                        }
                    }
                }
            }

            if !found {
                println!("No definition found at this location");
            }
        }
        QueryType::References { symbol } => {
            println!("Finding references for: {}", symbol);

            // シンボル名で検索
            let mut count = 0;
            for entry in std::fs::read_dir(db)? {
                if let Ok(entry) = entry {
                    let key = entry.file_name().to_string_lossy().to_string();
                    if key.contains(&symbol) {
                        if let Ok(sym_data) = storage.load_data::<Symbol>(&key) {
                            if let Some(sym) = sym_data {
                                if sym.name == symbol {
                                    println!(
                                        "  - {} at {}:{}",
                                        sym.file_path,
                                        sym.range.start.line + 1,
                                        sym.range.start.character
                                    );
                                    count += 1;
                                }
                            }
                        }
                    }
                }
            }
            println!("Found {} references", count);
        }
        QueryType::CallHierarchy { function, depth } => {
            println!("Call hierarchy for {} (depth: {})", function, depth);

            // 関数を検索
            let mut found = false;
            for entry in std::fs::read_dir(db)? {
                if let Ok(entry) = entry {
                    let key = entry.file_name().to_string_lossy().to_string();
                    if key.contains(&function) {
                        if let Ok(sym_data) = storage.load_data::<Symbol>(&key) {
                            if let Some(sym) = sym_data {
                                if sym.name == function
                                    && sym.kind == lsif_indexer::core::SymbolKind::Function
                                {
                                    println!("Function: {} in {}", sym.name, sym.file_path);
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            if !found {
                println!("Function not found");
            }
        }
        QueryType::DeadCode => {
            println!("Searching for dead code...");

            // すべてのシンボルを読み込んで、参照されていないものを探す
            let mut all_symbols = Vec::new();
            for entry in std::fs::read_dir(db)? {
                if let Ok(entry) = entry {
                    let key = entry.file_name().to_string_lossy().to_string();
                    if let Ok(sym_data) = storage.load_data::<Symbol>(&key) {
                        if let Some(sym) = sym_data {
                            all_symbols.push(sym);
                        }
                    }
                }
            }

            // main以外の未使用関数を検出（簡易版）
            for sym in &all_symbols {
                if sym.kind == lsif_indexer::core::SymbolKind::Function
                    && sym.name != "main"
                    && !sym.name.starts_with("test_")
                    && !sym.name.starts_with("bench_")
                {
                    // 簡易的なデッドコード判定
                    println!("  Potentially unused: {} in {}", sym.name, sym.file_path);
                }
            }
        }
        QueryType::TypeRelations { type_name } => {
            println!("Type relations for: {}", type_name);

            // 型を検索
            for entry in std::fs::read_dir(db)? {
                if let Ok(entry) = entry {
                    let key = entry.file_name().to_string_lossy().to_string();
                    if key.contains(&type_name) {
                        if let Ok(sym_data) = storage.load_data::<Symbol>(&key) {
                            if let Some(sym) = sym_data {
                                if sym.name == type_name
                                    && sym.kind == lsif_indexer::core::SymbolKind::Class
                                {
                                    println!("Type: {} in {}", sym.name, sym.file_path);
                                    // impl ブロックを検索
                                    let impl_name = format!("impl {}", type_name);
                                    for entry2 in std::fs::read_dir(db)? {
                                        if let Ok(entry2) = entry2 {
                                            let key2 =
                                                entry2.file_name().to_string_lossy().to_string();
                                            if key2.contains(&impl_name) {
                                                if let Ok(impl_data) =
                                                    storage.load_data::<Symbol>(&key2)
                                                {
                                                    if let Some(impl_sym) = impl_data {
                                                        println!(
                                                            "  Implementation in {}",
                                                            impl_sym.file_path
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn show_stats(db: &Path) -> Result<()> {
    let _storage = IndexStorage::open(db)?;

    println!("=== Database Statistics ===");
    println!("Path: {}", db.display());

    // ディレクトリの場合、全ファイルサイズを合計
    let size = if db.is_dir() {
        let mut total_size = 0u64;
        for entry in std::fs::read_dir(db)? {
            if let Ok(entry) = entry {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                }
            }
        }
        total_size
    } else {
        db.metadata()?.len()
    };

    println!("Size: {:.2} MB", size as f64 / (1024.0 * 1024.0));

    // シンボル数をカウント（簡易版）
    let mut symbol_count = 0;
    if db.is_dir() {
        for entry in std::fs::read_dir(db)? {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("db") {
                    symbol_count += 1;
                }
            }
        }
    }

    if symbol_count > 0 {
        println!("Estimated symbols: ~{}", symbol_count);
    }

    Ok(())
}

fn list_languages() {
    println!("=== Supported Languages ===");
    println!("Language     | LSP Server");
    println!("-------------|-------------------------");
    println!("Rust         | rust-analyzer");
    println!("TypeScript   | typescript-language-server");
    println!("JavaScript   | typescript-language-server");
    println!("Python       | pylsp");
    println!("Go           | gopls");
    println!("Java         | jdtls");
    println!("C/C++        | clangd");
    println!("\nUse --bin to specify a custom LSP server");
}

fn watch_files(pattern: &str, db: &Path, cli: &Cli) -> Result<()> {
    println!("Watching files matching: {}", pattern);
    println!("Press Ctrl+C to stop");

    // Implementation would use notify crate for file watching
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

// Storage trait for abstraction
trait Storage: Send + Sync {
    fn save_symbols(&self, symbols: &[Symbol]) -> Result<()>;
    fn flush(&self) -> Result<()>;
}

impl Storage for IndexStorage {
    fn save_symbols(&self, symbols: &[Symbol]) -> Result<()> {
        for symbol in symbols {
            self.save_data(&symbol.id, symbol)?;
        }
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

impl Storage for ParallelIndexStorage {
    fn save_symbols(&self, symbols: &[Symbol]) -> Result<()> {
        let data: Vec<(String, Symbol)> =
            symbols.iter().map(|s| (s.id.clone(), s.clone())).collect();
        self.save_symbols_parallel(&data)?;
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

impl Storage for CachedIndexStorage {
    fn save_symbols(&self, symbols: &[Symbol]) -> Result<()> {
        for symbol in symbols {
            self.save_data_cached(&symbol.id, symbol)?;
        }
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

impl Storage for MemoryPoolStorage {
    fn save_symbols(&self, symbols: &[Symbol]) -> Result<()> {
        self.save_symbols(symbols)
    }

    fn flush(&self) -> Result<()> {
        self.flush()
    }
}

// 新しい検索機能の実装
fn execute_search(
    db: &Path,
    query: &str,
    symbol_type: Option<String>,
    file_pattern: Option<String>,
    detailed: bool,
) -> Result<()> {
    let storage = IndexStorage::open(db)?;

    println!("=== Searching for '{}' ===", query);
    if let Some(ref t) = symbol_type {
        println!("Filter: type = {}", t);
    }
    if let Some(ref f) = file_pattern {
        println!("Filter: file = {}", f);
    }
    println!();

    let mut found_count = 0;
    let query_lower = query.to_lowercase();

    // sledデータベースのキーを取得して走査
    let keys = storage.list_keys()?;
    for key in keys {
        // メタデータキーをスキップ
        if key.starts_with("__") {
            continue;
        }

        // シンボルデータをロード
        if let Ok(symbol_data) = storage.load_data::<Symbol>(&key) {
            if let Some(symbol) = symbol_data {
                // クエリマッチング
                if !symbol.name.to_lowercase().contains(&query_lower) {
                    continue;
                }

                // タイプフィルタ
                if let Some(ref t) = symbol_type {
                    let symbol_type_str = format!("{:?}", symbol.kind).to_lowercase();
                    if !symbol_type_str.contains(&t.to_lowercase()) {
                        continue;
                    }
                }

                // ファイルフィルタ
                if let Some(ref f) = file_pattern {
                    if !symbol.file_path.contains(f) {
                        continue;
                    }
                }

                // 結果を表示
                found_count += 1;
                println!(
                    "[{}] {} ({})",
                    found_count,
                    symbol.name,
                    format!("{:?}", symbol.kind)
                );
                println!(
                    "  📍 {}:{}:{}",
                    symbol.file_path,
                    symbol.range.start.line + 1,
                    symbol.range.start.character + 1
                );

                if detailed {
                    if let Some(ref doc) = symbol.documentation {
                        println!("  📝 {}", doc);
                    }
                }
                println!();
            }
        }
    }

    if found_count == 0 {
        println!("No symbols found matching '{}'", query);
        println!("\n💡 Tip: Try a broader search or check your filters");
    } else {
        println!("Found {} matching symbols", found_count);
    }

    Ok(())
}

fn execute_find(db: &Path, what: FindType) -> Result<()> {
    let storage = IndexStorage::open(db)?;

    match what {
        FindType::Definition { symbol } => {
            println!("=== Finding definition of '{}' ===\n", symbol);

            let symbol_lower = symbol.to_lowercase();
            let mut found = false;

            for entry in std::fs::read_dir(db)? {
                if let Ok(entry) = entry {
                    let key = entry.file_name().to_string_lossy().to_string();

                    if let Ok(symbol_data) = storage.load_data::<Symbol>(&key) {
                        if let Some(sym) = symbol_data {
                            if sym.name.to_lowercase() == symbol_lower {
                                println!("✅ Definition found:");
                                println!("   Name: {}", sym.name);
                                println!("   Type: {:?}", sym.kind);
                                println!(
                                    "   Location: {}:{}:{}",
                                    sym.file_path,
                                    sym.range.start.line + 1,
                                    sym.range.start.character + 1
                                );
                                if let Some(ref doc) = sym.documentation {
                                    println!("   Documentation: {}", doc);
                                }
                                found = true;
                                break;
                            }
                        }
                    }
                }
            }

            if !found {
                println!("❌ No definition found for '{}'", symbol);
                println!(
                    "\n💡 Tip: Use 'lsif search {}' to find similar symbols",
                    symbol
                );
            }
        }

        FindType::References { symbol } => {
            println!("=== Finding references to '{}' ===\n", symbol);

            let symbol_lower = symbol.to_lowercase();
            let mut references = Vec::new();

            // この実装は簡易版。実際の参照解析にはグラフ構造が必要
            for entry in std::fs::read_dir(db)? {
                if let Ok(entry) = entry {
                    let key = entry.file_name().to_string_lossy().to_string();

                    if let Ok(symbol_data) = storage.load_data::<Symbol>(&key) {
                        if let Some(sym) = symbol_data {
                            // 名前に含まれていれば参照の可能性あり（簡易実装）
                            if key.contains(&symbol_lower) || sym.name.contains(&symbol) {
                                references.push((sym.file_path.clone(), sym.range.start.line + 1));
                            }
                        }
                    }
                }
            }

            if references.is_empty() {
                println!("No references found for '{}'", symbol);
            } else {
                println!("Found {} potential references:", references.len());
                for (file, line) in references.iter().take(20) {
                    println!("  📍 {}:{}", file, line);
                }
                if references.len() > 20 {
                    println!("  ... and {} more", references.len() - 20);
                }
            }
        }

        FindType::At { file, line, column } => {
            println!(
                "=== Finding symbol at {}:{}:{} ===\n",
                file.display(),
                line,
                column
            );

            let file_str = file.to_string_lossy().to_string();
            let mut found = false;

            for entry in std::fs::read_dir(db)? {
                if let Ok(entry) = entry {
                    let key = entry.file_name().to_string_lossy().to_string();

                    if let Ok(symbol_data) = storage.load_data::<Symbol>(&key) {
                        if let Some(sym) = symbol_data {
                            if sym.file_path.contains(&file_str) {
                                // 行番号が範囲内かチェック
                                if sym.range.start.line < line && sym.range.end.line >= line {
                                    println!("Found: {} ({:?})", sym.name, sym.kind);
                                    println!(
                                        "  Range: {}:{} - {}:{}",
                                        sym.range.start.line + 1,
                                        sym.range.start.character + 1,
                                        sym.range.end.line + 1,
                                        sym.range.end.character + 1
                                    );
                                    found = true;
                                }
                            }
                        }
                    }
                }
            }

            if !found {
                println!("No symbol found at this location");
                println!("\n💡 Tip: Try nearby lines or use 'lsif lsp symbols {}' to see all symbols in the file", file.display());
            }
        }
    }

    Ok(())
}

fn execute_differential_index(db: &Path, full: bool, verbose: bool) -> Result<()> {
    use lsif_indexer::cli::differential_indexer::DifferentialIndexer;

    // ログレベルを設定
    if verbose {
        tracing::subscriber::set_global_default(
            tracing_subscriber::FmtSubscriber::builder()
                .with_max_level(tracing::Level::DEBUG)
                .finish(),
        )
        .ok();
    }

    // 現在のディレクトリをプロジェクトルートとする
    let project_root = std::env::current_dir()?;

    println!("🔍 Starting differential indexing...");
    println!("Project root: {}", project_root.display());
    println!("Database: {}", db.display());

    // 差分インデクサーを作成
    let mut indexer = DifferentialIndexer::new(db, &project_root)?;

    // インデックス実行
    let result = if full {
        println!("Performing full reindex...");
        indexer.full_reindex()?
    } else {
        indexer.index_differential()?
    };

    // 結果を表示
    println!("\n✅ Differential indexing complete!");
    println!("┌─────────────────────────────────────┐");
    println!("│ Files:                              │");
    println!(
        "│   Added:    {:>5}                   │",
        result.files_added
    );
    println!(
        "│   Modified: {:>5}                   │",
        result.files_modified
    );
    println!(
        "│   Deleted:  {:>5}                   │",
        result.files_deleted
    );
    println!("├─────────────────────────────────────┤");
    println!("│ Symbols:                            │");
    println!(
        "│   Added:    {:>5}                   │",
        result.symbols_added
    );
    println!(
        "│   Updated:  {:>5}                   │",
        result.symbols_updated
    );
    println!(
        "│   Deleted:  {:>5}                   │",
        result.symbols_deleted
    );
    println!("├─────────────────────────────────────┤");
    println!(
        "│ Time: {:.2}s                         │",
        result.duration.as_secs_f64()
    );
    println!("└─────────────────────────────────────┘");

    Ok(())
}

fn run_interactive_mode(db: &Path) -> Result<()> {
    println!("🚀 LSIF Interactive Explorer");
    println!("Type 'help' for commands, 'quit' to exit\n");

    let storage = IndexStorage::open(db)?;
    let mut last_results = Vec::new();

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let command = parts[0];

        match command {
            "help" | "h" => {
                println!("Commands:");
                println!("  search <query>  - Search for symbols");
                println!("  find <symbol>   - Find symbol definition");
                println!("  refs <symbol>   - Find symbol references");
                println!("  last            - Show last search results");
                println!("  stats           - Show database statistics");
                println!("  clear           - Clear screen");
                println!("  quit            - Exit interactive mode");
            }

            "search" | "s" => {
                if parts.len() < 2 {
                    println!("Usage: search <query>");
                    continue;
                }
                let query = parts[1..].join(" ");
                last_results.clear();

                println!("\nSearching for '{}'...\n", query);
                let query_lower = query.to_lowercase();

                for entry in std::fs::read_dir(db)? {
                    if let Ok(entry) = entry {
                        let key = entry.file_name().to_string_lossy().to_string();

                        if let Ok(symbol_data) = storage.load_data::<Symbol>(&key) {
                            if let Some(symbol) = symbol_data {
                                if symbol.name.to_lowercase().contains(&query_lower) {
                                    last_results.push(symbol.clone());
                                }
                            }
                        }
                    }
                }

                if last_results.is_empty() {
                    println!("No results found");
                } else {
                    println!("Found {} results:", last_results.len());
                    for (i, symbol) in last_results.iter().enumerate().take(10) {
                        println!(
                            "  [{}] {} - {}:{}",
                            i + 1,
                            symbol.name,
                            symbol.file_path,
                            symbol.range.start.line + 1
                        );
                    }
                    if last_results.len() > 10 {
                        println!("  ... and {} more", last_results.len() - 10);
                    }
                }
            }

            "find" | "f" => {
                if parts.len() < 2 {
                    println!("Usage: find <symbol>");
                    continue;
                }
                let symbol = parts[1..].join(" ");
                execute_find(db, FindType::Definition { symbol })?;
            }

            "refs" | "r" => {
                if parts.len() < 2 {
                    println!("Usage: refs <symbol>");
                    continue;
                }
                let symbol = parts[1..].join(" ");
                execute_find(db, FindType::References { symbol })?;
            }

            "last" | "l" => {
                if last_results.is_empty() {
                    println!("No previous results");
                } else {
                    println!("Last {} results:", last_results.len());
                    for (i, symbol) in last_results.iter().enumerate() {
                        println!(
                            "  [{}] {} - {}:{}",
                            i + 1,
                            symbol.name,
                            symbol.file_path,
                            symbol.range.start.line + 1
                        );
                    }
                }
            }

            "stats" => {
                show_stats(db)?;
            }

            "clear" | "cls" => {
                print!("\x1B[2J\x1B[1;1H");
            }

            "quit" | "q" | "exit" => {
                println!("Goodbye! 👋");
                break;
            }

            _ => {
                println!(
                    "Unknown command: '{}'. Type 'help' for available commands.",
                    command
                );
            }
        }
        println!();
    }

    Ok(())
}
