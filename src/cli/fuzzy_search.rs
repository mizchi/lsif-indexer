/// 曖昧検索の実装例
/// 
/// 1. 部分文字列マッチング（大文字小文字無視）
/// 2. 前方一致
/// 3. Fuzzy matching（文字の順序を保持）
/// 4. 略語マッチング（大文字のみ抽出: RP -> RelationshipPattern）

use crate::core::Symbol;

/// 曖昧検索のスコアと結果
#[derive(Debug, Clone)]
pub struct FuzzyMatch<'a> {
    pub symbol: &'a Symbol,
    pub score: f32,  // 0.0 ~ 1.0 (1.0が完全一致)
}

/// 曖昧検索の実装
pub fn fuzzy_search<'a>(query: &str, symbols: &'a [Symbol]) -> Vec<FuzzyMatch<'a>> {
    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for symbol in symbols {
        let name_lower = symbol.name.to_lowercase();
        
        // 1. 完全一致
        if name_lower == query_lower {
            matches.push(FuzzyMatch {
                symbol,
                score: 1.0,
            });
            continue;
        }

        // 2. 前方一致
        if name_lower.starts_with(&query_lower) {
            matches.push(FuzzyMatch {
                symbol,
                score: 0.9,
            });
            continue;
        }

        // 3. 部分文字列マッチング
        if name_lower.contains(&query_lower) {
            matches.push(FuzzyMatch {
                symbol,
                score: 0.7,
            });
            continue;
        }

        // 4. Fuzzy matching（文字の順序を保持）
        if fuzzy_match(&query_lower, &name_lower) {
            matches.push(FuzzyMatch {
                symbol,
                score: 0.5,
            });
            continue;
        }

        // 5. 略語マッチング（大文字のみ）
        if abbreviation_match(query, &symbol.name) {
            matches.push(FuzzyMatch {
                symbol,
                score: 0.6,
            });
        }
    }

    // スコア順にソート
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
}