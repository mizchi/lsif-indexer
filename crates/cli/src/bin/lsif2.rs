use anyhow::Result;
use cli::improved_cli::Cli;
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run()
}