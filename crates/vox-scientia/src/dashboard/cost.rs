//! Cost rollup for `/api/v2/scientia/cost`.

use serde::{Deserialize, Serialize};

/// Response served at `GET /api/v2/scientia/cost`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CostRollup {
    pub this_quarter: QuarterlyCostSummary,
    pub per_finding_average_usd: f64,
    pub by_provider: Vec<CostByProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuarterlyCostSummary {
    pub extraction_usd: f64,
    pub critic_usd: f64,
    pub novelty_retrieval_usd: f64,
    pub scholarly_submission_usd: f64,
    pub total_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CostByProvider {
    pub provider: String,
    pub usd: f64,
}

/// Inputs assembled by the dashboard backend from cost-bearing telemetry
/// rows. Producers are responsible for windowing to "this quarter."
#[derive(Debug, Clone, PartialEq)]
pub struct CostInputs {
    pub extraction_usd: f64,
    pub critic_usd: f64,
    pub novelty_retrieval_usd: f64,
    pub scholarly_submission_usd: f64,
    /// Per-provider breakdown for the same window.
    pub by_provider: Vec<(String, f64)>,
    /// Number of findings published in the window. Used to compute the
    /// per-finding average; `0` yields `0.0` (avoid divide-by-zero).
    pub findings_published_this_quarter: u64,
}

/// Assemble a [`CostRollup`] from inputs. Totals are recomputed here so the
/// dashboard backend can't accidentally desync sub-line items vs the total.
pub fn build_cost_rollup(inputs: &CostInputs) -> CostRollup {
    let total = inputs.extraction_usd
        + inputs.critic_usd
        + inputs.novelty_retrieval_usd
        + inputs.scholarly_submission_usd;
    let per_finding_average = if inputs.findings_published_this_quarter == 0 {
        0.0
    } else {
        total / inputs.findings_published_this_quarter as f64
    };
    let by_provider: Vec<CostByProvider> = inputs
        .by_provider
        .iter()
        .map(|(p, u)| CostByProvider {
            provider: p.clone(),
            usd: *u,
        })
        .collect();
    CostRollup {
        this_quarter: QuarterlyCostSummary {
            extraction_usd: inputs.extraction_usd,
            critic_usd: inputs.critic_usd,
            novelty_retrieval_usd: inputs.novelty_retrieval_usd,
            scholarly_submission_usd: inputs.scholarly_submission_usd,
            total_usd: total,
        },
        per_finding_average_usd: per_finding_average,
        by_provider,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CostInputs {
        CostInputs {
            extraction_usd: 10.0,
            critic_usd: 5.0,
            novelty_retrieval_usd: 2.5,
            scholarly_submission_usd: 0.5,
            by_provider: vec![("anthropic".into(), 7.0), ("openai".into(), 11.0)],
            findings_published_this_quarter: 6,
        }
    }

    #[test]
    fn total_equals_sum_of_subcosts() {
        let r = build_cost_rollup(&sample());
        let want = 10.0 + 5.0 + 2.5 + 0.5;
        assert!((r.this_quarter.total_usd - want).abs() < 1e-9);
    }

    #[test]
    fn per_finding_average_uses_total_over_count() {
        let r = build_cost_rollup(&sample());
        let want = (10.0 + 5.0 + 2.5 + 0.5) / 6.0;
        assert!((r.per_finding_average_usd - want).abs() < 1e-9);
    }

    #[test]
    fn zero_findings_yields_zero_average_no_panic() {
        let mut i = sample();
        i.findings_published_this_quarter = 0;
        let r = build_cost_rollup(&i);
        assert_eq!(r.per_finding_average_usd, 0.0);
    }

    #[test]
    fn by_provider_preserves_order_and_values() {
        let r = build_cost_rollup(&sample());
        assert_eq!(
            r.by_provider,
            vec![
                CostByProvider { provider: "anthropic".into(), usd: 7.0 },
                CostByProvider { provider: "openai".into(), usd: 11.0 },
            ]
        );
    }

    #[test]
    fn json_round_trip_preserves_all_fields() {
        let r = build_cost_rollup(&sample());
        let j = serde_json::to_string(&r).unwrap();
        let back: CostRollup = serde_json::from_str(&j).unwrap();
        assert_eq!(back, r);
    }
}
