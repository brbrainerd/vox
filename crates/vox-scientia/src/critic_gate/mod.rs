//! SCIENTIA Phase D — solo-author critic gate.
//!
//! Lets a single developer clear the dual-distinct-approver requirement by
//! supplying an **audited LLM critic** as the second approver, while
//! preserving the project's hard rule against
//! GPT-4-grades-GPT-4 self-validation.
//!
//! ## Gate semantics
//!
//! A publication digest passes the approver gate iff one of:
//!
//! 1. **≥2 distinct human approvers** (the existing behavior; this crate
//!    does not change the human path), or
//!
//! 2. **≥1 human approver AND ≥1 [`AuditedLLMCritic`](ApproverRole) approver**
//!    where:
//!    - the venue catalog declares `allows_llm_critic = true`,
//!    - the critic's [`ModelFingerprint`] does **not** appear in the set of
//!      artifact-side model fingerprints (the GPT-4-grades-GPT-4 hole),
//!    - the critic's recommendation is `Approve` (or `ApproveWithNotes`).
//!
//! All four conditions must hold. This crate exposes
//! [`evaluate_gate`] which returns a structured [`GateOutcome`] — callers
//! (vox-db / vox-publisher / vox-cli) translate that to DB row inserts and
//! `next_actions`.

pub mod fingerprint;
pub mod gate;
pub mod role;
pub mod venue;

pub use fingerprint::ModelFingerprint;
pub use gate::{
    evaluate_gate, ApproverRecord, CriticRecommendation, GateInputs, GateOutcome, GateReason,
};
pub use role::ApproverRole;
pub use venue::VenueCriticPolicy;
