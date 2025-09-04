/// シンボル抽出戦略パターン
use anyhow::Result;
use lsif_core::Symbol;
use std::path::Path;
use tracing::{debug, info, warn};

/// シンボル抽出戦略のトレイト
pub trait SymbolExtractionStrategy: Send + Sync {
    /// 戦略の名前
    fn name(&self) -> &str;

    /// この戦略が指定されたファイルをサポートするか
    fn supports(&self, path: &Path) -> bool;

    /// シンボルを抽出
    fn extract(&self, path: &Path) -> Result<Vec<Symbol>>;

    /// 優先度（高いほど優先される）
    fn priority(&self) -> u32 {
        50 // デフォルト優先度
    }
}

/// LSPベースの抽出戦略
pub struct LspExtractionStrategy {
    lsp_pool: std::sync::Arc<std::sync::Mutex<lsp::lsp_pool::LspClientPool>>,
    project_root: std::path::PathBuf,
}

impl LspExtractionStrategy {
    pub fn new(
        lsp_pool: std::sync::Arc<std::sync::Mutex<lsp::lsp_pool::LspClientPool>>,
        project_root: std::path::PathBuf,
    ) -> Self {
        Self {
            lsp_pool,
            project_root,
        }
    }
}

impl SymbolExtractionStrategy for LspExtractionStrategy {
    fn name(&self) -> &str {
        "LSP"
    }

    fn supports(&self, path: &Path) -> bool {
        // LSPがサポートする拡張子をチェック
        use lsp::adapter::lsp::get_language_id;
        let language_id = get_language_id(path).unwrap_or_else(|| "unknown".to_string());

        // LSPプールに問い合わせて、この言語がサポートされているか確認
        if let Ok(pool) = self.lsp_pool.lock() {
            pool.has_capability_for_language(&language_id, "textDocument/documentSymbol")
        } else {
            false
        }
    }

    fn extract(&self, path: &Path) -> Result<Vec<Symbol>> {
        use std::fs::canonicalize;

        info!("LSP extraction for: {}", path.display());

        // ファイルの絶対パスを取得
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            canonicalize(path)?
        };

        let file_uri = format!("file://{}", absolute_path.display());

        // LSPプールからクライアントを取得
        let pool = self
            .lsp_pool
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock LSP pool: {}", e))?;

        let client_arc = pool.get_or_create_client(path, &self.project_root)?;
        let mut client = client_arc
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock LSP client: {}", e))?;

        // ドキュメントシンボルを取得
        let lsp_symbols = client.get_document_symbols(&file_uri)?;

        // LSPシンボルをコアのSymbol型に変換
        Ok(convert_lsp_symbols_to_core(&lsp_symbols, path))
    }

    fn priority(&self) -> u32 {
        100 // LSPは最優先
    }
}

// 正規表現ベースのフォールバック実装は削除
// LSPベースの実装のみを使用

/// チェーンオブレスポンシビリティパターンでの抽出器
pub struct ChainedSymbolExtractor {
    strategies: Vec<Box<dyn SymbolExtractionStrategy>>,
}

impl ChainedSymbolExtractor {
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    /// 戦略を追加
    pub fn add_strategy(mut self, strategy: Box<dyn SymbolExtractionStrategy>) -> Self {
        self.strategies.push(strategy);
        // 優先度順にソート
        self.strategies
            .sort_by_key(|s| std::cmp::Reverse(s.priority()));
        self
    }

    /// シンボルを抽出（最初に成功した戦略の結果を返す）
    pub fn extract(&self, path: &Path) -> Result<Vec<Symbol>> {
        for strategy in &self.strategies {
            if !strategy.supports(path) {
                debug!(
                    "Strategy '{}' does not support {}",
                    strategy.name(),
                    path.display()
                );
                continue;
            }

            debug!(
                "Trying strategy '{}' for {}",
                strategy.name(),
                path.display()
            );

            match strategy.extract(path) {
                Ok(symbols) if !symbols.is_empty() => {
                    info!(
                        "Strategy '{}' extracted {} symbols from {}",
                        strategy.name(),
                        symbols.len(),
                        path.display()
                    );
                    return Ok(symbols);
                }
                Ok(_) => {
                    debug!(
                        "Strategy '{}' returned no symbols for {}",
                        strategy.name(),
                        path.display()
                    );
                }
                Err(e) => {
                    warn!(
                        "Strategy '{}' failed for {}: {}",
                        strategy.name(),
                        path.display(),
                        e
                    );
                }
            }
        }

        // すべての戦略が失敗した場合
        warn!("All extraction strategies failed for {}", path.display());
        Ok(Vec::new())
    }

    /// 戦略の数を取得
    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
    }
}

