use lsif_indexer::cli::fuzzy_search::{fuzzy_search_paths, fuzzy_search_strings};

fn main() {
    // 例1: 汎用的な文字列検索
    println!("=== 文字列の曖昧検索 ===");
    let commands = vec![
        "definition",
        "references",
        "workspace-symbols",
        "call-hierarchy",
        "type-definition",
    ];

    let query = "def";
    let results = fuzzy_search_strings(query, &commands);

    println!("検索クエリ: '{}'", query);
    for result in results.iter().take(3) {
        println!("  {} (スコア: {:.2})", result.text, result.score);
    }

    // 例2: ファイルパスの曖昧検索
    println!("\n=== ファイルパスの曖昧検索 ===");
    let files = vec![
        "src/core/graph.rs",
        "src/core/graph_query.rs",
        "src/cli/fuzzy_search.rs",
        "tests/graph_test.rs",
        "benches/graph_benchmark.rs",
    ];

    let query = "graph";
    let results = fuzzy_search_paths(query, &files);

    println!("検索クエリ: '{}'", query);
    for result in results {
        println!("  {} (スコア: {:.2})", result.text, result.score);
    }

    // 例3: 略語検索
    println!("\n=== 略語検索 ===");
    let types = vec![
        "RelationshipPattern",
        "QueryEngine",
        "TypeRelations",
        "GraphQuery",
    ];

    let query = "QE";
    let results = fuzzy_search_strings(query, &types);

    println!("検索クエリ: '{}'", query);
    for result in results {
        println!("  {} (スコア: {:.2})", result.text, result.score);
    }
}
