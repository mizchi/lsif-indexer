use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn, error};

use crate::adaptive_parallel::{AdaptiveParallelConfig, AdaptiveIncrementalProcessor};
use crate::git_diff::{FileChange, FileChangeStatus, GitDiffDetector};
use crate::reference_finder;
use crate::storage::IndexStorage;
use lsif_core::{CodeGraph, Symbol, SymbolKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir;
use indicatif::{ProgressBar, ProgressStyle};

// LSP統合のためのインポート
use lsp::lsp_indexer::LspIndexer;
use lsp::language_detector::detect_project_language;
use lsp::lsp_pool::{LspClientPool, PoolConfig};

/// 差分インデックスのメタデータ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifferentialIndexMetadata {
    /// 最後のインデックス時刻
    pub last_indexed_at: DateTime<Utc>,
    /// 最後のコミットSHA（Git管理の場合）
    pub last_commit: Option<String>,
    /// インデックス済みファイル数
    pub indexed_files: usize,
    /// 総シンボル数
    pub total_symbols: usize,
    /// ファイルハッシュキャッシュのパス
    pub hash_cache_path: Option<PathBuf>,
    /// ファイルごとのコンテンツハッシュ（xxHash3）
    pub file_content_hashes: HashMap<PathBuf, String>,
}

/// 差分インデックス結果
#[derive(Debug, Clone)]
pub struct DifferentialIndexResult {
    /// 追加されたファイル数
    pub files_added: usize,
    /// 更新されたファイル数
    pub files_modified: usize,
    /// 削除されたファイル数
    pub files_deleted: usize,
    /// 追加されたシンボル数
    pub symbols_added: usize,
    /// 更新されたシンボル数
    pub symbols_updated: usize,
    /// 削除されたシンボル数
    pub symbols_deleted: usize,
    /// 処理時間
    pub duration: Duration,
    /// 追加されたシンボルのサマリー
    pub added_symbols: Vec<SymbolSummary>,
    /// 削除されたシンボルのサマリー
    pub deleted_symbols: Vec<SymbolSummary>,
    /// フルインデックスが実行されたか
    pub full_reindex: bool,
    /// 差分率（変更ファイル数 / 全ファイル数）
    pub change_ratio: f64,
}

/// シンボルのサマリー情報
#[derive(Debug, Clone)]
pub struct SymbolSummary {
    /// シンボル名
    pub name: String,
    /// シンボルの種類
    pub kind: SymbolKind,
    /// ファイルパス
    pub file_path: String,
    /// 行番号
    pub line: u32,
}


/// 差分インデクサー
pub struct DifferentialIndexer {
    storage: IndexStorage,
    git_detector: GitDiffDetector,
    project_root: PathBuf,
    metadata: Option<DifferentialIndexMetadata>,
    #[allow(dead_code)] // 将来の並列処理拡張用
    parallel_processor: AdaptiveIncrementalProcessor,
    /// LSPインデクサー
    lsp_indexer: Option<LspIndexer>,
    /// LSPクライアントプール
    lsp_pool: LspClientPool,
    /// フォールバックインデクサーのみを使用するかどうか
    fallback_only: bool,
}

impl DifferentialIndexer {
    /// 新しい差分インデクサーを作成
    pub fn new<P1: AsRef<Path>, P2: AsRef<Path>>(
        storage_path: P1,
        project_root: P2,
    ) -> Result<Self> {
        let storage = IndexStorage::open(&storage_path)?;
        let git_detector = GitDiffDetector::new(&project_root)?;
        let project_root = project_root.as_ref().to_path_buf();

        // メタデータを読み込み
        let metadata =
            storage.load_data::<DifferentialIndexMetadata>("__differential_metadata__")?;

        // 適応的並列処理の設定
        let parallel_config = AdaptiveParallelConfig::default();
        let parallel_processor = AdaptiveIncrementalProcessor::new(parallel_config)?;

        // LSPプールの設定（短いタイムアウトでフォールバックを早期実行）
        let pool_config = PoolConfig {
            max_idle_time: std::time::Duration::from_secs(300),
            init_timeout: std::time::Duration::from_secs(2),  // 2秒に短縮（さらに高速化）
            request_timeout: std::time::Duration::from_secs(1), // 1秒に短縮（さらに高速化）
            max_retries: 1,  // リトライを1回に削減
        };
        let lsp_pool = LspClientPool::new(pool_config);

        Ok(Self {
            storage,
            git_detector,
            project_root,
            metadata,
            parallel_processor,
            lsp_indexer: None,
            lsp_pool,
            fallback_only: false,
        })
    }

    /// フォールバックインデクサーのみを使用するモードを設定
    pub fn set_fallback_only(&mut self, fallback_only: bool) {
        self.fallback_only = fallback_only;
        if fallback_only {
            info!("Using fallback indexer only mode (faster but less accurate)");
            eprintln!("ℹ️  Using fallback indexer only mode (faster but less accurate)");
        }
    }
    
    /// 並列処理の設定
    pub fn set_parallel_config(&mut self, threads: usize, parallel: bool) -> Result<()> {
        use crate::adaptive_parallel::AdaptiveParallelConfig;
        
        let mut config = AdaptiveParallelConfig::default();
        
        // 並列処理を有効化
        if parallel {
            config.parallel_threshold = 1; // 即座に並列処理を開始
        }
        
        // スレッド数の設定
        if threads > 0 {
            config.max_threads = threads;
        } else if parallel {
            // 自動設定（CPU数に基づく）
            config.max_threads = 0; // 0 = auto
        }
        
        // チャンクサイズの調整
        if threads > 8 {
            config.chunk_size = 50; // 多スレッド時は小さめのチャンク
        }
        
        self.parallel_processor = AdaptiveIncrementalProcessor::new(config)?;
        
        info!("Parallel processing configured: threads={}, enabled={}", 
              if threads == 0 { num_cpus::get() } else { threads }, 
              parallel);
              
        Ok(())
    }

    /// LSPインデクサーを初期化（遅延初期化）
    #[allow(dead_code)]
    fn ensure_lsp_indexer(&mut self) -> Result<()> {
        if self.lsp_indexer.is_none() {
            info!("Initializing LSP indexer for project: {}", self.project_root.display());
            
            // プロジェクトの言語を検出
            let language = detect_project_language(&self.project_root);
            debug!("Detected project language: {:?}", language);
            
            // LSPインデクサーを作成（単純にnewで作成される）
            let indexer = LspIndexer::new(self.project_root.to_string_lossy().to_string());
            info!("LSP indexer initialized successfully");
            self.lsp_indexer = Some(indexer);
        }
        Ok(())
    }

