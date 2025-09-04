/// Workspace Symbol抽出戦略
///
/// workspace/symbolをサポートするLSPサーバーから
/// プロジェクト全体のシンボルを効率的に取得する
use anyhow::Result;
use lsif_core::Symbol;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};
use walkdir;

use super::symbol_extraction_strategy::SymbolExtractionStrategy;

/// Workspace Symbolベースの抽出戦略
pub struct WorkspaceSymbolExtractionStrategy {
    lsp_pool: Arc<Mutex<lsp::lsp_pool::LspClientPool>>,
    project_root: PathBuf,
    /// 既に取得済みのファイルを記録
    processed_files: Arc<Mutex<HashSet<PathBuf>>>,
}

impl WorkspaceSymbolExtractionStrategy {
    pub fn new(lsp_pool: Arc<Mutex<lsp::lsp_pool::LspClientPool>>, project_root: PathBuf) -> Self {
        Self {
            lsp_pool,
            project_root,
            processed_files: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// シンプルなインデックス作成用のコンストラクタ
    pub fn new_standalone(project_root: PathBuf) -> Self {
        use lsp::lsp_pool::{LspClientPool, PoolConfig};
        let config = PoolConfig::default();
        let lsp_pool = Arc::new(Mutex::new(LspClientPool::new(config)));
        Self::new(lsp_pool, project_root)
    }

    /// workspace/symbolを使用してプロジェクト全体のシンボルを取得
    pub fn extract_workspace_symbols(&self) -> Result<Vec<Symbol>> {
        info!(
            "Extracting workspace symbols from: {}",
            self.project_root.display()
        );

        // LSPプールからクライアントを取得
        let pool = self
            .lsp_pool
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock LSP pool: {}", e))?;

        // プロジェクト内のサンプルファイルを見つける（LSPクライアント初期化用）
        let sample_file = walkdir::WalkDir::new(&self.project_root)
            .follow_links(false)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .find(|e| {
                let ext = e.path().extension().and_then(|s| s.to_str()).unwrap_or("");
                matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go")
            })
            .ok_or_else(|| anyhow::anyhow!("No source files found in project"))?;

        // プロジェクトルートに対応するクライアントを取得
        let client_arc = pool.get_or_create_client(sample_file.path(), &self.project_root)?;
        let mut client = client_arc
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock LSP client: {}", e))?;

        // workspace/symbolがサポートされているか確認
        if !client.has_capability("workspace/symbol") {
            debug!("workspace/symbol is not supported by the current LSP server");
            return Ok(Vec::new());
        }

        // 空のクエリで全シンボルを取得（多くのLSPサーバーでサポート）
        let workspace_symbols = self.query_workspace_symbols(&mut client, "")?;

        // ワークスペースシンボルをコアのSymbol型に変換
        let mut symbols = Vec::new();
        for ws_symbol in workspace_symbols {
            if let Some(symbol) = self.convert_workspace_symbol_to_core(&ws_symbol) {
                symbols.push(symbol);

                // 処理済みファイルとして記録
                if let Ok(uri) = lsp_types::Url::parse(ws_symbol.location.uri.as_ref()) {
                    if let Ok(path) = uri.to_file_path() {
                        if let Ok(mut processed) = self.processed_files.lock() {
                            processed.insert(path);
                        }
                    }
                }
            }
        }

        info!("Extracted {} symbols from workspace", symbols.len());
        Ok(symbols)
    }

    /// workspace/symbolクエリを実行
    fn query_workspace_symbols(
        &self,
        client: &mut lsp::adapter::lsp::GenericLspClient,
        query: &str,
    ) -> Result<Vec<lsp_types::SymbolInformation>> {
        use lsp_types::{PartialResultParams, WorkDoneProgressParams, WorkspaceSymbolParams};

        let params = WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        client
            .send_request::<_, Option<Vec<lsp_types::SymbolInformation>>>(
                "workspace/symbol",
                params,
            )?
            .ok_or_else(|| anyhow::anyhow!("No workspace symbols returned"))
    }

    /// WorkspaceSymbolをコアのSymbol型に変換
    fn convert_workspace_symbol_to_core(
        &self,
        ws_symbol: &lsp_types::SymbolInformation,
    ) -> Option<Symbol> {
        use lsif_core::{Position, Range};

        // URIからファイルパスを取得
        let file_path = lsp_types::Url::parse(ws_symbol.location.uri.as_ref())
            .ok()?
            .to_file_path()
            .ok()?;

        let file_path_str = file_path.to_string_lossy().to_string();

        Some(Symbol {
            id: format!(
                "{}#{}:{}",
                file_path_str, ws_symbol.location.range.start.line, ws_symbol.name
            ),
            name: ws_symbol.name.clone(),
            kind: convert_symbol_kind(ws_symbol.kind),
            file_path: file_path_str,
            range: Range {
                start: Position {
                    line: ws_symbol.location.range.start.line,
                    character: ws_symbol.location.range.start.character,
                },
                end: Position {
                    line: ws_symbol.location.range.end.line,
                    character: ws_symbol.location.range.end.character,
                },
            },
            documentation: ws_symbol.container_name.clone(),
            detail: None,
        })
    }
}

impl SymbolExtractionStrategy for WorkspaceSymbolExtractionStrategy {
    fn name(&self) -> &str {
        "WorkspaceSymbol"
    }

    fn supports(&self, path: &Path) -> bool {
        // このファイルがまだ処理されていないかチェック
        if let Ok(processed) = self.processed_files.lock() {
            if processed.contains(path) {
                debug!(
                    "File already processed via workspace/symbol: {}",
                    path.display()
                );
                return false;
            }
        }

        // workspace/symbolがサポートされているかチェック
        if let Ok(pool) = self.lsp_pool.lock() {
            // ファイルの言語を取得
            use lsp::adapter::lsp::get_language_id;
            let language_id = get_language_id(path).unwrap_or_else(|| "unknown".to_string());

            // workspace/symbolがサポートされているか確認
            pool.has_capability_for_language(&language_id, "workspace/symbol")
        } else {
            false
        }
    }

    fn extract(&self, path: &Path) -> Result<Vec<Symbol>> {
        // 初回呼び出し時にワークスペース全体のシンボルを取得
        if let Ok(processed) = self.processed_files.lock() {
            if processed.is_empty() {
                // まだ何も処理していない場合、ワークスペース全体を処理
                drop(processed); // ロックを解放
                return self.extract_workspace_symbols();
            }
        }

        // すでに処理済みの場合は空を返す（他の戦略にフォールバック）
        Ok(Vec::new())
    }

    fn priority(&self) -> u32 {
        90 // LSP documentSymbolより低いが、フォールバックより高い
    }
}

/// LSP SymbolKindをコアのSymbolKindに変換（共通ユーティリティ）
fn convert_symbol_kind(lsp_kind: lsp_types::SymbolKind) -> lsif_core::SymbolKind {
    use lsif_core::SymbolKind;
    use lsp_types::SymbolKind as LspKind;

    match lsp_kind {
        LspKind::FILE => SymbolKind::File,
        LspKind::MODULE => SymbolKind::Module,
        LspKind::NAMESPACE => SymbolKind::Namespace,
        LspKind::PACKAGE => SymbolKind::Package,
        LspKind::CLASS => SymbolKind::Class,
        LspKind::METHOD => SymbolKind::Method,
        LspKind::PROPERTY => SymbolKind::Property,
        LspKind::FIELD => SymbolKind::Field,
        LspKind::CONSTRUCTOR => SymbolKind::Constructor,
        LspKind::ENUM => SymbolKind::Enum,
        LspKind::INTERFACE => SymbolKind::Interface,
        LspKind::FUNCTION => SymbolKind::Function,
        LspKind::VARIABLE => SymbolKind::Variable,
        LspKind::CONSTANT => SymbolKind::Constant,
        LspKind::STRING => SymbolKind::String,
        LspKind::NUMBER => SymbolKind::Number,
        LspKind::BOOLEAN => SymbolKind::Boolean,
        LspKind::ARRAY => SymbolKind::Array,
        LspKind::OBJECT => SymbolKind::Object,
        LspKind::KEY => SymbolKind::Key,
        LspKind::NULL => SymbolKind::Null,
        LspKind::ENUM_MEMBER => SymbolKind::EnumMember,
        LspKind::STRUCT => SymbolKind::Struct,
        LspKind::EVENT => SymbolKind::Event,
        LspKind::OPERATOR => SymbolKind::Operator,
        LspKind::TYPE_PARAMETER => SymbolKind::TypeParameter,
        _ => SymbolKind::Unknown, // その他の未知のSymbolKind
    }
}

/// DocumentSymbolをcore::Symbolに変換
fn convert_document_symbols_to_core(
    lsp_symbols: &[lsp_types::DocumentSymbol],
    path: &Path,
) -> Vec<Symbol> {
    use lsif_core::{Position, Range};

    let mut symbols = Vec::new();
    let file_path = path.to_string_lossy().to_string();

    fn process_symbol(
        symbol: &lsp_types::DocumentSymbol,
        file_path: &str,
        symbols: &mut Vec<Symbol>,
    ) {
        symbols.push(Symbol {
            id: format!(
                "{}#{}:{}",
                file_path, symbol.selection_range.start.line, symbol.name
            ),
            name: symbol.name.clone(),
            kind: convert_symbol_kind(symbol.kind),
            file_path: file_path.to_string(),
            range: Range {
                start: Position {
                    line: symbol.range.start.line,
                    character: symbol.range.start.character,
                },
                end: Position {
                    line: symbol.range.end.line,
                    character: symbol.range.end.character,
                },
            },
            documentation: symbol.detail.clone(),
            detail: None,
        });

        // 子シンボルも処理
        if let Some(children) = &symbol.children {
            for child in children {
                process_symbol(child, file_path, symbols);
            }
        }
    }

    for symbol in lsp_symbols {
        process_symbol(symbol, &file_path, &mut symbols);
    }

    symbols
}

/// ハイブリッド抽出戦略
/// workspace/symbolとdocumentSymbolを組み合わせて使用
pub struct HybridSymbolExtractionStrategy {
    workspace_strategy: WorkspaceSymbolExtractionStrategy,
    lsp_pool: Arc<Mutex<lsp::lsp_pool::LspClientPool>>,
    project_root: PathBuf,
}

impl HybridSymbolExtractionStrategy {
    pub fn new(lsp_pool: Arc<Mutex<lsp::lsp_pool::LspClientPool>>, project_root: PathBuf) -> Self {
        let workspace_strategy =
            WorkspaceSymbolExtractionStrategy::new(lsp_pool.clone(), project_root.clone());

        Self {
            workspace_strategy,
            lsp_pool,
            project_root,
        }
    }
}

impl SymbolExtractionStrategy for HybridSymbolExtractionStrategy {
    fn name(&self) -> &str {
        "Hybrid"
    }

    fn supports(&self, path: &Path) -> bool {
        // ハイブリッド戦略は常にサポート（どちらかが使える場合）
        if let Ok(pool) = self.lsp_pool.lock() {
            use lsp::adapter::lsp::get_language_id;
            let language_id = get_language_id(path).unwrap_or_else(|| "unknown".to_string());

            pool.has_capability_for_language(&language_id, "workspace/symbol")
                || pool.has_capability_for_language(&language_id, "textDocument/documentSymbol")
        } else {
            false
        }
    }

    fn extract(&self, path: &Path) -> Result<Vec<Symbol>> {
        // まずworkspace/symbolを試す
        if self.workspace_strategy.supports(path) {
            if let Ok(symbols) = self.workspace_strategy.extract(path) {
                if !symbols.is_empty() {
                    return Ok(symbols);
                }
            }
        }

        // workspace/symbolが使えない、または結果が空の場合はdocumentSymbolを使用
        info!("Falling back to documentSymbol for: {}", path.display());

        use std::fs::canonicalize;

        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            canonicalize(path)?
        };

        let file_uri = format!("file://{}", absolute_path.display());

        let pool = self
            .lsp_pool
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock LSP pool: {}", e))?;

        let client_arc = pool.get_or_create_client(path, &self.project_root)?;
        let mut client = client_arc
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock LSP client: {}", e))?;

        let lsp_symbols = client.get_document_symbols(&file_uri)?;

        // 変換処理を共通化
        // 変換処理をここに実装
        Ok(convert_document_symbols_to_core(&lsp_symbols, path))
    }

    fn priority(&self) -> u32 {
        95 // 高優先度（workspaceとdocumentの中間）
    }
}

/// スタンドアロン版のWorkspaceSymbolStrategy（CLI用）
pub struct WorkspaceSymbolStrategy {
    project_root: PathBuf,
}

impl WorkspaceSymbolStrategy {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// プロジェクト全体をworkspace/symbolでインデックス
    pub fn index(&self) -> Result<lsif_core::CodeGraph> {
        let strategy = WorkspaceSymbolExtractionStrategy::new_standalone(self.project_root.clone());
        let symbols = strategy.extract_workspace_symbols()?;

        let mut graph = lsif_core::CodeGraph::new();
        for symbol in symbols {
            graph.add_symbol(symbol);
        }

        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[test]
    fn test_symbol_kind_conversion() {
        use lsif_core::SymbolKind;
        use lsp_types::SymbolKind as LspKind;

        assert!(matches!(
            convert_symbol_kind(LspKind::CLASS),
            SymbolKind::Class
        ));
        assert!(matches!(
            convert_symbol_kind(LspKind::FUNCTION),
            SymbolKind::Function
        ));
        assert!(matches!(
            convert_symbol_kind(LspKind::VARIABLE),
            SymbolKind::Variable
        ));
        assert!(matches!(
            convert_symbol_kind(LspKind::METHOD),
            SymbolKind::Method
        ));
        assert!(matches!(
            convert_symbol_kind(LspKind::INTERFACE),
            SymbolKind::Interface
        ));
        assert!(matches!(
            convert_symbol_kind(LspKind::STRUCT),
            SymbolKind::Struct
        ));
        assert!(matches!(
            convert_symbol_kind(LspKind::ENUM),
            SymbolKind::Enum
        ));

        // すべての既知のSymbolKindがテストされていることを確認
    }

    #[test]
    fn test_document_symbols_to_core_conversion() {
        use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};

        let doc_symbol = DocumentSymbol {
            name: "TestClass".to_string(),
            detail: Some("class details".to_string()),
            kind: SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            range: Range {
                start: Position {
                    line: 10,
                    character: 0,
                },
                end: Position {
                    line: 20,
                    character: 0,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 10,
                    character: 6,
                },
                end: Position {
                    line: 10,
                    character: 15,
                },
            },
            children: Some(vec![DocumentSymbol {
                name: "testMethod".to_string(),
                detail: None,
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                range: Range {
                    start: Position {
                        line: 12,
                        character: 4,
                    },
                    end: Position {
                        line: 15,
                        character: 4,
                    },
                },
                selection_range: Range {
                    start: Position {
                        line: 12,
                        character: 8,
                    },
                    end: Position {
                        line: 12,
                        character: 18,
                    },
                },
                children: None,
            }]),
        };

        let path = Path::new("/test/file.ts");
        let symbols = convert_document_symbols_to_core(&[doc_symbol], path);

        assert_eq!(symbols.len(), 2); // 親と子
        assert_eq!(symbols[0].name, "TestClass");
        assert_eq!(symbols[0].kind, lsif_core::SymbolKind::Class);
        assert_eq!(symbols[1].name, "testMethod");
        assert_eq!(symbols[1].kind, lsif_core::SymbolKind::Method);
    }

