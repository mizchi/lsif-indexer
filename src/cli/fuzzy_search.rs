/// 曖昧検索の実装例
/// 
/// 1. 部分文字列マッチング（大文字小文字無視）
/// 2. 前方一致
/// 3. Fuzzy matching（文字の順序を保持）
/// 4. 略語マッチング（大文字のみ抽出: RP -> RelationshipPattern）

use crate::core::Symbol;

/// 汎用的な曖昧検索の結果
#[derive(Debug, Clone, PartialEq)]
pub struct StringMatch<'a> {
    pub text: &'a str,
    pub score: f32,  // 0.0 ~ 1.0 (1.0が完全一致)
    pub index: usize,  // 元のリストでのインデックス
}

/// シンボル用の曖昧検索結果
#[derive(Debug, Clone)]
pub struct FuzzyMatch<'a> {
    pub symbol: &'a Symbol,
    pub score: f32,  // 0.0 ~ 1.0 (1.0が完全一致)
}

/// 汎用的な文字列の曖昧検索
pub fn fuzzy_search_strings<'a>(query: &str, texts: &'a [&str]) -> Vec<StringMatch<'a>> {
    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for (index, text) in texts.iter().enumerate() {
        let text_lower = text.to_lowercase();
        
        // 1. 完全一致
        if text_lower == query_lower {
            matches.push(StringMatch {
                text,
                score: 1.0,
                index,
            });
            continue;
        }

        // 2. 前方一致
        if text_lower.starts_with(&query_lower) {
            matches.push(StringMatch {
                text,
                score: 0.9,
                index,
            });
            continue;
        }

        // 3. 部分文字列マッチング
        if text_lower.contains(&query_lower) {
            matches.push(StringMatch {
                text,
                score: 0.7,
                index,
            });
            continue;
        }

        // 4. Fuzzy matching（文字の順序を保持）
        if fuzzy_match(&query_lower, &text_lower) {
            matches.push(StringMatch {
                text,
                score: 0.5,
                index,
            });
            continue;
        }

        // 5. 略語マッチング（大文字のみ）
        if abbreviation_match(query, text) {
            matches.push(StringMatch {
                text,
                score: 0.6,
                index,
            });
        }
    }

    // スコア順にソート
    matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    matches
}

/// シンボルの曖昧検索（Symbol構造体用のラッパー）
pub fn fuzzy_search<'a>(query: &str, symbols: &'a [Symbol]) -> Vec<FuzzyMatch<'a>> {
    // シンボル名を文字列のスライスに変換
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    
    // 汎用関数で検索
    let string_matches = fuzzy_search_strings(query, &names);
    
    // 結果をFuzzyMatchに変換
    string_matches
        .into_iter()
        .map(|m| FuzzyMatch {
            symbol: &symbols[m.index],
            score: m.score,
        })
        .collect()
}

/// ファイルパスの曖昧検索（パス専用の特別な処理付き）
pub fn fuzzy_search_paths<'a>(query: &str, paths: &'a [&str]) -> Vec<StringMatch<'a>> {
    let mut matches = fuzzy_search_strings(query, paths);
    
    // パス用の追加処理：ファイル名のみでもマッチ
    let query_lower = query.to_lowercase();
    for (index, path) in paths.iter().enumerate() {
        if let Some(filename) = path.split('/').last() {
            let filename_lower = filename.to_lowercase();
            if filename_lower.contains(&query_lower) && !matches.iter().any(|m| m.index == index) {
                matches.push(StringMatch {
                    text: path,
                    score: 0.65,  // ファイル名マッチは中程度のスコア
                    index,
                });
            }
        }
    }
    
    matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    matches
}

/// Fuzzy matching: クエリの文字が順番通りに含まれているか
/// 例: "rp" は "RelationshipPattern" にマッチ
fn fuzzy_match(query: &str, target: &str) -> bool {
    let mut query_chars = query.chars();
    let mut current_char = query_chars.next();

    for target_char in target.chars() {
        if let Some(qc) = current_char {
            if qc == target_char {
                current_char = query_chars.next();
            }
        } else {
            return true; // すべての文字がマッチ
        }
    }

    current_char.is_none()
}

/// 略語マッチング: 大文字のみでマッチ
/// 例: "RP" は "RelationshipPattern" にマッチ
fn abbreviation_match(query: &str, target: &str) -> bool {
    let query_upper = query.to_uppercase();
    let capitals: String = target.chars()
        .filter(|c| c.is_uppercase())
        .collect();
    
    capitals == query_upper || fuzzy_match(&query_upper, &capitals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match() {
        assert!(fuzzy_match("rp", "relationshippattern"));
        assert!(fuzzy_match("qe", "queryengine"));
        assert!(!fuzzy_match("xyz", "abcdef"));
    }

    #[test]
    fn test_abbreviation_match() {
        assert!(abbreviation_match("RP", "RelationshipPattern"));
        assert!(abbreviation_match("QE", "QueryEngine"));
        assert!(!abbreviation_match("XY", "RelationshipPattern"));
    }
    
    #[test]
    fn test_fuzzy_search_strings() {
        let texts = vec![
            "RelationshipPattern",
            "QueryEngine",
            "TypeRelations",
            "JsonRpcRequest",
        ];
        
        let results = fuzzy_search_strings("rp", &texts);
        assert!(!results.is_empty());
        
        // JsonRpcRequestが最初に来るはず（部分文字列マッチ）
        assert_eq!(results[0].text, "JsonRpcRequest");
        assert_eq!(results[0].score, 0.7);
    }
    
    #[test]
    fn test_fuzzy_search_paths() {
        let paths = vec![
            "src/core/graph.rs",
            "src/cli/fuzzy_search.rs",
            "tests/test_fuzzy.rs",
        ];
        
        let results = fuzzy_search_paths("fuzzy", &paths);
        assert_eq!(results.len(), 2);
        // fuzzy_search.rsが最初（ファイル名で部分一致）
        assert!(results[0].text.contains("fuzzy_search.rs"));
    }
}