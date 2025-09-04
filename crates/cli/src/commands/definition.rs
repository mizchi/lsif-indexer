use super::utils::*;
use crate::output_format::{OutputFormat, OutputFormatter};
use anyhow::Result;

pub fn handle_definition(
    db_path: &str,
    location: &str,
    _show_all: bool,
    format: OutputFormat,
) -> Result<()> {
    let (file, line, column) = parse_location(location)?;

    let formatter = OutputFormatter::new(format);

    if format == OutputFormat::Human {
        print_info(
            &format!("Finding definition at {}:{}:{}", file, line, column),
            "🔍",
        );
    }

    let graph = load_graph(db_path)?;

    if let Some(symbol) = find_symbol_at_location(&graph, &file, line, column) {
        let output = formatter.format_symbol(symbol, None);
        println!("{}", output);
    } else if format == OutputFormat::Human {
        print_error("No definition found at this location");
    }

    Ok(())
}
