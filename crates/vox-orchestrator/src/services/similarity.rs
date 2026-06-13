//! Token-set similarity for near-duplicate task detection.
//!
//! Deliberately cheap (no embeddings, no model calls): lowercased alphanumeric
//! token sets + Jaccard. Good enough to catch "the user typed the same ask
//! twice" and "two tabs filed the same bug"; the GUI mediates anything fuzzier.

use std::collections::HashSet;

/// Threshold above which two task descriptions are treated as near-duplicates.
pub const NEAR_DUPLICATE_THRESHOLD: f64 = 0.85;

fn token_set(s: &str) -> HashSet<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// Jaccard similarity of the two descriptions' token sets, in [0, 1].
pub fn jaccard(a: &str, b: &str) -> f64 {
    let sa = token_set(a);
    let sb = token_set(b);
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    inter / union
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_descriptions_score_one() {
        assert!((jaccard("fix the flaky auth test", "fix the flaky auth test") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn disjoint_descriptions_score_zero() {
        assert_eq!(
            jaccard("refactor mesh dispatch", "write release notes"),
            0.0
        );
    }

    #[test]
    fn near_duplicates_score_high() {
        let a = "Fix the flaky auth test in vox-gui";
        let b = "fix flaky auth test in vox-gui please";
        assert!(jaccard(a, b) > 0.6, "got {}", jaccard(a, b));
    }

    #[test]
    fn case_and_punctuation_are_normalized() {
        assert!((jaccard("Add CI gate!", "add ci gate") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn empty_inputs_score_zero() {
        assert_eq!(jaccard("", "anything"), 0.0);
        assert_eq!(jaccard("", ""), 0.0);
    }
}
