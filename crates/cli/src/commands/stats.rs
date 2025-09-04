use super::utils::*;
use anyhow::Result;
use std::collections::HashMap;

pub fn handle_stats(db_path: &str, _detailed: bool, by_file: bool, by_type: bool) -> Result<()> {
    print_info("Project statistics:", "📊");

    let graph = load_graph(db_path)?;
    let total_symbols = graph.get_all_symbols().count();

    println!("  Total symbols: {}", total_symbols);

    if by_type {
        display_stats_by_type(&graph);
    }

    if by_file {
        display_stats_by_file(&graph);
    }

    Ok(())
}

fn display_stats_by_type(graph: &lsif_core::CodeGraph) {
    let mut by_kind: HashMap<String, usize> = HashMap::new();

    for symbol in graph.get_all_symbols() {
        *by_kind.entry(format!("{:?}", symbol.kind)).or_default() += 1;
    }

    println!("\n📈 By type:");
    let mut sorted: Vec<_> = by_kind.into_iter().collect();
    sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));

    for (kind, count) in sorted.iter().take(10) {
        let emoji = get_kind_emoji(kind);
        println!("  {} {}: {}", emoji, kind, count);
    }
}

fn display_stats_by_file(graph: &lsif_core::CodeGraph) {
    let mut by_file: HashMap<String, usize> = HashMap::new();

    for symbol in graph.get_all_symbols() {
        *by_file.entry(symbol.file_path.clone()).or_default() += 1;
    }

    println!("\n📁 Top files by symbol count:");
    let mut sorted: Vec<_> = by_file.into_iter().collect();
    sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));

    for (file, count) in sorted.iter().take(10) {
        println!("  {} - {} symbols", file, count);
    }
}

fn get_kind_emoji(kind: &str) -> &'static str {
    match kind {
        "Function" | "Method" => "🔧",
        "Class" => "📦",
        "Variable" | "Field" => "📝",
        "Interface" => "🔌",
        "Enum" => "📋",
        _ => "❓",
    }
}
