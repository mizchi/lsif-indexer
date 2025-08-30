# 開発ガイド

## プロジェクト構成

```
lsif-indexer/
├── src/
│   ├── core/           # コアロジック（グラフ、LSIF、クエリ）
│   │   ├── graph.rs
│   │   ├── lsif.rs
│   │   └── incremental.rs
│   ├── cli/            # CLI、言語アダプタ、ユーティリティ
│   │   ├── mod.rs
│   │   ├── simple_cli.rs
│   │   ├── lsp_adapter.rs
│   │   ├── go_adapter.rs
│   │   ├── python_adapter.rs
│   │   └── typescript_adapter.rs
│   └── bin/
│       ├── lsif.rs     # メインCLIエントリポイント
│       └── main.rs     # 代替エントリポイント
├── tests/              # 統合テスト
│   ├── fixtures/       # テスト用データ
│   └── *_test.rs      # 各種テスト
├── benches/           # ベンチマーク
└── docs/              # 追加ドキュメント
```

## 開発環境セットアップ

### 必要なツール

```bash
# Rust（最新安定版）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 開発ツール
cargo install cargo-watch   # ファイル監視
cargo install cargo-edit    # 依存関係管理
cargo install cargo-audit   # セキュリティ監査

# LSPサーバー（テスト用）
cargo install rust-analyzer
npm install -g typescript-language-server
go install golang.org/x/tools/gopls@latest
pip install python-lsp-server
```

## ビルドとテスト

### ビルド

```bash
# デバッグビルド
cargo build

# リリースビルド（最適化済み）
cargo build --release

# 特定機能のみ
cargo build --features "lsp"
```

### テスト

```bash
# 全テスト実行
cargo test

# 特定テストの実行
cargo test graph_test        # グラフ機能
cargo test lsp_integration   # LSP統合
cargo test differential      # 差分インデックス

# 統合テスト（順次実行）
cargo test --test '*' -- --test-threads=1

# カバレッジ計測
cargo install cargo-llvm-cov
cargo llvm-cov --html --output-dir tmp/coverage
```

### ベンチマーク

```bash
# 全ベンチマーク
cargo bench

# 特定ベンチマーク
cargo bench graph_construction
cargo bench symbol_operations

# 結果の比較
cargo bench -- --save-baseline main
cargo bench -- --baseline main
```

## コーディング規約

### スタイルガイド

- **フォーマット**: `cargo fmt` を実行
- **Linting**: `cargo clippy -- -D warnings` でエラーゼロ
- **命名規則**:
  - モジュール/ファイル: `snake_case`
  - 型/トレイト: `UpperCamelCase`
  - 関数/変数: `snake_case`
  - 定数: `SCREAMING_SNAKE_CASE`

### コミットメッセージ

```
<type>: <subject>

<body>

<footer>
```

タイプ:
- `feat`: 新機能
- `fix`: バグ修正
- `perf`: パフォーマンス改善
- `refactor`: リファクタリング
- `test`: テスト追加/修正
- `docs`: ドキュメント
- `chore`: ビルド/ツール関連

例:
```
feat: Go言語サポートを追加

- gopls統合を実装
- テストカバレッジ85%達成
- ドキュメント更新
```

## 新機能の追加

### 新しい言語サポート

1. `src/cli/` に言語アダプタを作成
2. `MinimalLanguageAdapter` トレイトを実装
3. `src/cli/language_detector.rs` に言語検出を追加
4. テストを `tests/` に追加
5. ドキュメントを更新

例: `src/cli/ruby_adapter.rs`
```rust
pub struct RubyAdapter;

impl MinimalLanguageAdapter for RubyAdapter {
    fn language_id(&self) -> &str {
        "ruby"
    }
    
    fn spawn_lsp_command(&self) -> Result<Child> {
        // Ruby LSPを起動
        Command::new("solargraph")
            .args(&["stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to spawn Ruby LSP")
    }
}
```

### 新しいクエリ機能

1. `src/core/graph_query.rs` にクエリロジックを追加
2. `src/cli/simple_cli.rs` にCLIコマンドを追加
3. テストとドキュメントを追加

## デバッグ

### ログレベル

```bash
# デバッグログ
RUST_LOG=debug cargo run

# 特定モジュールのみ
RUST_LOG=lsif_indexer::cli::lsp_adapter=trace cargo run

# 本番環境
RUST_LOG=warn cargo run
```

### プロファイリング

```bash
# CPU プロファイル
cargo install flamegraph
cargo flamegraph --bin lsif -- index

# メモリプロファイル
valgrind --tool=massif cargo run
ms_print massif.out.*
```

## CI/CD

### GitHub Actions

すべてのPRは以下のチェックを通過する必要があります：

- `cargo test` - テスト
- `cargo clippy` - Linting
- `cargo fmt` - フォーマット
- `cargo bench` - パフォーマンス劣化チェック
- `cargo audit` - セキュリティ監査

### リリース前チェックリスト

- [ ] すべてのテストが成功
- [ ] Clippyエラーなし
- [ ] フォーマット済み
- [ ] ベンチマークで劣化なし
- [ ] ドキュメント更新
- [ ] CHANGELOG更新
- [ ] バージョン番号更新

## トラブルシューティング

### よくある問題

**Q: LSPサーバーが起動しない**
```bash
# LSPサーバーがインストールされているか確認
which gopls
which pylsp
which typescript-language-server

# 手動インストール
npm install -g typescript-language-server
```

**Q: テストが失敗する**
```bash
# キャッシュクリア
cargo clean
rm -rf tmp/

# 依存関係更新
cargo update
```

**Q: パフォーマンスが悪い**
```bash
# リリースビルドを使用
cargo build --release

# CPU ガバナー設定（Linux）
sudo cpupower frequency-set -g performance
```

## 貢献方法

1. Issueで議論
2. フォークしてブランチ作成
3. 変更を実装
4. テストを追加
5. PRを作成

詳細は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。