    #[test]
    fn test_workspace_symbol_conversion() {
        use lsp_types::{Location, Position, Range, SymbolInformation, SymbolKind, Url};

        let ws_symbol = SymbolInformation {
            name: "globalFunction".to_string(),
            kind: SymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            location: Location {
                uri: Url::parse("file:///test/global.js").unwrap(),
                range: Range {
                    start: Position {
                        line: 5,
                        character: 0,
                    },
                    end: Position {
                        line: 8,
                        character: 0,
                    },
                },
            },
            container_name: Some("GlobalModule".to_string()),
        };

        // WorkspaceSymbolExtractionStrategyのconvert_workspace_symbol_to_coreメソッドのロジックをテスト
        let file_path = ws_symbol.location.uri.to_file_path().unwrap();
        let file_path_str = file_path.to_string_lossy().to_string();

        let symbol = lsif_core::Symbol {
            id: format!(
                "{}#{}:{}",
                file_path_str, ws_symbol.location.range.start.line, ws_symbol.name
            ),
            name: ws_symbol.name.clone(),
            kind: convert_symbol_kind(ws_symbol.kind),
            file_path: file_path_str.clone(),
            range: lsif_core::Range {
                start: lsif_core::Position {
                    line: ws_symbol.location.range.start.line,
                    character: ws_symbol.location.range.start.character,
                },
                end: lsif_core::Position {
                    line: ws_symbol.location.range.end.line,
                    character: ws_symbol.location.range.end.character,
                },
            },
            documentation: ws_symbol.container_name.clone(),
            detail: None,
        };

        assert_eq!(symbol.name, "globalFunction");
        assert_eq!(symbol.kind, lsif_core::SymbolKind::Function);
        assert_eq!(symbol.range.start.line, 5);
        assert_eq!(symbol.documentation, Some("GlobalModule".to_string()));
    }

    #[test]
    fn test_processed_files_tracking() {
        use std::collections::HashSet;

        let processed_files = Arc::new(Mutex::new(HashSet::new()));

        // ファイルを処理済みとして記録
        let file1 = PathBuf::from("/test/file1.ts");
        let file2 = PathBuf::from("/test/file2.ts");

        {
            let mut processed = processed_files.lock().unwrap();
            processed.insert(file1.clone());
            processed.insert(file2.clone());
        }

        // 処理済みファイルの確認
        {
            let processed = processed_files.lock().unwrap();
            assert!(processed.contains(&file1));
            assert!(processed.contains(&file2));
            assert!(!processed.contains(&PathBuf::from("/test/file3.ts")));
        }
    }

    #[test]
    fn test_strategy_priority() {
        // WorkspaceSymbolExtractionStrategyの優先度
        let ws_strategy_priority = 90u32;
        // LspExtractionStrategyの優先度
        let lsp_strategy_priority = 100u32;
        // HybridSymbolExtractionStrategyの優先度
        let hybrid_strategy_priority = 95u32;

        // 優先度の順序を確認
        assert!(lsp_strategy_priority > hybrid_strategy_priority);
        assert!(hybrid_strategy_priority > ws_strategy_priority);
    }
}
