//! Go language adapter

use super::{LanguageAdapter, ParsedQuery};
use anyhow::Result;
use lsif_core::{Symbol, SymbolKind};

pub struct GoAdapter;

impl Default for GoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GoAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageAdapter for GoAdapter {
    fn language(&self) -> &str {
        "go"
    }

    fn is_public(&self, symbol: &Symbol) -> bool {
        // Go convention: uppercase first letter means public
        symbol.name.chars().next().is_some_and(|c| c.is_uppercase())
    }

    fn get_import_statement(&self, symbol: &Symbol, _from_file: &str) -> Option<String> {
        let package = self.get_package_from_path(&symbol.file_path)?;
        Some(format!("import \"{}\"", package))
    }

    fn parse_query(&self, query: &str) -> Result<ParsedQuery> {
        let mut parsed = ParsedQuery {
            symbol_name: None,
            symbol_kind: None,
            modifiers: Vec::new(),
            scope: None,
        };

        let parts: Vec<&str> = query.split_whitespace().collect();

        for part in parts {
            match part {
                "func" | "function" => {
                    parsed.symbol_kind = Some(SymbolKind::Function);
                }
                "struct" => {
                    parsed.symbol_kind = Some(SymbolKind::Struct);
                }
                "interface" => {
                    parsed.symbol_kind = Some(SymbolKind::Interface);
                }
                "type" => {
                    parsed.symbol_kind = Some(SymbolKind::TypeAlias);
                }
                "const" | "var" => {
                    parsed.modifiers.push(part.to_string());
                }
                _ => {
                    if part.contains('.') {
                        let dot_parts: Vec<&str> = part.split('.').collect();
                        if dot_parts.len() == 2 {
                            parsed.scope = Some(dot_parts[0].to_string());
                            parsed.symbol_name = Some(dot_parts[1].to_string());
                        }
                    } else {
                        parsed.symbol_name = Some(part.to_string());
                    }
                }
            }
        }

        Ok(parsed)
    }

    fn get_doc_url(&self, symbol: &Symbol) -> Option<String> {
        // Generate pkg.go.dev URL
        let package = self.get_package_from_path(&symbol.file_path)?;

        if package.starts_with("github.com/") || package.starts_with("golang.org/") {
            Some(format!("https://pkg.go.dev/{}", package))
        } else {
            None
        }
    }

    fn is_test(&self, symbol: &Symbol) -> bool {
        symbol.name.starts_with("Test")
            || symbol.name.starts_with("Benchmark")
            || symbol.name.starts_with("Example")
            || symbol.file_path.ends_with("_test.go")
    }

    fn get_parent_scope(&self, symbol: &Symbol) -> Option<String> {
        // Check for receiver in method
        if let Some(detail) = &symbol.detail {
            if detail.contains("func (") {
                let receiver_part = detail.split("func (").nth(1)?;
                let receiver_end = receiver_part.find(')')?;
                let receiver = &receiver_part[..receiver_end];

                // Extract type from receiver
                let parts: Vec<&str> = receiver.split_whitespace().collect();
                if parts.len() >= 2 {
                    let type_name = parts[1].trim_start_matches('*');
                    return Some(type_name.to_string());
                }
            }
        }
        None
    }

    fn score_relevance(&self, symbol: &Symbol, query: &str) -> f32 {
        let mut score = 1.0;

        if symbol.name == query {
            score += 2.0;
        }

        if self.is_public(symbol) {
            score += 0.5;
        }

        if self.is_test(symbol) {
            score -= 0.5;
        }

        // Boost for main function
        if symbol.name == "main" && symbol.kind == SymbolKind::Function {
            score += 0.5;
        }

        // Boost for init function
        if symbol.name == "init" && symbol.kind == SymbolKind::Function {
            score += 0.3;
        }

        score
    }
}

impl GoAdapter {
    fn get_package_from_path(&self, file_path: &str) -> Option<String> {
        // Extract Go package path
        let path = std::path::Path::new(file_path);

        // Look for common Go paths
        if let Some(idx) = file_path.find("/src/") {
            let package_path = &file_path[idx + 5..];
            let package = std::path::Path::new(package_path).parent()?.to_str()?;
            return Some(package.to_string());
        }

        // Try to extract from go.mod structure
        if let Some(idx) = file_path.find("/go/") {
            let package_path = &file_path[idx + 4..];
            let package = std::path::Path::new(package_path).parent()?.to_str()?;
            return Some(package.to_string());
        }

        // Fallback to parent directory name
        path.parent()?.file_name()?.to_str().map(|s| s.to_string())
    }
}
