use anyhow::Result;
use clap::Parser;
use lsif_indexer::cli::Cli;

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Parse CLI arguments and execute
    let cli = Cli::parse();
    cli.execute()
}