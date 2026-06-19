//! Unified **when / whether to interrupt the pilot** policy (dynamic interruption control).
//!
//! Combines information-theoretic questioning signals with the live [`AttentionBudget`]
//! (focus depth, spent ratio) and channel-specific priors. Pure functions only.
//!
//! **Serialization contract:** [`InterruptionSignals`] / [`InterruptionDecision`] should stay aligned with
//! `contracts/communication/interruption-decision.schema.json` when that schema is updated for cross-surface telemetry.

use serde::{Deserialize, Serialize};

use super::budget::{AttentionBudget, FocusDepth};

/// Where the interruption candidate originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InterruptionChannel {
    /// Chat / inline Socrates clarification.
    #[default]
    ChatClarification,
    /// Planner or plan-review surfaces.
    PlanReview,
    /// Task submit or orchestrator enqueue requiring confirmation.
    TaskSubmit,
    /// Agent-to-agent escalation that may surface to the user.
    A2AEscalation,
    /// Ghost / inline-edit latency-style questioning debits.
    InlineAssist,
    /// Fallback / unknown channel.
    Other,
}

/// Inputs to [`evaluate_interruption`] — keep this serializable for telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptionSignals {
    pub channel: InterruptionChannel,
    /// Expected entropy reduction (bits) if the user answers.
    pub expected_information_gain_bits: f64,
    /// Normalized user burden in `[0, 1]` (time / complexity / interruption).
    pub expected_user_cost: f64,
    /// Calibrated confidence in `[0, 1]`.
    pub confidence_estimate: f64,
    /// Contradiction ratio in `[0, 1]` (higher → riskier to proceed silently).
    pub contradiction_ratio: f64,
    /// Unresolved human-visible prompts already open for this session.
    pub pending_clarification_backlog: u32,
    /// Zero-based clarification turn index in this session.
    pub clarification_turn_index: u32,
    /// Max clarification turns (e.g. from [`vox_orchestrator_types::socrates_policy::QuestioningPolicy`]).
    pub max_clarification_turns: u32,
    /// Irreversible, policy-sensitive, or high blast-radius action pending.
    pub irreversible_or_high_risk: bool,
    /// Baseline interrupt recovery cost in ms (e.g. Gloria Mark baseline).
    pub base_interrupt_cost_ms: u64,
    /// Agent trust in `[0, 1]` (higher trust → fewer interrupts).
    pub trust_score: f64,
    /// Open questioning session exists (batch / consolidate prompts).
    pub open_question_session: bool,
    /// Specification uncertainty in `[0, 1]`: how unresolved the *user's intent/parameters* are
    /// (high → a clarifying question is likely to help). Defaults to 0.0 for legacy payloads.
    #[serde(default)]
    pub spec_uncertainty: f64,
    /// Model uncertainty in `[0, 1]`: the LLM's *own* epistemic doubt about its prediction
    /// (high → asking the user may not help). Defaults to 0.0 for legacy payloads.
    #[serde(default)]
    pub model_uncertainty: f64,
}

/// Policy result — what to do with this interruption candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterruptionDecision {
    /// Show the prompt now; `scaled_cost_ms` is debited to attention when mirrored.
    InterruptNow { reason: String, scaled_cost_ms: u64 },
    /// Defer to a later checkpoint (e.g. after more tool evidence).
    DeferUntilCheckpoint { reason: String },
    /// Prefer consolidating with an existing open prompt.
    BatchWithExistingPrompt { reason: String },
    /// Continue without a user-visible question.
    ProceedAutonomously { reason: String },
    /// Safety / compliance — user must be engaged before continuing.
    RequireHumanBeforeContinue { reason: String, scaled_cost_ms: u64 },
}

impl InterruptionDecision {
    #[must_use]
    pub fn scaled_cost_ms(&self) -> u64 {
        match self {
            InterruptionDecision::InterruptNow { scaled_cost_ms, .. }
            | InterruptionDecision::RequireHumanBeforeContinue { scaled_cost_ms, .. } => {
                *scaled_cost_ms
            }
            _ => 0,
        }
    }
}

