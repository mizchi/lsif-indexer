//! TypeScript/JavaScript language adapter

use super::{LanguageAdapter, ParsedQuery};
use anyhow::Result;
use lsif_core::{Symbol, SymbolKind};

pub struct TypeScriptAdapter;

impl Default for TypeScriptAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeScriptAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageAdapter for TypeScriptAdapter {
    fn language(&self) -> &str {
        "typescript"
    }

    fn is_public(&self, symbol: &Symbol) -> bool {
        // Check if symbol is exported
        symbol.detail.as_ref().is_none_or(|d| {
            d.contains("export")
                || d.contains("public")
                || (!d.contains("private") && !d.contains("protected"))
        })
    }

    fn get_import_statement(&self, symbol: &Symbol, from_file: &str) -> Option<String> {
        // Generate import statement
        let relative_path = self.get_relative_path(&symbol.file_path, from_file)?;

        match symbol.kind {
            SymbolKind::Function
            | SymbolKind::Class
            | SymbolKind::Interface
            | SymbolKind::Variable
            | SymbolKind::Constant => {
                // Check if default export
                if symbol
                    .detail
                    .as_ref()
                    .is_some_and(|d| d.contains("export default"))
                {
                    Some(format!("import {} from '{}';", symbol.name, relative_path))
                } else {
                    Some(format!(
                        "import {{ {} }} from '{}';",
                        symbol.name, relative_path
                    ))
                }
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

        // Parse TypeScript-specific patterns
        // Examples: "class Foo", "interface Bar", "async function", "React.Component"
        let parts: Vec<&str> = query.split_whitespace().collect();

        for part in parts {
            match part {
                "export" | "public" | "private" | "protected" | "static" | "async" | "const"
                | "let" | "var" => {
                    parsed.modifiers.push(part.to_string());
                }
                "function" | "func" => {
                    parsed.symbol_kind = Some(SymbolKind::Function);
                }
                "class" => {
                    parsed.symbol_kind = Some(SymbolKind::Class);
                }
                "interface" => {
                    parsed.symbol_kind = Some(SymbolKind::Interface);
                }
                "enum" => {
                    parsed.symbol_kind = Some(SymbolKind::Enum);
                }
                "type" => {
                    parsed.symbol_kind = Some(SymbolKind::TypeAlias);
                }
                "namespace" | "module" => {
                    parsed.symbol_kind = Some(SymbolKind::Namespace);
                }
                _ => {
                    if part.contains('.') {
                        // Namespace.Symbol pattern
                        let dot_parts: Vec<&str> = part.split('.').collect();
                        if dot_parts.len() == 2 {
                            parsed.scope = Some(dot_parts[0].to_string());
                            parsed.symbol_name = Some(dot_parts[1].to_string());
                        }
                    } else if !part.starts_with('(') && !part.starts_with('<') {
                        parsed.symbol_name = Some(part.to_string());
                    }
                }
            }
        }

        Ok(parsed)
    }

    fn get_doc_url(&self, symbol: &Symbol) -> Option<String> {
        // Try to detect common libraries and generate appropriate docs URLs
        if symbol.file_path.contains("node_modules/@types") {
            // TypeScript definitions
            let type_package = self.extract_package_name(&symbol.file_path)?;
            Some(format!(
                "https://www.npmjs.com/package/@types/{}",
                type_package
            ))
        } else if symbol.file_path.contains("node_modules") {
            // NPM package
            let package = self.extract_package_name(&symbol.file_path)?;
            Some(format!("https://www.npmjs.com/package/{}", package))
        } else {
            None
        }
    }

    fn is_test(&self, symbol: &Symbol) -> bool {
        // Check if symbol is a test
        symbol.name.contains("test")
            || symbol.name.contains("spec")
            || symbol.file_path.contains(".test.")
            || symbol.file_path.contains(".spec.")
            || symbol.file_path.contains("__tests__")
            || symbol.detail.as_ref().is_some_and(|d| {
                d.contains("describe(")
                    || d.contains("it(")
                    || d.contains("test(")
                    || d.contains("expect(")
            })
    }

    fn get_parent_scope(&self, symbol: &Symbol) -> Option<String> {
        // Extract class or namespace from symbol
        if let Some(detail) = &symbol.detail {
            // Look for class membership
            if detail.contains("class ") {
                let class_part = detail.split("class ").nth(1)?;
                let class_name = class_part.split_whitespace().next()?;
                return Some(class_name.to_string());
            }

            // Look for namespace
            if detail.contains("namespace ") {
                let ns_part = detail.split("namespace ").nth(1)?;
                let ns_name = ns_part.split_whitespace().next()?;
                return Some(ns_name.to_string());
            }
        }

        None
    }

    fn score_relevance(&self, symbol: &Symbol, query: &str) -> f32 {
        let mut score = 1.0;

        // Exact match boost
        if symbol.name == query {
            score += 2.0;
        }

        // Export boost
        if symbol.detail.as_ref().is_some_and(|d| d.contains("export")) {
            score += 0.5;
        }

        // Default export boost
        if symbol
            .detail
            .as_ref()
            .is_some_and(|d| d.contains("export default"))
        {
            score += 0.3;
        }

        // Test penalty
        if self.is_test(symbol) {
            score -= 0.5;
        }

        // React component boost
        if symbol.kind == SymbolKind::Class
            && symbol.detail.as_ref().is_some_and(|d| d.contains("React"))
        {
            score += 0.3;
        }

        // Type/Interface boost for type queries
        if (symbol.kind == SymbolKind::Interface || symbol.kind == SymbolKind::TypeAlias)
            && (query.contains("type") || query.contains("interface"))
        {
            score += 0.5;
        }

        score
    }
}

impl TypeScriptAdapter {
    fn get_relative_path(&self, target_file: &str, from_file: &str) -> Option<String> {
        use std::path::Path;

        let target = Path::new(target_file);
        let from = Path::new(from_file);

        let from_dir = from.parent()?;
        let relative = pathdiff::diff_paths(target, from_dir)?;

        // Convert to module path (remove extension)
        let mut module_path = relative.to_str()?.to_string();
        if module_path.ends_with(".ts") || module_path.ends_with(".js") {
            module_path = module_path[..module_path.len() - 3].to_string();
        } else if module_path.ends_with(".tsx") || module_path.ends_with(".jsx") {
            module_path = module_path[..module_path.len() - 4].to_string();
        }

        // Ensure relative path starts with ./
        if !module_path.starts_with('.') {
            module_path = format!("./{}", module_path);
        }

        Some(module_path)
    }

    fn extract_package_name(&self, file_path: &str) -> Option<String> {
        let parts: Vec<&str> = file_path.split("node_modules/").collect();
        if parts.len() < 2 {
            return None;
        }

        let package_part = parts[1];
        let package_components: Vec<&str> = package_part.split('/').collect();

        if package_components[0].starts_with('@') {
            // Scoped package
            if package_components.len() >= 2 {
                Some(format!(
                    "{}/{}",
                    package_components[0], package_components[1]
                ))
            } else {
                None
            }
        } else {
            // Regular package
            Some(package_components[0].to_string())
        }
    }
}
