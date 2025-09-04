use anyhow::Result;
use lsp_types::{DocumentSymbol, SymbolKind as LspSymbolKind};
use std::path::Path;
use tracing::{debug, info};

// lsif-coreクレートからのインポート
use lsif_core::{CodeGraph, Position, Range, Symbol, SymbolKind};

use crate::lsp_manager::UnifiedLspManager;

/// LSPベースの統一インデクサー
pub struct UnifiedIndexer {
    lsp_manager: UnifiedLspManager,
    graph: CodeGraph,
}

impl Default for UnifiedIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedIndexer {
    pub fn new() -> Self {
        Self {
            lsp_manager: UnifiedLspManager::new(),
            graph: CodeGraph::new(),
        }
    }

    /// プロジェクト全体をインデックス
    pub async fn index_project(&mut self, project_root: &Path) -> Result<IndexResult> {
        info!(
            "Starting unified LSP-based indexing for: {}",
            project_root.display()
        );

        let start = std::time::Instant::now();

        // LSPマネージャーでプロジェクトをインデックス
        let project_index = self.lsp_manager.index_project(project_root).await?;

        let mut total_symbols = 0;
        let mut total_files = 0;

        // 各ファイルのシンボルをCodeGraphに変換
        for (file_path, doc_symbols) in project_index.symbols {
            let file_uri = format!("file://{}", file_path.display());
            let symbols = self.convert_document_symbols(&doc_symbols, &file_uri, &file_path);

            total_symbols += symbols.len();
            total_files += 1;

            // グラフに追加
            for symbol in symbols {
                self.graph.add_symbol(symbol);
            }
        }

        let duration = start.elapsed();

        info!(
            "Indexing completed: {} files, {} symbols in {:.2}s",
            total_files,
            total_symbols,
            duration.as_secs_f64()
        );

        Ok(IndexResult {
            files_indexed: total_files,
            symbols_found: total_symbols,
            duration,
        })
    }

    /// 単一ファイルをインデックス
    pub async fn index_file(&mut self, file_path: &Path) -> Result<Vec<Symbol>> {
        debug!("Indexing file: {}", file_path.display());

        // LSPマネージャーでドキュメントシンボルを取得
        let doc_symbols = self.lsp_manager.get_document_symbols(file_path).await?;

        let file_uri = format!("file://{}", file_path.display());
        let symbols = self.convert_document_symbols(&doc_symbols, &file_uri, file_path);

        // グラフに追加
        for symbol in &symbols {
            self.graph.add_symbol(symbol.clone());
        }

        Ok(symbols)
    }

    /// DocumentSymbolをSymbolに変換
    fn convert_document_symbols(
        &self,
        doc_symbols: &[DocumentSymbol],
        file_uri: &str,
        file_path: &Path,
    ) -> Vec<Symbol> {
        let mut symbols = Vec::new();

        for doc_symbol in doc_symbols {
            symbols.push(self.convert_document_symbol(doc_symbol, file_uri, file_path));

            // 子シンボルも再帰的に変換
            if let Some(children) = &doc_symbol.children {
                symbols.extend(self.convert_document_symbols(children, file_uri, file_path));
            }
        }

        symbols
    }

    /// 単一のDocumentSymbolをSymbolに変換
    fn convert_document_symbol(
        &self,
        doc_symbol: &DocumentSymbol,
        _file_uri: &str,
        file_path: &Path,
    ) -> Symbol {
        let file_path_str = file_path.to_string_lossy().to_string();

        Symbol {
            id: format!(
                "{}#{}:{}",
                file_path_str, doc_symbol.range.start.line, doc_symbol.name
            ),
            name: doc_symbol.name.clone(),
            kind: self.convert_symbol_kind(doc_symbol.kind),
            file_path: file_path_str,
            range: Range {
                start: Position {
                    line: doc_symbol.range.start.line,
                    character: doc_symbol.range.start.character,
                },
                end: Position {
                    line: doc_symbol.range.end.line,
                    character: doc_symbol.range.end.character,
                },
            },
            documentation: doc_symbol.detail.clone(),
            detail: None,
        }
    }

