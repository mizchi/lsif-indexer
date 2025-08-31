use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::adaptive_parallel::{AdaptiveParallelConfig, AdaptiveIncrementalProcessor};
use crate::git_diff::{FileChange, FileChangeStatus, GitDiffDetector};
use crate::reference_finder;
use crate::storage::IndexStorage;
use core::{CodeGraph, Symbol, SymbolKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir;

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

        // LSPプールの設定
        let pool_config = PoolConfig {
            max_idle_time: std::time::Duration::from_secs(300),
            init_timeout: std::time::Duration::from_secs(30),
            request_timeout: std::time::Duration::from_secs(5),
            max_retries: 3,
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
        })
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
        
        debug!("Using LSP to extract symbols from: {}", path.display());
        
        let start = Instant::now();
        
        // ファイルの絶対パスを取得
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            canonicalize(path)?
        };
        
        let file_uri = format!("file://{}", absolute_path.display());
        
        // LSPプールからクライアントを取得
        match self.lsp_pool.get_or_create_client(path, &self.project_root) {
            Ok(client_arc) => {
                // クライアントをロックして使用
                match client_arc.lock() {
                    Ok(mut client) => {
                        // ドキュメントシンボルを取得
                        match client.get_document_symbols(&file_uri) {
                            Ok(lsp_symbols) => {
                                debug!("LSP extracted {} symbols from {} in {:?}", 
                                       lsp_symbols.len(), path.display(), start.elapsed());
                                // LSPシンボルをコアのSymbol型に変換
                                let symbols = self.convert_lsp_symbols_to_core(&lsp_symbols, path);
                                Ok(symbols)
                            }
                            Err(e) => {
                                warn!("LSP extraction failed for {}: {}. Using fallback.", path.display(), e);
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
        
        debug!("Using fallback indexer for: {}", path.display());
        if let Some(fallback) = FallbackIndexer::from_extension(path) {
            let lsp_symbols = fallback.extract_symbols(path)?;
            // LSPシンボルをコアのSymbol型に変換
            let symbols = self.convert_lsp_symbols_to_core(&lsp_symbols, path);
            Ok(symbols)
        } else {
            // フォールバックも使えない場合は空のリストを返す
            debug!("No indexer available for file: {}", path.display());
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
                range: core::Range {
                    start: core::Position {
                        line: lsp_symbol.range.start.line,
                        character: lsp_symbol.range.start.character,
                    },
                    end: core::Position {
                        line: lsp_symbol.range.end.line,
                        character: lsp_symbol.range.end.character,
                    },
                },
                documentation: lsp_symbol.detail.clone(),
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

        // 前回のメタデータからハッシュキャッシュを復元
        if let Some(ref metadata) = self.metadata {
            if let Some(ref cache_path) = metadata.hash_cache_path {
                self.git_detector.load_hash_cache(cache_path).ok();
            }
        }

        // 前回のコミットを取得
        let last_commit = self
            .metadata
            .as_ref()
            .and_then(|m| m.last_commit.as_deref());

        // 変更ファイルを検出（初回の場合は全ファイル）
        let changes = if self.metadata.is_none() {
            info!("Initial indexing - scanning all files");
            self.scan_all_files()?
        } else {
            self.git_detector.detect_changes_since(last_commit)?
        };
        
        let total_files = changes.len();
        info!("Detected {} file changes", total_files);
        
        // プログレス表示の準備
        if total_files > 10 {
            eprintln!("🚀 Processing {} files...", total_files);
        }

        let mut result = DifferentialIndexResult {
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            symbols_added: 0,
            symbols_updated: 0,
            symbols_deleted: 0,
            duration: Duration::from_secs(0),
        };

        // 既存のCodeGraphを読み込むか新規作成
        let mut graph = self
            .storage
            .load_data::<CodeGraph>("graph")?
            .unwrap_or_else(CodeGraph::new);

        // ファイルごとに処理
        let mut new_file_hashes = HashMap::new();
        let mut processed_count = 0;

        for change in changes {
            // コンテンツハッシュを記録
            if let Some(ref hash) = change.content_hash {
                new_file_hashes.insert(change.path.clone(), hash.clone());
            }
            
            // プログレス表示
            processed_count += 1;
            if total_files > 10 && processed_count % 10 == 0 {
                eprintln!("  ⚡ Processed {}/{} files ({:.0}%)", 
                         processed_count, total_files, 
                         (processed_count as f64 / total_files as f64) * 100.0);
            }

            match change.status {
                FileChangeStatus::Added => {
                    result.files_added += 1;
                    let symbols = self.extract_symbols_from_file(&change.path)?;
                    debug!(
                        "Extracted {} symbols from {}",
                        symbols.len(),
                        change.path.display()
                    );
                    result.symbols_added += symbols.len();

                    // グラフにシンボルを追加
                    for symbol in symbols {
                        debug!("Adding symbol: {} ({})", symbol.name, symbol.id);
                        graph.add_symbol(symbol);
                    }

                    // 参照を検出してエッジを追加
                    self.add_references_to_graph(&mut graph, &change.path)?;
                }
                FileChangeStatus::Modified | FileChangeStatus::Renamed { .. } => {
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

                    // 参照を検出してエッジを追加
                    self.add_references_to_graph(&mut graph, &change.path)?;
                }
                FileChangeStatus::Deleted => {
                    result.files_deleted += 1;

                    // シンボルを削除
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

        // CodeGraphを保存
        debug!("Saving CodeGraph with {} symbols", graph.symbol_count());
        self.storage.save_data("graph", &graph)?;
        debug!("CodeGraph saved successfully");

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

    /// ファイルからシンボルを抽出
    fn extract_symbols_from_file(&mut self, path: &Path) -> Result<Vec<Symbol>> {
        // 常にLSPを使用し、失敗した場合はフォールバックを使用
        match self.extract_symbols_with_lsp(path) {
            Ok(symbols) => {
                debug!("Successfully extracted {} symbols using LSP from {}", 
                       symbols.len(), path.display());
                Ok(symbols)
            }
            Err(e) => {
                debug!("LSP extraction failed: {}, trying fallback", e);
                self.extract_symbols_with_fallback(path)
            }
        }
    }


    /// メタデータを更新
    fn update_metadata(&mut self) -> Result<()> {
        let hash_cache_path = self.project_root.join(".lsif-hash-cache.json");

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

        // メタデータをクリア
        self.metadata = None;

        // 全ファイルを変更として扱う
        self.index_differential()
    }

    /// プロジェクト内の全ファイルをスキャン
    fn scan_all_files(&self) -> Result<Vec<FileChange>> {
        let mut changes = Vec::new();

        for entry in walkdir::WalkDir::new(&self.project_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();

            // .gitディレクトリ、targetディレクトリなどを除外
            if self.should_exclude(path) {
                continue;
            }

            // 対象ファイルのみ処理
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx") {
                let content_hash = self.git_detector.calculate_file_hash(path).ok();
                changes.push(FileChange {
                    path: path.to_path_buf(),
                    status: FileChangeStatus::Added,
                    content_hash,
                });
            }
        }

        Ok(changes)
    }

    /// 除外すべきパスかどうかを判定
    fn should_exclude(&self, path: &Path) -> bool {
        for component in path.components() {
            if let Some(name) = component.as_os_str().to_str() {
                if matches!(
                    name,
                    ".git" | "target" | "node_modules" | ".idea" | ".vscode" | "tmp"
                ) {
                    return true;
                }
            }
        }
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
                            graph.add_edge(from_idx, to_idx, core::EdgeKind::Reference);
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
            range: core::Range {
                start: core::Position {
                    line: 0,
                    character: 0,
                },
                end: core::Position {
                    line: 5,
                    character: 10,
                },
            },
            documentation: None,
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

}
