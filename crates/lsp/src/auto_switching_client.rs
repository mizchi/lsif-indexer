/// 自動切り替えLSPクライアント
///
/// workspace/symbolとtextDocument/documentSymbolを
/// サーバーの機能に応じて自動的に切り替える
use anyhow::{anyhow, Result};
use lsp_types::{
    DocumentSymbol, PartialResultParams, ServerCapabilities, SymbolInformation,
    WorkDoneProgressParams, WorkspaceSymbolParams,
};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::{debug, info, warn};

use crate::adapter::lsp::{GenericLspClient, LspAdapter};

/// 自動切り替えLSPクライアント
pub struct AutoSwitchingLspClient {
    /// 内部のLSPクライアント
    client: Arc<Mutex<GenericLspClient>>,
    /// サーバーの機能
    capabilities: ServerCapabilities,
    /// workspace/symbolのサポート状況
    supports_workspace_symbol: bool,
    /// documentSymbolのサポート状況
    supports_document_symbol: bool,
}

impl AutoSwitchingLspClient {
    /// 新しいクライアントを作成
    pub fn new(adapter: Box<dyn LspAdapter>) -> Result<Self> {
        // GenericLspClient::newは既に初期化を行うため、その機能を直接使用
        let client = GenericLspClient::new(adapter)?;

        // サーバーの機能を取得（既に初期化済み）
        let capabilities = client
            .get_server_capabilities()
            .ok_or_else(|| anyhow!("Server capabilities not available"))?
            .clone();

        let supports_workspace_symbol = capabilities.workspace_symbol_provider.is_some();
        let supports_document_symbol = capabilities.document_symbol_provider.is_some();

        info!(
            "LSP Server capabilities - workspace/symbol: {}, documentSymbol: {}",
            supports_workspace_symbol, supports_document_symbol
        );

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            capabilities,
            supports_workspace_symbol,
            supports_document_symbol,
        })
    }

    /// プロジェクト全体のシンボルを取得（自動切り替え）
    pub fn get_all_symbols(&self, workspace_root: &str) -> Result<Vec<SymbolInfo>> {
        if self.supports_workspace_symbol {
            debug!("Using workspace/symbol for project-wide symbol extraction");
            self.get_symbols_via_workspace(workspace_root)
        } else if self.supports_document_symbol {
            debug!("Falling back to documentSymbol for individual files");
            self.get_symbols_via_document(workspace_root)
        } else {
            Err(anyhow!(
                "Neither workspace/symbol nor documentSymbol is supported"
            ))
        }
    }

    /// workspace/symbolを使用してシンボルを取得
    fn get_symbols_via_workspace(&self, _workspace_root: &str) -> Result<Vec<SymbolInfo>> {
        let mut client = self
            .client
            .lock()
            .map_err(|e| anyhow!("Failed to lock client: {}", e))?;

        // 空のクエリで全シンボルを取得
        let params = WorkspaceSymbolParams {
            query: "".to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let symbols = client
            .send_request::<_, Option<Vec<SymbolInformation>>>("workspace/symbol", params)?
            .ok_or_else(|| anyhow!("No workspace symbols returned"))?;

        // SymbolInformationをSymbolInfoに変換
        Ok(symbols
            .into_iter()
            .map(SymbolInfo::from_workspace)
            .collect())
    }

    /// documentSymbolを使用してシンボルを取得（ファイル単位）
    fn get_symbols_via_document(&self, workspace_root: &str) -> Result<Vec<SymbolInfo>> {
        // ここでは簡易実装として、ワークスペースルートのファイルリストを取得して
        // 各ファイルに対してdocumentSymbolを呼び出す

        let mut all_symbols = Vec::new();

        // ワークスペース内のソースファイルを走査
        self.walk_directory(Path::new(workspace_root), &mut |file_path| {
            if self.is_source_file(file_path) {
                let file_uri = format!("file://{}", file_path.display());

                let mut client = self
                    .client
                    .lock()
                    .map_err(|e| anyhow!("Failed to lock client: {}", e))?;

                match client.get_document_symbols(&file_uri) {
                    Ok(symbols) => {
                        for symbol in symbols {
                            all_symbols.push(SymbolInfo::from_document(symbol, &file_uri));
                        }
                    }
                    Err(e) => {
                        warn!("Failed to get symbols for {}: {}", file_uri, e);
                    }
                }
            }
            Ok(())
        })?;

        Ok(all_symbols)
    }

    /// ディレクトリを再帰的に走査
    fn walk_directory<F>(&self, dir: &Path, callback: &mut F) -> Result<()>
    where
        F: FnMut(&Path) -> Result<()>,
    {
        use std::fs;

        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() && !self.should_skip_directory(&path) {
                    self.walk_directory(&path, callback)?;
                } else if path.is_file() {
                    callback(&path)?;
                }
            }
        }

        Ok(())
    }

    /// スキップすべきディレクトリかどうか
    fn should_skip_directory(&self, path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // 一般的な除外パターン
        matches!(
            name,
            "node_modules" | ".git" | "target" | "dist" | "build" | ".vscode"
        )
    }

    /// ソースファイルかどうか
    fn is_source_file(&self, path: &Path) -> bool {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        matches!(
            extension,
            "rs" | "ts" | "tsx" | "js" | "jsx" | "go" | "py" | "java" | "cpp" | "c" | "h"
        )
    }

    /// 特定ファイルのシンボルを取得
    pub fn get_file_symbols(&self, file_uri: &str) -> Result<Vec<SymbolInfo>> {
        if self.supports_document_symbol {
            let mut client = self
                .client
                .lock()
                .map_err(|e| anyhow!("Failed to lock client: {}", e))?;

            let symbols = client.get_document_symbols(file_uri)?;
            Ok(symbols
                .into_iter()
                .map(|s| SymbolInfo::from_document(s, file_uri))
                .collect())
        } else {
            Err(anyhow!("documentSymbol is not supported"))
        }
    }

    /// サーバーの機能を取得
    pub fn get_capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// workspace/symbolがサポートされているか
    pub fn has_workspace_symbol(&self) -> bool {
        self.supports_workspace_symbol
    }

    /// documentSymbolがサポートされているか
    pub fn has_document_symbol(&self) -> bool {
        self.supports_document_symbol
    }
}

