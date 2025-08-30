use crate::cli::differential_indexer::DifferentialIndexer;
use crate::cli::git_diff::GitDiffDetector;
use crate::cli::storage::IndexStorage;
use crate::core::CodeGraph;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;
use std::time::Instant;
use tracing::{debug, info};

const DEFAULT_INDEX_PATH: &str = ".lsif-index.db";
const MAX_CHANGES_DISPLAY: usize = 15;

#[derive(Parser)]
#[command(name = "lsif")]
#[command(about = "AI-optimized code indexer with automatic differential updates")]
#[command(version)]
pub struct SimpleCli {
    /// Index database path (default: .lsif-index.db in current directory)
    #[arg(short = 'd', long, global = true)]
    pub db: Option<String>,

    /// Project root directory (default: current directory)
    #[arg(short = 'p', long, global = true)]
    pub project: Option<String>,

    /// Skip auto-index
    #[arg(long, global = true)]
    pub no_auto_index: bool,

    #[command(subcommand)]
    pub command: SimpleCommands,
}

#[derive(Subcommand)]
pub enum SimpleCommands {
    /// Go to definition (LSP: textDocument/definition)
    Definition {
        /// File path or file:line:column
        file: String,
        /// Line number (1-based, optional if included in file)
        line: Option<u32>,
        /// Column number (1-based, optional)
        column: Option<u32>,
    },

    /// Find all references (LSP: textDocument/references)
    References {
        /// File path or file:line:column
        file: String,
        /// Line number (1-based, optional if included in file)
        line: Option<u32>,
        /// Column number (1-based, optional)
        column: Option<u32>,
        /// Maximum depth for recursive search
        #[arg(long, default_value = "1")]
        depth: usize,
    },

    /// Show call hierarchy (LSP: textDocument/prepareCallHierarchy)
    CallHierarchy {
        /// Symbol name or file:line:column
        symbol: String,
        /// Maximum depth for hierarchy
        #[arg(short, long, default_value = "3")]
        depth: usize,
        /// Direction: incoming, outgoing, or both
        #[arg(short = 'D', long, default_value = "both")]
        direction: String,
    },

    /// Go to type definition (LSP: textDocument/typeDefinition)
    TypeDefinition {
        /// File path or file:line:column
        file: String,
        /// Line number (1-based, optional if included in file)
        line: Option<u32>,
        /// Column number (1-based, optional)
        column: Option<u32>,
        /// Maximum depth for type hierarchy
        #[arg(short, long, default_value = "2")]
        depth: usize,
    },

    /// Go to implementation (LSP: textDocument/implementation)
    Implementation {
        /// Type or interface name
        type_name: String,
        /// Maximum depth for implementation search
        #[arg(short, long, default_value = "2")]
        depth: usize,
    },

    /// Document symbols (LSP: textDocument/documentSymbol)
    Symbols {
        /// File path (optional, current directory if not specified)
        file: Option<String>,
        /// Filter by symbol kind (function, class, interface, etc.)
        #[arg(short, long)]
        kind: Option<String>,
    },

    /// Workspace symbols (LSP: workspace/symbol)
    WorkspaceSymbols {
        /// Query string
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "50")]
        limit: usize,
        /// Enable fuzzy search (supports partial matches, abbreviations)
        #[arg(short, long)]
        fuzzy: bool,
    },

    /// Graph query - Advanced Cypher-like queries
    Graph {
        /// Query pattern (Cypher syntax)
        /// Examples:
        /// - "MATCH (f:Function)-[:CALLS]->(g:Function) RETURN f,g"
        /// - "MATCH (c:Class)-[:IMPLEMENTS]->(i:Interface) RETURN c,i"
        /// - "MATCH (s:Symbol)-[:REFERENCES*1..3]->(t:Symbol) RETURN s,t"
        pattern: String,
        /// Maximum results
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// Maximum depth for path queries
        #[arg(short, long, default_value = "5")]
        depth: usize,
    },

    /// Find unused code (dead code detection)
    Unused {
        /// Show only public unused symbols
        #[arg(short, long)]
        public_only: bool,
    },

    /// Rebuild index
    Index {
        /// Force full reindex even if no changes
        #[arg(short, long)]
        force: bool,
        /// Show detailed progress
        #[arg(short, long)]
        verbose: bool,
    },

    /// Show graph diff - track related changes in the code graph
    Diff {
        /// Base commit or "HEAD" (optional, defaults to last indexed commit)
        #[arg(short, long)]
        base: Option<String>,
        /// Show only symbols related to changed files
        #[arg(short, long)]
        related: bool,
        /// Maximum depth for tracking related changes
        #[arg(short, long, default_value = "2")]
        depth: usize,
    },

    /// Show index status
    Status,

    /// Export index
    Export {
        /// Output format: lsif, json, graphml
        #[arg(short, long, default_value = "lsif")]
        format: String,
        /// Output file path
        output: String,
    },
}

