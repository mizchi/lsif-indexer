# 言語非依存設計ドキュメント

## 概要

LSIF Indexerを言語非依存の汎用ツールとして設計し、LSP（Language Server Protocol）から取得できるデータを最大限活用することで、言語固有の実装を最小限に抑える。

## 設計原則

### 1. LSPファースト
- **インデックス作成**: LSPのDocumentSymbol/SymbolInformationを標準データソースとする
- **定義・参照**: LSPのtextDocument/definition、textDocument/referencesを活用
- **型情報**: LSPのhoverやsignatureHelpから取得

### 2. 言語固有処理の最小化
- 言語アダプタは以下の最小限の機能のみ提供:
  - LSPサーバーの起動コマンド
  - ファイル拡張子の判定
  - 参照パターンの微調整（必要な場合のみ）

### 3. 汎用アルゴリズムの活用
- 文字列/コメント判定
- キーワードベースの定義検出
- 単語境界マッチング

## アーキテクチャ

```
┌─────────────────────────────────────────┐
│          Application Layer              │
│  (CLI Commands, Query Interface)        │
└─────────────────────────────────────────┘
                    │
┌─────────────────────────────────────────┐
│         Core Indexing Engine            │
│   (Language-Agnostic Processing)        │
├─────────────────────────────────────────┤
│ • Symbol extraction from LSP            │
│ • Generic pattern matching              │
│ • Cross-reference analysis              │
│ • Call hierarchy construction           │
└─────────────────────────────────────────┘
                    │
┌─────────────────────────────────────────┐
│           LSP Interface Layer           │
│    (Standard LSP Communication)         │
└─────────────────────────────────────────┘
                    │
┌─────────────────────────────────────────┐
│     Minimal Language Adapters           │
├─────────────────────────────────────────┤
│ Rust    │ TypeScript │ Python │  ...   │
│ Adapter │  Adapter   │ Adapter│        │
└─────────────────────────────────────────┘
```

## 実装詳細

### Phase 1: インデックス作成（完全にLSP依存）

```rust
// 言語非依存のインデックス処理
impl LspIndexer {
    pub fn index_from_lsp(&mut self, symbols: Vec<DocumentSymbol>) -> Result<()> {
        // LSPのシンボル情報をそのまま使用
        for symbol in symbols {
            self.process_lsp_symbol(&symbol)?;
        }
        Ok(())
    }
}
```

**利点**:
- 新言語サポートが容易（LSPサーバーがあれば即対応）
- 言語の構文を知る必要がない
- LSPサーバーの品質に依存した正確な解析

### Phase 2: 参照検索（軽量な言語固有処理）

```rust
// 基本的な参照パターン（99%のケースをカバー）
fn build_basic_reference_pattern(name: &str) -> String {
    format\!(r"{}", regex::escape(name))
}

// 言語固有の拡張（必要な場合のみ）
trait MinimalLanguageAdapter {
    fn extend_reference_pattern(&self, basic_pattern: &str, context: &Context) -> String {
        basic_pattern.to_string() // デフォルトは変更なし
    }
}
```

**言語別の最小限の拡張**:

| 言語 | 拡張内容 | 理由 |
|------|----------|------|
| Rust | `::` パス対応 | モジュールシステム |
| C++ | `::`, `->`, `.` | 名前空間とポインタ |
| Go | `.` のみ | パッケージ参照 |
| Python | `.` のみ | モジュール参照 |
| JavaScript/TypeScript | なし | 基本パターンで十分 |

### Phase 3: 汎用ヘルパー関数

```rust
// すべての言語で共通利用可能
mod generic_helpers {
    // 文字列/コメント判定（C系言語で共通）
    pub fn is_in_string_or_comment(line: &str, pos: usize) -> bool {
        // "//" コメント
        // "..." 文字列
        // '...' 文字/文字列
        // の汎用判定
    }
    
    // ブロックコメント対応版
    pub fn is_in_block_comment(
        content: &str, 
        pos: usize,
        block_start: &str,  // "/*", "<\!--", etc.
        block_end: &str     // "*/", "-->", etc.
    ) -> bool {
        // 汎用的なブロックコメント判定
    }
}
```

## 新言語サポートの追加手順

### 最小限の実装（5分で完了）

```rust
pub struct NewLanguageAdapter;

impl MinimalLanguageAdapter for NewLanguageAdapter {
    fn language_id(&self) -> &str { "new_lang" }
    fn file_extensions(&self) -> Vec<&str> { vec\!["ext"] }
    fn lsp_command(&self) -> Command {
        Command::new("new-lang-lsp")
    }
}
```

### オプション拡張（必要に応じて）

```rust
impl NewLanguageAdapter {
    // 特殊な参照パターンが必要な場合のみ
    fn extend_reference_pattern(&self, basic: &str, ctx: &Context) -> String {
        if ctx.symbol_kind == SymbolKind::Module {
            format\!("{}(?:\.\w+)*", basic) // モジュール.メンバー対応
        } else {
            basic.to_string()
        }
    }
}
```

## パフォーマンス最適化

### 1. LSPキャッシュ
- シンボル情報をメモリキャッシュ
- 差分更新時は変更ファイルのみ再取得

### 2. 並列処理
- ファイル単位での並列インデックス
- 言語非依存なので並列化が容易

### 3. インクリメンタル更新
- LSPのdidChangeイベントを活用
- 変更箇所のみ再インデックス

## 利点

1. **保守性**: 言語固有コードが最小限
2. **拡張性**: 新言語追加が極めて簡単
3. **正確性**: LSPサーバーの解析結果を信頼
4. **一貫性**: すべての言語で同じ動作
5. **テスト容易性**: 言語非依存部分のテストが簡単

## 制限事項

1. **LSP依存**: LSPサーバーの品質に依存
2. **カスタム解析**: 言語固有の高度な解析は困難
3. **パフォーマンス**: LSPサーバーの起動オーバーヘッド

## 移行計画

### Step 1: 現状の整理（完了）
- [x] 言語アダプタの責務を明確化
- [x] LSP依存部分と言語固有部分を分離

### Step 2: 汎用化（進行中）
- [ ] 汎用ヘルパー関数の拡充
- [ ] 言語アダプタインターフェースの簡素化
- [ ] テストの言語非依存化

### Step 3: 新言語での検証
- [ ] Python対応（最小限実装）
- [ ] Go対応（最小限実装）
- [ ] 性能・精度の評価

## まとめ

言語非依存の設計により、LSIF Indexerは真の汎用コード解析ツールとなる。LSPを基盤とすることで、言語の詳細を知らなくても高品質なインデックスを作成でき、新言語のサポートも最小限の労力で実現できる。