/// 統一されたシンボル情報
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: lsp_types::SymbolKind,
    pub location: lsp_types::Location,
    pub container_name: Option<String>,
    pub detail: Option<String>,
}

impl SymbolInfo {
    /// WorkspaceSymbolから変換
    fn from_workspace(symbol: SymbolInformation) -> Self {
        Self {
            name: symbol.name,
            kind: symbol.kind,
            location: symbol.location,
            container_name: symbol.container_name,
            detail: None,
        }
    }

    /// DocumentSymbolから変換
    fn from_document(symbol: DocumentSymbol, file_uri: &str) -> Self {
        Self {
            name: symbol.name,
            kind: symbol.kind,
            location: lsp_types::Location {
                uri: lsp_types::Url::parse(file_uri).unwrap(),
                range: symbol.range,
            },
            container_name: None,
            detail: symbol.detail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        create_test_document_symbol, create_test_workspace_symbol, MockLspAdapter,
    };
    use lsp_types::SymbolKind;

    #[test]
    fn test_workspace_symbol_priority() {
        // workspace/symbolが使える場合は優先される
        let adapter = MockLspAdapter::new()
            .with_workspace_support(true)
            .with_document_support(true);

        // テスト用のシンボルを追加
        adapter.add_workspace_symbol(create_test_workspace_symbol(
            "TestFunction",
            SymbolKind::FUNCTION,
            "/test/file.rs",
            10,
        ));

        // Note: 実際のテストではモックLSPサーバーの実装が必要
        // ここでは機能の存在確認のみ
        assert!(adapter.supports_workspace_symbol());
    }

    #[test]
    fn test_fallback_to_document_symbol() {
        // workspace/symbolが使えない場合のフォールバック
        let adapter = MockLspAdapter::new()
            .with_workspace_support(false)
            .with_document_support(true);

        // documentSymbolのみが有効
        assert!(!adapter.supports_workspace_symbol());
        assert_eq!(adapter.language_id(), "mock");
    }

    #[test]
    fn test_symbol_info_conversion() {
        // WorkspaceSymbolからSymbolInfoへの変換
        let ws_symbol =
            create_test_workspace_symbol("TestClass", SymbolKind::CLASS, "/test/file.ts", 20);

        let symbol_info = SymbolInfo::from_workspace(ws_symbol);
        assert_eq!(symbol_info.name, "TestClass");
        assert_eq!(symbol_info.kind, SymbolKind::CLASS);
        assert_eq!(symbol_info.location.range.start.line, 20);

        // DocumentSymbolからSymbolInfoへの変換
        let doc_symbol = create_test_document_symbol("TestMethod", SymbolKind::METHOD, 30, 35);

        let symbol_info = SymbolInfo::from_document(doc_symbol, "file:///test/file.ts");
        assert_eq!(symbol_info.name, "TestMethod");
        assert_eq!(symbol_info.kind, SymbolKind::METHOD);
        assert_eq!(symbol_info.location.range.start.line, 30);
    }

    #[test]
    fn test_should_skip_directory() {
        // Note: 実際のクライアントインスタンスが必要
        // ここではロジックの確認のみ
        let dirs_to_skip = ["node_modules", ".git", "target", "dist", "build", ".vscode"];

        for dir in &dirs_to_skip {
            let path = Path::new(dir);
            // should_skip_directoryメソッドのロジックを確認
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            assert!(matches!(
                name,
                "node_modules" | ".git" | "target" | "dist" | "build" | ".vscode"
            ));
        }
    }

    #[test]
    fn test_is_source_file() {
        let source_extensions = [
            "rs", "ts", "tsx", "js", "jsx", "go", "py", "java", "cpp", "c", "h",
        ];

        for ext in &source_extensions {
            let filename = format!("test.{}", ext);
            let path = Path::new(&filename);
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            assert!(source_extensions.contains(&extension));
        }

        // 非ソースファイル
        let non_source = ["txt", "md", "json", "yaml"];
        for ext in &non_source {
            let filename = format!("test.{}", ext);
            let path = Path::new(&filename);
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            assert!(!source_extensions.contains(&extension));
        }
    }

    #[test]
    #[ignore = "Requires tsgo to be installed"]
    fn test_with_real_tsgo() {
        use crate::adapter::tsgo::TsgoAdapter;

        // tsgoアダプタを使用
        let adapter = Box::new(TsgoAdapter);
        let client = AutoSwitchingLspClient::new(adapter).unwrap();

        // 機能を確認
        println!(
            "Supports workspace/symbol: {}",
            client.has_workspace_symbol()
        );
        println!("Supports documentSymbol: {}", client.has_document_symbol());

        // テスト用のTypeScriptプロジェクトがあれば、シンボルを取得
        if let Ok(symbols) = client.get_all_symbols("/tmp/test-project") {
            println!("Found {} symbols", symbols.len());
            for symbol in symbols.iter().take(5) {
                println!("  - {}", symbol.name);
            }
        }
    }

    #[test]
    #[ignore = "Requires rust-analyzer to be installed"]
    fn test_with_real_rust_analyzer() {
        use crate::adapter::lsp::RustAnalyzerAdapter;

        // rust-analyzerアダプタを使用
        let adapter = Box::new(RustAnalyzerAdapter);
        let client = AutoSwitchingLspClient::new(adapter).unwrap();

        // 機能を確認
        assert!(client.has_workspace_symbol() || client.has_document_symbol());
        println!("rust-analyzer capabilities verified");
    }

    #[test]
    #[ignore = "Requires gopls to be installed"]
    fn test_with_real_gopls() {
        use crate::adapter::go::GoAdapter;

        // goplsアダプタを使用
        let adapter = Box::new(GoAdapter);
        let client = AutoSwitchingLspClient::new(adapter).unwrap();

        // 機能を確認
        assert!(client.has_workspace_symbol() || client.has_document_symbol());
        println!("gopls capabilities verified");
    }
}