/// VSCode形式 (file:line:column) をパース
fn parse_location(file: &str, line: Option<u32>, column: Option<u32>) -> (String, u32, u32) {
    // file:line:column 形式をチェック
    if file.contains(':') && line.is_none() {
        // 最後の2つのコロンで分割（ファイルパスにコロンが含まれる可能性を考慮）
        let parts: Vec<&str> = file.rsplitn(3, ':').collect();

        if parts.len() == 3 {
            // file:line:column形式
            // rsplitnは逆順なので、parts[0]がcolumn、parts[1]がline、parts[2]がfile
            if let (Ok(col), Ok(line)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                return (parts[2].to_string(), line, col);
            }
        } else if parts.len() == 2 {
            // file:line形式
            // parts[0]がline、parts[1]がfile
            if let Ok(line) = parts[0].parse::<u32>() {
                return (parts[1].to_string(), line, 1);
            }
        }
    }

    // 通常の引数形式
    (file.to_string(), line.unwrap_or(1), column.unwrap_or(1))
}

impl SimpleCli {
    pub fn execute(self) -> Result<()> {
        // デフォルト値の設定
        let db_path = self.db.unwrap_or_else(|| DEFAULT_INDEX_PATH.to_string());
        let project_root = self.project.unwrap_or_else(|| ".".to_string());

        // 自動インデックスの実行（--no-auto-indexフラグがない限り）
        if !self.no_auto_index {
            auto_index(&db_path, &project_root)?;
        }

        // コマンドの実行
        match self.command {
            SimpleCommands::Definition { file, line, column } => {
                let (file_path, line_num, col_num) = parse_location(&file, line, column);
                find_definition(&db_path, &file_path, line_num, col_num)?;
            }
            SimpleCommands::References {
                file,
                line,
                column,
                depth,
            } => {
                let (file_path, line_num, col_num) = parse_location(&file, line, column);
                find_references_recursive(&db_path, &file_path, line_num, col_num, depth)?;
            }
            SimpleCommands::CallHierarchy {
                symbol,
                depth,
                direction,
            } => {
                show_call_hierarchy(&db_path, &symbol, depth, &direction)?;
            }
            SimpleCommands::TypeDefinition {
                file,
                line,
                column,
                depth,
            } => {
                let (file_path, line_num, col_num) = parse_location(&file, line, column);
                find_type_definition(&db_path, &file_path, line_num, col_num, depth)?;
            }
            SimpleCommands::Implementation { type_name, depth } => {
                find_implementations(&db_path, &type_name, depth)?;
            }
            SimpleCommands::Symbols { file, kind } => {
                show_document_symbols(&db_path, file.as_deref(), kind.as_deref())?;
            }
            SimpleCommands::WorkspaceSymbols {
                query,
                limit,
                fuzzy,
            } => {
                search_workspace_symbols(&db_path, &query, limit, fuzzy)?;
            }
            SimpleCommands::Graph {
                pattern,
                limit,
                depth,
            } => {
                execute_graph_query(&db_path, &pattern, limit, depth)?;
            }
            SimpleCommands::Unused { public_only } => {
                show_unused_code(&db_path, public_only)?;
            }
            SimpleCommands::Index { force, verbose } => {
                rebuild_index(&db_path, &project_root, force, verbose)?;
            }
            SimpleCommands::Diff {
                base,
                related,
                depth,
            } => {
                show_graph_diff(&db_path, &project_root, base.as_deref(), related, depth)?;
            }
            SimpleCommands::Status => {
                show_status(&db_path, &project_root)?;
            }
            SimpleCommands::Export { format, output } => {
                export_index(&db_path, &format, &output)?;
            }
        }

        Ok(())
    }
}

