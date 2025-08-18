# LSIF Indexer 使用ガイド

## インストール

```bash
cargo install --path .
```

## 基本コマンド

### 1. プロジェクトのインデックス化

```bash
# Rustプロジェクト全体をインデックス化
lsif --files="**/*.rs" --output=index.db

# 特定ディレクトリのみ
lsif --files="src/**/*.rs" --output=src_index.db

# 除外パターンを使用
lsif --files="**/*.rs" --exclude=target --exclude=tests
```

### 2. パフォーマンス最適化

```bash
# 並列処理とキャッシュを有効化（デフォルト）
lsif --files="**/*.rs" --parallel --cache

# スレッド数とバッチサイズを指定
lsif --files="**/*.rs" --threads=8 --batch-size=200

# インクリメンタル更新（変更ファイルのみ処理）
lsif --incremental --files="**/*.rs"
```

### 3. クエリとレポート

```bash
# サポート言語の一覧
lsif list

# データベース統計
lsif stats --db=index.db

# 定義を検索
lsif query --db=index.db definition src/main.rs 10 5

# 参照を検索
lsif query --db=index.db references MyStruct

# コールヒエラルキー
lsif query --db=index.db call-hierarchy main --depth=3

# デッドコード検出
lsif query --db=index.db dead-code
```

## 現在の実装状況

### ✅ 実装済み
- 基本的なRustコードのシンボル抽出（関数、構造体、impl）
- glob パターンによるファイル選択
- 除外パターン
- 並列処理とキャッシュ
- プログレスバー表示
- データベース統計

### 🚧 開発中
- 実際のLSP（rust-analyzer）との連携
- TypeScript/JavaScript サポート
- Python サポート
- より高度なクエリ機能
- インクリメンタル更新の最適化

## テスト済みコマンド例

```bash
# シンプルなRustファイルの解析
echo 'fn main() {
    println!("Hello");
}

struct User {
    name: String,
}' > test.rs

lsif --files="test.rs" --output=test.db
# 出力: 2つのシンボル（main関数とUser構造体）を検出

# 大規模プロジェクト
lsif --files="src/**/*.rs" --threads=16 --batch-size=500
# 高速並列処理でインデックス化
```

## トラブルシューティング

### Q: LSPサーバーが見つからない
A: 現在はビルトインのパーサーを使用しています。将来的に実際のLSPサーバー連携を実装予定です。

### Q: メモリ不足
A: バッチサイズを小さくしてください：
```bash
lsif --files="**/*.rs" --batch-size=50
```

### Q: 処理が遅い
A: スレッド数を増やしてください：
```bash
lsif --files="**/*.rs" --threads=16 --parallel
```