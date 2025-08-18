# LSIF Indexer 開発ガイド

このドキュメントは、LSIF Indexerの開発とメンテナンスに必要な手順を記載しています。

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
cargo run --release -- index-project -p . -o self-index.db -l rust

# 差分インデックスのテスト
echo "// test" >> src/main.rs
cargo run --release -- differential-index -p . -o self-index.db
# Expected: 0.06-0.12秒で完了

# 生成されたインデックスを検証
cargo run --release -- query -i self-index.db --query-type definition -f src/main.rs -l 10 -c 5
cargo run --release -- show-dead-code -i self-index.db
cargo run --release -- call-hierarchy -i self-index.db -s "main" -d full
```

### 3. 品質チェック

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
| 初回フルインデックス | < 1.5秒 | `cargo run --release -- index-project -p . -o test.db` |
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
rm -rf test-* self-index* tmp/

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
cargo flamegraph --bin lsif-indexer -- index-project -p . -o test.db

# メモリプロファイリング
valgrind --tool=massif cargo run --release -- index-project -p . -o test.db
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
    time ./target/release/lsif-indexer index-project -p . -o ci-test.db
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
RUST_LOG=debug cargo run -- index-project -p . -o debug.db

# 特定モジュールのログのみ
RUST_LOG=lsif_indexer::cli::git_diff=trace cargo run -- differential-index -p . -o debug.db
```

## メトリクス収集

定期的に以下のメトリクスを収集して、パフォーマンスの推移を監視：

```bash
# パフォーマンステスト自動化スクリプト
#!/bin/bash
echo "=== Performance Test $(date) ===" | tee -a performance.log

# フルインデックス
time cargo run --release -- index-project -p . -o perf-test.db 2>&1 | tee -a performance.log

# 差分インデックス
echo "// test" >> src/lib.rs
time cargo run --release -- differential-index -p . -o perf-test.db 2>&1 | tee -a performance.log
git checkout src/lib.rs

# ベンチマーク結果
cargo bench 2>&1 | tee -a performance.log
```

## 注意事項

- テストは並列実行されるため、ファイル名の衝突に注意
- ベンチマークは安定した環境で実行（他のプロセスを停止）
- 自己インデックスは最も包括的な動作確認方法
- 差分インデックスの性能は特に重要（頻繁に実行されるため）