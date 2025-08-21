# 言語サポート比較表

## 現在サポートされている言語

| 言語 | LSPサーバー | ファイル拡張子 | 定義パターン | 特殊な考慮事項 |
|------|------------|--------------|------------|--------------|
| Rust | rust-analyzer | .rs | fn, struct, enum, trait, impl, const, let | マクロ展開、ライフタイム、所有権 |
| TypeScript/JavaScript | @typescript/native-preview | .ts, .tsx, .js, .jsx | function, class, interface, const, let, var, type, enum | export文、import文、JSX構文 |

## 今後追加予定の言語

| 言語 | 推奨LSPサーバー | ファイル拡張子 | 定義パターン | 実装難易度 | 優先度 |
|------|---------------|--------------|------------|-----------|--------|
| Python | pylsp | .py, .pyw | def, class, import, from | 中 | 高 |
| Go | gopls | .go | func, type, interface, const, var | 低 | 高 |
| Java | jdtls | .java | class, interface, enum, @interface | 高 | 中 |
| C/C++ | clangd | .c, .cpp, .h, .hpp | 関数、struct, class, typedef | 高 | 中 |
| C# | OmniSharp | .cs | class, interface, struct, enum, delegate | 中 | 低 |
| Ruby | solargraph | .rb | def, class, module | 中 | 低 |
| PHP | intelephense | .php | function, class, interface, trait | 中 | 低 |

## 言語固有の実装詳細

### Rust
```rust
// 特徴的な定義パターン
fn function_name() {}           // 関数
struct StructName {}            // 構造体
enum EnumName {}                // 列挙型
trait TraitName {}              // トレイト
impl TraitName for Type {}      // トレイト実装
const CONSTANT: Type = value;   // 定数
let variable = value;           // 変数
```

**課題:**
- マクロによるコード生成
- ジェネリクスとライフタイム
- モジュールシステム（mod）

### TypeScript/JavaScript
```typescript
// 特徴的な定義パターン
function functionName() {}      // 関数
class ClassName {}              // クラス
interface InterfaceName {}      // インターフェース
const constantName = value;     // 定数
let variableName = value;       // 変数
type TypeAlias = Type;          // 型エイリアス
enum EnumName {}                // 列挙型

// エクスポート
export function exportedFunc() {}
export class ExportedClass {}
export interface ExportedInterface {}
```

**課題:**
- 動的型付け（JavaScript）
- JSX構文
- モジュールシステム（CommonJS vs ES Modules）
- デコレーター

### Python（計画中）
```python
# 特徴的な定義パターン
def function_name():            # 関数
async def async_function():     # 非同期関数
class ClassName:                # クラス
CONSTANT = value               # 定数（慣習）
variable = value               # 変数

# インポート
import module
from module import name
```

**課題:**
- インデントベースの構文
- 動的型付け
- デコレーター
- メタクラス

### Go（計画中）
```go
// 特徴的な定義パターン
func functionName() {}          // 関数
func (r Receiver) Method() {}   // メソッド
type TypeName struct {}         // 構造体
type InterfaceName interface {} // インターフェース
const ConstantName = value      // 定数
var variableName = value        // 変数
```

**課題:**
- インターフェースの暗黙的実装
- 大文字・小文字によるエクスポート制御
- ゴルーチンとチャネル

## パフォーマンス比較

| 言語 | LSP起動時間 | インデックス速度 | メモリ使用量 | 備考 |
|------|------------|----------------|------------|------|
| Rust | 中 (1-2秒) | 高速 | 中 | rust-analyzerは高機能だが起動が遅め |
| TypeScript | 速 (<1秒) | 高速 | 低 | native-previewは軽量で高速 |
| Python | 速 (<1秒) | 中速 | 低 | 動的型付けのため解析に限界あり |
| Go | 速 (<1秒) | 高速 | 低 | シンプルな言語仕様で高速 |
| Java | 遅 (3-5秒) | 中速 | 高 | JVMの起動オーバーヘッド |

## LSPサーバーの機能比較

| 機能 | rust-analyzer | tsserver | pylsp | gopls |
|------|--------------|----------|--------|--------|
| 定義ジャンプ | ✅ | ✅ | ✅ | ✅ |
| 参照検索 | ✅ | ✅ | ✅ | ✅ |
| シンボル一覧 | ✅ | ✅ | ✅ | ✅ |
| 型情報 | ✅ | ✅ | ⚠️ | ✅ |
| リネーム | ✅ | ✅ | ✅ | ✅ |
| コード補完 | ✅ | ✅ | ✅ | ✅ |
| フォーマット | ✅ | ✅ | ✅ | ✅ |

凡例: ✅ 完全サポート、⚠️ 部分的サポート、❌ 未サポート

## 実装の優先順位決定基準

1. **ユーザー需要**: GitHubのスター数、言語ランキング
2. **LSPの成熟度**: 機能の完全性、安定性
3. **実装の容易さ**: 言語の複雑さ、特殊な構文
4. **エコシステム**: パッケージマネージャー、ビルドツール
5. **パフォーマンス**: インデックス速度、メモリ効率

## まとめ

各言語には固有の特徴と課題がありますが、LSPを通じた抽象化により、統一的なインターフェースで扱うことができます。新しい言語を追加する際は、その言語の特性を理解し、適切なパターンマッチングと設定を行うことが重要です。