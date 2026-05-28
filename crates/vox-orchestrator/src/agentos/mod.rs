//! AgentOS core: mutation classification, guardrails, checkpoint hints, intent planning.
//!
//! SSOT: `docs/src/architecture/agentos-ssot-2026.md`.

pub mod checkpoint_engine;
pub mod context_budget_manager;
pub mod guardrail_kernel;
pub mod intent_planner;
pub mod mutation_classifier;
pub mod policy_runtime;
pub mod replay_fast_forward;
pub mod risk_scoring;
