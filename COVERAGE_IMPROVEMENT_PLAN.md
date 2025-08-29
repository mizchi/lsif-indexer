# テストカバレッジ改善計画

## 現状分析

### 現在のカバレッジ
- **ライン**: 32.91%
- **関数**: 33.98%
- **リージョン**: 35.14%

### カバレッジが低い主要モジュール

| モジュール | 現在のカバレッジ | 優先度 |
|-----------|------------------|--------|
| cli/mod.rs | 0% | 高 |
| cli/differential_indexer.rs | 0% | 高 |
| cli/simple_cli.rs | 0% | 高 |
| cli/lsp_minimal_client.rs | 0.93% | 中 |
| core/lsif.rs | 0% | 高 |
| cli/lsp_commands.rs | 0% | 中 |
| cli/lsp_features.rs | 0% | 中 |
| cli/optimized_incremental.rs | 0% | 低 |
| cli/ultra_fast_storage.rs | 0% | 低 |
| core/enhanced_graph.rs | 0% | 低 |

## 改善戦略

### Phase 1: 基盤テスト強化（目標: 50%カバレッジ）

#### 1.1 CLIモジュールのテスト追加
**対象**: `cli/mod.rs`, `cli/simple_cli.rs`
**アプローチ**:
- 統合テストの作成
- 各CLIコマンドの入出力テスト
- モックを使用したコマンド実行テスト

```rust
// tests/cli_commands_test.rs
#[test]
fn test_index_project_command() {
    // CLIコマンドのテスト実装
}

#[test]
fn test_query_command() {
    // クエリコマンドのテスト
}
```

#### 1.2 差分インデクサのテスト
**対象**: `cli/differential_indexer.rs`
**アプローチ**:
- Git操作のモック化
- ファイル変更検知のテスト
- 増分更新ロジックのテスト

```rust
// tests/differential_indexer_comprehensive_test.rs
#[test]
fn test_detect_file_changes() {
    // ファイル変更検知のテスト
}

#[test]
fn test_incremental_update() {
    // 増分更新のテスト
}
```

#### 1.3 LSIF形式処理のテスト
**対象**: `core/lsif.rs`
**アプローチ**:
- LSIF形式の生成テスト
- パース処理のテスト
- エッジケースの検証

```rust
// tests/lsif_format_test.rs
#[test]
fn test_lsif_generation() {
    // LSIF生成のテスト
}

#[test]
fn test_lsif_parsing() {
    // LSIFパースのテスト
}
```

### Phase 2: LSP統合テスト強化（目標: 65%カバレッジ）

#### 2.1 LSPクライアントのモック化
**対象**: `cli/lsp_minimal_client.rs`, `cli/lsp_client.rs`
**アプローチ**:
- モックLSPサーバーの実装
- プロトコル通信のテスト
- エラーハンドリングのテスト

```rust
// tests/lsp_client_mock_test.rs
struct MockLspServer;

#[test]
fn test_lsp_initialization() {
    // LSP初期化のテスト
}

#[test]
fn test_lsp_communication() {
    // LSP通信のテスト
}
```

#### 2.2 LSPコマンドのテスト
**対象**: `cli/lsp_commands.rs`, `cli/lsp_features.rs`
**アプローチ**:
- 各LSP機能の個別テスト
- 統合シナリオテスト

### Phase 3: 最適化モジュールのテスト（目標: 75%カバレッジ）

#### 3.1 パフォーマンス最適化モジュール
**対象**: `cli/optimized_incremental.rs`, `cli/ultra_fast_storage.rs`
**アプローチ**:
- ベンチマークテストの拡充
- パフォーマンス回帰テスト

#### 3.2 拡張グラフ機能
**対象**: `core/enhanced_graph.rs`
**アプローチ**:
- グラフ操作の網羅的テスト
- 複雑なクエリのテスト

## 実装計画

### Week 1-2: Phase 1の実装
- [ ] CLIモジュールのテスト作成
- [ ] 差分インデクサのテスト作成
- [ ] LSIF形式処理のテスト作成
- [ ] カバレッジ50%達成の確認

### Week 3-4: Phase 2の実装
- [ ] モックLSPサーバーの実装
- [ ] LSPクライアントのテスト作成
- [ ] LSPコマンド・機能のテスト作成
- [ ] カバレッジ65%達成の確認

### Week 5-6: Phase 3の実装
- [ ] 最適化モジュールのテスト作成
- [ ] 拡張グラフ機能のテスト作成
- [ ] 全体的なテストカバレッジの最適化
- [ ] カバレッジ75%達成の確認

## テスト作成のベストプラクティス

### 1. モック戦略
- 外部依存（ファイルシステム、ネットワーク）はモック化
- `mockall`クレートの活用
- テスト用のフィクスチャデータの準備

### 2. テストの構造化
```rust
// Arrange-Act-Assert パターンの使用
#[test]
fn test_example() {
    // Arrange: テストデータの準備
    let test_data = prepare_test_data();
    
    // Act: テスト対象の実行
    let result = function_under_test(test_data);
    
    // Assert: 結果の検証
    assert_eq!(result, expected_result);
}
```

### 3. プロパティベーステスト
- `proptest`クレートを使用した自動テスト生成
- エッジケースの自動発見

### 4. 統合テストの最適化
- テストの並列実行
- 共通セットアップの再利用
- テストデータの効率的な管理

## メトリクス目標

| フェーズ | 期間 | ライン | 関数 | リージョン |
|---------|------|--------|------|------------|
| 現在 | - | 32.91% | 33.98% | 35.14% |
| Phase 1 | 2週間 | 50% | 55% | 52% |
| Phase 2 | 4週間 | 65% | 70% | 67% |
| Phase 3 | 6週間 | 75% | 80% | 77% |

## 継続的改善

### 自動化
- PRごとのカバレッジチェック
- カバレッジ低下の自動検知
- カバレッジレポートの自動生成

### レビュープロセス
- 新機能追加時のテスト必須化
- カバレッジ目標の定期的な見直し
- テストコードのレビュー強化

## リスクと対策

### リスク
1. **LSPサーバー依存**: 外部LSPサーバーの動作に依存するテストの不安定性
2. **実行時間**: テスト増加による CI/CD パイプラインの遅延
3. **メンテナンス負荷**: テストコードのメンテナンスコスト増加

### 対策
1. **モック化の徹底**: 外部依存を最小限に抑える
2. **並列実行**: テストの並列化とキャッシュの活用
3. **テストの品質管理**: DRY原則の適用とヘルパー関数の活用

## 成功指標

- [ ] カバレッジ75%以上の達成
- [ ] CI/CDパイプラインの実行時間を10分以内に維持
- [ ] 新規バグの発見率向上
- [ ] リグレッションの早期発見
- [ ] コードレビューの効率化