use anyhow::Result;
use crate::core::{CodeGraph, CallHierarchyAnalyzer, format_hierarchy};
use super::storage::IndexStorage;

pub fn show_call_hierarchy(
    index_path: &str,
    symbol_id: &str,
    direction: &str,
    max_depth: usize,
) -> Result<()> {
    // Load the index
    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage.load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;
    
    let analyzer = CallHierarchyAnalyzer::new(&graph);
    
    match direction {
        "incoming" | "callers" => {
            if let Some(hierarchy) = analyzer.get_incoming_calls(symbol_id, max_depth) {
                println!("=== Incoming calls to {} ===", symbol_id);
                println!("{}", format_hierarchy(&hierarchy, "", true));
            } else {
                println!("Symbol not found: {}", symbol_id);
            }
        }
        "outgoing" | "callees" => {
            if let Some(hierarchy) = analyzer.get_outgoing_calls(symbol_id, max_depth) {
                println!("=== Outgoing calls from {} ===", symbol_id);
                println!("{}", format_hierarchy(&hierarchy, "", true));
            } else {
                println!("Symbol not found: {}", symbol_id);
            }
        }
        "full" | "both" => {
            if let Some(hierarchy) = analyzer.get_full_hierarchy(symbol_id, max_depth) {
                println!("=== Full call hierarchy for {} ===", symbol_id);
                println!("{}", format_hierarchy(&hierarchy, "", true));
            } else {
                println!("Symbol not found: {}", symbol_id);
            }
        }
        _ => {
            anyhow::bail!("Invalid direction: {}. Use 'incoming', 'outgoing', or 'full'", direction);
        }
    }
    
    Ok(())
}

pub fn find_paths(
    index_path: &str,
    from_symbol: &str,
    to_symbol: &str,
    max_depth: usize,
) -> Result<()> {
    // Load the index
    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage.load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;
    
    let analyzer = CallHierarchyAnalyzer::new(&graph);
    let paths = analyzer.find_call_paths(from_symbol, to_symbol, max_depth);
    
    if paths.is_empty() {
        println!("No paths found from {} to {}", from_symbol, to_symbol);
    } else {
        println!("=== Call paths from {} to {} ===", from_symbol, to_symbol);
        for (i, path) in paths.iter().enumerate() {
            println!("Path {}: {}", i + 1, path.join(" → "));
        }
        println!("\nTotal paths found: {}", paths.len());
    }
    
    Ok(())
}