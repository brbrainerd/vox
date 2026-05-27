use serde::{Deserialize, Serialize};

use crate::compaction::CompactionConfig;
use crate::contract::{OrchestrationMigrationFlags, TaskCapabilityHints};
use crate::memory::MemoryConfig;
use crate::scope::ScopeEnforcement;
use crate::session::SessionConfig;
use crate::types::TaskPriority;
use vox_orchestrator_types::socrates_policy::ConfidencePolicyOverride;

use super::defaults::*;
use super::enums::{CostPreference, OverflowStrategy, ScalingProfile};
use super::news::NewsConfig;
use super::scientia_research_mesh::ScientiaResearchMeshConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct OrchestratorConfig {
    /// Whether the orchestrator is enabled (default: true).
    pub enabled: bool,
    /// Maximum number of concurrent agents (default: 8).
    pub max_agents: usize,
    /// Default priority for new tasks (default: Normal).
    pub default_priority: TaskPriority,
    /// How to handle queue overflow (default: SpawnNewAgent).
    pub queue_overflow_strategy: OverflowStrategy,
    /// Lock timeout in milliseconds (default: 30000).
    pub lock_timeout_ms: u64,
    /// Bulletin board broadcast channel capacity (default: 256).
    pub bulletin_capacity: usize,
    /// Whether to fall back to a single agent when routing is ambiguous (default: true).
    pub fallback_to_single_agent: bool,
    /// Automated testing requirement rules engine.
    #[serde(default)]
    pub test_decision_policy: crate::planning::TestDecisionPolicy,
    /// Whether to run TOESTUB validation after each completed task (default: true).
    pub toestub_gate: bool,
    /// When true, task completion runs the behavioral gate (nested workspace `cargo test` / npm test).
    /// Disabled in [`OrchestratorConfig::for_testing`] so integration tests do not recurse into Cargo.
    #[serde(default = "default_true")]
    pub behavioral_gate_on_complete: bool,
    /// When true, completion verifies Markdown writes via nested `vox ci check-links` (default: true).
    /// Disabled in [`OrchestratorConfig::for_testing`] so integration/stress tests avoid subprocess-heavy audits.
    #[serde(default = "default_true")]
    pub completion_markdown_link_audit_enabled: bool,
    /// Maximum number of times a task can be re-routed due to validation failures (default: 3).
    pub max_debug_iterations: u8,
    /// TOESTUB-specific max auto-debug retries (default: 3).
    #[serde(default = "default_max_toestub_debug_iterations")]
    pub max_toestub_debug_iterations: u8,
    /// Socrates-specific max requeue retries (default: 3).
    #[serde(default = "default_max_socrates_debug_iterations")]
    pub max_socrates_debug_iterations: u8,
    /// Emit Socrates gate decisions to logs without blocking completion (default: false).
    #[serde(default = "default_false")]
    pub socrates_gate_shadow: bool,
    /// When true, a non-answer Socrates risk decision requeues the task for remediation (default: false).
    #[serde(default = "default_false")]
    pub socrates_gate_enforce: bool,
    /// Blend `agent_reliability` (Arca V10) into routing when a VoxDb is attached (default: false).
    #[serde(default = "default_false")]
    pub socrates_reputation_routing: bool,
    /// Optional Socrates confidence thresholds merged onto `ConfidencePolicy::workspace_default`.
    #[serde(default)]
    pub socrates_policy: Option<ConfidencePolicyOverride>,
    /// Prefer the dedicated research synthesis lane when orchestrator wiring exposes it (Lane G).
    #[serde(default)]
    pub research_model_enabled: bool,
    /// Weight applied to Arca `agent_reliability` when blending into routing scores (default: 1.0).
    #[serde(default = "default_socrates_reputation_weight")]
    pub socrates_reputation_weight: f64,
    /// When true and Codex `agent_reliability` for the agent meets
    /// [`Self::trust_gate_relax_min_reliability`], **Socrates enforce**, **completion grounding enforce**,
    /// and **strict scope** may skip requeue / denial (see [`crate::services::PolicyEngine`] and `complete_task`).
    #[serde(default = "default_false")]
    pub trust_gate_relax_enabled: bool,
    /// Minimum reliability (0.0–1.0) for [`Self::trust_gate_relax_enabled`] (default: 0.85).
    #[serde(default = "default_trust_gate_relax_min_reliability")]
    pub trust_gate_relax_min_reliability: f64,
    /// Log level for orchestrator events (default: "info").
    pub log_level: String,
    /// Global system idle timeout in milliseconds (default: 600000 / 10min).
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_ms: u64,
    /// Default task execution timeout in milliseconds (default: 1800000 / 30min).
    #[serde(default = "default_task_timeout")]
    pub task_timeout_ms: u64,

    // ── Phase 1: New fields ──────────────────────────────────
    /// Heartbeat check interval in milliseconds (default: 5000).
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_ms: u64,
    /// Threshold in milliseconds before an agent is considered stale (default: 60000).
    ///
    /// Also used when MCP embeds build [`crate::populi_federation::RemotePopuliRoutingHint`]:
    /// Populi nodes whose `last_seen_unix_ms` is older than this at poll time get
    /// `heartbeat_stale` and are excluded from experimental federation routing signals.
    #[serde(default = "default_stale_threshold")]
    pub stale_threshold_ms: u64,
    /// Whether auto-continuation is enabled (default: true).
    #[serde(default = "default_true")]
    pub auto_continue_enabled: bool,
    /// Cooldown between auto-continuations per agent in ms (default: 30000).
    #[serde(default = "default_continuation_cooldown")]
    pub continuation_cooldown_ms: u64,
    /// Maximum auto-continuations before requiring manual intervention (default: 5).
    #[serde(default = "default_max_auto_continuations")]
    pub max_auto_continuations: u32,
    /// How strictly to enforce agent scope boundaries (default: Warn).
    #[serde(default)]
    pub scope_enforcement: ScopeEnforcement,
    /// Event bus capacity (default: 1024).
    #[serde(default = "default_event_capacity")]
    pub event_bus_capacity: usize,
    /// Default GPU / capability hints for newly spawned agent queues.
    #[serde(default)]
    pub default_agent_capabilities: TaskCapabilityHints,
    /// MCP/CLI wire migration toggles (v2 contract hints, legacy fallback).
    #[serde(default)]
    pub orchestration_migration: OrchestrationMigrationFlags,

    // ── Phase 12: Scaling & Cost ─────────────────────────────
    /// Baseline multiplier for safety when computing execution time budgets (default: 1.5).
    #[serde(default = "default_execution_time_budget_multiplier")]
    pub execution_time_budget_multiplier: f64,
    /// Absolute capitalistic cost allowed across tasks before blocking (default: 50,000 micros).
    #[serde(default = "default_financial_cost_budget_micros")]
    pub financial_cost_budget_micros: i64,

    /// Minimum number of concurrent agents (default: 1).
    #[serde(default = "default_min_agents")]
    pub min_agents: usize,
    /// Number of queued tasks per agent to trigger scaling (default: 5).
    #[serde(default = "default_scaling_threshold")]
    pub scaling_threshold: usize,
    /// Time an idle dynamic agent lives before retirement in ms (default: 300000 / 5min).
    #[serde(default = "default_idle_retirement")]
    pub idle_retirement_ms: u64,
    /// Whether dynamic scaling is enabled (default: false).
    #[serde(default = "default_false")]
    pub scaling_enabled: bool,
    /// Preference for cost vs performance (default: Economy — free-by-default product directive).
    #[serde(default = "default_cost_preference")]
    pub cost_preference: CostPreference,
    /// Number of ticks to look back for predictive scaling (default: 5).
    #[serde(default = "default_lookback_ticks")]
    pub scaling_lookback_ticks: usize,
    /// Weight of system resource usage in load calculation (0.0 to 1.0, default: 0.3).
    #[serde(default = "default_resource_weight")]
    pub resource_weight: f64,
    /// Baseline multiplier for CPU usage in the load calculation (default: 0.7).
    #[serde(default = "default_cpu_multiplier")]
    pub resource_cpu_multiplier: f64,
    /// Baseline multiplier for Memory usage in the load calculation (default: 0.3).
    #[serde(default = "default_mem_multiplier")]
    pub resource_mem_multiplier: f64,
    /// Exponent to apply to the final resource factor, allowing exponential scaling (default: 1.0).
    #[serde(default = "default_resource_exponent")]
    pub resource_exponent: f64,
    /// User-governable scaling profile (conservative / balanced / aggressive).
    #[serde(default)]
    pub scaling_profile: ScalingProfile,
    /// Max number of agents to spawn in one scaling tick (default: 1).
    #[serde(default = "default_max_spawn_per_tick")]
    pub max_spawn_per_tick: usize,
    /// Cooldown in ms between scale-up actions (default: 5000).
    #[serde(default = "default_scaling_cooldown_ms")]
    pub scaling_cooldown_ms: u64,
    /// Number of Urgent tasks on a single agent that triggers an automatic rebalance (default: 3).
    /// Set to 0 to disable urgent auto-rebalance.
    #[serde(default = "default_urgent_rebalance_threshold")]
    pub urgent_rebalance_threshold: usize,

    // ── OpenClaw-Inspired Features ───────────────────────────────────────
    /// Configuration for the context compaction engine.
    #[serde(default)]
    pub compaction: CompactionConfig,
    /// Configuration for the persistent memory system.
    #[serde(default)]
    pub memory: MemoryConfig,
    /// Configuration for the session lifecycle manager.
    #[serde(default)]
    pub session: SessionConfig,
    /// Optional mens HTTP control plane base URL (`GET /v1/populi/nodes`) for read-only status federation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub populi_control_url: Option<String>,
    /// Optional Ollama-shaped inference base (`POPULI_URL` target), e.g. `http://127.0.0.1:11434` for Schola or Ollama.app.
    /// From `Vox.toml` `[mesh].inference_base_url` (env `VOX_ORCHESTRATOR_POPULI_INFERENCE_BASE_URL` overrides in `merge_env_overrides`).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "mesh_inference_base_url"
    )]
    pub populi_inference_base_url: Option<String>,
    /// Optional mens cluster / tenancy id from `Vox.toml` `[mens].scope_id` or `VOX_MESH_SCOPE_ID` (env wins).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "mesh_scope_id"
    )]
    pub populi_scope_id: Option<String>,
    /// Background poll interval (seconds) for MCP populi federation cache; `0` disables the poller.
    #[serde(
        default = "default_populi_poll_interval_secs",
        alias = "mesh_poll_interval_secs"
    )]
    pub populi_poll_interval_secs: u64,
    /// HTTP client timeout (milliseconds) for populi control plane `GET /v1/populi/nodes`.
    #[serde(
        default = "default_populi_http_timeout_ms",
        alias = "mesh_http_timeout_ms"
    )]
    pub populi_http_timeout_ms: u64,
    /// Experimental: use remote populi node labels when scoring routes (no remote task execution).
    #[serde(default = "default_false", alias = "mesh_routing_experimental")]
    pub populi_routing_experimental: bool,
    /// When [`Self::populi_routing_experimental`] is on and federation-schedulable remote node count
    /// **drops** after a hint refresh, run [`crate::orchestrator::Orchestrator::rebalance`] once
    /// (load work-steering across **local** queues; does not replay `RoutingService::route` per task).
    #[serde(
        default = "default_false",
        alias = "mesh_rebalance_on_remote_schedulable_drop"
    )]
    pub populi_rebalance_on_remote_schedulable_drop: bool,
    /// When [`Self::populi_routing_experimental`] is on and federation-schedulable remote node count
    /// **drops**, re-run `RoutingService::route` for each **queued** (not in-progress) task and move
    /// tasks whose preferred agent changed (after optional rebalance). Default off.
    #[serde(
        default = "default_false",
        alias = "mesh_replay_queued_routes_on_remote_schedulable_drop"
    )]
    pub populi_replay_queued_routes_on_remote_schedulable_drop: bool,
    /// Experimental: apply training-task specific placement boosts/penalties.
    #[serde(
        default = "default_false",
        alias = "mesh_training_routing_experimental"
    )]
    pub populi_training_routing_experimental: bool,
    /// Soft budget-pressure scalar applied to expensive training placements (0.0-1.0).
    #[serde(default = "default_populi_training_budget_pressure")]
    pub populi_training_budget_pressure: f64,
    /// Experimental: allow remote task-envelope dispatch over populi A2A relay with local fallback.
    #[serde(default = "default_false", alias = "mesh_remote_execute_experimental")]
    pub populi_remote_execute_experimental: bool,
    /// Receiver **numeric** agent id (string form in env/TOML) for experimental remote relay.
    #[serde(default, alias = "mesh_remote_execute_receiver_agent")]
    pub populi_remote_execute_receiver_agent: Option<String>,
    /// Sender **numeric** agent id for experimental remote relay (defaults to `1` when unset/invalid).
    #[serde(default, alias = "mesh_remote_execute_sender_agent")]
    pub populi_remote_execute_sender_agent: Option<String>,
    /// Poll interval (seconds) for **`remote_task_result`** inbox draining when experimental remote execute is on.
    /// `0` disables the dedicated poller. Independent of [`Self::populi_poll_interval_secs`].
    #[serde(
        default = "default_populi_remote_result_poll_interval_secs",
        alias = "mesh_remote_result_poll_interval_secs"
    )]
    pub populi_remote_result_poll_interval_secs: u64,
    /// Max number of `remote_task_result` messages processed per poll tick (minimum 1).
    #[serde(default = "default_populi_remote_result_max_messages_per_poll")]
    pub populi_remote_result_max_messages_per_poll: usize,
    /// Poll interval (seconds) for remote worker inbox ticks (`remote_task_envelope` consumer).
    /// `0` disables worker polling while leaving result polling enabled.
    #[serde(
        default = "default_populi_remote_worker_poll_interval_secs",
        alias = "mesh_remote_worker_poll_interval_secs"
    )]
    pub populi_remote_worker_poll_interval_secs: u64,
    /// Single-owner remote path: await mesh relay before local enqueue when the task matches
    /// [`Self::populi_remote_lease_gated_roles`].
    #[serde(default = "default_false", alias = "mesh_remote_lease_gating_enabled")]
    pub populi_remote_lease_gating_enabled: bool,
    /// Roles that use lease-style gating when [`Self::populi_remote_lease_gating_enabled`] is true.
    /// Empty means no task matches (configure explicitly).
    #[serde(default, alias = "mesh_remote_lease_gated_roles")]
    pub populi_remote_lease_gated_roles: Vec<crate::reconstruction::AgentExecutionRole>,
    /// Timeout in milliseconds for authoritative Populi remote leases (default: 300000 / 5min).
    #[serde(
        default = "default_populi_remote_lease_timeout_ms",
        alias = "mesh_remote_lease_timeout_ms"
    )]
    pub populi_remote_lease_timeout_ms: u64,
    /// When true, MCP tool LLM calls collapse system/user turns into a single string
    /// formatted with `<|im_start|>` markers instead of JSON message arrays.
    #[serde(default = "default_false")]
    pub chatml_strict: bool,
    /// Enable dynamic planning mode (router + plan execution bridge).
    #[serde(default = "default_false")]
    pub planning_enabled: bool,
    /// Enable intake router classification at ingress.
    #[serde(default = "default_false")]
    pub planning_router_enabled: bool,
    /// Enable branch-based replanning after qualifying failures.
    #[serde(default = "default_false")]
    pub planning_replan_enabled: bool,
    /// Allow workflow runtime handoff path from planner.
    #[serde(default = "default_false")]
    pub planning_workflow_handoff_enabled: bool,
    /// Use LLM to synthesize plan nodes instead of heuristics
    #[serde(default = "default_false")]
    pub planning_llm_synthesis_enabled: bool,
    /// Compute planning decisions but keep direct execution path.
    #[serde(default = "default_false")]
    pub planning_shadow_mode: bool,
    /// Enable `planning_mode=auto` behavior for goal ingress.
    #[serde(default = "default_false")]
    pub planning_auto_mode_enabled: bool,
    /// Rollout percentage for auto planning (0-100).
    #[serde(default)]
    pub planning_rollout_percent: u8,
    #[serde(default = "default_planning_depth")]
    pub planning_depth: String,
    #[serde(default = "default_parallel_context_enabled")]
    pub parallel_context_enabled: bool,
    #[serde(default = "default_context_gather_timeout_secs")]
    pub context_gather_timeout_secs: u64,
    #[serde(default = "default_min_quality_score")]
    pub min_quality_score: f64,
    #[serde(default = "default_context_compression_enabled")]
    pub context_compression_enabled: bool,
    /// When true (default), plan adequacy is recorded in lineage/telemetry only; enqueue behavior is unchanged.
    #[serde(default = "default_true")]
    pub plan_adequacy_shadow: bool,
    /// When true, goals that produce structurally thin native plans are rejected at enqueue (after quality gate).
    #[serde(default = "default_false")]
    pub plan_adequacy_enforce: bool,

    /// When true, validate [`crate::ContextEnvelope`] at MCP/orchestrator ingress and log violations without blocking.
    ///
    /// Persisted/config precedence vs session overrides: see **`docs/src/reference/env-vars.md`** (`VOX_ORCHESTRATOR_*` /
    /// orchestrator TOML fields).
    #[serde(default = "default_false")]
    pub context_lifecycle_shadow: bool,
    /// When true, reject invalid or cross-boundary context envelopes at ingress (merge + validation failures block the operation).
    ///
    /// Same precedence story as [`Self::context_lifecycle_shadow`]; telemetry contract
    /// `contracts/orchestration/context-lifecycle-telemetry.schema.json`.
    #[serde(default = "default_false")]
    pub context_lifecycle_enforce: bool,

    /// Log completion citation grounding mismatches (`[[voxcite:...]]` / `evidence_citations`).
    #[serde(default = "default_false")]
    pub completion_grounding_shadow: bool,
    /// Requeue tasks when declared citations are absent from the session context envelope.
    #[serde(default = "default_false")]
    pub completion_grounding_enforce: bool,

    // ── Phase 15: Attention Budget ─────────────────────────────────────────────
    /// Enable attention budget tracking. Default: false (shadow/observe mode).
    #[serde(default = "default_false")]
    pub attention_enabled: bool,
    /// Pilot attention budget per session period in ms. Default: 3_600_000 (1 hr).
    #[serde(default = "default_attention_budget_ms")]
    pub attention_budget_ms: u64,
    /// Ratio of budget that triggers AttentionHigh signal. Default: 0.7.
    #[serde(default = "default_attention_alert_threshold")]
    pub attention_alert_threshold: f64,
    /// Baseline interrupt recovery cost in ms. Default: 23_250 (Gloria Mark).
    #[serde(default = "default_attention_interrupt_cost_ms")]
    pub attention_interrupt_cost_ms: u64,
    /// EWMA alpha for trust score updates. Default: 0.1.
    #[serde(default = "default_trust_ewma_alpha")]
    pub trust_ewma_alpha: f64,
    /// Exploration fallback epsilon for routing decisions when attention_enabled is true. Default: 0.05.
    #[serde(default = "default_routing_exploration_epsilon")]
    pub routing_exploration_epsilon: f64,
    /// Minimum outcomes for Untrusted → Provisional. Default: 5.
    #[serde(default = "default_trust_provisional_threshold")]
    pub trust_provisional_threshold: u32,
    /// Minimum outcomes for Provisional → Trusted. Default: 20.
    #[serde(default = "default_trust_trusted_threshold")]
    pub trust_trusted_threshold: u32,
    /// Minimum trust score for auto-approve eligibility. Default: 0.85.
    #[serde(default = "default_trust_auto_approve_min")]
    pub trust_auto_approve_min: f64,
    /// Routing weight applied to trust scores in step 3d. Default: 2.0.
    #[serde(default = "default_attention_trust_routing_weight")]
    pub attention_trust_routing_weight: f64,
    /// Disqualifying floor for task completion trust rollups during routing.
    #[serde(default = "default_trust_task_completion_floor")]
    pub trust_task_completion_floor: f64,
    /// Weight for task completion trust rollups during routing.
    #[serde(default = "default_trust_task_completion_weight")]
    pub trust_task_completion_weight: f64,
    /// Routing bonus for shard-role specialization (`[PHASE:SHARD_*]`, `[PHASE:REDUCE]`).
    #[serde(default = "default_repo_shard_specialization_weight")]
    pub repo_shard_specialization_weight: f64,
    /// Penalty per recent shard validation failure.
    #[serde(default = "default_repo_shard_validation_failure_penalty")]
    pub repo_shard_validation_failure_penalty: f64,
    /// Penalty while an agent is in reducer conflict cooldown.
    #[serde(default = "default_repo_reduce_conflict_cooldown_penalty")]
    pub repo_reduce_conflict_cooldown_penalty: f64,
    /// Cooldown window in ms applied after reducer conflict churn.
    #[serde(default = "default_repo_reduce_conflict_cooldown_ms")]
    pub repo_reduce_conflict_cooldown_ms: u64,
    /// NASA TLX subscale weights for attention cost computation.
    /// Defaults to validated pilot-study values (mental=0.35, temporal=0.25, etc.).
    #[serde(default)]
    pub attention_tlx_weights: crate::attention::NasaTlxWeights,
    /// Approval tier gate thresholds. Override to tune auto-approve graduation.
    #[serde(default)]
    pub tier_gate: crate::attention::TierGateConfig,
    /// Dynamic interruption calibration overrides by channel and context pressure.
    #[serde(default)]
    pub interruption_calibration: crate::attention::InterruptionCalibrationConfig,
    /// Configuration for the unified news publisher (docs/news/ → RSS/X/GitHub).
    #[serde(default)]
    pub news: NewsConfig,
    /// SCIENTIA research mesh on-disk intake and optional promoted-ledger consumer.
    #[serde(default)]
    pub scientia_research_mesh: ScientiaResearchMeshConfig,

    // ── Phase 16: OAPV Observer ─────────────────────────────────────────────
    /// Enable the autonomous Observer loop (OAPV). Default: false.
    #[serde(default = "default_false")]
    pub observer_enabled: bool,
    /// Model to use for routine observation inference. Configurable from VS Code.
    #[serde(default)]
    pub observer_model: Option<String>,
    /// Background poll interval (milliseconds) for the Observer loop. Default: 10_000.
    #[serde(default = "default_observer_poll_interval_ms")]
    pub observer_poll_interval_ms: u64,

    // ── Phase 17: Execution Time Budgeting ─────────────────────────────────────
    /// Enable per-tool execution time budget learning. Default: true.
    #[serde(default = "default_true")]
    pub exec_time_budget_enabled: bool,
    /// Safety multiplier applied to P90 to derive recommended_budget_ms. Default: 2.0.
    #[serde(default = "default_exec_time_safety_multiplier")]
    pub exec_time_safety_multiplier: f64,
    /// Timeout rate threshold for ToolLatencyHigh signal. Default: 0.20.
    #[serde(default = "default_exec_time_timeout_rate_alert")]
    pub exec_time_timeout_rate_alert: f64,
    /// Default budget ms when no history exists. Default: 30_000.
    #[serde(default = "default_exec_time_default_budget_ms")]
    pub exec_time_default_budget_ms: u64,
    /// History window in days for agent_exec_history queries. Default: 30.
    #[serde(default = "default_exec_time_history_window_days")]
    pub exec_time_history_window_days: u32,

    // ── AgentOS (ACI envelopes + guardrails) ─────────────────────────────────
    /// When true, MCP tool JSON responses include a validated sibling `aci` block.
    /// Default: `true` since v0.6 (CR-L5; council D20, 2026-05-15).
    #[serde(default = "default_true")]
    pub agentos_aci_envelope_enabled: bool,
    /// When true, [`crate::agentos::guardrail_kernel`] runs before mutating / dangerous tools.
    #[serde(default = "default_false")]
    pub agentos_guardrail_kernel_enabled: bool,
    /// When true, orchestrator may emit sparse checkpoint hints for telemetry consumers.
    #[serde(default = "default_false")]
    pub agentos_checkpoint_hints_enabled: bool,

    /// Daily local token threshold before enforcing local-tier inference. Default: 9.1M.
    #[serde(default = "default_local_breakeven_tokens")]
    pub local_breakeven_tokens: u64,

    // ── Research Localization ──────────────────────────────────
    /// Maximum retrieval hops for iterative research loops (default: 3).
    #[serde(default = "default_research_max_hops")]
    pub research_max_hops: u8,
    /// Automated quality check on retrieved evidence before synthesis (default: true).
    #[serde(default = "default_true")]
    pub research_quality_gate_enabled: bool,
    /// Minimum evidence quality [0, 1] to skip iterative hops (default: 0.8).
    #[serde(default = "default_research_quality_target")]
    pub research_quality_target: f64,
    /// Persistent HMAC key (32 bytes hex) for the tool receipt ledger.
    /// If empty, a new key is generated each session (ephemeral).
    #[serde(default)]
    pub tool_ledger_key: String,
    /// Optional configuration for the orchestrator-policy budget gate (D7).
    #[serde(default)]
    pub budget_gate_config: Option<crate::budget_gate::BudgetGateConfig>,
}
