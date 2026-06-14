use crate::types::{AgentId, TaskId, TaskPriority};
use serde::{Deserialize, Serialize};

// ── Configurable weight structs ────────────────────────────────────────────

/// Pilot-calibratable NASA TLX subscale weights for attention cost computation.
/// All four weights should sum to 1.0; they are not validated at parse time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NasaTlxWeights {
    /// Mental demand (complexity, surprise). Default: 0.35
    pub mental: f64,
    /// Temporal demand (urgency). Default: 0.25
    pub temporal: f64,
    /// Frustration (repeated patterns). Default: 0.20
    pub frustration: f64,
    /// Trust discount (inverse of agent trust). Default: 0.20
    pub trust_discount: f64,
}

impl Default for NasaTlxWeights {
    fn default() -> Self {
        Self {
            mental: 0.35,
            temporal: 0.25,
            frustration: 0.20,
            trust_discount: 0.20,
        }
    }
}

/// Config-driven thresholds for `classify_tier()`.
/// Decouples gating policy from compiled constants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierGateConfig {
    /// Shannon entropy (bits) below which auto-approve graduation triggers. Default: 0.15
    pub entropy_auto_approve_threshold: f64,
    /// Minimum repeated approvals before entropy graduation. Default: 10
    pub auto_approve_min_observations: u32,
    /// Minimum trust score for auto-approve (from OrchestratorConfig). Default: 0.85
    pub auto_approve_min_trust: f64,
    /// Max write files before blocking an Untrusted agent. Default: 3
    pub untrusted_max_writes_before_block: usize,
    /// Max write files before requiring Review from a Trusted agent. Default: 1
    pub trusted_single_file_confirm_limit: usize,
}

impl Default for TierGateConfig {
    fn default() -> Self {
        Self {
            entropy_auto_approve_threshold: 0.15,
            auto_approve_min_observations: 10,
            auto_approve_min_trust: 0.85,
            untrusted_max_writes_before_block: 3,
            trusted_single_file_confirm_limit: 1,
        }
    }
}

/// Runtime calibration knobs for dynamic interruption decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptionCalibrationConfig {
    /// Utility offsets (bits) by channel prior to policy evaluation.
    pub plan_review_gain_offset_bits: f64,
    pub task_submit_gain_offset_bits: f64,
    pub a2a_escalation_gain_offset_bits: f64,
    pub inline_assist_gain_offset_bits: f64,
    /// Additional expected-user-cost pressure per unresolved clarification backlog item.
    pub backlog_cost_penalty_per_item: f64,
    /// Multiplier for trust-adjusted thresholding pressure.
    pub trust_adjustment_scale: f64,
}

impl Default for InterruptionCalibrationConfig {
    fn default() -> Self {
        Self {
            plan_review_gain_offset_bits: 0.00,
            task_submit_gain_offset_bits: 0.00,
            a2a_escalation_gain_offset_bits: 0.00,
            inline_assist_gain_offset_bits: 0.00,
            backlog_cost_penalty_per_item: 0.05,
            trust_adjustment_scale: 1.0,
        }
    }
}

/// Baseline interrupt recovery cost from Gloria Mark (UC Irvine, 2023): 23 min 15 sec.
pub const DEFAULT_INTERRUPT_COST_MS: u64 = 23_250;

/// Default pilot attention budget: 1 hour per session period.
pub const DEFAULT_ATTENTION_BUDGET_MS: u64 = 3_600_000;

// ── Enums ──────────────────────────────────────────────────────────────────

/// Classification of an action's approval requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ApprovalTier {
    /// No human review needed — read-only, reversible, or earned trust.
    AutoApprove,
    /// Quick confirm — low-risk mutation, earned trust above threshold.
    #[default]
    Confirm,
    /// Full review — architectural, security-critical, multi-file, low trust.
    Review,
    /// Blocked — requires explicit pilot sign-off (destructive, external, deploy).
    Blocked,
}

/// Quantified trust level for an agent, replacing `Option<String>` in `MessageEnvelope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TrustTier {
    /// Brand-new agent or unknown provenance. Trust score < 0.45.
    #[default]
    Untrusted,
    /// Some successful completions. Trust score [0.45, 0.70).
    Provisional,
    /// Consistent track record. Trust score [0.70, 0.90).
    Trusted,
    /// Operator-provisioned. Trust score [0.90, 1.0]. Not auto-promoted.
    System,
}

/// What kind of action consumed pilot attention.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AttentionEventType {
    /// A command (cargo test, git status) required approval.
    CommandApproval,
    /// Code diff presented for review.
    CodeReview,
    /// New task submission for confirmation.
    TaskSubmission,
    /// A2A message interrupted the pilot.
    A2AInterrupt,
    /// Error escalation.
    ErrorReport,
    /// Plan review request.
    PlanReview,
    /// Dynamic policy chose to defer an interrupt (batch / backoff).
    PolicyDeferred,
    /// Dynamic policy continued without user-visible interrupt.
    PolicyProceedAuto,
    /// User responded to a clarification (realized outcome for learning).
    ClarificationAnswered,
}

