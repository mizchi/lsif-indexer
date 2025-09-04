//! Fuzzy search functionality

use anyhow::Result;
use dashmap::DashMap;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use lsif_core::{CodeGraph, Symbol, SymbolKind};
use std::collections::HashSet;
use std::sync::Arc;

/// Match type for fuzzy search results
#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    Exact,
    Prefix,
    Substring,
    CamelCase,
    Fuzzy,
    Typo,
}

/// Fuzzy search match result
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub symbol: Symbol,
    pub score: f32,
    pub match_type: MatchType,
    pub matched_indices: Vec<usize>,
}

/// Fuzzy searcher with multiple strategies
pub struct FuzzySearcher {
    graph: CodeGraph,
    /// Symbol index: symbol ID -> Symbol
    symbols: Arc<DashMap<String, Symbol>>,
    /// Name index: lowercase name -> symbol IDs
    name_index: Arc<DashMap<String, HashSet<String>>>,
    /// Bigram index: 2-gram -> symbol IDs
    bigram_index: Arc<DashMap<String, HashSet<String>>>,
    /// Trigram index: 3-gram -> symbol IDs  
    trigram_index: Arc<DashMap<String, HashSet<String>>>,
    /// Prefix index: prefix (max 5 chars) -> symbol IDs
    prefix_index: Arc<DashMap<String, HashSet<String>>>,
    /// CamelCase word index: word -> symbol IDs
    word_index: Arc<DashMap<String, HashSet<String>>>,
    /// Fuzzy matcher
    matcher: SkimMatcherV2,
}

impl FuzzySearcher {
    /// Create a new fuzzy searcher
    pub fn new(graph: CodeGraph) -> Self {
        let mut searcher = Self {
            graph: graph.clone(),
            symbols: Arc::new(DashMap::new()),
            name_index: Arc::new(DashMap::new()),
            bigram_index: Arc::new(DashMap::new()),
            trigram_index: Arc::new(DashMap::new()),
            prefix_index: Arc::new(DashMap::new()),
            word_index: Arc::new(DashMap::new()),
            matcher: SkimMatcherV2::default(),
        };

        searcher.build_indices();
        searcher
    }

    /// Build all indices from the graph
    fn build_indices(&mut self) {
        for symbol in self.graph.get_all_symbols() {
            self.add_symbol(symbol.clone());
        }
    }

    /// Add a symbol to the indices
    pub fn add_symbol(&self, symbol: Symbol) {
        let id = symbol.id.clone();
        let name = symbol.name.clone();

        // Store symbol
        self.symbols.insert(id.clone(), symbol);

        // Name index (case-insensitive)
        let name_lower = name.to_lowercase();
        self.name_index
            .entry(name_lower.clone())
            .or_default()
            .insert(id.clone());

        // N-gram indices
        for bigram in Self::generate_ngrams(&name_lower, 2) {
            self.bigram_index
                .entry(bigram)
                .or_default()
                .insert(id.clone());
        }

        for trigram in Self::generate_ngrams(&name_lower, 3) {
            self.trigram_index
                .entry(trigram)
                .or_default()
                .insert(id.clone());
        }

        // Prefix index
        for i in 1..=5.min(name_lower.len()) {
            let prefix = name_lower[..i].to_string();
            self.prefix_index
                .entry(prefix)
                .or_default()
                .insert(id.clone());
        }

        // CamelCase word index
        for word in Self::split_camel_case(&name) {
            self.word_index
                .entry(word.to_lowercase())
                .or_default()
                .insert(id.clone());
        }
    }

    /// Search for symbols matching the query
    pub fn search(&self, query: &str, limit: Option<usize>) -> Result<Vec<FuzzyMatch>> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        // 1. Exact match
        if let Some(exact_matches) = self.name_index.get(&query_lower) {
            for id in exact_matches.iter() {
                if let Some(symbol) = self.symbols.get(id) {
                    results.push(FuzzyMatch {
                        symbol: symbol.clone(),
                        score: 100.0,
                        match_type: MatchType::Exact,
                        matched_indices: vec![],
                    });
                }
            }
        }

