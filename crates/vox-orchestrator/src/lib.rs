//! # vox-orchestrator
//!
//! Multi-agent file-affinity queue system for the Vox programming language.
//!
//! Routes tasks to agents based on **file ownership** — ensuring only one agent
//! writes to any given file at a time. Prevents race conditions and lost updates
//! when multiple AI agents work concurrently across a Vox workspace.
//!
//! ## Architecture
//!
//! ```text
//!   User Request
//!       │
//!       ▼
//!   Orchestrator ──► FileAffinityMap ──► route to Agent
//!       │                                    │
//!       ▼                                    ▼
//!   BulletinBoard ◄──── AgentQueue ──► FileLockManager
//! ```
//!
//! ## Features
//!
//! - `runtime` — Actor-based agents using `vox-actor-runtime` Scheduler/Supervisor
//! - `toestub-gate` — Post-task quality validation using TOESTUB (on by default)
//! - `lsp` — LSP diagnostic integration for file ownership info
//!
//! Module-level behavior is documented in each submodule; the crate root is a large re-export surface
//! for the orchestrator binary and integration tests.
//!
//! **Embedding:** the usual MCP host is the `vox-mcp` crate (stdio server), which
//! holds `Orchestrator` plus optional Turso `VoxDb` for Codex/Arca. Training and model SSOT for
//! Mens live in mdBook [`mens-training-ssot.md`](../../../docs/src/architecture/mens-training-ssot.md)
//! (three levels up from `src/` to repo root).

#![allow(clippy::collapsible_if)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unwrap_or_default)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::let_underscore_future)]

/// AgentOS: ACI-aligned mutation classification, guardrails, checkpoint hints.
pub mod agentos;
pub mod attachment_manifest;
// dei_shim extracted to the `vox-dei-shim` crate (A-12). Use `vox_dei_shim::*` instead.
pub use vox_orchestrator_queue::sync_lock;

// mcp_tools/ moved to crate `vox-orchestrator-mcp` in 2026-05-08 reorg Phase 4.
// Use `vox_orchestrator_mcp::*` instead of `vox_orchestrator::mcp_tools::*`.

/// JSON-shaped VCS / workspace views for MCP and CLI parity.
pub mod json_vcs_facade;

pub mod lineage;

/// `VOX_ROUTE_*` alignment helpers for registry/MCP paths (see contracts orchestration routing YAML).
pub mod route_policy;
/// Centralized model-routing policy + Thompson exploration (see `contracts/orchestration/model-routing.v1.yaml`).
pub mod routing;

/// Agent-to-agent messaging types and helpers.
pub mod a2a;
/// Durable orchestrator state logs (drain records, etc.).
pub mod drain_oplog;
/// Submitter-side spot-check sampler for result verification (P5-T5).
pub mod spot_check;
/// File and task affinity groups for routing work to the right agent.
pub use vox_orchestrator_queue::affinity;
/// Developer attention budget tracking — treats pilot attention as a first-class resource (Phase 15).
pub mod attention;
/// VoxDB persistence layer for attention events and agent trust scores (Phase 15).
pub mod attention_tracker;
/// Shared bootstrap helpers for repository-aware orchestrator construction.
pub mod bootstrap;
/// Token and cost budgets per agent and orchestrator-wide tracking.
pub mod budget;
/// Orchestrator-policy token/cost budget gate (D7).
pub mod budget_gate;
/// Shared bulletin board for cross-agent notices.
pub mod bulletin;
/// Prefix-cache hit predictor (D7).
pub mod cache_predictor;
/// Calibration loop: Welford drift detection + Thompson bandit (D10).
pub mod calibration;
/// Host capability probing and merge with `OrchestratorConfig::default_agent_capabilities`.
pub mod capability_probe;
/// Dynamic model catalogs.
pub mod catalog;
pub mod catalog_classifier;
/// Five-signal circuit breaker for doom-loop detection (D6).
pub mod circuit_breaker;
/// DB-backed clarification inbox drain (Codex `a2a_messages`).
pub mod clarification_db_inbox_poll;
/// Context window compaction for long-running agent sessions.
pub mod compaction;
/// Compaction strategy selector by context utilization (D7).
pub mod compaction_trigger;
/// Composite confidence fusion for Socrates invocation decision (D3).
pub mod confidence_fusion;
/// Orchestrator configuration load, merge, and validation.
pub mod config;
/// File conflict detection and resolution hooks.
pub mod conflicts;
/// Ephemeral context store for orchestrator-visible state.
pub mod context;
/// Canonical context envelope contract for cross-surface context payloads.
pub mod context_envelope;
/// Context envelope ingest validation, merge policy, and lifecycle hooks.
pub mod context_lifecycle;
/// Continuation strategies when tasks pause or hand off.
pub mod continuation;
/// Canonical orchestration contract types (v2 payloads, plan surface alignment).
pub mod contract;
/// Agent activity events and pub/sub bus.
pub mod events;
/// Developer mental fatigue monitoring and cognitive pacing.
pub mod fatigue_monitor;
/// Pre/post task gates (including TOESTUB quality checks).
pub mod gate;
pub mod generated;
/// Completion citation grounding against session context envelopes.
pub mod grounding;
/// Affinity group registry built from repository layout.
pub mod groups;
/// Structured handoff payloads between agents.
pub mod handoff;
pub mod legacy;
/// Unified orchestrator policy façade composing all D1–D10.
pub mod orchestrator_policy;
/// Sensitivity classifier and privacy-level-aware routing (D8).
pub mod privacy_classifier;
/// SCIENTIA pre-registration gate — refuses campaigns without a signed prereg (§5.1).
pub mod research_gate;
/// Four-dimension risk scorer and HITL escalation matrix (D5 + D9).
pub mod risk_matrix;
/// Sub-agent dispatch router: spawn vs. inline decision (D4).
pub mod subagent_dispatch;
/// Three-tier model cascade for autonomous model-routing (D1).
pub mod tier_cascade;

