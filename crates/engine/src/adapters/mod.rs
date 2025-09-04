//! Language-specific search adapters

mod go;
mod python;
mod rust;
mod typescript;

pub use go::GoAdapter;
pub use python::PythonAdapter;
pub use rust::RustAdapter;
pub use typescript::TypeScriptAdapter;

use anyhow::Result;
use lsif_core::{Symbol, SymbolKind};

/// Language-specific search adapter trait
pub trait LanguageAdapter {
    /// Get the language name
    fn language(&self) -> &str;

    /// Check if a symbol is exported/public
    fn is_public(&self, symbol: &Symbol) -> bool;

    /// Get import statements for a symbol
    fn get_import_statement(&self, symbol: &Symbol, from_file: &str) -> Option<String>;

    /// Parse language-specific search query
    fn parse_query(&self, query: &str) -> Result<ParsedQuery>;

    /// Get documentation URL for a symbol
    fn get_doc_url(&self, symbol: &Symbol) -> Option<String>;

    /// Check if a symbol is a test
    fn is_test(&self, symbol: &Symbol) -> bool;

    /// Get the parent scope of a symbol
    fn get_parent_scope(&self, symbol: &Symbol) -> Option<String>;

    /// Score relevance for language-specific patterns
    fn score_relevance(&self, symbol: &Symbol, query: &str) -> f32;
}

/// Parsed language-specific query
#[derive(Debug, Clone)]
pub struct ParsedQuery {
    pub symbol_name: Option<String>,
    pub symbol_kind: Option<SymbolKind>,
    pub modifiers: Vec<String>,
    pub scope: Option<String>,
}

/// Registry for language adapters
pub struct AdapterRegistry {
    adapters: std::collections::HashMap<String, Box<dyn LanguageAdapter + Send + Sync>>,
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AdapterRegistry {
    /// Create a new adapter registry
    pub fn new() -> Self {
        let mut registry = Self {
            adapters: std::collections::HashMap::new(),
        };

        // Register default adapters
        registry.register("rust", Box::new(rust::RustAdapter::new()));
        registry.register("typescript", Box::new(typescript::TypeScriptAdapter::new()));
        registry.register("javascript", Box::new(typescript::TypeScriptAdapter::new()));
        registry.register("python", Box::new(python::PythonAdapter::new()));
        registry.register("go", Box::new(go::GoAdapter::new()));

        registry
    }

    /// Register a language adapter
    pub fn register(&mut self, language: &str, adapter: Box<dyn LanguageAdapter + Send + Sync>) {
        self.adapters.insert(language.to_string(), adapter);
    }

    /// Get an adapter for a language
    pub fn get(&self, language: &str) -> Option<&(dyn LanguageAdapter + Send + Sync)> {
        self.adapters.get(language).map(|a| a.as_ref())
    }

    /// Detect language from file extension
    pub fn detect_language(&self, file_path: &str) -> Option<&str> {
        let extension = file_path.rsplit('.').next()?;

        match extension {
            "rs" => Some("rust"),
            "ts" | "tsx" => Some("typescript"),
            "js" | "jsx" => Some("javascript"),
            "py" => Some("python"),
            "go" => Some("go"),
            _ => None,
        }
    }
}