#[inline]
fn channel_gain_prior(ch: InterruptionChannel) -> f64 {
    match ch {
        InterruptionChannel::PlanReview => 0.06,
        InterruptionChannel::TaskSubmit => 0.05,
        InterruptionChannel::A2AEscalation => 0.04,
        InterruptionChannel::InlineAssist => 0.03,
        InterruptionChannel::ChatClarification | InterruptionChannel::Other => 0.0,
    }
}

#[inline]
fn focus_multiplier(depth: FocusDepth) -> f64 {
    match depth {
        FocusDepth::Deep => 1.35,
        FocusDepth::Focused => 1.12,
        FocusDepth::Ambient => 1.0,
    }
}

#[inline]
fn min_utility_threshold(spent_ratio: f64, alert_threshold: f64) -> f64 {
    let base = 0.11_f64;
    let alert = alert_threshold.clamp(0.05, 0.99);
    if spent_ratio <= alert {
        base + 0.12 * (spent_ratio / alert)
    } else {
        let over = (spent_ratio - alert) / (1.0 - alert).max(0.01);
        base + 0.12 + 0.35 * over
    }
}

/// Decide whether to interrupt the pilot now (enforce mode). When `attention_enabled` is false,
/// returns [`InterruptionDecision::InterruptNow`] with a neutral reason so existing UX is preserved
/// (shadow / no dynamic gating on the questioning path).
#[must_use]
pub fn evaluate_interruption(
    signals: &InterruptionSignals,
    attention: &AttentionBudget,
    attention_enabled: bool,
    attention_alert_threshold: f64,
) -> InterruptionDecision {
    if !attention_enabled {
        let cost = scaled_interrupt_cost_ms(signals, attention);
        return InterruptionDecision::InterruptNow {
            reason: "attention_policy_disabled_or_shadow".to_string(),
            scaled_cost_ms: cost.max(1),
        };
    }

    let spent_ratio = attention.spent_ratio();
    let exhausted = attention.exhausted();
    let depth = attention.focus_depth();

    // Hard safety: abstain-level risk or strong contradiction on consequential work.
    if signals.irreversible_or_high_risk
        && (signals.confidence_estimate < 0.42 || signals.contradiction_ratio > 0.38)
    {
        let cost = scaled_interrupt_cost_ms(signals, attention);
        return InterruptionDecision::RequireHumanBeforeContinue {
            reason: "high_risk_or_low_confidence_requires_human".to_string(),
            scaled_cost_ms: cost.max(1),
        };
    }

    if signals.clarification_turn_index >= signals.max_clarification_turns {
        return InterruptionDecision::ProceedAutonomously {
            reason: "max_clarification_turns_reached".to_string(),
        };
    }

    if exhausted && !signals.irreversible_or_high_risk {
        // Out of attention budget: only ask if information value is very high.
        let min_gain = 0.22_f64 + 0.15 * focus_multiplier(depth);
        if signals.expected_information_gain_bits < min_gain {
            return InterruptionDecision::ProceedAutonomously {
                reason: "attention_budget_exhausted_marginal_gain_insufficient".to_string(),
            };
        }
    }

    if exhausted && signals.irreversible_or_high_risk {
        let cost = scaled_interrupt_cost_ms(signals, attention);
        return InterruptionDecision::RequireHumanBeforeContinue {
            reason: "attention_exhausted_but_high_risk".to_string(),
            scaled_cost_ms: cost.max(1),
        };
    }

    // Batch / defer when a session is already open and this question is weak.
    if signals.open_question_session && signals.pending_clarification_backlog == 0 {
        let min_gain = 0.10 + channel_gain_prior(signals.channel);
        if signals.expected_information_gain_bits < min_gain {
            return InterruptionDecision::BatchWithExistingPrompt {
                reason: "open_session_low_marginal_gain".to_string(),
            };
        }
    }

    if signals.pending_clarification_backlog >= 1
        && signals.expected_information_gain_bits < 0.11
        && !signals.irreversible_or_high_risk
    {
        return InterruptionDecision::DeferUntilCheckpoint {
            reason: "backlog_and_low_diagnostic_value".to_string(),
        };
    }

    let trust_adj = (1.0 - signals.trust_score.clamp(0.0, 1.0)) * 0.08;
    let utility =
        signals.expected_information_gain_bits / signals.expected_user_cost.clamp(1e-6, 1.0);
    let threshold = min_utility_threshold(spent_ratio, attention_alert_threshold) + trust_adj;

    if utility < threshold && !signals.irreversible_or_high_risk {
        return InterruptionDecision::DeferUntilCheckpoint {
            reason: format!(
                "utility_below_threshold utility={utility:.4} threshold={threshold:.4} spent_ratio={spent_ratio:.3}"
            ),
        };
    }

    let cost = scaled_interrupt_cost_ms(signals, attention);
    InterruptionDecision::InterruptNow {
        reason: "value_over_dynamic_cost".to_string(),
        scaled_cost_ms: cost.max(1),
    }
}

