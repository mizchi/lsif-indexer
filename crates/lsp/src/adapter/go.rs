use super::common::{c_style_comments, spawn_lsp_server};
use super::language::{LanguageAdapter, DefinitionPattern, PatternType};
use super::minimal::CommentStyles;
use anyhow::Result;

/// Go言語のアダプタ実装
/// goplsを使用してGo言語のコードを解析
pub struct GoAdapter;

impl LanguageAdapter for GoAdapter {
    fn language_id(&self) -> &str {
        "go"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["go"]
    }

    fn spawn_lsp_command(&self) -> Result<std::process::Child> {
        spawn_lsp_server("gopls", &["serve"])
    }

    fn definition_patterns(&self) -> Vec<DefinitionPattern> {
        vec![
            DefinitionPattern {
                keywords: vec!["func".to_string()],
                pattern_type: PatternType::FunctionDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["type".to_string()],
                pattern_type: PatternType::TypeDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["var".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["const".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["struct".to_string()],
                pattern_type: PatternType::ClassDef,
                requires_name_after: false,
            },
            DefinitionPattern {
                keywords: vec!["interface".to_string()],
                pattern_type: PatternType::InterfaceDef,
                requires_name_after: false,
            },
        ]
    }

    fn build_reference_pattern(&self, name: &str, _kind: &core::SymbolKind) -> String {
        format!(r"\b{}\b", regex::escape(name))
    }

    fn is_definition_context(&self, line: &str, position: usize) -> bool {
        // Goの定義キーワードの前にあるかチェック
        let before = &line[..position.min(line.len())];
        before.contains("func ") || before.contains("type ") || 
        before.contains("var ") || before.contains("const ")
    }

    fn is_in_string_or_comment(&self, line: &str, position: usize) -> bool {
        let before = &line[..position.min(line.len())];
        // 簡易的な判定
        before.contains("//") || before.contains("/*") || 
        before.chars().filter(|&c| c == '"').count() % 2 == 1 ||
        before.chars().filter(|&c| c == '`').count() % 2 == 1
    }
}

impl GoAdapter {
    /// Goの定義キーワードかどうかを判定
    pub fn is_definition_keyword(&self, keyword: &str) -> bool {
        matches!(
            keyword,
            "func" | "var" | "const" | "type" | "struct" | "interface" | "package"
        )
    }

    /// Go特有の参照パターンを構築
    pub fn build_reference_pattern(&self, name: &str, is_package: bool) -> String {
        if is_package {
            // パッケージ参照の場合、ドットチェーンを考慮
            format!(r"\b{}(?:\.\w+)*\b", regex::escape(name))
        } else {
            // 通常の識別子
            format!(r"\b{}\b", regex::escape(name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_adapter_basic_info() {
        let adapter = GoAdapter;
        assert_eq!(adapter.language_id(), "go");
        assert_eq!(adapter.supported_extensions(), vec!["go"]);
    }

    #[test]
    fn test_go_reference_patterns() {
        let adapter = GoAdapter;

        // 通常の識別子
        let pattern = adapter.build_reference_pattern("main", false);
        assert_eq!(pattern, r"\bmain\b");

        // パッケージ参照（ドットチェーン対応）
        let pattern = adapter.build_reference_pattern("fmt", true);
        assert_eq!(pattern, r"\bfmt(?:\.\w+)*\b");
    }

    #[test]
    fn test_go_definition_keywords() {
        let adapter = GoAdapter;
        assert!(adapter.is_definition_keyword("func"));
        assert!(adapter.is_definition_keyword("type"));
        assert!(adapter.is_definition_keyword("var"));
        assert!(adapter.is_definition_keyword("const"));
        assert!(adapter.is_definition_keyword("struct"));
        assert!(adapter.is_definition_keyword("interface"));
        assert!(!adapter.is_definition_keyword("if"));
        assert!(!adapter.is_definition_keyword("for"));
    }
}
