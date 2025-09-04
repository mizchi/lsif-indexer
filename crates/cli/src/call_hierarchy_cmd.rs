use crate::storage::IndexStorage;
use anyhow::Result;
use lsif_core::call_hierarchy::{format_hierarchy, CallHierarchyAnalyzer};
use lsif_core::CodeGraph;

pub fn show_call_hierarchy(
    index_path: &str,
    symbol_id: &str,
    direction: &str,
    max_depth: usize,
) -> Result<()> {
    // Load the index
    let storage = IndexStorage::open(index_path)?;
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;

    let analyzer = CallHierarchyAnalyzer::new(&graph);

    match direction {
        "incoming" | "callers" => {
            if let Some(hierarchy) = analyzer.get_incoming_calls(symbol_id, max_depth) {
                println!("=== Incoming calls to {symbol_id} ===");
                println!("{}", format_hierarchy(&hierarchy, "", true));
            } else {
                println!("Symbol not found: {symbol_id}");
            }
        }
        "outgoing" | "callees" => {
            if let Some(hierarchy) = analyzer.get_outgoing_calls(symbol_id, max_depth) {
                println!("=== Outgoing calls from {symbol_id} ===");
                println!("{}", format_hierarchy(&hierarchy, "", true));
            } else {
                println!("Symbol not found: {symbol_id}");
            }
        }
        "full" | "both" => {
            if let Some(hierarchy) = analyzer.get_full_hierarchy(symbol_id, max_depth) {
                println!("=== Full call hierarchy for {symbol_id} ===");
                println!("{}", format_hierarchy(&hierarchy, "", true));
            } else {
                println!("Symbol not found: {symbol_id}");
            }
        }
        _ => {
            anyhow::bail!(
                "Invalid direction: {}. Use 'incoming', 'outgoing', or 'full'",
                direction
            );
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
    let graph: CodeGraph = storage
        .load_data("graph")?
        .ok_or_else(|| anyhow::anyhow!("No graph found in index"))?;

    let analyzer = CallHierarchyAnalyzer::new(&graph);
    let paths = analyzer.find_call_paths(from_symbol, to_symbol, max_depth);

    if paths.is_empty() {
        println!("No paths found from {from_symbol} to {to_symbol}");
    } else {
        println!("=== Call paths from {from_symbol} to {to_symbol} ===");
        for (i, path) in paths.iter().enumerate() {
            println!("Path {}: {}", i + 1, path.join(" → "));
        }
        println!("\nTotal paths found: {}", paths.len());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsif_core::{EdgeKind, Position, Range, Symbol, SymbolKind};
    use tempfile::TempDir;

    fn create_test_graph() -> CodeGraph {
        let mut graph = CodeGraph::new();

        // Create test symbols
        let main_fn = Symbol {
            id: "main".to_string(),
            name: "main".to_string(),
            kind: SymbolKind::Function,
            file_path: "main.rs".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
            documentation: None,
            detail: None,
        };

        let helper_fn = Symbol {
            id: "helper".to_string(),
            name: "helper".to_string(),
            kind: SymbolKind::Function,
            file_path: "main.rs".to_string(),
            range: Range {
                start: Position {
                    line: 5,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 10,
                },
            },
            documentation: None,
            detail: None,
        };

        let util_fn = Symbol {
            id: "util".to_string(),
            name: "util".to_string(),
            kind: SymbolKind::Function,
            file_path: "util.rs".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
            documentation: None,
            detail: None,
        };

        // Add symbols to graph
        let main_idx = graph.add_symbol(main_fn);
        let helper_idx = graph.add_symbol(helper_fn);
        let util_idx = graph.add_symbol(util_fn);

        // Add edges (main calls helper, helper calls util)
        graph.add_edge(main_idx, helper_idx, EdgeKind::Reference);
        graph.add_edge(helper_idx, util_idx, EdgeKind::Reference);

        graph
    }

    #[test]
    #[ignore] // TODO: Fix test - needs proper graph setup
    fn test_show_call_hierarchy_incoming() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test.db");

        // Create test storage with graph
        let storage = IndexStorage::open(&index_path).unwrap();
        let graph = create_test_graph();
        storage.save_data("graph", &graph).unwrap();

        // Test incoming calls
        let result = show_call_hierarchy(index_path.to_str().unwrap(), "helper", "incoming", 3);

        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // TODO: Fix test
    fn test_show_call_hierarchy_outgoing() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test.db");

        // Create test storage with graph
        let storage = IndexStorage::open(&index_path).unwrap();
        let graph = create_test_graph();
        storage.save_data("graph", &graph).unwrap();

        // Test outgoing calls
        let result = show_call_hierarchy(index_path.to_str().unwrap(), "helper", "outgoing", 3);

        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // TODO: Fix test
    fn test_show_call_hierarchy_full() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test.db");

        // Create test storage with graph
        let storage = IndexStorage::open(&index_path).unwrap();
        let graph = create_test_graph();
        storage.save_data("graph", &graph).unwrap();

        // Test full hierarchy
        let result = show_call_hierarchy(index_path.to_str().unwrap(), "helper", "full", 3);

        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // TODO: Fix test
    fn test_show_call_hierarchy_invalid_direction() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test.db");

        // Create test storage with graph
        let storage = IndexStorage::open(&index_path).unwrap();
        let graph = create_test_graph();
        storage.save_data("graph", &graph).unwrap();

        // Test invalid direction
        let result = show_call_hierarchy(index_path.to_str().unwrap(), "helper", "invalid", 3);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid direction"));
    }

    #[test]
    #[ignore] // TODO: Fix test
    fn test_show_call_hierarchy_no_graph() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test.db");

        // Create empty storage
        let _storage = IndexStorage::open(&index_path).unwrap();

        // Test with no graph
        let result = show_call_hierarchy(index_path.to_str().unwrap(), "helper", "incoming", 3);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No graph found"));
    }

    #[test]
    #[ignore] // TODO: Fix test
    fn test_find_paths() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test.db");

        // Create test storage with graph
        let storage = IndexStorage::open(&index_path).unwrap();
        let graph = create_test_graph();
        storage.save_data("graph", &graph).unwrap();

        // Test finding paths
        let result = find_paths(index_path.to_str().unwrap(), "main", "util", 5);

        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // TODO: Fix test
    fn test_find_paths_no_connection() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test.db");

        // Create test storage with graph
        let storage = IndexStorage::open(&index_path).unwrap();
        let graph = create_test_graph();
        storage.save_data("graph", &graph).unwrap();

        // Test finding paths with no connection
        let result = find_paths(index_path.to_str().unwrap(), "util", "main", 5);

        assert!(result.is_ok());
    }
}
