//! Real per-direction token pricing, sourced from the model registry by the
//! CLI and passed into the pipeline. The library never reads the registry
//! (keeps this crate free of a vox-orchestrator dependency). When a model's
//! price is unknown, cost is `None` — never a fabricated $0.00.

/// USD-per-1K-token rates for the resolved judge model.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ModelRates {
    pub input_per_1k_usd: f64,
    pub output_per_1k_usd: f64,
    /// False when the registry had no price for the model — cost() returns None.
    pub known: bool,
}

impl ModelRates {
    /// Real cost in USD, or None when pricing is unknown (NOT a fake 0.0).
    pub fn cost_usd(&self, prompt_tokens: u64, completion_tokens: u64) -> Option<f64> {
        if !self.known {
            return None;
        }
        Some(
            (prompt_tokens as f64 / 1000.0) * self.input_per_1k_usd
                + (completion_tokens as f64 / 1000.0) * self.output_per_1k_usd,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_rates_compute_real_cost() {
        let r = ModelRates {
            input_per_1k_usd: 3.0,
            output_per_1k_usd: 15.0,
            known: true,
        };
        // 2000 prompt @ $3/1k = $6; 1000 completion @ $15/1k = $15 → $21
        assert_eq!(r.cost_usd(2000, 1000), Some(21.0));
    }

    #[test]
    fn unknown_rates_return_none_not_zero() {
        let r = ModelRates::default(); // known = false
        assert_eq!(r.cost_usd(1000, 1000), None);
    }

    #[test]
    fn known_rates_with_zero_tokens_is_honest_zero() {
        // Real 0 tokens × real rate = real $0 — distinct from unknown's None.
        let r = ModelRates {
            input_per_1k_usd: 3.0,
            output_per_1k_usd: 15.0,
            known: true,
        };
        assert_eq!(r.cost_usd(0, 0), Some(0.0));
    }
}

#[cfg(test)]
mod semcov_wave8_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: known=true with zero rates returning None instead of Some(0.0),
    // confusing "free model" with "unknown pricing".
    #[test]
    fn zero_rates_known_true_is_some_not_none() {
        let r = ModelRates {
            input_per_1k_usd: 0.0,
            output_per_1k_usd: 0.0,
            known: true,
        };
        assert_eq!(
            r.cost_usd(1000, 1000),
            Some(0.0),
            "known=true with zero rates must be Some(0.0), not None"
        );
    }

    // Catches: u64::MAX tokens overflowing f64 cast and producing Inf instead of a real value.
    #[test]
    fn max_u64_tokens_does_not_produce_nan_or_inf() {
        let r = ModelRates {
            input_per_1k_usd: 3.0,
            output_per_1k_usd: 15.0,
            known: true,
        };
        let cost = r.cost_usd(u64::MAX, 0).unwrap();
        // Should be a very large finite float (u64::MAX / 1000 * 3.0 ≈ 5.5e16), not Inf/NaN.
        assert!(cost.is_finite(), "cost for u64::MAX tokens must be finite, got {cost}");
        assert!(cost > 0.0);
    }

    // Catches: cost_usd asymmetry — input vs output rates swapped in implementation.
    #[test]
    fn input_rate_applied_to_prompt_not_completion() {
        let r = ModelRates {
            input_per_1k_usd: 1.0,
            output_per_1k_usd: 100.0,
            known: true,
        };
        // 1000 prompt tokens at $1/1k = $1; 0 completion = $0.
        assert_eq!(r.cost_usd(1000, 0), Some(1.0));
        // 0 prompt; 1000 completion at $100/1k = $100.
        assert_eq!(r.cost_usd(0, 1000), Some(100.0));
    }

    // Catches: default() returning known=true (would make None branch unreachable).
    #[test]
    fn default_model_rates_known_is_false() {
        let r = ModelRates::default();
        assert!(!r.known, "ModelRates::default() must have known=false");
    }
}
