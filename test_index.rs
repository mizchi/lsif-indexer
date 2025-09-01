use anyhow::Result;
use cli::differential_indexer::DifferentialIndexer;
use std::path::Path;

fn main() -> Result<()> {
    // 環境変数でログレベルを設定
    std::env::set_var("RUST_LOG", "info");
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let db_path = "tmp/direct-test.db";
    let project_path = Path::new("tmp/sample-project");
    
    println!("Creating indexer for: {}", project_path.display());
    let mut indexer = DifferentialIndexer::new(db_path, project_path)?;
    
    println!("Running full reindex...");
    let result = indexer.full_reindex()?;
    
    println!("Results:");
    println!("  Files added: {}", result.files_added);
    println!("  Symbols added: {}", result.symbols_added);
    println!("  Duration: {:?}", result.duration);
    
    Ok(())
}