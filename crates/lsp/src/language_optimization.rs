use std::path::Path;
use std::collections::HashMap;
use tracing::{info, debug};
use lsp_types::DocumentSymbol;

use crate::Language;
use crate::fallback_indexer::FallbackIndexer;

/// 言語特有の最適化戦略を提供するトレイト
pub trait LanguageOptimization: Send + Sync {
    /// この言語で並列処理を有効にするべきか
    fn should_parallelize(&self) -> bool {
        true
    }
    
    /// 並列処理時の最適なチャンクサイズ
    fn optimal_chunk_size(&self) -> usize {
        10
    }
    
    /// ファイルをキャッシュする価値があるか判定
    fn should_cache_file(&self, path: &Path) -> bool {
        // デフォルトでは大きなファイルのみキャッシュ
        if let Ok(metadata) = std::fs::metadata(path) {
            metadata.len() > 10_000 // 10KB以上
        } else {
            false
        }
    }
    
    /// シンボル抽出前の前処理
    fn preprocess_file(&self, content: &str) -> String {
        content.to_string()
    }
    
    /// バッチ処理に適しているか
    fn supports_batch_processing(&self) -> bool {
        false
    }
    
    /// インクリメンタル解析が可能か
    fn supports_incremental_parsing(&self) -> bool {
        false
    }
    
    /// 言語固有の高速シンボル抽出（オプション）
    fn fast_symbol_extraction(&self, _path: &Path) -> Option<Vec<DocumentSymbol>> {
        None
    }
    
    /// この言語でLSPを使用すべきか
    fn prefer_lsp(&self) -> bool {
        true
    }
    
    /// LSPのタイムアウト設定（ミリ秒）
    fn lsp_timeout_ms(&self) -> u64 {
        2000
    }
    
    /// 優先すべきLSPサーバー名を返す
    fn preferred_lsp_server(&self) -> Option<&'static str> {
        None
    }
    
    /// ワークスペースシンボル検索をキャッシュすべきか
    fn should_cache_workspace_symbols(&self) -> bool {
        true
    }
    
    /// ファイルをスキップすべきか判定
    fn should_skip_file(&self, path: &Path) -> bool {
        // デフォルトでは生成ファイルや巨大ファイルをスキップ
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // 生成ファイルのパターン
            if name.ends_with(".min.js") || 
               name.ends_with(".min.css") ||
               name.contains(".generated.") ||
               name.contains(".pb.go") ||  // Protocol Buffers
               name.contains("_pb2.py") {  // Protocol Buffers Python
                return true;
            }
        }
        
        // 100KB以上のファイルをスキップ
        if let Ok(metadata) = std::fs::metadata(path) {
            metadata.len() > 100_000
        } else {
            false
        }
    }
}

/// Rust言語の最適化戦略
pub struct RustOptimization;

impl LanguageOptimization for RustOptimization {
    fn should_parallelize(&self) -> bool {
        true // Rustは並列処理に適している
    }
    
    fn optimal_chunk_size(&self) -> usize {
        20 // モジュール構造を考慮して大きめのチャンク
    }
    
    fn prefer_lsp(&self) -> bool {
        true // rust-analyzerは高性能
    }
    
    fn lsp_timeout_ms(&self) -> u64 {
        5000 // rust-analyzerは初回起動が遅い
    }
    
    fn should_skip_file(&self, path: &Path) -> bool {
        if let Some(path_str) = path.to_str() {
            // targetディレクトリ内のファイルはスキップ
            if path_str.contains("/target/") || path_str.contains("\\target\\") 
                || path_str.starts_with("target/") || path_str.starts_with("target\\") {
                return true;
            }
        }
        false
    }
}

/// Go言語の最適化戦略
pub struct GoOptimization;

impl LanguageOptimization for GoOptimization {
    fn should_parallelize(&self) -> bool {
        true
    }
    
    fn optimal_chunk_size(&self) -> usize {
        15 // パッケージ単位での処理を考慮
    }
    
    fn prefer_lsp(&self) -> bool {
        true // goplsは高速
    }
    
    fn should_skip_file(&self, path: &Path) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // vendorディレクトリや生成ファイルをスキップ
            if name.ends_with(".pb.go") || name.ends_with("_test.go") {
                return true;
            }
        }
        if let Some(path_str) = path.to_str() {
            if path_str.contains("/vendor/") || path_str.contains("\\vendor\\") {
                return true;
            }
        }
        false
    }
}