/// 自動インデックス実行（変更検知と差分更新）
fn auto_index(db_path: &str, project_root: &str) -> Result<()> {
    let project_path = Path::new(project_root);

    // インデックスファイルが存在しない場合は初回インデックス
    if !Path::new(db_path).exists() {
        println!("🔍 Creating initial index...");
        let start = Instant::now();

        let mut indexer = DifferentialIndexer::new(db_path, project_path)?;
        let result = indexer.index_differential()?;

        // 初回は Modified として扱われるので、適切に表示
        let total_files = result.files_added + result.files_modified;
        let total_symbols = result.symbols_added + result.symbols_updated;

        println!(
            "✅ Initial index created in {:.2}s ({} files, {} symbols)",
            start.elapsed().as_secs_f64(),
            total_files,
            total_symbols
        );
        return Ok(());
    }

    // 既存インデックスの変更チェック（読み取り専用）
    let storage = IndexStorage::open_read_only(db_path)?;
    let metadata = storage.load_metadata()?;
    drop(storage); // 読み取り後すぐに解放

    if metadata.is_none() {
        info!("No metadata found, creating new index");
        let mut indexer = DifferentialIndexer::new(db_path, project_path)?;
        indexer.index_differential()?;
        return Ok(());
    }

    // Git差分検知
    let mut detector = GitDiffDetector::new(project_path)?;
    let all_changes = detector
        .detect_changes_since(metadata.as_ref().and_then(|m| m.git_commit_hash.as_deref()))?;

    // インデックス対象のファイルのみフィルタリング
    let changes: Vec<_> = all_changes
        .into_iter()
        .filter(|change| {
            let ext = change
                .path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx")
        })
        .collect();

    if changes.is_empty() {
        debug!("No indexable changes detected");
        return Ok(());
    }

    // インデックス対象の変更をコンソールに表示（最大15件）
    println!("⚡ Detected {} indexable changes:", changes.len());
    for (i, change) in changes.iter().take(MAX_CHANGES_DISPLAY).enumerate() {
        let status_symbol = match &change.status {
            crate::cli::git_diff::FileChangeStatus::Added => "➕",
            crate::cli::git_diff::FileChangeStatus::Modified => "📝",
            crate::cli::git_diff::FileChangeStatus::Deleted => "❌",
            crate::cli::git_diff::FileChangeStatus::Renamed { .. } => "✏️",
            crate::cli::git_diff::FileChangeStatus::Untracked => "🆕",
        };
        println!("  {} {} {}", i + 1, status_symbol, change.path.display());
    }
    if changes.len() > MAX_CHANGES_DISPLAY {
        println!("  ... and {} more", changes.len() - MAX_CHANGES_DISPLAY);
    }

    // 差分インデックス実行
    let start = Instant::now();
    let mut indexer = DifferentialIndexer::new(db_path, project_path)?;
    let result = indexer.index_differential()?;

    if result.files_modified > 0 || result.files_added > 0 || result.files_deleted > 0 {
        println!(
            "✅ Index updated in {:.2}s ({} modified, {} added, {} deleted)",
            start.elapsed().as_secs_f64(),
            result.files_modified,
            result.files_added,
            result.files_deleted
        );
    }

    Ok(())
}

