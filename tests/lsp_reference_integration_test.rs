use anyhow::Result;
use cli::storage::IndexStorage;
use lsif_core::CodeGraph;
use lsp::adapter::lsp::{RustAnalyzerAdapter, TypeScriptAdapter};
use lsp::lsp_client::LspClient;
use lsp::lsp_indexer::LspIndexer;
use lsp_types::*;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(test)]
mod rust_lsp_integration {
    use super::*;

    fn setup_rust_project_with_references() -> Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;

        // lib.rs - ライブラリの定義
        let lib_file = temp_dir.path().join("lib.rs");
        fs::write(
            &lib_file,
            r#"
pub struct Config {
    pub name: String,
    pub value: i32,
}

impl Config {
    pub fn new(name: String) -> Self {
        Config { name, value: 0 }
    }
    
    pub fn set_value(&mut self, value: i32) {
        self.value = value;
    }
    
    pub fn get_value(&self) -> i32 {
        self.value
    }
}

pub fn create_default_config() -> Config {
    Config::new("default".to_string())
}
"#,
        )?;

        // main.rs - lib.rsを使用
        let main_file = temp_dir.path().join("main.rs");
        fs::write(
            &main_file,
            r#"
mod lib;
use lib::{Config, create_default_config};

fn main() {
    let mut config = Config::new("test".to_string());
    config.set_value(42);
    println!("Value: {}", config.get_value());
    
    let default = create_default_config();
    println!("Default: {}", default.get_value());
}

fn test_function() {
    let config = Config::new("another".to_string());
    println!("{}", config.get_value());
}
"#,
        )?;

        let path = temp_dir.path().to_path_buf();
        Ok((temp_dir, path))
    }

    #[test]
    #[ignore] // rust-analyzerが必要
    fn test_find_struct_references_with_lsp() -> Result<()> {
        let (_temp_dir, project_path) = setup_rust_project_with_references()?;

        // LSPクライアントの初期化
        let adapter = Box::new(RustAnalyzerAdapter);
        let client = LspClient::new(adapter)?;

        // lib.rsのシンボル取得
        let lib_uri = Url::from_file_path(project_path.join("lib.rs")).unwrap();
        let lib_content = fs::read_to_string(project_path.join("lib.rs"))?;
        client.open_document(lib_uri.clone(), lib_content, "rust".to_string())?;
        let lib_symbols = client.document_symbols(lib_uri.clone())?;

        // main.rsのシンボル取得
        let main_uri = Url::from_file_path(project_path.join("main.rs")).unwrap();
        let main_content = fs::read_to_string(project_path.join("main.rs"))?;
        client.open_document(main_uri.clone(), main_content, "rust".to_string())?;
        let main_symbols = client.document_symbols(main_uri.clone())?;

        // グラフの構築
        let mut _graph = CodeGraph::new();

        // lib.rsのシンボルをインデックス
        if !lib_symbols.is_empty() {
            let mut indexer = LspIndexer::new(lib_uri.path().to_string());
            indexer.index_from_symbols(lib_symbols)?;
            let _lib_graph = indexer.into_graph();
            // TODO: グラフをマージ
        }

        // main.rsのシンボルをインデックス
        if !main_symbols.is_empty() {
            let mut indexer = LspIndexer::new(main_uri.path().to_string());
            indexer.index_from_symbols(main_symbols)?;
            let _main_graph = indexer.into_graph();
            // TODO: グラフをマージ
        }

        // Config構造体への参照を検索
        let config_refs = client.find_references(
            lib_uri,
            Position {
                line: 1,
                character: 11,
            }, // Config構造体の位置
            false,
        )?;

        {
            assert!(
                !config_refs.is_empty(),
                "Config構造体への参照が見つかるべき"
            );

            // main.rs内での参照があることを確認
            let main_refs: Vec<_> = config_refs
                .iter()
                .filter(|r| r.uri.path().contains("main.rs"))
                .collect();
            assert!(
                !main_refs.is_empty(),
                "main.rs内にConfig構造体への参照があるべき"
            );
        }

        Ok(())
    }

    #[test]
    #[ignore] // rust-analyzerが必要
    fn test_find_function_references_with_lsp() -> Result<()> {
        let (_temp_dir, project_path) = setup_rust_project_with_references()?;

        let adapter = Box::new(RustAnalyzerAdapter);
        let client = LspClient::new(adapter)?;

        let lib_uri = Url::from_file_path(project_path.join("lib.rs")).unwrap();

        // create_default_config関数への参照を検索
        let func_refs = client.find_references(
            lib_uri,
            Position {
                line: 20,
                character: 7,
            }, // create_default_config関数の位置
            false,
        )?;

        assert!(
            !func_refs.is_empty(),
            "create_default_config関数への参照が見つかるべき"
        );

        Ok(())
    }
}

