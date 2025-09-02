use anyhow::Result;
use std::path::Path;
use std::time::Instant;
use crate::differential_indexer::DifferentialIndexer;
use super::utils::*;

pub fn handle_index(db_path: &str, project_root: &str, force: bool, _show_progress: bool, fallback_only: bool) -> Result<()> {
    let start = Instant::now();
    
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