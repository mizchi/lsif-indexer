use anyhow::Result;
use super::utils::*;

pub fn handle_definition(db_path: &str, location: &str, _show_all: bool) -> Result<()> {
    let (file, line, column) = parse_location(location)?;
    
    print_info(&format!("Finding definition at {}:{}:{}", file, line, column), "🔍");
    
    let graph = load_graph(db_path)?;
    
    if let Some(symbol) = find_symbol_at_location(&graph, &file, line, column) {
        print_symbol_info(symbol, "📍");
    } else {
        print_error("No definition found at this location");
    }
    
    Ok(())
}