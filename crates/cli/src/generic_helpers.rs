/// 言語非依存の汎用ヘルパー関数
///
/// すべての言語で共通利用可能な解析ロジック
use lsif_core::SymbolKind;

/// 基本的な参照パターンを構築（99%のケースをカバー）
pub fn build_basic_reference_pattern(name: &str) -> String {
    format!(r"\b{}\b", regex::escape(name))
}

/// 文字列リテラルまたはコメント内かを判定（C系言語で共通）
pub fn is_in_string_or_comment(line: &str, position: usize) -> bool {
    let before = &line[..position.min(line.len())];

    // 単一行コメントチェック
    if let Some(comment_pos) = before.find("//") {
        // 文字列内の // でない場合のみ
        let before_comment = &before[..comment_pos];
        if !is_in_string_literal(before_comment, comment_pos) {
            return position > comment_pos;
        }
    }

    // # スタイルのコメント（Python, Ruby, Shell等）
    if let Some(comment_pos) = before.find('#') {
        let before_comment = &before[..comment_pos];
        if !is_in_string_literal(before_comment, comment_pos) {
            return position > comment_pos;
        }
    }

    // -- スタイルのコメント（SQL, Haskell等）
    if let Some(comment_pos) = before.find("--") {
        let before_comment = &before[..comment_pos];
        if !is_in_string_literal(before_comment, comment_pos) {
            return position > comment_pos;
        }
    }

    // 文字列リテラル内かチェック
    is_in_string_literal(before, position)
}

/// 文字列リテラル内かを判定
pub fn is_in_string_literal(text: &str, _position: usize) -> bool {
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    let mut in_backtick = false;
    let mut in_triple_double = false;
    let mut in_triple_single = false;
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

        // トリプルクォート（Python）のチェック
        if i + 2 < chars.len() {
            if chars[i] == '"'
                && chars[i + 1] == '"'
                && chars[i + 2] == '"'
                && !in_single_quote
                && !in_double_quote
            {
                in_triple_double = !in_triple_double;
                i += 3;
                continue;
            }
            if chars[i] == '\''
                && chars[i + 1] == '\''
                && chars[i + 2] == '\''
                && !in_single_quote
                && !in_double_quote
            {
                in_triple_single = !in_triple_single;
                i += 3;
                continue;
            }
        }

        // トリプルクォート内ではエスケープを無視
        if in_triple_double || in_triple_single {
            i += 1;
            continue;
        }

        match chars[i] {
            '\\' if !in_raw_string => escaped = true,
            'r' if i + 1 < chars.len()
                && chars[i + 1] == '"'
                && !in_double_quote
                && !in_single_quote
                && !in_backtick =>
            {
                // Rust/Pythonの raw string
                in_raw_string = true;
                i += 1;
            }
            '"' if !in_single_quote && !in_backtick => {
                if in_raw_string {
                    in_raw_string = false;
                } else {
                    in_double_quote = !in_double_quote;
                }
            }
            '\'' if !in_double_quote && !in_backtick && !in_raw_string => {
                in_single_quote = !in_single_quote;
            }
            '`' if !in_double_quote && !in_single_quote && !in_raw_string => {
                // JavaScript/TypeScript/Go のテンプレートリテラル
                in_backtick = !in_backtick;
            }
            _ => {}
        }
        i += 1;
    }

    in_double_quote
        || in_single_quote
        || in_backtick
        || in_raw_string
        || in_triple_double
        || in_triple_single
}

/// ブロックコメント内かを判定
pub fn is_in_block_comment(
    content: &str,
    position: usize,
    block_start: &str,
    block_end: &str,
) -> bool {
    let before = &content[..position.min(content.len())];
    let after = &content[position.min(content.len())..];

    // 直前までのブロック開始/終了をカウント
    let starts_before = before.matches(block_start).count();
    let ends_before = before.matches(block_end).count();

    // 位置以降の最初の終了タグを探す
    let has_end_after = after.contains(block_end);

    // 開始が終了より多く、かつ後ろに終了タグがある場合はコメント内
    starts_before > ends_before && has_end_after
}

