/// LSP機能の使用例
fn main() {
    println!("LSP機能の使用例:");
    println!();

    // ホバー情報の取得
    println!("1. ホバー情報の取得:");
    println!(
        "   cargo run --bin lsif-indexer -- lsp hover --file src/lib.rs --line 10 --column 15"
    );
    println!();

    // コード補完
    println!("2. コード補完:");
    println!(
        "   cargo run --bin lsif-indexer -- lsp complete --file src/main.rs --line 20 --column 10"
    );
    println!();

    // 実装の検索
    println!("3. 実装の検索:");
    println!("   cargo run --bin lsif-indexer -- lsp implementations --file src/lib.rs --line 50 --column 5");
    println!();

    // 型定義の検索
    println!("4. 型定義の検索:");
    println!("   cargo run --bin lsif-indexer -- lsp type-definition --file src/main.rs --line 30 --column 20");
    println!();

    // シンボルのリネーム
    println!("5. シンボルのリネーム:");
    println!("   cargo run --bin lsif-indexer -- lsp rename --file src/lib.rs --line 15 --column 5 --new-name NewSymbolName");
    println!();

    // 診断情報の取得
    println!("6. 診断情報の取得:");
    println!("   cargo run --bin lsif-indexer -- lsp diagnostics --file src/main.rs");
    println!();

    // LSP統合でのインデックス作成
    println!("7. LSP統合でのプロジェクトインデックス作成:");
    println!(
        "   cargo run --bin lsif-indexer -- lsp index-with-lsp --project . --output lsp_index.db"
    );
}

/// サンプル構造体（LSP機能のテスト用）
pub struct ExampleStruct {
    pub field1: String,
    pub field2: i32,
}

impl Default for ExampleStruct {
    fn default() -> Self {
        Self::new()
    }
}

impl ExampleStruct {
    /// 新しいインスタンスを作成
    pub fn new() -> Self {
        Self {
            field1: String::new(),
            field2: 0,
        }
    }

    /// フィールドを更新
    pub fn update(&mut self, value: i32) {
        self.field2 = value;
    }
}

/// トレイトの例
pub trait ExampleTrait {
    fn do_something(&self);
}

impl ExampleTrait for ExampleStruct {
    fn do_something(&self) {
        println!("Field2 value: {}", self.field2);
    }
}