/// 定義を検索
fn find_definition(db_path: &str, file: &str, line: u32, column: u32) -> Result<()> {
    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif reindex' first."))?;

    // まず指定位置にあるシンボルを探す
    let position = crate::core::Position {
        line: line - 1, // 0-based indexに変換
        character: column - 1,
    };

    // 最も近いシンボルを探す（行が一致するものを優先）
    let mut best_match: Option<&crate::core::Symbol> = None;
    for symbol in graph.get_all_symbols() {
        if symbol.file_path == file && symbol.range.start.line == position.line {
            // 同じ行にあるシンボルを優先
            best_match = Some(symbol);
            break;
        } else if symbol.file_path == file
            && symbol.range.start.line <= position.line
            && symbol.range.end.line >= position.line
        {
            // 範囲内にあるシンボル
            best_match = Some(symbol);
        }
    }

    if let Some(symbol) = best_match {
        println!("📍 Symbol found: {}", symbol.name);
        println!("   Type: {:?}", symbol.kind);
        println!(
            "   Location: {}:{}:{}",
            symbol.file_path,
            symbol.range.start.line + 1,
            symbol.range.start.character + 1
        );
        if let Some(doc) = &symbol.documentation {
            println!("   📖 {doc}");
        }
    } else {
        println!("❌ No symbol found at {file}:{line}:{column}");
    }

    Ok(())
}

/// 参照を再帰的に検索
fn find_references_recursive(
    db_path: &str,
    file: &str,
    line: u32,
    column: u32,
    _depth: usize,
) -> Result<()> {
    use crate::cli::reference_finder;
    use std::path::Path;

    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif reindex' first."))?;

    // メタデータからプロジェクトルートを取得
    let metadata = storage
        .load_metadata()?
        .ok_or_else(|| anyhow::anyhow!("No metadata found in index"))?;
    let project_root = Path::new(&metadata.project_root);

    // まず指定位置にあるシンボルを探す
    let position = crate::core::Position {
        line: line - 1, // 0-based indexに変換
        character: column - 1,
    };

    // 最も近いシンボルを探す（行が一致するものを優先）
    let mut target_symbol: Option<&crate::core::Symbol> = None;
    for symbol in graph.get_all_symbols() {
        if symbol.file_path == file && symbol.range.start.line == position.line {
            // 同じ行にあるシンボルを優先
            target_symbol = Some(symbol);
            break;
        } else if symbol.file_path == file
            && symbol.range.start.line <= position.line
            && symbol.range.end.line >= position.line
        {
            // 範囲内にあるシンボル
            target_symbol = Some(symbol);
        }
    }

    if let Some(symbol) = target_symbol {
        println!(
            "🔗 Finding references for '{}' ({:?})...",
            symbol.name, symbol.kind
        );

        // 実際のファイル内容を検索して参照を見つける
        let references =
            reference_finder::find_all_references(project_root, &symbol.name, &symbol.kind)?;

        if references.is_empty() {
            println!("No references found for '{}'", symbol.name);
        } else {
            // 定義と使用を分けてカウント
            let definitions: Vec<_> = references.iter().filter(|r| r.is_definition).collect();
            let usages: Vec<_> = references.iter().filter(|r| !r.is_definition).collect();

            println!(
                "Found {} references ({} definitions, {} usages):",
                references.len(),
                definitions.len(),
                usages.len()
            );

            // 定義を表示
            if !definitions.is_empty() {
                println!("\n📝 Definitions:");
                for (i, ref_item) in definitions.iter().take(MAX_CHANGES_DISPLAY).enumerate() {
                    println!(
                        "  {} {} at {}:{}:{}",
                        i + 1,
                        ref_item.symbol.name,
                        ref_item.symbol.file_path,
                        ref_item.symbol.range.start.line + 1,
                        ref_item.symbol.range.start.character + 1
                    );
                }
            }

            // 使用箇所を表示
            if !usages.is_empty() {
                println!("\n🔍 Usages:");
                for (i, ref_item) in usages.iter().take(MAX_CHANGES_DISPLAY).enumerate() {
                    println!(
                        "  {} {} at {}:{}:{}",
                        i + 1,
                        ref_item.symbol.name,
                        ref_item.symbol.file_path,
                        ref_item.symbol.range.start.line + 1,
                        ref_item.symbol.range.start.character + 1
                    );
                }
                if usages.len() > MAX_CHANGES_DISPLAY {
                    println!("  ... and {} more", usages.len() - MAX_CHANGES_DISPLAY);
                }
            }
        }
    } else {
        println!("❌ No symbol found at {file}:{line}:{column}");
    }

    Ok(())
}

