use anyhow::Result;
use lsif_core::SymbolKind;
use super::utils::*;

pub fn handle_search(
    db_path: &str,
    query: &str,
    fuzzy: bool,
    symbol_type: Option<String>,
    path_pattern: Option<String>,
    max_results: usize,
) -> Result<()> {
    let mode = if fuzzy { "fuzzy" } else { "exact" };
    print_info(&format!("Searching for '{}' ({})", query, mode), "🔍");
    
    let graph = load_graph(db_path)?;
    let mut results = Vec::new();
    
    for symbol in graph.get_all_symbols() {
        if should_include_symbol(symbol, &symbol_type, &path_pattern, query, fuzzy) {
            results.push(symbol.clone());
            if results.len() >= max_results {
                break;
            }
        }
    }
    
    display_search_results(&results, max_results);
    Ok(())
}

fn should_include_symbol(
    symbol: &lsif_core::Symbol,
    symbol_type: &Option<String>,
    path_pattern: &Option<String>,
    query: &str,
    fuzzy: bool,
) -> bool {
    // Type filter
    if let Some(ref st) = symbol_type {
        if !matches_symbol_type(&symbol.kind, st) {
            return false;
        }
    }
    
    // Path filter
    if let Some(ref pattern) = path_pattern {
        if !symbol.file_path.contains(pattern) {
            return false;
        }
    }
    
    // Name matching
    if fuzzy {
        symbol.name.to_lowercase().contains(&query.to_lowercase())
    } else {
        symbol.name == query
    }
}

fn matches_symbol_type(kind: &SymbolKind, type_str: &str) -> bool {
    match type_str {
        "function" => matches!(kind, SymbolKind::Function | SymbolKind::Method),
        "class" => matches!(kind, SymbolKind::Class),
        "variable" => matches!(kind, SymbolKind::Variable | SymbolKind::Field),
        "interface" => matches!(kind, SymbolKind::Interface),
        "enum" => matches!(kind, SymbolKind::Enum),
        _ => false,
    }
}

fn display_search_results(results: &[lsif_core::Symbol], max_results: usize) {
    if results.is_empty() {
        print_error("No symbols found");
    } else {
        println!("Found {} symbols (max: {})", results.len(), max_results);
        for symbol in results {
            let kind = format!("{:?}", symbol.kind).to_lowercase();
            println!(
                "  🔹 {} ({}) - {}",
                symbol.name,
                kind,
                format_symbol_location(symbol)
            );
        }
    }
}