/// 定義キーワードのパターン
#[derive(Debug, Clone)]
pub struct DefinitionKeywords {
    pub function: Vec<&'static str>,
    pub class: Vec<&'static str>,
    pub interface: Vec<&'static str>,
    pub variable: Vec<&'static str>,
    pub type_alias: Vec<&'static str>,
    pub enum_def: Vec<&'static str>,
    pub module: Vec<&'static str>,
}

impl Default for DefinitionKeywords {
    fn default() -> Self {
        Self {
            // 多言語で共通のキーワード
            function: vec!["function", "fn", "def", "func", "sub", "proc"],
            class: vec!["class", "struct", "record", "data"],
            interface: vec!["interface", "trait", "protocol"],
            variable: vec!["let", "const", "var", "val", "static"],
            type_alias: vec!["type", "typedef", "alias"],
            enum_def: vec!["enum", "enumeration"],
            module: vec!["module", "mod", "namespace", "package"],
        }
    }
}

/// 汎用的な定義コンテキスト判定
pub fn is_definition_context(line: &str, position: usize, keywords: &DefinitionKeywords) -> bool {
    // 現在位置が単語の先頭かを確認
    if position > 0 {
        let prev_char = line.chars().nth(position - 1);
        if let Some(ch) = prev_char {
            if ch.is_alphanumeric() || ch == '_' {
                return false;
            }
        }
    }

    let before = &line[..position.min(line.len())];
    let words: Vec<&str> = before.split_whitespace().collect();

    if words.is_empty() {
        return false;
    }

    let last_word = words[words.len() - 1];

    // 各種定義キーワードをチェック
    let all_keywords = [
        &keywords.function[..],
        &keywords.class[..],
        &keywords.interface[..],
        &keywords.variable[..],
        &keywords.type_alias[..],
        &keywords.enum_def[..],
        &keywords.module[..],
    ]
    .concat();

    // 直前の単語が定義キーワードか
    if words.len() >= 2 {
        let prev_word = words[words.len() - 2];
        if all_keywords.contains(&prev_word) {
            return true;
        }

        // export/public/private + キーワードのパターン
        if words.len() >= 3 {
            let modifier = words[words.len() - 3];
            let keyword = words[words.len() - 2];
            if matches!(modifier, "export" | "public" | "private" | "protected")
                && all_keywords.contains(&keyword)
            {
                return true;
            }
        }
    }

    // 代入文のパターン（const name = ...）
    if last_word != "=" && before.contains('=') {
        if let Some(eq_pos) = before.rfind('=') {
            let before_eq = &before[..eq_pos].trim();
            let words_before_eq: Vec<&str> = before_eq.split_whitespace().collect();
            if words_before_eq.len() >= 2 {
                let var_keyword = words_before_eq[words_before_eq.len() - 2];
                if keywords.variable.contains(&var_keyword) {
                    return true;
                }
            }
        }
    }

    false
}

/// シンボル種別に応じた参照パターンの拡張
pub struct ReferencePatternContext {
    pub symbol_kind: SymbolKind,
    pub language_id: String,
}

