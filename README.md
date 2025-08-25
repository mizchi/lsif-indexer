# LSIF Indexer

高速な言語非依存コードインデックスツール。LSPベースの統一インターフェースで複数言語に対応。差分更新により0.05秒での増分インデックスを実現。

## 特徴

- **言語非依存アーキテクチャ**: LSPを活用した統一的なコード解析
- **マルチ言語サポート**: Rust, Go, Python, TypeScript/JavaScript対応
- **超高速インデックス**: 0.6秒以下でフルインデックス、0.05秒で差分更新
- **LSP統合**: 各言語のLSPサーバーとネイティブ統合
- **AIフレンドリー**: シンプルなCLIでAIツールとの統合が容易
- **低メモリ使用**: 50MB以下の効率的な動作

## インストール

### 必要なツール

```bash
# Rust (必須)
cargo install --path .

# 言語別LSPサーバー（オプション）
go install golang.org/x/tools/gopls@latest           # Go
pip install python-lsp-server                        # Python
npm install -g typescript-language-server typescript # TypeScript/JavaScript
```

## 使い方

### 基本コマンド

```bash
# 初回インデックス作成（言語自動検出）
lsif index                       # 自動検出
lsif index -l rust              # Rust指定
lsif index -l go                # Go指定（gopls使用）
lsif index -l python            # Python指定（pylsp使用）
lsif index -l typescript        # TypeScript指定

# 定義を検索
lsif definition ./src/main.rs 42

# 参照を検索
lsif references ./src/main.go 42

# シンボル検索
lsif workspace-symbols MyClass

# 曖昧検索（fuzzy search）
lsif workspace-symbols relat --fuzzy   # RelationshipPatternなどがマッチ
lsif workspace-symbols rp --fuzzy      # 文字順でマッチ

# コールヒエラルキー
lsif call-hierarchy function_name --depth 3
```

### オプション

- `--db <path>`: インデックスDBのパス（デフォルト: `.lsif-index.db`）
- `--project <path>`: プロジェクトルート（デフォルト: `.`）
- `--no-auto-index`: 自動インデックスを無効化

## パフォーマンス

### 実測値（自プロジェクト）

| 操作 | 時間 | 詳細 |
|------|------|------|
| フルインデックス（Rust） | 0.595秒 | 並列処理、CPU使用率250% |
| 差分更新 | 0.05秒 | 2-3ファイルの変更 |
| LSP統合（Go） | ~0.5秒 | gopls使用、20シンボル |
| メモリ使用量 | < 50MB | 全言語共通 |
| インデックスサイズ | 3.3MB | 圧縮済み |

## コマンド一覧

### LSP標準コマンド

| コマンド | 説明 | LSP対応 |
|----------|------|---------|
| `definition` | 定義へジャンプ | textDocument/definition |
| `references` | 参照を検索 | textDocument/references |
| `call-hierarchy` | 呼び出し階層 | textDocument/prepareCallHierarchy |
| `type-definition` | 型定義へジャンプ | textDocument/typeDefinition |
| `implementation` | 実装を検索 | textDocument/implementation |
| `symbols` | ドキュメントシンボル | textDocument/documentSymbol |
| `workspace-symbols` | ワークスペースシンボル | workspace/symbol |

### 拡張コマンド

| コマンド | 説明 |
|----------|------|
| `graph` | Cypher風のグラフクエリ |
| `unused` | 未使用コード検出 |
| `diff` | グラフ上の関連差分を追跡 |
| `status` | インデックス状態確認 |
| `export` | LSIF/JSON形式でエクスポート |

## AI統合例

```python
import subprocess
import json

def find_definition(file, line):
    """AIツールから定義を検索"""
    result = subprocess.run(
        ["lsif", "--db", "index.db", "definition", file, str(line)],
        capture_output=True, text=True
    )
    return result.stdout

def find_references(file, line):
    """AIツールから参照を検索"""
    result = subprocess.run(
        ["lsif", "--db", "index.db", "references", file, str(line)],
        capture_output=True, text=True
    )
    return result.stdout
```

## 曖昧検索機能

汎用的な曖昧検索機能を提供：

```rust
use lsif_indexer::cli::fuzzy_search::{fuzzy_search_strings, fuzzy_search_paths};

// 文字列リストの曖昧検索
let items = vec!["definition", "references", "workspace-symbols"];
let results = fuzzy_search_strings("def", &items);

// ファイルパスの曖昧検索（ファイル名でもマッチ）
let paths = vec!["src/core/graph.rs", "src/cli/fuzzy_search.rs"];
let results = fuzzy_search_paths("fuzzy", &paths);
```

マッチングアルゴリズム：
- 完全一致（スコア: 1.0）
- 前方一致（スコア: 0.9）
- 部分文字列（スコア: 0.7）
- 文字順序保持（スコア: 0.5）
- 略語マッチ（スコア: 0.6）

## サポート言語

| 言語 | LSPサーバー | 状態 | 自動検出 |
|------|------------|------|---------|
| Rust | rust-analyzer | ✅ ネイティブ | Cargo.toml |
| Go | gopls | ✅ LSP統合 | go.mod |
| Python | pylsp/pyright | ✅ LSP統合 | requirements.txt, setup.py |
| TypeScript | typescript-language-server | ✅ LSP統合 | tsconfig.json |
| JavaScript | typescript-language-server | ✅ LSP統合 | package.json |

### 新言語の追加

最小限のアダプタ実装（~100行）で新言語をサポート可能：

```rust
use lsif_indexer::cli::minimal_language_adapter::MinimalLanguageAdapter;

struct MyLanguageAdapter;

impl MinimalLanguageAdapter for MyLanguageAdapter {
    fn language_id(&self) -> &str { "mylang" }
    fn supported_extensions(&self) -> Vec<&str> { vec!["ml"] }
    fn spawn_lsp_command(&self) -> Result<Child> {
        Command::new("mylang-lsp").spawn()
    }
}
```

## 制限事項

- **Rust参照検索**: LSP未統合のため簡易実装
- **型推論**: 基本的なもののみサポート
- **クロスファイル解析**: 言語により精度が異なる

## 開発

詳細な開発手順は[CLAUDE.md](CLAUDE.md)を参照。

```bash
# テスト
cargo test

# ベンチマーク
cargo bench

# セルフインデックス
cargo run --release -- index -d self-index.db
```

## ライセンス

MIT