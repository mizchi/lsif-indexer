use anyhow::{Result, Context};
use lsp_types::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use tracing::{debug, info};

pub struct LspClient {
    process: Child,
    request_id: i32,
}

impl LspClient {
    pub fn spawn_rust_analyzer() -> Result<Self> {
        let process = Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn rust-analyzer")?;

        let mut client = Self {
            process,
            request_id: 0,
        };

        client.initialize()?;
        Ok(client)
    }

    fn next_request_id(&mut self) -> i32 {
        self.request_id += 1;
        self.request_id
    }

    fn send_request(&mut self, method: &str, params: Value) -> Result<Value> {
        let request_id = self.next_request_id();
        let request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        });

        self.send_message(&request)?;
        self.read_response(request_id)
    }

    fn send_message(&mut self, message: &Value) -> Result<()> {
        let content = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", content.len());
        
        let stdin = self.process.stdin.as_mut()
            .context("Failed to get stdin")?;
        
        stdin.write_all(header.as_bytes())?;
        stdin.write_all(content.as_bytes())?;
        stdin.flush()?;
        
        debug!("Sent: {}", content);
        Ok(())
    }

    fn read_response(&mut self, expected_id: i32) -> Result<Value> {
        let stdout = self.process.stdout.as_mut()
            .context("Failed to get stdout")?;
        let mut reader = BufReader::new(stdout);
        
        loop {
            // Read header
            let mut header = String::new();
            reader.read_line(&mut header)?;
            
            if !header.starts_with("Content-Length:") {
                continue;
            }
            
            let content_length: usize = header
                .trim_start_matches("Content-Length:")
                .trim()
                .parse()?;
            
            // Skip empty line
            let mut empty = String::new();
            reader.read_line(&mut empty)?;
            
            // Read content
            let mut content = vec![0; content_length];
            reader.read_exact(&mut content)?;
            
            let response: Value = serde_json::from_slice(&content)?;
            debug!("Received: {}", serde_json::to_string_pretty(&response)?);
            
            // Check if this is our response
            if let Some(id) = response.get("id").and_then(|v| v.as_i64()) {
                if id == expected_id as i64 {
                    if let Some(error) = response.get("error") {
                        anyhow::bail!("LSP error: {}", error);
                    }
                    return Ok(response.get("result").unwrap_or(&Value::Null).clone());
                }
            }
        }
    }

    fn initialize(&mut self) -> Result<()> {
        let params = json!({
            "processId": std::process::id(),
            "rootUri": format!("file://{}", std::env::current_dir()?.display()),
            "capabilities": {
                "textDocument": {
                    "documentSymbol": {
                        "hierarchicalDocumentSymbolSupport": true
                    }
                }
            },
            "initializationOptions": {}
        });

        let _response = self.send_request("initialize", params)?;
        
        // Send initialized notification
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        self.send_message(&notification)?;
        
        info!("LSP client initialized");
        Ok(())
    }

    pub fn get_document_symbols(&mut self, file_uri: &str) -> Result<Vec<DocumentSymbol>> {
        // First, open the document
        let open_params = json!({
            "textDocument": {
                "uri": file_uri,
                "languageId": "rust",
                "version": 1,
                "text": std::fs::read_to_string(
                    file_uri.trim_start_matches("file://")
                )?
            }
        });
        
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": open_params
        });
        self.send_message(&notification)?;
        
        // Request document symbols
        let params = json!({
            "textDocument": {
                "uri": file_uri
            }
        });
        
        let response = self.send_request("textDocument/documentSymbol", params)?;
        
        // Parse response
        let symbols: Vec<DocumentSymbol> = serde_json::from_value(response)?;
        Ok(symbols)
    }

    pub fn shutdown(mut self) -> Result<()> {
        let _response = self.send_request("shutdown", Value::Null)?;
        
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "exit"
        });
        self.send_message(&notification)?;
        
        self.process.wait()?;
        info!("LSP client shut down");
        Ok(())
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}