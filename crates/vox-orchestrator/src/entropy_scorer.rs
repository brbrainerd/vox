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

#[cfg(test)]
mod semcov_wave1_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn semantic_drift_sigma_is_zero_when_text_matches_baseline() {
        // baseline == entropy of the same text => delta 0 => sigma 0
        let text = "hello world";
        let baseline = calculate_entropy(text);
        let sigma = semantic_drift_sigma(text, baseline);
        // approximately zero: entropy accumulation order (HashMap) yields tiny FP noise
        assert!(sigma.abs() < 1e-9, "expected ~0 sigma, got {sigma}");
    }

    #[test]
    fn semantic_drift_sigma_divides_delta_by_exactly_point75() {
        // Divisor is 0.75_f64.max(EPSILON) == 0.75. With baseline 0.0,
        // sigma == |h - 0| / 0.75 == h / 0.75.
        let text = "abcd"; // 4 distinct chars, uniform => entropy == 2.0 bits
        let h = calculate_entropy(text);
        assert_eq!(h, 2.0, "precondition h={h}");
        let sigma = semantic_drift_sigma(text, 0.0);
        let expected = 2.0_f64 / 0.75_f64;
        assert!(
            (sigma - expected).abs() < 1e-12,
            "sigma={sigma} expected={expected}"
        );
    }
}
