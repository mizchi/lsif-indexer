use super::utils::*;
use crate::differential_indexer::DifferentialIndexer;
use anyhow::Result;
use std::path::Path;
use std::time::Instant;

pub fn handle_index(
    db_path: &str,
    project_root: &str,
    force: bool,
    _show_progress: bool,
    fallback_only: bool,
    workspace_symbol: bool,
) -> Result<()> {
    let start = Instant::now();

    // workspace/symbolモードが明示的に指定された場合
    if workspace_symbol {
        print_info("Using workspace/symbol for fast indexing...", "🚀");

        use crate::storage::IndexStorage;
        use crate::workspace_symbol_strategy::WorkspaceSymbolStrategy;

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
            std::fs::remove_dir_all(db_path).ok(); // DBディレクトリを削除
        }
    } else {
        print_info("Indexing project...", "📇");
    }

    let mut indexer = DifferentialIndexer::new(db_path, Path::new(project_root))?;

    // フォールバックオンリーモードを設定
    if fallback_only {
        indexer.set_fallback_only(true);
    }

    // full_reindexは内部でworkspace/symbolを試みる
    let result = if force || !Path::new(db_path).exists() {
        indexer.full_reindex()? // 内部でworkspace/symbolを優先的に使用
    } else {
        indexer.index_differential()? // 差分時はdocument symbolを使用
    };

    // 結果の表示を改善
    if result.full_reindex && result.files_added == 0 {
        // workspace/symbolを使った場合
        print_success(&format!(
            "Indexed {} symbols in {:.2}s (using workspace/symbol)",
            result.symbols_added,
            start.elapsed().as_secs_f64(),
        ));
    } else {
        // 通常のファイル単位インデックスの場合
        print_success(&format!(
            "Indexed {} symbols in {:.2}s (+{} ~{} -{} files)",
            result.symbols_added,
            start.elapsed().as_secs_f64(),
            result.files_added,
            result.files_modified,
            result.files_deleted
        ));
    }

    Ok(())
}