/// Python言語の最適化戦略
pub struct PythonOptimization;

impl LanguageOptimization for PythonOptimization {
    fn should_parallelize(&self) -> bool {
        true
    }
    
    fn optimal_chunk_size(&self) -> usize {
        10 // GILの影響を考慮して小さめ
    }
    
    fn prefer_lsp(&self) -> bool {
        false // Pythonは正規表現ベースの方が速い場合が多い
    }
    
    fn fast_symbol_extraction(&self, path: &Path) -> Option<Vec<DocumentSymbol>> {
        // 簡易的な正規表現ベースの抽出を使用
        if let Ok(fallback) = FallbackIndexer::for_python() {
            fallback.extract_symbols(path).ok()
        } else {
            None
        }
    }
    
    fn should_skip_file(&self, path: &Path) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // キャッシュディレクトリや生成ファイルをスキップ
            if name.starts_with("__pycache__") || 
               name.ends_with(".pyc") ||
               name.ends_with("_pb2.py") {
                return true;
            }
        }
        false
    }
}

/// TypeScript/JavaScript言語の最適化戦略
pub struct TypeScriptOptimization {
    is_javascript: bool,
}

impl Default for TypeScriptOptimization {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeScriptOptimization {
    pub fn new() -> Self {
        Self { is_javascript: false }
    }
    
    pub fn javascript() -> Self {
        Self { is_javascript: true }
    }
}

impl LanguageOptimization for TypeScriptOptimization {
    fn should_parallelize(&self) -> bool {
        true
    }
    
    fn optimal_chunk_size(&self) -> usize {
        5 // node_modulesの影響で小さめに
    }
    
    fn prefer_lsp(&self) -> bool {
        !self.is_javascript // TypeScriptはLSP、JavaScriptは正規表現
    }
    
    fn lsp_timeout_ms(&self) -> u64 {
        3000 // tsserverは中程度の起動時間
    }
    
