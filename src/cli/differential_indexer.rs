use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, info};

use crate::cli::git_diff::{FileChange, FileChangeStatus, GitDiffDetector};
use crate::cli::storage::IndexStorage;
use crate::core::{CodeGraph, Symbol, SymbolKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir;

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

        Ok(Self {
            storage,
            git_detector,
            project_root,
            metadata,
        })
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
        info!("Detected {} file changes", changes.len());

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
        let mut graph = self.storage.load_data::<CodeGraph>("graph")?
            .unwrap_or_else(CodeGraph::new);

        // ファイルごとに処理
        let mut new_file_hashes = HashMap::new();

        for change in changes {
            // コンテンツハッシュを記録
            if let Some(ref hash) = change.content_hash {
                new_file_hashes.insert(change.path.clone(), hash.clone());
            }

            match change.status {
                FileChangeStatus::Added => {
                    result.files_added += 1;
                    let symbols = self.extract_symbols_from_file(&change.path)?;
                    debug!("Extracted {} symbols from {}", symbols.len(), change.path.display());
                    result.symbols_added += symbols.len();
                    
                    // グラフにシンボルを追加
                    for symbol in symbols {
                        debug!("Adding symbol: {} ({})", symbol.name, symbol.id);
                        graph.add_symbol(symbol);
                    }
                }
                FileChangeStatus::Modified | FileChangeStatus::Renamed { .. } => {
                    result.files_modified += 1;
                    
                    // 既存のシンボルを削除
                    let path_str = change.path.to_string_lossy();
                    let old_symbols: Vec<_> = graph.get_all_symbols()
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
                FileChangeStatus::Deleted => {
                    result.files_deleted += 1;
                    
                    // シンボルを削除
                    let path_str = change.path.to_string_lossy();
                    let old_symbols: Vec<_> = graph.get_all_symbols()
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
                            let old_symbols: Vec<_> = graph.get_all_symbols()
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
            debug!("Updated file hashes: {} total", metadata.file_content_hashes.len());
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

    /// ファイルからシンボルを抽出（簡易版）
    fn extract_symbols_from_file(&self, path: &Path) -> Result<Vec<Symbol>> {
        let mut symbols = Vec::new();

        // ファイル拡張子で言語を判定
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        debug!("Processing file: {} (extension: {})", path.display(), extension);

        match extension {
            "rs" => {
                // Rustファイルの解析
                let content = std::fs::read_to_string(path)?;
                let rust_symbols = self.extract_rust_symbols(path, &content)?;
                debug!("Found {} Rust symbols in {}", rust_symbols.len(), path.display());
                symbols.extend(rust_symbols);
            }
            "ts" | "tsx" | "js" | "jsx" => {
                // TypeScript/JavaScriptファイルの解析
                let content = std::fs::read_to_string(path)?;
                let ts_symbols = self.extract_typescript_symbols(path, &content)?;
                debug!("Found {} TypeScript symbols in {}", ts_symbols.len(), path.display());
                symbols.extend(ts_symbols);
            }
            _ => {
                debug!("Unsupported file type: {}", extension);
            }
        }

        Ok(symbols)
    }

    /// Rustシンボルを抽出（簡易版）
    fn extract_rust_symbols(&self, path: &Path, content: &str) -> Result<Vec<Symbol>> {
        let mut symbols = Vec::new();
        let path_str = path.to_string_lossy().to_string();

        for (line_no, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // 関数定義
            if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                if let Some(name) = extract_function_name(trimmed) {
                    symbols.push(Symbol {
                        id: format!("{}#{}:{}", path_str, line_no + 1, name),
                        kind: SymbolKind::Function,
                        name: name.to_string(),
                        file_path: path_str.clone(),
                        range: crate::core::Range {
                            start: crate::core::Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: crate::core::Position {
                                line: line_no as u32,
                                character: line.len() as u32,
                            },
                        },
                        documentation: None,
                    });
                }
            }

            // 構造体定義
            if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") {
                if let Some(name) = extract_struct_name(trimmed) {
                    symbols.push(Symbol {
                        id: format!("{}#{}:{}", path_str, line_no + 1, name),
                        kind: SymbolKind::Class,
                        name: name.to_string(),
                        file_path: path_str.clone(),
                        range: crate::core::Range {
                            start: crate::core::Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: crate::core::Position {
                                line: line_no as u32,
                                character: line.len() as u32,
                            },
                        },
                        documentation: None,
                    });
                }
            }
        }

        Ok(symbols)
    }

    /// TypeScriptシンボルを抽出（簡易版）
    fn extract_typescript_symbols(&self, path: &Path, content: &str) -> Result<Vec<Symbol>> {
        let mut symbols = Vec::new();
        let path_str = path.to_string_lossy().to_string();

        for (line_no, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // 関数定義
            if trimmed.starts_with("function ") || trimmed.starts_with("export function ") {
                if let Some(name) = extract_ts_function_name(trimmed) {
                    symbols.push(Symbol {
                        id: format!("{}#{}:{}", path_str, line_no + 1, name),
                        kind: SymbolKind::Function,
                        name: name.to_string(),
                        file_path: path_str.clone(),
                        range: crate::core::Range {
                            start: crate::core::Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: crate::core::Position {
                                line: line_no as u32,
                                character: line.len() as u32,
                            },
                        },
                        documentation: None,
                    });
                }
            }

            // クラス定義
            if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
                if let Some(name) = extract_ts_class_name(trimmed) {
                    symbols.push(Symbol {
                        id: format!("{}#{}:{}", path_str, line_no + 1, name),
                        kind: SymbolKind::Class,
                        name: name.to_string(),
                        file_path: path_str.clone(),
                        range: crate::core::Range {
                            start: crate::core::Position {
                                line: line_no as u32,
                                character: 0,
                            },
                            end: crate::core::Position {
                                line: line_no as u32,
                                character: line.len() as u32,
                            },
                        },
                        documentation: None,
                    });
                }
            }
        }

        Ok(symbols)
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
        let storage_metadata = crate::cli::storage::IndexMetadata {
            format: crate::cli::storage::IndexFormat::Lsif,
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
                if matches!(name, ".git" | "target" | "node_modules" | ".idea" | ".vscode" | "tmp") {
                    return true;
                }
            }
        }
        false
    }
}

// ヘルパー関数
fn extract_function_name(line: &str) -> Option<&str> {
    let line = line.trim_start_matches("pub ").trim_start_matches("fn ");
    line.split(&['(', '<'][..]).next()
}

fn extract_struct_name(line: &str) -> Option<&str> {
    let line = line
        .trim_start_matches("pub ")
        .trim_start_matches("struct ");
    line.split(&[' ', '<', '{'][..]).next()
}

fn extract_ts_function_name(line: &str) -> Option<&str> {
    let line = line
        .trim_start_matches("export ")
        .trim_start_matches("function ");
    line.split(&['(', '<'][..]).next()
}

fn extract_ts_class_name(line: &str) -> Option<&str> {
    let line = line
        .trim_start_matches("export ")
        .trim_start_matches("class ");
    line.split(&[' ', '<', '{'][..]).next()
}
