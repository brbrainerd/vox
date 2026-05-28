//! Symbolic verifiers for numeric directional claims and Bayesian sequential stopping.
//!
//! # [`NumericComparatorVerifier`]
//! Verifies claims of the form "X increased/decreased/rose/fell" by comparing
//! the sign of `(measured_value - baseline_value)` against the direction keyword.
//! No LLM is involved — this is the AlphaEvolve lesson applied: if the ground truth
//! is arithmetic, verify arithmetically.
//!
//! # [`BayesianStoppingRule`]
//! Implements pre-declared sequential stopping per SCIENTIA plan §5.2.
//! The stopping threshold is read from [`StopRule::threshold`]; campaigns stop
//! as soon as the posterior crosses the threshold (accept) or its complement (reject).

use vox_research_events::preregistration::StopRule;

/// Verdict from the [`NumericComparatorVerifier`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolicVerdict {
    /// The measured direction matches the claimed direction.
    Confirmed,
    /// The measured direction contradicts the claimed direction.
    Refuted,
    /// Direction cannot be determined from the claim text, or measured == baseline.
    Inconclusive,
}

/// Decision from the [`BayesianStoppingRule`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopDecision {
    /// Posterior has not crossed either boundary — collect more samples.
    Continue,
    /// Posterior >= threshold — stop and accept the hypothesis.
    StopAccept,
    /// Posterior <= (1 - threshold) — stop and reject the hypothesis.
    StopReject,
}

/// Verifies numeric directional claims symbolically (no LLM).
#[derive(Debug, Default, Clone)]
pub struct NumericComparatorVerifier;

impl NumericComparatorVerifier {
    pub fn new() -> Self {
        Self
    }

    /// Verify `claim_text` against `(measured_value, baseline_value)`.
    ///
    /// Extracts the direction keyword from the claim text, then checks whether
    /// `(measured_value - baseline_value)` has the correct sign.
    pub fn verify(
        &self,
        claim_text: &str,
        measured_value: f64,
        baseline_value: f64,
    ) -> SymbolicVerdict {
        let lower = claim_text.to_ascii_lowercase();

        let upward_keywords = ["increased", "rose", "risen", "grew", "higher"];
        let downward_keywords = ["decreased", "fell", "fallen", "dropped", "lower", "reduced"];

        // Use word-boundary matching: keyword must be surrounded by non-alphanumeric chars
        // and non-underscore, or at start/end of string to avoid substring collisions
        // (e.g. "up" in "update", "fell" in "value_fell_2").
        let word_match = |text: &str, kw: &str| -> bool {
            let mut start = 0;
            while let Some(pos) = text[start..].find(kw) {
                let abs_pos = start + pos;
                let before_ok = abs_pos == 0 || {
                    let b = text.as_bytes()[abs_pos - 1];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };
                let after_pos = abs_pos + kw.len();
                let after_ok = after_pos >= text.len() || {
                    let b = text.as_bytes()[after_pos];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };
                if before_ok && after_ok {
                    return true;
                }
                start = abs_pos + 1;
            }
            false
        };

        let claims_increase = upward_keywords.iter().any(|kw| word_match(&lower, kw));
        let claims_decrease = downward_keywords.iter().any(|kw| word_match(&lower, kw));

        // If the claim has no clear direction, it is inconclusive
        if !claims_increase && !claims_decrease {
            return SymbolicVerdict::Inconclusive;
        }

        let diff = measured_value - baseline_value;

        // Zero difference: inconclusive even if direction keyword is present
        if diff == 0.0 {
            return SymbolicVerdict::Inconclusive;
        }

        let measured_increase = diff > 0.0;

        if (claims_increase && measured_increase) || (claims_decrease && !measured_increase) {
            SymbolicVerdict::Confirmed
        } else {
            SymbolicVerdict::Refuted
        }
    }
}

/// Implements Bayesian sequential stopping per a pre-declared [`StopRule`].
#[derive(Debug, Default, Clone)]
pub struct BayesianStoppingRule;

const DEFAULT_POSTERIOR_THRESHOLD: f64 = 0.95;

impl BayesianStoppingRule {
    pub fn new() -> Self {
        Self
    }

    /// Determine whether to stop based on `posterior` and the stopping rule.
    ///
    /// - `posterior >= threshold` → [`StopDecision::StopAccept`]
    /// - `posterior <= (1.0 - threshold)` → [`StopDecision::StopReject`]
    /// - otherwise → [`StopDecision::Continue`]
    ///
    /// If `stop_rule.threshold` is `None`, the default threshold of 0.95 is used.
    pub fn should_stop(&self, posterior: f64, stop_rule: &StopRule) -> StopDecision {
        let threshold = stop_rule.threshold.unwrap_or(DEFAULT_POSTERIOR_THRESHOLD);
        if posterior >= threshold {
            StopDecision::StopAccept
        } else if posterior <= (1.0 - threshold) {
            StopDecision::StopReject
        } else {
            StopDecision::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_research_events::preregistration::StopRule;

    fn stop_rule(threshold: f64) -> StopRule {
        StopRule {
            max_n: 1000,
            alpha: None,
            threshold: Some(threshold),
        }
    }

    #[test]
    fn increased_claim_confirmed_when_measured_higher() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("p95 latency increased by 15ms", 215.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Confirmed);
    }

    #[test]
    fn increased_claim_refuted_when_measured_lower() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("p95 latency increased by 15ms", 185.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Refuted);
    }

    #[test]
    fn decreased_claim_confirmed_when_measured_lower() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("refusal rate decreased after update", 1.5, 3.0);
        assert_eq!(verdict, SymbolicVerdict::Confirmed);
    }

    #[test]
    fn decreased_claim_refuted_when_measured_higher() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("refusal rate decreased after update", 4.0, 3.0);
        assert_eq!(verdict, SymbolicVerdict::Refuted);
    }

    #[test]
    fn no_direction_keyword_is_inconclusive() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("p95 latency changed significantly", 210.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Inconclusive);
    }

    #[test]
    fn equal_values_are_inconclusive_even_with_direction() {
        let verifier = NumericComparatorVerifier::new();
        let verdict = verifier.verify("latency rose after update", 200.0, 200.0);
        assert_eq!(verdict, SymbolicVerdict::Inconclusive);
    }

    #[test]
    fn high_posterior_stops_accept() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.97, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::StopAccept);
    }

    #[test]
    fn low_posterior_stops_reject() {
        let rule = BayesianStoppingRule::new();
        // posterior = 0.02 → below (1 - 0.95) = 0.05 → StopReject
        let decision = rule.should_stop(0.02, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::StopReject);
    }

    #[test]
    fn mid_posterior_continues() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.50, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::Continue);
    }

    #[test]
    fn boundary_at_exactly_threshold_stops_accept() {
        let rule = BayesianStoppingRule::new();
        let decision = rule.should_stop(0.95, &stop_rule(0.95));
        assert_eq!(decision, StopDecision::StopAccept);
    }

    #[test]
    fn no_threshold_uses_default_095() {
        let rule = BayesianStoppingRule::new();
        let no_threshold_rule = StopRule {
            max_n: 500,
            alpha: None,
            threshold: None,
        };
        // Default threshold = 0.95; posterior 0.96 should stop-accept
        assert_eq!(
            rule.should_stop(0.96, &no_threshold_rule),
            StopDecision::StopAccept
        );
        // posterior 0.50 should continue
        assert_eq!(
            rule.should_stop(0.50, &no_threshold_rule),
            StopDecision::Continue
        );
    }
}
