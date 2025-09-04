use crate::adapter::lsp::LspAdapter;
/// テスト用のユーティリティとモックLSPアダプタ
use anyhow::Result;
use lsp_types::{
    DocumentSymbol, Location, Position, Range, ServerCapabilities, SymbolInformation, SymbolKind,
    Url,
};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

/// モックLSPアダプタ（テスト用）
pub struct MockLspAdapter {
    pub supports_workspace: bool,
    pub supports_document: bool,
    pub workspace_symbols: Arc<Mutex<Vec<SymbolInformation>>>,
    pub document_symbols: Arc<Mutex<Vec<DocumentSymbol>>>,
}

impl MockLspAdapter {
    pub fn new() -> Self {
        Self {
            supports_workspace: true,
            supports_document: true,
            workspace_symbols: Arc::new(Mutex::new(Vec::new())),
            document_symbols: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_workspace_support(mut self, supported: bool) -> Self {
        self.supports_workspace = supported;
        self
    }

    pub fn with_document_support(mut self, supported: bool) -> Self {
        self.supports_document = supported;
        self
    }

    pub fn add_workspace_symbol(&self, symbol: SymbolInformation) {
        self.workspace_symbols.lock().unwrap().push(symbol);
    }

    pub fn add_document_symbol(&self, symbol: DocumentSymbol) {
        self.document_symbols.lock().unwrap().push(symbol);
    }
}

impl LspAdapter for MockLspAdapter {
    fn spawn_command(&self) -> Result<Child> {
        // テスト用のダミープロセスを起動（実際にはechoコマンドなど）
        Command::new("echo")
            .arg("mock lsp server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn mock process: {}", e))
    }

    fn language_id(&self) -> &str {
        "mock"
    }

    fn supports_workspace_symbol(&self) -> bool {
        self.supports_workspace
    }
}

/// テスト用のシンボル生成ヘルパー
pub fn create_test_workspace_symbol(
    name: &str,
    kind: SymbolKind,
    file_path: &str,
    line: u32,
) -> SymbolInformation {
    SymbolInformation {
        name: name.to_string(),
        kind,
        tags: None,
        deprecated: None,
        location: Location {
            uri: Url::parse(&format!("file://{}", file_path)).unwrap(),
            range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: 10,
                },
            },
        },
        container_name: None,
    }
}

pub fn create_test_document_symbol(
    name: &str,
    kind: SymbolKind,
    start_line: u32,
    end_line: u32,
) -> DocumentSymbol {
    DocumentSymbol {
        name: name.to_string(),
        detail: None,
        kind,
        tags: None,
        deprecated: None,
        range: Range {
            start: Position {
                line: start_line,
                character: 0,
            },
            end: Position {
                line: end_line,
                character: 0,
            },
        },
        selection_range: Range {
            start: Position {
                line: start_line,
                character: 0,
            },
            end: Position {
                line: start_line,
                character: name.len() as u32,
            },
        },
        children: None,
    }
}

/// テスト用のサーバー機能を生成
pub fn create_test_capabilities(
    workspace_symbol: bool,
    document_symbol: bool,
) -> ServerCapabilities {
    ServerCapabilities {
        workspace_symbol_provider: if workspace_symbol {
            Some(lsp_types::OneOf::Left(true))
        } else {
            None
        },
        document_symbol_provider: if document_symbol {
            Some(lsp_types::OneOf::Left(true))
        } else {
            None
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_adapter_creation() {
        let adapter = MockLspAdapter::new()
            .with_workspace_support(true)
            .with_document_support(false);

        assert!(adapter.supports_workspace_symbol());
        assert_eq!(adapter.language_id(), "mock");
    }

    #[test]
    fn test_workspace_symbol_creation() {
        let symbol =
            create_test_workspace_symbol("TestFunction", SymbolKind::FUNCTION, "/test/file.rs", 10);

        assert_eq!(symbol.name, "TestFunction");
        assert_eq!(symbol.kind, SymbolKind::FUNCTION);
        assert_eq!(symbol.location.range.start.line, 10);
    }

    #[test]
    fn test_document_symbol_creation() {
        let symbol = create_test_document_symbol("TestClass", SymbolKind::CLASS, 5, 15);

        assert_eq!(symbol.name, "TestClass");
        assert_eq!(symbol.kind, SymbolKind::CLASS);
        assert_eq!(symbol.range.start.line, 5);
        assert_eq!(symbol.range.end.line, 15);
    }

    #[test]
    fn test_capabilities_creation() {
        let caps = create_test_capabilities(true, false);

        assert!(caps.workspace_symbol_provider.is_some());
        assert!(caps.document_symbol_provider.is_none());
    }
}
