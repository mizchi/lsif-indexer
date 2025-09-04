use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::RwLock;

/// 言語ごとの正規表現パターン
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Go,
    Python,
    TypeScript,
    JavaScript,
    Java,
    CSharp,
    Cpp,
}

/// プリコンパイルされた正規表現のキャッシュ
pub struct RegexCache {
    patterns: HashMap<(Language, PatternType), Regex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternType {
    Function,
    Class,
    Interface,
    Struct,
    Enum,
    Constant,
    Variable,
    Method,
    Property,
    Import,
    TypeAlias,
}

/// グローバルな正規表現キャッシュ
static REGEX_CACHE: Lazy<RwLock<RegexCache>> = Lazy::new(|| RwLock::new(RegexCache::new()));

impl Default for RegexCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RegexCache {
    /// 新しいキャッシュを作成し、よく使用されるパターンをプリコンパイル
    pub fn new() -> Self {
        let mut cache = RegexCache {
            patterns: HashMap::new(),
        };
        cache.precompile_common_patterns();
        cache
    }

    /// よく使用される正規表現パターンをプリコンパイル
    fn precompile_common_patterns(&mut self) {
        // Rust patterns
        self.compile_and_cache(
            Language::Rust,
            PatternType::Function,
            r#"^\s*(pub\s+)?(async\s+)?(unsafe\s+)?(extern\s+"[^"]+"\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)"#
        );
        self.compile_and_cache(
            Language::Rust,
            PatternType::Struct,
            r"^\s*(pub\s+)?struct\s+([A-Z][a-zA-Z0-9_]*)",
        );
        self.compile_and_cache(
            Language::Rust,
            PatternType::Enum,
            r"^\s*(pub\s+)?enum\s+([A-Z][a-zA-Z0-9_]*)",
        );
        self.compile_and_cache(
            Language::Rust,
            PatternType::Class,
            r"^\s*impl(?:\s+<[^>]+>)?\s+(?:([A-Z][a-zA-Z0-9_]*)|<[^>]+>)",
        );
        self.compile_and_cache(
            Language::Rust,
            PatternType::Constant,
            r"^\s*(pub\s+)?const\s+([A-Z_][A-Z0-9_]*)",
        );

        // TypeScript/JavaScript patterns
        self.compile_and_cache(
            Language::TypeScript,
            PatternType::Function,
            r"^\s*(export\s+)?(async\s+)?function\s+([a-zA-Z_$][a-zA-Z0-9_$]*)",
        );
        self.compile_and_cache(
            Language::TypeScript,
            PatternType::Class,
            r"^\s*(export\s+)?class\s+([A-Z][a-zA-Z0-9_]*)",
        );
        self.compile_and_cache(
            Language::TypeScript,
            PatternType::Interface,
            r"^\s*(export\s+)?interface\s+([A-Z][a-zA-Z0-9_]*)",
        );
        self.compile_and_cache(
            Language::TypeScript,
            PatternType::TypeAlias,
            r"^\s*(export\s+)?type\s+([A-Z][a-zA-Z0-9_]*)",
        );

        // Python patterns
        self.compile_and_cache(
            Language::Python,
            PatternType::Function,
            r"^\s*(?:async\s+)?def\s+([a-z_][a-z0-9_]*)",
        );
        self.compile_and_cache(
            Language::Python,
            PatternType::Class,
            r"^\s*class\s+([A-Z][a-zA-Z0-9_]*)",
        );

        // Go patterns
        self.compile_and_cache(
            Language::Go,
            PatternType::Function,
            r"^\s*func\s+(?:\([^)]+\)\s+)?([A-Z][a-zA-Z0-9_]*)",
        );
        self.compile_and_cache(
            Language::Go,
            PatternType::Struct,
            r"^\s*type\s+([A-Z][a-zA-Z0-9_]*)\s+struct",
        );
        self.compile_and_cache(
            Language::Go,
            PatternType::Interface,
            r"^\s*type\s+([A-Z][a-zA-Z0-9_]*)\s+interface",
        );
    }

