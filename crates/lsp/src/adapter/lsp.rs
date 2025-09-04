use anyhow::{anyhow, Result};
use lsp_types::{
    ClientCapabilities, ClientInfo, DidOpenTextDocumentParams, DocumentSymbol, DocumentSymbolParams, 
    GotoDefinitionParams, GotoDefinitionResponse, InitializeParams, InitializeResult, 
    InitializedParams, Location, PartialResultParams, ReferenceParams, SymbolInformation,
    TextDocumentIdentifier, TextDocumentItem, Url, WorkDoneProgressParams, WorkspaceFolder,
};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use crate::timeout_predictor::TimeoutPredictor;
use crate::lsp_health_check::{LspHealthChecker, LspStartupValidator, LspOperationType};
use tracing::debug;

/// Trait for language-specific LSP configurations
pub trait LspAdapter {
    /// Get the command to spawn the language server
    fn spawn_command(&self) -> Result<Child>;

    /// Get the language ID for LSP
    fn language_id(&self) -> &str;
    
    /// Whether this LSP supports workspace/symbol
    fn supports_workspace_symbol(&self) -> bool {
        false  // デフォルトはfalse、各実装でオーバーライド可能
    }

    /// Get initialization parameters specific to this language
    fn get_init_params(&self) -> InitializeParams {
        #[allow(deprecated)]
        InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: None,  // 後で設定
            initialization_options: None,
            capabilities: lsp_types::ClientCapabilities {
                text_document: Some(lsp_types::TextDocumentClientCapabilities {
                    document_symbol: Some(lsp_types::DocumentSymbolClientCapabilities {
                        dynamic_registration: Some(false),
                        symbol_kind: None,
                        hierarchical_document_symbol_support: Some(true),
                        tag_support: None,
                    }),
                    definition: Some(lsp_types::GotoCapability {
                        dynamic_registration: Some(false),
                        link_support: Some(false),
                    }),
                    references: Some(lsp_types::ReferenceClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            trace: Some(lsp_types::TraceValue::Off),
            workspace_folders: None,
            client_info: Some(ClientInfo {
                name: "lsif-indexer".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            locale: None,
            root_path: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
        }
    }
}

/// Rust Analyzer adapter
pub struct RustAnalyzerAdapter;

impl LspAdapter for RustAnalyzerAdapter {
    fn spawn_command(&self) -> Result<Child> {
        Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn rust-analyzer: {}", e))
    }

    fn language_id(&self) -> &str {
        "rust"
    }
}

/// TypeScript language server adapter (using tsgo)
pub struct TypeScriptAdapter;

impl LspAdapter for TypeScriptAdapter {
    fn spawn_command(&self) -> Result<Child> {
        // Use tsgo (@typescript/native-preview)
        Command::new("npx")
            .arg("-y")
            .arg("@typescript/native-preview")
            .arg("--lsp")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn tsgo: {}", e))
    }

    fn language_id(&self) -> &str {
        "typescript"
    }
}

/// Generic LSP client that works with any adapter
pub struct GenericLspClient {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    writer: BufWriter<std::process::ChildStdin>,
    request_id: i64,
    language_id: String,
    timeout_predictor: TimeoutPredictor,
    health_checker: LspHealthChecker,
    /// LSPサーバーのCapabilities
    server_capabilities: Option<lsp_types::ServerCapabilities>,
}

impl GenericLspClient {
    /// Create a new LSP client with the given adapter (without initialization)
    pub fn new_uninit(adapter: Box<dyn LspAdapter>) -> Result<Self> {
        use tracing::info;
        
        let language_id = adapter.language_id().to_string();
        info!("Creating LSP client for language: {}", language_id);
        
        // LSPプロセスを起動
        let mut child = adapter.spawn_command()?;
        
        // 起動確認
        let validator = LspStartupValidator::new();
        validator.validate_startup(&mut child, &language_id)?;
        
        // 言語別の起動待機
        validator.wait_for_startup(&language_id);
        
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("No stdout"))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("No stdin"))?;

        let mut client = Self {
            child,
            reader: BufReader::new(stdout),
            writer: BufWriter::new(stdin),
            request_id: 0,
            language_id: language_id.clone(),
            timeout_predictor: TimeoutPredictor::new(),
            health_checker: LspHealthChecker::new(),
            server_capabilities: None,
        };
        
        // 初期化前にプロセスが生きているか再確認
        LspHealthChecker::check_process_alive(&mut client.child)?;
        
        Ok(client)
    }
    
    /// Create a new LSP client with the given adapter (with initialization)
    pub fn new(adapter: Box<dyn LspAdapter>) -> Result<Self> {
        let mut client = Self::new_uninit(adapter)?;
        
        // デフォルトの初期化パラメータで初期化
        // 注意: root_uriがNoneのため、一部のLSPサーバーでは動作しない可能性がある
        let init_timeout = client.health_checker.calculate_init_timeout();
        match client.initialize_with_params(Default::default(), Some(init_timeout)) {
            Ok(_) => Ok(client),
            Err(e) => {
                let _ = client.child.kill();
                Err(e)
            }
        }
    }

    fn initialize_with_params(&mut self, params: InitializeParams, timeout: Option<Duration>) -> Result<InitializeResult> {
        use tracing::{debug, info};
        use std::time::Instant;
        
        debug!("Sending initialize request for {}", self.language_id);
        debug!("Root URI: {:?}", params.root_uri);
        
        // 初期化用のタイムアウトを取得
        let timeout = timeout.unwrap_or_else(|| {
            self.health_checker.get_timeout_for_operation(LspOperationType::Initialize)
        });
        
        let start = Instant::now();
        let response: InitializeResult = self.send_request_with_timeout("initialize", params, timeout)?;
        let duration = start.elapsed();
        
        // 初期化時間を記録
        self.health_checker.record_init_time(duration);
        
        info!("Received initialize response from {} in {:?}", self.language_id, duration);
        debug!("Server capabilities: {:?}", response.capabilities);
        
        // サーバーのCapabilitiesを保存
        self.server_capabilities = Some(response.capabilities.clone());
        
        // 言語固有の最適化を適用
        self.optimize_for_language();
        
        // initialized通知を送信する前に少し待つ（大幅に削減）
        std::thread::sleep(Duration::from_millis(10));  // 100ms -> 10ms
        
        debug!("Sending initialized notification for {}", self.language_id);
        self.send_notification("initialized", InitializedParams {})?;
        
        info!("Successfully completed initialization for {}", self.language_id);
        Ok(response)
    }

    pub fn initialize(&mut self, project_root: &Path, timeout: Option<Duration>) -> Result<()> {
        use std::fs::canonicalize;
        
        let root_uri = canonicalize(project_root)
            .and_then(|p| Url::from_file_path(p).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid path")))?;
        
        // 言語固有のクライアントCapabilitiesを構築
        let client_capabilities = self.build_client_capabilities();
        
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: None,
            capabilities: client_capabilities,
            initialization_options: None,
            client_info: Some(ClientInfo {
                name: "lsif-indexer".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            locale: None,
            root_path: None,
            trace: None,
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri,
                name: "workspace".to_string(),
            }]),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        
        self.initialize_with_params(params, timeout)?;
        Ok(())
    }
    
    /// 言語固有のクライアントCapabilitiesを構築
    fn build_client_capabilities(&self) -> ClientCapabilities {
        use lsp_types::*;
        
        let mut capabilities = ClientCapabilities::default();
        
        // テキストドキュメント関連のCapabilities
        let mut text_document = TextDocumentClientCapabilities::default();
        
        // DocumentSymbol
        text_document.document_symbol = Some(DocumentSymbolClientCapabilities {
            dynamic_registration: Some(false),
            hierarchical_document_symbol_support: Some(true),
            symbol_kind: Some(SymbolKindCapability {
                value_set: Some(vec![
                    SymbolKind::FILE,
                    SymbolKind::MODULE,
                    SymbolKind::NAMESPACE,
                    SymbolKind::PACKAGE,
                    SymbolKind::CLASS,
                    SymbolKind::METHOD,
                    SymbolKind::PROPERTY,
                    SymbolKind::FIELD,
                    SymbolKind::CONSTRUCTOR,
                    SymbolKind::ENUM,
                    SymbolKind::INTERFACE,
                    SymbolKind::FUNCTION,
                    SymbolKind::VARIABLE,
                    SymbolKind::CONSTANT,
                    SymbolKind::STRING,
                    SymbolKind::NUMBER,
                    SymbolKind::BOOLEAN,
                    SymbolKind::ARRAY,
                    SymbolKind::OBJECT,
                    SymbolKind::KEY,
                    SymbolKind::NULL,
                    SymbolKind::ENUM_MEMBER,
                    SymbolKind::STRUCT,
                    SymbolKind::EVENT,
                    SymbolKind::OPERATOR,
                    SymbolKind::TYPE_PARAMETER,
                ]),
            }),
            tag_support: None,
        });
        
        // Definition
        text_document.definition = Some(GotoCapability {
            dynamic_registration: Some(false),
            link_support: Some(true),
        });
        
        // References
        text_document.references = Some(ReferenceClientCapabilities {
            dynamic_registration: Some(false),
        });
        
        // Type Definition
        text_document.type_definition = Some(GotoCapability {
            dynamic_registration: Some(false),
            link_support: Some(true),
        });
        
        // Implementation
        text_document.implementation = Some(GotoCapability {
            dynamic_registration: Some(false),
            link_support: Some(true),
        });
        
        // Call Hierarchy
        text_document.call_hierarchy = Some(CallHierarchyClientCapabilities {
            dynamic_registration: Some(false),
        });
        
        capabilities.text_document = Some(text_document);
        
        // Workspace関連のCapabilities
        let mut workspace = WorkspaceClientCapabilities::default();
        workspace.symbol = Some(WorkspaceSymbolClientCapabilities {
            dynamic_registration: Some(false),
            symbol_kind: Some(SymbolKindCapability {
                value_set: Some(vec![
                    SymbolKind::FILE,
                    SymbolKind::MODULE,
                    SymbolKind::NAMESPACE,
                    SymbolKind::PACKAGE,
                    SymbolKind::CLASS,
                    SymbolKind::METHOD,
                    SymbolKind::PROPERTY,
                    SymbolKind::FIELD,
                    SymbolKind::CONSTRUCTOR,
                    SymbolKind::ENUM,
                    SymbolKind::INTERFACE,
                    SymbolKind::FUNCTION,
                    SymbolKind::VARIABLE,
                    SymbolKind::CONSTANT,
                    SymbolKind::STRING,
                    SymbolKind::NUMBER,
                    SymbolKind::BOOLEAN,
                    SymbolKind::ARRAY,
                    SymbolKind::OBJECT,
                    SymbolKind::KEY,
                    SymbolKind::NULL,
                    SymbolKind::ENUM_MEMBER,
                    SymbolKind::STRUCT,
                    SymbolKind::EVENT,
                    SymbolKind::OPERATOR,
                    SymbolKind::TYPE_PARAMETER,
                ]),
            }),
            tag_support: None,
            resolve_support: None,
        });
        
        capabilities.workspace = Some(workspace);
        
        capabilities
    }

    /// サーバーがサポートする機能をチェック
    pub fn has_capability(&self, capability: &str) -> bool {
        match &self.server_capabilities {
            Some(caps) => match capability {
                "textDocument/documentSymbol" => caps.document_symbol_provider.is_some(),
                "textDocument/definition" => caps.definition_provider.is_some(),
                "textDocument/references" => caps.references_provider.is_some(),
                "textDocument/typeDefinition" => caps.type_definition_provider.is_some(),
                "textDocument/implementation" => caps.implementation_provider.is_some(),
                "workspace/symbol" => caps.workspace_symbol_provider.is_some(),
                "textDocument/prepareCallHierarchy" => caps.call_hierarchy_provider.is_some(),
                "textDocument/hover" => caps.hover_provider.is_some(),
                "textDocument/completion" => caps.completion_provider.is_some(),
                "textDocument/signatureHelp" => caps.signature_help_provider.is_some(),
                "textDocument/codeAction" => caps.code_action_provider.is_some(),
                "textDocument/codeLens" => caps.code_lens_provider.is_some(),
                "textDocument/documentHighlight" => caps.document_highlight_provider.is_some(),
                "textDocument/documentLink" => caps.document_link_provider.is_some(),
                "textDocument/formatting" => caps.document_formatting_provider.is_some(),
                "textDocument/rangeFormatting" => caps.document_range_formatting_provider.is_some(),
                "textDocument/onTypeFormatting" => caps.document_on_type_formatting_provider.is_some(),
                "textDocument/rename" => caps.rename_provider.is_some(),
                "textDocument/foldingRange" => caps.folding_range_provider.is_some(),
                "textDocument/selectionRange" => caps.selection_range_provider.is_some(),
                "textDocument/semanticTokens" => {
                    caps.semantic_tokens_provider.is_some()
                }
                "textDocument/linkedEditingRange" => caps.linked_editing_range_provider.is_some(),
                "textDocument/moniker" => caps.moniker_provider.is_some(),
                "textDocument/inlayHint" => caps.inlay_hint_provider.is_some(),
                "textDocument/inlineValue" => caps.inline_value_provider.is_some(),
                "textDocument/diagnostic" => caps.diagnostic_provider.is_some(),
                _ => false,
            },
            None => false,
        }
    }
    
    /// サーバーがサポートするシンボルの種類を取得
    pub fn get_supported_symbol_kinds(&self) -> Vec<lsp_types::SymbolKind> {
        // デフォルトで全てのシンボル種類をサポート
        vec![
            lsp_types::SymbolKind::FILE,
            lsp_types::SymbolKind::MODULE,
            lsp_types::SymbolKind::NAMESPACE,
            lsp_types::SymbolKind::PACKAGE,
            lsp_types::SymbolKind::CLASS,
            lsp_types::SymbolKind::METHOD,
            lsp_types::SymbolKind::PROPERTY,
            lsp_types::SymbolKind::FIELD,
            lsp_types::SymbolKind::CONSTRUCTOR,
            lsp_types::SymbolKind::ENUM,
            lsp_types::SymbolKind::INTERFACE,
            lsp_types::SymbolKind::FUNCTION,
            lsp_types::SymbolKind::VARIABLE,
            lsp_types::SymbolKind::CONSTANT,
            lsp_types::SymbolKind::STRING,
            lsp_types::SymbolKind::NUMBER,
            lsp_types::SymbolKind::BOOLEAN,
            lsp_types::SymbolKind::ARRAY,
            lsp_types::SymbolKind::OBJECT,
            lsp_types::SymbolKind::KEY,
            lsp_types::SymbolKind::NULL,
            lsp_types::SymbolKind::ENUM_MEMBER,
            lsp_types::SymbolKind::STRUCT,
            lsp_types::SymbolKind::EVENT,
            lsp_types::SymbolKind::OPERATOR,
            lsp_types::SymbolKind::TYPE_PARAMETER,
        ]
    }
    
    /// 言語IDを取得
    pub fn get_language_id(&self) -> &str {
        &self.language_id
    }
    
    /// サーバーCapabilitiesを取得
    pub fn get_server_capabilities(&self) -> Option<&lsp_types::ServerCapabilities> {
        self.server_capabilities.as_ref()
    }
    
    /// 言語固有の最適化を適用（Capabilitiesに基づく）
    pub fn optimize_for_language(&mut self) {
        use tracing::info;
        
        match self.language_id.as_str() {
            "rust" => {
                // Rustの場合、rust-analyzerの特性に合わせて最適化
                if self.has_capability("textDocument/semanticTokens") {
                    info!("Rust: Semantic tokens are supported, using for better symbol extraction");
                }
                if self.has_capability("textDocument/inlayHint") {
                    info!("Rust: Inlay hints are supported, can extract type information");
                }
            }
            "typescript" | "javascript" => {
                // TypeScript/JavaScriptの場合
                if self.has_capability("textDocument/completion") {
                    info!("TypeScript: Completion is supported, can extract more detailed type info");
                }
            }
            "python" => {
                // Pythonの場合
                if self.has_capability("textDocument/hover") {
                    info!("Python: Hover is supported, can extract docstrings");
                }
            }
            "go" => {
                // Goの場合
                if self.has_capability("textDocument/implementation") {
                    info!("Go: Implementation is supported, can track interface implementations");
                }
            }
            _ => {
                info!("Using default LSP capabilities for language: {}", self.language_id);
            }
        }
    }

    pub fn get_document_symbols(&mut self, file_uri: &str) -> Result<Vec<DocumentSymbol>> {
        use std::time::Instant;
        
        // Capabilityをチェック
        if !self.has_capability("textDocument/documentSymbol") {
            return Err(anyhow!(
                "LSP server for {} does not support textDocument/documentSymbol",
                self.language_id
            ));
        }
        
        let start = Instant::now();
        
        // First, open the document
        let file_path = file_uri.strip_prefix("file://").unwrap_or(file_uri);
        let content = std::fs::read_to_string(file_path)?;
        
        // ファイルサイズと行数を取得
        let file_size = content.len();
        let line_count = content.lines().count();
        
        // 操作種別に応じたタイムアウトを取得
        let timeout = self.health_checker.get_timeout_for_operation(LspOperationType::DocumentSymbol);
        debug!("Processing {} ({}KB, {} lines) with timeout: {:?}", 
                  file_path, file_size / 1024, line_count, timeout);  // eprintln -> debug

        self.send_notification(
            "textDocument/didOpen",
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: Url::parse(file_uri)?,
                    language_id: self.language_id.clone(),
                    version: 0,
                    text: content,
                },
            },
        )?;

        // Request document symbols with adaptive timeout
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier {
                uri: Url::parse(file_uri)?,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let response: Option<lsp_types::DocumentSymbolResponse> =
            self.send_request_with_timeout("textDocument/documentSymbol", params, timeout)?;
        
        // 処理時間を記録
        let actual_duration = start.elapsed();
        self.health_checker.record_response_time_for_operation(actual_duration, LspOperationType::DocumentSymbol);
        self.timeout_predictor.record_processing(file_size, line_count, actual_duration);
        
        debug!("DocumentSymbol completed in {:?} (phase: {})", 
               actual_duration, 
               self.health_checker.get_health_status().current_phase);

        match response {
            Some(lsp_types::DocumentSymbolResponse::Nested(symbols)) => Ok(symbols),
            Some(lsp_types::DocumentSymbolResponse::Flat(symbols)) => {
                // Convert flat symbols to nested format
                Ok(symbols
                    .into_iter()
                    .map(|s| {
                        #[allow(deprecated)]
                        DocumentSymbol {
                            name: s.name,
                            kind: s.kind,
                            tags: s.tags,
                            deprecated: None,
                            detail: s.container_name,
                            range: s.location.range,
                            selection_range: s.location.range,
                            children: None,
                        }
                    })
                    .collect())
            }
            None => Ok(Vec::new()),
        }
    }

    pub fn find_references(&mut self, params: ReferenceParams) -> Result<Vec<Location>> {
        // Capabilityをチェック
        if !self.has_capability("textDocument/references") {
            return Err(anyhow!(
                "LSP server for {} does not support textDocument/references",
                self.language_id
            ));
        }
        
        let response: Option<Vec<Location>> =
            self.send_request("textDocument/references", params)?;

        Ok(response.unwrap_or_default())
    }

    pub fn goto_definition(&mut self, params: GotoDefinitionParams) -> Result<Location> {
        // Capabilityをチェック
        if !self.has_capability("textDocument/definition") {
            return Err(anyhow!(
                "LSP server for {} does not support textDocument/definition",
                self.language_id
            ));
        }
        
        let response: Option<GotoDefinitionResponse> =
            self.send_request("textDocument/definition", params)?;

        match response {
            Some(GotoDefinitionResponse::Scalar(location)) => Ok(location),
            Some(GotoDefinitionResponse::Array(locations)) => locations
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("No definition found")),
            Some(GotoDefinitionResponse::Link(links)) => links
                .into_iter()
                .next()
                .map(|link| Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                })
                .ok_or_else(|| anyhow!("No definition found")),
            None => Err(anyhow!("No definition found")),
        }
    }

