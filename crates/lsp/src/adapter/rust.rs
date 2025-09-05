use super::common::CommonAdapter;
use super::language::{LanguageAdapter, DefinitionPattern, PatternType};
use super::lsp::LspAdapter;
use lsp_types::SymbolKind;
use anyhow::Result;
use std::process::{Child, Command, Stdio};

/// Rust言語用のLSPアダプタ
pub struct RustAdapter {
    common: CommonAdapter,
}

impl Default for RustAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RustAdapter {
    pub fn new() -> Self {
        Self {
            common: CommonAdapter::new("rust", "rust-analyzer", vec!["rs"], vec!["//", "/*", "*/"]),
        }
    }

    pub fn get_adapter(&self) -> &CommonAdapter {
        &self.common
    }

    /// Rust固有の定義パターン
    pub fn get_definition_keywords(&self) -> Vec<&str> {
        vec![
            "fn", "struct", "enum", "trait", "impl", "mod", "type", "const", "static", "macro",
        ]
    }

    /// Rust固有の参照パターン
    pub fn get_reference_patterns(&self) -> Vec<&str> {
        vec![
            r"\b{}\s*\(",            // 関数呼び出し
            r"\b{}::",               // モジュールパス
            r"::{}\b",               // use文
            r"\b{}\s*\{{",           // 構造体初期化
            r"<\s*{}\s*>",           // ジェネリクス
            r":\s*{}\b",             // 型注釈
            r"impl\s+.*\s+for\s+{}", // trait実装
            r"as\s+{}\b",            // 型キャスト
        ]
    }

    /// Rustのシンボル種別を判定
    pub fn infer_symbol_kind(&self, context: &str) -> SymbolKind {
        let trimmed = context.trim();
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
            SymbolKind::FUNCTION
        } else if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") {
            SymbolKind::STRUCT
        } else if trimmed.starts_with("enum ") || trimmed.starts_with("pub enum ") {
            SymbolKind::ENUM
        } else if trimmed.starts_with("trait ") || trimmed.starts_with("pub trait ") {
            SymbolKind::INTERFACE
        } else if trimmed.starts_with("impl ") {
            SymbolKind::CLASS
        } else if trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") {
            SymbolKind::MODULE
        } else if trimmed.starts_with("type ") || trimmed.starts_with("pub type ") {
            SymbolKind::TYPE_PARAMETER
        } else if trimmed.starts_with("const ") || trimmed.starts_with("pub const ") {
            SymbolKind::CONSTANT
        } else if trimmed.starts_with("static ") || trimmed.starts_with("pub static ") {
            SymbolKind::VARIABLE
        } else if trimmed.starts_with("macro_rules!") {
            SymbolKind::FUNCTION
        } else {
            SymbolKind::VARIABLE
        }
    }
}

impl LanguageAdapter for RustAdapter {
    fn language_id(&self) -> &str {
        &self.common.language_id
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["rs"]
    }

    fn spawn_lsp_command(&self) -> Result<Child> {
        Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn rust-analyzer: {}", e))
    }

    fn definition_patterns(&self) -> Vec<DefinitionPattern> {
        vec![
            DefinitionPattern {
                keywords: vec!["fn".to_string()],
                pattern_type: PatternType::FunctionDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["struct".to_string()],
                pattern_type: PatternType::ClassDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["enum".to_string()],
                pattern_type: PatternType::EnumDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["trait".to_string()],
                pattern_type: PatternType::InterfaceDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["type".to_string()],
                pattern_type: PatternType::TypeDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["const".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["mod".to_string()],
                pattern_type: PatternType::ModuleDef,
                requires_name_after: true,
            },
        ]
    }

    fn build_reference_pattern(&self, name: &str, _kind: &lsif_core::SymbolKind) -> String {
        format!(r"\b{}\b", regex::escape(name))
    }

    fn is_definition_context(&self, line: &str, position: usize) -> bool {
        // Check if position is after a definition keyword
        let keywords = self.get_definition_keywords();
        for keyword in keywords {
            if let Some(idx) = line.find(keyword) {
                if position > idx {
                    return true;
                }
            }
        }
        false
    }

    fn is_in_string_or_comment(&self, line: &str, position: usize) -> bool {
        // Simple check for comments and strings
        let before_pos = &line[..position.min(line.len())];
        
        // Check for line comment
        if before_pos.contains("//") {
            return true;
        }
        
        // Check for string literals (simple check)
        let mut in_string = false;
        let mut escape_next = false;
        for (i, ch) in before_pos.chars().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }
            if ch == '\\' {
                escape_next = true;
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
            }
            if i == position && in_string {
                return true;
            }
        }
        
        false
    }
}

impl LspAdapter for RustAdapter {
    fn spawn_command(&self) -> Result<Child> {
        Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn rust-analyzer: {}", e))
    }

    fn language_id(&self) -> &str {
        LanguageAdapter::language_id(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_adapter_basic_info() {
        let adapter = RustAdapter::new();
        assert_eq!(adapter.get_adapter().language_id, "rust");
        assert_eq!(adapter.get_adapter().lsp_server_name, "rust-analyzer");
        assert!(adapter
            .get_adapter()
            .file_extensions
            .contains(&"rs".to_string()));
    }

    #[test]
    fn test_rust_definition_keywords() {
        let adapter = RustAdapter::new();
        let keywords = adapter.get_definition_keywords();
        assert!(keywords.contains(&"fn"));
        assert!(keywords.contains(&"struct"));
        assert!(keywords.contains(&"trait"));
        assert!(keywords.contains(&"impl"));
    }

    #[test]
    fn test_rust_symbol_kind_inference() {
        let adapter = RustAdapter::new();
        assert_eq!(adapter.infer_symbol_kind("fn main()"), SymbolKind::FUNCTION);
        assert_eq!(
            adapter.infer_symbol_kind("pub struct User"),
            SymbolKind::STRUCT
        );
        assert_eq!(
            adapter.infer_symbol_kind("trait Display"),
            SymbolKind::INTERFACE
        );
        assert_eq!(
            adapter.infer_symbol_kind("impl Display for User"),
            SymbolKind::CLASS
        );
        assert_eq!(adapter.infer_symbol_kind("mod utils"), SymbolKind::MODULE);
    }
}