    /// パターンをコンパイルしてキャッシュに保存
    fn compile_and_cache(&mut self, lang: Language, pattern_type: PatternType, pattern: &str) {
        if let Ok(regex) = Regex::new(pattern) {
            self.patterns.insert((lang, pattern_type), regex);
        }
    }

    /// キャッシュから正規表現を取得
    pub fn get(&self, lang: Language, pattern_type: PatternType) -> Option<&Regex> {
        self.patterns.get(&(lang, pattern_type))
    }

    /// 動的にパターンを追加
    pub fn add_pattern(
        &mut self,
        lang: Language,
        pattern_type: PatternType,
        pattern: &str,
    ) -> Result<(), regex::Error> {
        let regex = Regex::new(pattern)?;
        self.patterns.insert((lang, pattern_type), regex);
        Ok(())
    }
}

/// グローバルキャッシュから正規表現を取得
pub fn get_cached_regex(lang: Language, pattern_type: PatternType) -> Option<Regex> {
    REGEX_CACHE
        .read()
        .ok()
        .and_then(|cache| cache.get(lang, pattern_type).cloned())
}

/// グローバルキャッシュに新しいパターンを追加
pub fn add_regex_pattern(
    lang: Language,
    pattern_type: PatternType,
    pattern: &str,
) -> Result<(), regex::Error> {
    REGEX_CACHE
        .write()
        .map_err(|_| regex::Error::Syntax("Failed to acquire write lock".to_string()))?
        .add_pattern(lang, pattern_type, pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precompiled_patterns() {
        let cache = RegexCache::new();

        // Rustパターンが正しくプリコンパイルされているか
        assert!(cache.get(Language::Rust, PatternType::Function).is_some());
        assert!(cache.get(Language::Rust, PatternType::Struct).is_some());
        assert!(cache.get(Language::Rust, PatternType::Enum).is_some());

        // TypeScriptパターンが正しくプリコンパイルされているか
        assert!(cache
            .get(Language::TypeScript, PatternType::Function)
            .is_some());
        assert!(cache
            .get(Language::TypeScript, PatternType::Class)
            .is_some());
        assert!(cache
            .get(Language::TypeScript, PatternType::Interface)
            .is_some());
    }

    #[test]
    fn test_pattern_matching() {
        let cache = RegexCache::new();

        // Rust関数パターンのテスト
        let func_regex = cache.get(Language::Rust, PatternType::Function).unwrap();
        assert!(func_regex.is_match("pub fn test_function()"));
        assert!(func_regex.is_match("async fn async_function()"));
        assert!(func_regex.is_match("pub async fn public_async()"));
        assert!(func_regex.is_match("unsafe fn unsafe_function()"));

        // TypeScriptクラスパターンのテスト
        let class_regex = cache.get(Language::TypeScript, PatternType::Class).unwrap();
        assert!(class_regex.is_match("export class MyClass"));
        assert!(class_regex.is_match("class Component"));
    }

    #[test]
    fn test_dynamic_pattern_addition() {
        let mut cache = RegexCache::new();

        // 新しいパターンを動的に追加
        cache
            .add_pattern(
                Language::Rust,
                PatternType::Variable,
                r"^\s*(let|const)\s+(mut\s+)?([a-z_][a-z0-9_]*)",
            )
            .unwrap();

        assert!(cache.get(Language::Rust, PatternType::Variable).is_some());
    }

    #[test]
    fn test_global_cache_access() {
        // グローバルキャッシュから取得
        let regex = get_cached_regex(Language::Rust, PatternType::Function);
        assert!(regex.is_some());

        let func_regex = regex.unwrap();
        assert!(func_regex.is_match("fn main()"));
    }

    #[test]
    fn test_performance_improvement() {
        use std::time::Instant;

        // キャッシュなしの場合
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = Regex::new(r"^\s*(pub\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)");
        }
        let without_cache = start.elapsed();

        // キャッシュありの場合
        let cache = RegexCache::new();
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = cache.get(Language::Rust, PatternType::Function);
        }
        let with_cache = start.elapsed();

        // キャッシュありの方が高速であることを確認
        assert!(
            with_cache < without_cache / 10,
            "Cache should be at least 10x faster. Without cache: {:?}, With cache: {:?}",
            without_cache,
            with_cache
        );
    }
}
