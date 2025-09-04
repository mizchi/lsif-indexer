use anyhow::{Context, Result};
use dashmap::DashMap;
use lsp_types::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::lsp_rpc_client::LspRpcClient;

/// LSPサーバーの設定
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    pub language_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub initialization_options: Option<Value>,
    pub workspace_folders: Vec<WorkspaceFolder>,
}

/// 言語ごとのLSPサーバー設定
pub struct LspServerRegistry {
    configs: HashMap<String, LspServerConfig>,
}

impl Default for LspServerRegistry {
    fn default() -> Self {
        let mut configs = HashMap::new();

        // Rust
        configs.insert(
            "rust".to_string(),
            LspServerConfig {
                language_id: "rust".to_string(),
                command: "rust-analyzer".to_string(),
                args: vec![],
                initialization_options: Some(serde_json::json!({
                    "cargo": {
                        "loadOutDirsFromCheck": true,
                        "runBuildScripts": true
                    },
                    "procMacro": {
                        "enable": true
                    }
                })),
                workspace_folders: vec![],
            },
        );

        // TypeScript/JavaScript
        configs.insert(
            "typescript".to_string(),
            LspServerConfig {
                language_id: "typescript".to_string(),
                command: "typescript-language-server".to_string(),
                args: vec!["--stdio".to_string()],
                initialization_options: None,
                workspace_folders: vec![],
            },
        );

        configs.insert(
            "javascript".to_string(),
            LspServerConfig {
                language_id: "javascript".to_string(),
                command: "typescript-language-server".to_string(),
                args: vec!["--stdio".to_string()],
                initialization_options: None,
                workspace_folders: vec![],
            },
        );

        // Python
        configs.insert(
            "python".to_string(),
            LspServerConfig {
                language_id: "python".to_string(),
                command: "pylsp".to_string(),
                args: vec![],
                initialization_options: None,
                workspace_folders: vec![],
            },
        );

        // Go
        configs.insert(
            "go".to_string(),
            LspServerConfig {
                language_id: "go".to_string(),
                command: "gopls".to_string(),
                args: vec![],
                initialization_options: None,
                workspace_folders: vec![],
            },
        );

        // Java
        configs.insert(
            "java".to_string(),
            LspServerConfig {
                language_id: "java".to_string(),
                command: "jdtls".to_string(),
                args: vec![],
                initialization_options: None,
                workspace_folders: vec![],
            },
        );

        // C/C++
        configs.insert(
            "cpp".to_string(),
            LspServerConfig {
                language_id: "cpp".to_string(),
                command: "clangd".to_string(),
                args: vec!["--background-index".to_string()],
                initialization_options: None,
                workspace_folders: vec![],
            },
        );

        Self { configs }
    }
}

impl LspServerRegistry {
    pub fn get_config(&self, language: &str) -> Option<&LspServerConfig> {
        self.configs.get(language)
    }

    pub fn detect_language(path: &Path) -> Option<String> {
        let ext = path.extension()?.to_str()?;

        match ext {
            "rs" => Some("rust".to_string()),
            "ts" | "tsx" => Some("typescript".to_string()),
            "js" | "jsx" | "mjs" => Some("javascript".to_string()),
            "py" | "pyi" => Some("python".to_string()),
            "go" => Some("go".to_string()),
            "java" => Some("java".to_string()),
            "c" | "cc" | "cpp" | "cxx" | "h" | "hpp" => Some("cpp".to_string()),
            "cs" => Some("csharp".to_string()),
            "rb" => Some("ruby".to_string()),
            "php" => Some("php".to_string()),
            "swift" => Some("swift".to_string()),
            "kt" | "kts" => Some("kotlin".to_string()),
            _ => None,
        }
    }
}

/// 統一されたLSPマネージャー
pub struct UnifiedLspManager {
    registry: Arc<LspServerRegistry>,
    clients: Arc<DashMap<String, Arc<RwLock<LspRpcClient>>>>,
    file_cache: Arc<crate::optimized_io::FileContentCache>,
}

