use anyhow::Result;
use std::path::Path;
use std::time::Instant;
use crate::differential_indexer::DifferentialIndexer;
use super::utils::*;

pub fn handle_index(db_path: &str, project_root: &str, force: bool, _show_progress: bool, fallback_only: bool, workspace_symbol: bool) -> Result<()> {
    let start = Instant::now();
    
    // workspace/symbolモードの場合
    if workspace_symbol {
        print_info("Using workspace/symbol for fast indexing...", "🚀");
        
        use crate::workspace_symbol_strategy::WorkspaceSymbolStrategy;
        use crate::storage::IndexStorage;
        use lsif_core::CodeGraph;
        use std::path::PathBuf;
        
        let strategy = WorkspaceSymbolStrategy::new(PathBuf::from(project_root));
        let graph = strategy.index()?;
        
        // ストレージに保存
        let storage = IndexStorage::open(db_path)?;
        storage.save_data("graph", &graph)?;
        
        print_success(&format!(
            "Indexed {} symbols in {:.2}s using workspace/symbol",
            graph.symbol_count(),
            start.elapsed().as_secs_f64(),
        ));
        
        return Ok(());
    }
    
    // 通常のインデックスモード
    if force {
        print_info("Force reindexing project...", "🔄");
        if Path::new(db_path).exists() {
            std::fs::remove_file(db_path)?;
        }
    } else {
        print_info("Indexing project...", "📇");
    }
    
    let mut indexer = DifferentialIndexer::new(db_path, Path::new(project_root))?;
    
    // フォールバックオンリーモードを設定
    if fallback_only {
        indexer.set_fallback_only(true);
    }
    
    let result = if force || !Path::new(db_path).exists() {
        indexer.full_reindex()?
    } else {
        indexer.index_differential()?
    };
    
    print_success(&format!(
        "Indexed {} symbols in {:.2}s (+{} ~{} -{} files)",
        result.symbols_added,
        start.elapsed().as_secs_f64(),
        result.files_added,
        result.files_modified,
        result.files_deleted
    ));
    
    Ok(())
}