/// What the pilot decided.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalOutcome {
    /// Pilot approved as-is.
    Approved,
    /// Pilot rejected the proposed action.
    Rejected,
    /// Pilot approved with modifications.
    Modified,
    /// System auto-approved — zero attention cost.
    AutoApproved,
    /// Pilot did not respond within timeout.
    TimedOut,
}

// ── Structs ────────────────────────────────────────────────────────────────

/// Descriptor for an action about to be gated. Used by `classify_tier()`.
#[derive(Debug, Clone)]
pub struct ActionDescriptor {
    /// Estimated task complexity (1–10) from `AgentTask.estimated_complexity`.
    pub estimated_complexity: u8,
    /// Token count of the LLM output that generated this action.
    pub tokens_output: u64,
    /// Task priority from the orchestrator queue.
    pub priority: TaskPriority,
    /// Number of files in the write manifest.
    pub write_file_count: usize,
    /// Whether the action touches external resources (network, deploy).
    pub external: bool,
    /// How many times this (agent, pattern) pair has been approved without failure.
    pub repeated_approve_count: u32,
    /// Number of concurrent tasks the pilot is overseeing right now.
    pub concurrent_tasks: usize,
}

/// Single attention event recorded in the orchestrator and persisted to Arca.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionEvent {
    pub agent_id: AgentId,
    pub task_id: Option<TaskId>,
    pub event_type: AttentionEventType,
    pub tier: ApprovalTier,
    /// Computed attention cost in ms (0 for AutoApproved events).
    pub cost_ms: u64,
    pub outcome: ApprovalOutcome,
    /// Agent's EWMA trust score at the moment this event was recorded.
    pub trust_score_at_time: f64,
    /// Effective complexity: 0.4 × estimated + 0.6 × token_complexity.
    pub effective_complexity: f64,
    /// Shannon entropy in bits for this (agent, pattern) pair.
    pub decision_entropy_bits: f64,
    pub timestamp_ms: u64,
    /// Surface channel / tool name for audit (e.g. `mcp_chat`, `plan`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Short human-readable policy rationale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_reason: Option<String>,
}

/// Per-session attention budget tracked in-memory, periodically flushed to Arca.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionBudget {
    /// Maximum attention budget in milliseconds per period.
    pub max_attention_ms: u64,
    /// Total attention spent so far this period.
    pub spent_ms: u64,
    /// Total approval requests this period.
    pub total_requests: u32,
    /// Requests that were auto-approved (zero attention cost).
    pub auto_approved: u32,
    /// Requests where pilot rejected (wasted attention).
    pub rejected: u32,
    /// EWMA interrupt frequency (events per hour).
    pub interrupt_freq_per_hour: f64,
    /// Timestamp of last non-auto interrupt (for gap calculation).
    pub last_interrupt_ms: u64,
    /// Number of A2A messages suppressed due to Deep focus depth.
    pub inbox_suppressed_count: u32,
}

impl Default for AttentionBudget {
    fn default() -> Self {
        Self {
            max_attention_ms: DEFAULT_ATTENTION_BUDGET_MS,
            spent_ms: 0,
            total_requests: 0,
            auto_approved: 0,
            rejected: 0,
            interrupt_freq_per_hour: 0.0,
            last_interrupt_ms: 0,
            inbox_suppressed_count: 0,
        }
    }
}

impl AttentionBudget {
    /// Create a budget with a custom ceiling.
    pub fn with_max(max_attention_ms: u64) -> Self {
        Self {
            max_attention_ms,
            ..Default::default()
        }
    }

    /// Fraction of budget consumed (0.0–1.0+).
    pub fn spent_ratio(&self) -> f64 {
        if self.max_attention_ms == 0 {
            return 1.0;
        }
        self.spent_ms as f64 / self.max_attention_ms as f64
    }

    /// True when spent exceeds the given alert threshold.
    pub fn alert(&self, threshold: f64) -> bool {
        self.spent_ratio() > threshold
    }

    /// True when budget is fully exhausted.
    pub fn exhausted(&self) -> bool {
        self.spent_ms >= self.max_attention_ms
    }

    /// Efficiency: what fraction of approvals were useful (not rejected).
    pub fn efficiency(&self) -> f64 {
        if self.total_requests == 0 {
            return 1.0;
        }
        1.0 - (self.rejected as f64 / self.total_requests as f64)
    }

    /// Auto-approve ratio: higher means less attention drain.
    pub fn auto_approve_ratio(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.auto_approved as f64 / self.total_requests as f64
    }

    /// Current focus depth from interrupt frequency.
    pub fn focus_depth(&self) -> FocusDepth {
        if self.interrupt_freq_per_hour >= 8.0 {
            FocusDepth::Deep
        } else if self.interrupt_freq_per_hour >= 3.0 {
            FocusDepth::Focused
        } else {
            FocusDepth::Ambient
        }
    }
}

