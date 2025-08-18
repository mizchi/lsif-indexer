use lsif_indexer::cli::MemoryPoolStorage;
use lsif_indexer::core::{Symbol, SymbolKind, Range, Position};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    // 一時ディレクトリを作成
    let temp_dir = tempfile::TempDir::new()?;
    
    // キャッシュサイズ1000でストレージを作成
    let storage = MemoryPoolStorage::with_cache_size(
        lsif_indexer::cli::UltraFastStorage::open(temp_dir.path())?,
        1000
    );
    
    // テストシンボルを作成
    let mut symbols = Vec::new();
    for i in 0..2000 {
        symbols.push(Symbol {
            id: format!("symbol_{}", i),
            kind: SymbolKind::Function,
            name: format!("test_function_{}", i),
            file_path: format!("src/file_{}.rs", i % 10),
            range: Range {
                start: Position { line: i as u32, character: 0 },
                end: Position { line: i as u32 + 1, character: 0 },
            },
            documentation: None,
        });
    }
    
    println!("=== Memory Pool + LRU Cache Storage Test ===");
    
    // データを保存
    let start = Instant::now();
    for symbol in &symbols[..1000] {
        storage.save_with_pool(&symbol.id, symbol)?;
    }
    println!("Saved 1000 symbols in {:?}", start.elapsed());
    
    // キャッシュヒット率のテスト
    println!("\n--- Cache Hit Rate Test ---");
    let start = Instant::now();
    let mut reads = 0;
    
    // 80%は既存データ（キャッシュヒット）、20%は未保存データ（キャッシュミス）
    for i in 0..1000 {
        let id = if i % 5 == 0 {
            format!("symbol_{}", 1000 + i) // キャッシュミス
        } else {
            format!("symbol_{}", i % 1000) // キャッシュヒット
        };
        
        let _: Option<Symbol> = storage.load_data(&id)?;
        reads += 1;
    }
    
    println!("Performed {} reads in {:?}", reads, start.elapsed());
    
    // キャッシュ統計を表示
    let stats = storage.get_cache_stats();
    println!("\n--- Cache Statistics ---");
    println!("Cache size: {}/{}", stats.size, stats.capacity);
    println!("Cache hits: {}", stats.hits);
    println!("Cache misses: {}", stats.misses);
    println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
    
    // キャッシュサイズの効果をテスト
    println!("\n--- Cache Size Impact ---");
    for cache_size in [100, 500, 2000] {
        let storage = MemoryPoolStorage::with_cache_size(
            lsif_indexer::cli::UltraFastStorage::open(temp_dir.path())?,
            cache_size
        );
        
        // 全データを保存
        for symbol in &symbols {
            storage.save_with_pool(&symbol.id, symbol)?;
        }
        
        // ランダムアクセス
        let start = Instant::now();
        for i in 0..1000 {
            let id = format!("symbol_{}", i * 3 % 2000);
            let _: Option<Symbol> = storage.load_data(&id)?;
        }
        
        let stats = storage.get_cache_stats();
        println!(
            "Cache size {}: Hit rate {:.2}% in {:?}",
            cache_size,
            stats.hit_rate * 100.0,
            start.elapsed()
        );
    }
    
    Ok(())
}