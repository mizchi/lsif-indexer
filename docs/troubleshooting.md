# トラブルシューティングガイド

## パフォーマンス問題

### 問題: 初回インデックスが遅い（140ファイルで8分以上）

**症状:**
- 大規模プロジェクトで初回インデックスが30秒以上かかる
- 100ファイル以上でタイムアウトが発生

**原因:**
1. LSPサーバーの起動オーバーヘッド
2. 並列処理のMutex競合
3. 正規表現のコンパイルコスト
4. 非効率なファイルI/O

**解決策:**

1. **フォールバックモードを使用**
   ```bash
   # LSPを使わずに高速インデックス
   lsif-indexer index --fallback-only
   
   # 環境変数でも設定可能
   export LSIF_FALLBACK_ONLY=1
   ```

2. **並列度の調整**
   ```bash
   # CPUコア数に応じて調整
   lsif-indexer index --parallel 4
   ```

3. **キャッシュのクリア**
   ```bash
   rm -rf tmp/lsp_cache/
   rm -rf tmp/*.db
   ```

4. **タイムアウト設定の調整**
   ```bash
   # より短いタイムアウトで高速化
   RUST_LOG=debug cargo run -- index --reset-timeouts
   ```

### 問題: メモリ使用量が多い

**症状:**
- 1000ファイル以上でメモリ不足
- LSPインスタンスが累積してメモリを圧迫

**解決策:**

1. **LSPプール設定の調整**
   ```rust
   // 低メモリ環境向け設定
   max_instances_per_language: 2,  // デフォルト4から削減
   max_idle_time: Duration::from_secs(60), // 5分から1分に短縮
   ```

2. **キャッシュサイズの制限**
   ```rust
   l1_max_entries: 500,  // デフォルト1000から削減
   l2_max_size_bytes: 50 * 1024 * 1024, // 100MBから50MBに
   ```

3. **ガベージコレクションの強制実行**
   ```bash
   # 定期的にアイドルクライアントをクリーンアップ
   lsif-indexer cleanup-idle
   ```

## LSP関連の問題

### 問題: LSPサーバーが起動しない

**症状:**
- "Failed to spawn LSP process"エラー
- タイムアウトエラーが頻発

**原因:**
- LSPサーバーがインストールされていない
- PATHが通っていない
- 権限不足

**解決策:**

1. **必要なLSPサーバーをインストール**
   ```bash
   # Rust
   rustup component add rust-analyzer
   
   # TypeScript
   npm install -g @typescript/native-preview
   # または
   npm install -g typescript-language-server
   
   # Go
   go install golang.org/x/tools/gopls@latest
   
   # Python
   pip install python-lsp-server
   ```

2. **PATHを確認**
   ```bash
   which rust-analyzer
   which tsgo
   which gopls
   ```

3. **権限を確認**
   ```bash
   chmod +x $(which rust-analyzer)
   ```

### 問題: workspace/symbolが失敗する

**症状:**
- "no project found for URI"エラー
- 空のシンボルリストが返される

**原因:**
- プロジェクト設定ファイルがない
- ワークスペースフォルダが正しくない

**解決策:**

1. **プロジェクト設定ファイルを作成**
   ```bash
   # TypeScript
   echo '{}' > tsconfig.json
   
   # Go
   go mod init myproject
   
   # Rust
   cargo init
   ```

2. **ワークスペースルートを指定**
   ```bash
   lsif-indexer index -p /path/to/project/root
   ```

## インデックス関連の問題

### 問題: 定義へのジャンプが不正確

**症状:**
- シンボル位置がずれている
- 間違ったファイルにジャンプする

**原因:**
- フォールバックインデクサーの位置情報が不正確
- 文字エンコーディングの問題

**解決策:**

1. **LSPモードを使用（精度優先）**
   ```bash
   # フォールバックを無効化
   lsif-indexer index --no-fallback
   ```

2. **UTF-8エンコーディングを確認**
   ```bash
   file -i *.rs
   ```

### 問題: 差分インデックスが動作しない

**症状:**
- 常にフルインデックスが実行される
- ファイル変更が検知されない

**原因:**
- Gitリポジトリではない
- ハッシュ計算の問題

**解決策:**

1. **Gitリポジトリを初期化**
   ```bash
   git init
   git add .
   git commit -m "Initial commit"
   ```

2. **ハッシュキャッシュをクリア**
   ```bash
   rm tmp/*.hash
   ```

## データベース関連の問題

### 問題: "Database is corrupted"エラー

**症状:**
- インデックス読み込み時にパニック
- 不正なデータが返される

**解決策:**

1. **データベースを再作成**
   ```bash
   rm tmp/*.db
   lsif-indexer index --force
   ```

2. **整合性チェック**
   ```bash
   lsif-indexer validate -i index.db
   ```

### 問題: ディスク容量不足

**症状:**
- "No space left on device"エラー
- インデックス作成が途中で停止

**解決策:**

1. **不要なキャッシュを削除**
   ```bash
   # すべての一時ファイルを削除
   rm -rf tmp/*
   
   # 古いインデックスのみ削除
   find tmp -name "*.db" -mtime +7 -delete
   ```

2. **圧縮オプションを使用**
   ```bash
   lsif-indexer index --compress
   ```

## デバッグ方法

### 詳細ログの有効化

```bash
# デバッグレベルのログ
RUST_LOG=debug lsif-indexer index

# 特定モジュールのトレース
RUST_LOG=lsif_indexer::cli::git_diff=trace cargo run -- differential-index

# LSP通信のダンプ
RUST_LOG=lsp=trace cargo run -- index
```

### プロファイリング

```bash
# CPU使用率の分析
cargo install flamegraph
cargo flamegraph --bin lsif-indexer -- index-project -p . -o tmp/test.db

# メモリ使用量の分析
valgrind --tool=massif cargo run --release -- index
ms_print massif.out.*

# 実行時間の測定
time cargo run --release -- index
/usr/bin/time -v cargo run --release -- index
```

### ベンチマーク

```bash
# パフォーマンステスト
cargo bench

# 特定のベンチマーク
cargo bench index_performance
cargo bench storage
cargo bench cache

# 結果の比較
cargo bench -- --save-baseline main
git checkout feature-branch
cargo bench -- --baseline main
```

## よくある質問（FAQ）

### Q: どの言語が最もパフォーマンスが良いですか？

A: パフォーマンス分析の結果：
- **TypeScript**: tsgoが最速（初期化0.078秒）
- **Rust**: rust-analyzerが特定クエリで最速（0.001秒）
- **Python**: 正規表現フォールバックの方が速い場合が多い

### Q: 最適な並列度は？

A: CPU論理コア数の半分が推奨：
- 4コア → 2並列
- 8コア → 4並列
- 16コア → 8並列

### Q: キャッシュはいつクリアすべき？

A: 以下の場合にクリア推奨：
- LSP設定を変更した後
- 大規模なリファクタリング後
- パフォーマンスが劣化した時
- エラーが頻発する時

### Q: メモリ不足を防ぐには？

A: 以下の設定を調整：
- `max_instances_per_language`: 2以下に
- `max_idle_time`: 60秒以下に
- `l1_max_entries`: 500以下に
- フォールバックモードを使用

## サポート

問題が解決しない場合：

1. [GitHubでIssueを作成](https://github.com/yourusername/lsif-indexer/issues)
2. 以下の情報を含める：
   - エラーメッセージ全文
   - 実行したコマンド
   - OS/環境情報
   - `RUST_LOG=debug`での実行ログ