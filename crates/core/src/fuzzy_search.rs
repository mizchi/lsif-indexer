use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::graph::{CodeGraph, Symbol};

/// 検索結果の型
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub symbol: Symbol,
    pub score: f32,
    pub match_type: MatchType,
}

/// マッチの種類
#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    Exact,     // 完全一致
    Prefix,    // 前方一致
    Substring, // 部分文字列
    CamelCase, // CamelCase略語
    Fuzzy,     // ファジーマッチ
    Typo,      // タイポ許容
}

/// Fuzzy search用のインデックス
///
/// 複数の検索戦略を組み合わせて高精度な検索を実現:
/// - N-gram（2-gram, 3-gram）によるインデックス
/// - 前方一致用のトライ構造（概念的に）
/// - CamelCase分解
/// - レーベンシュタイン距離による類似度計算
pub struct FuzzySearchIndex {
    /// シンボル情報: symbol ID -> Symbol
    symbols: Arc<DashMap<String, Symbol>>,
    /// 名前の正規化マップ: lowercase name -> symbol IDs
    name_index: Arc<DashMap<String, HashSet<String>>>,
    /// 2-gramインデックス: bigram -> symbol IDs
    bigram_index: Arc<DashMap<String, HashSet<String>>>,
    /// 3-gramインデックス: trigram -> symbol IDs
    trigram_index: Arc<DashMap<String, HashSet<String>>>,
    /// 前方一致用: prefix -> symbol IDs（最大5文字）
    prefix_index: Arc<DashMap<String, HashSet<String>>>,
    /// CamelCase分解後の単語インデックス: word -> symbol IDs
    word_index: Arc<DashMap<String, HashSet<String>>>,
}

impl Default for FuzzySearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzySearchIndex {
    /// 新しいFuzzySearchIndexを作成
    pub fn new() -> Self {
        Self {
            symbols: Arc::new(DashMap::new()),
            name_index: Arc::new(DashMap::new()),
            bigram_index: Arc::new(DashMap::new()),
            trigram_index: Arc::new(DashMap::new()),
            prefix_index: Arc::new(DashMap::new()),
            word_index: Arc::new(DashMap::new()),
        }
    }

    /// CodeGraphからインデックスを構築
    pub fn build_from_graph(graph: &CodeGraph) -> Self {
        let index = Self::new();

        for symbol in graph.get_all_symbols() {
            index.add_symbol(symbol.clone());
        }

        index
    }

    /// シンボルをインデックスに追加
    pub fn add_symbol(&self, symbol: Symbol) {
        let id = symbol.id.clone();
        let name = symbol.name.clone();

        // シンボル情報を保存
        self.symbols.insert(id.clone(), symbol);

        // 正規化された名前でインデックス
        let name_lower = name.to_lowercase();
        self.name_index
            .entry(name_lower.clone())
            .or_default()
            .insert(id.clone());

        // N-gramインデックス
        let bigrams = Self::generate_ngrams(&name_lower, 2);
        for bigram in bigrams {
            self.bigram_index
                .entry(bigram)
                .or_default()
                .insert(id.clone());
        }

        let trigrams = Self::generate_ngrams(&name_lower, 3);
        for trigram in trigrams {
            self.trigram_index
                .entry(trigram)
                .or_default()
                .insert(id.clone());
        }

        // 前方一致インデックス（最大5文字まで）
        let chars: Vec<char> = name_lower.chars().collect();
        for i in 1..=5.min(chars.len()) {
            let prefix: String = chars[..i].iter().collect();
            self.prefix_index
                .entry(prefix)
                .or_default()
                .insert(id.clone());
        }

        // CamelCase分解
        let words = Self::split_camel_case(&name);
        for word in words {
            let word_lower = word.to_lowercase();
            if !word_lower.is_empty() {
                self.word_index
                    .entry(word_lower)
                    .or_default()
                    .insert(id.clone());
            }
        }
    }

    /// 検索を実行
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut results = HashMap::new();

