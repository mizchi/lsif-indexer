use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};

/// 言語固有の定義パターン
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionPattern {
    /// キーワード（例: ["export", "function"], ["class"]）
    pub keywords: Vec<String>,
    /// パターンの種類
    pub pattern_type: PatternType,
    /// キーワードの後に名前が必要か
    pub requires_name_after: bool,
}

/// パターンの種類
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PatternType {
    FunctionDef,  // function, fn, def
    ClassDef,     // class, struct
    InterfaceDef, // interface, trait
    VariableDef,  // let, const, var
    TypeDef,      // type, typedef
    EnumDef,      // enum
    ModuleDef,    // module, mod
}

/// 言語アダプターのトレイト
pub trait LanguageAdapter: Send + Sync {
    /// 言語ID（例: "rust", "typescript"）
    fn language_id(&self) -> &str;

    /// サポートする拡張子
    fn supported_extensions(&self) -> Vec<&str>;

    /// LSPサーバーを起動
    fn spawn_lsp_command(&self) -> Result<Child>;

    /// 定義パターンを取得
    fn definition_patterns(&self) -> Vec<DefinitionPattern>;

    /// 参照パターンの正規表現を構築
    fn build_reference_pattern(&self, name: &str, kind: &lsif_core::SymbolKind) -> String;

    /// 定義コンテキストかを判定
    fn is_definition_context(&self, line: &str, position: usize) -> bool;

    /// 文字列リテラルやコメント内かを判定
    fn is_in_string_or_comment(&self, line: &str, position: usize) -> bool;

    /// ソースファイルかを判定
    fn is_source_file(&self, path: &std::path::Path) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                return self.supported_extensions().contains(&ext_str);
            }
        }
        false
    }
}

/// Rust言語アダプター
pub struct RustLanguageAdapter;

impl LanguageAdapter for RustLanguageAdapter {
    fn language_id(&self) -> &str {
        "rust"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["rs"]
    }

