use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use lsp_types::request::{
    GotoImplementationParams, GotoImplementationResponse, GotoTypeDefinitionParams,
    GotoTypeDefinitionResponse,
};
use lsp_types::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::adapter::lsp::detect_language;
use super::lsp_features::{LspClient, LspCodeAnalyzer};

#[derive(Debug, Args)]
pub struct LspCommand {
    #[command(subcommand)]
    pub command: LspSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum LspSubcommand {
    /// 指定位置のホバー情報を表示
    Hover {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
    },

    /// 指定位置の定義にジャンプ
    Definition {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
    },

    /// 指定位置の参照を検索
    References {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
        /// 宣言を含めるかどうか
        #[arg(long, default_value = "true")]
        include_declaration: bool,
    },

    /// 指定位置の実装にジャンプ
    Implementation {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
    },

    /// 指定位置の型定義にジャンプ
    TypeDefinition {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
    },

    /// ファイル内のシンボルを表示
    Symbols {
        /// ファイルパス
        file: String,
        /// 階層表示
        #[arg(long, default_value = "true")]
        hierarchical: bool,
    },

    /// ワークスペース内のシンボルを検索
    WorkspaceSymbols {
        /// 検索クエリ
        query: String,
        /// 最大結果数
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// コード補完を取得
    Complete {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
        /// トリガー文字
        #[arg(long)]
        trigger_character: Option<String>,
    },

    /// シグネチャヘルプを表示
    SignatureHelp {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
    },

    /// コードアクションを取得
    CodeActions {
        /// ファイルパス
        file: String,
        /// 開始行番号（1から開始）
        #[arg(long)]
        start_line: u32,
        /// 開始列番号（1から開始）
        #[arg(long)]
        start_column: u32,
        /// 終了行番号（1から開始）
        #[arg(long)]
        end_line: u32,
        /// 終了列番号（1から開始）
        #[arg(long)]
        end_column: u32,
    },

    /// ドキュメントをフォーマット
    Format {
        /// ファイルパス
        file: String,
        /// タブサイズ
        #[arg(long, default_value = "4")]
        tab_size: u32,
        /// スペースでインデント
        #[arg(long, default_value = "true")]
        insert_spaces: bool,
    },

    /// シンボルの名前を変更
    Rename {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
        /// 新しい名前
        new_name: String,
    },

    /// コール階層を表示
    CallHierarchy {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
        /// 方向（incoming/outgoing）
        #[arg(long, default_value = "incoming")]
        direction: String,
    },

    /// 型階層を表示
    TypeHierarchy {
        /// ファイルパス
        file: String,
        /// 行番号（1から開始）
        #[arg(long)]
        line: u32,
        /// 列番号（1から開始）
        #[arg(long)]
        column: u32,
        /// 方向（supertypes/subtypes）
        #[arg(long, default_value = "supertypes")]
        direction: String,
    },

    /// ファイルの診断情報を表示
    Diagnostics {
        /// ファイルパス
        file: String,
        /// 重要度フィルタ（error/warning/information/hint）
        #[arg(long)]
        severity: Option<String>,
    },

    /// インレイヒントを表示
    InlayHints {
        /// ファイルパス
        file: String,
        /// 開始行番号
        #[arg(long)]
        start_line: Option<u32>,
        /// 終了行番号
        #[arg(long)]
        end_line: Option<u32>,
    },

    /// ファイルの依存関係を解析
    Dependencies {
        /// ファイルパス
        file: String,
        /// 再帰的に解析
        #[arg(long, default_value = "true")]
        recursive: bool,
    },
}

impl LspCommand {
    pub async fn execute(&self) -> Result<()> {
        match &self.command {
            LspSubcommand::Hover { file, line, column } => {
                self.execute_hover(file, *line, *column).await
            }
            LspSubcommand::Definition { file, line, column } => {
                self.execute_definition(file, *line, *column).await
            }
            LspSubcommand::References {
                file,
                line,
                column,
                include_declaration,
            } => {
                self.execute_references(file, *line, *column, *include_declaration)
                    .await
            }
            LspSubcommand::Implementation { file, line, column } => {
                self.execute_implementation(file, *line, *column).await
            }
            LspSubcommand::TypeDefinition { file, line, column } => {
                self.execute_type_definition(file, *line, *column).await
            }
            LspSubcommand::Symbols { file, hierarchical } => {
                self.execute_symbols(file, *hierarchical).await
            }
            LspSubcommand::WorkspaceSymbols { query, limit } => {
                self.execute_workspace_symbols(query, *limit).await
            }
            LspSubcommand::Complete {
                file,
                line,
                column,
                trigger_character,
            } => {
                self.execute_complete(file, *line, *column, trigger_character.as_deref())
                    .await
            }
            LspSubcommand::SignatureHelp { file, line, column } => {
                self.execute_signature_help(file, *line, *column).await
            }
            LspSubcommand::CodeActions {
                file,
                start_line,
                start_column,
                end_line,
                end_column,
            } => {
                self.execute_code_actions(file, *start_line, *start_column, *end_line, *end_column)
                    .await
            }
            LspSubcommand::Format {
                file,
                tab_size,
                insert_spaces,
            } => self.execute_format(file, *tab_size, *insert_spaces).await,
            LspSubcommand::Rename {
                file,
                line,
                column,
                new_name,
            } => self.execute_rename(file, *line, *column, new_name).await,
            LspSubcommand::CallHierarchy {
                file,
                line,
                column,
                direction,
            } => {
                self.execute_call_hierarchy(file, *line, *column, direction)
                    .await
            }
            LspSubcommand::TypeHierarchy {
                file,
                line,
                column,
                direction,
            } => {
                self.execute_type_hierarchy(file, *line, *column, direction)
                    .await
            }
            LspSubcommand::Diagnostics { file, severity } => {
                self.execute_diagnostics(file, severity.as_deref()).await
            }
            LspSubcommand::InlayHints {
                file,
                start_line,
                end_line,
            } => self.execute_inlay_hints(file, *start_line, *end_line).await,
            LspSubcommand::Dependencies { file, recursive } => {
                self.execute_dependencies(file, *recursive).await
            }
        }
    }

    async fn execute_hover(&self, file: &str, line: u32, column: u32) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let hover = client.hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        })?;