        // 1. 完全一致（最高スコア）
        if let Some(ids) = self.name_index.get(&query_lower) {
            for id in ids.iter() {
                if let Some(symbol) = self.symbols.get(id) {
                    results.insert(
                        id.clone(),
                        SearchResult {
                            symbol: symbol.clone(),
                            score: 100.0,
                            match_type: MatchType::Exact,
                        },
                    );
                }
            }
        }

        // 2. 前方一致
        if query_lower.len() <= 5 {
            if let Some(ids) = self.prefix_index.get(&query_lower) {
                for id in ids.iter() {
                    if !results.contains_key(id) {
                        if let Some(symbol) = self.symbols.get(id) {
                            let name_lower = symbol.name.to_lowercase();
                            if name_lower.starts_with(&query_lower) {
                                let score =
                                    90.0 - (name_lower.len() - query_lower.len()) as f32 * 0.5;
                                results.insert(
                                    id.clone(),
                                    SearchResult {
                                        symbol: symbol.clone(),
                                        score,
                                        match_type: MatchType::Prefix,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }

        // 3. CamelCase略語マッチ
        let camel_matches = self.search_camel_case(query);
        for (id, symbol) in camel_matches {
            results.entry(id).or_insert(SearchResult {
                symbol,
                score: 85.0,
                match_type: MatchType::CamelCase,
            });
        }

        // 4. 部分文字列マッチ
        for entry in self.name_index.iter() {
            let name = entry.key();
            if name.contains(&query_lower) {
                for id in entry.value().iter() {
                    // すでに結果に含まれている場合はスキップ
                    if !results.contains_key(id) {
                        if let Some(symbol) = self.symbols.get(id) {
                            let position_penalty = name.find(&query_lower).unwrap_or(0) as f32;
                            let score = 70.0 - position_penalty * 0.5;
                            results.insert(
                                id.clone(),
                                SearchResult {
                                    symbol: symbol.clone(),
                                    score,
                                    match_type: MatchType::Substring,
                                },
                            );
                        }
                    }
                }
            }
        }

        // 5. N-gram ベースのファジーマッチ（短いクエリにも対応）
        let candidates = if query.len() >= 3 {
            self.search_by_trigrams(&query_lower)
        } else if query.len() >= 2 {
            self.search_by_bigrams(&query_lower)
        } else {
            // 1文字の場合は前方一致のみ
            HashMap::new()
        };

        for (id, score) in candidates {
            if let std::collections::hash_map::Entry::Vacant(e) = results.entry(id.clone()) {
                if let Some(symbol) = self.symbols.get(&id) {
                    e.insert(SearchResult {
                        symbol: symbol.clone(),
                        score: score * 60.0,
                        match_type: MatchType::Fuzzy,
                    });
                }
            }
        }

        // 6. レーベンシュタイン距離によるタイポ許容（クエリが3文字以上）
        if query.len() >= 3 {
            for entry in self.name_index.iter() {
                let name = entry.key();
                let distance = Self::levenshtein_distance(&query_lower, name);

                // 編集距離が文字列長の30%以下なら採用
                let max_len = query_lower.len().max(name.len());
                if distance <= (max_len as f32 * 0.3) as usize {
                    for id in entry.value().iter() {
                        // すでに結果に含まれている場合はスキップ
                        if !results.contains_key(id) {
                            if let Some(symbol) = self.symbols.get(id) {
                                let score = 50.0 * (1.0 - distance as f32 / max_len as f32);
                                results.insert(
                                    id.clone(),
                                    SearchResult {
                                        symbol: symbol.clone(),
                                        score,
                                        match_type: MatchType::Typo,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }

        // スコアでソートして上位N件を返す
        let mut results: Vec<SearchResult> = results.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap()
                .then_with(|| a.symbol.name.len().cmp(&b.symbol.name.len()))
        });
        results.truncate(max_results);
        results
    }

    /// CamelCase略語検索
    fn search_camel_case(&self, query: &str) -> HashMap<String, Symbol> {
        let mut results = HashMap::new();
        let query_upper = query.to_uppercase();

        // クエリが大文字のみまたは短い場合にCamelCase検索
        // 大文字と小文字が混在していても、大文字のみを抽出してマッチ
        for entry in self.symbols.iter() {
            let symbol = entry.value();
            let capitals = Self::extract_capitals(&symbol.name);

            // 大文字のマッチング
            if !capitals.is_empty()
                && (capitals == query_upper
                    || capitals.starts_with(&query_upper)
                    || capitals.contains(&query_upper))
            {
                results.insert(entry.key().clone(), symbol.clone());
            }
        }

        results
    }

    /// Bigramでの検索
    fn search_by_bigrams(&self, query: &str) -> HashMap<String, f32> {
        let query_bigrams = Self::generate_ngrams(query, 2);
        if query_bigrams.is_empty() {
            return HashMap::new();
        }

        let mut candidates: HashMap<String, f32> = HashMap::new();

        for bigram in &query_bigrams {
            if let Some(ids) = self.bigram_index.get(bigram) {
                for id in ids.iter() {
                    *candidates.entry(id.clone()).or_insert(0.0) += 1.0;
                }
            }
        }

        // スコアを正規化
        let max_score = query_bigrams.len() as f32;
        for score in candidates.values_mut() {
            *score /= max_score;
        }

        candidates.retain(|_, score| *score >= 0.3); // 30%以上マッチ
        candidates
    }

    /// Trigramでの検索
    fn search_by_trigrams(&self, query: &str) -> HashMap<String, f32> {
        let query_trigrams = Self::generate_ngrams(query, 3);
        if query_trigrams.is_empty() {
            return HashMap::new();
        }

        let mut candidates: HashMap<String, f32> = HashMap::new();

        for trigram in &query_trigrams {
            if let Some(ids) = self.trigram_index.get(trigram) {
                for id in ids.iter() {
                    *candidates.entry(id.clone()).or_insert(0.0) += 1.0;
                }
            }
        }

        // スコアを正規化
        let max_score = query_trigrams.len() as f32;
        for score in candidates.values_mut() {
            *score /= max_score;
        }

        candidates.retain(|_, score| *score >= 0.25); // 25%以上マッチ
        candidates
    }

    /// N-gramを生成
    fn generate_ngrams(text: &str, n: usize) -> HashSet<String> {
        let mut ngrams = HashSet::new();

        if text.len() < n {
            return ngrams;
        }

        // パディングを追加
        let padded = format!("{}{}{}", " ".repeat(n - 1), text, " ".repeat(n - 1));
        let chars: Vec<char> = padded.chars().collect();

        for i in 0..=chars.len() - n {
            let ngram: String = chars[i..i + n].iter().collect();
            ngrams.insert(ngram);
        }

        ngrams
    }

    /// CamelCaseを単語に分割
    fn split_camel_case(text: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut current = String::new();
        let mut prev_is_upper = false;

        for ch in text.chars() {
            if ch.is_uppercase() {
                if !current.is_empty() && !prev_is_upper {
                    words.push(current.clone());
                    current.clear();
                }
                current.push(ch);
                prev_is_upper = true;
            } else if ch.is_ascii_lowercase() {
                if prev_is_upper && current.len() > 1 {
                    // "HTTPServer" -> ["HTTP", "Server"]
                    let last = current.pop().unwrap();
                    if !current.is_empty() {
                        words.push(current.clone());
                    }
                    current = String::from(last);
                }
                current.push(ch);
                prev_is_upper = false;
            } else if ch.is_numeric() {
                if !current.is_empty() && !current.chars().last().unwrap().is_numeric() {
                    words.push(current.clone());
                    current.clear();
                }
                current.push(ch);
                prev_is_upper = false;
            } else {
                // 記号などで区切る
                if !current.is_empty() {
                    words.push(current.clone());
                    current.clear();
                }
                prev_is_upper = false;
            }
        }

        if !current.is_empty() {
            words.push(current);
        }

        words
    }

    /// 大文字のみを抽出
    fn extract_capitals(text: &str) -> String {
        text.chars().filter(|c| c.is_uppercase()).collect()
    }

    /// レーベンシュタイン距離を計算
    #[allow(clippy::needless_range_loop)]
    fn levenshtein_distance(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();

        if a_len == 0 {
            return b_len;
        }
        if b_len == 0 {
            return a_len;
        }

        let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

        for i in 0..=a_len {
            matrix[i][0] = i;
        }
        for j in 0..=b_len {
            matrix[0][j] = j;
        }

        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = (matrix[i - 1][j] + 1) // 削除
                    .min(matrix[i][j - 1] + 1) // 挿入
                    .min(matrix[i - 1][j - 1] + cost); // 置換
            }
        }

        matrix[a_len][b_len]
    }

    /// 統計情報を取得
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            total_symbols: self.symbols.len(),
            total_bigrams: self.bigram_index.len(),
            total_trigrams: self.trigram_index.len(),
            total_prefixes: self.prefix_index.len(),
            total_words: self.word_index.len(),
            avg_symbols_per_bigram: if self.bigram_index.is_empty() {
                0.0
            } else {
                let total: usize = self
                    .bigram_index
                    .iter()
                    .map(|entry| entry.value().len())
                    .sum();
                total as f32 / self.bigram_index.len() as f32
            },
            avg_symbols_per_trigram: if self.trigram_index.is_empty() {
                0.0
            } else {
                let total: usize = self
                    .trigram_index
                    .iter()
                    .map(|entry| entry.value().len())
                    .sum();
                total as f32 / self.trigram_index.len() as f32
            },
        }
    }
}

/// インデックスの統計情報
#[derive(Debug)]
pub struct IndexStats {
    pub total_symbols: usize,
    pub total_bigrams: usize,
    pub total_trigrams: usize,
    pub total_prefixes: usize,
    pub total_words: usize,
    pub avg_symbols_per_bigram: f32,
    pub avg_symbols_per_trigram: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range, SymbolKind};

    fn create_test_symbol(id: &str, name: &str) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: name.to_string(),
            kind: SymbolKind::Function,
            file_path: format!("test/{}.rs", name.to_lowercase()),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 0,
                },
            },
            documentation: None,
            detail: None,
        }
    }

    #[test]
    fn test_exact_match() {
        let index = FuzzySearchIndex::new();
        index.add_symbol(create_test_symbol("1", "getUserById"));
        index.add_symbol(create_test_symbol("2", "getUserByName"));

        let results = index.search("getuserbyid", 10);
        assert!(!results.is_empty(), "No results found");

        // 大文字小文字を区別しないので、高スコアでマッチするはず
        // 小文字化されたクエリは name_index でExactマッチする
        let get_user_match = results.iter().find(|r| r.symbol.name == "getUserById");
        assert!(get_user_match.is_some(), "getUserById not found in results");
        let matched = get_user_match.unwrap();

        // 大文字小文字が異なってもname_indexで一致するのでExactマッチ
        assert_eq!(
            matched.match_type,
            MatchType::Exact,
            "Should be exact match for case-insensitive search"
        );
        assert!(matched.score > 99.0, "Score should be 100 for exact match");
    }

    #[test]
    fn test_prefix_match() {
        let index = FuzzySearchIndex::new();
        index.add_symbol(create_test_symbol("1", "getUserById"));
        index.add_symbol(create_test_symbol("2", "setUserName"));

        let results = index.search("get", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].symbol.name, "getUserById");
        // 短いクエリなので前方一致として扱われる
        assert!(matches!(
            results[0].match_type,
            MatchType::Prefix | MatchType::Substring
        ));
    }

    #[test]
    fn test_camel_case_match() {
        let index = FuzzySearchIndex::new();
        index.add_symbol(create_test_symbol("1", "getUserById"));
        index.add_symbol(create_test_symbol("2", "HTTPServerConfig"));

        // getUserByIdの大文字を抽出すると"UBI"になる
        let results = index.search("UBI", 10);
        assert!(!results.is_empty(), "No results found for UBI");
        let ubi_match = results
            .iter()
            .find(|r| r.symbol.name == "getUserById")
            .expect("getUserById not found");
        assert_eq!(ubi_match.match_type, MatchType::CamelCase);

        // HTTPServerConfigの大文字は"HTTPSC"
        let results = index.search("HTTPSC", 10);
        assert!(!results.is_empty(), "No results found for HTTPSC");
        let hsc_match = results
            .iter()
            .find(|r| r.symbol.name == "HTTPServerConfig")
            .expect("HTTPServerConfig not found");
        assert_eq!(hsc_match.match_type, MatchType::CamelCase);
    }

    #[test]
    fn test_substring_match() {
        let index = FuzzySearchIndex::new();
        index.add_symbol(create_test_symbol("1", "calculateUserScore"));
        index.add_symbol(create_test_symbol("2", "getTotalAmount"));

        let results = index.search("User", 10);
        assert!(!results.is_empty());
        assert!(results
            .iter()
            .any(|r| r.symbol.name == "calculateUserScore"));
    }

    #[test]
    fn test_fuzzy_match_with_typo() {
        let index = FuzzySearchIndex::new();
        index.add_symbol(create_test_symbol("1", "getUserById"));

        // タイポ: "getUserByld" (Id -> ld)
        let results = index.search("getUserByld", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].symbol.name, "getUserById");
        assert!(matches!(
            results[0].match_type,
            MatchType::Typo | MatchType::Fuzzy
        ));
    }

