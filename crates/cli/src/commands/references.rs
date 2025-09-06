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
        // シンボルの参照を検索
        let references = graph.find_references(&symbol.id);
        
        match references {
            Ok(refs) if !refs.is_empty() => {
                if format == OutputFormat::Human {
                    println!("Found {} references for '{}':", refs.len(), symbol.name);
                    for reference in &refs {
                        // 1ベースの行番号で表示
                        println!("  📍 {}:{}:{}", 
                            reference.file_path, 
                            reference.range.start.line + 1, 
                            reference.range.start.character + 1
                        );
                    }
                } else {
                    let formatter = OutputFormatter::new(format);
                    for reference in refs {
                        println!("{}", formatter.format_symbol(&reference, None));
                    }
                }
            }
            Ok(_) => {
                if format == OutputFormat::Human {
                    print_warning(&format!("No references found for '{}'", symbol.name));
                }
            }
            Err(e) => {
                if format == OutputFormat::Human {
                    print_error(&format!("Error finding references: {}", e));
                }
            }
        }
    } else if format == OutputFormat::Human {
        print_error("No symbol found at this location");
    }

    Ok(())
}