#[cfg(test)]
mod typescript_lsp_integration {
    use super::*;

    fn setup_typescript_project_with_references() -> Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;

        // utils.ts - ユーティリティモジュール
        let utils_file = temp_dir.path().join("utils.ts");
        fs::write(
            &utils_file,
            r#"
export interface ILogger {
    log(message: string): void;
    error(message: string): void;
}

export class ConsoleLogger implements ILogger {
    log(message: string): void {
        console.log(`[LOG] ${message}`);
    }
    
    error(message: string): void {
        console.error(`[ERROR] ${message}`);
    }
}

export function createLogger(): ILogger {
    return new ConsoleLogger();
}

export const DEFAULT_LOGGER = createLogger();
"#,
        )?;

        // app.ts - utilsを使用
        let app_file = temp_dir.path().join("app.ts");
        fs::write(
            &app_file,
            r#"
import { ILogger, ConsoleLogger, createLogger, DEFAULT_LOGGER } from './utils';

class Application {
    private logger: ILogger;
    
    constructor() {
        this.logger = createLogger();
    }
    
    run(): void {
        this.logger.log('Application started');
        DEFAULT_LOGGER.log('Using default logger');
    }
    
    setLogger(logger: ILogger): void {
        this.logger = logger;
    }
}

const app = new Application();
app.run();

// カスタムロガーの実装
class CustomLogger implements ILogger {
    log(message: string): void {
        console.log(`[CUSTOM] ${message}`);
    }
    
    error(message: string): void {
        console.error(`[CUSTOM ERROR] ${message}`);
    }
}

const customLogger = new CustomLogger();
app.setLogger(customLogger);
"#,
        )?;

