use anyhow::Result;
use lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
    GotoImplementationResponse, GotoTypeDefinitionParams, GotoTypeDefinitionResponse,
};
use lsp_types::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::info;

use super::adapter::lsp::{GenericLspClient, LspAdapter};

/// 高度なLSP機能を提供するクライアント
pub struct LspClient {
    pub client: Arc<Mutex<GenericLspClient>>,
    /// ファイルごとの診断情報を保持
    diagnostics: Arc<Mutex<HashMap<Url, Vec<Diagnostic>>>>,
    /// プログレス情報を保持
    progress: Arc<Mutex<HashMap<ProgressToken, WorkDoneProgress>>>,
}

impl LspClient {
    pub fn new(adapter: Box<dyn LspAdapter>) -> Result<Self> {
        let client = GenericLspClient::new(adapter)?;

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            diagnostics: Arc::new(Mutex::new(HashMap::new())),
            progress: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// ホバー情報を取得
    pub fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/hover", params)
    }

    /// 補完を取得
    pub fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/completion", params)
    }

    /// 補完アイテムの詳細を取得
    pub fn completion_resolve(&self, item: CompletionItem) -> Result<CompletionItem> {
        let mut client = self.client.lock().unwrap();
        client.send_request("completionItem/resolve", item)
    }

    /// シグネチャヘルプを取得
    pub fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/signatureHelp", params)
    }

    /// コールヒエラルキーの準備
    pub fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/prepareCallHierarchy", params)
    }

    /// 呼び出し元を取得
    pub fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("callHierarchy/incomingCalls", params)
    }

    /// 呼び出し先を取得
    pub fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("callHierarchy/outgoingCalls", params)
    }

    /// 型階層の準備
    pub fn prepare_type_hierarchy(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/prepareTypeHierarchy", params)
    }

    /// スーパータイプを取得
    pub fn type_hierarchy_supertypes(
        &self,
        params: TypeHierarchySupertypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("typeHierarchy/supertypes", params)
    }

    /// サブタイプを取得
    pub fn type_hierarchy_subtypes(
        &self,
        params: TypeHierarchySubtypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("typeHierarchy/subtypes", params)
    }

    /// ドキュメントハイライトを取得
    pub fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/documentHighlight", params)
    }

    /// ドキュメントリンクを取得
    pub fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/documentLink", params)
    }

    /// コードレンズを取得
    pub fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/codeLens", params)
    }

    /// コードレンズを解決
    pub fn code_lens_resolve(&self, code_lens: CodeLens) -> Result<CodeLens> {
        let mut client = self.client.lock().unwrap();
        client.send_request("codeLens/resolve", code_lens)
    }

    /// ドキュメントカラーを取得
    pub fn document_color(&self, params: DocumentColorParams) -> Result<Vec<ColorInformation>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/documentColor", params)
    }

    /// カラープレゼンテーションを取得
    pub fn color_presentation(
        &self,
        params: ColorPresentationParams,
    ) -> Result<Vec<ColorPresentation>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/colorPresentation", params)
    }

    /// フォーマットを実行
    pub fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/formatting", params)
    }

    /// 範囲フォーマットを実行
    pub fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/rangeFormatting", params)
    }

    /// 型フォーマットを実行
    pub fn on_type_formatting(
        &self,
        params: DocumentOnTypeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/onTypeFormatting", params)
    }

    /// リネームを実行
    pub fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/rename", params)
    }

    /// リネーム準備
    pub fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/prepareRename", params)
    }

    /// 折りたたみ範囲を取得
    pub fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/foldingRange", params)
    }

    /// 選択範囲を取得
    pub fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/selectionRange", params)
    }

    /// セマンティックトークンを取得（フル）
    pub fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/semanticTokens/full", params)
    }

    /// セマンティックトークンを取得（差分）
    pub fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> Result<Option<SemanticTokensFullDeltaResult>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/semanticTokens/full/delta", params)
    }

    /// セマンティックトークンを取得（範囲）
    pub fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/semanticTokens/range", params)
    }

    /// モニカーを取得
    pub fn moniker(&self, params: MonikerParams) -> Result<Option<Vec<Moniker>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/moniker", params)
    }

    /// インレイヒントを取得
    pub fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/inlayHint", params)
    }

    /// インレイヒントを解決
    pub fn inlay_hint_resolve(&self, hint: InlayHint) -> Result<InlayHint> {
        let mut client = self.client.lock().unwrap();
        client.send_request("inlayHint/resolve", hint)
    }

    /// インラインバリューを取得
    pub fn inline_value(&self, params: InlineValueParams) -> Result<Option<Vec<InlineValue>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/inlineValue", params)
    }

    /// コードアクションを取得
    pub fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/codeAction", params)
    }

    /// コードアクションを解決
    pub fn code_action_resolve(&self, action: CodeAction) -> Result<CodeAction> {
        let mut client = self.client.lock().unwrap();
        client.send_request("codeAction/resolve", action)
    }

    /// ワークスペースシンボルを取得
    pub fn workspace_symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("workspace/symbol", params)
    }

    /// ワークスペースシンボルを解決
    pub fn workspace_symbol_resolve(&self, symbol: WorkspaceSymbol) -> Result<WorkspaceSymbol> {
        let mut client = self.client.lock().unwrap();
        client.send_request("workspaceSymbol/resolve", symbol)
    }

    /// コマンドを実行
    pub fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("workspace/executeCommand", params)
    }

    /// 診断情報を取得
    pub fn get_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        self.diagnostics
            .lock()
            .unwrap()
            .get(uri)
            .cloned()
            .unwrap_or_default()
    }

    /// 診断情報を更新
    pub fn update_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) {
        self.diagnostics.lock().unwrap().insert(uri, diagnostics);
    }

    /// プログレス情報を取得
    pub fn get_progress(&self, token: &ProgressToken) -> Option<WorkDoneProgress> {
        self.progress.lock().unwrap().get(token).cloned()
    }

    /// プログレス情報を更新
    pub fn update_progress(&self, token: ProgressToken, progress: WorkDoneProgress) {
        self.progress.lock().unwrap().insert(token, progress);
    }

    /// 実装へジャンプ
    pub fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/implementation", params)
    }

    /// 型定義へジャンプ
    pub fn goto_type_definition(
        &self,
        params: GotoTypeDefinitionParams,
    ) -> Result<Option<GotoTypeDefinitionResponse>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/typeDefinition", params)
    }

    /// 宣言へジャンプ
    pub fn goto_declaration(
        &self,
        params: GotoDeclarationParams,
    ) -> Result<Option<GotoDeclarationResponse>> {
        let mut client = self.client.lock().unwrap();
        client.send_request("textDocument/declaration", params)
    }
}

