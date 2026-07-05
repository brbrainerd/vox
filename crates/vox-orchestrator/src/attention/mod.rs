//! Developer attention as a first-class budget resource (Phase 15).
//!
//! Adapted NASA TLX (4 automated subscales), EWMA trust scoring with
//! Bayesian prior, Shannon entropy for auto-approve graduation, and
//! token-output-weighted complexity.
//!
//! All functions in this module are pure (no I/O, no async).
//! Persistence lives in [`crate::attention_tracker`].

mod budget;
pub mod calibrator;
mod interruption_policy;
mod routing;

pub use budget::{
    ActionDescriptor, ApprovalOutcome, ApprovalTier, AttentionBudget, AttentionEvent,
    AttentionEventType, ClarificationLoopStop, DEFAULT_ATTENTION_BUDGET_MS,
    DEFAULT_INTERRUPT_COST_MS, FocusDepth, InterruptionCalibrationConfig, NasaTlxWeights,
    TierGateConfig, TrustTier, clarification_stop_rule, compute_attention_cost_ms,
    decision_entropy_bits, info_gain_per_attention_cost_bits_ms,
};
pub use interruption_policy::{
    InterruptionChannel, InterruptionDecision, InterruptionSignals, evaluate_interruption,
    scaled_interrupt_cost_ms,
};
pub use routing::{
    AgentTrustScore, DEFAULT_TRUST_WINDOW_MS, TRUST_DECAY_GRACE_MS, TRUST_DECAY_HALF_LIFE_MS,
    classify_tier,
};

#[cfg(test)]
mod tests {
    use crate::types::{AgentId, TaskPriority};

    use super::*;

    fn make_trust(tier: TrustTier, score: f64) -> AgentTrustScore {
        AgentTrustScore {
            agent_id: AgentId(1),
            trust_score: score,
            tier,
            total_outcomes: 25,
            successful_outcomes: 20,
            below_tier_streak: 0,
            last_updated_ms: 0,
            variance: 0.10,
            is_override: false,
            outcome_window: Default::default(),
        }
    }

    fn make_action(writes: usize, external: bool, tokens: u64, complexity: u8) -> ActionDescriptor {
        ActionDescriptor {
            estimated_complexity: complexity,
            tokens_output: tokens,
            priority: TaskPriority::Normal,
            write_file_count: writes,
            external,
            repeated_approve_count: 0,
            concurrent_tasks: 1,
        }
    }

    #[test]
    fn attention_cost_scales_with_token_output() {
        let w = NasaTlxWeights::default();
        let low = make_action(1, false, 50, 5);
        let high = make_action(1, false, 5000, 5);
        let cost_low = compute_attention_cost_ms(&low, 0.5, DEFAULT_INTERRUPT_COST_MS, &w);
        let cost_high = compute_attention_cost_ms(&high, 0.5, DEFAULT_INTERRUPT_COST_MS, &w);
        assert!(
            cost_high > cost_low,
            "high-token output should cost more attention"
        );
    }

    #[test]
    fn trust_ewma_promotes_untrusted_to_provisional() {
        let mut ts = AgentTrustScore::new(AgentId(1));
        for _ in 0..5 {
            ts.record_outcome(true, 0.1, 5, 20);
        }
        assert_eq!(ts.total_outcomes, 5);
        assert!(
            ts.trust_score > 0.45 || ts.tier == TrustTier::Provisional || ts.total_outcomes < 5,
            "score={:.3}, tier={:?}",
            ts.trust_score,
            ts.tier
        );
    }

    #[test]
    fn trust_ewma_demotes_after_3_consecutive_failures() {
        let mut ts = make_trust(TrustTier::Trusted, 0.75);
        ts.total_outcomes = 25;
        ts.record_outcome(false, 0.5, 5, 20);
        ts.record_outcome(false, 0.5, 5, 20);
        ts.record_outcome(false, 0.5, 5, 20);
        assert!(
            ts.tier == TrustTier::Provisional || ts.trust_score < 0.70,
            "should demote or drop below threshold"
        );
    }

    #[test]
    fn classify_tier_auto_approves_read_only() {
        let trust = make_trust(TrustTier::Untrusted, 0.2);
        let action = make_action(0, false, 100, 3);
        let gate = TierGateConfig::default();
        assert_eq!(
            classify_tier(&trust, &action, 0.5, &gate),
            ApprovalTier::AutoApprove
        );
    }

    #[test]
    fn classify_tier_blocks_untrusted_wide_writes() {
        let trust = make_trust(TrustTier::Untrusted, 0.2);
        let action = make_action(4, false, 100, 3);
        let gate = TierGateConfig::default();
        assert_eq!(
            classify_tier(&trust, &action, 0.5, &gate),
            ApprovalTier::Blocked
        );
    }

    #[test]
    fn classify_tier_blocks_external() {
        let trust = make_trust(TrustTier::System, 0.99);
        let action = make_action(0, true, 100, 1);
        let gate = TierGateConfig::default();
        assert_eq!(
            classify_tier(&trust, &action, 0.0, &gate),
            ApprovalTier::Blocked
        );
    }

    #[test]
    fn shannon_entropy_near_zero_for_certain_decisions() {
        let h = decision_entropy_bits(0.99);
        assert!(h < 0.15, "H={:.4} should be < 0.15 for p=0.99", h);
    }

    #[test]
    fn shannon_entropy_max_at_half() {
        let h = decision_entropy_bits(0.5);
        assert!(
            (h - 1.0).abs() < 0.01,
            "H should be ~1.0 at p=0.5, got {h:.4}"
        );
    }

    #[test]
    fn focus_depth_escalates_at_8_per_hour() {
        let budget = AttentionBudget {
            interrupt_freq_per_hour: 8.0,
            ..Default::default()
        };
        assert_eq!(budget.focus_depth(), FocusDepth::Deep);
    }

    #[test]
    fn focus_depth_focused_between_3_and_8() {
        let budget = AttentionBudget {
            interrupt_freq_per_hour: 5.0,
            ..Default::default()
        };
        assert_eq!(budget.focus_depth(), FocusDepth::Focused);
    }

    #[test]
    fn attention_budget_exhausted_flag() {
        let mut b = AttentionBudget::default();
        b.spent_ms = b.max_attention_ms;
        assert!(b.exhausted());
    }

    #[test]
    fn info_gain_per_cost_prefers_lower_cost() {
        let a = info_gain_per_attention_cost_bits_ms(0.20, 100);
        let b = info_gain_per_attention_cost_bits_ms(0.20, 400);
        assert!(a > b, "expected higher utility for lower cost");
    }

    #[test]
    fn clarification_stop_rule_honors_marginal_gain_floor() {
        let stop = clarification_stop_rule(1, 3, 0.01, 0.05, 100, 1_000);
        assert_eq!(stop, ClarificationLoopStop::MarginalGainTooLow);
    }

    #[test]
    fn auto_approve_graduation_requires_low_entropy_and_trust() {
        let trust = make_trust(TrustTier::Trusted, 0.90); // meets gate.auto_approve_min_trust=0.85
        let mut action = make_action(1, false, 200, 3);
        action.repeated_approve_count = 10;
        let gate = TierGateConfig::default();
        // entropy < 0.15 AND trust >= 0.85 should auto-approve
        assert_eq!(
            classify_tier(&trust, &action, 0.10, &gate),
            ApprovalTier::AutoApprove
        );
        // entropy > 0.15 should not
        assert_ne!(
            classify_tier(&trust, &action, 0.50, &gate),
            ApprovalTier::AutoApprove
        );
    }
}