        // 2. Prefix match
        if let Some(prefix_matches) = self.prefix_index.get(&query_lower) {
            for id in prefix_matches.iter() {
                if let Some(symbol) = self.symbols.get(id) {
                    if !results.iter().any(|r| r.symbol.id == *id) {
                        results.push(FuzzyMatch {
                            symbol: symbol.clone(),
                            score: 80.0,
                            match_type: MatchType::Prefix,
                            matched_indices: vec![],
                        });
                    }
                }
            }
        }

        // 3. CamelCase match
        let query_words: Vec<String> = Self::split_camel_case(query)
            .into_iter()
            .map(|s| s.to_lowercase())
            .collect();

        let mut camel_candidates = HashSet::new();
        for word in &query_words {
            if let Some(word_matches) = self.word_index.get(word) {
                for id in word_matches.iter() {
                    camel_candidates.insert(id.clone());
                }
            }
        }

        for id in camel_candidates {
            if let Some(symbol) = self.symbols.get(&id) {
                if !results.iter().any(|r| r.symbol.id == id) {
                    let symbol_words: HashSet<String> = Self::split_camel_case(&symbol.name)
                        .into_iter()
                        .map(|s| s.to_lowercase())
                        .collect();

                    if query_words.iter().all(|w| symbol_words.contains(w)) {
                        results.push(FuzzyMatch {
                            symbol: symbol.clone(),
                            score: 70.0,
                            match_type: MatchType::CamelCase,
                            matched_indices: vec![],
                        });
                    }
                }
            }
        }

        // 4. Fuzzy match using fuzzy_matcher
        if results.len() < limit.unwrap_or(50) {
            for entry in self.symbols.iter() {
                let id = entry.key();
                let symbol = entry.value();

                if !results.iter().any(|r| r.symbol.id == *id) {
                    if let Some((score, indices)) = self.matcher.fuzzy_indices(&symbol.name, query)
                    {
                        if score > 30 {
                            results.push(FuzzyMatch {
                                symbol: symbol.clone(),
                                score: score as f32,
                                match_type: MatchType::Fuzzy,
                                matched_indices: indices,
                            });
                        }
                    }
                }

                if let Some(limit) = limit {
                    if results.len() >= limit * 2 {
                        break;
                    }
                }
            }
        }

        // Sort by score
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Apply limit
        if let Some(limit) = limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Search with specific symbol kind filter
    pub fn search_by_kind(
        &self,
        query: &str,
        kind: SymbolKind,
        limit: Option<usize>,
    ) -> Result<Vec<FuzzyMatch>> {
        let all_results = self.search(query, None)?;
        let mut filtered: Vec<FuzzyMatch> = all_results
            .into_iter()
            .filter(|m| m.symbol.kind == kind)
            .collect();

        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }

    /// Generate n-grams from a string
    fn generate_ngrams(text: &str, n: usize) -> Vec<String> {
        if text.len() < n {
            return vec![];
        }

        let chars: Vec<char> = text.chars().collect();
        let mut ngrams = Vec::new();

        for i in 0..=chars.len() - n {
            let ngram: String = chars[i..i + n].iter().collect();
            ngrams.push(ngram);
        }

        ngrams
    }

    /// Split CamelCase string into words
    fn split_camel_case(text: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut current = String::new();
        let mut prev_upper = false;

        for ch in text.chars() {
            if ch.is_uppercase() {
                if !current.is_empty() && !prev_upper {
                    words.push(current.clone());
                    current.clear();
                }
                prev_upper = true;
            } else {
                prev_upper = false;
            }
            current.push(ch);
        }

        if !current.is_empty() {
            words.push(current);
        }

        // Also include the full text as a word
        if words.len() > 1 {
            words.push(text.to_string());
        }

        words
    }

    /// Calculate Levenshtein distance between two strings
    pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();
        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        for (i, c1) in s1.chars().enumerate() {
            for (j, c2) in s2.chars().enumerate() {
                let cost = if c1 == c2 { 0 } else { 1 };
                matrix[i + 1][j + 1] = std::cmp::min(
                    std::cmp::min(matrix[i][j + 1] + 1, matrix[i + 1][j] + 1),
                    matrix[i][j] + cost,
                );
            }
        }

        matrix[len1][len2]
    }
}