/// 診断情報を監視するワーカー
pub struct DiagnosticsWatcher {
    client: Arc<LspClient>,
    rx: mpsc::Receiver<PublishDiagnosticsParams>,
}

impl DiagnosticsWatcher {
    pub fn new(client: Arc<LspClient>) -> (Self, mpsc::Sender<PublishDiagnosticsParams>) {
        let (tx, rx) = mpsc::channel(100);
        (Self { client, rx }, tx)
    }

    pub async fn run(mut self) {
        while let Some(params) = self.rx.recv().await {
            info!("Received diagnostics for {}", params.uri);
            self.client
                .update_diagnostics(params.uri, params.diagnostics);
        }
    }
}

/// プログレス情報を監視するワーカー
pub struct ProgressWatcher {
    client: Arc<LspClient>,
    rx: mpsc::Receiver<ProgressParams>,
}

impl ProgressWatcher {
    pub fn new(client: Arc<LspClient>) -> (Self, mpsc::Sender<ProgressParams>) {
        let (tx, rx) = mpsc::channel(100);
        (Self { client, rx }, tx)
    }

    pub async fn run(mut self) {
        while let Some(params) = self.rx.recv().await {
            match params.value {
                ProgressParamsValue::WorkDone(progress) => {
                    info!("Work done progress: {:?}", progress);
                    self.client.update_progress(params.token, progress);
                }
            }
        }
    }
}

/// LSPベースのコード解析ユーティリティ
pub struct LspCodeAnalyzer {
    client: Arc<LspClient>,
}