    fn should_skip_file(&self, path: &Path) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // minifyされたファイルや生成ファイルをスキップ
            if name.ends_with(".min.js") || 
               name.ends_with(".min.mjs") ||
               name.ends_with(".bundle.js") ||
               name.ends_with(".d.ts") && name != "index.d.ts" {
                return true;
            }
        }
        if let Some(path_str) = path.to_str() {
            // node_modulesとdistディレクトリをスキップ
            if path_str.contains("/node_modules/") || 
               path_str.contains("\\node_modules\\") ||
               path_str.starts_with("node_modules/") ||
               path_str.starts_with("node_modules\\") ||
               path_str.contains("/dist/") ||
               path_str.contains("\\dist\\") ||
               path_str.starts_with("dist/") ||
               path_str.starts_with("dist\\") ||
               path_str.contains("/build/") ||
               path_str.contains("\\build\\") {
                return true;
            }
        }
        false
    }
    
    fn fast_symbol_extraction(&self, path: &Path) -> Option<Vec<DocumentSymbol>> {
        if self.is_javascript {
            // JavaScriptは正規表現ベースの高速抽出
            if let Ok(fallback) = FallbackIndexer::for_javascript() {
                fallback.extract_symbols(path).ok()
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// 言語最適化戦略のファクトリ
pub struct OptimizationStrategy {
    strategies: HashMap<String, Box<dyn LanguageOptimization>>,
}

impl Default for OptimizationStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl OptimizationStrategy {
    pub fn new() -> Self {
        let mut strategies: HashMap<String, Box<dyn LanguageOptimization>> = HashMap::new();
        
        strategies.insert("rust".to_string(), Box::new(RustOptimization));
        strategies.insert("go".to_string(), Box::new(GoOptimization));
        strategies.insert("python".to_string(), Box::new(PythonOptimization));
        strategies.insert("typescript".to_string(), Box::new(TypeScriptOptimization::new()));
        strategies.insert("javascript".to_string(), Box::new(TypeScriptOptimization::javascript()));
        
        Self { strategies }
    }
    
    /// 言語に応じた最適化戦略を取得
    pub fn get_strategy(&self, language: &Language) -> Option<&dyn LanguageOptimization> {
        let lang_key = match language {
            Language::Rust => "rust",
            Language::Go => "go",
            Language::Python => "python",
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Unknown => return None,
        };
        
        self.strategies.get(lang_key).map(|s| s.as_ref())
    }
    
    /// ファイルパスから最適化戦略を取得
    pub fn get_strategy_for_file(&self, path: &Path) -> Option<&dyn LanguageOptimization> {
        use crate::language_detector::detect_file_language;
        let language = detect_file_language(path);
        self.get_strategy(&language)
    }
    
    /// プロジェクト全体の最適化設定を決定
    pub fn analyze_project(&self, project_path: &Path) -> ProjectOptimizationConfig {
        use crate::language_detector::detect_project_language;
        
        let main_language = detect_project_language(project_path);
        info!("Detected main project language: {:?}", main_language);
        
        let mut config = ProjectOptimizationConfig::default();
        
        if let Some(strategy) = self.get_strategy(&main_language) {
            config.use_parallel = strategy.should_parallelize();
            config.chunk_size = strategy.optimal_chunk_size();
            config.prefer_lsp = strategy.prefer_lsp();
            config.lsp_timeout_ms = strategy.lsp_timeout_ms();
            
            debug!("Project optimization config: parallel={}, chunk_size={}, prefer_lsp={}", 
                   config.use_parallel, config.chunk_size, config.prefer_lsp);
        }
        
        config.main_language = main_language;
        config
    }
}

/// プロジェクト全体の最適化設定
#[derive(Debug, Clone)]
pub struct ProjectOptimizationConfig {
    pub main_language: Language,
    pub use_parallel: bool,
    pub chunk_size: usize,
    pub prefer_lsp: bool,
    pub lsp_timeout_ms: u64,
}

impl Default for ProjectOptimizationConfig {
    fn default() -> Self {
        Self {
            main_language: Language::Unknown,
            use_parallel: true,
            chunk_size: 10,
            prefer_lsp: true,
            lsp_timeout_ms: 2000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    
    #[test]
    fn test_rust_optimization() {
        let opt = RustOptimization;
        assert!(opt.should_parallelize());
        assert_eq!(opt.optimal_chunk_size(), 20);
        assert!(opt.prefer_lsp());
        
        // targetディレクトリ内のファイルはスキップ
        assert!(opt.should_skip_file(Path::new("target/debug/main.rs")));
        assert!(!opt.should_skip_file(Path::new("src/main.rs")));
    }
    
    #[test]
    fn test_python_optimization() {
        let opt = PythonOptimization;
        assert!(opt.should_parallelize());
        assert_eq!(opt.optimal_chunk_size(), 10);
        assert!(!opt.prefer_lsp()); // Pythonは正規表現優先
        
        // __pycache__やpycファイルはスキップ
        assert!(opt.should_skip_file(Path::new("__pycache__/main.pyc")));
        assert!(!opt.should_skip_file(Path::new("main.py")));
    }
    
    #[test]
    fn test_typescript_optimization() {
        let ts_opt = TypeScriptOptimization::new();
        assert!(ts_opt.prefer_lsp());
        
        let js_opt = TypeScriptOptimization::javascript();
        assert!(!js_opt.prefer_lsp());
        
        // node_modulesやminファイルはスキップ
        assert!(ts_opt.should_skip_file(Path::new("node_modules/lib/index.js")));
        assert!(ts_opt.should_skip_file(Path::new("dist/bundle.min.js")));
        assert!(!ts_opt.should_skip_file(Path::new("src/index.ts")));
    }
    
    #[test]
    fn test_optimization_factory() {
        let factory = OptimizationStrategy::new();
        
        assert!(factory.get_strategy(&Language::Rust).is_some());
        assert!(factory.get_strategy(&Language::Python).is_some());
        assert!(factory.get_strategy(&Language::Unknown).is_none());
    }
    
    #[test]
    fn test_project_optimization_config() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]").unwrap();
        
        let factory = OptimizationStrategy::new();
        let config = factory.analyze_project(temp_dir.path());
        
        assert_eq!(config.main_language, Language::Rust);
        assert!(config.use_parallel);
        assert_eq!(config.chunk_size, 20);
        assert!(config.prefer_lsp);
    }
}