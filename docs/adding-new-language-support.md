# 新しい言語サポートの追加ガイド

このガイドでは、LSIF Indexerに新しいプログラミング言語のサポートを追加する方法を説明します。

## 前提条件

- 対象言語のLSP（Language Server Protocol）実装が存在すること
- Rustの基本的な知識
- 対象言語の構文に関する理解

## 実装手順

### 1. LSPアダプターの作成

`src/cli/lsp_adapter.rs` に新しいアダプター構造体を追加します。

```rust
// 例: Python言語サポートの追加
pub struct PythonAdapter;

impl LspAdapter for PythonAdapter {
    fn spawn_command(&self) -> Result<Child> {
        // Python LSPサーバーを起動
        Command::new("pylsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn Python LSP. Install with: pip install python-lsp-server")
    }
    
    fn get_init_params(&self) -> InitializeParams {
        let mut params = InitializeParams::default();
        params.capabilities = ClientCapabilities {
            text_document: Some(TextDocumentClientCapabilities {
                definition: Some(GotoCapability {
                    dynamic_registration: Some(false),
                    link_support: Some(false),
                }),
                references: Some(ReferenceClientCapabilities {
                    dynamic_registration: Some(false),
                }),
                document_symbol: Some(DocumentSymbolClientCapabilities {
                    dynamic_registration: Some(false),
                    hierarchical_document_symbol_support: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        params
    }
    
    fn language_id(&self) -> &str {
        "python"
    }
}
```

### 2. 言語検出の更新

`detect_language` 関数に新しい言語の拡張子を追加します。

```rust
pub fn detect_language(file_path: &str) -> Option<Box<dyn LspAdapter>> {
    let extension = std::path::Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())?;
    
    match extension {
        "rs" => Some(Box::new(RustAnalyzerAdapter)),
        "ts" | "tsx" | "js" | "jsx" => Some(Box::new(TypeScriptAdapter)),
        "py" | "pyw" => Some(Box::new(PythonAdapter)),  // Python追加
        _ => None,
    }
}
```

### 3. 参照検索パターンの追加

`src/cli/reference_finder.rs` を更新して、言語固有のパターンを追加します。

#### ソースファイル判定の更新

```rust
fn is_source_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        matches!(
            ext.to_str().unwrap_or(""),
            "rs" | "ts" | "tsx" | "js" | "jsx" | 
            "py" | "pyw" |  // Python
            "go" | "java" | "cpp" | "c" | "h"
        )
    } else {
        false
    }
}
```

#### 定義パターンの追加

```rust
fn is_definition_context(line: &str, position: usize) -> bool {
    // ... 既存のコード
    
    let definition_keywords = [
        // ... 既存のパターン
        
        // Python固有のパターン
        "def",              // 関数定義
        "async def",        // 非同期関数定義  
        "class",            // クラス定義
        "import",           // インポート
        "from",             // from ... import
    ];
    
    // ... パターンマッチングロジック
}
```

### 4. インデクサーの更新

`src/cli/indexer.rs` または `src/cli/mod.rs` で新しい言語をサポートするように更新します。

```rust
fn index_project(project_path: &str, output_path: &str, language: &str) -> Result<()> {
    // ... 既存のコード
    
    let adapter: Box<dyn LspAdapter> = match language {
        "rust" => Box::new(RustAnalyzerAdapter),
        "typescript" | "ts" | "javascript" | "js" => Box::new(TypeScriptAdapter),
        "python" | "py" => Box::new(PythonAdapter),  // Python追加
        _ => {
            return Err(anyhow::anyhow!("Unsupported language: {}", language));
        }
    };
    
    // ... インデックス処理
}
```

### 5. テストの作成

新しい言語のテストファイルを作成します。

```rust
// tests/python_references_test.rs
use lsif_indexer::cli::reference_finder::find_all_references;
use lsif_indexer::core::SymbolKind;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_python_function_references() {
    let temp_dir = TempDir::new().unwrap();
    
    let content = r#"
def calculate_sum(a, b):
    return a + b

def main():
    result = calculate_sum(10, 20)
    print(f"Result: {result}")
    
class Calculator:
    def add(self, a, b):
        return calculate_sum(a, b)

if __name__ == "__main__":
    calc = Calculator()
    calc.add(5, 3)
"#;
    
    fs::write(temp_dir.path().join("test.py"), content).unwrap();
    
    let references = find_all_references(
        temp_dir.path(),
        "calculate_sum",
        &SymbolKind::Function
    ).unwrap();
    
    assert!(references.len() >= 2, "Should find definition and usage");
}

#[test]
fn test_python_class_references() {
    // クラス参照のテスト
}
```

### 6. ドキュメントの更新

- README.mdに新しい言語のサポートを追加
- USAGE.mdに言語固有の使用例を追加

## 言語固有の考慮事項

### Python
- インデントベースの構文
- デコレーターの扱い
- `__init__.py` によるモジュール構造

### Go
- パッケージシステム
- インターフェースの暗黙的実装
- 大文字・小文字によるエクスポート制御

### Java
- パッケージ階層とディレクトリ構造の対応
- 内部クラス
- アノテーション

## トラブルシューティング

### LSPサーバーが起動しない

1. LSPサーバーがインストールされているか確認
2. PATHに追加されているか確認
3. 必要な依存関係がインストールされているか確認

### シンボルが正しく検出されない

1. LSPサーバーのログを確認
2. `RUST_LOG=debug` で詳細ログを有効化
3. LSPサーバーのバージョンを確認

### パフォーマンスの問題

1. ファイル数が多い場合は除外パターンを設定
2. インクリメンタルインデックスを活用
3. LSPサーバーの設定を最適化

## チェックリスト

- [ ] LSPアダプターの実装
- [ ] 言語検出の追加
- [ ] 参照検索パターンの追加
- [ ] 基本的なテストの作成
- [ ] ドキュメントの更新
- [ ] サンプルプロジェクトでの動作確認
- [ ] パフォーマンステスト
- [ ] CIへの統合

## 参考リソース

- [Language Server Protocol Specification](https://microsoft.github.io/language-server-protocol/)
- [各言語のLSP実装一覧](https://langserver.org/)
- [LSIF仕様](https://lsif.dev/)