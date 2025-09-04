use core::{CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind};
use std::time::Instant;

fn main() {
    println!("=== LSIF Indexer Core - 使い勝手検証 ===\n");

    // 1. グラフの作成
    let start = Instant::now();
    let mut graph = CodeGraph::new();
    println!("✓ グラフ作成: {:?}", start.elapsed());

    // 2. シンボルの追加
    let start = Instant::now();
    let symbols = vec![
        Symbol {
            id: "main".to_string(),
            name: "main".to_string(),
            kind: SymbolKind::Function,
            file_path: "src/main.rs".to_string(),
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 1,
                },
            },
            documentation: Some("メイン関数".to_string()),
            detail: None,
        },
        Symbol {
            id: "Calculator".to_string(),
            name: "Calculator".to_string(),
            kind: SymbolKind::Struct,
            file_path: "src/lib.rs".to_string(),
            range: Range {
                start: Position {
                    line: 10,
                    character: 0,
                },
                end: Position {
                    line: 15,
                    character: 1,
                },
            },
            documentation: Some("計算機構造体".to_string()),
            detail: Some("pub struct Calculator".to_string()),
        },
        Symbol {
            id: "add".to_string(),
            name: "add".to_string(),
            kind: SymbolKind::Method,
            file_path: "src/lib.rs".to_string(),
            range: Range {
                start: Position {
                    line: 20,
                    character: 4,
                },
                end: Position {
                    line: 22,
                    character: 5,
                },
            },
            documentation: Some("加算メソッド".to_string()),
            detail: Some("pub fn add(&self, a: i32, b: i32) -> i32".to_string()),
        },
    ];

    for symbol in &symbols {
        graph.add_symbol(symbol.clone());
    }
    println!(
        "✓ シンボル追加 ({}個): {:?}",
        symbols.len(),
        start.elapsed()
    );

    // 3. 関係の追加
    let start = Instant::now();
    if let (Some(&main_idx), Some(&calc_idx)) = (
        graph.symbol_index.get("main"),
        graph.symbol_index.get("Calculator"),
    ) {
        graph
            .graph
            .add_edge(main_idx, calc_idx, EdgeKind::Reference);
    }
    if let (Some(&main_idx), Some(&add_idx)) = (
        graph.symbol_index.get("main"),
        graph.symbol_index.get("add"),
    ) {
        graph.graph.add_edge(main_idx, add_idx, EdgeKind::Reference);
    }
    println!("✓ 関係追加: {:?}", start.elapsed());

    // 4. 定義検索
    let start = Instant::now();
    let position = Position {
        line: 20,
        character: 10,
    };
    let definition = graph.find_definition_at("src/lib.rs", position);
    println!(
        "✓ 定義検索: {:?} ({:?})",
        definition.map(|s| &s.name),
        start.elapsed()
    );

    // 5. 参照検索
    let start = Instant::now();
    let refs = graph
        .find_references("Calculator")
        .unwrap_or_else(|_| vec![]);
    println!("✓ 参照検索: {}件 ({:?})", refs.len(), start.elapsed());

    // 6. 全シンボル取得
    let start = Instant::now();
    let all_symbols: Vec<_> = graph.get_all_symbols().collect();
    println!(
        "✓ 全シンボル取得: {}件 ({:?})",
        all_symbols.len(),
        start.elapsed()
    );

    // 7. グラフのサイズ
    println!("\n=== 統計情報 ===");
    println!("シンボル数: {}", graph.symbol_count());
    println!("エッジ数: {}", graph.graph.edge_count());

    // 8. パフォーマンス結果
    println!("\n=== パフォーマンス ===");
    let start = Instant::now();
    for _ in 0..1000 {
        graph.find_symbol("Calculator");
    }
    let elapsed = start.elapsed();
    println!(
        "シンボル検索 (1000回): {:?} (平均: {:?}/回)",
        elapsed,
        elapsed / 1000
    );

    // 9. 使い勝手の評価
    println!("\n=== 使い勝手の評価 ===");
    println!("良い点:");
    println!("  ✓ APIがシンプルで直感的");
    println!("  ✓ 高速な検索性能");
    println!("  ✓ グラフベースの柔軟な構造");

    println!("\n改善が必要な点:");
    println!("  - CLIツールのビルドエラー");
    println!("  - LSP統合の安定性");
    println!("  - ドキュメントの充実");

    println!("\n推奨事項:");
    println!("  1. lspクレートのエラー修正を優先");
    println!("  2. CLIインターフェースの簡略化");
    println!("  3. エラーハンドリングの改善");
}