/// コールヒエラルキーを表示
fn show_call_hierarchy(
    db_path: &str,
    symbol: &str,
    max_depth: usize,
    direction: &str,
) -> Result<()> {
    use crate::cli::call_hierarchy_cmd;

    let dir_symbol = match direction {
        "incoming" => "⬅️",
        "outgoing" => "➡️",
        _ => "↔️",
    };

    println!("{dir_symbol} Call hierarchy for '{symbol}' ({direction})");
    call_hierarchy_cmd::show_call_hierarchy(db_path, symbol, direction, max_depth)?;

    Ok(())
}

/// 型情報を表示
#[allow(dead_code)]
fn show_type_info(db_path: &str, type_name: &str, show_hierarchy: bool) -> Result<()> {
    use crate::core::TypeRelationsAnalyzer;

    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif reindex' first."))?;

    let analyzer = TypeRelationsAnalyzer::new(&graph);

    if show_hierarchy {
        let hierarchy = analyzer.find_type_hierarchy(type_name);

        println!("🔺 Type hierarchy for '{type_name}':");
        if !hierarchy.parents.is_empty() {
            println!("  Parents:");
            for p in hierarchy.parents.iter().take(5) {
                println!("    - {}", p.name);
            }
        }
        if !hierarchy.children.is_empty() {
            println!("  Children:");
            for c in hierarchy.children.iter().take(5) {
                println!("    - {}", c.name);
            }
        }
    } else if let Some(_relations) = analyzer.collect_type_relations(type_name, 3) {
        println!("🔷 Type relations for '{type_name}':");
        // Note: TypeRelations struct fields may vary
        println!("  Relations found");
    } else {
        println!("❌ Type '{type_name}' not found");
    }

    Ok(())
}

/// グラフクエリを実行（拡張版）
fn execute_graph_query(db_path: &str, pattern: &str, limit: usize, _depth: usize) -> Result<()> {
    use crate::core::{QueryEngine, QueryParser};

    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif reindex' first."))?;

    let query_pattern =
        QueryParser::parse(pattern).map_err(|e| anyhow::anyhow!("Failed to parse query: {}", e))?;

    let engine = QueryEngine::new(&graph);
    let results = engine.execute(&query_pattern);

    if results.matches.is_empty() {
        println!("❌ No matches found for pattern: {pattern}");
    } else {
        println!("🔍 Found {} matches:", results.matches.len());
        for (i, match_result) in results.matches.iter().take(limit).enumerate() {
            println!("  Match #{}:", i + 1);
            for (var_name, symbol) in &match_result.bindings {
                println!("    {} = {} ({:?})", var_name, symbol.name, symbol.kind);
            }
        }
        if results.matches.len() > limit {
            println!(
                "  ... {} more matches (use --limit to see more)",
                results.matches.len() - limit
            );
        }
    }

    Ok(())
}

