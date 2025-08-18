use anyhow::Result;
use clap::Parser;
use lsif_indexer::cli::Cli;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    cli.execute()?;
    
    Ok(())
}// Benchmark test comment 1755525810
