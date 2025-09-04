use anyhow::{Context, Result};
use lsp_types::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, error, info, warn};

/// JSON-RPC Request
#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Value,
}

/// JSON-RPC Notification
#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcNotification {
    jsonrpc: String,
    method: String,
    params: Value,
}

/// JSON-RPC Response
#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC Error
#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

/// LSP RPC Client with full JSON-RPC implementation
pub struct LspRpcClient {
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    request_id: Arc<AtomicU64>,
    pending_requests: Arc<RwLock<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
    language_id: String,
    initialized: bool,
    capabilities: Option<ServerCapabilities>,
}

impl LspRpcClient {
    /// Create a new LSP RPC client
    pub fn new(command: &str, args: &[String], language_id: String) -> Result<Self> {
        info!("Starting LSP server: {} {:?}", command, args);

        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .context(format!("Failed to start LSP server: {}", command))?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to get stdin from LSP process")?;

        let stdout = child
            .stdout
            .take()
            .context("Failed to get stdout from LSP process")?;

        let client = Self {
            process: Some(child),
            stdin: Some(stdin),
            request_id: Arc::new(AtomicU64::new(1)),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            language_id,
            initialized: false,
            capabilities: None,
        };

        // Start the response reader in a background task
        let pending_requests = client.pending_requests.clone();
        tokio::spawn(async move {
            Self::read_responses(stdout, pending_requests).await;
        });

        Ok(client)
    }

