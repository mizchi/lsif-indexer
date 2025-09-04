/// 最小限のLSPクライアント実装
/// 言語アダプタと組み合わせて使用する軽量なLSPクライアント
use anyhow::{anyhow, Result};
use lsp_types::{
    ClientCapabilities, DidOpenTextDocumentParams, DocumentSymbol, DocumentSymbolParams,
    DocumentSymbolResponse, InitializeParams, InitializeResult, Location, Position,
    PublishDiagnosticsClientCapabilities, ReferenceContext, ReferenceParams,
    TextDocumentClientCapabilities, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, Url, WorkspaceClientCapabilities, WorkspaceFolder,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::adapter::language::LanguageAdapter;

pub struct LspClient {
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    request_id: AtomicU64,
    adapter: Box<dyn LanguageAdapter>,
}

impl LspClient {
    /// 新しいLSPクライアントを作成
    pub fn new(adapter: Box<dyn LanguageAdapter>) -> Result<Self> {
        let mut process = adapter.spawn_lsp_command()?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to get stdin"))?;
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to get stdout"))?;

        Ok(Self {
            process,
            stdin,
            stdout: BufReader::new(stdout),
            request_id: AtomicU64::new(1),
            adapter,
        })
    }

    /// LSPサーバーを初期化（タイムアウト付き）
    pub fn initialize(&mut self, root_path: &Path) -> Result<InitializeResult> {
        self.initialize_with_timeout(root_path, Duration::from_secs(10))
    }

    /// LSPサーバーを初期化（カスタムタイムアウト付き）
    pub fn initialize_with_timeout(
        &mut self,
        root_path: &Path,
        timeout: Duration,
    ) -> Result<InitializeResult> {
        // 絶対パスに変換
        let absolute_path = root_path
            .canonicalize()
            .map_err(|e| anyhow!("Failed to get absolute path: {}", e))?;

        let root_uri = Url::from_file_path(&absolute_path)
            .map_err(|_| anyhow!("Invalid root path: {:?}", absolute_path))?;

        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_path: None,
            root_uri: None,
            initialization_options: None,
            capabilities: self.client_capabilities(),
            trace: Some(lsp_types::TraceValue::Off),
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri,
                name: absolute_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string(),
            }]),
            client_info: Some(lsp_types::ClientInfo {
                name: "lsif-indexer".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            locale: None,
            work_done_progress_params: Default::default(),
        };

        let response: InitializeResult =
            self.send_request_with_timeout("initialize", params, timeout)?;

        // initialized通知を送信
        self.send_notification("initialized", serde_json::json!({}))?;

        Ok(response)
    }

    /// ドキュメントシンボルを取得
    pub fn get_document_symbols(&mut self, file_path: &Path) -> Result<Vec<DocumentSymbol>> {
        // 絶対パスに変換
        let absolute_path = file_path
            .canonicalize()
            .map_err(|e| anyhow!("Failed to get absolute path: {}", e))?;

        // ファイルを開く
        self.open_document(&absolute_path)?;

        let uri = Url::from_file_path(&absolute_path)
            .map_err(|_| anyhow!("Invalid file path: {:?}", absolute_path))?;

        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let response: DocumentSymbolResponse =
            self.send_request("textDocument/documentSymbol", params)?;

        match response {
            DocumentSymbolResponse::Nested(symbols) => Ok(symbols),
            DocumentSymbolResponse::Flat(symbols) => {
                // SymbolInformationをDocumentSymbolに変換
                Ok(symbols
                    .into_iter()
                    .map(|s| {
                        #[allow(deprecated)]
                        DocumentSymbol {
                            name: s.name,
                            kind: s.kind,
                            tags: None,
                            deprecated: None,
                            detail: s.container_name,
                            range: s.location.range,
                            selection_range: s.location.range,
                            children: None,
                        }
                    })
                    .collect())
            }
        }
    }

    /// 参照を検索
    pub fn find_references(
        &mut self,
        file_path: &Path,
        position: Position,
    ) -> Result<Vec<Location>> {
        // 絶対パスに変換
        let absolute_path = file_path
            .canonicalize()
            .map_err(|e| anyhow!("Failed to get absolute path: {}", e))?;

        let uri = Url::from_file_path(&absolute_path)
            .map_err(|_| anyhow!("Invalid file path: {:?}", absolute_path))?;

        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            context: ReferenceContext {
                include_declaration: false,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let response: Option<Vec<Location>> =
            self.send_request("textDocument/references", params)?;

        Ok(response.unwrap_or_default())
    }

    /// 定義位置を取得
    pub fn go_to_definition(
        &mut self,
        file_path: &Path,
        position: Position,
    ) -> Result<Option<Location>> {
        // 絶対パスに変換
        let absolute_path = file_path
            .canonicalize()
            .map_err(|e| anyhow!("Failed to get absolute path: {}", e))?;

        // ファイルを開く
        self.open_document(&absolute_path)?;

        let uri = Url::from_file_path(&absolute_path)
            .map_err(|_| anyhow!("Invalid file path: {:?}", absolute_path))?;

        let params = TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        };

        let response: Option<lsp_types::GotoDefinitionResponse> =
            self.send_request("textDocument/definition", params)?;

        match response {
            Some(lsp_types::GotoDefinitionResponse::Scalar(location)) => Ok(Some(location)),
            Some(lsp_types::GotoDefinitionResponse::Array(mut locations)) => Ok(locations.pop()),
            Some(lsp_types::GotoDefinitionResponse::Link(mut links)) => {
                Ok(links.pop().map(|link| Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                }))
            }
            None => Ok(None),
        }
    }

    /// 型情報を取得（ホバー）
    pub fn get_hover(&mut self, file_path: &Path, position: Position) -> Result<Option<String>> {
        // 絶対パスに変換
        let absolute_path = file_path
            .canonicalize()
            .map_err(|e| anyhow!("Failed to get absolute path: {}", e))?;

        // ファイルを開く
        self.open_document(&absolute_path)?;

        let uri = Url::from_file_path(&absolute_path)
            .map_err(|_| anyhow!("Invalid file path: {:?}", absolute_path))?;

        let params = lsp_types::HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
        };

        let response: Option<lsp_types::Hover> = self.send_request("textDocument/hover", params)?;

        Ok(response.map(|hover| match hover.contents {
            lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(s)) => s,
            lsp_types::HoverContents::Scalar(lsp_types::MarkedString::LanguageString(ls)) => {
                ls.value
            }
            lsp_types::HoverContents::Array(arr) => {
                let strings: Vec<String> = arr
                    .into_iter()
                    .map(|ms| match ms {
                        lsp_types::MarkedString::String(s) => s,
                        lsp_types::MarkedString::LanguageString(ls) => ls.value,
                    })
                    .collect();
                strings.join("\n")
            }
            lsp_types::HoverContents::Markup(markup) => markup.value,
        }))
    }

    /// シャットダウン
    pub fn shutdown(&mut self) -> Result<()> {
        let _: Value = self.send_request("shutdown", Value::Null)?;
        self.send_notification("exit", Value::Null)?;

        // プロセスの終了を待つ
        let _ = self.process.wait();

        Ok(())
    }

    // Private methods

    fn client_capabilities(&self) -> ClientCapabilities {
        ClientCapabilities {
            workspace: Some(WorkspaceClientCapabilities {
                apply_edit: Some(false),
                workspace_edit: None,
                did_change_configuration: None,
                did_change_watched_files: None,
                symbol: None,
                execute_command: None,
                workspace_folders: Some(true),
                configuration: Some(false),
                semantic_tokens: None,
                code_lens: None,
                file_operations: None,
                diagnostic: None,
                inlay_hint: None,
                inline_value: None,
            }),
            text_document: Some(TextDocumentClientCapabilities {
                synchronization: None,
                completion: None,
                hover: None,
                signature_help: None,
                references: None,
                document_highlight: None,
                document_symbol: None,
                formatting: None,
                range_formatting: None,
                on_type_formatting: None,
                declaration: None,
                definition: None,
                type_definition: None,
                implementation: None,
                code_action: None,
                code_lens: None,
                document_link: None,
                color_provider: None,
                rename: None,
                publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                    related_information: Some(false),
                    tag_support: None,
                    version_support: Some(false),
                    code_description_support: None,
                    data_support: None,
                }),
                folding_range: None,
                selection_range: None,
                linked_editing_range: None,
                call_hierarchy: None,
                semantic_tokens: None,
                moniker: None,
                type_hierarchy: None,
                inline_value: None,
                inlay_hint: None,
                diagnostic: None,
            }),
            window: None,
            general: None,
            experimental: None,
        }
    }

    pub fn open_document(&mut self, file_path: &Path) -> Result<()> {
        let uri = Url::from_file_path(file_path).map_err(|_| anyhow!("Invalid file path"))?;

        let content = std::fs::read_to_string(file_path)?;

        // 拡張子から言語IDを推測
        let language_id = self.adapter.language_id();

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version: 1,
                text: content,
            },
        };

        self.send_notification("textDocument/didOpen", params)?;
        Ok(())
    }

    fn send_request<P: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R> {
        self.send_request_with_timeout(method, params, Duration::from_secs(30))
    }

    fn send_request_with_timeout<P: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: P,
        timeout: Duration,
    ) -> Result<R> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        self.write_message(&request)?;

        let start = Instant::now();

        // レスポンスを読む（タイムアウト付き）
        loop {
            // タイムアウトチェック
            if start.elapsed() > timeout {
                return Err(anyhow!(
                    "LSP request '{}' timed out after {:?}",
                    method,
                    timeout
                ));
            }

            // ノンブロッキング読み取りを試みる
            match self.try_read_message(Duration::from_millis(100)) {
                Ok(Some(response)) => {
                    // 通知の場合はスキップ
                    if response.get("id").is_none() {
                        continue;
                    }

                    // IDが一致するレスポンスを探す
                    if response.get("id") == Some(&serde_json::json!(id)) {
                        if let Some(error) = response.get("error") {
                            return Err(anyhow!("LSP error: {:?}", error));
                        }

                        if let Some(result) = response.get("result") {
                            return serde_json::from_value(result.clone())
                                .map_err(|e| anyhow!("Failed to parse response: {}", e));
                        }
                    }
                }
                Ok(None) => continue, // タイムアウトして読み取れなかった
                Err(e) => return Err(e),
            }
        }
    }

    fn send_notification<P: Serialize>(&mut self, method: &str, params: P) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        self.write_message(&notification)
    }

    fn write_message(&mut self, message: &Value) -> Result<()> {
        let content = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        self.stdin.write_all(header.as_bytes())?;
        self.stdin.write_all(content.as_bytes())?;
        self.stdin.flush()?;

        Ok(())
    }

    fn try_read_message(&mut self, timeout: Duration) -> Result<Option<Value>> {
        use std::io::ErrorKind;

        // タイムアウト付きでヘッダーを読む試行
        let start = Instant::now();
        let mut content_length = 0;

        loop {
            if start.elapsed() > timeout {
                return Ok(None);
            }

            let mut line = String::new();
            match self.stdout.read_line(&mut line) {
                Ok(0) => return Ok(None), // EOF
                Ok(_) => {
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                    if let Some(length_str) = line.strip_prefix("Content-Length: ") {
                        content_length = length_str
                            .trim()
                            .parse()
                            .map_err(|_| anyhow!("Invalid content length"))?;
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => return Err(anyhow!("Failed to read header: {}", e)),
            }
        }

        // コンテンツを読む
        let mut buffer = vec![0u8; content_length];
        self.stdout.read_exact(&mut buffer)?;

        let content = String::from_utf8(buffer)?;
        Ok(Some(
            serde_json::from_str(&content).map_err(|e| anyhow!("Failed to parse JSON: {}", e))?,
        ))
    }
}

// エイリアスメソッド（既存コードとの互換性のため）
impl LspClient {
    pub fn document_symbols(&mut self, uri: Url) -> Result<Vec<DocumentSymbol>> {
        let path = uri
            .to_file_path()
            .map_err(|_| anyhow!("Invalid file URI: {}", uri))?;
        self.get_document_symbols(&path)
    }

    pub fn goto_definition(&mut self, uri: Url, position: Position) -> Result<Option<Location>> {
        let path = uri
            .to_file_path()
            .map_err(|_| anyhow!("Invalid file URI: {}", uri))?;
        self.go_to_definition(&path, position)
    }

    pub fn get_definition(&mut self, path: &Path, position: Position) -> Result<Option<Location>> {
        self.go_to_definition(path, position)
    }

    pub fn hover(&mut self, uri: Url, position: Position) -> Result<Option<String>> {
        let path = uri
            .to_file_path()
            .map_err(|_| anyhow!("Invalid file URI: {}", uri))?;
        self.get_hover(&path, position)
    }

    pub fn call_hierarchy_prepare(
        &mut self,
        _uri: Url,
        _position: Position,
    ) -> Result<Vec<lsp_types::CallHierarchyItem>> {
        // 簡易実装（TODO: 実際のcall hierarchy実装が必要）
        Ok(vec![])
    }

    pub fn incoming_calls(
        &mut self,
        _item: lsp_types::CallHierarchyItem,
    ) -> Result<Vec<lsp_types::CallHierarchyIncomingCall>> {
        // 簡易実装
        Ok(vec![])
    }

    pub fn outgoing_calls(
        &mut self,
        _item: lsp_types::CallHierarchyItem,
    ) -> Result<Vec<lsp_types::CallHierarchyOutgoingCall>> {
        // 簡易実装
        Ok(vec![])
    }

    pub fn completion(
        &mut self,
        _uri: Url,
        _position: Position,
    ) -> Result<Vec<lsp_types::CompletionItem>> {
        // 簡易実装
        Ok(vec![])
    }

    pub fn references(&mut self, uri: Url, position: Position) -> Result<Vec<Location>> {
        let path = uri
            .to_file_path()
            .map_err(|_| anyhow!("Invalid file URI: {}", uri))?;
        self.find_references(&path, position)
    }

    pub fn get_references(&mut self, path: &Path, position: Position) -> Result<Vec<Location>> {
        self.find_references(path, position)
    }

    pub fn diagnostics(&mut self, _uri: Url) -> Result<Vec<lsp_types::Diagnostic>> {
        // 簡易実装
        Ok(vec![])
    }

    pub fn implementation(&mut self, _uri: Url, _position: Position) -> Result<Vec<Location>> {
        // 簡易実装
        Ok(vec![])
    }

    pub fn type_definition(&mut self, _uri: Url, _position: Position) -> Result<Vec<Location>> {
        // 簡易実装
        Ok(vec![])
    }

    pub fn rename(
        &mut self,
        _uri: Url,
        _position: Position,
        _new_name: String,
    ) -> Result<lsp_types::WorkspaceEdit> {
        // 簡易実装
        Ok(lsp_types::WorkspaceEdit::default())
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // クライアントが削除される時にプロセスを終了
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