/// Entropy-based hallucination detection.
pub mod entropy_scorer;
/// Agent liveness heartbeats and staleness policy.
pub mod heartbeat;
/// Cross-model consensus and judge logic.
pub mod judge_model;
/// Per-file lock manager for exclusive writer access.
pub use vox_orchestrator_queue::locks;
/// Unified task hopper — intake funnel for developer-sourced work (Hp-T1).
pub mod hopper;
/// MCP tool surface and plugin-skills bridge stubs (implementation pending SP6).
pub mod mcp_tools;
/// Long-term and daily agent memory backed by Codex when enabled.
pub mod memory;
/// Live mesh node registry — authoritative in-memory topology view for the dashboard.
pub mod mesh;
/// Populi control-plane poll loop shared by MCP and `vox-orchestrator-d`.
#[cfg(feature = "populi-transport")]
pub mod mesh_federation_poll;
#[cfg(not(feature = "populi-transport"))]
#[path = "mesh_federation_poll_noop.rs"]
pub mod mesh_federation_poll;
/// LLM model registry and provider configuration.
pub mod models;
/// Lightweight AI usage / behavior monitor hooks.
pub mod monitor;
/// File and task observer: real-time structural health evaluation (Wave 2).
pub mod observer;
/// Append-only operation log for durable orchestration history.
pub use vox_orchestrator_queue::oplog;
pub mod mode;
/// TCP JSON-line orchestrator daemon (`vox-orchestrator-d`) and client helpers.
pub mod orch_daemon;
pub mod orchestration_feature_flags;
/// Core multi-agent orchestrator implementation.
pub mod orchestrator;
/// PII-aware redacting filter for sensitive data.
pub mod pii_filter;
/// Dynamic planning domain (router, synthesis, policies, replanning).
pub mod planning;
/// Read-only mens HTTP federation snapshot types (filled by MCP / embedders).
pub mod populi_federation;
/// Populi remote execution gating and lease-class helpers.
pub mod populi_remote;
/// PII-aware privacy routing and data isolation policies.
pub mod privacy_router;
/// Question/answer routing between agents.
pub mod qa;
/// Priority task queues and overflow handling.
pub mod queue;
/// Load-based agent scale-up/down suggestions.
pub mod rebalance;
/// Reconstruction campaign tiers, evidence scoring, and resumable campaign state.
pub mod reconstruction;
pub mod registry_model_resolve;
pub mod retrieval;
/// JSON schemas for persisted orchestrator artifacts.
pub mod schema;
/// Task path scopes and enforcement guards.
pub mod scope;
/// Orchestrator ↔ `vox-search` adapters (lexical fallback, future native retrieval).
pub mod search_bridge;
/// Security policies, audit log, and guard checks.
pub mod security;
/// Embeddings, routing, scaling, policy, and gateway services.
pub mod services;
/// Durable agent session records and managers.
pub mod session;
/// Workspace snapshots and content hashing for diffs.
pub mod snapshot;
/// Socrates evidence gate and shared task context types.
pub mod socrates;
/// Serializable orchestrator state snapshots for UI and persistence.
pub mod state;
/// Rolling summarization of agent interactions.
pub mod summary;
/// Cryptographic receipts for tool execution grounding.
pub mod tool_receipt;
pub mod topology;
/// Core identifiers, tasks, messages, and shared value types.
pub mod types;
/// Aggregated LLM usage, quotas, and cost accounting.
pub mod usage;
/// Provider daily quota policy (dynamic + defaults).
pub mod usage_policy;
/// Per-agent workspace views and pending change tracking.
pub mod workspace;