    /// Initialize the LSP server
    pub async fn initialize(
        &mut self,
        root_uri: Url,
        initialization_options: Option<Value>,
    ) -> Result<InitializeResult> {
        if self.initialized {
            return Err(anyhow::anyhow!("LSP server already initialized"));
        }

        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            root_path: None,
            root_uri: None,
            initialization_options,
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri.clone(),
                name: "workspace".to_string(),
            }]),
            capabilities: ClientCapabilities {
                workspace: Some(WorkspaceClientCapabilities {
                    apply_edit: Some(false),
                    workspace_edit: None,
                    did_change_configuration: Some(DynamicRegistrationClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    did_change_watched_files: None,
                    symbol: Some(WorkspaceSymbolClientCapabilities {
                        dynamic_registration: Some(false),
                        symbol_kind: None,
                        tag_support: None,
                        resolve_support: None,
                    }),
                    execute_command: None,
                    workspace_folders: Some(false),
                    configuration: Some(false),
                    semantic_tokens: None,
                    code_lens: None,
                    file_operations: None,
                    inline_value: None,
                    inlay_hint: None,
                    diagnostic: None,
                }),
                text_document: Some(TextDocumentClientCapabilities {
                    synchronization: Some(TextDocumentSyncClientCapabilities {
                        dynamic_registration: Some(false),
                        will_save: Some(false),
                        will_save_wait_until: Some(false),
                        did_save: Some(false),
                    }),
                    completion: None,
                    hover: None,
                    signature_help: None,
                    references: None,
                    document_highlight: None,
                    document_symbol: Some(DocumentSymbolClientCapabilities {
                        dynamic_registration: Some(false),
                        symbol_kind: None,
                        hierarchical_document_symbol_support: Some(true),
                        tag_support: None,
                    }),
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
                    publish_diagnostics: None,
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
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(false),
                    show_message: None,
                    show_document: None,
                }),
                general: None,
                experimental: None,
            },
            trace: Some(TraceValue::Off),
            client_info: Some(ClientInfo {
                name: "lsif-indexer".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            locale: None,
        };

        let result = self
            .send_request::<InitializeResult>("initialize", params)
            .await?;

        // Send initialized notification
        self.send_notification("initialized", InitializedParams {})
            .await?;

        self.initialized = true;
        self.capabilities = Some(result.capabilities.clone());

        Ok(result)
    }

    /// Open a text document
    pub async fn did_open(&self, uri: Url, text: String) -> Result<()> {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: self.language_id.clone(),
                version: 0,
                text,
            },
        };

        self.send_notification("textDocument/didOpen", params).await
    }

    /// Close a text document
    pub async fn did_close(&self, uri: Url) -> Result<()> {
        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        };

        self.send_notification("textDocument/didClose", params)
            .await
    }

    /// Get document symbols
    pub async fn document_symbols(&self, uri: Url) -> Result<Vec<DocumentSymbol>> {
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let response = self
            .send_request::<Value>("textDocument/documentSymbol", params)
            .await?;

        // Try to parse as DocumentSymbol array first, then SymbolInformation array
        if let Ok(symbols) = serde_json::from_value::<Vec<DocumentSymbol>>(response.clone()) {
            Ok(symbols)
        } else if let Ok(symbol_infos) = serde_json::from_value::<Vec<SymbolInformation>>(response)
        {
            // Convert SymbolInformation to DocumentSymbol (simplified)
            Ok(symbol_infos
                .into_iter()
                .map(|info| {
                    #[allow(deprecated)]
                    DocumentSymbol {
                        name: info.name,
                        detail: None,
                        kind: info.kind,
                        tags: info.tags,
                        deprecated: None, // deprecated field
                        range: info.location.range,
                        selection_range: info.location.range,
                        children: None,
                    }
                })
                .collect())
        } else {
            Ok(vec![])
        }
    }

    /// Search workspace symbols
    pub async fn workspace_symbols(&self, query: &str) -> Result<Vec<SymbolInformation>> {
        let params = WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let response = self
            .send_request::<Value>("workspace/symbol", params)
            .await?;

        serde_json::from_value(response).context("Failed to parse workspace symbols response")
    }

    /// Shutdown the LSP server
    pub async fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        // Send shutdown request
        self.send_request::<Value>("shutdown", ()).await?;

        // Send exit notification
        self.send_notification("exit", ()).await?;

        // Wait for process to exit
        if let Some(mut process) = self.process.take() {
            let _ = process.wait();
        }

        self.initialized = false;

        Ok(())
    }

    /// Send a JSON-RPC request and wait for response
    async fn send_request<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<T> {
        if !self.initialized && method != "initialize" {
            return Err(anyhow::anyhow!("LSP server not initialized"));
        }

        let id = self.request_id.fetch_add(1, Ordering::SeqCst);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params: serde_json::to_value(params)?,
        };

        // Create a channel for the response
        let (tx, rx) = oneshot::channel();

        // Register the pending request
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(id, tx);
        }

        // Send the request
        self.write_message(&serde_json::to_value(request)?)?;

        // Wait for the response
        let response = rx
            .await
            .context("Failed to receive response from LSP server")?
            .context(format!("LSP request '{}' failed", method))?;

        serde_json::from_value(response)
            .context(format!("Failed to parse response for '{}'", method))
    }

    /// Send a JSON-RPC notification
    async fn send_notification(&self, method: &str, params: impl Serialize) -> Result<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: serde_json::to_value(params)?,
        };

        self.write_message(&serde_json::to_value(notification)?)
    }

    /// Write a message to the LSP server
    fn write_message(&self, message: &Value) -> Result<()> {
        let mut stdin = self
            .stdin
            .as_ref()
            .context("LSP process stdin not available")?;

        let content = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        stdin.write_all(header.as_bytes())?;
        stdin.write_all(content.as_bytes())?;
        stdin.flush()?;

        debug!("Sent LSP message: {}", method_from_message(message));

        Ok(())
    }

    /// Read responses from the LSP server
    async fn read_responses(
        stdout: ChildStdout,
        pending_requests: Arc<RwLock<HashMap<u64, oneshot::Sender<Result<Value>>>>>,
    ) {
        let mut reader = BufReader::new(stdout);
        let mut buffer = String::new();

        loop {
            buffer.clear();

            // Read headers
            let mut content_length = 0;
            loop {
                if reader.read_line(&mut buffer).unwrap_or(0) == 0 {
                    warn!("LSP server closed connection");
                    return;
                }

                let line = buffer.trim();
                if line.is_empty() {
                    break;
                }

                if let Some(length_str) = line.strip_prefix("Content-Length: ") {
                    content_length = length_str.parse().unwrap_or(0);
                }

                buffer.clear();
            }

            if content_length == 0 {
                continue;
            }

            // Read content
            let mut content = vec![0; content_length];
            if let Err(e) = reader.read_exact(&mut content) {
                error!("Failed to read LSP response content: {}", e);
                break;
            }

            // Parse JSON
            let message: Value = match serde_json::from_slice(&content) {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Failed to parse LSP response: {}", e);
                    continue;
                }
            };

            // Handle response
            if let Some(id) = message.get("id").and_then(|v| v.as_u64()) {
                let mut pending = pending_requests.write().await;
                if let Some(tx) = pending.remove(&id) {
                    if let Some(error) = message.get("error") {
                        let err_msg = error
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown error");
                        let _ = tx.send(Err(anyhow::anyhow!("LSP error: {}", err_msg)));
                    } else if let Some(result) = message.get("result") {
                        let _ = tx.send(Ok(result.clone()));
                    } else {
                        let _ = tx.send(Ok(Value::Null));
                    }
                }
            }

            // Handle notifications (e.g., diagnostics)
            if message.get("method").is_some() && message.get("id").is_none() {
                debug!(
                    "Received LSP notification: {}",
                    method_from_message(&message)
                );
            }
        }
    }
}

/// Extract method name from a JSON-RPC message
fn method_from_message(message: &Value) -> &str {
    message
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
}

impl Drop for LspRpcClient {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_increment() {
        let id = Arc::new(AtomicU64::new(1));
        assert_eq!(id.fetch_add(1, Ordering::SeqCst), 1);
        assert_eq!(id.fetch_add(1, Ordering::SeqCst), 2);
        assert_eq!(id.load(Ordering::SeqCst), 3);
    }
}
