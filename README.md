# LSIF Indexer

高速なコードインデックスツール。差分更新により0.05秒での増分インデックスを実現。LSP標準のコマンド体系でAIとの統合に最適化。

## 特徴

- **超高速差分インデックス**: Git差分検知により変更ファイルのみ0.05秒で更新
- **自動インデックス**: コマンド実行前に自動で変更を検知・更新
- **LSP標準コマンド**: definition, references等のLSP準拠コマンド
- **AIフレンドリー**: シンプルなCLIでAIツールとの統合が容易

## インストール

```bash
cargo install --path .
```

## 使い方

### 基本コマンド

```bash
# 初回インデックス作成（自動実行される）
lsif index

# 定義を検索
lsif definition ./src/main.rs 42

# 参照を検索
lsif references ./src/main.rs 42

# シンボル検索
lsif workspace-symbols MyClass

# コールヒエラルキー
lsif call-hierarchy function_name --depth 3
```

### オプション

- `--db <path>`: インデックスDBのパス（デフォルト: `.lsif-index.db`）
- `--project <path>`: プロジェクトルート（デフォルト: `.`）
- `--no-auto-index`: 自動インデックスを無効化

## パフォーマンス

セルフインデックスの実測値：

| 操作 | 時間 | 詳細 |
|------|------|------|
| 初回インデックス | 0.13秒 | 72ファイル、1021シンボル |
| 差分更新 | 0.05秒 | 2-3ファイルの変更 |
| 検索 | <0.01秒 | ほぼ瞬時 |

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