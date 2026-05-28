use std::collections::HashMap;

/// Calculates Shannon entropy of a string's character distribution.
///
/// High entropy suggests high variance (potential hallucination).
/// Very low entropy suggests repetition (stochastic parrot loop).
pub fn calculate_entropy(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }

    let mut counts = HashMap::new();
    let mut total = 0.0;
    for c in text.chars() {
        *counts.entry(c).or_insert(0.0) += 1.0;
        total += 1.0;
    }

    let mut entropy = 0.0;
    for count in counts.values() {
        let p: f64 = count / total;
        entropy -= p * p.log2();
    }

    entropy
}

/// Scores the confidence of a model output based on lexical diversity and entropy.
pub fn score_confidence(text: &str) -> f64 {
    let entropy = calculate_entropy(text);

    // Heuristic: for most natural language, entropy is between 3.0 and 5.0.
    // Below 2.0 is likely repetitive garbage.
    // Above 6.0 might be high-variance hallucination (random chars).

    if entropy < 1.5 {
        return 0.1; // Extremely repetitive
    }

    if entropy > 7.0 {
        return 0.2; // Likely random noise
    }

    // Map [1.5, 4.5] to [0.1, 1.0] and [4.5, 7.0] to [1.0, 0.2]
    if entropy <= 4.5 {
        0.1 + (entropy - 1.5) * (0.9 / 3.0)
    } else {
        1.0 - (entropy - 4.5) * (0.8 / 2.5)
    }
}

#[must_use]
pub fn semantic_drift_sigma(completion_text: &str, session_baseline_entropy: f64) -> f64 {
    let h = calculate_entropy(completion_text);
    let delta = (h - session_baseline_entropy).abs();
    delta / 0.75_f64.max(f64::EPSILON)
}

#[cfg(test)]
mod drift_tests {
    use super::*;

    #[test]
    fn semantic_drift_sigma_spikes_on_repetition() {
        let baseline = calculate_entropy("The quick brown fox jumps over the lazy dog.");
        let sigma = semantic_drift_sigma(&"a".repeat(400), baseline);
        assert!(sigma >= 2.0, "sigma={sigma}");
    }
}