        // tsconfig.json
        let tsconfig = temp_dir.path().join("tsconfig.json");
        fs::write(
            &tsconfig,
            r#"{
    "compilerOptions": {
        "target": "ES2020",
        "module": "commonjs",
        "strict": true,
        "esModuleInterop": true,
        "skipLibCheck": true,
        "forceConsistentCasingInFileNames": true
    },
    "include": ["*.ts"]
}"#,
        )?;

        let path = temp_dir.path().to_path_buf();
        Ok((temp_dir, path))
    }

    #[test]
    #[ignore] // TypeScript LSPが必要
    fn test_find_interface_implementations_with_lsp() -> Result<()> {
        let (_temp_dir, project_path) = setup_typescript_project_with_references()?;

        let adapter = Box::new(TypeScriptAdapter);
        let client = LspClient::new(adapter)?;

        let utils_uri = Url::from_file_path(project_path.join("utils.ts")).unwrap();
        let utils_content = fs::read_to_string(project_path.join("utils.ts"))?;
        client.open_document(utils_uri.clone(), utils_content, "typescript".to_string())?;

        // ILoggerインターフェースの実装を検索
        let impl_refs = client.find_references(
            utils_uri,
            Position {
                line: 1,
                character: 17,
            }, // ILoggerインターフェースの位置
            false,
        )?;

        {
            assert!(
                !impl_refs.is_empty(),
                "ILoggerインターフェースへの参照（実装含む）が見つかるべき"
            );

            // ConsoleLoggerとCustomLoggerの実装が含まれることを確認
            let console_logger_impl = impl_refs.iter().any(|r| r.uri.path().contains("utils.ts"));
            let custom_logger_impl = impl_refs.iter().any(|r| r.uri.path().contains("app.ts"));

            assert!(console_logger_impl, "ConsoleLoggerの実装が見つかるべき");
            assert!(custom_logger_impl, "CustomLoggerの実装が見つかるべき");
        }

        Ok(())
    }

    #[test]
    #[ignore] // TypeScript LSPが必要
    fn test_find_function_imports_with_lsp() -> Result<()> {
        let (_temp_dir, project_path) = setup_typescript_project_with_references()?;

        let adapter = Box::new(TypeScriptAdapter);
        let client = LspClient::new(adapter)?;

        let utils_uri = Url::from_file_path(project_path.join("utils.ts")).unwrap();
        let utils_content = fs::read_to_string(project_path.join("utils.ts"))?;
        client.open_document(utils_uri.clone(), utils_content, "typescript".to_string())?;

        // createLogger関数への参照を検索
        let func_refs = client.find_references(
            utils_uri,
            Position {
                line: 16,
                character: 16,
            }, // createLogger関数の位置
            false,
        )?;

        {
            assert!(
                !func_refs.is_empty(),
                "createLogger関数への参照が見つかるべき"
            );

            // import文とコンストラクタ内での使用が含まれることを確認
            let app_refs: Vec<_> = func_refs
                .iter()
                .filter(|r| r.uri.path().contains("app.ts"))
                .collect();
            assert!(
                !app_refs.is_empty(),
                "app.ts内でcreateLogger関数が使用されているべき"
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod cross_file_reference_tests {
    use super::*;

    #[test]
    #[ignore] // LSPサーバーが必要
    fn test_build_complete_reference_graph() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path().to_path_buf();
        let storage_path = temp_path.join("test.sled");

        // テストプロジェクトの作成
        let src_dir = temp_path.join("src");
        fs::create_dir(&src_dir)?;

        let lib_file = src_dir.join("lib.rs");
        fs::write(
            &lib_file,
            r#"
pub struct Data {
    value: i32,
}

impl Data {
    pub fn new(value: i32) -> Self {
        Data { value }
    }
    
    pub fn process(&self) -> i32 {
        self.value * 2
    }
}
"#,
        )?;

        let main_file = src_dir.join("main.rs");
        fs::write(
            &main_file,
            r#"
mod lib;
use lib::Data;

fn main() {
    let data = Data::new(10);
    let result = data.process();
    println!("Result: {}", result);
}
"#,
        )?;

        // ストレージの初期化
        let _storage = IndexStorage::open(&storage_path)?;

        // 各ファイルをインデックス
        for entry in fs::read_dir(&src_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension() == Some(std::ffi::OsStr::new("rs")) {
                // TODO: LSPクライアントを使用してシンボルを取得
                // let adapter = Box::new(RustAnalyzerAdapter);
                // let client = LspClient::new(adapter)?;
                // let uri = Url::from_file_path(&path).unwrap();
                // let symbols = client.document_symbols(...)?;

                // シンボルをグラフに追加
                // let mut indexer = LspIndexer::new(path.to_string_lossy().to_string());
                // indexer.index_from_symbols(symbols)?;
                // let graph = indexer.into_graph();

                // ストレージに保存
                // storage.save_graph(&graph)?;
            }
        }

        // グラフ全体での参照解析
        // TODO: IndexStorageにグラフロード機能を実装
        let graph = CodeGraph::new();

        // Data構造体への参照を確認
        let data_refs = graph.find_references("src/lib.rs#1:Data");
        assert!(!data_refs.is_empty(), "Data構造体への参照が見つかるべき");

        // process()メソッドへの参照を確認
        let process_refs = graph.find_references("src/lib.rs#10:process");
        assert!(
            !process_refs.is_empty(),
            "process()メソッドへの参照が見つかるべき"
        );

        Ok(())
    }
}
