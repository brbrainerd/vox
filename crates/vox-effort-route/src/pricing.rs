//! Real per-direction token pricing, sourced from the model registry by the
//! CLI and passed into the router. The library never reads the registry (keeps
//! this crate free of a vox-orchestrator dep). Unknown price → None, never a
//! fabricated $0.00.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelRates {
    pub input_per_1k_usd: f64,
    pub output_per_1k_usd: f64,
    pub known: bool,
}

impl ModelRates {
    /// Real USD cost for a single judge call, or `None` when the model is not in
    /// the pricing catalog (we never invent a $0.00 for an unpriced model).
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
        assert_eq!(r.cost_usd(2000, 1000), Some(21.0));
    }
    #[test]
    fn unknown_rates_return_none_not_zero() {
        assert_eq!(ModelRates::default().cost_usd(1000, 1000), None);
    }
}

#[cfg(test)]
mod semcov_wave7_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: cost_usd returning Some(0.0) for zero tokens instead of Some(0.0)
    // (this is actually correct behaviour — zero tokens costs zero; the test
    // verifies the computation is right and not accidentally returning None)
    #[test]
    fn zero_tokens_known_rates_yields_some_zero() {
        let r = ModelRates {
            input_per_1k_usd: 3.0,
            output_per_1k_usd: 15.0,
            known: true,
        };
        assert_eq!(
            r.cost_usd(0, 0),
            Some(0.0),
            "zero tokens with known rates must yield Some(0.0), not None"
        );
    }

    // Catches: cost_usd using output rate for input tokens (direction swap bug)
    #[test]
    fn cost_usd_applies_correct_rate_per_direction() {
        let r = ModelRates {
            input_per_1k_usd: 1.0,   // $1 per 1k input
            output_per_1k_usd: 10.0, // $10 per 1k output
            known: true,
        };
        // 1000 input @ $1/1k = $1.00; 0 output = $0.00 total
        assert_eq!(
            r.cost_usd(1000, 0),
            Some(1.0),
            "cost must use input rate for prompt tokens"
        );
        // 0 input; 1000 output @ $10/1k = $10.00
        assert_eq!(
            r.cost_usd(0, 1000),
            Some(10.0),
            "cost must use output rate for completion tokens"
        );
    }

    // Catches: known=true with 0.0 rates being mistaken for "unknown"
    // (zero-cost model is legitimately priced at zero, must return Some not None)
    #[test]
    fn known_zero_rates_return_some_not_none() {
        let r = ModelRates {
            input_per_1k_usd: 0.0,
            output_per_1k_usd: 0.0,
            known: true,
        };
        assert_eq!(
            r.cost_usd(100, 100),
            Some(0.0),
            "a model priced at $0 must return Some(0.0), never None"
        );
    }

    // Catches: ModelRates::default() having known=true (default should be unknown)
    #[test]
    fn default_model_rates_are_unknown() {
        let r = ModelRates::default();
        assert!(!r.known, "default ModelRates must have known=false");
        assert_eq!(
            r.cost_usd(9999, 9999),
            None,
            "default (unknown) rates must return None for any token count"
        );
    }
}
