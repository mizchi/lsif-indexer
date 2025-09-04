#[cfg(test)]
mod tests {
    use lsp::adapter::lsp::*;

    use lsp::lsp_features::*;
    use lsp_types::*;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    #[ignore] // 実際のrust-analyzerが必要
    fn test_rust_analyzer_integration() {
        let adapter = Box::new(RustAnalyzerAdapter);
        let client = LspClient::new(adapter);

        assert!(client.is_ok());
    }

    #[test]
    #[ignore] // 実際のTypeScript LSPが必要
    fn test_typescript_lsp_integration() {
        let adapter = Box::new(TypeScriptAdapter);
        let client = LspClient::new(adapter);

        assert!(client.is_ok());
    }

    #[test]
    fn test_language_detection() {
        assert!(detect_language("main.rs").is_some());
        assert!(detect_language("index.ts").is_some());
        assert!(detect_language("app.tsx").is_some());
        assert!(detect_language("script.js").is_some());
        assert!(detect_language("component.jsx").is_some());
        assert!(detect_language("unknown.xyz").is_none());
    }

    #[test]
    #[ignore] // 実際のLSPサーバーが必要
    fn test_hover_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");

        fs::write(
            &test_file,
            r#"
fn main() {
    let x = 42;
    println!("{}", x);
}
"#,
        )
        .unwrap();

        let adapter = Box::new(RustAnalyzerAdapter);
        let client = LspClient::new(adapter).unwrap();

        let uri = Url::from_file_path(&test_file).unwrap();
        let hover_result = client.hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 2,
                    character: 8,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        });

        assert!(hover_result.is_ok());
    }

    #[test]
    #[ignore] // 実際のLSPサーバーが必要
    fn test_completion_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");

        fs::write(
            &test_file,
            r#"
fn main() {
    let x = String::new();
    x.
}
"#,
        )
        .unwrap();

        let adapter = Box::new(RustAnalyzerAdapter);
        let client = LspClient::new(adapter).unwrap();

        let uri = Url::from_file_path(&test_file).unwrap();
        let completion_result = client.completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 3,
                    character: 6,
                },
            },
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        });

        assert!(completion_result.is_ok());
        if let Ok(Some(response)) = completion_result {
            match response {
                CompletionResponse::Array(items) => {
                    assert!(!items.is_empty());
                }
                CompletionResponse::List(list) => {
                    assert!(!list.items.is_empty());
                }
            }
        }
    }

    #[test]
    #[ignore] // 実際のLSPサーバーが必要
    fn test_code_analysis() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");

        fs::write(
            &test_file,
            r#"
struct Foo {
    bar: String,
}

impl Foo {
    fn new() -> Self {
        Self {
            bar: String::new(),
        }
    }
    
    fn get_bar(&self) -> &str {
        &self.bar
    }
}

fn main() {
    let foo = Foo::new();
    println!("{}", foo.get_bar());
}
"#,
        )
        .unwrap();

        let adapter = Box::new(RustAnalyzerAdapter);
        let client = Arc::new(LspClient::new(adapter).unwrap());
        let analyzer = LspCodeAnalyzer::new(client);

        let uri = Url::from_file_path(&test_file).unwrap();
        let structure = analyzer.analyze_file_structure(uri.as_str());

        assert!(structure.is_ok());
        let structure = structure.unwrap();
        assert!(!structure.symbols.is_empty());

        // 構造体、impl、関数が含まれているか確認
        let has_struct = structure
            .symbols
            .iter()
            .any(|s| s.kind == SymbolKind::STRUCT);
        let has_function = structure
            .symbols
            .iter()
            .any(|s| s.kind == SymbolKind::FUNCTION);

        assert!(has_struct || has_function);
    }

    #[test]
    #[ignore] // 実際のLSPサーバーが必要
    fn test_diagnostics_watcher() {
        let adapter = Box::new(RustAnalyzerAdapter);
        let client = Arc::new(LspClient::new(adapter).unwrap());

        let (watcher, tx) = DiagnosticsWatcher::new(client.clone());

        // テスト用の診断情報を送信
        let test_uri = Url::parse("file:///test.rs").unwrap();
        let diagnostics = vec![Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("E0001".to_string())),
            source: Some("test".to_string()),
            message: "Test error".to_string(),
            related_information: None,
            tags: None,
            code_description: None,
            data: None,
        }];

        let params = PublishDiagnosticsParams {
            uri: test_uri.clone(),
            diagnostics: diagnostics.clone(),
            version: None,
        };

        // 非同期で診断情報を送信
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            tx.send(params).await.unwrap();

            // ワーカーを少し実行
            tokio::select! {
                _ = watcher.run() => {},
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {},
            }
        });

        // 診断情報が保存されたか確認
        let saved_diagnostics = client.get_diagnostics(&test_uri);
        assert_eq!(saved_diagnostics.len(), 1);
        assert_eq!(saved_diagnostics[0].message, "Test error");
    }

    #[test]
    fn test_dependency_graph() {
        let mut graph = DependencyGraph::new();

        graph.add_dependency("file1.rs", "file2.rs");
        graph.add_dependency("file1.rs", "file3.rs");
        graph.add_dependency("file2.rs", "file3.rs");

        let deps = graph.get_dependencies("file1.rs");
        assert!(deps.is_some());
        assert_eq!(deps.unwrap().len(), 2);

        let all_deps = graph.get_all_dependencies();
        assert_eq!(all_deps.len(), 2);
    }

    #[test]
    fn test_symbol_info() {
        let position = Position {
            line: 10,
            character: 5,
        };
        let hover = Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String("Test hover".to_string())),
            range: None,
        });
        let definition = Some(Location {
            uri: Url::parse("file:///test.rs").unwrap(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
        });
        let references = vec![
            Location {
                uri: Url::parse("file:///test1.rs").unwrap(),
                range: Range {
                    start: Position {
                        line: 5,
                        character: 0,
                    },
                    end: Position {
                        line: 5,
                        character: 10,
                    },
                },
            },
            Location {
                uri: Url::parse("file:///test2.rs").unwrap(),
                range: Range {
                    start: Position {
                        line: 15,
                        character: 0,
                    },
                    end: Position {
                        line: 15,
                        character: 10,
                    },
                },
            },
        ];

        let symbol_info = SymbolInfo {
            position,
            hover,
            definition,
            references,
        };

        assert_eq!(symbol_info.position.line, 10);
        assert!(symbol_info.hover.is_some());
        assert!(symbol_info.definition.is_some());
        assert_eq!(symbol_info.references.len(), 2);
    }
}