impl Default for ChainedSymbolExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// LSPシンボルをコアのSymbol型に変換（ヘルパー関数）
fn convert_lsp_symbols_to_core(
    lsp_symbols: &[lsp_types::DocumentSymbol],
    path: &Path,
) -> Vec<Symbol> {
    use lsif_core::{Position, Range};

    let mut symbols = Vec::new();
    let file_path = path.to_string_lossy().to_string();

    fn convert_symbol(
        symbol: &lsp_types::DocumentSymbol,
        file_path: &str,
        parent_name: Option<&str>,
        results: &mut Vec<Symbol>,
    ) {
        let full_name = if let Some(parent) = parent_name {
            format!("{}::{}", parent, symbol.name)
        } else {
            symbol.name.clone()
        };

        let core_symbol = Symbol {
            id: format!("{}#{}:{}", file_path, symbol.range.start.line, full_name),
            name: symbol.name.clone(),
            kind: convert_symbol_kind(symbol.kind),
            file_path: file_path.to_string(),
            range: Range {
                start: Position {
                    line: symbol.range.start.line,
                    character: symbol.range.start.character,
                },
                end: Position {
                    line: symbol.range.end.line,
                    character: symbol.range.end.character,
                },
            },
            documentation: symbol.detail.clone(),
            detail: None,
        };

        results.push(core_symbol);

        // 子シンボルを再帰的に処理
        if let Some(children) = &symbol.children {
            for child in children {
                convert_symbol(child, file_path, Some(&full_name), results);
            }
        }
    }

    for symbol in lsp_symbols {
        convert_symbol(symbol, &file_path, None, &mut symbols);
    }

    symbols
}

/// LSP SymbolKindをコアのSymbolKindに変換
fn convert_symbol_kind(lsp_kind: lsp_types::SymbolKind) -> lsif_core::SymbolKind {
    use lsif_core::SymbolKind;
    use lsp_types::SymbolKind as LspKind;

    match lsp_kind {
        LspKind::FILE => SymbolKind::File,
        LspKind::MODULE => SymbolKind::Module,
        LspKind::NAMESPACE => SymbolKind::Namespace,
        LspKind::PACKAGE => SymbolKind::Package,
        LspKind::CLASS => SymbolKind::Class,
        LspKind::METHOD => SymbolKind::Method,
        LspKind::PROPERTY => SymbolKind::Property,
        LspKind::FIELD => SymbolKind::Field,
        LspKind::CONSTRUCTOR => SymbolKind::Constructor,
        LspKind::ENUM => SymbolKind::Enum,
        LspKind::INTERFACE => SymbolKind::Interface,
        LspKind::FUNCTION => SymbolKind::Function,
        LspKind::VARIABLE => SymbolKind::Variable,
        LspKind::CONSTANT => SymbolKind::Constant,
        LspKind::STRING => SymbolKind::String,
        LspKind::NUMBER => SymbolKind::Number,
        LspKind::BOOLEAN => SymbolKind::Boolean,
        LspKind::ARRAY => SymbolKind::Array,
        LspKind::OBJECT => SymbolKind::Object,
        LspKind::KEY => SymbolKind::Key,
        LspKind::NULL => SymbolKind::Null,
        LspKind::ENUM_MEMBER => SymbolKind::EnumMember,
        LspKind::STRUCT => SymbolKind::Struct,
        LspKind::EVENT => SymbolKind::Event,
        LspKind::OPERATOR => SymbolKind::Operator,
        LspKind::TYPE_PARAMETER => SymbolKind::TypeParameter,
        _ => SymbolKind::Unknown, // その他の未知のSymbolKind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chained_extractor() {
        // テスト用のダミー戦略
        struct DummyStrategy;
        impl SymbolExtractionStrategy for DummyStrategy {
            fn name(&self) -> &str {
                "Dummy"
            }
            fn supports(&self, _: &Path) -> bool {
                true
            }
            fn extract(&self, _: &Path) -> Result<Vec<Symbol>> {
                Ok(Vec::new())
            }
            fn priority(&self) -> u32 {
                50
            }
        }

        let extractor = ChainedSymbolExtractor::new().add_strategy(Box::new(DummyStrategy));

        assert_eq!(extractor.strategy_count(), 1);
    }

    #[test]
    fn test_strategy_priority_sorting() {
        struct TestStrategy(u32, &'static str);

        impl SymbolExtractionStrategy for TestStrategy {
            fn name(&self) -> &str {
                self.1
            }
            fn supports(&self, _: &Path) -> bool {
                true
            }
            fn extract(&self, _: &Path) -> Result<Vec<Symbol>> {
                Ok(Vec::new())
            }
            fn priority(&self) -> u32 {
                self.0
            }
        }

        let extractor = ChainedSymbolExtractor::new()
            .add_strategy(Box::new(TestStrategy(10, "Low")))
            .add_strategy(Box::new(TestStrategy(100, "High")))
            .add_strategy(Box::new(TestStrategy(50, "Medium")));

        assert_eq!(extractor.strategy_count(), 3);
        // 優先度順に並んでいることを確認
        assert_eq!(extractor.strategies[0].name(), "High");
        assert_eq!(extractor.strategies[1].name(), "Medium");
        assert_eq!(extractor.strategies[2].name(), "Low");
    }
}
