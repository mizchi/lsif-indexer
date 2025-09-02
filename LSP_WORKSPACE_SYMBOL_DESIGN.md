# LSP Workspace Symbolベースの高速インデックス設計

## 問題の背景

現在のLSIFインデクサーは、各ファイルを個別に開いてdocumentSymbolを取得する方式を採用しています。
これにより、140ファイルの処理に8分以上かかっています。

しかし、rust-analyzer自体のベンチマークでは：
- 初期化: 0.07秒
- workspace/symbol（空クエリ）: 4.2秒

つまり、workspace/symbol APIを使えば、プロジェクト全体のシンボルを5秒以内に取得できます。

## 解決策

### 1. workspace/symbol APIの活用

workspace/symbolは、プロジェクト全体からシンボルを検索するLSPのAPIです。
空のクエリを送信することで、全シンボルを取得できます。

```json
// リクエスト
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "workspace/symbol",
  "params": {
    "query": ""
  }
}

// レスポンス
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": [
    {
      "name": "CodeGraph",
      "kind": 5,  // Class
      "location": {
        "uri": "file:///path/to/file.rs",
        "range": {...}
      },
      "containerName": "lsif_core"
    },
    // ...さらに多くのシンボル
  ]
}
```

### 2. 実装方法

#### ステップ1: LSPサーバー起動
```rust
let mut command = Command::new("rust-analyzer");
command.stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
let process = command.spawn()?;
```

#### ステップ2: 初期化
```rust
let params = InitializeParams {
    capabilities: ClientCapabilities {
        workspace: Some(WorkspaceClientCapabilities {
            symbol: Some(WorkspaceSymbolClientCapabilities {
                dynamic_registration: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    },
    workspace_folders: Some(vec![WorkspaceFolder {
        uri: root_uri,
        name: "workspace".to_string(),
    }]),
    ..Default::default()
};
```

#### ステップ3: workspace/symbol呼び出し
```rust
let params = WorkspaceSymbolParams {
    query: "".to_string(),  // 空クエリで全シンボル取得
    ..Default::default()
};

let symbols: Vec<SymbolInformation> = 
    client.send_request("workspace/symbol", params)?;
```

### 3. パフォーマンス比較

| 方法 | 所要時間 | メモリ使用量 | 精度 |
|------|----------|--------------|--------|
| documentSymbol（現在） | 8刉55秒 | 高 | 高 |
| workspace/symbol | ~5秒 | 低 | 高 |
| フォールバックパーサー | ~70秒 | 中 | 低 |

### 4. 利点

1. **高速**: プロジェクト全体を数秒でインデックス
2. **シンプル**: ファイルを個別に開く必要がない
3. **正確**: LSPサーバーの解析結果を使用
4. **スケーラブル**: 大規模プロジェクトでも高速

### 5. 制限事項

1. **LSPサーバー依存**: workspace/symbolをサポートするLSPサーバーが必要
2. **初回のみ高速**: 差分更新には別の戦略が必要
3. **メモリ使用**: 全シンボルを一度にメモリに読み込む

### 6. 対応言語

workspace/symbolをサポートする主要なLSPサーバー：

- **Rust**: rust-analyzer ✅
- **TypeScript/JavaScript**: tsgo ✅ (--lsp --stdio)
- **Python**: pylsp ✅
- **Go**: gopls ✅
- **C/C++**: clangd ✅
- **Java**: jdtls ✅

### 7. テスト結果

```bash
# rust-analyzerの直接テスト
$ time python test_rust_analyzer_speed.py
Initialization time: 0.07 seconds
Workspace symbols query time: 4.20 seconds
Total symbols found: 3521

real    0m4.35s
```

### 8. 今後の改善点

1. **並列処理**: 複数のLSPサーバーを並列に起動
2. **キャッシュ**: workspace/symbolの結果をキャッシュ
3. **差分更新**: 変更ファイルのみ再取得
4. **プログレッシブロード**: 大規模プロジェクトでの段階的読み込み

## まとめ

workspace/symbol APIを使用することで、インデックス時間を**8分から5秒**に短縮できます。
これは100倍以上の高速化であり、ユーザーエクスペリエンスを大幅に改善します。
