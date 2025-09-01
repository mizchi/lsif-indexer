# LSIF Indexer 開発ガイド

このドキュメントは、LSIF Indexerの開発とメンテナンスに必要な手順を記載しています。

## 重要：TypeScript LSP (tsgo) の起動方法

**毎回必ず確認すること：tsgoは`tsgo --lsp --stdio`で起動する**

### tsgoのインストール
```bash
# TypeScript Native Preview (tsgo) のインストール
npm install -g @typescript/native-preview

# インストール確認
tsgo --version
```

### tsgoのLSPモード起動
```bash
# LSPサーバーとして起動（stdioモード）
tsgo --lsp --stdio
```

### Rustコードでの使用方法
```rust
use anyhow::Result;
use std::process::{Child, Command, Stdio};

// tsgo LSPアダプタの実装
pub struct TsgoAdapter;

impl LspAdapter for TsgoAdapter {
    fn spawn_command(&self) -> Result<Child> {
        Command::new("tsgo")
            .args(&["--lsp", "--stdio"])  // 重要：必ず --lsp --stdio を指定
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn tsgo LSP: {}", e))
    }
}
```

### tsgoの特徴
- **workspace/symbol対応**: プロジェクト全体のシンボル検索が可能
- **documentSymbol対応**: ファイル単位の詳細なシンボル階層を取得可能
- **高速**: TypeScriptコンパイラのネイティブ実装
- **TypeScript/JavaScript両対応**: 両言語で同じLSPサーバーを使用可能

## プロジェクト概要

LSIF Indexerは、高速なコードインデックス作成と検索を提供するRust製のツールです。LSP（Language Server Protocol）を活用して、複数の言語に対応した統一的なコード解析を実現します。

### 主な特徴

- **高速インデックス作成**: 並列処理により大規模コードベースも高速にインデックス化
- **差分更新**: Gitの変更を検知して必要な部分のみを再インデックス（0.1秒以内）
- **LSP統合**: 既存のLSPサーバーを活用した正確なシンボル解析
- **多言語対応**: Rust、Go、Python、TypeScript/JavaScriptなどに対応
- **リッチなクエリ**: 定義参照、型階層、コールグラフ、未使用コード検出など

### アーキテクチャ

プロジェクトは3つのクレートで構成されています：

```
crates/
├── core/       # コアロジック
│   ├── graph.rs           # コードグラフのデータ構造
│   ├── graph_builder.rs   # グラフ構築
│   ├── graph_query.rs     # クエリエンジン
│   ├── incremental.rs     # インクリメンタル更新
│   ├── call_hierarchy.rs  # コール階層解析
│   ├── type_relations.rs  # 型関係解析
│   └── lsif.rs           # LSIF形式のサポート
│
├── lsp/        # LSP統合層
│   ├── lsp_client.rs      # 汎用LSPクライアント
│   ├── lsp_minimal_client.rs # 軽量LSPクライアント
│   ├── language_detector.rs  # 言語自動検出
│   ├── *_adapter.rs       # 各言語のアダプタ実装
│   └── fallback_indexer.rs # フォールバックインデクサ
│
└── cli/        # CLI・IO層
    ├── simple_cli.rs      # CLIインターフェース
    ├── storage.rs         # Sledベースの永続化
    ├── indexer.rs         # インデックス作成
    ├── differential_indexer.rs # 差分インデックス
    └── git_diff.rs        # Git差分検出
```

### 使用方法

```bash
# インデックス作成
./target/release/lsif index

# 定義へジャンプ
./target/release/lsif definition --file src/main.rs --line 10 --column 5

# 参照検索
./target/release/lsif references --file src/lib.rs --line 20 --column 10

# コール階層表示
./target/release/lsif call-hierarchy --symbol "main"

# 未使用コード検出
./target/release/lsif unused
```

## 重要な開発ルール

### 一時ファイルの生成場所

**すべての一時ファイル（テスト用DBファイル、キャッシュファイル等）は必ず `tmp/` ディレクトリ以下に生成すること。**

- テスト実行時: `tmp/test-*.db`
- ベンチマーク実行時: `tmp/bench-*.db`
- デバッグ実行時: `tmp/debug-*.db`
- その他の一時ファイル: `tmp/` 以下の適切な場所

```bash
# tmpディレクトリの作成（必要に応じて）
mkdir -p tmp/
```

