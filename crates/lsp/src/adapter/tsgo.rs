use super::lsp::LspAdapter;
use anyhow::Result;
use std::process::{Child, Command, Stdio};

/// tsgo (TypeScript Native Preview) LSP adapter
pub struct TsgoAdapter;

impl LspAdapter for TsgoAdapter {
    fn spawn_command(&self) -> Result<Child> {
        Command::new("tsgo")
            .args(["--lsp", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn tsgo LSP: {}", e))
    }

    fn language_id(&self) -> &str {
        "typescript"
    }

    fn supports_workspace_symbol(&self) -> bool {
        true // tsgoはworkspace/symbolをサポート
    }
}

/// TypeScript-language-server adapter (フォールバック用)
pub struct TypeScriptLSAdapter;

impl LspAdapter for TypeScriptLSAdapter {
    fn spawn_command(&self) -> Result<Child> {
        Command::new("typescript-language-server")
            .args(["--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn typescript-language-server: {}", e))
    }

    fn language_id(&self) -> &str {
        "typescript"
    }

    fn supports_workspace_symbol(&self) -> bool {
        true
    }
}

/// JavaScript用のアダプタ（tsgoベース）
pub struct JavaScriptAdapter;

impl LspAdapter for JavaScriptAdapter {
    fn spawn_command(&self) -> Result<Child> {
        // tsgoはJavaScriptもサポート
        Command::new("tsgo")
            .args(["--lsp", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn tsgo for JavaScript: {}", e))
    }

    fn language_id(&self) -> &str {
        "javascript"
    }

    fn supports_workspace_symbol(&self) -> bool {
        true
    }
}