/// グラフ上の差分を表示
fn show_graph_diff(
    db_path: &str,
    project_root: &str,
    base: Option<&str>,
    related: bool,
    depth: usize,
) -> Result<()> {
    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif index' first."))?;

    let metadata = storage.load_metadata()?;

    // Git差分を取得
    let mut detector = GitDiffDetector::new(project_root)?;
    let base_commit = base.or(metadata.as_ref().and_then(|m| m.git_commit_hash.as_deref()));
    let changes = detector.detect_changes_since(base_commit)?;

    // インデックス対象のファイルのみフィルタリング
    let indexable_changes: Vec<_> = changes
        .into_iter()
        .filter(|change| {
            let ext = change
                .path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx")
        })
        .collect();

    if indexable_changes.is_empty() {
        println!("📊 No changes detected");
        return Ok(());
    }

    println!("📊 Graph diff (depth: {depth}):");
    println!("  Base: {}", base_commit.unwrap_or("initial"));
    println!("  Changes: {} files", indexable_changes.len());
    println!();

    // 変更されたファイルのシンボルを収集
    let mut changed_symbols = Vec::new();
    for change in &indexable_changes {
        let path_str = change.path.to_string_lossy();
        for symbol in graph.get_all_symbols() {
            if symbol.file_path == path_str {
                changed_symbols.push(symbol);
            }
        }
    }

    println!("🔄 Changed symbols: {}", changed_symbols.len());
    for (i, symbol) in changed_symbols.iter().take(10).enumerate() {
        println!(
            "  {} {} ({:?}) at {}:{}:{}",
            i + 1,
            symbol.name,
            symbol.kind,
            symbol.file_path,
            symbol.range.start.line + 1,
            symbol.range.start.character + 1
        );
    }
    if changed_symbols.len() > 10 {
        println!("  ... and {} more", changed_symbols.len() - 10);
    }

    if related && depth > 0 {
        println!();
        println!("🔗 Related symbols (depth {depth}):");

        // 関連するシンボルを探す（同じ名前のシンボル = 簡易版）
        let mut related_symbols = std::collections::HashSet::new();
        for symbol in &changed_symbols {
            for other in graph.get_all_symbols() {
                if other.name == symbol.name && other.id != symbol.id {
                    related_symbols.insert((other.name.clone(), other.file_path.clone()));
                }
            }
        }

        for (i, (name, path)) in related_symbols.iter().take(15).enumerate() {
            println!("  {} {} in {}", i + 1, name, path);
        }
        if related_symbols.len() > 15 {
            println!("  ... and {} more", related_symbols.len() - 15);
        }
    }

    Ok(())
}

/// インデックス状態を表示
fn show_status(db_path: &str, project_root: &str) -> Result<()> {
    if !Path::new(db_path).exists() {
        println!("❌ No index found at {db_path}");
        println!("   Run any command to create an initial index");
        return Ok(());
    }

    let storage = IndexStorage::open_read_only(db_path)?;
    let metadata = storage.load_metadata()?;

    if let Some(meta) = metadata {
        println!("📊 Index Status:");
        println!("  Database: {db_path}");
        println!("  Project: {project_root}");
        println!("  Created: {}", meta.created_at.format("%Y-%m-%d %H:%M:%S"));
        println!("  Files: {}", meta.files_count);
        println!("  Symbols: {}", meta.symbols_count);

        if let Some(commit) = &meta.git_commit_hash {
            println!("  Git commit: {}", &commit[..8.min(commit.len())]);
        }

        // 変更チェック
        let mut detector = GitDiffDetector::new(project_root)?;
        let changes = detector.detect_changes_since(meta.git_commit_hash.as_deref())?;

        if changes.is_empty() {
            println!("  Status: ✅ Up to date");
        } else {
            println!("  Status: ⚠️  {} pending changes", changes.len());
            println!("  Run any query command to auto-update");
        }
    } else {
        println!("⚠️  Index exists but no metadata found");
    }

    // ディスク使用量
    if let Ok(file_meta) = std::fs::metadata(db_path) {
        let size_mb = file_meta.len() as f64 / (1024.0 * 1024.0);
        println!("  Disk usage: {size_mb:.2} MB");
    }

    Ok(())
}

/// インデックスをエクスポート
fn export_index(db_path: &str, format: &str, output: &str) -> Result<()> {
    use crate::core::generate_lsif;

    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif index' first."))?;

    match format {
        "lsif" => {
            println!("📦 Exporting to LSIF format...");
            let lsif_content = generate_lsif(graph)?;
            std::fs::write(output, &lsif_content)?;
        }
        "json" => {
            println!("📦 Exporting to JSON format...");
            let json_content = serde_json::to_string_pretty(&graph)?;
            std::fs::write(output, &json_content)?;
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported format: {}. Use 'lsif' or 'json'",
                format
            ));
        }
    }

    println!("✅ Exported to {output}");

    Ok(())
}

