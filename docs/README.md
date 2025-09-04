# LSIF Indexer ドキュメント

## 📚 ドキュメント一覧

### 開発者向け

#### 入門
- [開発ガイド](./DEVELOPMENT.md) - 開発環境のセットアップと基本的な開発フロー
- [言語サポート追加](./adding-new-language-support.md) - 新しい言語のサポートを追加する方法

#### パフォーマンス
- [LSPパフォーマンス最適化](./lsp-performance-optimization.md) - LSP統合の最適化戦略 **NEW**
- [パフォーマンス最適化サマリー](./performance-optimization-summary.md) - 全体的な最適化手法
- [パフォーマンス実装ガイド](./performance-implementation-guide.md) - 具体的な実装方法
- [ロックフリー実装](./lockfree-implementation-details.md) - 並行処理の最適化

#### トラブルシューティング
- [トラブルシューティング](./troubleshooting.md) - よくある問題と解決方法 **NEW**
- [ベンチマーク](./benchmark.md) - パフォーマンステストの実行方法

### アーキテクチャ

- [言語非依存設計](./language-agnostic-design.md) - コア設計の原則
- [言語比較](./language-comparison.md) - 各言語の実装比較
- [パフォーマンス分析](./performance-optimization-analysis.md) - ボトルネック分析

### プロジェクト管理

- [ロードマップ](./roadmap.md) - 今後の開発計画 **NEW**
- [パフォーマンス目標](./PERFORMANCE.md) - 目標メトリクス

## 🔍 クイックリンク

### よく参照される項目

1. **パフォーマンスが遅い場合**
   - [トラブルシューティング > パフォーマンス問題](./troubleshooting.md#パフォーマンス問題)
   - [LSPパフォーマンス最適化](./lsp-performance-optimization.md)

2. **新しい言語を追加したい**
   - [言語サポート追加ガイド](./adding-new-language-support.md)
   - [言語別最適化戦略](./lsp-performance-optimization.md#言語別最適化)

3. **開発を始める**
   - [開発環境セットアップ](./DEVELOPMENT.md#開発環境セットアップ)
   - [開発ワークフロー](./DEVELOPMENT.md#開発ワークフロー)

## 📊 最新の更新状況

### 2025-09-04 更新
- ✅ LSPパフォーマンス最適化完了
  - 階層的キャッシュシステム実装
  - 適応的タイムアウト機能追加
  - 言語別最適化戦略実装
- ✅ ドキュメント統合・整理
  - 重複コンテンツの削除
  - 新規トラブルシューティングガイド作成
  - ロードマップの更新

### パフォーマンス達成状況
| メトリクス | 目標 | 現在 | 状態 |
|-----------|------|------|------|
| 初回インデックス | <1.5秒 | 1.2秒 | ✅ |
| 差分インデックス | <0.15秒 | 0.06-0.12秒 | ✅ |
| メモリ使用量 | <150MB | 測定中 | 🔄 |

## 🤝 貢献方法

ドキュメントの改善や追加は歓迎します：

1. 誤字脱字の修正
2. より分かりやすい説明への改善
3. 新しい例の追加
4. 未文書化機能のドキュメント作成

[GitHub](https://github.com/yourusername/lsif-indexer)でPull Requestを送ってください。

## 📝 ドキュメント規約

### ファイル命名
- 小文字とハイフンを使用: `feature-name.md`
- 略語は大文字可: `LSP-optimization.md`

### 内容構成
1. タイトルと概要
2. 目次（長いドキュメントの場合）
3. 本文（見出しで構造化）
4. 例とコードサンプル
5. 関連リンク

### 更新時の注意
- 最終更新日を記載
- 変更履歴を残す（重要な変更のみ）
- 古い情報は削除せず取り消し線で残す場合がある