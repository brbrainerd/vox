//! Three-tier routing: keyword rules → Jaccard word similarity → LLM (high-value only).

use crate::knowledge_base::types::{KbRoutingRule, KbRoutingRuleType};

/// Returns `true` if `content` matches the routing rule's pattern.
pub fn keyword_rule_matches(content: &str, rule: &KbRoutingRule) -> bool {
    let content_lower = content.to_ascii_lowercase();
    let pattern_lower = rule.pattern.to_ascii_lowercase();
    match rule.rule_type {
        // Regex: fall back to substring match for the MVP (avoids the `regex` dep).
        // A future enhancement can compile regex patterns explicitly.
        KbRoutingRuleType::Keyword | KbRoutingRuleType::Regex => {
            content_lower.contains(&pattern_lower)
        }
    }
}

/// Jaccard word-set similarity between two strings (tokenized by whitespace).
/// Returns a value in `[0.0, 1.0]`. Both empty → 0.0.
pub fn jaccard_word_similarity(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;
    let a_words: HashSet<&str> = a.split_whitespace().collect();
    let b_words: HashSet<&str> = b.split_whitespace().collect();
    if a_words.is_empty() && b_words.is_empty() {
        return 0.0;
    }
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

/// Apply keyword/regex rules to content.
///
/// Rules are sorted by priority (highest first). One match per KB is emitted
/// (the first matching rule for each KB wins). Returns `(kb_id, confidence=1.0)` pairs,
/// sorted by confidence descending.
pub fn apply_keyword_rules(content: &str, rules: &[KbRoutingRule]) -> Vec<(String, f64)> {
    let mut sorted = rules.to_vec();
    sorted.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut matched: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for rule in &sorted {
        if !matched.contains_key(&rule.kb_id) && keyword_rule_matches(content, rule) {
            matched.insert(rule.kb_id.clone(), 1.0);
        }
    }

    let mut result: Vec<(String, f64)> = matched.into_iter().collect();
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    result
}

/// Apply Jaccard similarity against each KB's sample entries.
///
/// Returns KBs where average Jaccard similarity across samples exceeds `threshold`.
/// `kb_samples` is `(kb_id, sample_entry_contents)` pairs.
/// Results are sorted by score descending.
pub fn apply_similarity_routing(
    content: &str,
    kb_samples: &[(String, Vec<String>)],
    threshold: f64,
) -> Vec<(String, f64)> {
    let mut result = Vec::new();
    for (kb_id, samples) in kb_samples {
        if samples.is_empty() {
            continue;
        }
        let scores: Vec<f64> = samples
            .iter()
            .map(|s| jaccard_word_similarity(content, s))
            .collect();
        let avg = scores.iter().sum::<f64>() / scores.len() as f64;
        if avg >= threshold {
            result.push((kb_id.clone(), avg));
        }
    }
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    result
}

/// Minimum Jaccard score to route an item to a KB via similarity.
/// Below this threshold, tier 2 routing passes and tier 3 (LLM) applies.
pub const SIMILARITY_THRESHOLD: f64 = 0.15;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_base::types::{KbRoutingRule, KbRoutingRuleType};

    fn kw_rule(kb_id: &str, pattern: &str, priority: i64) -> KbRoutingRule {
        KbRoutingRule {
            id: "r".to_string(),
            kb_id: kb_id.to_string(),
            rule_type: KbRoutingRuleType::Keyword,
            pattern: pattern.to_string(),
            priority,
            created_at_ms: 0,
        }
    }

    #[test]
    fn keyword_rule_matches_case_insensitively() {
        let rule = kw_rule("kb1", "brown", 0);
        assert!(keyword_rule_matches("The quick BROWN fox", &rule));
    }

    #[test]
    fn keyword_rule_no_match() {
        let rule = kw_rule("kb1", "qdrant", 0);
        assert!(!keyword_rule_matches("The quick brown fox", &rule));
    }

    #[test]
    fn jaccard_identical() {
        assert!((jaccard_word_similarity("rust async tokio", "rust async tokio") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_no_overlap() {
        assert_eq!(jaccard_word_similarity("rust tokio", "python django"), 0.0);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let s = jaccard_word_similarity("rust async tokio", "rust sync blocking");
        assert!(s > 0.0 && s < 1.0, "score={s}");
    }

    #[test]
    fn apply_keyword_rules_returns_matching_kb() {
        let rules = vec![
            kw_rule("kb_retrieval", "qdrant", 10),
            kw_rule("kb_rust", "tokio", 5),
        ];
        let matches = apply_keyword_rules("using Qdrant for vector search", &rules);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "kb_retrieval");
        assert!((matches[0].1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn apply_keyword_rules_priority_ordering() {
        let rules = vec![
            kw_rule("kb_low", "the", 0),
            kw_rule("kb_high", "quick", 10),
        ];
        let matches = apply_keyword_rules("the quick brown fox", &rules);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn similarity_above_threshold_routes() {
        let samples = vec![
            ("kb_rust".to_string(), vec!["tokio async runtime".to_string()]),
        ];
        let results = apply_similarity_routing("tokio async executor", &samples, 0.15);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "kb_rust");
    }

    #[test]
    fn similarity_below_threshold_no_route() {
        let samples = vec![
            ("kb_rust".to_string(), vec!["python django flask".to_string()]),
        ];
        let results = apply_similarity_routing("tokio async runtime", &samples, 0.15);
        assert!(results.is_empty());
    }
}