/// 言語別の参照パターン拡張
pub fn extend_reference_pattern(basic_pattern: &str, context: &ReferencePatternContext) -> String {
    match context.language_id.as_str() {
        "rust" => {
            // Rustは :: でモジュールパスを表現
            match context.symbol_kind {
                SymbolKind::Module | SymbolKind::Struct | SymbolKind::Enum => {
                    // モジュール::要素 のパターンに対応
                    format!(r"{}(?:::\w+)*", basic_pattern.trim_end_matches(r"\b"))
                }
                _ => basic_pattern.to_string(),
            }
        }
        "cpp" | "c++" => {
            // C++は ::, ->, . でメンバーアクセス
            match context.symbol_kind {
                SymbolKind::Class | SymbolKind::Struct => {
                    format!(
                        r"{}(?:(?:::|-&gt;|\.)\w+)*",
                        basic_pattern.trim_end_matches(r"\b")
                    )
                }
                _ => basic_pattern.to_string(),
            }
        }
        "go" => {
            // Goは . でパッケージメンバーアクセス
            match context.symbol_kind {
                SymbolKind::Module => {
                    format!(r"{}(?:\.\w+)*", basic_pattern.trim_end_matches(r"\b"))
                }
                _ => basic_pattern.to_string(),
            }
        }
        "python" | "java" | "csharp" | "c#" => {
            // これらの言語は . でメンバーアクセス
            match context.symbol_kind {
                SymbolKind::Module | SymbolKind::Class => {
                    format!(r"{}(?:\.\w+)*", basic_pattern.trim_end_matches(r"\b"))
                }
                _ => basic_pattern.to_string(),
            }
        }
        _ => {
            // その他の言語は基本パターンのまま
            basic_pattern.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_literal_detection() {
        // is_in_string_literal checks if the whole string up to position is in a literal
        // For "hello world", we need to check within the quotes
        assert!(is_in_string_literal("\"hello", 6));
        assert!(!is_in_string_literal("\"hello\" world", 14));
        assert!(is_in_string_literal("'hello", 6));
        assert!(is_in_string_literal("`template", 9));
        assert!(is_in_string_literal("r\"raw", 5));
        assert!(is_in_string_literal("\"\"\"triple", 9));
    }

    #[test]
    fn test_comment_detection() {
        assert!(is_in_string_or_comment("// comment", 5));
        assert!(is_in_string_or_comment("# python comment", 10));
        assert!(is_in_string_or_comment("-- sql comment", 8));
        assert!(!is_in_string_or_comment("normal code", 5));
    }

    #[test]
    fn test_block_comment() {
        let content = "code /* comment */ more";
        assert!(is_in_block_comment(content, 10, "/*", "*/"));
        assert!(!is_in_block_comment(content, 20, "/*", "*/"));
    }

    #[test]
    fn test_definition_context() {
        let keywords = DefinitionKeywords::default();

        // The function requires at least 2 words in the substring before position
        // It checks if words[len-2] is a definition keyword

        // Test with positions after identifier with space/symbol
        assert!(is_definition_context("function myFunc ", 16, &keywords)); // after "myFunc "
        assert!(is_definition_context("class C ", 8, &keywords)); // after 'C '
        assert!(is_definition_context("let v ", 6, &keywords)); // after 'v '

        // 3 words with modifier
        assert!(is_definition_context("export function f ", 18, &keywords)); // after 'f '
        assert!(is_definition_context("public class C ", 15, &keywords)); // after 'C '

        // Variable assignment
        assert!(is_definition_context("const x = 5", 8, &keywords)); // after 'x '

        // Not definition contexts
        assert!(!is_definition_context("myFunc()", 7, &keywords)); // No keyword
        assert!(!is_definition_context("function ", 9, &keywords)); // Only ["function"], needs 2 words

        // Edge case: position at start of identifier name
        assert!(!is_definition_context("let", 3, &keywords)); // Only one word
    }

    #[test]
    fn test_reference_pattern_extension() {
        let basic = r"\bfoo\b";

        let rust_context = ReferencePatternContext {
            symbol_kind: SymbolKind::Module,
            language_id: "rust".to_string(),
        };
        assert!(extend_reference_pattern(basic, &rust_context).contains("::"));

        let python_context = ReferencePatternContext {
            symbol_kind: SymbolKind::Module,
            language_id: "python".to_string(),
        };
        assert!(extend_reference_pattern(basic, &python_context).contains(r"\."));

        let js_context = ReferencePatternContext {
            symbol_kind: SymbolKind::Function,
            language_id: "javascript".to_string(),
        };
        assert_eq!(extend_reference_pattern(basic, &js_context), basic);
    }
}
