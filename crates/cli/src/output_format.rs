use anyhow::Result;
use lsif_core::Symbol;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported output formats for editor integration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    /// Human-readable format with emojis (default)
    Human,
    /// Vim quickfix format: file:line:col: text
    Quickfix,
    /// LSP Location format (JSON)
    Lsp,
    /// Grep-like format: file:line:col:text
    Grep,
    /// JSON format for machine processing
    Json,
    /// Tab-separated values
    Tsv,
    /// Null-separated for xargs
    Null,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "human" | "default" => Ok(Self::Human),
            "quickfix" | "qf" | "vim" => Ok(Self::Quickfix),
            "lsp" => Ok(Self::Lsp),
            "grep" => Ok(Self::Grep),
            "json" => Ok(Self::Json),
            "tsv" | "tab" => Ok(Self::Tsv),
            "null" | "0" => Ok(Self::Null),
            _ => anyhow::bail!(
                "Unknown format: {}. Valid formats: human, quickfix, lsp, grep, json, tsv, null",
                s
            ),
        }
    }
}

/// Formatter for converting symbols to various output formats
pub struct OutputFormatter {
    format: OutputFormat,
}

impl OutputFormatter {
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Format a single symbol
    pub fn format_symbol(&self, symbol: &Symbol, context: Option<&str>) -> String {
        match self.format {
            OutputFormat::Human => self.format_human(symbol, context),
            OutputFormat::Quickfix => self.format_quickfix(symbol, context),
            OutputFormat::Lsp => self.format_lsp(symbol),
            OutputFormat::Grep => self.format_grep(symbol, context),
            OutputFormat::Json => self.format_json(symbol),
            OutputFormat::Tsv => self.format_tsv(symbol),
            OutputFormat::Null => self.format_null(symbol),
        }
    }

    /// Format multiple symbols
    pub fn format_symbols(&self, symbols: &[Symbol], context: Option<&str>) -> String {
        match self.format {
            OutputFormat::Json => {
                // For JSON, wrap in array
                let items: Vec<_> = symbols.iter().map(|s| self.to_json_object(s)).collect();
                serde_json::to_string_pretty(&items).unwrap_or_default()
            }
            OutputFormat::Lsp => {
                // For LSP, return array of Locations
                let locations: Vec<_> = symbols.iter().map(|s| self.to_lsp_location(s)).collect();
                serde_json::to_string_pretty(&locations).unwrap_or_default()
            }
            _ => {
                // For other formats, join with newline (or null for Null format)
                let separator = if self.format == OutputFormat::Null {
                    "\0"
                } else {
                    "\n"
                };
                symbols
                    .iter()
                    .map(|s| self.format_symbol(s, context))
                    .collect::<Vec<_>>()
                    .join(separator)
            }
        }
    }

    // Format implementations

    fn format_human(&self, symbol: &Symbol, context: Option<&str>) -> String {
        let emoji = match symbol.kind {
            lsif_core::SymbolKind::Function => "🔧",
            lsif_core::SymbolKind::Class | lsif_core::SymbolKind::Struct => "📦",
            lsif_core::SymbolKind::Interface | lsif_core::SymbolKind::Trait => "📋",
            lsif_core::SymbolKind::Variable | lsif_core::SymbolKind::Field => "📌",
            lsif_core::SymbolKind::Constant => "🔒",
            lsif_core::SymbolKind::Module | lsif_core::SymbolKind::Namespace => "📁",
            _ => "📍",
        };

        let location = format!(
            "{}:{}:{}",
            symbol.file_path, symbol.range.start.line, symbol.range.start.character
        );

        if let Some(ctx) = context {
            format!("{} {} at {} - {}", emoji, symbol.name, location, ctx)
        } else {
            format!("{} {} at {}", emoji, symbol.name, location)
        }
    }

    fn format_quickfix(&self, symbol: &Symbol, context: Option<&str>) -> String {
        // Vim quickfix format: filename:line:col: text
        let text = context.unwrap_or(&symbol.name);
        format!(
            "{}:{}:{}: {}",
            symbol.file_path, symbol.range.start.line, symbol.range.start.character, text
        )
    }

