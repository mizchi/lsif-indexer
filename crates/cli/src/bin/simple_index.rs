use anyhow::Result;
use clap::Parser;
use cli::differential_indexer::DifferentialIndexer;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(author, version, about = "Simple indexer for testing", long_about = None)]
struct Args {
    /// Project directory to index
    #[arg(short, long)]
    project: String,

    /// Database path
    #[arg(short, long, default_value = "index.db")]
    database: String,

    /// Force full reindex
    #[arg(short, long)]
    force: bool,
}

fn main() -> Result<()> {
    // Set up logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let project_path = Path::new(&args.project);

    // Remove existing database if force flag is set
    if args.force && Path::new(&args.database).exists() {
        std::fs::remove_dir_all(&args.database)?;
        println!("Removed existing database");
    }

    println!("Indexing project: {}", project_path.display());
    println!("Database: {}", args.database);

    let mut indexer = DifferentialIndexer::new(&args.database, project_path)?;

    let result = if args.force {
        indexer.full_reindex()?
    } else {
        indexer.index_differential()?
    };

    println!("\nIndexing complete!");
    println!(
        "  Files: added={}, modified={}, deleted={}",
        result.files_added, result.files_modified, result.files_deleted
    );
    println!(
        "  Symbols: added={}, updated={}, deleted={}",
        result.symbols_added, result.symbols_updated, result.symbols_deleted
    );
    println!("  Time: {:?}", result.duration);

    if !result.added_symbols.is_empty() {
        println!("\nSample symbols added:");
        for (i, sym) in result.added_symbols.iter().enumerate().take(5) {
            println!(
                "  {}. {} ({:?}) at {}:{}",
                i + 1,
                sym.name,
                sym.kind,
                sym.file_path,
                sym.line
            );
        }
    }

    Ok(())
}