    /// ワークスペースシンボルを検索
    pub fn search_workspace_symbols(&mut self, query: &str) -> Result<Vec<SymbolInformation>> {
        use lsp_types::{WorkspaceSymbolParams, WorkDoneProgressParams, PartialResultParams};
        
        let params = WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        self.send_request::<_, Option<Vec<SymbolInformation>>>("workspace/symbol", params)?
            .ok_or_else(|| anyhow!("No workspace symbols found"))
    }

    pub fn send_request<P: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R> {
        // デフォルトタイムアウト（30秒）で送信
        self.send_request_with_timeout(method, params, std::time::Duration::from_secs(30))
    }
    
    pub fn send_request_with_timeout<P: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: P,
        timeout: std::time::Duration,
    ) -> Result<R> {
        use std::time::Instant;
        use tracing::debug;
        
        self.request_id += 1;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: self.request_id,
            method: method.to_string(),
            params,
        };

        let request_str = serde_json::to_string(&request)?;
        let content_length = request_str.len();

        debug!("Sending LSP request '{}' (id: {})", method, self.request_id);
        
        writeln!(self.writer, "Content-Length: {content_length}\r")?;
        writeln!(self.writer, "\r")?;
        self.writer.write_all(request_str.as_bytes())?;
        self.writer.flush()?;

        // Read response with timeout
        let start = Instant::now();
        loop {
            let elapsed = start.elapsed();
            if elapsed > timeout {
                return Err(anyhow!("LSP request '{}' timed out after {:?}", method, timeout));
            }
            
            // ノンブロッキング読み取りを試みる（100ms -> 10ms）
            match self.try_read_message(std::time::Duration::from_millis(10)) {
                Ok(Some(response)) => {
                    if response["id"] == self.request_id {
                        // レスポンス時間を記録
                        let response_time = start.elapsed();
                        self.health_checker.record_response_time(response_time);
                        debug!("LSP request '{}' completed in {:?}", method, response_time);
                        
                        if let Some(error) = response.get("error") {
                            return Err(anyhow!("LSP error: {:?}", error));
                        }
                        if let Some(result) = response.get("result") {
                            return Ok(serde_json::from_value(result.clone())?);
                        }
                    }
                }
                Ok(None) => {
                    // タイムアウトまで待機（10ms -> 1ms）
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(e) => {
                    return Err(anyhow!("Failed to read response: {}", e));
                }
            }
        }
    }

    pub fn send_notification<P: Serialize>(&mut self, method: &str, params: P) -> Result<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        let notification_str = serde_json::to_string(&notification)?;
        let content_length = notification_str.len();

        writeln!(self.writer, "Content-Length: {content_length}\r")?;
        writeln!(self.writer, "\r")?;
        self.writer.write_all(notification_str.as_bytes())?;
        self.writer.flush()?;

        Ok(())
    }

    
    fn try_read_message(&mut self, timeout: std::time::Duration) -> Result<Option<serde_json::Value>> {
        use std::io::{ErrorKind, Read};
        use std::time::Instant;
        
        let start = Instant::now();
        let mut headers = Vec::new();
        let mut content_length = 0;
        
        // ヘッダーを読む（タイムアウト付き）
        loop {
            if start.elapsed() > timeout {
                return Ok(None);
            }
            
            // タイムアウトチェックを追加
            if start.elapsed() > timeout {
                return Ok(None);
            }
            
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => return Ok(None), // EOF
                Ok(_) => {
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                    if line.starts_with("Content-Length:") {
                        content_length = line
                            .trim_start_matches("Content-Length:")
                            .trim()
                            .trim_end_matches('\r')
                            .parse()?;
                    }
                    headers.push(line);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                Err(e) => return Err(anyhow!("Failed to read header: {}", e)),
            }
        }

        if content_length == 0 {
            return Ok(None);
        }

        // コンテンツを読む（タイムアウト付き）
        if start.elapsed() > timeout {
            return Ok(None);
        }
        
        let mut buffer = vec![0u8; content_length];
        self.reader.read_exact(&mut buffer)?;

        let response: serde_json::Value = serde_json::from_slice(&buffer)?;
        Ok(Some(response))
    }

    pub fn shutdown(mut self) -> Result<()> {
        let _: () = self.send_request("shutdown", serde_json::Value::Null)?;
        self.send_notification("exit", serde_json::Value::Null)?;
        self.child.wait()?;
        Ok(())
    }
}

