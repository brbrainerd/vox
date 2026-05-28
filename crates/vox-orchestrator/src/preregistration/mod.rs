//! `vox-prereg` — SCIENTIA Phase 2 pre-registration crate.
//!
//! # Responsibilities
//! - Compute Trusty URIs (content-hash-in-URI) for `PreregistrationV1` records.
//! - Sign and verify pre-registrations with Ed25519 via [`vox_crypto`].
//! - Detect analysis-plan deviations between a signed prereg and the actual run.
//! - Provide symbolic verifiers for numeric claims and Bayesian sequential stopping.
//! - Expose [`gate::PreregGate`] — the orchestrator calls this before launching any campaign.
//!
//! # Layer
//! L2 (pure domain logic). No async, no DB, no direct I/O.

pub mod deviation;
pub mod gate;
pub mod living_review;
pub mod reply_window;
pub mod retraction;
pub mod signing;
pub mod symbolic;
pub mod trusty_uri;

pub use deviation::{DeviationDetector, DeviationReport};
pub use gate::{GateResult, PreregGate};
pub use living_review::{DoiVersion, LivingReviewManifest};
pub use reply_window::{ReplyWindowGate, ReplyWindowRecord, WindowStatus, ingest_reply};
pub use retraction::{
    RetractionReason, RetractionRecord, emit_retraction, mark_crossref_propagated,
};
pub use signing::{SignError, Signature, VerifyError, sign_prereg, verify_prereg};
pub use symbolic::{
    BayesianStoppingRule, NumericComparatorVerifier, StopDecision, SymbolicVerdict,
};
pub use trusty_uri::compute_trusty_uri;
