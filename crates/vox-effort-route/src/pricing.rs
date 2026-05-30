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