#[must_use]
pub fn scaled_interrupt_cost_ms(signals: &InterruptionSignals, attention: &AttentionBudget) -> u64 {
    let base = signals.base_interrupt_cost_ms.max(1) as f64;
    let cost_norm = signals.expected_user_cost.clamp(0.05, 1.0);
    let backlog = 1.0 + 0.22_f64 * signals.pending_clarification_backlog as f64;
    let trust = (1.0 - signals.trust_score.clamp(0.0, 1.0)) * 0.25 + 0.75;
    let fm = focus_multiplier(attention.focus_depth());
    let contradiction = 1.0 + signals.contradiction_ratio * 0.35;
    (base * cost_norm * backlog * trust * fm * contradiction).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_signals() -> InterruptionSignals {
        InterruptionSignals {
            channel: InterruptionChannel::ChatClarification,
            expected_information_gain_bits: 0.18,
            expected_user_cost: 0.35,
            confidence_estimate: 0.62,
            contradiction_ratio: 0.15,
            pending_clarification_backlog: 0,
            clarification_turn_index: 0,
            max_clarification_turns: 3,
            irreversible_or_high_risk: false,
            base_interrupt_cost_ms: 23_250,
            trust_score: 0.55,
            open_question_session: false,
            spec_uncertainty: 0.0,
            model_uncertainty: 0.0,
        }
    }

    #[test]
    fn shadow_mode_always_interrupts_with_positive_cost() {
        let att = AttentionBudget::default();
        let d = evaluate_interruption(&sample_signals(), &att, false, 0.7);
        assert!(matches!(d, InterruptionDecision::InterruptNow { .. }));
        assert!(d.scaled_cost_ms() >= 1);
    }

    #[test]
    fn high_risk_low_confidence_requires_human() {
        let mut s = sample_signals();
        s.irreversible_or_high_risk = true;
        s.confidence_estimate = 0.35;
        let att = AttentionBudget::default();
        let d = evaluate_interruption(&s, &att, true, 0.7);
        assert!(matches!(
            d,
            InterruptionDecision::RequireHumanBeforeContinue { .. }
        ));
    }

    #[test]
    fn low_utility_defers_when_enabled() {
        let mut s = sample_signals();
        s.expected_information_gain_bits = 0.03;
        s.expected_user_cost = 0.9;
        let mut att = AttentionBudget::default();
        att.spent_ms = (att.max_attention_ms as f64 * 0.75) as u64;
        let d = evaluate_interruption(&s, &att, true, 0.7);
        assert!(
            matches!(d, InterruptionDecision::DeferUntilCheckpoint { .. }),
            "got {d:?}"
        );
    }

    #[test]
    fn max_turns_stops_questions() {
        let mut s = sample_signals();
        s.clarification_turn_index = 5;
        s.max_clarification_turns = 3;
        let att = AttentionBudget::default();
        let d = evaluate_interruption(&s, &att, true, 0.7);
        assert!(matches!(
            d,
            InterruptionDecision::ProceedAutonomously { .. }
        ));
    }

    #[test]
    fn signals_default_separates_spec_and_model_uncertainty() {
        let json = r#"{
            "channel": "inline_assist","expected_information_gain_bits":0.2,"expected_user_cost":0.3,
            "confidence_estimate":0.6,"contradiction_ratio":0.0,"pending_clarification_backlog":0,
            "clarification_turn_index":0,"max_clarification_turns":3,"irreversible_or_high_risk":false,
            "base_interrupt_cost_ms":23250,"trust_score":0.5,"open_question_session":false }"#;
        let s: InterruptionSignals = serde_json::from_str(json).expect("legacy JSON must deserialize");
        assert_eq!(s.spec_uncertainty, 0.0);
        assert_eq!(s.model_uncertainty, 0.0);
    }
}