    #[test]
    fn test_short_query() {
        let index = FuzzySearchIndex::new();
        index.add_symbol(create_test_symbol("1", "getUserById"));
        index.add_symbol(create_test_symbol("2", "getId"));
        index.add_symbol(create_test_symbol("3", "ID_CONSTANT"));

        // 2文字のクエリ
        let results = index.search("id", 10);
        assert!(results.len() >= 2);

        // 1文字のクエリ（前方一致のみ）
        let results = index.search("g", 10);
        assert!(results
            .iter()
            .any(|r| r.symbol.name.to_lowercase().starts_with("g")));
    }

    #[test]
    fn test_split_camel_case() {
        assert_eq!(
            FuzzySearchIndex::split_camel_case("getUserById"),
            vec!["get", "User", "By", "Id"]
        );
        assert_eq!(
            FuzzySearchIndex::split_camel_case("HTTPServerConfig"),
            vec!["HTTP", "Server", "Config"]
        );
        assert_eq!(
            FuzzySearchIndex::split_camel_case("calculateSum123"),
            vec!["calculate", "Sum", "123"]
        );
    }

    #[test]
    fn test_japanese_support() {
        let index = FuzzySearchIndex::new();
        index.add_symbol(create_test_symbol("1", "ユーザー取得"));
        index.add_symbol(create_test_symbol("2", "データ保存"));

        let results = index.search("ユーザー", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].symbol.name, "ユーザー取得");
    }

    #[test]
    fn test_ranking_order() {
        let index = FuzzySearchIndex::new();
        index.add_symbol(create_test_symbol("1", "getUserById")); // Exact
        index.add_symbol(create_test_symbol("2", "getUserByName")); // Prefix
        index.add_symbol(create_test_symbol("3", "updateUserById")); // Substring
        index.add_symbol(create_test_symbol("4", "fetchUserData")); // Fuzzy

        let results = index.search("getUserById", 10);

        // 少なくとも一つの結果があることを確認
        assert!(!results.is_empty());

        // getUserByIdが結果に含まれることを確認（完全一致または他の高スコアマッチ）
        let _get_user_by_id = results
            .iter()
            .find(|r| r.symbol.name == "getUserById")
            .unwrap();

        // スコアが降順であることを確認
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }
    }
}
