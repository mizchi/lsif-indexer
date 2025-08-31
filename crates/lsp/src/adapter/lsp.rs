use anyhow::{anyhow, Result};
use lsp_types::{
    DidOpenTextDocumentParams, DocumentSymbol, DocumentSymbolParams, GotoDefinitionParams,
    GotoDefinitionResponse, InitializeParams, InitializeResult, InitializedParams, Location,
    PartialResultParams, ReferenceParams, TextDocumentIdentifier, TextDocumentItem, Url,
    WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, Command, Stdio};

/// Trait for language-specific LSP configurations
pub trait LspAdapter {
    /// Get the command to spawn the language server
    fn spawn_command(&self) -> Result<Child>;

    /// Get the language ID for LSP
    fn language_id(&self) -> &str;

    /// Get initialization parameters specific to this language
    fn get_init_params(&self) -> InitializeParams {
        InitializeParams {
            capabilities: lsp_types::ClientCapabilities {
                text_document: Some(lsp_types::TextDocumentClientCapabilities {
                    document_symbol: Some(lsp_types::DocumentSymbolClientCapabilities {
                        dynamic_registration: Some(false),
                        symbol_kind: None,
                        hierarchical_document_symbol_support: Some(true),
                        tag_support: None,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
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
}

impl GenericLspClient {
    /// Create a new LSP client with the given adapter
    pub fn new(adapter: Box<dyn LspAdapter>) -> Result<Self> {
        let mut child = adapter.spawn_command()?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("No stdout"))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("No stdin"))?;

        let mut client = Self {
            child,
            reader: BufReader::new(stdout),
            writer: BufWriter::new(stdin),
            request_id: 0,
            language_id: adapter.language_id().to_string(),
        };

        // Initialize the LSP
        client.initialize(adapter.get_init_params())?;

        Ok(client)
    }

    fn initialize(&mut self, params: InitializeParams) -> Result<InitializeResult> {
        let response = self.send_request("initialize", params)?;

        // Send initialized notification
        self.send_notification("initialized", InitializedParams {})?;

        Ok(response)
    }

    pub fn get_document_symbols(&mut self, file_uri: &str) -> Result<Vec<DocumentSymbol>> {
        // First, open the document
        let content =
            std::fs::read_to_string(file_uri.strip_prefix("file://").unwrap_or(file_uri))?;

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

        // Request document symbols
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier {
                uri: Url::parse(file_uri)?,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let response: Option<lsp_types::DocumentSymbolResponse> =
            self.send_request("textDocument/documentSymbol", params)?;

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
                            detail: None,
                            kind: s.kind,
                            tags: s.tags,
                            deprecated: None,
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
        let response: Option<Vec<Location>> =
            self.send_request("textDocument/references", params)?;

        Ok(response.unwrap_or_default())
    }

    pub fn goto_definition(&mut self, params: GotoDefinitionParams) -> Result<Location> {
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

    pub fn send_request<P: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R> {
        self.request_id += 1;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: self.request_id,
            method: method.to_string(),
            params,
        };

        let request_str = serde_json::to_string(&request)?;
        let content_length = request_str.len();

        writeln!(self.writer, "Content-Length: {content_length}\r")?;
        writeln!(self.writer, "\r")?;
        self.writer.write_all(request_str.as_bytes())?;
        self.writer.flush()?;

        // Read response
        loop {
            let response = self.read_message()?;
            if let Some(response) = response {
                if response["id"] == self.request_id {
                    if let Some(error) = response.get("error") {
                        return Err(anyhow!("LSP error: {:?}", error));
                    }
                    if let Some(result) = response.get("result") {
                        return Ok(serde_json::from_value(result.clone())?);
                    }
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

    fn read_message(&mut self) -> Result<Option<serde_json::Value>> {
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line)?;

            if line == "\r\n" || line == "\n" {
                break;
            }
            headers.push(line);
        }

        let mut content_length = 0;
        for header in headers {
            if header.starts_with("Content-Length:") {
                content_length = header
                    .trim_start_matches("Content-Length:")
                    .trim()
                    .trim_end_matches('\r')
                    .parse()?;
            }
        }

        if content_length == 0 {
            return Ok(None);
        }

        let mut buffer = vec![0u8; content_length];
        use std::io::Read;
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
