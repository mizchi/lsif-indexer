# LSPパフォーマンス最適化ガイド

## 概要

このドキュメントは、LSIF IndexerにおけるLSP（Language Server Protocol）のパフォーマンス最適化に関する分析結果と実装戦略をまとめたものです。

## パフォーマンス計測結果

### LSPサーバー比較

| LSPサーバー | 初期化時間 | workspace/symbol | documentSymbol | 特徴 |
|------------|-----------|-----------------|----------------|------|
| **tsgo** | 0.078秒 ✅ | 0.000-0.007秒 | N/A | TypeScript Native Preview、最速の初期化 |
| **rust-analyzer** | 0.082秒 | 0.001-0.069秒 | N/A | 特定クエリで超高速（0.001秒） |
| **gopls** | 未計測 | - | - | go.modベースの正確なモジュール解決 |

### 操作別パフォーマンス特性

#### workspace/symbol（プロジェクト全体のシンボル検索）

| クエリ | tsgo | rust-analyzer |
|--------|------|---------------|
| 'main' | 0.007秒 | **0.001秒** ✅ |
| 'test' | **0.001秒** ✅ | 0.000秒 ✅ |
| 'handle' | **0.000秒** ✅ | 0.069秒 |
| 'process' | **0.000秒** ✅ | 0.008秒 |
| '' (全シンボル) | **0.000秒** ✅ | エラー |

## 実装済みの最適化戦略

### 1. LSPプール管理 (`lsp_pool.rs`)

```rust
pub struct PoolConfig {
    /// 言語ごとの最大インスタンス数（推奨: 4）
    pub max_instances_per_language: 4,
    /// クライアントの最大アイドル時間
    pub max_idle_time: Duration::from_secs(300),
    /// 初期化タイムアウト
    pub init_timeout: Duration::from_secs(5),
    /// リクエストタイムアウト
    pub request_timeout: Duration::from_secs(2),
}
```

**特徴:**
- 言語ごとに最大4インスタンスまで保持
- ラウンドロビン方式での負荷分散
- アイドルインスタンスの自動削除
- 参照カウントによる効率的な管理

### 2. 階層的キャッシュシステム (`hierarchical_cache.rs`)

```
L1キャッシュ: メモリ内（100ms TTL）
  └─ documentSymbol結果
  
L2キャッシュ: ディスク（1秒 TTL）
  └─ workspace/symbol結果
  
L3キャッシュ: 永続化DB（無期限）
  └─ 定義・参照情報
```

**キャッシュ無効化:**
- ファイル変更時に自動的に関連キャッシュを削除
- LRU方式でメモリ効率を最適化

### 3. 適応的タイムアウト設定 (`timeout_predictor.rs`)

| 操作 | 初回 | 通常 | 最大 |
|------|------|------|------|
| Initialize | 5秒 | 2秒 | 30秒 |
| WorkspaceSymbol | 2秒 | 500ms | 5秒 |
| DocumentSymbol | 1秒 | 200ms | 2秒 |
| Definition/References | 1.5秒 | 300ms | 3秒 |

**適応的調整:**
- 成功が10回続いたら「通常」タイムアウトに短縮
- 失敗が3回続いたらタイムアウトを1.5倍に延長
- 操作履歴から最適な値を学習

### 4. 言語別最適化 (`language_optimization.rs`)

#### TypeScript/JavaScript
- **優先LSP**: tsgo（起動0.078秒、空クエリ対応）
- **フォールバック**: typescript-language-server
- **戦略**: workspace/symbolを積極的にキャッシュ

#### Rust
- **優先LSP**: rust-analyzer
- **戦略**: 大規模プロジェクトではインデックスをプリロード
- **特徴**: 特定クエリで0.001秒の高速応答

#### Go
- **優先LSP**: gopls
- **戦略**: go.modベースでワークスペースを管理
- **注意**: 初期化に時間がかかる傾向

### 5. メトリクス収集 (`lsp_metrics.rs`)

収集するメトリクス:
- 操作別のレスポンス時間統計
- キャッシュヒット率（L1/L2/L3）
- LSPプールの再利用率
- エラー率と成功率

## ベストプラクティス

### インクリメンタル更新戦略

1. **Git差分を使用した変更ファイル特定**
   - 変更ファイルのみ再インデックス
   - 10ファイル単位でバッチ処理

2. **優先度付きキュー**
   - 最近アクセスされたファイルを優先
   - ユーザーが開いているファイルを最優先

3. **並列処理の最適化**
   - 最大4インスタンス/言語で並列化
   - CPU親和性を考慮したスケジューリング

### コネクション管理

```rust
// Keep-alive機能
async fn maintain_connection(&mut self) {
    // 30秒ごとにヘルスチェック
    self.send_notification("$/ping", json!({}));
}

// 自動再接続
async fn auto_reconnect(&mut self) -> Result<()> {
    if !self.is_connected() {
        self.restart()?;
        self.initialize().await?;
    }
}
```

### フォールバック機構

1. プライマリLSPが失敗 → セカンダリLSPを試行
2. LSPが利用不可 → 正規表現ベースのフォールバック
3. 完全失敗 → エラーをユーザーに報告

## パフォーマンス目標と実績

| メトリクス | 目標値 | 実績値 | 状態 |
|-----------|--------|--------|------|
| 初回フルインデックス | < 1.5秒 | 1.2秒 | ✅ 達成 |
| 差分インデックス | < 0.15秒 | 0.06-0.12秒 | ✅ 達成 |
| メモリ使用量 | < 150MB | 測定中 | 🔄 |
| ファイル処理速度 | > 30 files/sec | 測定中 | 🔄 |

## トラブルシューティング

### LSPサーバーが遅い場合

1. **タイムアウト設定の確認**
   ```bash
   # 適応的タイムアウトのリセット
   RUST_LOG=debug cargo run -- index --reset-timeouts
   ```

2. **キャッシュのクリア**
   ```bash
   rm -rf tmp/lsp_cache/
   ```

3. **プール設定の調整**
   ```rust
   // 環境に応じて調整
   max_instances_per_language: 2, // 低メモリ環境
   max_instances_per_language: 8, // 高性能環境
   ```

### メモリ使用量が多い場合

1. インスタンス数を削減
2. キャッシュサイズを制限
3. アイドルタイムアウトを短縮

## 今後の改善点

1. **WebSocketベースの永続接続**: stdio通信のオーバーヘッド削減
2. **増分解析**: ファイルの差分のみを再解析
3. **プリフェッチング**: 関連ファイルの先読み
4. **分散キャッシュ**: Redisベースの共有キャッシュ
5. **機械学習による予測**: アクセスパターンの学習

## 関連ドキュメント

- [開発ガイド](./DEVELOPMENT.md)
- [パフォーマンス最適化](./performance-optimization-summary.md)
- [言語サポート追加](./adding-new-language-support.md)