    fn spawn_lsp_command(&self) -> Result<Child> {
        Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start rust-analyzer: {}", e))
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
                keywords: vec!["let".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["static".to_string()],
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

    fn build_reference_pattern(&self, name: &str, kind: &lsif_core::SymbolKind) -> String {
        use lsif_core::SymbolKind;
        let escaped = regex::escape(name);

        match kind {
            SymbolKind::Function | SymbolKind::Method => {
                // 関数は名前の後に括弧かジェネリクス
                format!(r"\b{}\b", escaped)
            }
            SymbolKind::Struct | SymbolKind::Enum => {
                // 構造体は :: でメソッドアクセスされることがある
                format!(r"\b{}(?:\b|::)", escaped)
            }
            _ => {
                // その他は単純な単語境界
                format!(r"\b{}\b", escaped)
            }
        }
    }

    fn is_definition_context(&self, line: &str, position: usize) -> bool {
        generic_is_definition_context(line, position, &self.definition_patterns())
    }

    fn is_in_string_or_comment(&self, line: &str, position: usize) -> bool {
        generic_is_in_string_or_comment(line, position)
    }
}

/// TypeScript言語アダプター
pub struct TypeScriptLanguageAdapter;

impl LanguageAdapter for TypeScriptLanguageAdapter {
    fn language_id(&self) -> &str {
        "typescript"
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["ts", "tsx", "js", "jsx"]
    }

    fn spawn_lsp_command(&self) -> Result<Child> {
        Command::new("npx")
            .arg("-y")
            .arg("@typescript/native-preview")
            .arg("--lsp")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start TypeScript LSP: {}", e))
    }

    fn definition_patterns(&self) -> Vec<DefinitionPattern> {
        vec![
            // Export patterns
            DefinitionPattern {
                keywords: vec!["export".to_string(), "function".to_string()],
                pattern_type: PatternType::FunctionDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["export".to_string(), "class".to_string()],
                pattern_type: PatternType::ClassDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["export".to_string(), "interface".to_string()],
                pattern_type: PatternType::InterfaceDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["export".to_string(), "const".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["export".to_string(), "let".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["export".to_string(), "var".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["export".to_string(), "type".to_string()],
                pattern_type: PatternType::TypeDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["export".to_string(), "enum".to_string()],
                pattern_type: PatternType::EnumDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec![
                    "export".to_string(),
                    "async".to_string(),
                    "function".to_string(),
                ],
                pattern_type: PatternType::FunctionDef,
                requires_name_after: true,
            },
            // Non-export patterns
            DefinitionPattern {
                keywords: vec!["async".to_string(), "function".to_string()],
                pattern_type: PatternType::FunctionDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["function".to_string()],
                pattern_type: PatternType::FunctionDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["class".to_string()],
                pattern_type: PatternType::ClassDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["interface".to_string()],
                pattern_type: PatternType::InterfaceDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["const".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["let".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["var".to_string()],
                pattern_type: PatternType::VariableDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["type".to_string()],
                pattern_type: PatternType::TypeDef,
                requires_name_after: true,
            },
            DefinitionPattern {
                keywords: vec!["enum".to_string()],
                pattern_type: PatternType::EnumDef,
                requires_name_after: true,
            },
        ]
    }

    fn build_reference_pattern(&self, name: &str, _kind: &lsif_core::SymbolKind) -> String {
        let escaped = regex::escape(name);
        // TypeScriptではすべて単純な単語境界でOK（::は使わない）
        format!(r"\b{}\b", escaped)
    }

    fn is_definition_context(&self, line: &str, position: usize) -> bool {
        generic_is_definition_context(line, position, &self.definition_patterns())
    }

    fn is_in_string_or_comment(&self, line: &str, position: usize) -> bool {
        generic_is_in_string_or_comment(line, position)
    }
}

/// 汎用的な定義コンテキスト判定
fn generic_is_definition_context(
    line: &str,
    position: usize,
    patterns: &[DefinitionPattern],
) -> bool {
    // 現在位置が単語の先頭かを確認
    if position > 0 {
        let prev_char = line.chars().nth(position - 1);
        if let Some(ch) = prev_char {
            if ch.is_alphanumeric() || ch == '_' {
                // 単語の途中なので定義ではない
                return false;
            }
        }
    }

    // 位置より前の部分を取得
    let before = &line[..position.min(line.len())];

    // 前方の単語列を取得
    let words: Vec<&str> = before.split_whitespace().collect();
    if words.is_empty() {
        return false;
    }

    // パターンマッチング
    for pattern in patterns {
        if words.len() >= pattern.keywords.len() {
            let start_idx = words.len() - pattern.keywords.len();
            let matching_part = &words[start_idx..];
            let pattern_words: Vec<&str> = pattern.keywords.iter().map(|s| s.as_str()).collect();
            if matching_part == pattern_words.as_slice() {
                return true;
            }
        }
    }

    // 変数定義の特別処理
    if words.len() >= 2 {
        let last_word = words[words.len() - 1];
        let second_last = words[words.len() - 2];

        // 変数定義パターン（const/let/var name = ...）
        if matches!(second_last, "const" | "let" | "var") && !last_word.contains('=') {
            return true;
        }

        // export const/let/var name = ...
        if second_last == "export" && words.len() >= 3 {
            let third_last = words[words.len() - 3];
            if matches!(third_last, "const" | "let" | "var") && !last_word.contains('=') {
                return true;
            }
        }
    }

    false
}

/// 汎用的な文字列・コメント判定
fn generic_is_in_string_or_comment(line: &str, position: usize) -> bool {
    let before = &line[..position.min(line.len())];

    // 単一行コメントチェック
    if let Some(comment_pos) = before.find("//") {
        // 文字列内の // でない場合のみ
        let before_comment = &before[..comment_pos];
        if !is_in_string_literal(before_comment, comment_pos) {
            return position > comment_pos;
        }
    }

    // 文字列リテラル内かチェック
    is_in_string_literal(before, position)
}

/// 文字列リテラル内かを判定
fn is_in_string_literal(text: &str, _position: usize) -> bool {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut in_raw_string = false;

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        match chars[i] {
            '\\' if !in_raw_string => escaped = true,
            'r' if i + 1 < chars.len() && chars[i + 1] == '"' && !in_string && !in_char => {
                in_raw_string = true;
                i += 1; // Skip the next character
            }
            '"' if !in_char => {
                if in_raw_string {
                    in_raw_string = false;
                } else {
                    in_string = !in_string;
                }
            }
            '\'' if !in_string && !in_raw_string => in_char = !in_char,
            _ => {}
        }
        i += 1;
    }

    in_string || in_char || in_raw_string
}

/// ファイル拡張子から言語アダプターを検出
pub fn detect_language_adapter(file_path: &str) -> Option<Box<dyn LanguageAdapter>> {
    let extension = std::path::Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())?;

    match extension {
        "rs" => Some(Box::new(RustLanguageAdapter)),
        "ts" | "tsx" | "js" | "jsx" => Some(Box::new(TypeScriptLanguageAdapter)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_adapter() {
        let adapter = RustLanguageAdapter;
        assert_eq!(adapter.language_id(), "rust");
        assert_eq!(adapter.supported_extensions(), vec!["rs"]);

        let patterns = adapter.definition_patterns();
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_typescript_adapter() {
        let adapter = TypeScriptLanguageAdapter;
        assert_eq!(adapter.language_id(), "typescript");
        assert_eq!(
            adapter.supported_extensions(),
            vec!["ts", "tsx", "js", "jsx"]
        );

        let patterns = adapter.definition_patterns();
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_detect_language() {
        assert!(detect_language_adapter("main.rs").is_some());
        assert!(detect_language_adapter("index.ts").is_some());
        assert!(detect_language_adapter("unknown.xyz").is_none());
    }
}
