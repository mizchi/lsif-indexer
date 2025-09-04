//! Rust language adapter

use super::{LanguageAdapter, ParsedQuery};
use anyhow::Result;
use lsif_core::{Symbol, SymbolKind};

pub struct RustAdapter;

impl Default for RustAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RustAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageAdapter for RustAdapter {
    fn language(&self) -> &str {
        "rust"
    }

    fn is_public(&self, symbol: &Symbol) -> bool {
        // Check if symbol is public
        !symbol.name.starts_with("_")
            && !symbol
                .detail
                .as_ref()
                .is_some_and(|d| d.contains("pub(crate)") || d.contains("pub(super)"))
    }

    fn get_import_statement(&self, symbol: &Symbol, from_file: &str) -> Option<String> {
        // Generate use statement
        let module_path = self.get_module_path(symbol, from_file)?;

        match symbol.kind {
            SymbolKind::Function | SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait => {
                Some(format!("use {};", module_path))
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

        // Parse Rust-specific patterns
        // Examples: "impl Trait", "pub fn", "mod::", "trait Foo"
        let parts: Vec<&str> = query.split_whitespace().collect();

        for (i, part) in parts.iter().enumerate() {
            match *part {
                "pub" | "pub(crate)" | "pub(super)" => {
                    parsed.modifiers.push(part.to_string());
                }
                "fn" | "function" => {
                    parsed.symbol_kind = Some(SymbolKind::Function);
                }
                "struct" => {
                    parsed.symbol_kind = Some(SymbolKind::Struct);
                }
                "enum" => {
                    parsed.symbol_kind = Some(SymbolKind::Enum);
                }
                "trait" => {
                    parsed.symbol_kind = Some(SymbolKind::Trait);
                }
                "impl" => {
                    parsed.modifiers.push("impl".to_string());
                }
                "mod" | "module" => {
                    parsed.symbol_kind = Some(SymbolKind::Module);
                }
                _ => {
                    if part.contains("::") {
                        // Module path
                        let path_parts: Vec<&str> = part.split("::").collect();
                        if path_parts.len() > 1 {
                            parsed.scope = Some(path_parts[..path_parts.len() - 1].join("::"));
                            parsed.symbol_name = Some(path_parts.last().unwrap().to_string());
                        }
                    } else if i == parts.len() - 1 || !part.starts_with('&') {
                        // Likely the symbol name
                        parsed.symbol_name = Some(part.to_string());
                    }
                }
            }
        }

        Ok(parsed)
    }

    fn get_doc_url(&self, symbol: &Symbol) -> Option<String> {
        // Generate docs.rs URL for public symbols
        if !self.is_public(symbol) {
            return None;
        }

        // Extract crate name from file path
        let crate_name = self.extract_crate_name(&symbol.file_path)?;
        let module_path = self.get_module_path(symbol, "")?;

        Some(format!(
            "https://docs.rs/{}/latest/{}/",
            crate_name,
            module_path.replace("::", "/")
        ))
    }

    fn is_test(&self, symbol: &Symbol) -> bool {
        // Check if symbol is a test function
        symbol
            .detail
            .as_ref()
            .is_some_and(|d| d.contains("#[test]") || d.contains("#[cfg(test)]"))
            || symbol.name.starts_with("test_")
            || symbol.name.ends_with("_test")
            || symbol.file_path.contains("/tests/")
            || symbol.file_path.ends_with("_test.rs")
    }

    fn get_parent_scope(&self, symbol: &Symbol) -> Option<String> {
        // Extract module path from symbol
        if let Some(detail) = &symbol.detail {
            if detail.contains("impl") {
                // Extract type from impl block
                let impl_part = detail.split("impl").nth(1)?;
                let type_name = impl_part.split_whitespace().next()?;
                return Some(type_name.to_string());
            }
        }

        // Try to extract from file path
        let path = std::path::Path::new(&symbol.file_path);
        let stem = path.file_stem()?.to_str()?;

        if stem != "mod" && stem != "lib" && stem != "main" {
            Some(stem.to_string())
        } else {
            None
        }
    }

    fn score_relevance(&self, symbol: &Symbol, query: &str) -> f32 {
        let mut score = 1.0;

        // Boost for exact name match
        if symbol.name == query {
            score += 2.0;
        }

        // Boost for public symbols
        if self.is_public(symbol) {
            score += 0.5;
        }

        // Penalty for tests
        if self.is_test(symbol) {
            score -= 0.5;
        }

        // Boost for common patterns
        if symbol.kind == SymbolKind::Trait && query.contains("trait") {
            score += 0.5;
        }

        if symbol.kind == SymbolKind::Struct && query.contains("struct") {
            score += 0.5;
        }

        // Boost for impl blocks
        if let Some(detail) = &symbol.detail {
            if detail.contains("impl") && query.contains("impl") {
                score += 0.5;
            }
        }

        score
    }
}

impl RustAdapter {
    fn get_module_path(&self, symbol: &Symbol, _from_file: &str) -> Option<String> {
        // Construct module path from file path and symbol name
        let path = std::path::Path::new(&symbol.file_path);
        let mut components = Vec::new();

        // Extract module components from path
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_str()?;
                if name_str == "src" {
                    continue;
                }
                if let Some(module_name) = name_str.strip_suffix(".rs") {
                    if module_name != "main" && module_name != "lib" {
                        components.push(module_name.to_string());
                    }
                } else {
                    components.push(name_str.to_string());
                }
            }
        }

        components.push(symbol.name.clone());
        Some(components.join("::"))
    }

    fn extract_crate_name(&self, file_path: &str) -> Option<String> {
        // Extract crate name from Cargo.toml or path structure
        let path = std::path::Path::new(file_path);

        // Look for Cargo.toml in parent directories
        let mut current = path.parent()?;
        while current.parent().is_some() {
            if current.join("Cargo.toml").exists() {
                return current.file_name()?.to_str().map(|s| s.to_string());
            }
            current = current.parent()?;
        }

        None
    }
}