/// 型定義を検索
fn find_type_definition(
    db_path: &str,
    file: &str,
    line: u32,
    column: u32,
    depth: usize,
) -> Result<()> {
    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif index' first."))?;

    let symbol_id = format!("{file}#{line}:{column}");

    println!("🔷 Type definition for {file}:{line}:{column} (depth: {depth})");

    // Find the symbol and its type
    if let Some(symbol) = graph.find_symbol(&symbol_id) {
        // Note: type_ref field might not exist in current Symbol struct
        // This is a simplified version
        println!("  Symbol: {}", symbol.name);
        println!("  Kind: {:?}", symbol.kind);
        if let Some(doc) = &symbol.documentation {
            println!("  Documentation: {doc}");
        }
    } else {
        println!("❌ No symbol found at {file}:{line}:{column}");
    }

    Ok(())
}

/// 実装を検索
fn find_implementations(db_path: &str, type_name: &str, depth: usize) -> Result<()> {
    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif index' first."))?;

    println!("🔨 Implementations of '{type_name}' (depth: {depth})");

    // Find all implementations
    // Note: implements field might not exist in current Symbol struct
    // Using name matching as a workaround
    let mut implementations = Vec::new();
    for symbol in graph.get_all_symbols() {
        // Check if symbol name contains "impl" and the type name
        if symbol.name.contains("impl") && symbol.name.contains(type_name) {
            implementations.push(symbol);
        }
    }

    if implementations.is_empty() {
        println!("  No implementations found");
    } else {
        println!("  Found {} implementations:", implementations.len());
        for (i, impl_symbol) in implementations.iter().take(10).enumerate() {
            println!(
                "  {} {} at {}:{}:{}",
                i + 1,
                impl_symbol.name,
                impl_symbol.file_path,
                impl_symbol.range.start.line + 1,
                impl_symbol.range.start.character + 1
            );
        }
        if implementations.len() > 10 {
            println!("  ... and {} more", implementations.len() - 10);
        }
    }

    Ok(())
}

/// ドキュメントシンボルを表示
fn show_document_symbols(db_path: &str, file: Option<&str>, kind: Option<&str>) -> Result<()> {
    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif index' first."))?;

    let target_file = file.unwrap_or(".");
    println!("📄 Document symbols in '{target_file}'");

    let mut symbols: Vec<_> = graph
        .get_all_symbols()
        .filter(|s| file.is_none() || s.file_path.contains(target_file))
        .collect();

    // Filter by kind if specified
    if let Some(kind_filter) = kind {
        symbols.retain(|s| {
            format!("{:?}", s.kind)
                .to_lowercase()
                .contains(&kind_filter.to_lowercase())
        });
    }

    if symbols.is_empty() {
        println!("  No symbols found");
    } else {
        println!("  Found {} symbols:", symbols.len());

        // Group by file
        let mut by_file: std::collections::HashMap<&str, Vec<&crate::core::Symbol>> =
            std::collections::HashMap::new();
        for symbol in symbols.iter() {
            by_file.entry(&symbol.file_path).or_default().push(symbol);
        }

        for (file_path, file_symbols) in by_file.iter().take(5) {
            println!("\n  {file_path}:");
            for symbol in file_symbols.iter().take(10) {
                println!("    {:?} {}", symbol.kind, symbol.name);
            }
            if file_symbols.len() > 10 {
                println!("    ... and {} more", file_symbols.len() - 10);
            }
        }

        if by_file.len() > 5 {
            println!("\n  ... and {} more files", by_file.len() - 5);
        }
    }

    Ok(())
}