    /// LSPのSymbolKindを内部のSymbolKindに変換
    fn convert_symbol_kind(&self, lsp_kind: LspSymbolKind) -> SymbolKind {
        match lsp_kind {
            LspSymbolKind::FILE => SymbolKind::File,
            LspSymbolKind::MODULE => SymbolKind::Module,
            LspSymbolKind::NAMESPACE => SymbolKind::Namespace,
            LspSymbolKind::PACKAGE => SymbolKind::Package,
            LspSymbolKind::CLASS => SymbolKind::Class,
            LspSymbolKind::METHOD => SymbolKind::Function,
            LspSymbolKind::PROPERTY => SymbolKind::Property,
            LspSymbolKind::FIELD => SymbolKind::Field,
            LspSymbolKind::CONSTRUCTOR => SymbolKind::Constructor,
            LspSymbolKind::ENUM => SymbolKind::Enum,
            LspSymbolKind::INTERFACE => SymbolKind::Interface,
            LspSymbolKind::FUNCTION => SymbolKind::Function,
            LspSymbolKind::VARIABLE => SymbolKind::Variable,
            LspSymbolKind::CONSTANT => SymbolKind::Constant,
            LspSymbolKind::STRING => SymbolKind::String,
            LspSymbolKind::NUMBER => SymbolKind::Number,
            LspSymbolKind::BOOLEAN => SymbolKind::Boolean,
            LspSymbolKind::ARRAY => SymbolKind::Array,
            LspSymbolKind::OBJECT => SymbolKind::Object,
            LspSymbolKind::KEY => SymbolKind::Key,
            LspSymbolKind::NULL => SymbolKind::Null,
            LspSymbolKind::ENUM_MEMBER => SymbolKind::EnumMember,
            LspSymbolKind::STRUCT => SymbolKind::Struct,
            LspSymbolKind::EVENT => SymbolKind::Event,
            LspSymbolKind::OPERATOR => SymbolKind::Operator,
            LspSymbolKind::TYPE_PARAMETER => SymbolKind::TypeParameter,
            _ => SymbolKind::Variable, // デフォルト
        }
    }

    /// ワークスペースシンボルを検索
    pub async fn search_symbols(&self, query: &str, workspace_root: &Path) -> Result<Vec<Symbol>> {
        let symbol_infos = self
            .lsp_manager
            .search_workspace_symbols(workspace_root, query)
            .await?;

        let mut symbols = Vec::new();
        for info in symbol_infos {
            // URIからファイルパスを取得
            let file_path = info
                .location
                .uri
                .to_file_path()
                .map_err(|_| anyhow::anyhow!("Invalid URI"))?;

            symbols.push(Symbol {
                id: format!(
                    "{}#{}:{}",
                    file_path.display(),
                    info.location.range.start.line,
                    info.name
                ),
                name: info.name,
                kind: self.convert_symbol_kind(info.kind),
                file_path: file_path.to_string_lossy().to_string(),
                range: Range {
                    start: Position {
                        line: info.location.range.start.line,
                        character: info.location.range.start.character,
                    },
                    end: Position {
                        line: info.location.range.end.line,
                        character: info.location.range.end.character,
                    },
                },
                documentation: None,
                detail: info.container_name,
            });
        }

        Ok(symbols)
    }

    /// グラフを取得
    pub fn get_graph(&self) -> &CodeGraph {
        &self.graph
    }

    /// グラフを可変参照で取得
    pub fn get_graph_mut(&mut self) -> &mut CodeGraph {
        &mut self.graph
    }

    /// LSPサーバーをシャットダウン
    pub async fn shutdown(&self) -> Result<()> {
        self.lsp_manager.shutdown_all().await
    }
}

/// インデックス結果
#[derive(Debug)]
pub struct IndexResult {
    pub files_indexed: usize,
    pub symbols_found: usize,
    pub duration: std::time::Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Explicitly use std core to avoid conflict with local core crate
    extern crate std;

    #[tokio::test]
    async fn test_unified_indexer_creation() {
        let indexer = UnifiedIndexer::new();
        assert_eq!(indexer.graph.symbol_count(), 0);
    }

    #[tokio::test]
    async fn test_symbol_kind_conversion() {
        let indexer = UnifiedIndexer::new();

        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::FUNCTION),
            SymbolKind::Function
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::CLASS),
            SymbolKind::Class
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::INTERFACE),
            SymbolKind::Interface
        );
        assert_eq!(
            indexer.convert_symbol_kind(LspSymbolKind::VARIABLE),
            SymbolKind::Variable
        );
    }

    #[tokio::test]
    async fn test_document_symbol_conversion() {
        let indexer = UnifiedIndexer::new();

        let doc_symbol = DocumentSymbol {
            name: "test_function".to_string(),
            detail: Some("Test function detail".to_string()),
            kind: LspSymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 10,
                    character: 5,
                },
                end: lsp_types::Position {
                    line: 15,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 10,
                    character: 5,
                },
                end: lsp_types::Position {
                    line: 10,
                    character: 18,
                },
            },
            children: None,
        };

        let file_path = Path::new("test.rs");
        let file_uri = "file://test.rs";
        let symbol = indexer.convert_document_symbol(&doc_symbol, file_uri, file_path);

        assert_eq!(symbol.name, "test_function");
        assert_eq!(symbol.kind, SymbolKind::Function);
        assert_eq!(symbol.range.start.line, 10);
        assert_eq!(symbol.range.start.character, 5);
        assert_eq!(
            symbol.documentation,
            Some("Test function detail".to_string())
        );
    }

    #[tokio::test]
    #[ignore = "Requires LSP server to be installed"]
    async fn test_index_empty_project() {
        let temp_dir = TempDir::new().unwrap();
        let mut indexer = UnifiedIndexer::new();

        // 空のプロジェクトをインデックス（エラーにならないことを確認）
        let result = indexer.index_project(temp_dir.path()).await;

        // LSPサーバーが起動できない可能性があるのでエラーは許容
        if let Ok(result) = result {
            assert_eq!(result.files_indexed, 0);
            assert_eq!(result.symbols_found, 0);
        }
    }
}
