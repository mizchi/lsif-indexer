# LSIF Indexer

高速な言語非依存コードインデックスツール。LSPベースの統一インターフェースで複数言語に対応。

[![CI](https://github.com/mizchi/lsif-indexer/actions/workflows/ci.yml/badge.svg)](https://github.com/mizchi/lsif-indexer/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/badge/coverage-44%25-yellow)](https://github.com/mizchi/lsif-indexer)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

## 特徴

- 🚀 **超高速**: 0.6秒以下でフルインデックス、0.05秒で差分更新
- 🌍 **マルチ言語**: Rust, Go, Python, TypeScript/JavaScript対応
- 🔌 **LSP統合**: 各言語のLSPサーバーとネイティブ統合
- 💾 **低メモリ**: 50MB以下の効率的な動作
- 🤖 **AIフレンドリー**: シンプルなCLIでAIツールとの統合が容易

## インストール

```bash
# Rustツールチェーンのインストール（必須）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# lsif-indexerのインストール
cargo install --path .

# 言語別LSPサーバー（オプション）
cargo install rust-analyzer                          # Rust
go install golang.org/x/tools/gopls@latest          # Go
pip install python-lsp-server                       # Python
npm install -g typescript-language-server typescript # TypeScript/JavaScript
```

## 使い方

### 基本コマンド

```bash
# プロジェクトのインデックス作成
lsif index                      # 言語自動検出
lsif index -l rust             # 特定言語指定
lsif index --project ./src     # ディレクトリ指定

# コード検索
lsif definition main.rs 42     # 定義へジャンプ
lsif references main.go 10     # 参照を検索
lsif symbols                   # シンボル一覧

# 曖昧検索
lsif workspace-symbols "user" --fuzzy  # ファジー検索
lsif workspace-symbols "usr" --fuzzy   # 部分一致

# 高度な機能
lsif call-hierarchy main --depth 3     # コールヒエラルキー
lsif unused                            # 未使用コード検出
lsif graph "(n:Function)"              # グラフクエリ
```

### オプション

| オプション | 説明 | デフォルト |
|----------|------|-----------|
| `--db <path>` | インデックスDBのパス | `.lsif-index.db` |
| `--project <path>` | プロジェクトルート | `.` |
| `--language <lang>` | 言語指定 | 自動検出 |
| `--no-auto-index` | 自動インデックス無効化 | false |
| `--json` | JSON出力 | false |

## パフォーマンス

実測値（自プロジェクト、約12,000行）:

| 操作 | 時間 | 詳細 |
|------|------|------|
| **フルインデックス** | 0.595秒 | 並列処理、CPU使用率250% |
| **差分更新** | 0.05秒 | 2-3ファイルの変更 |
| **メモリ使用量** | < 50MB | 全言語共通 |
| **インデックスサイズ** | 3.3MB | 圧縮済み |

## サポート言語

| 言語 | LSPサーバー | サポート機能 |
|------|------------|-------------|
| **Rust** | rust-analyzer | フル機能 |
| **Go** | gopls | フル機能 |
| **Python** | pylsp/pyright | フル機能 |
| **TypeScript** | typescript-language-server | フル機能 |
| **JavaScript** | typescript-language-server | フル機能 |

## コマンド一覧

### LSP標準コマンド

| コマンド | 説明 | LSPメソッド |
|----------|------|------------|
| `definition` | 定義へジャンプ | textDocument/definition |
| `references` | 参照を検索 | textDocument/references |
| `symbols` | ドキュメントシンボル | textDocument/documentSymbol |
| `workspace-symbols` | ワークスペース検索 | workspace/symbol |
| `call-hierarchy` | 呼び出し階層 | textDocument/prepareCallHierarchy |
| `type-definition` | 型定義へ | textDocument/typeDefinition |
| `implementation` | 実装を検索 | textDocument/implementation |

### 拡張コマンド

| コマンド | 説明 |
|----------|------|
| `index` | プロジェクトをインデックス |
| `graph` | Cypherクエリ実行 |
| `unused` | 未使用コード検出 |
| `diff` | 変更影響範囲表示 |
| `status` | インデックス状態確認 |
| `export` | LSIF/JSON形式エクスポート |

## アーキテクチャ

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   CLI/API   │────▶│   Core      │────▶│   Storage   │
└─────────────┘     └─────────────┘     └─────────────┘
       │                   ▲                     │
       ▼                   │                     ▼
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ LSP Adapter │────▶│ Graph Model │     │   Sled DB   │
└─────────────┘     └─────────────┘     └─────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────┐
│              Language Servers (LSP)                 │
│  rust-analyzer │ gopls │ pylsp │ tsserver          │
└─────────────────────────────────────────────────────┘
```

## ドキュメント

- [開発ガイド](docs/DEVELOPMENT.md) - 開発環境セットアップとコーディング規約
- [パフォーマンス](docs/PERFORMANCE.md) - ベンチマークと最適化
- [新言語サポート追加](docs/adding-new-language-support.md) - 新しい言語の追加方法
- [言語比較](docs/language-comparison.md) - 各言語の特性と対応状況
- [アーキテクチャ](docs/language-agnostic-design.md) - 言語非依存設計の詳細

## 貢献

貢献を歓迎します！

1. Issueで機能提案や不具合報告
2. Pull Requestで改善を提案
3. ドキュメントの改善

詳細は[開発ガイド](docs/DEVELOPMENT.md)をご覧ください。

## ライセンス

MIT License - 詳細は[LICENSE](LICENSE)を参照してください。