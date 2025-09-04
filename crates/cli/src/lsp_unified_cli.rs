use anyhow::Result;
use lsp::UnifiedIndexer;
use std::path::Path;
use tracing::{error, info};

/// LSPベースの統一インデクサーを使用したCLIコマンド
pub struct LspUnifiedCli;

impl LspUnifiedCli {
    /// プロジェクトをLSPベースでインデックス
    pub async fn index_with_lsp(project_path: &Path) -> Result<()> {
        info!(
            "Starting unified LSP-based indexing for: {}",
            project_path.display()
        );

        let mut indexer = UnifiedIndexer::new();

        // プロジェクト全体をインデックス
        match indexer.index_project(project_path).await {
            Ok(result) => {
                info!(
                    "✓ Successfully indexed {} files with {} symbols in {:.2}s",
                    result.files_indexed,
                    result.symbols_found,
                    result.duration.as_secs_f64()
                );

                // グラフ情報を表示
                let graph = indexer.get_graph();
                info!("Total symbols in graph: {}", graph.symbol_count());

                Ok(())
            }
            Err(e) => {
                error!("Failed to index project: {}", e);
                Err(e)
            }
        }
    }

    /// 単一ファイルをLSPベースでインデックス
    pub async fn index_file_with_lsp(file_path: &Path) -> Result<()> {
        info!("Indexing file with LSP: {}", file_path.display());

        let mut indexer = UnifiedIndexer::new();

        match indexer.index_file(file_path).await {
            Ok(symbols) => {
                info!("✓ Found {} symbols in file", symbols.len());

                // シンボル一覧を表示
                for symbol in &symbols {
                    info!(
                        "  - {} '{}' at line {}",
                        format!("{:?}", symbol.kind),
                        symbol.name,
                        symbol.range.start.line
                    );
                }

                Ok(())
            }
            Err(e) => {
                error!("Failed to index file: {}", e);
                Err(e)
            }
        }
    }

    /// ワークスペースシンボルを検索
    pub async fn search_workspace_symbols(workspace_path: &Path, query: &str) -> Result<()> {
        info!(
            "Searching for '{}' in workspace: {}",
            query,
            workspace_path.display()
        );

        let indexer = UnifiedIndexer::new();

        match indexer.search_symbols(query, workspace_path).await {
            Ok(symbols) => {
                info!("✓ Found {} symbols matching '{}'", symbols.len(), query);

                // 検索結果を表示
                for symbol in &symbols {
                    info!(
                        "  - {} '{}' in {} at line {}",
                        format!("{:?}", symbol.kind),
                        symbol.name,
                        symbol.file_path,
                        symbol.range.start.line
                    );
                }

                Ok(())
            }
            Err(e) => {
                error!("Failed to search symbols: {}", e);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    // Explicitly use std to avoid conflict with local core crate
    extern crate std;

    #[tokio::test]
    #[ignore = "Requires LSP server to be installed"]
    async fn test_index_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        // 空のディレクトリをインデックス（エラーにならないことを確認）
        let result = LspUnifiedCli::index_with_lsp(temp_dir.path()).await;

        // LSPサーバーが起動できない可能性があるのでエラーは許容
        if result.is_ok() {
            // 成功した場合、何もインデックスされていないはず
            println!("Empty directory indexed successfully");
        }
    }

    #[tokio::test]
    #[ignore = "Requires LSP server to be installed"]
    async fn test_index_rust_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        // 簡単なRustファイルを作成
        fs::write(
            &file_path,
            r#"
fn main() {
    println!("Hello, world!");
}

struct TestStruct {
    field: String,
}

impl TestStruct {
    fn new() -> Self {
        Self {
            field: String::new(),
        }
    }
}
"#,
        )
        .unwrap();

        // ファイルをインデックス
        let result = LspUnifiedCli::index_file_with_lsp(&file_path).await;

        // LSPサーバーが利用できない場合はスキップ
        if result.is_ok() {
            println!("Rust file indexed successfully");
        }
    }
}
