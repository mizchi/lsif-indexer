# LSIF Indexer

[![CI](https://github.com/mizchi/lsif-indexer/actions/workflows/ci.yml/badge.svg)](https://github.com/mizchi/lsif-indexer/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/mizchi/lsif-indexer/branch/main/graph/badge.svg)](https://codecov.io/gh/mizchi/lsif-indexer)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/crates/v/lsif-indexer.svg)](https://crates.io/crates/lsif-indexer)

LSIFベースの高速コードインデックス・グラフ検索システム。AI支援開発ツールとの統合を前提に設計され、大規模コードベースの構造を効率的に解析し、シンボル間の関係をグラフとして管理します。Language Server Protocol (LSP) と LSIF (Language Server Index Format) を活用し、言語に依存しない汎用的なコード解析を実現。

## 特徴

- 🚀 **高速処理**: 並列処理により最大59倍の高速化、xxHash3による高速差分検知
- 🔍 **高度な解析**: 定義・参照検索、コールグラフ、デッドコード検出、型階層分析
- 🌍 **多言語対応**: Rust (rust-analyzer), TypeScript/JavaScript (typescript-language-server)
- 💾 **効率的なストレージ**: Git差分検知とコンテンツハッシュによる差分インデックス、90%の時間削減
- 📊 **グラフ構造**: Cypher風クエリによる複雑な依存関係の検索
- 🤖 **AI最適化**: コードグラフをAIが理解しやすい形式で提供、コンテキスト生成支援

## インストール

```bash
cargo install --path .
```

または直接実行:

```bash
cargo build --release
./target/release/lsif-indexer --help
```

## 使用方法

### 基本的な使い方

```bash
# Rustプロジェクトをインデックス化
lsif-indexer index-project --project . --output index.db --language rust

# TypeScriptプロジェクトをインデックス化
lsif-indexer index-project --project . --output index.db --language typescript

# 差分インデックス（Git差分検知とxxHash3による高速処理）
lsif-indexer differential-index --project . --output index.db

# LSIFフォーマットでエクスポート
lsif-indexer export-lsif --index index.db --output output.lsif
```

### 高度な使い方

```bash
# 除外パターンを指定
lsif-indexer --files="**/*.rs" --exclude="target" --exclude="tests"

# スレッド数とバッチサイズを指定
lsif-indexer --threads=8 --batch-size=200 --files="**/*.rs"

# インクリメンタル更新（変更されたファイルのみ処理）
lsif-indexer --incremental --files="**/*.rs" --output=index.db

# 詳細なログ出力
lsif-indexer --verbose --files="**/*.rs"
```

### クエリ機能

```bash
# 定義を検索
lsif-indexer query --index index.db --query-type definition --file src/main.rs --line 10 --column 15

# 参照を検索
lsif-indexer query --index index.db --query-type references --file src/lib.rs --line 20 --column 10

# コールグラフを表示
lsif-indexer call-hierarchy --index index.db --symbol "main" --direction full --max-depth 5

# デッドコードを検出
lsif-indexer show-dead-code --index index.db

# 型関係を解析
lsif-indexer type-relations --index index.db --type-symbol "User" --max-depth 3 --hierarchy

# Cypher風グラフクエリ
lsif-indexer query-pattern --index index.db --pattern "MATCH (s:Struct {name: 'Config'})<-[:USES]-(f:Function) RETURN f"
```

### 高度なLSP連携機能

```bash
# ホバー情報を取得
lsif lsp hover --file src/main.rs --line 10 --column 15

# コード補完
lsif lsp complete --file src/main.rs --line 10 --column 15

# 実装を検索
lsif lsp implementations --file src/lib.rs --line 20 --column 5

# 型定義を検索
lsif lsp type-definition --file src/main.rs --line 30 --column 10

# シンボルをリネーム
lsif lsp rename --file src/lib.rs --line 15 --column 5 --new-name "NewName"

# 診断情報を取得
lsif lsp diagnostics --file src/main.rs

# LSP統合でプロジェクト全体をインデックス化
lsif lsp index-with-lsp --project . --output advanced_index.db
```

### ウォッチモード

```bash
# ファイルの変更を監視して自動更新
lsif-indexer watch --files="**/*.rs" --db=index.db
```

### 統計情報

```bash
# インデックスの統計を表示
lsif-indexer stats --db=index.db
```

## CLI オプション

| オプション | 短縮 | デフォルト | 説明 |
|-----------|------|-----------|------|
| `--files` | `-f` | `**/*.rs` | インデックス対象のファイル（glob パターン） |
| `--output` | `-o` | `./index.db` | 出力データベースのパス |
| `--bin` | `-b` | 自動検出 | 使用する LSP バイナリ |
| `--language` | `-l` | 自動検出 | プログラミング言語 |
| `--parallel` | `-p` | `true` | 並列処理を有効化 |
| `--cache` | `-c` | `true` | キャッシュを有効化 |
| `--verbose` | `-v` | `false` | 詳細ログ出力 |
| `--threads` | `-t` | 自動 | スレッド数 |
| `--batch-size` | `-B` | `100` | バッチ処理サイズ |
| `--progress` | `-P` | `true` | プログレスバー表示 |
| `--incremental` | `-i` | `false` | インクリメンタル更新 |
| `--exclude` | `-e` | なし | 除外パターン |

## サポート言語

| 言語 | 拡張子 | LSP サーバー |
|------|--------|-------------|
| Rust | `.rs` | rust-analyzer |
| TypeScript | `.ts`, `.tsx` | typescript-language-server |
| JavaScript | `.js`, `.jsx` | typescript-language-server |
| Python | `.py` | pylsp |
| Go | `.go` | gopls |
| Java | `.java` | jdtls |
| C/C++ | `.c`, `.cpp`, `.h` | clangd |

## パフォーマンス

### ベンチマーク結果（自己インデックス実測）

| 操作 | 時間 | 詳細 |
|------|------|------|
| 初回フルインデックス | 0.7-1.2秒 | 全ファイル解析、シンボル抽出 |
| 差分インデックス | 0.06-0.12秒 | Git差分検知、変更ファイルのみ処理 |
| ファイル変更後の再インデックス | 0.08秒 | xxHash3による高速ハッシュ比較 |
| メモリ使用量 | 50-100MB | 10万シンボル規模 |

### 最適化技術

- **Git差分検知**: git2-rsによる高速な変更検出
- **xxHash3**: SHA256より10-100倍高速なハッシュ計算
- **並列処理**: Rayon による自動並列化
- **メモリプール**: UltraFastStorageによる効率的なメモリ管理
- **差分更新**: 変更ファイルのみ処理で 90% 時間削減
- **キャッシュ戦略**: LRU キャッシュとプリフェッチで頻繁アクセスを高速化

## 開発

### Makefile

プロジェクトには開発タスクを簡単に実行できるMakefileが含まれています：

```bash
# ビルド
make build        # リリースビルド
make check        # コード品質チェック（clippy, fmt）

# テスト
make test         # 全てのテストを実行
make test-unit    # ユニットテストのみ
make test-reference  # 参照解析テストを実行

# クリーンアップ
make clean        # 全てクリーン（ビルド含む）
make clean-temp   # 一時ファイルのみクリーン

# セルフインデックス
make self-index   # 自身のコードベースをインデックス化
make interactive  # インタラクティブモードで探索

# その他
make fmt          # コードフォーマット
make bench        # ベンチマーク実行
make help         # ヘルプ表示
```

### スクリプト

`scripts/`ディレクトリには便利なスクリプトが含まれています：

- `clean.sh` - 一時ファイルとテストアーティファクトをクリーンアップ
- `self-index.sh` - LSIF Indexer自身をインデックス化

### ディレクトリ構造

```
lsif-indexer/
├── src/           # ソースコード
├── tests/         # 統合テスト
├── benches/       # ベンチマーク
├── scripts/       # ユーティリティスクリプト
├── tmp/           # 一時ファイル（gitignore対象）
└── examples/      # 使用例
```

### 一時ファイル管理

- `test-*` および `self-index*` ファイルは自動的にgitignoreされます
- `tmp/`ディレクトリは一時的なインデックスファイル用です
- `make clean-temp`で簡単にクリーンアップできます

## 使用例

### React プロジェクト

```bash
# JavaScript と TypeScript ファイルをインデックス化
lsif-indexer \
  --files="src/**/*.{js,jsx,ts,tsx}" \
  --exclude="node_modules" \
  --exclude="build" \
  --language=typescript \
  --output=react.db

# 実行結果
# Files processed: 4,222
# Total symbols: 45,678
# Time: 180s
# Speed: 23 files/sec
```

### Rust プロジェクト (Deno)

```bash
# Rust ファイルを高速インデックス化
lsif-indexer \
  --files="**/*.rs" \
  --exclude="target" \
  --parallel \
  --threads=16 \
  --batch-size=500 \
  --output=deno.db

# 実行結果
# Files processed: 593
# Total symbols: 12,345
# Time: 45s
# Speed: 13 files/sec
```

### モノレポ対応

```bash
# 複数言語のモノレポをインデックス化
for lang in rust typescript python; do
  lsif-indexer \
    --files="packages/**/src/**/*" \
    --language=$lang \
    --output=monorepo_$lang.db
done
```

## 設定ファイル

`.lsif-indexer.toml` で設定を永続化:

```toml
[default]
files = "**/*.rs"
output = "./index.db"
parallel = true
cache = true
batch_size = 200

[exclude]
patterns = ["target", "node_modules", ".git"]

[languages.rust]
bin = "rust-analyzer"
extensions = ["rs"]

[languages.typescript]
bin = "typescript-language-server"
extensions = ["ts", "tsx", "js", "jsx"]
```

## AI統合での活用

### コード理解支援

```rust
use lsif_indexer::core::CodeGraph;
use lsif_indexer::cli::storage::IndexStorage;

// AIがコードベースを理解するためのコンテキスト生成
let storage = IndexStorage::open("index.db")?;
let graph: CodeGraph = storage.load_data("graph")?.unwrap();

// シンボルのコンテキストを取得（定義、参照、依存関係）
let symbol = graph.find_symbol("MyFunction")?;
let references = graph.find_references("MyFunction");
let call_hierarchy = graph.get_call_hierarchy("MyFunction");
// -> AIが関数の役割と影響範囲を理解
```

### リファクタリング提案

```rust
use lsif_indexer::cli::differential_indexer::DifferentialIndexer;

// 未使用コードの検出
let indexer = DifferentialIndexer::new("index.db", ".")?;
let result = indexer.index_differential()?;
// -> AIがデッドコードの削除やリファクタリングを提案

// 型の階層関係分析
let analyzer = TypeRelationsAnalyzer::new(&graph);
let hierarchy = analyzer.find_type_hierarchy("BaseClass");
// -> AIが継承構造の改善を提案
```

### コード生成支援

```rust
use lsif_indexer::core::{QueryEngine, QueryParser};

// 既存のパターンを学習
let pattern = QueryParser::parse("MATCH (f:Function)-[:CALLS]->(g:Function) WHERE f.name CONTAINS 'test'")?;
let engine = QueryEngine::new(&graph);
let results = engine.execute(&pattern);
// -> AIがテストパターンを学習して新しいテストを生成
```

### 差分インデックスによる効率化

```rust
// Git差分とxxHash3による高速な変更検知
let mut detector = GitDiffDetector::new(".")?;
let changes = detector.detect_changes_since(None)?;
// -> AIが変更の影響範囲を即座に把握

// 差分のみを再インデックス（0.06-0.12秒）
let result = indexer.index_differential()?;
println!("Files modified: {}, Symbols updated: {}", 
         result.files_modified, result.symbols_updated);
```

## API

ライブラリとして使用:

```rust
use lsif_indexer::cli::parallel_storage::ParallelIndexStorage;
use lsif_indexer::core::CodeGraph;

// インデックスを作成
let storage = ParallelIndexStorage::open("index.db")?;
let mut graph = CodeGraph::new();

// シンボルを追加
graph.add_symbol(symbol);

// クエリ実行
let definition = graph.find_definition("file.rs#10:5")?;
let references = graph.find_references("MyStruct");
let dead_code = graph.find_dead_code();
```

## トラブルシューティング

### LSP サーバーが見つからない

```bash
# LSP サーバーをインストール
npm install -g typescript-language-server
cargo install rust-analyzer
pip install python-lsp-server
```

### メモリ不足

```bash
# バッチサイズを小さくする
lsif-indexer --batch-size=50 --files="**/*.rs"
```

### 処理が遅い

```bash
# スレッド数を増やす
lsif-indexer --threads=16 --parallel --files="**/*.rs"
```

## 開発

```bash
# テスト実行
cargo test

# ベンチマーク
cargo bench

# ドキュメント生成
cargo doc --open
```

## ライセンス

MIT License

## コントリビューション

Pull Request 歓迎！Issue での機能要望・バグ報告もお待ちしています。