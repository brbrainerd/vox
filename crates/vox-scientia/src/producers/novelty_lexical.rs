//! Lexical (shingle-based) similarity scoring for finding-candidate dedup,
//! reusing the same 4-gram character shingling approach as
//! `vox_search::novelty::NoveltyScorer`, applied here across the full
//! history of prior findings rather than a single session.

use std::collections::HashSet;

fn fnv1a(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in s.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn shingle_set(content: &str, n: usize) -> HashSet<u64> {
    let chars: Vec<char> = content.chars().collect();
    if chars.len() < n {
        return [fnv1a(content)].into_iter().collect();
    }
    chars
        .windows(n)
        .map(|w| fnv1a(&w.iter().collect::<String>()))
        .collect()
}

/// Jaccard similarity between two texts' 4-gram character shingle sets.
/// 1.0 = identical shingle sets, 0.0 = no overlap.
pub fn lexical_similarity(a: &str, b: &str) -> f64 {
    let sa = shingle_set(a, 4);
    let sb = shingle_set(b, 4);
    if sa.is_empty() && sb.is_empty() {
        return 1.0;
    }
    let intersection = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_scores_one() {
        assert_eq!(lexical_similarity("the same text here", "the same text here"), 1.0);
    }

    #[test]
    fn completely_different_text_scores_low() {
        let sim = lexical_similarity(
            "quantum entanglement in superconducting circuits",
            "sourdough bread fermentation temperature control",
        );
        assert!(sim < 0.1, "expected low similarity, got {sim}");
    }

    #[test]
    fn near_restatement_scores_high() {
        let sim = lexical_similarity(
            "the confidence gate fuses citation and claim support scores",
            "the confidence gate fuses citation and claim-support scores",
        );
        assert!(sim > 0.7, "expected high similarity for near-restatement, got {sim}");
    }

    #[test]
    fn empty_strings_score_one() {
        assert_eq!(lexical_similarity("", ""), 1.0);
    }
}