#[cfg(test)]
mod command_tests {
    use cli::lsp_commands::{LspCommand, LspSubcommand};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    #[ignore] // 実際のLSPサーバーが必要
    fn test_hover_command() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");

        fs::write(
            &test_file,
            r#"
fn main() {
    let x = 42;
}
"#,
        )
        .unwrap();

        let cmd = LspCommand {
            command: LspSubcommand::Hover {
                file: test_file.to_string_lossy().to_string(),
                line: 3,
                column: 9,
            },
        };

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(cmd.execute());
        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // 実際のLSPサーバーが必要
    fn test_symbols_command() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");

        fs::write(
            &test_file,
            r#"
struct Foo {
    bar: i32,
}

fn main() {
    let foo = Foo { bar: 42 };
}
"#,
        )
        .unwrap();

        let cmd = LspCommand {
            command: LspSubcommand::Symbols {
                file: test_file.to_string_lossy().to_string(),
                hierarchical: true,
            },
        };

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(cmd.execute());
        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // 実際のLSPサーバーが必要
    fn test_completion_command() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");

        fs::write(
            &test_file,
            r#"
fn main() {
    let s = String::new();
    s.
}
"#,
        )
        .unwrap();

        let cmd = LspCommand {
            command: LspSubcommand::Complete {
                file: test_file.to_string_lossy().to_string(),
                line: 4,
                column: 6,
                trigger_character: Some(".".to_string()),
            },
        };

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(cmd.execute());
        assert!(result.is_ok());
    }
}