    /// ファイルからシンボルを抽出（LSPモード）
    fn extract_symbols_with_lsp(&mut self, path: &Path) -> Result<Vec<Symbol>> {
        use std::fs::canonicalize;
        use std::time::Instant;
        
        info!("Attempting LSP extraction for: {}", path.display());
        
        let start = Instant::now();
        
        // ファイルの絶対パスを取得
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            match canonicalize(path) {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to canonicalize path {}: {}, using fallback", path.display(), e);
                    return self.extract_symbols_with_fallback(path);
                }
            }
        };
        
        let file_uri = format!("file://{}", absolute_path.display());
        debug!("File URI: {}", file_uri);
        
        // 言語IDを取得
        use lsp::adapter::lsp::get_language_id;
        let language_id = get_language_id(path).unwrap_or_else(|| "unknown".to_string());
        
        // LSPがドキュメントシンボルをサポートしているかチェック
        if !self.lsp_pool.has_capability_for_language(&language_id, "textDocument/documentSymbol") {
            debug!("LSP server for {} does not support documentSymbol, using fallback", language_id);
            return self.extract_symbols_with_fallback(path);
        }
        
        // LSPプールからクライアントを取得（短いタイムアウト）
        match self.lsp_pool.get_or_create_client(path, &self.project_root) {
            Ok(client_arc) => {
                debug!("Successfully got LSP client from pool");
                // クライアントをロックして使用
                match client_arc.lock() {
                    Ok(mut client) => {
                        debug!("Successfully locked LSP client, requesting symbols");
                        // ドキュメントシンボルを取得
                        match client.get_document_symbols(&file_uri) {
                            Ok(lsp_symbols) => {
                                info!("LSP extracted {} symbols from {} in {:?}", 
                                       lsp_symbols.len(), path.display(), start.elapsed());
                                // LSPシンボルをコアのSymbol型に変換
                                let symbols = self.convert_lsp_symbols_to_core(&lsp_symbols, path);
                                debug!("Converted {} LSP symbols to core symbols", symbols.len());
                                Ok(symbols)
                            }
                            Err(e) => {
                                // エラーメッセージにサポートされていないという情報が含まれている場合
                                if e.to_string().contains("does not support") {
                                    info!("LSP server does not support documentSymbol for {}, using fallback", path.display());
                                } else {
                                    warn!("LSP symbol extraction failed for {}: {}. Using fallback.", path.display(), e);
                                }
                                self.extract_symbols_with_fallback(path)
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to lock LSP client for {}: {}. Using fallback.", path.display(), e);
                        self.extract_symbols_with_fallback(path)
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get LSP client from pool for {}: {}. Using fallback.", path.display(), e);
                self.extract_symbols_with_fallback(path)
            }
        }
    }

    /// ファイルからシンボルを抽出（フォールバック）
    fn extract_symbols_with_fallback(&self, path: &Path) -> Result<Vec<Symbol>> {
        use lsp::fallback_indexer::FallbackIndexer;
        
        info!("Using fallback indexer for: {}", path.display());
        
        if let Some(fallback) = FallbackIndexer::from_extension(path) {
            debug!("Fallback indexer found for extension: {:?}", path.extension());
            
            match fallback.extract_symbols(path) {
                Ok(lsp_symbols) => {
                    info!("Fallback indexer extracted {} symbols from {}", 
                           lsp_symbols.len(), path.display());
                    
                    // LSPシンボルをコアのSymbol型に変換
                    let symbols = self.convert_lsp_symbols_to_core(&lsp_symbols, path);
                    debug!("Converted {} fallback symbols to core symbols", symbols.len());
                    
                    // 各シンボルをログ出力
                    for symbol in &symbols {
                        debug!("  - {} ({:?}) at {}:{}", 
                               symbol.name, symbol.kind, 
                               symbol.range.start.line + 1, symbol.range.start.character + 1);
                    }
                    
                    Ok(symbols)
                }
                Err(e) => {
                    warn!("Fallback indexer failed for {}: {}", path.display(), e);
                    Ok(Vec::new())
                }
            }
        } else {
            // フォールバックも使えない場合は空のリストを返す
            warn!("No indexer available for file: {} (extension: {:?})", 
                  path.display(), path.extension());
            Ok(Vec::new())
        }
    }
    
    /// LSPのDocumentSymbolをコアのSymbol型に変換
    fn convert_lsp_symbols_to_core(&self, lsp_symbols: &[lsp_types::DocumentSymbol], path: &Path) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        let path_str = path.to_string_lossy().to_string();
        
        for lsp_symbol in lsp_symbols {
            let symbol = Symbol {
                id: format!("{}#{}:{}", path_str, lsp_symbol.range.start.line + 1, lsp_symbol.name),
                kind: self.convert_lsp_symbol_kind(lsp_symbol.kind),
                name: lsp_symbol.name.clone(),
                file_path: path_str.clone(),
                range: lsif_core::Range {
                    start: lsif_core::Position {
                        line: lsp_symbol.range.start.line,
                        character: lsp_symbol.range.start.character,
                    },
                    end: lsif_core::Position {
                        line: lsp_symbol.range.end.line,
                        character: lsp_symbol.range.end.character,
                    },
                },
                documentation: lsp_symbol.detail.clone(),
            detail: None,
            };
            symbols.push(symbol);
            
            // 子シンボルも処理
            if let Some(children) = &lsp_symbol.children {
                symbols.extend(self.convert_lsp_symbols_to_core(children, path));
            }
        }
        
        symbols
    }
    
    /// LSPのSymbolKindをコアのSymbolKindに変換
    fn convert_lsp_symbol_kind(&self, lsp_kind: lsp_types::SymbolKind) -> SymbolKind {
        use lsp_types::SymbolKind as LspKind;
        
        match lsp_kind {
            LspKind::FUNCTION | LspKind::METHOD => SymbolKind::Function,
            LspKind::CLASS | LspKind::STRUCT | LspKind::INTERFACE => SymbolKind::Class,
            LspKind::MODULE | LspKind::NAMESPACE => SymbolKind::Module,
            LspKind::VARIABLE | LspKind::CONSTANT | LspKind::PROPERTY | LspKind::FIELD => SymbolKind::Variable,
            LspKind::ENUM | LspKind::ENUM_MEMBER => SymbolKind::Enum,
            _ => SymbolKind::Unknown,
        }
    }

    /// 差分インデックスを実行
    pub fn index_differential(&mut self) -> Result<DifferentialIndexResult> {
        let start = Instant::now();
        info!("Starting differential indexing...");
        debug!("Project root: {}", self.project_root.display());
        
        // LSPモードの場合、使用される言語を検出してLSPを事前起動
        if !self.fallback_only {
            self.warm_up_lsp_clients()?;
        }

        // 前回のメタデータからハッシュキャッシュを復元
        if let Some(ref metadata) = self.metadata {
            // ファイルコンテンツハッシュをGitDetectorに設定
            for (path, hash) in &metadata.file_content_hashes {
                self.git_detector.set_cached_hash(path.clone(), hash.clone());
            }
            info!("Restored {} file hashes from metadata", metadata.file_content_hashes.len());
            
            // ハッシュキャッシュファイルも読み込み（存在する場合）
            if let Some(ref cache_path) = metadata.hash_cache_path {
                if cache_path.exists() {
                    self.git_detector.load_hash_cache(cache_path).ok();
                }
            }
        }

        // 前回のコミットを取得
        let last_commit = self
            .metadata
            .as_ref()
            .and_then(|m| m.last_commit.as_deref());

        // 全ファイル数を取得（差分率計算用）
        let total_file_count = self.count_total_files()?;
        
        // 変更ファイルを検出（初回の場合は全ファイル）
        let changes = if self.metadata.is_none() {
            info!("Initial indexing - scanning all files");
            let files = self.scan_all_files()?;
            info!("scan_all_files returned {} files", files.len());
            files
        } else {
            self.git_detector.detect_changes_since(last_commit)?
        };
        
        let change_count = changes.len();
        let change_ratio = if total_file_count > 0 {
            change_count as f64 / total_file_count as f64
        } else {
            0.0
        };
        
        info!("Detected {} file changes (total files: {}, change ratio: {:.1}%)", 
              change_count, total_file_count, change_ratio * 100.0);
        
        // 差分が50%以上の場合はフルインデックスを実行
        let (changes, full_reindex) = if change_ratio >= 0.5 && self.metadata.is_some() {
            warn!("Change ratio {:.1}% >= 50%, performing full reindex", change_ratio * 100.0);
            eprintln!("⚠️  Large number of changes detected ({:.1}%), performing full reindex...", change_ratio * 100.0);
            (self.scan_all_files()?, true)
        } else {
            (changes, false)
        };
        
        let total_files = changes.len();
        
        // プログレスバーの設定
        let progress_bar = if total_files > 0 {
            let pb = ProgressBar::new(total_files as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
                    .unwrap()
                    .progress_chars("#>-")
            );
            pb.set_message("Indexing files...");
            Some(pb)
        } else {
            None
        };

        let mut result = DifferentialIndexResult {
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            symbols_added: 0,
            symbols_updated: 0,
            symbols_deleted: 0,
            duration: Duration::from_secs(0),
            added_symbols: Vec::new(),
            deleted_symbols: Vec::new(),
            full_reindex,
            change_ratio,
        };

        // 既存のCodeGraphを読み込むか新規作成
        let mut graph = self
            .storage
            .load_data::<CodeGraph>("graph")?
            .unwrap_or_else(CodeGraph::new);

        // ファイルごとに処理（並列処理対応）
        let mut new_file_hashes = HashMap::new();
        let mut processed_count = 0;
        
        // 並列処理の閾値を確認（デフォルトは30ファイル以上で並列化）
        let should_parallel = total_files >= self.parallel_processor.config.parallel_threshold;
        
        // 並列処理を使用（フォールバックモードの場合のみ）
        if should_parallel && self.fallback_only {
            // 並列処理モード
            use rayon::prelude::*;
            
            use lsp::fallback_indexer::FallbackIndexer;
            
            eprintln!("⚡ Using parallel processing for {} files", total_files);
            
            // シンボル抽出を並列化
            let symbols_results: Vec<(PathBuf, FileChangeStatus, Option<Vec<Symbol>>)> = changes
                .par_iter()
                .map(|change| {
                    let symbols = match &change.status {
                        FileChangeStatus::Added | FileChangeStatus::Modified | FileChangeStatus::Renamed { .. } | FileChangeStatus::Untracked => {
                            // ファイルごとにフォールバックインデクサーを作成
                            match FallbackIndexer::from_extension(&change.path) {
                                Some(indexer) => {
                                    match indexer.extract_symbols(&change.path) {
                                        Ok(doc_symbols) => {
                                            // DocumentSymbolをSymbolに変換
                                            let file_uri = format!("file://{}", change.path.display());
                                            let symbols: Vec<Symbol> = doc_symbols.into_iter().map(|doc_sym| {
                                                let file_path = change.path.to_string_lossy().to_string();
                                                Symbol {
                                                    id: format!("{}#{}:{}", file_path, doc_sym.range.start.line, doc_sym.name),
                                                    name: doc_sym.name,
                                                    kind: match doc_sym.kind {
                                                        lsp_types::SymbolKind::FUNCTION => SymbolKind::Function,
                                                        lsp_types::SymbolKind::CLASS => SymbolKind::Class,
                                                        lsp_types::SymbolKind::INTERFACE => SymbolKind::Interface,
                                                        lsp_types::SymbolKind::STRUCT => SymbolKind::Class,
                                                        lsp_types::SymbolKind::ENUM => SymbolKind::Enum,
                                                        lsp_types::SymbolKind::CONSTANT => SymbolKind::Constant,
                                                        lsp_types::SymbolKind::VARIABLE => SymbolKind::Variable,
                                                        lsp_types::SymbolKind::METHOD => SymbolKind::Function,
                                                        lsp_types::SymbolKind::PROPERTY => SymbolKind::Property,
                                                        _ => SymbolKind::Variable,
                                                    },
                                                    file_path: file_path.clone(),
                                                    range: lsif_core::Range {
                                                        start: lsif_core::Position {
                                                            line: doc_sym.range.start.line,
                                                            character: doc_sym.range.start.character,
                                                        },
                                                        end: lsif_core::Position {
                                                            line: doc_sym.range.end.line,
                                                            character: doc_sym.range.end.character,
                                                        },
                                                    },
                                                    documentation: doc_sym.detail,
                                                    detail: None,
                                                }
                                            }).collect();
                                            Some(symbols)
                                        }
                                        Err(e) => {
                                            warn!("Failed to extract symbols from {}: {}", change.path.display(), e);
                                            None
                                        }
                                    }
                                }
                                None => {
                                    debug!("No fallback indexer for {}", change.path.display());
                                    None
                                }
                            }
                        }
                        FileChangeStatus::Deleted => None,
                    };
                    (change.path.clone(), change.status.clone(), symbols)
                })
                .collect();
            
            // グラフとハッシュの更新
            for (path, status, symbols_opt) in symbols_results {
                // ハッシュを記録
                if let Some(change) = changes.iter().find(|c| c.path == path) {
                    if let Some(ref hash) = change.content_hash {
                        new_file_hashes.insert(path.clone(), hash.clone());
                    }
                }
                
                // プログレス表示
                processed_count += 1;
                if total_files > 10 && processed_count % 10 == 0 {
                    eprintln!("  ⚡ Processed {}/{} files ({:.0}%)", 
                             processed_count, total_files, 
                             (processed_count as f64 / total_files as f64) * 100.0);
                }
                
                match status {
                    FileChangeStatus::Added | FileChangeStatus::Untracked => {
                        result.files_added += 1;
                        
                        if let Some(symbols) = symbols_opt {
                            result.symbols_added += symbols.len();
                            
                            for symbol in symbols {
                                if result.added_symbols.len() < 20 {
                                    result.added_symbols.push(SymbolSummary {
                                        name: symbol.name.clone(),
                                        kind: symbol.kind,
                                        file_path: symbol.file_path.clone(),
                                        line: symbol.range.start.line,
                                    });
                                }
                                graph.add_symbol(symbol);
                            }
                            
                            // 参照の追加は並列処理モードではスキップ（非常に重いため）
                            // 必要に応じて後で別途実行可能
                        }
                    }
                    FileChangeStatus::Modified | FileChangeStatus::Renamed { .. } => {
                        result.files_modified += 1;
                        
                        // 既存シンボルの削除
                        let path_str = path.to_string_lossy();
                        let old_symbols: Vec<_> = graph
                            .get_all_symbols()
                            .filter(|s| s.file_path == path_str)
                            .cloned()
                            .collect();
                        
                        for symbol in &old_symbols {
                            if result.deleted_symbols.len() < 20 {
                                result.deleted_symbols.push(SymbolSummary {
                                    name: symbol.name.clone(),
                                    kind: symbol.kind,
                                    file_path: symbol.file_path.clone(),
                                    line: symbol.range.start.line,
                                });
                            }
                            graph.remove_symbol(&symbol.id);
                        }
                        
                        result.symbols_deleted += old_symbols.len();
                        
                        // 新規シンボルの追加
                        if let Some(symbols) = symbols_opt {
                            result.symbols_updated += symbols.len();
                            
                            for symbol in symbols {
                                if result.added_symbols.len() < 20 {
                                    result.added_symbols.push(SymbolSummary {
                                        name: symbol.name.clone(),
                                        kind: symbol.kind,
                                        file_path: symbol.file_path.clone(),
                                        line: symbol.range.start.line,
                                    });
                                }
                                graph.add_symbol(symbol);
                            }
                            
                            // 参照の追加はスキップ（非常に重いため）
                            // if let Err(e) = self.add_references_to_graph(&mut graph, &path) {
                            //     warn!("Failed to add references for {}: {}", path.display(), e);
                            // }
                        }
                    }
                    FileChangeStatus::Deleted => {
                        result.files_deleted += 1;
                        
                        let path_str = path.to_string_lossy();
                        let old_symbols: Vec<_> = graph
                            .get_all_symbols()
                            .filter(|s| s.file_path == path_str)
                            .cloned()
                            .collect();
                        
                        for symbol in &old_symbols {
                            if result.deleted_symbols.len() < 20 {
                                result.deleted_symbols.push(SymbolSummary {
                                    name: symbol.name.clone(),
                                    kind: symbol.kind,
                                    file_path: symbol.file_path.clone(),
                                    line: symbol.range.start.line,
                                });
                            }
                            graph.remove_symbol(&symbol.id);
                        }
                        
                        result.symbols_deleted += old_symbols.len();
                    }
                }
            }
            
        } else {
            // シーケンシャル処理（小規模プロジェクト用）
            for change in changes {
                // コンテンツハッシュを記録
                if let Some(ref hash) = change.content_hash {
                    new_file_hashes.insert(change.path.clone(), hash.clone());
                }
                
                // プログレスバー更新
                processed_count += 1;
                if let Some(ref pb) = progress_bar {
                    let file_name = change.path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    pb.set_position(processed_count as u64);
                    pb.set_message(format!("Processing: {}", file_name));
                }

                match change.status {
                FileChangeStatus::Added | FileChangeStatus::Untracked => {
                    result.files_added += 1;
                    
                    info!("Processing added file: {}", change.path.display());
                    
                    // プログレスバーに抽出方法を表示
                    if let Some(ref pb) = progress_bar {
                        let file_name = change.path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        let extraction_mode = if self.fallback_only { "[Fallback]" } else { "[LSP/Fallback]" };
                        pb.set_message(format!("{} Extracting: {} ...", extraction_mode, file_name));
                    }
                    
                    let extraction_start = Instant::now();
                    let symbols = self.extract_symbols_from_file(&change.path)?;
                    let extraction_time = extraction_start.elapsed();
                    
                    info!("Successfully extracted {} symbols from {} in {:.2}s", 
                          symbols.len(), change.path.display(), extraction_time.as_secs_f64());
                    
                    // 遅い処理の警告
                    if extraction_time.as_secs() > 2 {
                        if let Some(ref pb) = progress_bar {
                            pb.set_message(format!("⚠️  Slow: {} took {:.1}s", 
                                change.path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
                                extraction_time.as_secs_f64()));
                        }
                    }
                    
                    result.symbols_added += symbols.len();

                    // グラフにシンボルを追加し、サマリーを記録
                    for symbol in symbols {
                        info!("Adding symbol to graph: {} (id: {}, kind: {:?})", 
                              symbol.name, symbol.id, symbol.kind);
                        
                        // サマリーを記録（最大20件まで）
                        if result.added_symbols.len() < 20 {
                            result.added_symbols.push(SymbolSummary {
                                name: symbol.name.clone(),
                                kind: symbol.kind,
                                file_path: symbol.file_path.clone(),
                                line: symbol.range.start.line,
                            });
                        }
                        
                        // グラフにシンボルを実際に追加
                        graph.add_symbol(symbol.clone());
                        debug!("Symbol added to graph successfully");
                    }

                    // 参照を検出してエッジを追加
                    debug!("Adding references to graph for: {}", change.path.display());
                    // 参照の追加はスキップ（非常に重いため）
                    // if let Err(e) = self.add_references_to_graph(&mut graph, &change.path) {
                    //     warn!("Failed to add references for {}: {}", change.path.display(), e);
                    // }
                }
                FileChangeStatus::Modified | FileChangeStatus::Renamed { .. } => {
                    result.files_modified += 1;

                    // 既存のシンボルを削除
                    let path_str = change.path.to_string_lossy();
                    let old_symbols: Vec<_> = graph
                        .get_all_symbols()
                        .filter(|s| s.file_path == path_str)
                        .cloned()
                        .collect();

                    for symbol in &old_symbols {
                        // サマリーを記録（最大20件まで）
                        if result.deleted_symbols.len() < 20 {
                            result.deleted_symbols.push(SymbolSummary {
                                name: symbol.name.clone(),
                                kind: symbol.kind,
                                file_path: symbol.file_path.clone(),
                                line: symbol.range.start.line,
                            });
                        }
                        graph.remove_symbol(&symbol.id);
                    }
                    result.symbols_deleted += old_symbols.len();

                    // 新しいシンボルを追加
                    let symbols = self.extract_symbols_from_file(&change.path)?;
                    result.symbols_updated += symbols.len();

                    for symbol in symbols {
                        // サマリーを記録（最大20件まで）
                        if result.added_symbols.len() < 20 {
                            result.added_symbols.push(SymbolSummary {
                                name: symbol.name.clone(),
                                kind: symbol.kind,
                                file_path: symbol.file_path.clone(),
                                line: symbol.range.start.line,
                            });
                        }
                        graph.add_symbol(symbol);
                    }

                    // 参照を検出してエッジを追加
                    // 参照の追加はスキップ（非常に重いため）
                    // self.add_references_to_graph(&mut graph, &change.path)?;
                }
                FileChangeStatus::Deleted => {
                    result.files_deleted += 1;

                    // シンボルを削除
                    let path_str = change.path.to_string_lossy();
                    let old_symbols: Vec<_> = graph
                        .get_all_symbols()
                        .filter(|s| s.file_path == path_str)
                        .cloned()
                        .collect();

                    for symbol in &old_symbols {
                        // サマリーを記録（最大20件まで）
                        if result.deleted_symbols.len() < 20 {
                            result.deleted_symbols.push(SymbolSummary {
                                name: symbol.name.clone(),
                                kind: symbol.kind,
                                file_path: symbol.file_path.clone(),
                                line: symbol.range.start.line,
                            });
                        }
                        graph.remove_symbol(&symbol.id);
                    }
                    result.symbols_deleted += old_symbols.len();
                }
                FileChangeStatus::Untracked => {
                    // コンテンツハッシュで管理
                    if let Some(ref hash) = change.content_hash {
                        if self.is_file_changed(&change.path, hash)? {
                            result.files_modified += 1;

                            // 既存のシンボルを削除
                            let path_str = change.path.to_string_lossy();
                            let old_symbols: Vec<_> = graph
                                .get_all_symbols()
                                .filter(|s| s.file_path == path_str)
                                .map(|s| s.id.clone())
                                .collect();

                            for id in &old_symbols {
                                graph.remove_symbol(id);
                            }
                            result.symbols_deleted += old_symbols.len();

                            // 新しいシンボルを追加
                            let symbols = self.extract_symbols_from_file(&change.path)?;
                            result.symbols_updated += symbols.len();

                            for symbol in symbols {
                                graph.add_symbol(symbol);
                            }
                        }
                    }
                }
            }
        }
        }
        
        // プログレスバー完了
        if let Some(pb) = progress_bar {
            pb.finish_with_message(format!(
                "Completed: {} files, {} symbols (added: {}, updated: {}, deleted: {})",
                total_files,
                result.symbols_added + result.symbols_updated,
                result.symbols_added,
                result.symbols_updated,
                result.symbols_deleted
            ));
        }

        // CodeGraphを保存
        info!("Saving CodeGraph with {} symbols to database", graph.symbol_count());
        
        // 保存前に少しサンプルをログ出力
        let sample_symbols: Vec<_> = graph.get_all_symbols().take(5).collect();
        for symbol in &sample_symbols {
            debug!("Sample symbol in graph: {} ({:?}) from {}", 
                   symbol.name, symbol.kind, symbol.file_path);
        }
        
        match self.storage.save_data("graph", &graph) {
            Ok(()) => {
                info!("CodeGraph with {} symbols saved successfully to database", graph.symbol_count());
            }
            Err(e) => {
                error!("Failed to save CodeGraph: {}", e);
                return Err(e);
            }
        }

        // ファイルハッシュを保存
        if let Some(ref mut metadata) = self.metadata {
            metadata.file_content_hashes.extend(new_file_hashes.clone());
            debug!(
                "Updated file hashes: {} total",
                metadata.file_content_hashes.len()
            );
        }

        // メタデータを更新
        debug!("Updating metadata...");
        self.update_metadata()?;
        debug!("Metadata updated successfully");

        result.duration = start.elapsed();

        info!(
            "Differential indexing complete: {} files added, {} modified, {} deleted in {:.2}s",
            result.files_added,
            result.files_modified,
            result.files_deleted,
            result.duration.as_secs_f64()
        );

        Ok(result)
    }

    /// ファイルが変更されているかをハッシュで確認
    fn is_file_changed(&self, path: &Path, new_hash: &str) -> Result<bool> {
        if let Some(ref metadata) = self.metadata {
            if let Some(old_hash) = metadata.file_content_hashes.get(path) {
                return Ok(old_hash != new_hash);
            }
        }
        // ハッシュが見つからない場合は変更として扱う
        Ok(true)
    }

    /// ファイルからシンボルを抽出（処理時間を計測）
    fn extract_symbols_from_file(&mut self, path: &Path) -> Result<Vec<Symbol>> {
        info!("Extracting symbols from: {}", path.display());
        let start_time = Instant::now();
        
        // フォールバックオンリーモードの場合は直接フォールバックを使用
        if self.fallback_only {
            let _fallback_start = Instant::now();
            match self.extract_symbols_with_fallback(path) {
                Ok(symbols) => {
                    let elapsed = start_time.elapsed();
                    info!("Fallback extracted {} symbols from {} in {:.3}s", 
                          symbols.len(), path.display(), elapsed.as_secs_f64());
                    if elapsed.as_secs() >= 2 {
                        warn!("⚠️  Slow extraction: {} took {:.1}s (fallback)", 
                              path.display(), elapsed.as_secs_f64());
                    }
                    Ok(symbols)
                }
                Err(e) => {
                    warn!("Fallback indexer failed for {}: {}", path.display(), e);
                    Ok(Vec::new())
                }
            }
        } else {
            // LSPインデクサーを優先的に使用（より正確なシンボル情報を取得）
            // まずLSPを試行
            let lsp_start = Instant::now();
            match self.extract_symbols_with_lsp(path) {
                Ok(symbols) if !symbols.is_empty() => {
                    let elapsed = lsp_start.elapsed();
                    info!("Successfully extracted {} symbols using LSP from {} in {:.3}s", 
                          symbols.len(), path.display(), elapsed.as_secs_f64());
                    if elapsed.as_secs() >= 3 {
                        warn!("⚠️  Slow LSP extraction: {} took {:.1}s", 
                              path.display(), elapsed.as_secs_f64());
                    }
                    Ok(symbols)
                }
                Ok(_) => {
                    let lsp_elapsed = lsp_start.elapsed();
                    // LSPで空の結果が返った場合はフォールバックを試行
                    info!("LSP returned no symbols after {:.3}s, trying fallback for: {}", 
                          lsp_elapsed.as_secs_f64(), path.display());
                    let fallback_start = Instant::now();
                    match self.extract_symbols_with_fallback(path) {
                        Ok(symbols) => {
                            let fallback_elapsed = fallback_start.elapsed();
                            info!("Fallback extracted {} symbols from {} in {:.3}s (total: {:.3}s)", 
                                  symbols.len(), path.display(), 
                                  fallback_elapsed.as_secs_f64(),
                                  start_time.elapsed().as_secs_f64());
                            Ok(symbols)
                        }
                        Err(e) => {
                            warn!("Both LSP and fallback failed for {} (total time: {:.3}s): {}", 
                                  path.display(), start_time.elapsed().as_secs_f64(), e);
                            Ok(Vec::new())
                        }
                    }
                }
                Err(e) => {
                    let lsp_elapsed = lsp_start.elapsed();
                    warn!("LSP indexer failed for {} after {:.3}s: {}, trying fallback", 
                          path.display(), lsp_elapsed.as_secs_f64(), e);
                    let fallback_start = Instant::now();
                    match self.extract_symbols_with_fallback(path) {
                        Ok(symbols) => {
                            info!("Fallback extracted {} symbols after LSP failed from {}", 
                                  symbols.len(), path.display());
                            Ok(symbols)
                        }
                        Err(fallback_error) => {
                            warn!("Both LSP and fallback failed for {}: lsp={}, fallback={}", 
                                  path.display(), e, fallback_error);
                            Ok(Vec::new())
                        }
                    }
                }
            }
        }
    }


    /// メタデータを更新
    fn update_metadata(&mut self) -> Result<()> {
        // DBディレクトリ内にハッシュキャッシュを保存（プロジェクトルートではなく）
        let db_dir = self.storage.get_db_path()?;
        let hash_cache_path = db_dir.join("hash-cache.json");
        
        info!("Saving hash cache to: {:?}", hash_cache_path);

        // ハッシュキャッシュを保存
        self.git_detector.save_hash_cache(&hash_cache_path)?;

        // 現在のファイルハッシュを収集
        let mut file_content_hashes = self
            .metadata
            .as_ref()
            .map(|m| m.file_content_hashes.clone())
            .unwrap_or_default();

        // 各ファイルの最新ハッシュを計算
        for entry in walkdir::WalkDir::new(&self.project_root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if path
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| {
                    ext == "rs" || ext == "ts" || ext == "js" || ext == "tsx" || ext == "jsx"
                })
                .unwrap_or(false)
            {
                if let Ok(hash) = self.git_detector.calculate_file_hash(path) {
                    file_content_hashes.insert(path.to_path_buf(), hash);
                }
            }
        }

        let metadata = DifferentialIndexMetadata {
            last_indexed_at: Utc::now(),
            last_commit: self.git_detector.get_head_commit(),
            indexed_files: self.count_indexed_files()?,
            total_symbols: self.count_total_symbols()?,
            hash_cache_path: Some(hash_cache_path),
            file_content_hashes,
        };

        self.storage
            .save_data("__differential_metadata__", &metadata)?;
        self.metadata = Some(metadata.clone());

        // IndexStorageのメタデータも更新
        let storage_metadata = crate::storage::IndexMetadata {
            format: crate::storage::IndexFormat::Lsif,
            version: "1.0.0".to_string(),
            created_at: Utc::now(),
            project_root: self.project_root.to_string_lossy().to_string(),
            files_count: metadata.indexed_files,
            symbols_count: metadata.total_symbols,
            git_commit_hash: metadata.last_commit.clone(),
            file_hashes: metadata
                .file_content_hashes
                .iter()
                .map(|(k, v)| (k.to_string_lossy().to_string(), v.clone()))
                .collect(),
        };

        self.storage.save_metadata(&storage_metadata)?;

        Ok(())
    }

    /// 全ファイル数をカウント（プロジェクト内の対象ファイル）
    fn count_total_files(&self) -> Result<usize> {
        let mut count = 0;
        for entry in walkdir::WalkDir::new(&self.project_root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if path
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| {
                    ext == "rs" || ext == "ts" || ext == "js" || ext == "tsx" || ext == "jsx" 
                    || ext == "py" || ext == "go" || ext == "java"
                })
                .unwrap_or(false)
            {
                count += 1;
            }
        }
        Ok(count)
    }
    
    /// インデックス済みファイル数をカウント
    fn count_indexed_files(&self) -> Result<usize> {
        // CodeGraphから取得
        if let Some(graph) = self.storage.load_data::<CodeGraph>("graph")? {
            let mut files = HashSet::new();
            for symbol in graph.get_all_symbols() {
                files.insert(symbol.file_path.clone());
            }
            Ok(files.len())
        } else {
            Ok(0)
        }
    }

    /// 総シンボル数をカウント
    fn count_total_symbols(&self) -> Result<usize> {
        // CodeGraphから取得
        if let Some(graph) = self.storage.load_data::<CodeGraph>("graph")? {
            Ok(graph.symbol_count())
        } else {
            Ok(0)
        }
    }

    /// 完全再インデックス
    pub fn full_reindex(&mut self) -> Result<DifferentialIndexResult> {
        info!("Performing full reindex...");
        let start = Instant::now();

        // メタデータをクリア
        self.metadata = None;

        // workspace/symbolがサポートされているかチェック
        if self.try_workspace_symbol_index()? {
            // workspace/symbolで成功した場合は結果を返す
            info!("Full reindex completed using workspace/symbol");
            let total_symbols = self.count_total_symbols()?;
            return Ok(DifferentialIndexResult {
                files_added: 0,  // workspace/symbolではファイル単位の情報はない
                files_modified: 0,
                files_deleted: 0,
                symbols_added: total_symbols,
                symbols_updated: 0,
                symbols_deleted: 0,
                duration: start.elapsed(),
                added_symbols: Vec::new(),
                deleted_symbols: Vec::new(),
                full_reindex: true,
                change_ratio: 1.0,
            });
        }

        // workspace/symbolが使えない場合は通常の処理
        info!("workspace/symbol not available, falling back to file-by-file indexing");
        self.index_differential()
    }

    /// workspace/symbolを使用してインデックスを試みる
    fn try_workspace_symbol_index(&mut self) -> Result<bool> {
        info!("Attempting to use workspace/symbol for fast indexing...");
        
        // プロジェクトの主要言語を検出
        let language = detect_project_language(&self.project_root);
        
        // 言語IDを文字列に変換
        let language_id = match language {
            lsp::Language::Rust => "rust",
            lsp::Language::TypeScript => "typescript",
            lsp::Language::JavaScript => "javascript",
            lsp::Language::Python => "python",
            lsp::Language::Go => "go",
            lsp::Language::Unknown => {
                info!("Could not detect project language, skipping workspace/symbol");
                return Ok(false);
            }
        };
        
        info!("Detected project language: {}", language_id);
        
        // LSPクライアントを事前に初期化（キャパビリティを取得するため）
        // サンプルファイルを見つける
        let sample_file = walkdir::WalkDir::new(&self.project_root)
            .follow_links(false)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .find(|e| {
                let ext = e.path().extension().and_then(|s| s.to_str()).unwrap_or("");
                match language_id {
                    "rust" => ext == "rs",
                    "typescript" => ext == "ts" || ext == "tsx",
                    "javascript" => ext == "js" || ext == "jsx",
                    "python" => ext == "py",
                    "go" => ext == "go",
                    _ => false,
                }
            });
        
        if let Some(sample) = sample_file {
            // クライアントを初期化
            match self.lsp_pool.get_or_create_client(sample.path(), &self.project_root) {
                Ok(_) => {
                    info!("LSP client initialized for capability check");
                }
                Err(e) => {
                    warn!("Failed to initialize LSP client: {}", e);
                    return Ok(false);
                }
            }
        }
        
        // workspace/symbolがサポートされているかチェック
        if !self.lsp_pool.has_capability_for_language(language_id, "workspace/symbol") {
            info!("Language {} does not support workspace/symbol", language_id);
            return Ok(false);
        }
        
        // WorkspaceSymbolStrategyを使用（スタンドアロン版）
        use crate::workspace_symbol_strategy::WorkspaceSymbolStrategy;
        
        let strategy = WorkspaceSymbolStrategy::new(self.project_root.clone());
        
        match strategy.index() {
            Ok(graph) => {
                info!("workspace/symbol extracted {} symbols", graph.symbol_count());
                
                // ストレージに保存
                self.storage.save_data("graph", &graph)?;
                info!("Saved {} symbols to storage", graph.symbol_count());
                
                // メタデータを更新
                self.update_metadata()?;
                
                Ok(true)
            }
            Err(e) => {
                warn!("workspace/symbol failed: {}", e);
                Ok(false)
            }
        }
    }

    /// プロジェクト内の全ファイルをスキャン
    fn scan_all_files(&self) -> Result<Vec<FileChange>> {
        let mut changes = Vec::new();
        info!("Scanning all files in: {}", self.project_root.display());
        
        // プロジェクトルートの存在確認
        if !self.project_root.exists() {
            error!("Project root does not exist: {}", self.project_root.display());
            return Err(anyhow::anyhow!("Project root does not exist: {}", self.project_root.display()));
        }
        
        if !self.project_root.is_dir() {
            error!("Project root is not a directory: {}", self.project_root.display());
            return Err(anyhow::anyhow!("Project root is not a directory: {}", self.project_root.display()));
        }
        
        debug!("Project root exists and is directory: {}", self.project_root.display());
        debug!("Project root canonical path: {:?}", std::fs::canonicalize(&self.project_root));

        // walkdirの動作を詳細にログ
        let walkdir = walkdir::WalkDir::new(&self.project_root)
            .follow_links(false)
            .max_depth(100);
        info!("Created walkdir for path: {}", self.project_root.display());
        
        let mut entry_count = 0;
        let mut file_count = 0;
        let mut error_count = 0;

        for entry_result in walkdir.into_iter() {
            entry_count += 1;
            
            let entry = match entry_result {
                Ok(e) => e,
                Err(err) => {
                    error_count += 1;
                    warn!("Walkdir error #{}: {}", error_count, err);
                    continue;
                }
            };
            
            debug!("Walkdir entry #{}: {} (is_file: {}, is_dir: {})", 
                   entry_count, entry.path().display(), 
                   entry.file_type().is_file(), entry.file_type().is_dir());
            
            if !entry.file_type().is_file() {
                debug!("  -> Skipping directory: {}", entry.path().display());
                continue;
            }
            
            file_count += 1;
            let path = entry.path();

            // .gitディレクトリ、targetディレクトリなどを除外
            if self.should_exclude(path) {
                debug!("  -> Excluding file: {}", path.display());
                continue;
            }

            // 対象ファイルのみ処理
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            debug!("  -> File extension: '{}' for {}", ext, path.display());
            
            if matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx") {
                info!("  -> Found source file: {}", path.display());
                let content_hash = self.git_detector.calculate_file_hash(path).ok();
                changes.push(FileChange {
                    path: path.to_path_buf(),
                    status: FileChangeStatus::Added,
                    content_hash,
                });
            } else {
                debug!("  -> Skipping non-source file: {} (ext: '{}')", path.display(), ext);
            }
        }

        info!("Walkdir scan complete: {} entries processed, {} files found, {} errors, {} source files selected", 
              entry_count, file_count, error_count, changes.len());
        info!("scan_all_files found {} files", changes.len());
        Ok(changes)
    }

    /// 使用される言語を検出してLSPクライアントを事前起動
    fn warm_up_lsp_clients(&mut self) -> Result<()> {
        info!("Detecting languages in project...");
        
        // プロジェクト内の言語を検出
        let mut languages = HashSet::new();
        let mut sample_count = 0;
        const MAX_SAMPLES: usize = 20; // 最初の20ファイルで判断
        
        for entry in walkdir::WalkDir::new(&self.project_root)
            .follow_links(false)
            .max_depth(3) // 深さを制限して高速化
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            
            if self.should_exclude(path) {
                continue;
            }
            
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let lang = match ext {
                    "rs" => Some("rust"),
                    "ts" | "tsx" => Some("typescript"),
                    "js" | "jsx" => Some("javascript"),
                    "py" => Some("python"),
                    "go" => Some("go"),
                    "java" => Some("java"),
                    _ => None,
                };
                
                if let Some(l) = lang {
                    languages.insert(l);
                    sample_count += 1;
                    
                    if sample_count >= MAX_SAMPLES {
                        break;
                    }
                }
            }
        }
        
        if languages.is_empty() {
            info!("No supported languages detected, skipping LSP warm-up");
            return Ok(());
        }
        
        // 検出された言語のLSPクライアントを事前起動
        let langs: Vec<&str> = languages.into_iter().collect();
        info!("Warming up LSP clients for languages: {:?}", langs);
        
        let warm_up_start = Instant::now();
        self.lsp_pool.warm_up(&self.project_root, &langs)?;
        info!("LSP warm-up completed in {:.2}s", warm_up_start.elapsed().as_secs_f64());
        
        Ok(())
    }
    
    /// 除外すべきパスかどうかを判定
    fn should_exclude(&self, path: &Path) -> bool {
        debug!("Checking if path should be excluded: {}", path.display());
        
        // プロジェクトルートからの相対パスを取得
        let relative_path = if let Ok(rel_path) = path.strip_prefix(&self.project_root) {
            rel_path
        } else {
            // プロジェクトルート外のパスは除外
            debug!("  -> Path outside project root, excluded: {}", path.display());
            return true;
        };

        debug!("  -> Relative path: {}", relative_path.display());

        // 相対パスの各コンポーネントをチェック
        for component in relative_path.components() {
            if let Some(name) = component.as_os_str().to_str() {
                debug!("  -> Checking relative path component: '{}'", name);
                if matches!(
                    name,
                    ".git" | "target" | "node_modules" | ".idea" | ".vscode" | "tmp"
                ) {
                    debug!("  -> Path excluded due to relative component: '{}'", name);
                    return true;
                }
            }
        }
        debug!("  -> Path not excluded: {}", path.display());
        false
    }

    /// ファイルから参照を検出してグラフにエッジを追加
    fn add_references_to_graph(&self, graph: &mut CodeGraph, file_path: &Path) -> Result<()> {
        // グラフの中のすべてのシンボルを取得
        let all_symbols: Vec<_> = graph
            .get_all_symbols()
            .map(|s| (s.name.clone(), s.id.clone(), s.kind))
            .collect();

        // 各シンボルに対して、このファイル内での参照を検索
        for (name, symbol_id, kind) in all_symbols {
            let references =
                reference_finder::find_all_references(&self.project_root, &name, &kind)?;

            // このファイル内の参照のみを処理
            let file_path_str = file_path.to_string_lossy();
            for ref_item in references {
                if ref_item.symbol.file_path == file_path_str && !ref_item.is_definition {
                    // 参照元シンボルを特定
                    // 現在の位置に最も近いシンボルを探す
                    if let Some(source_symbol) = self.find_symbol_at_position(
                        graph,
                        &ref_item.symbol.file_path,
                        ref_item.symbol.range.start.line,
                        ref_item.symbol.range.start.character,
                    ) {
                        // 参照エッジを追加
                        if let (Some(from_idx), Some(to_idx)) = (
                            graph.get_node_index(&source_symbol.id),
                            graph.get_node_index(&symbol_id),
                        ) {
                            graph.add_edge(from_idx, to_idx, lsif_core::EdgeKind::Reference);
                            debug!(
                                "Added reference edge: {} -> {}",
                                source_symbol.id, symbol_id
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 指定位置のシンボルを探す
    fn find_symbol_at_position(
        &self,
        graph: &CodeGraph,
        file_path: &str,
        line: u32,
        character: u32,
    ) -> Option<Symbol> {
        graph
            .get_all_symbols()
            .filter(|s| s.file_path == file_path)
            .find(|s| {
                s.range.start.line <= line
                    && s.range.end.line >= line
                    && s.range.start.character <= character
                    && s.range.end.character >= character
            })
            .cloned()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_differential_index_metadata_new() {
        let metadata = DifferentialIndexMetadata {
            last_indexed_at: Utc::now(),
            last_commit: Some("abc123".to_string()),
            indexed_files: 10,
            total_symbols: 50,
            hash_cache_path: None,
            file_content_hashes: HashMap::new(),
        };

        assert_eq!(metadata.indexed_files, 10);
        assert_eq!(metadata.total_symbols, 50);
        assert_eq!(metadata.last_commit, Some("abc123".to_string()));
    }

    #[test]
    fn test_differential_index_result() {
        let result = DifferentialIndexResult {
            files_added: 5,
            files_modified: 3,
            files_deleted: 2,
            symbols_added: 20,
            symbols_updated: 15,
            symbols_deleted: 10,
            duration: Duration::from_secs(1),
            added_symbols: Vec::new(),
            deleted_symbols: Vec::new(),
            full_reindex: false,
            change_ratio: 0.3,
        };

        assert_eq!(result.files_added, 5);
        assert_eq!(result.files_modified, 3);
        assert_eq!(result.files_deleted, 2);
        assert_eq!(result.symbols_added, 20);
        assert_eq!(result.symbols_updated, 15);
        assert_eq!(result.symbols_deleted, 10);
        assert_eq!(result.duration.as_secs(), 1);
    }


    #[test]
    fn test_new_differential_indexer() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test.db");
        let project_root = temp_dir.path();

        // Git初期化
        fs::create_dir_all(project_root.join(".git")).unwrap();

        let indexer = DifferentialIndexer::new(&storage_path, project_root);
        assert!(indexer.is_ok());

        let indexer = indexer.unwrap();
        assert_eq!(indexer.project_root, project_root);
    }

    #[test]
    #[ignore] // TODO: Fix test - needs proper LSP setup
    fn test_full_reindex() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test.db");
        let project_root = temp_dir.path();

        // Git初期化とテストファイル作成
        fs::create_dir_all(project_root.join(".git")).unwrap();
        fs::write(project_root.join("test.rs"), "fn main() {}").unwrap();

        let mut indexer = DifferentialIndexer::new(&storage_path, project_root).unwrap();

        // フルリインデックスを実行
        let result = indexer.full_reindex();
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.files_added > 0 || result.files_modified > 0);
    }

    #[test]
    fn test_find_symbol_at_position() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test.db");
        let project_root = temp_dir.path();

        // Git初期化
        fs::create_dir_all(project_root.join(".git")).unwrap();

        let indexer = DifferentialIndexer::new(&storage_path, project_root).unwrap();
        let mut graph = CodeGraph::new();

        let symbol = Symbol {
            id: "test".to_string(),
            name: "test".to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: lsif_core::Range {
                start: lsif_core::Position {
                    line: 0,
                    character: 0,
                },
                end: lsif_core::Position {
                    line: 5,
                    character: 10,
                },
            },
            documentation: None,
            detail: None,
        };

        graph.add_symbol(symbol.clone());

        let found = indexer.find_symbol_at_position(&graph, "test.rs", 2, 5);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "test");

        let not_found = indexer.find_symbol_at_position(&graph, "test.rs", 10, 5);
        assert!(not_found.is_none());
    }


    #[test]
    fn test_convert_lsp_symbol_kind() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test.db");
        let project_root = temp_dir.path();

        // Git初期化
        fs::create_dir_all(project_root.join(".git")).unwrap();

        let indexer = DifferentialIndexer::new(&storage_path, project_root).unwrap();
        
        // LSP SymbolKindをコアのSymbolKindに変換
        assert_eq!(
            indexer.convert_lsp_symbol_kind(lsp_types::SymbolKind::FUNCTION),
            SymbolKind::Function
        );
        assert_eq!(
            indexer.convert_lsp_symbol_kind(lsp_types::SymbolKind::CLASS),
            SymbolKind::Class
        );
        assert_eq!(
            indexer.convert_lsp_symbol_kind(lsp_types::SymbolKind::MODULE),
            SymbolKind::Module
        );
        assert_eq!(
            indexer.convert_lsp_symbol_kind(lsp_types::SymbolKind::VARIABLE),
            SymbolKind::Variable
        );
        assert_eq!(
            indexer.convert_lsp_symbol_kind(lsp_types::SymbolKind::ENUM),
            SymbolKind::Enum
        );
    }

    #[test]
    fn test_scan_all_files_debug() {
        // プロジェクトルートディレクトリを取得
        let cwd = std::env::current_dir().unwrap();
        let project_root = cwd.parent().unwrap().parent().unwrap().join("tmp/sample-project");
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test.db");

        // プロジェクトディレクトリが存在することを確認
        if !project_root.exists() {
            eprintln!("Test project directory does not exist: {}", project_root.display());
            eprintln!("Current working directory: {}", cwd.display());
            return;
        }


        // Git初期化
        if !project_root.join(".git").exists() {
            fs::create_dir_all(project_root.join(".git")).unwrap();
        }

        let indexer = DifferentialIndexer::new(&storage_path, &project_root).unwrap();
        
        // scan_all_files を実行
        match indexer.scan_all_files() {
            Ok(changes) => {
                eprintln!("scan_all_files succeeded: found {} files", changes.len());
                for change in &changes {
                    eprintln!("  -> {}", change.path.display());
                }
                // 5つのRustファイルが見つかることを期待（追加したdataも含む）
                assert!(changes.len() >= 4, "Expected at least 4 .rs files, found {}", changes.len());
            }
            Err(e) => {
                eprintln!("scan_all_files failed: {}", e);
                panic!("scan_all_files should not fail");
            }
        }
    }

}