## 開発環境セットアップ

### 必要なツール

```bash
# Rust (最新安定版)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# LSPサーバー（テスト用）
cargo install rust-analyzer
npm install -g typescript-language-server

# 開発ツール
cargo install cargo-watch
cargo install cargo-edit
```

## 開発ワークフロー

### 1. コード変更後の検証手順

#### 型チェック（テスト含む）

```bash
# すべてのターゲット（ライブラリ、バイナリ、テスト、ベンチマーク）の型チェック
cargo check --all-targets

# より厳密なチェック（警告をエラーとして扱う）
RUSTFLAGS="-D warnings" cargo check --all-targets
```

#### テスト実行

```bash
# 全テスト実行
cargo test

# 特定のテストモジュールのみ
cargo test git_diff        # Git差分検知テスト
cargo test differential    # 差分インデックステスト
cargo test restore        # リストアシナリオテスト

# 並列テストの実行（デフォルト）
cargo test -- --test-threads=8

# テストの出力を表示
cargo test -- --nocapture

# 統合テストのみ
cargo test --test '*'
```

#### コードフォーマット

```bash
# フォーマット実行
cargo fmt

# フォーマットチェック（CIで使用）
cargo fmt -- --check
```

### 2. パフォーマンス検証

#### ベンチマーク実行

```bash
# 全ベンチマーク実行
cargo bench

# 特定のベンチマークのみ
cargo bench index_performance  # インデックス性能
cargo bench storage           # ストレージ性能
cargo bench cache            # キャッシュ性能
cargo bench parallel         # 並列処理性能

# ベースラインとの比較
cargo bench -- --save-baseline main
# 変更後
cargo bench -- --baseline main
```

#### 自己インデックスによる実使用検証

```bash
# 自身のコードベースをインデックス（最も実践的なテスト）
cargo run --release -- index-project -p . -o tmp/self-index.db -l rust

# 差分インデックスのテスト
echo "// test" >> src/main.rs
cargo run --release -- differential-index -p . -o tmp/self-index.db
# Expected: 0.06-0.12秒で完了

# 生成されたインデックスを検証
cargo run --release -- query -i tmp/self-index.db --query-type definition -f src/main.rs -l 10 -c 5
cargo run --release -- show-dead-code -i tmp/self-index.db
cargo run --release -- call-hierarchy -i tmp/self-index.db -s "main" -d full
```

### 3. 品質チェック

#### テストカバレッジ計測

```bash
# cargo-llvm-covのインストール（初回のみ）
cargo install cargo-llvm-cov

# カバレッジレポート生成（HTML形式）
mkdir -p tmp
cargo llvm-cov --lib --html --output-dir tmp/coverage-report

# カバレッジサマリーの表示
cargo llvm-cov --lib --summary-only

# 統合テストを含む全テストのカバレッジ
cargo llvm-cov --html --output-dir tmp/coverage-report

# カバレッジレポートの閲覧
# ブラウザで tmp/coverage-report/html/index.html を開く
```

##### 現在のカバレッジ状況

| メトリクス | 現在値 | 目標値 |
|-----------|--------|--------|
| ライン | 32.91% | > 70% |
| 関数 | 33.98% | > 80% |
| リージョン | 35.14% | > 75% |

##### カバレッジが低いモジュール（優先改善対象）

| モジュール | 現在のカバレッジ | 理由 |
|-----------|------------------|------|
| cli/mod.rs | 0% | CLIメインモジュール（統合テスト必要） |
| cli/differential_indexer.rs | 0% | 差分インデックサー（モック化必要） |
| cli/lsp_minimal_client.rs | 0.93% | LSPクライアント（モックサーバー必要） |
| cli/simple_cli.rs | 0% | CLIコマンド実装（統合テスト必要） |
| core/lsif.rs | 0% | LSIF形式処理（テスト追加必要） |

#### Clippy（Lintツール）

```bash
# 基本的なlint
cargo clippy

# すべてのターゲットでlint
cargo clippy --all-targets

# より厳密なlint
cargo clippy -- -W clippy::pedantic

# 自動修正可能な問題を修正
cargo clippy --fix
```

#### セキュリティ監査

```bash
# 依存関係のセキュリティ監査
cargo install cargo-audit
cargo audit
```

