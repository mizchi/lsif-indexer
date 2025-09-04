use crate::storage::IndexStorage;
/// 定義ベースの増分クローラー
///
/// 現在のファイルから開始し、定義ジャンプとリファレンスを使って
/// 関連ファイルを徐々にインデックスしていく
use anyhow::Result;
use lsif_core::{Position, Range, Symbol, SymbolKind};
use lsp::adapter::lsp::GenericLspClient;
use lsp::lsp_pool::LspClientPool;
use lsp_types;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// クロール対象のファイル（優先度付き）
#[derive(Clone, Debug)]
struct CrawlTarget {
    path: PathBuf,
    priority: i32, // 参照カウントなどから計算される優先度
    depth: u32,    // 開始ファイルからの深さ
}

impl PartialEq for CrawlTarget {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for CrawlTarget {}

impl PartialOrd for CrawlTarget {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CrawlTarget {
    fn cmp(&self, other: &Self) -> Ordering {
        // 優先度が高い方を優先（逆順）
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| self.depth.cmp(&other.depth)) // 深さが浅い方を優先
    }
}

pub struct DefinitionCrawler {
    storage: IndexStorage,
    lsp_pool: LspClientPool,
    project_root: PathBuf,
    max_depth: u32,
    max_files: usize,

    // 統計情報
    crawled_files: HashSet<PathBuf>,
    reference_counts: HashMap<PathBuf, i32>,
}

impl DefinitionCrawler {
    pub fn new(
        storage_path: &Path,
        project_root: &Path,
        max_depth: u32,
        max_files: usize,
    ) -> Result<Self> {
        let storage = IndexStorage::open(storage_path)?;
        let lsp_pool = LspClientPool::with_defaults();

        Ok(Self {
            storage,
            lsp_pool,
            project_root: project_root.to_path_buf(),
            max_depth,
            max_files,
            crawled_files: HashSet::new(),
            reference_counts: HashMap::new(),
        })
    }