/// TOESTUB-based output validation gate integration.
#[cfg(feature = "toestub-gate")]
pub mod validation;

/// Aggregate multi-tier verification signals.
pub mod victory;

/// In-process `SkillRuntime` implementation (P0-T7).
pub mod skill_runtime_inproc;

/// Tokio scheduler bridge for running tasks against a live `crate::Orchestrator`.
#[cfg(feature = "runtime")]
pub mod runtime;

/// LSP-facing helpers for ownership and diagnostics surfacing.
#[cfg(feature = "lsp")]
pub mod lsp;

// Re-export key public types for ergonomic access.
pub use a2a::{
    A2ARoute, DbA2AMessage, acknowledge_db_message, poll_inbox_from_db, prune_old_a2a_messages,
    send_to_db,
};
pub use attachment_manifest::{AttachmentEntry, AttachmentManifest, VisualSegment};
pub use attention::{
    ActionDescriptor, AgentTrustScore, ApprovalOutcome, ApprovalTier, AttentionBudget,
    AttentionEvent, AttentionEventType, DEFAULT_ATTENTION_BUDGET_MS, DEFAULT_INTERRUPT_COST_MS,
    FocusDepth, InterruptionChannel, InterruptionDecision, InterruptionSignals, NasaTlxWeights,
    TierGateConfig, TrustTier, classify_tier, compute_attention_cost_ms, decision_entropy_bits,
    evaluate_interruption, scaled_interrupt_cost_ms,
};
pub use bootstrap::{
    RepoScopedOrchestratorBuild, build_repo_scoped_orchestrator,
    build_repo_scoped_orchestrator_for_repository, discover_repository_from_cwd,
    repo_scoped_orchestrator_config, repo_scoped_orchestrator_parts,
};
pub use budget::{AgentBudgetAllocation, BudgetManager, BudgetSignal};
pub use compaction::{
    CompactionConfig, CompactionEngine, CompactionResult, CompactionStrategy, Turn,
};
pub use config::{OrchestratorConfig, ScalingProfile};
pub use conflicts::{ConflictId, ConflictManager, ConflictResolution, FileConflict};
pub use context::ContextStore;
pub use context_envelope::{
    ContextAgentOsHints, ContextBudget, ContextCaptureMode, ContextConflictClass,
    ContextConflictPolicy, ContextContent, ContextDerivedRef, ContextEnvelope, ContextEnvelopeType,
    ContextFact, ContextFreshnessTier, ContextInjectionMode, ContextLineage, ContextMergeStrategy,
    ContextPriority, ContextProvenance, ContextRetrievalCostClass, ContextSafety,
    ContextSourcePlane, ContextSubject, ContextTrust, ContextTrustTier,
};
pub use continuation::{ContinuationEngine, ContinuationStrategy};
pub use contract::{
    DEI_PLAN_METHODS_NEW_REPLAN_STATUS, MCP_PLAN_TOOL_NAMES, OrchestrationContractVersion,
    OrchestrationMigrationFlags, SessionContractEnvelope, TaskCapabilityHints,
    plan_tool_daemon_alignment_valid,
};
pub use entropy_scorer::{calculate_entropy, score_confidence, semantic_drift_sigma};
pub use events::{AgentActivity, AgentEvent, AgentEventKind, BuildStageKind, EventBus, MeshAction};
pub use gate::{BudgetGate, Gate, GateResult};
pub use generated::agent_harness::{
    Adapter as HarnessAdapter, AgentHarnessSpec, CompletionGate as HarnessGate,
    Contracts as HarnessContracts, FailureTaxonomy as HarnessFailureMode,
    RequiredOutput as HarnessArtifactSpec, Role as HarnessRole, Stage as HarnessStage,
    State as HarnessState, Subject as HarnessSubject,
};
pub use groups::{AffinityGroup, AffinityGroupRegistry, load_from_config};
pub use handoff::{
    HandoffInvariantError, HandoffPayload, execute_handoff, validate_handoff_invariants,
};
pub use heartbeat::{
    AgentHeartbeat, HeartbeatMonitor, HeartbeatPolicy, StalenessLevel, evict_dead_heartbeats,
    live_nodes_from_db, persist_heartbeat,
};
pub use judge_model::{JudgeModel, JudgePolicy, JudgeVerdict};
pub use legacy::harness_hand::{
    HarnessIngestExpectations, apply_harness_subject_defaults, validate_agent_harness_ingest,
};
pub use memory::{LongTermMemory, MemoryConfig, MemoryManager, SearchHit};
pub use monitor::AiMonitor;
pub use observer::{ObservationSummary, Observer, ObserverPolicy};
pub use oplog::{OpLog, OperationEntry, OperationId, OperationKind};
pub use orchestrator::{Orchestrator, TaskTraceStep};
pub use pii_filter::PiiFilter;
pub use planning::{
    ExecutionPolicy, PlanNode, PlanSessionRecord, PlanStatus, PlanVersionRecord, PlanningMode,
    PlanningStrategy, PlanningTaskMeta, ReplanTrigger, RouterEvaluation,
};
pub use populi_federation::{
    PopuliNodeBrief, PopuliRoutingHintUpdate, RemotePopuliRoutingHint, RemotePopuliSnapshot,
};
pub use privacy_router::{
    PrivacyLevel, PrivacyRouter, PrivacyRoutingPolicy, model_supports_privacy_local_inference,
};
pub use reconstruction::{
    AgentExecutionRole, CampaignMemorySnapshot, ReconstructionArtifactKind,
    ReconstructionArtifactRecord, ReconstructionBenchmarkKpis, ReconstructionBenchmarkTier,
    ReconstructionEvidence, ReconstructionShardBoundary, RepoReconstructionSpec,
    VerificationFailureKind, VerificationLayerStatus, campaign_context_prefix,
};
pub use registry_model_resolve::{
    RegistryModelResolutionParams, infer_prompt_capability_hints,
    resolve_model_with_registry_fallbacks,
};
pub use scope::{ScopeCheckResult, ScopeEnforcement, ScopeGuard};
pub use security::{
    AuditEntry, AuditLog, AuditResult, PolicyRule, SecurityAction, SecurityGuard, SecurityPolicy,
};
pub use services::{
    CampaignSchedulePlan, CampaignScheduler, CampaignSchedulingMode, MessageGateway,
    PolicyCheckResult, PolicyEngine, PolicyTrustRelax, RouteResult, RoutingService, ScalingAction,
    ScalingService,
};
pub use session::{Session, SessionConfig, SessionManager, SessionState};
pub use snapshot::{SnapshotId, SnapshotStore};
pub use socrates::{
    SessionRetrievalEnvelope, SocratesGateOutcome, SocratesTaskContext, evaluate_socrates_gate,
    session_context_envelope_key,
};
pub use summary::SummaryManager;
pub use tool_receipt::{ReceiptValidationResult, ToolReceipt, ToolReceiptLedger};
pub use topology::{
    AgentDelegationBinding, AgentRole, AgentTopologyNode, AgentTopologySnapshot, DelegationEdge,
    DynamicSpawnContext, TopologyGap,
};
pub use types::{
    A2AMessage, A2AMessageType, AccessKind, AgentId, AgentIdGenerator, AgentMessage, AgentTask,
    BatchId, Budget, CompletionAttestation, CorrelationId, CorrelationIdGenerator, FileAffinity,
    MessageEnvelope, MessageId, MessagePriority, TaskCategory, TaskDescriptor, TaskEnqueueHints,
    TaskId, TaskIdGenerator, TaskPriority, TaskStatus, ThreadId, VcsContext, now_unix_ms,
};
pub use vox_db::store::types::VictoryCondition;
pub use vox_db::store::{
    ObservationReport, ObserverAction, TestDecision, TestDecisionPolicy, TierResult, VictoryVerdict,
};
pub use vox_search::{HybridSearchHit, MemorySearchEngine};

pub use usage::LlmUsageKey;
pub use workspace::{AgentWorkspace, ChangeId, ChangeStatus, WorkspaceManager};

// ── Orchestrator policy (D1–D10) re-exports ───────────────────────────────────
// Single ergonomic surface for the autonomous orchestration policy program.
// See `docs/superpowers/plans/orchestrator/2026-05-08-orchestrator-master-plan.md`.
pub use mode::{ExecutionModeProfile, InferenceConfig, Modalities, QualityLevel, TierProfile};
pub use orchestration_feature_flags::OrchestrationFeatureFlags;
pub use orchestrator_policy::{
    OrchestratorPolicy, OrchestratorPolicyConfig, PolicyContext, PolicyDecision,
};

pub mod preregistration;
