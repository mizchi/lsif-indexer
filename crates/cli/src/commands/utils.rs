use crate::storage::IndexStorage;
use anyhow::Result;
use lsif_core::{CodeGraph, Symbol};

/// Parse location format: file.rs:10:5 or file.rs
pub fn parse_location(location: &str) -> Result<(String, u32, u32)> {
    let parts: Vec<&str> = location.split(':').collect();
    let file = parts[0].to_string();
    let line = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let column = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    Ok((file, line, column))
}

/// Load graph from database
pub fn load_graph(db_path: &str) -> Result<CodeGraph> {
    let storage = IndexStorage::open(db_path)?;
    Ok(storage.load_data::<CodeGraph>("graph")?.unwrap_or_default())
}

/// Find symbol at location with fuzzy column matching
pub fn find_symbol_at_location<'a>(
    graph: &'a CodeGraph,
    file: &str,
    line: u32,
    column: u32,
) -> Option<&'a Symbol> {
    // ユーザー入力は1ベース、内部表現は0ベース
    let zero_based_line = line.saturating_sub(1);
    let zero_based_column = column.saturating_sub(1);
    
    // まず正確な位置でシンボルを検索
    let symbol = graph.get_all_symbols().find(|s| {
        // ファイルパスの正規化（相対パスと絶対パスの両方に対応）
        let matches_file = s.file_path == file 
            || s.file_path.ends_with(file) 
            || file.ends_with(&s.file_path);
            
        matches_file
            && s.range.start.line <= zero_based_line
            && s.range.end.line >= zero_based_line
            && (zero_based_line != s.range.start.line || s.range.start.character <= zero_based_column)
            && (zero_based_line != s.range.end.line || s.range.end.character >= zero_based_column)
    });
    
    // 見つからない場合は、より緩い条件で検索
    symbol.or_else(|| {
        graph.get_all_symbols().find(|s| {
            let matches_file = s.file_path == file 
                || s.file_path.ends_with(file) 
                || file.ends_with(&s.file_path);
                
            matches_file
                && s.range.start.line == zero_based_line
                && s.range.start.character <= zero_based_column + 10
        })
    })
}

/// Format symbol location for display
pub fn format_symbol_location(symbol: &Symbol) -> String {
    format!(
        "{}:{}:{}",
        symbol.file_path, symbol.range.start.line, symbol.range.start.character
    )
}

/// Print error message with emoji
pub fn print_error(message: &str) {
    println!("❌ {}", message);
}

/// Print success message with emoji
pub fn print_success(message: &str) {
    println!("✅ {}", message);
}

/// Print info message with emoji
pub fn print_info(message: &str, emoji: &str) {
    println!("{} {}", emoji, message);
}

/// Print warning message with emoji
pub fn print_warning(message: &str) {
    println!("⚠️  {}", message);
}