    /// 単一ファイルから開始してクロール
    pub fn crawl_from_file(&mut self, start_file: &Path) -> Result<CrawlStats> {
        info!(
            "Starting definition-based crawl from: {}",
            start_file.display()
        );

        let mut queue = BinaryHeap::new();
        queue.push(CrawlTarget {
            path: start_file.to_path_buf(),
            priority: 100, // 開始ファイルは最高優先度
            depth: 0,
        });

        let mut stats = CrawlStats::default();

        while let Some(target) = queue.pop() {
            // 最大深度チェック
            if target.depth > self.max_depth {
                debug!(
                    "Skipping {} (depth {} > max {})",
                    target.path.display(),
                    target.depth,
                    self.max_depth
                );
                continue;
            }

            // 最大ファイル数チェック
            if self.crawled_files.len() >= self.max_files {
                info!("Reached max file limit: {}", self.max_files);
                break;
            }

            // 既にクロール済みならスキップ
            if self.crawled_files.contains(&target.path) {
                continue;
            }

            // ファイルをインデックス
            match self.index_single_file(&target.path, target.depth) {
                Ok(file_stats) => {
                    stats.files_indexed += 1;
                    stats.symbols_found += file_stats.symbols_found;

                    // 関連ファイルをキューに追加
                    for related_file in file_stats.related_files {
                        let priority = self.calculate_priority(&related_file);
                        queue.push(CrawlTarget {
                            path: related_file,
                            priority,
                            depth: target.depth + 1,
                        });
                    }

                    self.crawled_files.insert(target.path.clone());

                    // 進捗表示
                    if stats.files_indexed % 10 == 0 {
                        info!(
                            "Progress: {} files indexed, {} symbols found",
                            stats.files_indexed, stats.symbols_found
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to index {}: {}", target.path.display(), e);
                    stats.files_failed += 1;
                }
            }
        }

        info!(
            "Crawl completed: {} files indexed, {} symbols found",
            stats.files_indexed, stats.symbols_found
        );

        Ok(stats)
    }

    /// 複数ファイルから開始してクロール（ホットスポット検出）
    pub fn crawl_from_files(&mut self, start_files: Vec<PathBuf>) -> Result<CrawlStats> {
        info!(
            "Starting definition-based crawl from {} files",
            start_files.len()
        );

        let mut queue = BinaryHeap::new();

        // 開始ファイル群を優先度付きでキューに追加
        for (idx, file) in start_files.into_iter().enumerate() {
            queue.push(CrawlTarget {
                path: file,
                priority: 100 - idx as i32, // 順番に従って優先度を設定
                depth: 0,
            });
        }

        // 同じロジックで処理
        let mut stats = CrawlStats::default();

        while let Some(target) = queue.pop() {
            if target.depth > self.max_depth || self.crawled_files.len() >= self.max_files {
                break;
            }

            if self.crawled_files.contains(&target.path) {
                continue;
            }

            match self.index_single_file(&target.path, target.depth) {
                Ok(file_stats) => {
                    stats.files_indexed += 1;
                    stats.symbols_found += file_stats.symbols_found;

                    for related_file in file_stats.related_files {
                        let priority = self.calculate_priority(&related_file);
                        queue.push(CrawlTarget {
                            path: related_file,
                            priority,
                            depth: target.depth + 1,
                        });
                    }

                    self.crawled_files.insert(target.path.clone());
                }
                Err(e) => {
                    warn!("Failed to index {}: {}", target.path.display(), e);
                    stats.files_failed += 1;
                }
            }
        }

        Ok(stats)
    }

    /// 単一ファイルをインデックスし、関連ファイルを発見
    fn index_single_file(&mut self, file_path: &Path, depth: u32) -> Result<FileIndexStats> {
        debug!("Indexing file: {} (depth: {})", file_path.display(), depth);

        // LSPクライアントを取得または作成
        let client = self
            .lsp_pool
            .get_or_create_client(file_path, &self.project_root)?;
        let mut client_guard = client.lock().unwrap();

        // ファイル内のシンボルを取得
        let symbols = self.extract_symbols(&mut client_guard, file_path)?;
        let symbol_count = symbols.len();

        // シンボルを保存
        for symbol in &symbols {
            // add_symbolメソッドがない場合は、グラフに追加する方法を使用
            // self.storage.save_data("symbols", &symbols)?;
        }

        // 関連ファイルを発見
        let mut related_files = HashSet::new();

        // 各シンボルの定義と参照を調査
        for symbol in &symbols {
            // 定義元を取得
            if let Ok(definitions) =
                self.get_definitions(&mut client_guard, file_path, &symbol.range.start)
            {
                for def_path in definitions {
                    if def_path != *file_path && self.is_project_file(&def_path) {
                        related_files.insert(def_path.clone());
                        *self.reference_counts.entry(def_path).or_insert(0) += 1;
                    }
                }
            }

            // 参照を取得（深さが浅い場合のみ）
            if depth < 2 {
                // 参照は深さ2まで
                if let Ok(references) =
                    self.get_references(&mut client_guard, file_path, &symbol.range.start)
                {
                    for ref_path in references {
                        if ref_path != *file_path && self.is_project_file(&ref_path) {
                            related_files.insert(ref_path.clone());
                            *self.reference_counts.entry(ref_path).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        debug!(
            "Found {} symbols and {} related files",
            symbol_count,
            related_files.len()
        );

        Ok(FileIndexStats {
            symbols_found: symbol_count,
            related_files: related_files.into_iter().collect(),
        })
    }

    /// LSPを使ってシンボルを抽出
    fn extract_symbols(
        &self,
        client: &mut GenericLspClient,
        file_path: &Path,
    ) -> Result<Vec<Symbol>> {
        // 簡易実装：フォールバックインデクサーを使用
        use lsp::fallback_indexer::FallbackIndexer;

        let fallback = FallbackIndexer::from_extension(file_path)
            .ok_or_else(|| anyhow::anyhow!("Unsupported file type"))?;

        let symbols = fallback.extract_symbols(file_path)?;

        // LSPシンボルをcore::Symbolに変換
        let mut result = Vec::new();
        for doc_symbol in symbols {
            result.push(Symbol {
                id: format!(
                    "{}#{}:{}",
                    file_path.display(),
                    doc_symbol.range.start.line,
                    doc_symbol.name
                ),
                name: doc_symbol.name,
                kind: Self::convert_symbol_kind(doc_symbol.kind),
                file_path: file_path.to_string_lossy().to_string(),
                range: Range {
                    start: Position {
                        line: doc_symbol.range.start.line,
                        character: doc_symbol.range.start.character,
                    },
                    end: Position {
                        line: doc_symbol.range.end.line,
                        character: doc_symbol.range.end.character,
                    },
                },
                documentation: doc_symbol.detail,
                detail: None,
            });
        }

        Ok(result)
    }

    /// 定義元を取得
    fn get_definitions(
        &self,
        client: &mut GenericLspClient,
        file_path: &Path,
        position: &Position,
    ) -> Result<Vec<PathBuf>> {
        let lsp_position = lsp_types::Position {
            line: position.line,
            character: position.character,
        };

        // get_definitionメソッドがない場合は、send_requestを使用
        let file_uri = format!("file://{}", file_path.display());
        // TODO: 実際のLSP定義ジャンプAPIを呼び出す
        let locations: Vec<lsp_types::Location> = Vec::new();

        let mut paths = Vec::new();
        for location in locations {
            if let Ok(path) = location.uri.to_file_path() {
                paths.push(path);
            }
        }

        Ok(paths)
    }

    /// 参照を取得
    fn get_references(
        &self,
        client: &mut GenericLspClient,
        file_path: &Path,
        position: &Position,
    ) -> Result<Vec<PathBuf>> {
        let lsp_position = lsp_types::Position {
            line: position.line,
            character: position.character,
        };

        // get_referencesメソッドがない場合は、send_requestを使用
        let file_uri = format!("file://{}", file_path.display());
        // TODO: 実際のLSP参照検索APIを呼び出す
        let locations: Vec<lsp_types::Location> = Vec::new();

        let mut paths = Vec::new();
        for location in locations {
            if let Ok(path) = location.uri.to_file_path() {
                paths.push(path);
            }
        }

        Ok(paths)
    }

    /// ファイルがプロジェクト内かチェック
    fn is_project_file(&self, path: &Path) -> bool {
        path.starts_with(&self.project_root)
    }

    /// ファイルの優先度を計算（参照カウントベース）
    fn calculate_priority(&self, file_path: &Path) -> i32 {
        self.reference_counts.get(file_path).copied().unwrap_or(1)
    }

    /// LSPシンボル種別を変換
    fn convert_symbol_kind(lsp_kind: lsp_types::SymbolKind) -> SymbolKind {
        match lsp_kind {
            lsp_types::SymbolKind::FUNCTION | lsp_types::SymbolKind::METHOD => SymbolKind::Function,
            lsp_types::SymbolKind::CLASS | lsp_types::SymbolKind::STRUCT => SymbolKind::Class,
            lsp_types::SymbolKind::INTERFACE => SymbolKind::Interface,
            lsp_types::SymbolKind::ENUM => SymbolKind::Enum,
            lsp_types::SymbolKind::VARIABLE | lsp_types::SymbolKind::PROPERTY => {
                SymbolKind::Variable
            }
            lsp_types::SymbolKind::CONSTANT => SymbolKind::Constant,
            _ => SymbolKind::Variable,
        }
    }
}

/// クロール統計
#[derive(Debug, Default)]
pub struct CrawlStats {
    pub files_indexed: usize,
    pub files_failed: usize,
    pub symbols_found: usize,
}

/// ファイルインデックス統計
#[derive(Debug)]
struct FileIndexStats {
    symbols_found: usize,
    related_files: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crawl_target_ordering() {
        let t1 = CrawlTarget {
            path: PathBuf::from("a.rs"),
            priority: 10,
            depth: 1,
        };

        let t2 = CrawlTarget {
            path: PathBuf::from("b.rs"),
            priority: 20,
            depth: 1,
        };

        let t3 = CrawlTarget {
            path: PathBuf::from("c.rs"),
            priority: 10,
            depth: 2,
        };

        // 優先度が高い方が先に来る
        assert!(t2 < t1);
        // 優先度が同じなら深さが浅い方が先
        assert!(t1 < t3);
    }
}