impl Default for UnifiedLspManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedLspManager {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(LspServerRegistry::default()),
            clients: Arc::new(DashMap::new()),
            file_cache: Arc::new(crate::optimized_io::FileContentCache::default()),
        }
    }

    /// ファイルに対応するLSPクライアントを取得または作成
    pub async fn get_client_for_file(&self, file_path: &Path) -> Result<Arc<RwLock<LspRpcClient>>> {
        let language = LspServerRegistry::detect_language(file_path)
            .context("Failed to detect language for file")?;

        self.get_or_create_client(&language, file_path.parent().unwrap_or(Path::new(".")))
            .await
    }

    /// 言語に対応するLSPクライアントを取得または作成
    pub async fn get_or_create_client(
        &self,
        language: &str,
        workspace_root: &Path,
    ) -> Result<Arc<RwLock<LspRpcClient>>> {
        // 既存のクライアントを確認
        if let Some(client) = self.clients.get(language) {
            return Ok(client.clone());
        }

        // 新しいクライアントを作成
        let config = self
            .registry
            .get_config(language)
            .context(format!("No LSP configuration for language: {}", language))?;

        let client = self.create_lsp_client(config, workspace_root).await?;
        let client_arc = Arc::new(RwLock::new(client));

        self.clients
            .insert(language.to_string(), client_arc.clone());

        Ok(client_arc)
    }

    /// LSPクライアントを作成して初期化
    async fn create_lsp_client(
        &self,
        config: &LspServerConfig,
        workspace_root: &Path,
    ) -> Result<LspRpcClient> {
        info!(
            "Starting LSP server for {}: {}",
            config.language_id, config.command
        );

        // Create LSP RPC client
        let mut client =
            LspRpcClient::new(&config.command, &config.args, config.language_id.clone())?;

        // Initialize with workspace root
        let root_uri = Url::from_file_path(workspace_root)
            .map_err(|_| anyhow::anyhow!("Invalid workspace root path"))?;

        client
            .initialize(root_uri, config.initialization_options.clone())
            .await?;

        info!("LSP server initialized for {}", config.language_id);

        Ok(client)
    }

    /// ファイルのドキュメントシンボルを取得
    pub async fn get_document_symbols(&self, file_path: &Path) -> Result<Vec<DocumentSymbol>> {
        let client = self.get_client_for_file(file_path).await?;
        let client = client.read().await;

        // ファイル内容を取得（キャッシュ利用）
        let content = self.file_cache.get_or_read(file_path)?;

        // ドキュメントを開く
        let uri =
            Url::from_file_path(file_path).map_err(|_| anyhow::anyhow!("Invalid file path"))?;

        client.did_open(uri.clone(), content).await?;

        // シンボルを取得
        let symbols = client.document_symbols(uri.clone()).await?;

        // ドキュメントを閉じる
        client.did_close(uri).await?;

        Ok(symbols)
    }

    /// ワークスペースシンボルを検索
    pub async fn search_workspace_symbols(
        &self,
        _workspace_root: &Path,
        query: &str,
    ) -> Result<Vec<SymbolInformation>> {
        let mut all_symbols = Vec::new();

        // すべてのアクティブなクライアントでシンボルを検索
        for entry in self.clients.iter() {
            let client = entry.value();
            let client = client.read().await;

            match client.workspace_symbols(query).await {
                Ok(symbols) => all_symbols.extend(symbols),
                Err(e) => warn!(
                    "Failed to get workspace symbols from {}: {}",
                    entry.key(),
                    e
                ),
            }
        }

        Ok(all_symbols)
    }

    /// プロジェクト全体をインデックス
    pub async fn index_project(&self, project_root: &Path) -> Result<ProjectIndex> {
        let mut index = ProjectIndex::new();

        // プロジェクト内のすべてのファイルを検索
        let files = self.find_source_files(project_root)?;

        info!("Indexing {} files in project", files.len());

        // 並列処理でファイルをインデックス
        use rayon::prelude::*;
        let symbols_per_file: Vec<_> = files
            .par_iter()
            .filter_map(|file_path| {
                // 各ファイルのシンボルを取得
                match tokio::runtime::Handle::current()
                    .block_on(self.get_document_symbols(file_path))
                {
                    Ok(symbols) => Some((file_path.clone(), symbols)),
                    Err(e) => {
                        warn!("Failed to index {}: {}", file_path.display(), e);
                        None
                    }
                }
            })
            .collect();

        // インデックスに追加
        for (file_path, symbols) in symbols_per_file {
            index.add_file_symbols(file_path, symbols);
        }

        info!(
            "Indexed {} files with {} total symbols",
            index.file_count(),
            index.symbol_count()
        );

        Ok(index)
    }

    /// ソースファイルを検索
    fn find_source_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for entry in walkdir::WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && LspServerRegistry::detect_language(path).is_some() {
                files.push(path.to_path_buf());
            }
        }

        Ok(files)
    }

    /// すべてのLSPクライアントをシャットダウン
    pub async fn shutdown_all(&self) -> Result<()> {
        for entry in self.clients.iter() {
            let client = entry.value();
            let mut client = client.write().await;

            if let Err(e) = client.shutdown().await {
                error!("Failed to shutdown LSP client for {}: {}", entry.key(), e);
            }
        }

        self.clients.clear();
        Ok(())
    }
}

/// プロジェクトインデックス
pub struct ProjectIndex {
    pub symbols: HashMap<PathBuf, Vec<DocumentSymbol>>,
}

impl Default for ProjectIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectIndex {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }

    pub fn add_file_symbols(&mut self, path: PathBuf, symbols: Vec<DocumentSymbol>) {
        self.symbols.insert(path, symbols);
    }

    pub fn file_count(&self) -> usize {
        self.symbols.len()
    }

    pub fn symbol_count(&self) -> usize {
        self.symbols.values().map(|s| s.len()).sum()
    }

    pub fn get_file_symbols(&self, path: &Path) -> Option<&Vec<DocumentSymbol>> {
        self.symbols.get(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Explicitly use std core to avoid conflict with local core crate
    extern crate std;

    #[test]
    fn test_language_detection() {
        assert_eq!(
            LspServerRegistry::detect_language(Path::new("test.rs")),
            Some("rust".to_string())
        );
        assert_eq!(
            LspServerRegistry::detect_language(Path::new("test.ts")),
            Some("typescript".to_string())
        );
        assert_eq!(
            LspServerRegistry::detect_language(Path::new("test.py")),
            Some("python".to_string())
        );
        assert_eq!(
            LspServerRegistry::detect_language(Path::new("test.go")),
            Some("go".to_string())
        );
    }

    #[test]
    fn test_registry_default_configs() {
        let registry = LspServerRegistry::default();

        assert!(registry.get_config("rust").is_some());
        assert!(registry.get_config("typescript").is_some());
        assert!(registry.get_config("python").is_some());
        assert!(registry.get_config("go").is_some());

        let rust_config = registry.get_config("rust").unwrap();
        assert_eq!(rust_config.command, "rust-analyzer");
        assert_eq!(rust_config.language_id, "rust");
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let manager = UnifiedLspManager::new();
        assert_eq!(manager.clients.len(), 0);
    }
}