impl LspCodeAnalyzer {
    pub fn new(client: Arc<LspClient>) -> Self {
        Self { client }
    }

    /// ファイル内のすべてのシンボルを取得して階層構造を構築
    pub fn analyze_file_structure(&self, file_uri: &str) -> Result<FileStructure> {
        let mut client = self.client.client.lock().unwrap();
        let symbols = client.get_document_symbols(file_uri)?;

        Ok(FileStructure {
            uri: file_uri.to_string(),
            symbols,
        })
    }

    /// 指定位置のシンボルの完全な情報を取得
    pub fn get_symbol_info(&self, uri: &Url, position: Position) -> Result<SymbolInfo> {
        let hover = self.client.hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        })?;

        let definition = self
            .client
            .client
            .lock()
            .unwrap()
            .goto_definition(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .ok();

        let references = self
            .client
            .client
            .lock()
            .unwrap()
            .find_references(ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                context: ReferenceContext {
                    include_declaration: true,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })?;

        Ok(SymbolInfo {
            position,
            hover,
            definition,
            references,
        })
    }

    /// 依存関係グラフを構築
    pub fn build_dependency_graph(&self, root_uri: &str) -> Result<DependencyGraph> {
        let mut graph = DependencyGraph::new();
        let mut visited = std::collections::HashSet::new();

        self.analyze_dependencies_recursive(root_uri, &mut graph, &mut visited)?;

        Ok(graph)
    }

    fn analyze_dependencies_recursive(
        &self,
        uri: &str,
        graph: &mut DependencyGraph,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        if visited.contains(uri) {
            return Ok(());
        }
        visited.insert(uri.to_string());

        // ドキュメントシンボルを取得
        let mut client = self.client.client.lock().unwrap();
        let symbols = client.get_document_symbols(uri)?;

        // 各シンボルの参照を解析
        for symbol in symbols {
            self.analyze_symbol_dependencies(&symbol, uri, graph)?;
        }

        Ok(())
    }

    fn analyze_symbol_dependencies(
        &self,
        symbol: &DocumentSymbol,
        uri: &str,
        graph: &mut DependencyGraph,
    ) -> Result<()> {
        // シンボルの参照を取得
        let references = self
            .client
            .client
            .lock()
            .unwrap()
            .find_references(ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: Url::parse(uri)?,
                    },
                    position: symbol.selection_range.start,
                },
                context: ReferenceContext {
                    include_declaration: false,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })?;

        for reference in references {
            graph.add_dependency(uri, reference.uri.as_str());
        }

        // 子シンボルも再帰的に解析
        if let Some(children) = &symbol.children {
            for child in children {
                self.analyze_symbol_dependencies(child, uri, graph)?;
            }
        }

        Ok(())
    }
}

/// ファイル構造情報
#[derive(Debug, Clone)]
pub struct FileStructure {
    pub uri: String,
    pub symbols: Vec<DocumentSymbol>,
}

/// シンボル情報
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub position: Position,
    pub hover: Option<Hover>,
    pub definition: Option<Location>,
    pub references: Vec<Location>,
}

/// 依存関係グラフ
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    edges: HashMap<String, Vec<String>>,
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
        }
    }

    pub fn add_dependency(&mut self, from: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_default()
            .push(to.to_string());
    }

    pub fn get_dependencies(&self, uri: &str) -> Option<&Vec<String>> {
        self.edges.get(uri)
    }

    pub fn get_all_dependencies(&self) -> &HashMap<String, Vec<String>> {
        &self.edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::lsp::RustAnalyzerAdapter;

    #[test]
    #[ignore = "Requires LSP server to be installed"]
    fn test_advanced_lsp_features() {
        let adapter = Box::new(RustAnalyzerAdapter);
        let client = Arc::new(LspClient::new(adapter).unwrap());

        // Test hover
        let hover_params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::parse("file:///test.rs").unwrap(),
                },
                position: Position {
                    line: 0,
                    character: 0,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        // This would fail without a real file, but tests the API
        let _ = client.hover(hover_params);

        // Test code analyzer
        let analyzer = LspCodeAnalyzer::new(client);
        let _ = analyzer.analyze_file_structure("file:///test.rs");
    }
}