    fn format_grep(&self, symbol: &Symbol, context: Option<&str>) -> String {
        // Grep format: filename:line:col:text (no space after colon)
        let text = context.unwrap_or(&symbol.name);
        format!(
            "{}:{}:{}:{}",
            symbol.file_path, symbol.range.start.line, symbol.range.start.character, text
        )
    }

    fn format_lsp(&self, symbol: &Symbol) -> String {
        let location = self.to_lsp_location(symbol);
        serde_json::to_string(&location).unwrap_or_default()
    }

    fn format_json(&self, symbol: &Symbol) -> String {
        let obj = self.to_json_object(symbol);
        serde_json::to_string(&obj).unwrap_or_default()
    }

    fn format_tsv(&self, symbol: &Symbol) -> String {
        // Tab-separated: file\tline\tcol\tname\tkind
        format!(
            "{}\t{}\t{}\t{}\t{:?}",
            symbol.file_path,
            symbol.range.start.line,
            symbol.range.start.character,
            symbol.name,
            symbol.kind
        )
    }

    fn format_null(&self, symbol: &Symbol) -> String {
        // Null-separated paths for xargs -0
        symbol.file_path.clone()
    }

    // Helper methods

    fn to_lsp_location(&self, symbol: &Symbol) -> LspLocation {
        LspLocation {
            uri: format!("file://{}", symbol.file_path),
            range: LspRange {
                start: LspPosition {
                    line: symbol.range.start.line,
                    character: symbol.range.start.character,
                },
                end: LspPosition {
                    line: symbol.range.end.line,
                    character: symbol.range.end.character,
                },
            },
        }
    }

    fn to_json_object(&self, symbol: &Symbol) -> HashMap<String, serde_json::Value> {
        let mut obj = HashMap::new();
        obj.insert("name".to_string(), serde_json::json!(symbol.name));
        obj.insert("file".to_string(), serde_json::json!(symbol.file_path));
        obj.insert(
            "line".to_string(),
            serde_json::json!(symbol.range.start.line),
        );
        obj.insert(
            "column".to_string(),
            serde_json::json!(symbol.range.start.character),
        );
        obj.insert(
            "kind".to_string(),
            serde_json::json!(format!("{:?}", symbol.kind)),
        );

        if let Some(detail) = &symbol.detail {
            obj.insert("type".to_string(), serde_json::json!(detail));
        }

        if let Some(doc) = &symbol.documentation {
            obj.insert("doc".to_string(), serde_json::json!(doc));
        }

        obj
    }
}

// LSP-compatible structures

#[derive(Serialize, Deserialize)]
struct LspLocation {
    uri: String,
    range: LspRange,
}

#[derive(Serialize, Deserialize)]
struct LspRange {
    start: LspPosition,
    end: LspPosition,
}

#[derive(Serialize, Deserialize)]
struct LspPosition {
    line: u32,
    character: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_parsing() {
        assert_eq!(
            OutputFormat::from_str("quickfix").unwrap(),
            OutputFormat::Quickfix
        );
        assert_eq!(
            OutputFormat::from_str("QF").unwrap(),
            OutputFormat::Quickfix
        );
        assert_eq!(
            OutputFormat::from_str("vim").unwrap(),
            OutputFormat::Quickfix
        );
        assert_eq!(OutputFormat::from_str("json").unwrap(), OutputFormat::Json);
        assert!(OutputFormat::from_str("invalid").is_err());
    }

    #[test]
    fn test_quickfix_format() {
        let symbol = Symbol {
            id: "test".to_string(),
            name: "test_function".to_string(),
            kind: lsif_core::SymbolKind::Function,
            file_path: "src/main.rs".to_string(),
            range: lsif_core::Range {
                start: lsif_core::Position {
                    line: 10,
                    character: 5,
                },
                end: lsif_core::Position {
                    line: 10,
                    character: 18,
                },
            },
            documentation: None,
            detail: None,
        };

        let formatter = OutputFormatter::new(OutputFormat::Quickfix);
        let output = formatter.format_symbol(&symbol, Some("function definition"));
        assert_eq!(output, "src/main.rs:10:5: function definition");
    }
}
