use super::utils::*;
use crate::output_format::{OutputFormat, OutputFormatter};
use anyhow::Result;

pub fn handle_references(
    db_path: &str,
    location: &str,
    _include_defs: bool,
    _group: bool,
    format: OutputFormat,
) -> Result<()> {
    let (file, line, column) = parse_location(location)?;

    let _formatter = OutputFormatter::new(format);

    if format == OutputFormat::Human {
        print_info(
            &format!("Finding references for {}:{}:{}", file, line, column),
            "🔗",
        );
    }

    let graph = load_graph(db_path)?;

    if let Some(symbol) = find_symbol_at_location(&graph, &file, line, column) {
        if format == OutputFormat::Human {
            println!("Found symbol: {}", symbol.name);
            // TODO: Implement actual reference finding
            println!("Reference finding not yet implemented in simplified version");
        }
    } else if format == OutputFormat::Human {
        print_error("No symbol found at this location");
    }

    Ok(())
}
