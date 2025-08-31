use super::adapter::lsp::{
    detect_language, GenericLspClient, RustAnalyzerAdapter, TypeScriptAdapter,
};
use anyhow::{Context, Result};
use std::path::Path;

/// LSPクライアントを作成するためのヘルパー関数群
pub struct LspClientHelpers;

impl LspClientHelpers {
    /// ファイルパスから適切なLSPクライアントを作成
    pub fn create_for_file(file_path: &Path) -> Result<GenericLspClient> {
        let adapter = detect_language(file_path.to_str().unwrap_or("")).ok_or_else(|| {
            anyhow::anyhow!("Failed to detect language for file: {:?}", file_path)
        })?;
        GenericLspClient::new(adapter).context("Failed to create LSP client")
    }

    /// Rust用のLSPクライアントを作成
    pub fn create_rust_client() -> Result<GenericLspClient> {
        GenericLspClient::new(Box::new(RustAnalyzerAdapter))
            .context("Failed to create Rust LSP client")
    }

    /// TypeScript/JavaScript用のLSPクライアントを作成
    pub fn create_typescript_client() -> Result<GenericLspClient> {
        GenericLspClient::new(Box::new(TypeScriptAdapter))
            .context("Failed to create TypeScript LSP client")
    }

    /// 言語名から適切なLSPクライアントを作成
    pub fn create_for_language(language: &str) -> Result<GenericLspClient> {
        match language {
            "rust" => Self::create_rust_client(),
            "typescript" | "ts" => Self::create_typescript_client(),
            "javascript" | "js" => Self::create_typescript_client(),
            _ => {
                // detect_languageで自動検出を試みる
                let dummy_path = format!("dummy.{}", language);
                let adapter = detect_language(&dummy_path)
                    .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language))?;
                GenericLspClient::new(adapter).context("Failed to create LSP client")
            }
        }
    }
}