/// ワークスペースシンボルを検索
fn search_workspace_symbols(db_path: &str, query: &str, limit: usize, fuzzy: bool) -> Result<()> {
    use crate::cli::fuzzy_search::{fuzzy_search, needs_fuzzy_search};

    let storage = IndexStorage::open_read_only(db_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No index found. Run 'lsif index' first."))?;

    if fuzzy {
        println!("🔍 Fuzzy searching workspace for '{query}'");

        // Collect all symbols for fuzzy search
        let all_symbols: Vec<_> = graph.get_all_symbols().cloned().collect();
        let matches = fuzzy_search(query, &all_symbols);

        if matches.is_empty() {
            println!("  No symbols found matching '{query}'");
            println!("  💡 Tips:");
            println!("     - Try partial matches: 'relat' for 'RelationshipPattern'");
            println!("     - Use lowercase: 'rp' matches 'JsonRpcRequest'");
            println!("     - Abbreviations work best with capitals: 'RP' for names with R and P");
        } else {
            let limited = matches.into_iter().take(limit).collect::<Vec<_>>();
            println!(
                "  Found {} symbols (showing top {}):",
                limited.len(),
                limited.len()
            );

            for (i, fuzzy_match) in limited.iter().enumerate() {
                let symbol = fuzzy_match.symbol;
                println!(
                    "  {} {:?} {} at {}:{}:{} (score: {:.2})",
                    i + 1,
                    symbol.kind,
                    symbol.name,
                    symbol.file_path,
                    symbol.range.start.line + 1,
                    symbol.range.start.character + 1,
                    fuzzy_match.score
                );
            }
        }
    } else {
        println!("🔍 Searching workspace for '{query}'");

        let query_lower = query.to_lowercase();
        let matches: Vec<_> = graph
            .get_all_symbols()
            .filter(|s| s.name.to_lowercase().contains(&query_lower))
            .take(limit)
            .collect();

        if matches.is_empty() {
            println!("  No symbols found matching '{query}'");

            // 曖昧検索を提案
            if needs_fuzzy_search(query, 0) {
                println!("\n  💡 Try fuzzy search: lsif workspace-symbols {query} --fuzzy");

                // プレビューとして少し表示
                let all_symbols: Vec<_> = graph.get_all_symbols().cloned().collect();
                let fuzzy_matches = fuzzy_search(query, &all_symbols);
                if !fuzzy_matches.is_empty() {
                    println!(
                        "     Would find {} symbols like:",
                        fuzzy_matches.len().min(3)
                    );
                    for m in fuzzy_matches.iter().take(3) {
                        println!("       - {}", m.symbol.name);
                    }
                }
            }
        } else {
            println!("  Found {} symbols:", matches.len());
            for (i, symbol) in matches.iter().enumerate() {
                println!(
                    "  {} {:?} {} at {}:{}:{}",
                    i + 1,
                    symbol.kind,
                    symbol.name,
                    symbol.file_path,
                    symbol.range.start.line + 1,
                    symbol.range.start.character + 1
                );
            }

            // 結果が少ない場合は曖昧検索も提案
            if needs_fuzzy_search(query, matches.len()) {
                println!("\n  💡 For more results, try: lsif workspace-symbols {query} --fuzzy");
            }
        }
    }

    Ok(())
}

/// verboseオプション付き
fn rebuild_index(db_path: &str, project_root: &str, force: bool, verbose: bool) -> Result<()> {
    let project_path = Path::new(project_root);
    let start = Instant::now();

    let mut indexer = DifferentialIndexer::new(db_path, project_path)?;

    if verbose {
        println!("🔍 Starting index rebuild...");
        println!("  Database: {db_path}");
        println!("  Project: {project_root}");
    }

    let result = if force {
        if verbose {
            println!("  Mode: Full reindex (forced)");
        }
        indexer.full_reindex()?
    } else {
        if verbose {
            println!("  Mode: Differential index");
        }
        indexer.index_differential()?
    };

    let elapsed = start.elapsed();

    println!("✅ Index rebuilt in {:.2}s:", elapsed.as_secs_f64());
    println!(
        "  Files: +{} ~{} -{}",
        result.files_added, result.files_modified, result.files_deleted
    );
    println!(
        "  Symbols: +{} ~{} -{}",
        result.symbols_added, result.symbols_updated, result.symbols_deleted
    );

    if verbose {
        println!("\n📊 Performance metrics:");
        println!(
            "  Files/sec: {:.1}",
            (result.files_added + result.files_modified) as f64 / elapsed.as_secs_f64()
        );
        println!(
            "  Symbols/sec: {:.1}",
            (result.symbols_added + result.symbols_updated) as f64 / elapsed.as_secs_f64()
        );
    }

    Ok(())
}

/// public_onlyオプション付き
fn show_unused_code(_db_path: &str, _public_only: bool) -> Result<()> {
    // Dead code detection needs reimplementation with enhanced graph
    println!("Dead code detection is currently being refactored.");
    Ok(())
}
