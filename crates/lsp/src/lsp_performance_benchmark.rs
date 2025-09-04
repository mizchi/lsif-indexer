use anyhow::{Context, Result};
use lsp_types::*;
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct LspBenchmark {
    lsp_command: Vec<String>,
    workspace_path: PathBuf,
    process: Option<Child>,
    stdin: Option<std::process::ChildStdin>,
    stdout: Option<BufReader<std::process::ChildStdout>>,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub operation: String,
    pub duration: Duration,
    pub success: bool,
    pub error: Option<String>,
}

impl LspBenchmark {
    pub fn new(lsp_command: Vec<String>, workspace_path: PathBuf) -> Self {
        Self {
            lsp_command,
            workspace_path,
            process: None,
            stdin: None,
            stdout: None,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        let mut cmd = Command::new(&self.lsp_command[0]);
        if self.lsp_command.len() > 1 {
            cmd.args(&self.lsp_command[1..]);
        }

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn LSP process")?;

        let stdin = child.stdin.take().context("Failed to get stdin")?;
        let stdout = child.stdout.take().context("Failed to get stdout")?;

        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.process = Some(child);

        Ok(())
    }

    pub fn initialize(&mut self) -> Result<BenchmarkResult> {
        let start = Instant::now();

        let init_params = InitializeParams {
            process_id: Some(std::process::id()),
            root_path: None,
            root_uri: Url::from_file_path(&self.workspace_path).ok(),
            capabilities: ClientCapabilities {
                workspace: Some(WorkspaceClientCapabilities {
                    workspace_folders: Some(true),
                    symbol: Some(WorkspaceSymbolClientCapabilities {
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
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                text_document: Some(TextDocumentClientCapabilities {
                    document_symbol: Some(DocumentSymbolClientCapabilities {
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
                        hierarchical_document_symbol_support: Some(true),
                        ..Default::default()
                    }),
                    definition: Some(GotoCapability {
                        dynamic_registration: Some(false),
                        link_support: Some(false),
                    }),
                    references: Some(ReferenceClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            initialization_options: None,
            trace: Some(TraceValue::Off),
            workspace_folders: Url::from_file_path(&self.workspace_path).ok().map(|uri| {
                vec![WorkspaceFolder {
                    uri,
                    name: "workspace".to_string(),
                }]
            }),
            client_info: Some(ClientInfo {
                name: "lsp-benchmark".to_string(),
                version: Some("1.0.0".to_string()),
            }),
            locale: Some("en".to_string()),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.send_request("initialize", json!(init_params))?;
        let response = self.read_response()?;
        let duration = start.elapsed();

        let success = response.get("result").is_some();
        let error = response
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .map(|s| s.to_string());

        self.send_notification("initialized", json!({}))?;

        Ok(BenchmarkResult {
            operation: "initialize".to_string(),
            duration,
            success,
            error,
        })
    }

    pub fn benchmark_workspace_symbol(&mut self, query: &str) -> Result<BenchmarkResult> {
        let start = Instant::now();

        let params = WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.send_request("workspace/symbol", json!(params))?;
        let response = self.read_response()?;
        let duration = start.elapsed();

        let success = response.get("result").is_some();
        let error = response
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .map(|s| s.to_string());

        Ok(BenchmarkResult {
            operation: format!("workspace/symbol (query: '{}')", query),
            duration,
            success,
            error,
        })
    }

    pub fn benchmark_document_symbol(&mut self, file_path: &str) -> Result<BenchmarkResult> {
        let start = Instant::now();

        let uri = Url::from_file_path(self.workspace_path.join(file_path))
            .map_err(|_| anyhow::anyhow!("Invalid file path"))?;

        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.send_request("textDocument/documentSymbol", json!(params))?;
        let response = self.read_response()?;
        let duration = start.elapsed();

        let success = response.get("result").is_some();
        let error = response
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .map(|s| s.to_string());

        Ok(BenchmarkResult {
            operation: format!("textDocument/documentSymbol (file: '{}')", file_path),
            duration,
            success,
            error,
        })
    }

    pub fn benchmark_definition(
        &mut self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<BenchmarkResult> {
        let start = Instant::now();

        let uri = Url::from_file_path(self.workspace_path.join(file_path))
            .map_err(|_| anyhow::anyhow!("Invalid file path"))?;

        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.send_request("textDocument/definition", json!(params))?;
        let response = self.read_response()?;
        let duration = start.elapsed();

        let success = response.get("result").is_some();
        let error = response
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .map(|s| s.to_string());

        Ok(BenchmarkResult {
            operation: format!(
                "textDocument/definition ({}:{}:{})",
                file_path, line, column
            ),
            duration,
            success,
            error,
        })
    }

    pub fn benchmark_references(
        &mut self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<BenchmarkResult> {
        let start = Instant::now();

        let uri = Url::from_file_path(self.workspace_path.join(file_path))
            .map_err(|_| anyhow::anyhow!("Invalid file path"))?;

        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line,
                    character: column,
                },
            },
            context: ReferenceContext {
                include_declaration: true,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.send_request("textDocument/references", json!(params))?;
        let response = self.read_response()?;
        let duration = start.elapsed();

        let success = response.get("result").is_some();
        let error = response
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .map(|s| s.to_string());

        Ok(BenchmarkResult {
            operation: format!(
                "textDocument/references ({}:{}:{})",
                file_path, line, column
            ),
            duration,
            success,
            error,
        })
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.send_request("shutdown", json!(null))?;
        let _ = self.read_response()?;
        self.send_notification("exit", json!(null))?;

        if let Some(mut process) = self.process.take() {
            let _ = process.wait();
        }

        Ok(())
    }

    fn send_request(&mut self, method: &str, params: serde_json::Value) -> Result<()> {
        static mut REQUEST_ID: u64 = 0;
        let id = unsafe {
            REQUEST_ID += 1;
            REQUEST_ID
        };

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        self.send_message(&request)
    }

    fn send_notification(&mut self, method: &str, params: serde_json::Value) -> Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        self.send_message(&notification)
    }

    fn send_message(&mut self, message: &serde_json::Value) -> Result<()> {
        let content = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        if let Some(stdin) = &mut self.stdin {
            stdin.write_all(header.as_bytes())?;
            stdin.write_all(content.as_bytes())?;
            stdin.flush()?;
        }

        Ok(())
    }

    fn read_response(&mut self) -> Result<serde_json::Value> {
        if let Some(stdout) = &mut self.stdout {
            let mut headers = HashMap::new();
            let mut line = String::new();

            loop {
                line.clear();
                stdout.read_line(&mut line)?;
                if line == "\r\n" {
                    break;
                }
                let parts: Vec<&str> = line.trim().split(':').collect();
                if parts.len() == 2 {
                    headers.insert(parts[0].to_string(), parts[1].trim().to_string());
                }
            }

            if let Some(content_length) = headers.get("Content-Length") {
                let length: usize = content_length.parse()?;
                let mut buffer = vec![0u8; length];
                stdout.read_exact(&mut buffer)?;
                let response: serde_json::Value = serde_json::from_slice(&buffer)?;
                Ok(response)
            } else {
                Err(anyhow::anyhow!("No Content-Length header"))
            }
        } else {
            Err(anyhow::anyhow!("No stdout available"))
        }
    }
}

pub fn run_benchmark_suite(
    lsp_name: &str,
    lsp_command: Vec<String>,
    workspace_path: PathBuf,
    test_files: Vec<&str>,
) -> Vec<BenchmarkResult> {
    let mut results = Vec::new();
    let mut benchmark = LspBenchmark::new(lsp_command, workspace_path.clone());

    println!("\n=== Benchmarking {} ===", lsp_name);

    if let Err(e) = benchmark.start() {
        println!("Failed to start {}: {}", lsp_name, e);
        return results;
    }

    match benchmark.initialize() {
        Ok(result) => {
            println!("✓ Initialize: {:.3}s", result.duration.as_secs_f64());
            results.push(result);
        }
        Err(e) => {
            println!("✗ Initialize failed: {}", e);
            return results;
        }
    }

    std::thread::sleep(Duration::from_millis(500));

    let queries = vec!["main", "test", "handle", "process", ""];
    for query in queries {
        match benchmark.benchmark_workspace_symbol(query) {
            Ok(result) => {
                if result.success {
                    println!(
                        "✓ workspace/symbol ('{}'): {:.3}s",
                        query,
                        result.duration.as_secs_f64()
                    );
                } else {
                    println!(
                        "✗ workspace/symbol ('{}') failed: {:?}",
                        query, result.error
                    );
                }
                results.push(result);
            }
            Err(e) => {
                println!("✗ workspace/symbol ('{}') error: {}", query, e);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    for file_path in test_files {
        match benchmark.benchmark_document_symbol(file_path) {
            Ok(result) => {
                if result.success {
                    println!(
                        "✓ documentSymbol ({}): {:.3}s",
                        file_path,
                        result.duration.as_secs_f64()
                    );
                } else {
                    println!(
                        "✗ documentSymbol ({}) failed: {:?}",
                        file_path, result.error
                    );
                }
                results.push(result);
            }
            Err(e) => {
                println!("✗ documentSymbol ({}) error: {}", file_path, e);
            }
        }
        std::thread::sleep(Duration::from_millis(100));

        match benchmark.benchmark_definition(file_path, 10, 5) {
            Ok(result) => {
                if result.success {
                    println!(
                        "✓ definition ({}:10:5): {:.3}s",
                        file_path,
                        result.duration.as_secs_f64()
                    );
                } else {
                    println!(
                        "✗ definition ({}:10:5) failed: {:?}",
                        file_path, result.error
                    );
                }
                results.push(result);
            }
            Err(e) => {
                println!("✗ definition ({}:10:5) error: {}", file_path, e);
            }
        }
        std::thread::sleep(Duration::from_millis(100));

        match benchmark.benchmark_references(file_path, 10, 5) {
            Ok(result) => {
                if result.success {
                    println!(
                        "✓ references ({}:10:5): {:.3}s",
                        file_path,
                        result.duration.as_secs_f64()
                    );
                } else {
                    println!(
                        "✗ references ({}:10:5) failed: {:?}",
                        file_path, result.error
                    );
                }
                results.push(result);
            }
            Err(e) => {
                println!("✗ references ({}:10:5) error: {}", file_path, e);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let _ = benchmark.shutdown();
    results
}

pub fn analyze_results(all_results: HashMap<String, Vec<BenchmarkResult>>) {
    println!("\n=== パフォーマンス分析結果 ===\n");

    let mut operation_stats: HashMap<String, Vec<(String, Duration)>> = HashMap::new();

    for (lsp_name, results) in &all_results {
        for result in results {
            if result.success {
                let op_type = result
                    .operation
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown");
                operation_stats
                    .entry(op_type.to_string())
                    .or_default()
                    .push((lsp_name.clone(), result.duration));
            }
        }
    }

    println!("## 操作別平均レスポンス時間\n");
    println!("| 操作 | tsgo | rust-analyzer | gopls |");
    println!("|------|------|---------------|-------|");

    for op_type in [
        "initialize",
        "workspace/symbol",
        "textDocument/documentSymbol",
        "textDocument/definition",
        "textDocument/references",
    ] {
        if let Some(stats) = operation_stats.get(op_type) {
            let mut row = format!("| {} |", op_type);

            for lsp in ["tsgo", "rust-analyzer", "gopls"] {
                let times: Vec<Duration> = stats
                    .iter()
                    .filter(|(name, _)| name == lsp)
                    .map(|(_, d)| *d)
                    .collect();

                if !times.is_empty() {
                    let avg = times.iter().sum::<Duration>() / times.len() as u32;
                    row.push_str(&format!(" {:.3}s |", avg.as_secs_f64()));
                } else {
                    row.push_str(" N/A |");
                }
            }
            println!("{}", row);
        }
    }

    println!("\n## 考察\n");

    let mut fastest_init = ("", Duration::from_secs(999));
    let mut fastest_workspace_symbol = ("", Duration::from_secs(999));
    let mut fastest_document_symbol = ("", Duration::from_secs(999));

    for (lsp_name, results) in &all_results {
        for result in results {
            if result.success {
                if result.operation == "initialize" && result.duration < fastest_init.1 {
                    fastest_init = (lsp_name.as_str(), result.duration);
                }
                if result.operation.starts_with("workspace/symbol")
                    && result.duration < fastest_workspace_symbol.1
                {
                    fastest_workspace_symbol = (lsp_name.as_str(), result.duration);
                }
                if result.operation.starts_with("textDocument/documentSymbol")
                    && result.duration < fastest_document_symbol.1
                {
                    fastest_document_symbol = (lsp_name.as_str(), result.duration);
                }
            }
        }
    }

    println!("### 初期化パフォーマンス");
    println!(
        "- 最速: {} ({:.3}s)",
        fastest_init.0,
        fastest_init.1.as_secs_f64()
    );
    println!("- 推奨: 初期化は1度だけなので、機能の充実度を優先すべき\n");

    println!("### workspace/symbol (プロジェクト全体検索)");
    println!(
        "- 最速: {} ({:.3}s)",
        fastest_workspace_symbol.0,
        fastest_workspace_symbol.1.as_secs_f64()
    );
    println!("- 推奨: 大規模プロジェクトでは応答速度が重要。キャッシュ戦略も検討\n");

    println!("### textDocument/documentSymbol (ファイル内シンボル)");
    println!(
        "- 最速: {} ({:.3}s)",
        fastest_document_symbol.0,
        fastest_document_symbol.1.as_secs_f64()
    );
    println!("- 推奨: 頻繁に呼ばれるため、レスポンス時間が重要\n");

    println!("## 最適なアクセス戦略\n");
    println!("1. **並列処理**: 複数のLSPインスタンスをプールして並列アクセス");
    println!("2. **キャッシュ**: workspace/symbolの結果を積極的にキャッシュ");
    println!("3. **バッチ処理**: 複数のdocumentSymbolリクエストをバッチ化");
    println!("4. **インクリメンタル更新**: ファイル変更時のみ再インデックス");
    println!("5. **タイムアウト設定**: 操作ごとに適切なタイムアウトを設定");
    println!("   - initialize: 30秒");
    println!("   - workspace/symbol: 5秒");
    println!("   - documentSymbol: 2秒");
    println!("   - definition/references: 3秒");
}