#[derive(Serialize)]
struct JsonRpcRequest<P> {
    jsonrpc: String,
    id: i64,
    method: String,
    params: P,
}

#[derive(Serialize)]
struct JsonRpcNotification<P> {
    jsonrpc: String,
    method: String,
    params: P,
}

/// Detect language from file extension
pub fn detect_language(file_path: &str) -> Option<Box<dyn LspAdapter>> {
    let extension = std::path::Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())?;

    match extension {
        "rs" => Some(Box::new(RustAnalyzerAdapter)),
        "ts" | "tsx" | "js" | "jsx" => Some(Box::new(TypeScriptAdapter)),
        _ => None,
    }
}

/// Get language ID from file path
pub fn get_language_id(file_path: &Path) -> Option<String> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())?;

    match extension {
        "rs" => Some("rust".to_string()),
        "ts" | "tsx" => Some("typescript".to_string()),
        "js" | "jsx" => Some("javascript".to_string()),
        "py" => Some("python".to_string()),
        "go" => Some("go".to_string()),
        "java" => Some("java".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert!(detect_language("main.rs").is_some());
        assert!(detect_language("index.ts").is_some());
        assert!(detect_language("app.tsx").is_some());
        assert!(detect_language("script.js").is_some());
        assert!(detect_language("component.jsx").is_some());
        assert!(detect_language("unknown.xyz").is_none());
    }

    #[test]
    fn test_rust_analyzer_adapter() {
        let adapter = RustAnalyzerAdapter;
        assert_eq!(adapter.language_id(), "rust");
        
        // 初期化パラメータを取得できることを確認
        let init_params = adapter.get_init_params();
        assert!(init_params.capabilities.text_document.is_some());
        let text_doc = init_params.capabilities.text_document.unwrap();
        assert!(text_doc.document_symbol.is_some());
        let doc_symbol = text_doc.document_symbol.unwrap();
        assert_eq!(doc_symbol.hierarchical_document_symbol_support, Some(true));
    }

    #[test]
    fn test_typescript_adapter() {
        let adapter = TypeScriptAdapter;
        assert_eq!(adapter.language_id(), "typescript");
        
        // 初期化パラメータを取得できることを確認
        let init_params = adapter.get_init_params();
        assert!(init_params.capabilities.text_document.is_some());
    }

    #[test]
    fn test_detect_language_with_paths() {
        // 絶対パス
        assert!(detect_language("/home/user/project/main.rs").is_some());
        assert!(detect_language("/src/index.ts").is_some());
        
        // 相対パス
        assert!(detect_language("./src/main.rs").is_some());
        assert!(detect_language("../lib/index.ts").is_some());
        
        // 複雑なパス
        assert!(detect_language("some/deep/path/to/file.rs").is_some());
        assert!(detect_language("path with spaces/file.ts").is_some());
    }

    #[test]
    fn test_detect_language_returns_correct_adapter() {
        // Rustファイルに対してRustAnalyzerAdapterを返すことを確認
        if let Some(adapter) = detect_language("test.rs") {
            // Box<dyn LspAdapter>として返されるため、language_idで判定
            assert_eq!(adapter.language_id(), "rust");
        } else {
            panic!("Expected RustAnalyzer adapter for .rs file");
        }

        // TypeScript/JavaScriptファイルに対してTypeScriptAdapterを返すことを確認
        for ext in &["ts", "tsx", "js", "jsx"] {
            let filename = format!("test.{}", ext);
            if let Some(adapter) = detect_language(&filename) {
                assert_eq!(adapter.language_id(), "typescript");
            } else {
                panic!("Expected TypeScript adapter for .{} file", ext);
            }
        }
    }
}
