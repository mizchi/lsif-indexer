use lsif_indexer::cli::storage::IndexStorage;
use anyhow::Result;

fn main() -> Result<()> {
    let storage = IndexStorage::open("tmp/self-index.lsif")?;
    
    println!("=== Database Keys ===");
    let keys = storage.list_keys()?;
    
    println!("Total keys: {}", keys.len());
    
    // 最初の10個のキーを表示
    for (i, key) in keys.iter().take(10).enumerate() {
        println!("{}: {}", i + 1, key);
    }
    
    if keys.len() > 10 {
        println!("... and {} more", keys.len() - 10);
    }
    
    // メタデータを確認
    if let Some(metadata) = storage.load_metadata()? {
        println!("\n=== Metadata ===");
        println!("Files: {}", metadata.files_indexed);
        println!("Total symbols: {}", metadata.total_symbols);
        println!("Indexed at: {:?}", metadata.indexed_at);
    }
    
    Ok(())
}