## リリース前チェックリスト

### 必須項目

- [ ] `cargo test` がすべて成功
- [ ] `cargo bench` でパフォーマンス劣化がない
- [ ] 自己インデックスが1.2秒以内に完了
- [ ] 差分インデックスが0.12秒以内に完了
- [ ] `cargo fmt` 実行済み
- [ ] `cargo clippy` で警告なし

### 推奨項目

- [ ] README.mdの更新
- [ ] CHANGELOG.mdの更新
- [ ] 新機能のテストカバレッジ80%以上
- [ ] ドキュメントコメントの追加

## パフォーマンス目標

### インデックス性能

| メトリクス | 目標値 | 測定方法 |
|-----------|--------|----------|
| 初回フルインデックス | < 1.5秒 | `cargo run --release -- index-project -p . -o tmp/test.db` |
| 差分インデックス | < 0.15秒 | ファイル変更後の `differential-index` |
| メモリ使用量 | < 150MB | `/usr/bin/time -v` で測定 |
| ファイル処理速度 | > 30 files/sec | ベンチマーク結果 |

### ストレージ性能

| メトリクス | 目標値 | 測定方法 |
|-----------|--------|----------|
| シンボル保存 | > 10,000 ops/sec | `cargo bench storage` |
| キャッシュヒット率 | > 80% | 統計情報から |
| DB圧縮率 | > 50% | ファイルサイズ比較 |

## トラブルシューティング

### テストが失敗する場合

```bash
# テスト用の一時ファイルをクリーンアップ
rm -rf tmp/

# キャッシュをクリア
cargo clean

# 依存関係を更新
cargo update
```

### ベンチマークが不安定な場合

```bash
# CPUガバナーを性能優先に設定（Linux）
sudo cpupower frequency-set -g performance

# 単一コアで実行
taskset -c 0 cargo bench
```

### メモリ使用量が多い場合

```bash
# プロファイリング
cargo install flamegraph
cargo flamegraph --bin lsif-indexer -- index-project -p . -o tmp/test.db

# メモリプロファイリング
valgrind --tool=massif cargo run --release -- index-project -p . -o tmp/test.db
ms_print massif.out.*
```

## 継続的インテグレーション

### GitHub Actions設定

```yaml
# .github/workflows/ci.yml
- name: Run tests
  run: |
    cargo test --all-targets
    cargo clippy --all-targets
    cargo fmt -- --check

- name: Run benchmarks
  run: cargo bench --no-fail-fast

- name: Self-index test
  run: |
    cargo build --release
    mkdir -p tmp
    time ./target/release/lsif-indexer index-project -p . -o tmp/ci-test.db
```

## 開発のベストプラクティス

### コミット前

1. `cargo test` を実行
2. `cargo fmt` を実行
3. 自己インデックスで動作確認
4. 差分インデックスの性能を確認

### PR作成前

1. すべてのテストが通ることを確認
2. ベンチマークでパフォーマンス劣化がないことを確認
3. READMEを更新（新機能の場合）
4. CHANGELOGを更新

### デバッグ時

```bash
# デバッグビルドで詳細ログ
RUST_LOG=debug cargo run -- index-project -p . -o tmp/debug.db

# 特定モジュールのログのみ
RUST_LOG=lsif_indexer::cli::git_diff=trace cargo run -- differential-index -p . -o tmp/debug.db
```

## メトリクス収集

定期的に以下のメトリクスを収集して、パフォーマンスの推移を監視：

```bash
# パフォーマンステスト自動化スクリプト
#!/bin/bash
mkdir -p tmp
echo "=== Performance Test $(date) ===" | tee -a performance.log

# フルインデックス
time cargo run --release -- index-project -p . -o tmp/perf-test.db 2>&1 | tee -a performance.log

# 差分インデックス
echo "// test" >> src/lib.rs
time cargo run --release -- differential-index -p . -o tmp/perf-test.db 2>&1 | tee -a performance.log
git checkout src/lib.rs

# ベンチマーク結果
cargo bench 2>&1 | tee -a performance.log
```

## 注意事項

- テストは並列実行されるため、ファイル名の衝突に注意
- ベンチマークは安定した環境で実行（他のプロセスを停止）
- 自己インデックスは最も包括的な動作確認方法
- 差分インデックスの性能は特に重要（頻繁に実行されるため）