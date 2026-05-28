use super::enums::CostPreference;

pub(super) fn default_heartbeat_interval() -> u64 {
    5_000
}
pub(super) fn default_stale_threshold() -> u64 {
    60_000
}
pub(super) fn default_true() -> bool {
    true
}
pub(super) fn default_continuation_cooldown() -> u64 {
    30_000
}
pub(super) fn default_max_auto_continuations() -> u32 {
    5
}
pub(super) fn default_event_capacity() -> usize {
    1024
}
pub(super) fn default_min_agents() -> usize {
    1
}
pub(super) fn default_scaling_threshold() -> usize {
    5
}
pub(super) fn default_idle_retirement() -> u64 {
    300_000
}
pub(super) fn default_false() -> bool {
    false
}
pub(super) fn default_cost_preference() -> CostPreference {
    // Free-by-default: economy is the baseline; callers must opt-in to Performance explicitly.
    CostPreference::Economy
}
pub(super) fn default_lookback_ticks() -> usize {
    5
}
pub(super) fn default_resource_weight() -> f64 {
    0.3
}
pub(super) fn default_cpu_multiplier() -> f64 {
    0.7
}
pub(super) fn default_mem_multiplier() -> f64 {
    0.3
}
pub(super) fn default_resource_exponent() -> f64 {
    1.0
}
pub(super) fn default_max_spawn_per_tick() -> usize {
    5
}
pub(super) fn default_scaling_cooldown_ms() -> u64 {
    5_000
}
pub(super) fn default_urgent_rebalance_threshold() -> usize {
    3
}
pub(super) fn default_execution_time_budget_multiplier() -> f64 {
    1.5
}
pub(super) fn default_financial_cost_budget_micros() -> i64 {
    50_000 // default $0.05
}
pub(super) fn default_max_toestub_debug_iterations() -> u8 {
    3
}
pub(super) fn default_max_socrates_debug_iterations() -> u8 {
    3
}

pub(super) fn default_populi_poll_interval_secs() -> u64 {
    30
}

pub(super) fn default_populi_remote_result_poll_interval_secs() -> u64 {
    5
}

pub(super) fn default_populi_remote_worker_poll_interval_secs() -> u64 {
    5
}

pub(super) fn default_populi_remote_result_max_messages_per_poll() -> usize {
    64
}

pub(super) fn default_populi_http_timeout_ms() -> u64 {
    10_000
}

pub(super) fn default_populi_training_budget_pressure() -> f64 {
    0.0
}

pub(super) fn default_socrates_reputation_weight() -> f64 {
    1.0
}

pub(super) fn default_idle_timeout() -> u64 {
    600_000
}

pub(super) fn default_task_timeout() -> u64 {
    1_800_000
}

// Phase 15: Attention budget defaults
pub(super) fn default_attention_budget_ms() -> u64 {
    3_600_000
}
pub(super) fn default_attention_alert_threshold() -> f64 {
    0.7
}
pub(super) fn default_attention_interrupt_cost_ms() -> u64 {
    23_250
}
pub(super) fn default_trust_ewma_alpha() -> f64 {
    0.1
}
pub(super) fn default_trust_provisional_threshold() -> u32 {
    5
}
pub(super) fn default_trust_trusted_threshold() -> u32 {
    20
}
pub(super) fn default_trust_auto_approve_min() -> f64 {
    0.85
}
pub(super) fn default_trust_gate_relax_min_reliability() -> f64 {
    default_trust_auto_approve_min()
}
pub(super) fn default_attention_trust_routing_weight() -> f64 {
    2.0
}
pub(super) fn default_trust_task_completion_floor() -> f64 {
    0.25
}
pub(super) fn default_trust_task_completion_weight() -> f64 {
    1.5
}
pub(super) fn default_routing_exploration_epsilon() -> f64 {
    0.05
}

/// Routing bonus for shard-role specialization.
///
/// Chosen to sit between baseline reliability blending (1.0) and attention-trust
/// influence (2.0), so specialization influences ties without dominating trust.
pub(super) fn default_repo_shard_specialization_weight() -> f64 {
    1.5
}

/// Penalty multiplier applied for each recent shard validation failure.
///
/// Kept below baseline reliability weight to avoid overreacting to single failures
/// while still steering validation tasks away from unstable agents.
pub(super) fn default_repo_shard_validation_failure_penalty() -> f64 {
    0.8
}

/// Penalty applied to reducer placement when agent is in reducer conflict cooldown.
///
/// Set above trust weight to strongly discourage immediate repeat reducer assignment
/// after merge-conflict churn.
pub(super) fn default_repo_reduce_conflict_cooldown_penalty() -> f64 {
    2.5
}

/// Reducer conflict cooldown window in milliseconds.
///
/// Defaults to the same horizon as idle dynamic retirement so conflict cooling and
/// ephemeral worker retirement operate on consistent time scales.
pub(super) fn default_repo_reduce_conflict_cooldown_ms() -> u64 {
    default_idle_retirement()
}

pub(super) fn default_populi_remote_lease_timeout_ms() -> u64 {
    300_000
}

pub(super) fn default_observer_poll_interval_ms() -> u64 {
    10_000
}

pub(super) fn default_planning_depth() -> String {
    "standard".to_string()
}
pub(super) fn default_parallel_context_enabled() -> bool {
    true
}
pub(super) fn default_context_gather_timeout_secs() -> u64 {
    15
}
pub(super) fn default_min_quality_score() -> f64 {
    0.0
}
pub(super) fn default_context_compression_enabled() -> bool {
    true
}

pub(super) fn default_exec_time_safety_multiplier() -> f64 {
    2.0
}
pub(super) fn default_exec_time_timeout_rate_alert() -> f64 {
    0.20
}
pub(super) fn default_exec_time_default_budget_ms() -> u64 {
    30_000
}
pub(super) fn default_exec_time_history_window_days() -> u32 {
    30
}
pub(super) fn default_local_breakeven_tokens() -> u64 {
    9_100_000
}
pub(super) fn default_research_max_hops() -> u8 {
    3
}
pub(super) fn default_research_quality_target() -> f64 {
    0.8
}