        if let Some(hover) = hover {
            println!("=== Hover Information ===");
            match hover.contents {
                HoverContents::Scalar(content) => {
                    self.print_marked_string(&content);
                }
                HoverContents::Array(contents) => {
                    for content in contents {
                        self.print_marked_string(&content);
                    }
                }
                HoverContents::Markup(markup) => {
                    println!("{}", markup.value);
                }
            }

            if let Some(range) = hover.range {
                println!(
                    "\nRange: {}:{} - {}:{}",
                    range.start.line + 1,
                    range.start.character + 1,
                    range.end.line + 1,
                    range.end.character + 1
                );
            }
        } else {
            println!("No hover information available at this position");
        }

        Ok(())
    }

    async fn execute_definition(&self, file: &str, line: u32, column: u32) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let mut lsp_client = client.client.lock().unwrap();
        let location = lsp_client.goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })?;

        println!("=== Definition ===");
        self.print_location(&location);

        Ok(())
    }

    async fn execute_references(
        &self,
        file: &str,
        line: u32,
        column: u32,
        include_declaration: bool,
    ) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let mut lsp_client = client.client.lock().unwrap();
        let references = lsp_client.find_references(ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            context: ReferenceContext {
                include_declaration,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })?;

        println!("=== References ({} found) ===", references.len());
        for (i, reference) in references.iter().enumerate() {
            println!("\n[{}]", i + 1);
            self.print_location(reference);
        }

        Ok(())
    }

    async fn execute_implementation(&self, file: &str, line: u32, column: u32) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let result = client.goto_implementation(GotoImplementationParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })?;

        println!("=== Implementation ===");
        if let Some(response) = result {
            match response {
                GotoImplementationResponse::Scalar(location) => {
                    self.print_location(&location);
                }
                GotoImplementationResponse::Array(locations) => {
                    for (i, location) in locations.iter().enumerate() {
                        println!("\n[{}]", i + 1);
                        self.print_location(location);
                    }
                }
                GotoImplementationResponse::Link(links) => {
                    for (i, link) in links.iter().enumerate() {
                        println!("\n[{}]", i + 1);
                        self.print_location_link(link);
                    }
                }
            }
        } else {
            println!("No implementation found");
        }

        Ok(())
    }

    async fn execute_type_definition(&self, file: &str, line: u32, column: u32) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let result = client.goto_type_definition(GotoTypeDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })?;

        println!("=== Type Definition ===");
        if let Some(response) = result {
            match response {
                GotoTypeDefinitionResponse::Scalar(location) => {
                    self.print_location(&location);
                }
                GotoTypeDefinitionResponse::Array(locations) => {
                    for (i, location) in locations.iter().enumerate() {
                        println!("\n[{}]", i + 1);
                        self.print_location(location);
                    }
                }
                GotoTypeDefinitionResponse::Link(links) => {
                    for (i, link) in links.iter().enumerate() {
                        println!("\n[{}]", i + 1);
                        self.print_location_link(link);
                    }
                }
            }
        } else {
            println!("No type definition found");
        }

        Ok(())
    }

    async fn execute_symbols(&self, file: &str, hierarchical: bool) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let analyzer = LspCodeAnalyzer::new(Arc::new(client));
        let structure = analyzer.analyze_file_structure(uri.as_str())?;

        println!("=== Document Symbols ===");
        println!("File: {file}");
        println!();

        for symbol in &structure.symbols {
            self.print_symbol(symbol, 0, hierarchical);
        }

        Ok(())
    }

    async fn execute_workspace_symbols(&self, query: &str, limit: usize) -> Result<()> {
        let client = self.create_client(".")?; // Use current directory

        let result = client.workspace_symbol(WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })?;

        println!("=== Workspace Symbols ===");
        println!("Query: {query}");

        if let Some(symbols) = result {
            let symbols: Vec<_> = symbols.into_iter().take(limit).collect();
            println!("Found {} symbols (showing up to {})", symbols.len(), limit);
            println!();

            for (i, symbol) in symbols.iter().enumerate() {
                println!(
                    "[{}] {} ({})",
                    i + 1,
                    symbol.name,
                    self.symbol_kind_to_string(symbol.kind)
                );
                println!(
                    "    Location: {}:{}:{}",
                    symbol.location.uri,
                    symbol.location.range.start.line + 1,
                    symbol.location.range.start.character + 1
                );
                if let Some(container) = &symbol.container_name {
                    println!("    Container: {container}");
                }
            }
        } else {
            println!("No symbols found");
        }

        Ok(())
    }

    async fn execute_complete(
        &self,
        file: &str,
        line: u32,
        column: u32,
        trigger_character: Option<&str>,
    ) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let mut context = None;
        if let Some(trigger) = trigger_character {
            context = Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(trigger.to_string()),
            });
        }

        let result = client.completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            context,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })?;

        println!("=== Completions ===");

        if let Some(response) = result {
            let items = match response {
                CompletionResponse::Array(items) => items,
                CompletionResponse::List(list) => list.items,
            };

            println!("Found {} completions", items.len());
            println!();

            for (i, item) in items.iter().enumerate().take(20) {
                println!("[{}] {}", i + 1, item.label);
                if let Some(kind) = item.kind {
                    println!("    Kind: {kind:?}");
                }
                if let Some(detail) = &item.detail {
                    println!("    Detail: {detail}");
                }
                if let Some(doc) = &item.documentation {
                    match doc {
                        Documentation::String(s) => {
                            println!("    Doc: {}", s.lines().next().unwrap_or(""));
                        }
                        Documentation::MarkupContent(content) => {
                            println!("    Doc: {}", content.value.lines().next().unwrap_or(""));
                        }
                    }
                }
            }

            if items.len() > 20 {
                println!("\n... and {} more", items.len() - 20);
            }
        } else {
            println!("No completions available");
        }

        Ok(())
    }

    async fn execute_signature_help(&self, file: &str, line: u32, column: u32) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let result = client.signature_help(SignatureHelpParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            context: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
        })?;

        println!("=== Signature Help ===");

        if let Some(help) = result {
            for (i, signature) in help.signatures.iter().enumerate() {
                let is_active = help
                    .active_signature
                    .map(|idx| idx as usize == i)
                    .unwrap_or(false);
                println!(
                    "{}[{}] {}",
                    if is_active { ">" } else { " " },
                    i + 1,
                    signature.label
                );

                if let Some(doc) = &signature.documentation {
                    match doc {
                        Documentation::String(s) => {
                            println!("    Doc: {s}");
                        }
                        Documentation::MarkupContent(content) => {
                            println!("    Doc: {}", content.value);
                        }
                    }
                }

                if let Some(params) = &signature.parameters {
                    println!("    Parameters:");
                    for (j, param) in params.iter().enumerate() {
                        let is_active_param = help
                            .active_parameter
                            .map(|idx| idx as usize == j)
                            .unwrap_or(false);
                        println!(
                            "      {}[{}] {}",
                            if is_active_param { ">" } else { " " },
                            j + 1,
                            match &param.label {
                                ParameterLabel::Simple(s) => s.clone(),
                                ParameterLabel::LabelOffsets([start, end]) => {
                                    signature.label[*start as usize..*end as usize].to_string()
                                }
                            }
                        );
                    }
                }
            }
        } else {
            println!("No signature help available");
        }

        Ok(())
    }

    async fn execute_code_actions(
        &self,
        file: &str,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let result = client.code_action(CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position {
                    line: start_line - 1,
                    character: start_column - 1,
                },
                end: Position {
                    line: end_line - 1,
                    character: end_column - 1,
                },
            },
            context: CodeActionContext {
                diagnostics: client.get_diagnostics(&uri),
                only: None,
                trigger_kind: Some(CodeActionTriggerKind::INVOKED),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })?;

        println!("=== Code Actions ===");

        if let Some(response) = result {
            let actions: Vec<CodeAction> = response
                .into_iter()
                .filter_map(|action| match action {
                    CodeActionOrCommand::CodeAction(action) => Some(action),
                    CodeActionOrCommand::Command(cmd) => {
                        println!("Command: {} ({})", cmd.title, cmd.command);
                        None
                    }
                })
                .collect();

            println!("Found {} code actions", actions.len());
            println!();

            for (i, action) in actions.iter().enumerate() {
                println!("[{}] {}", i + 1, action.title);
                if let Some(kind) = &action.kind {
                    println!("    Kind: {kind:?}");
                }
                if let Some(edit) = &action.edit {
                    println!(
                        "    Has workspace edit with {} changes",
                        edit.changes.as_ref().map(|c| c.len()).unwrap_or(0)
                    );
                }
            }
        } else {
            println!("No code actions available");
        }

        Ok(())
    }

    async fn execute_format(&self, file: &str, tab_size: u32, insert_spaces: bool) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let result = client.formatting(DocumentFormattingParams {
            text_document: TextDocumentIdentifier { uri },
            options: FormattingOptions {
                tab_size,
                insert_spaces,
                ..Default::default()
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        })?;

        println!("=== Format Result ===");

        if let Some(edits) = result {
            println!("Formatting would apply {} edits:", edits.len());
            for (i, edit) in edits.iter().enumerate() {
                println!("\n[Edit {}]", i + 1);
                println!(
                    "Range: {}:{} - {}:{}",
                    edit.range.start.line + 1,
                    edit.range.start.character + 1,
                    edit.range.end.line + 1,
                    edit.range.end.character + 1
                );
                if edit.new_text.is_empty() {
                    println!("Action: Delete");
                } else {
                    println!("Action: Replace with {} characters", edit.new_text.len());
                }
            }
        } else {
            println!("No formatting changes needed");
        }

        Ok(())
    }

    async fn execute_rename(
        &self,
        file: &str,
        line: u32,
        column: u32,
        new_name: &str,
    ) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        // First check if rename is valid at this position
        let prepare_result = client.prepare_rename(TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: line - 1,
                character: column - 1,
            },
        })?;

        if prepare_result.is_none() {
            println!("Cannot rename at this position");
            return Ok(());
        }

        let result = client.rename(RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            new_name: new_name.to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
        })?;

        println!("=== Rename Result ===");
        println!("New name: {new_name}");

        if let Some(edit) = result {
            self.print_workspace_edit(&edit);
        } else {
            println!("No changes needed");
        }

        Ok(())
    }

    async fn execute_call_hierarchy(
        &self,
        file: &str,
        line: u32,
        column: u32,
        direction: &str,
    ) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        // Prepare call hierarchy
        let items = client.prepare_call_hierarchy(CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        })?;

        if let Some(items) = items {
            if items.is_empty() {
                println!("No call hierarchy available at this position");
                return Ok(());
            }

            println!("=== Call Hierarchy ===");

            for item in items {
                println!(
                    "Item: {} ({})",
                    item.name,
                    self.symbol_kind_to_string(item.kind)
                );
                println!(
                    "Location: {}:{}:{}",
                    item.uri,
                    item.range.start.line + 1,
                    item.range.start.character + 1
                );

                match direction {
                    "incoming" => {
                        let calls = client.incoming_calls(CallHierarchyIncomingCallsParams {
                            item: item.clone(),
                            work_done_progress_params: WorkDoneProgressParams::default(),
                            partial_result_params: PartialResultParams::default(),
                        })?;

                        if let Some(calls) = calls {
                            println!("\nIncoming calls ({}):", calls.len());
                            for call in calls {
                                println!(
                                    "  <- {} ({})",
                                    call.from.name,
                                    self.symbol_kind_to_string(call.from.kind)
                                );
                                println!(
                                    "     From: {}:{}:{}",
                                    call.from.uri,
                                    call.from.range.start.line + 1,
                                    call.from.range.start.character + 1
                                );
                            }
                        }
                    }
                    "outgoing" => {
                        let calls = client.outgoing_calls(CallHierarchyOutgoingCallsParams {
                            item: item.clone(),
                            work_done_progress_params: WorkDoneProgressParams::default(),
                            partial_result_params: PartialResultParams::default(),
                        })?;

                        if let Some(calls) = calls {
                            println!("\nOutgoing calls ({}):", calls.len());
                            for call in calls {
                                println!(
                                    "  -> {} ({})",
                                    call.to.name,
                                    self.symbol_kind_to_string(call.to.kind)
                                );
                                println!(
                                    "     To: {}:{}:{}",
                                    call.to.uri,
                                    call.to.range.start.line + 1,
                                    call.to.range.start.character + 1
                                );
                            }
                        }
                    }
                    _ => {
                        println!("Invalid direction. Use 'incoming' or 'outgoing'");
                    }
                }
            }
        } else {
            println!("No call hierarchy available at this position");
        }

        Ok(())
    }

    async fn execute_type_hierarchy(
        &self,
        file: &str,
        line: u32,
        column: u32,
        direction: &str,
    ) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        // Prepare type hierarchy
        let items = client.prepare_type_hierarchy(TypeHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: line - 1,
                    character: column - 1,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        })?;

        if let Some(items) = items {
            if items.is_empty() {
                println!("No type hierarchy available at this position");
                return Ok(());
            }

            println!("=== Type Hierarchy ===");

            for item in items {
                println!(
                    "Type: {} ({})",
                    item.name,
                    self.symbol_kind_to_string(item.kind)
                );
                println!(
                    "Location: {}:{}:{}",
                    item.uri,
                    item.range.start.line + 1,
                    item.range.start.character + 1
                );

                match direction {
                    "supertypes" => {
                        let types =
                            client.type_hierarchy_supertypes(TypeHierarchySupertypesParams {
                                item: item.clone(),
                                work_done_progress_params: WorkDoneProgressParams::default(),
                                partial_result_params: PartialResultParams::default(),
                            })?;

                        if let Some(types) = types {
                            println!("\nSupertypes ({}):", types.len());
                            for t in types {
                                println!(
                                    "  <- {} ({})",
                                    t.name,
                                    self.symbol_kind_to_string(t.kind)
                                );
                                println!(
                                    "     At: {}:{}:{}",
                                    t.uri,
                                    t.range.start.line + 1,
                                    t.range.start.character + 1
                                );
                            }
                        }
                    }
                    "subtypes" => {
                        let types =
                            client.type_hierarchy_subtypes(TypeHierarchySubtypesParams {
                                item: item.clone(),
                                work_done_progress_params: WorkDoneProgressParams::default(),
                                partial_result_params: PartialResultParams::default(),
                            })?;

                        if let Some(types) = types {
                            println!("\nSubtypes ({}):", types.len());
                            for t in types {
                                println!(
                                    "  -> {} ({})",
                                    t.name,
                                    self.symbol_kind_to_string(t.kind)
                                );
                                println!(
                                    "     At: {}:{}:{}",
                                    t.uri,
                                    t.range.start.line + 1,
                                    t.range.start.character + 1
                                );
                            }
                        }
                    }
                    _ => {
                        println!("Invalid direction. Use 'supertypes' or 'subtypes'");
                    }
                }
            }
        } else {
            println!("No type hierarchy available at this position");
        }

        Ok(())
    }

    async fn execute_diagnostics(&self, file: &str, severity: Option<&str>) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        // Wait a bit for diagnostics to be published
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let diagnostics = client.get_diagnostics(&uri);

        println!("=== Diagnostics ===");
        println!("File: {file}");

        let filtered_diagnostics: Vec<_> = if let Some(severity_filter) = severity {
            let severity = match severity_filter {
                "error" => DiagnosticSeverity::ERROR,
                "warning" => DiagnosticSeverity::WARNING,
                "information" => DiagnosticSeverity::INFORMATION,
                "hint" => DiagnosticSeverity::HINT,
                _ => {
                    println!("Invalid severity filter. Use: error, warning, information, or hint");
                    return Ok(());
                }
            };
            diagnostics
                .into_iter()
                .filter(|d| d.severity == Some(severity))
                .collect()
        } else {
            diagnostics
        };

        println!("Found {} diagnostics", filtered_diagnostics.len());
        println!();

        for (i, diagnostic) in filtered_diagnostics.iter().enumerate() {
            println!("[{}] {}", i + 1, diagnostic.message);
            println!(
                "    Severity: {:?}",
                diagnostic
                    .severity
                    .map(|s| match s {
                        DiagnosticSeverity::ERROR => "Error",
                        DiagnosticSeverity::WARNING => "Warning",
                        DiagnosticSeverity::INFORMATION => "Information",
                        DiagnosticSeverity::HINT => "Hint",
                        _ => "Unknown",
                    })
                    .unwrap_or("Unknown")
            );
            println!(
                "    Range: {}:{} - {}:{}",
                diagnostic.range.start.line + 1,
                diagnostic.range.start.character + 1,
                diagnostic.range.end.line + 1,
                diagnostic.range.end.character + 1
            );
            if let Some(code) = &diagnostic.code {
                match code {
                    NumberOrString::Number(n) => println!("    Code: {n}"),
                    NumberOrString::String(s) => println!("    Code: {s}"),
                }
            }
            if let Some(source) = &diagnostic.source {
                println!("    Source: {source}");
            }
        }

        Ok(())
    }

    async fn execute_inlay_hints(
        &self,
        file: &str,
        start_line: Option<u32>,
        end_line: Option<u32>,
    ) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        // Get file length if end_line not specified
        let content = std::fs::read_to_string(file)?;
        let total_lines = content.lines().count() as u32;

        let range = Range {
            start: Position {
                line: start_line.map(|l| l - 1).unwrap_or(0),
                character: 0,
            },
            end: Position {
                line: end_line.map(|l| l - 1).unwrap_or(total_lines),
                character: 0,
            },
        };

        let hints = client.inlay_hint(InlayHintParams {
            text_document: TextDocumentIdentifier { uri },
            range,
            work_done_progress_params: WorkDoneProgressParams::default(),
        })?;

        println!("=== Inlay Hints ===");
        println!("File: {file}");
        println!(
            "Range: lines {} to {}",
            range.start.line + 1,
            range.end.line + 1
        );

        if let Some(hints) = hints {
            println!("Found {} inlay hints", hints.len());
            println!();

            for (i, hint) in hints.iter().enumerate() {
                println!(
                    "[{}] Position: {}:{}",
                    i + 1,
                    hint.position.line + 1,
                    hint.position.character + 1
                );

                match &hint.label {
                    InlayHintLabel::String(s) => {
                        println!("    Label: {s}");
                    }
                    InlayHintLabel::LabelParts(parts) => {
                        print!("    Label: ");
                        for part in parts {
                            print!("{}", part.value);
                        }
                        println!();
                    }
                }

                if let Some(kind) = &hint.kind {
                    println!("    Kind: {kind:?}");
                }

                if let Some(tooltip) = &hint.tooltip {
                    match tooltip {
                        InlayHintTooltip::String(s) => {
                            println!("    Tooltip: {s}");
                        }
                        InlayHintTooltip::MarkupContent(content) => {
                            println!("    Tooltip: {}", content.value);
                        }
                    }
                }
            }
        } else {
            println!("No inlay hints available");
        }

        Ok(())
    }

    async fn execute_dependencies(&self, file: &str, recursive: bool) -> Result<()> {
        let client = self.create_client(file)?;
        let uri = self.file_to_uri(file)?;

        let analyzer = LspCodeAnalyzer::new(Arc::new(client));

        println!("=== Dependencies Analysis ===");
        println!("File: {file}");
        println!("Recursive: {recursive}");
        println!();

        if recursive {
            let graph = analyzer.build_dependency_graph(uri.as_str())?;
            let deps = graph.get_all_dependencies();

            println!("Dependency graph ({} files):", deps.len());
            for (from, to_list) in deps {
                println!("\n{from}");
                for to in to_list {
                    println!("  -> {to}");
                }
            }
        } else {
            let structure = analyzer.analyze_file_structure(uri.as_str())?;
            println!("File structure analysis:");
            println!("Total symbols: {}", structure.symbols.len());

            // Analyze each symbol for external dependencies
            for symbol in &structure.symbols {
                self.analyze_symbol_dependencies(&analyzer, symbol, &uri)?;
            }
        }

        Ok(())
    }

    // Helper methods

    fn create_client(&self, file: &str) -> Result<LspClient> {
        let adapter = detect_language(file).context("Failed to detect language for file")?;

        LspClient::new(adapter)
    }

    fn file_to_uri(&self, file: &str) -> Result<Url> {
        let path = Path::new(file)
            .canonicalize()
            .context("Failed to canonicalize file path")?;

        Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Failed to convert path to URI"))
    }

    fn print_marked_string(&self, content: &MarkedString) {
        match content {
            MarkedString::String(s) => println!("{s}"),
            MarkedString::LanguageString(ls) => {
                println!("```{}", ls.language);
                println!("{}", ls.value);
                println!("```");
            }
        }
    }

    fn print_location(&self, location: &Location) {
        println!("File: {}", location.uri);
        println!(
            "Range: {}:{} - {}:{}",
            location.range.start.line + 1,
            location.range.start.character + 1,
            location.range.end.line + 1,
            location.range.end.character + 1
        );
    }

    fn print_location_link(&self, link: &LocationLink) {
        println!("Target: {}", link.target_uri);
        println!(
            "Range: {}:{} - {}:{}",
            link.target_range.start.line + 1,
            link.target_range.start.character + 1,
            link.target_range.end.line + 1,
            link.target_range.end.character + 1
        );
        if let Some(origin) = &link.origin_selection_range {
            println!(
                "Origin: {}:{} - {}:{}",
                origin.start.line + 1,
                origin.start.character + 1,
                origin.end.line + 1,
                origin.end.character + 1
            );
        }
    }

    fn print_symbol(&self, symbol: &DocumentSymbol, indent: usize, hierarchical: bool) {
        let indent_str = "  ".repeat(indent);
        println!(
            "{}{} {} ({})",
            indent_str,
            match symbol.kind {
                SymbolKind::FILE => "📄",
                SymbolKind::MODULE => "📦",
                SymbolKind::NAMESPACE => "🏷️",
                SymbolKind::PACKAGE => "📦",
                SymbolKind::CLASS => "🏛️",
                SymbolKind::METHOD => "🔧",
                SymbolKind::PROPERTY => "📌",
                SymbolKind::FIELD => "🏷️",
                SymbolKind::CONSTRUCTOR => "🔨",
                SymbolKind::ENUM => "📋",
                SymbolKind::INTERFACE => "🔌",
                SymbolKind::FUNCTION => "⚡",
                SymbolKind::VARIABLE => "📦",
                SymbolKind::CONSTANT => "🔒",
                SymbolKind::STRING => "📝",
                SymbolKind::NUMBER => "🔢",
                SymbolKind::BOOLEAN => "☑️",
                SymbolKind::ARRAY => "📚",
                SymbolKind::OBJECT => "📦",
                SymbolKind::KEY => "🗝️",
                SymbolKind::NULL => "⭕",
                SymbolKind::ENUM_MEMBER => "📍",
                SymbolKind::STRUCT => "🏗️",
                SymbolKind::EVENT => "📢",
                SymbolKind::OPERATOR => "➕",
                SymbolKind::TYPE_PARAMETER => "🏷️",
                _ => "❓",
            },
            symbol.name,
            self.symbol_kind_to_string(symbol.kind)
        );

        if let Some(detail) = &symbol.detail {
            println!("{indent_str}  Detail: {detail}");
        }

        if hierarchical {
            if let Some(children) = &symbol.children {
                for child in children {
                    self.print_symbol(child, indent + 1, hierarchical);
                }
            }
        }
    }

    fn symbol_kind_to_string(&self, kind: SymbolKind) -> &'static str {
        match kind {
            SymbolKind::FILE => "File",
            SymbolKind::MODULE => "Module",
            SymbolKind::NAMESPACE => "Namespace",
            SymbolKind::PACKAGE => "Package",
            SymbolKind::CLASS => "Class",
            SymbolKind::METHOD => "Method",
            SymbolKind::PROPERTY => "Property",
            SymbolKind::FIELD => "Field",
            SymbolKind::CONSTRUCTOR => "Constructor",
            SymbolKind::ENUM => "Enum",
            SymbolKind::INTERFACE => "Interface",
            SymbolKind::FUNCTION => "Function",
            SymbolKind::VARIABLE => "Variable",
            SymbolKind::CONSTANT => "Constant",
            SymbolKind::STRING => "String",
            SymbolKind::NUMBER => "Number",
            SymbolKind::BOOLEAN => "Boolean",
            SymbolKind::ARRAY => "Array",
            SymbolKind::OBJECT => "Object",
            SymbolKind::KEY => "Key",
            SymbolKind::NULL => "Null",
            SymbolKind::ENUM_MEMBER => "EnumMember",
            SymbolKind::STRUCT => "Struct",
            SymbolKind::EVENT => "Event",
            SymbolKind::OPERATOR => "Operator",
            SymbolKind::TYPE_PARAMETER => "TypeParameter",
            _ => "Unknown",
        }
    }

    fn print_workspace_edit(&self, edit: &WorkspaceEdit) {
        if let Some(changes) = &edit.changes {
            println!("Changes in {} files:", changes.len());
            for (uri, edits) in changes {
                println!("\nFile: {uri}");
                println!("  {} edits", edits.len());
                for (i, edit) in edits.iter().enumerate() {
                    println!(
                        "  [{}] {}:{} - {}:{}",
                        i + 1,
                        edit.range.start.line + 1,
                        edit.range.start.character + 1,
                        edit.range.end.line + 1,
                        edit.range.end.character + 1
                    );
                }
            }
        }

        if let Some(document_changes) = &edit.document_changes {
            let count = match document_changes {
                DocumentChanges::Edits(edits) => edits.len(),
                DocumentChanges::Operations(ops) => ops.len(),
            };
            println!("Document changes: {count} operations");
        }

        if let Some(change_annotations) = &edit.change_annotations {
            println!("Change annotations: {}", change_annotations.len());
        }
    }

    fn analyze_symbol_dependencies(
        &self,
        analyzer: &LspCodeAnalyzer,
        symbol: &DocumentSymbol,
        uri: &Url,
    ) -> Result<()> {
        let info = analyzer.get_symbol_info(uri, symbol.selection_range.start)?;

        if !info.references.is_empty() {
            println!(
                "\nSymbol: {} ({})",
                symbol.name,
                self.symbol_kind_to_string(symbol.kind)
            );
            println!("  References: {}", info.references.len());

            // Group references by file
            let mut refs_by_file: HashMap<String, Vec<&Location>> = HashMap::new();
            for reference in &info.references {
                refs_by_file
                    .entry(reference.uri.to_string())
                    .or_default()
                    .push(reference);
            }

            for (file_uri, refs) in refs_by_file {
                if file_uri != uri.as_str() {
                    println!("    -> {} ({} references)", file_uri, refs.len());
                }
            }
        }

        // Analyze children recursively
        if let Some(children) = &symbol.children {
            for child in children {
                self.analyze_symbol_dependencies(analyzer, child, uri)?;
            }
        }

        Ok(())
    }
}