// ── FocusDepth ─────────────────────────────────────────────────────────────

/// Agent inbox suppression level based on interrupt frequency.
/// Maps to brain's Salience/Executive Control Networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FocusDepth {
    /// Full SN+ECN; unrestricted A2A. Interrupt freq < 3/hr.
    #[default]
    Ambient,
    /// ECN at boundaries; non-critical A2A deferred. 3 ≤ freq < 8/hr.
    Focused,
    /// ECN pre/post task only; ScopeConflict/ErrorReport A2A only. freq ≥ 8/hr.
    Deep,
}

/// Stop reasons for clarification loops constrained by attention budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClarificationLoopStop {
    /// Continue asking clarification questions.
    Continue,
    /// Stop because turn budget was reached.
    MaxTurnsReached,
    /// Stop because marginal gain is too low.
    MarginalGainTooLow,
    /// Stop because attention budget is exhausted.
    AttentionBudgetExceeded,
}

// ── Computation functions ──────────────────────────────────────────────────

/// Compute attention cost in ms using configurable NASA TLX subscales.
///
/// `attention_cost = base_ms × Σ(wᵢ × subscaleᵢ) × context_switch_mult`
pub fn compute_attention_cost_ms(
    action: &ActionDescriptor,
    agent_trust: f64,
    base_ms: u64,
    weights: &NasaTlxWeights,
) -> u64 {
    let token_complexity = (action.tokens_output as f64 / 1000.0).min(10.0);
    let effective_complexity = 0.4 * (action.estimated_complexity as f64) + 0.6 * token_complexity;
    let mental = (effective_complexity / 10.0).clamp(0.0, 1.0);

    let temporal = match action.priority {
        TaskPriority::Urgent => 1.0,
        TaskPriority::Normal => 0.5,
        TaskPriority::Background => 0.2,
    };

    let r = action.repeated_approve_count as f64;
    let frustration = r / (r + 5.0);

    let trust_discount = (1.0 - agent_trust).clamp(0.0, 1.0);

    let weighted = weights.mental * mental
        + weights.temporal * temporal
        + weights.frustration * frustration
        + weights.trust_discount * trust_discount;

    let csm = (1.0 + (action.concurrent_tasks.saturating_sub(1) as f64) * 0.3).clamp(1.0, 3.0);

    (base_ms as f64 * weighted * csm) as u64
}

/// Shannon entropy for a binary approve/reject decision.
///
/// `H = −p·log₂(p) − (1−p)·log₂(1−p)`
///
/// Returns NaN-safe result; clamps p to [0.001, 0.999].
pub fn decision_entropy_bits(approve_rate: f64) -> f64 {
    let p = approve_rate.clamp(0.001, 0.999);
    -(p * p.log2() + (1.0 - p) * (1.0 - p).log2())
}

/// Information gain normalized by estimated attention cost (bits/ms).
#[must_use]
pub fn info_gain_per_attention_cost_bits_ms(
    expected_information_gain_bits: f64,
    cost_ms: u64,
) -> f64 {
    expected_information_gain_bits.max(0.0) / (cost_ms.max(1) as f64)
}

/// Clarification-loop stop rule combining turn caps, marginal gains, and attention budget.
#[must_use]
pub fn clarification_stop_rule(
    turn_index: u32,
    max_turns: u32,
    marginal_gain_bits: f64,
    min_marginal_gain_bits: f64,
    spent_attention_ms: u64,
    max_attention_ms: u64,
) -> ClarificationLoopStop {
    if turn_index >= max_turns {
        return ClarificationLoopStop::MaxTurnsReached;
    }
    if marginal_gain_bits < min_marginal_gain_bits {
        return ClarificationLoopStop::MarginalGainTooLow;
    }
    if spent_attention_ms >= max_attention_ms {
        return ClarificationLoopStop::AttentionBudgetExceeded;
    }
    ClarificationLoopStop::Continue
}

#[cfg(test)]
mod semcov_wave1c_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn attention_cost_matches_exact_nasa_tlx_formula() {
        let action = ActionDescriptor {
            estimated_complexity: 10,
            tokens_output: 1000,
            priority: TaskPriority::Urgent,
            write_file_count: 0,
            external: false,
            repeated_approve_count: 0,
            concurrent_tasks: 1,
        };
        let weights = NasaTlxWeights::default();
        // token_complexity = min(1000/1000, 10) = 1.0
        // effective_complexity = 0.4*10 + 0.6*1.0 = 4.6 -> mental = 0.46
        // temporal(Urgent)=1.0, frustration=0, trust_discount(trust=1.0)=0
        // weighted = 0.35*0.46 + 0.25*1.0 = 0.411 ; csm = 1.0
        // 100 * 0.411 * 1.0 = 41.1 -> 41
        let cost = compute_attention_cost_ms(&action, 1.0, 100, &weights);
        assert_eq!(
            cost, 41,
            "exact NASA TLX attention cost for the fixed input"
        );
    }
}
