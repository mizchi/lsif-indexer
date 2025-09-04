//! Python language adapter

use super::{LanguageAdapter, ParsedQuery};
use anyhow::Result;
use lsif_core::{Symbol, SymbolKind};

pub struct PythonAdapter;

impl Default for PythonAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageAdapter for PythonAdapter {
    fn language(&self) -> &str {
        "python"
    }

    fn is_public(&self, symbol: &Symbol) -> bool {
        // Python convention: _ prefix means private
        !symbol.name.starts_with('_') || symbol.name.starts_with("__init__")
    }

    fn get_import_statement(&self, symbol: &Symbol, _from_file: &str) -> Option<String> {
        let module = self.get_module_from_path(&symbol.file_path)?;

        match symbol.kind {
            SymbolKind::Function | SymbolKind::Class | SymbolKind::Variable => {
                Some(format!("from {} import {}", module, symbol.name))
            }
            _ => None,
        }
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
                "def" | "function" => {
                    parsed.symbol_kind = Some(SymbolKind::Function);
                }
                "class" => {
                    parsed.symbol_kind = Some(SymbolKind::Class);
                }
                "async" | "await" | "global" | "nonlocal" => {
                    parsed.modifiers.push(part.to_string());
                }
                _ => {
                    if part.contains('.') {
                        let dot_parts: Vec<&str> = part.split('.').collect();
                        if dot_parts.len() >= 2 {
                            parsed.scope = Some(dot_parts[..dot_parts.len() - 1].join("."));
                            parsed.symbol_name = Some(dot_parts.last().unwrap().to_string());
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
        // Generate Python docs URL for standard library
        if symbol.file_path.contains("site-packages") {
            let package = self.extract_package_name(&symbol.file_path)?;
            Some(format!("https://pypi.org/project/{}/", package))
        } else {
            None
        }
    }

    fn is_test(&self, symbol: &Symbol) -> bool {
        symbol.name.starts_with("test_")
            || symbol.file_path.contains("test_")
            || symbol.file_path.contains("tests/")
            || symbol
                .detail
                .as_ref()
                .is_some_and(|d| d.contains("unittest") || d.contains("pytest"))
    }

    fn get_parent_scope(&self, symbol: &Symbol) -> Option<String> {
        if let Some(detail) = &symbol.detail {
            if detail.contains("class ") {
                let class_part = detail.split("class ").nth(1)?;
                let class_name = class_part.split(':').next()?.trim();
                return Some(class_name.to_string());
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

        if symbol.name.starts_with("__") && symbol.name.ends_with("__") {
            // Dunder methods
            score -= 0.3;
        }

        score
    }
}

impl PythonAdapter {
    fn get_module_from_path(&self, file_path: &str) -> Option<String> {
        let path = std::path::Path::new(file_path);
        let mut components = Vec::new();

        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_str()?;
                if let Some(module_name) = name_str.strip_suffix(".py") {
                    if module_name != "__init__" {
                        components.push(module_name.to_string());
                    }
                } else {
                    components.push(name_str.to_string());
                }
            }
        }

        Some(components.join("."))
    }

    fn extract_package_name(&self, file_path: &str) -> Option<String> {
        let parts: Vec<&str> = file_path.split("site-packages/").collect();
        if parts.len() < 2 {
            return None;
        }

        let package_part = parts[1];
        package_part.split('/').next().map(|s| s.to_string())
    }
}
