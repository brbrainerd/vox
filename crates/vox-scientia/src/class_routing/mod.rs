//! SCIENTIA Phase E — AI/SWE micro-publication track (non-Atlas) per-class
//! routing configuration.
//!
//! The Atlas track (longitudinal provider-observation papers) is the
//! Finalization Plan's terminal artifact, but the candidate ledger admits
//! four other classes — `algorithmic_improvement`, `reproducibility_infra`,
//! `policy_governance`, `telemetry_trust` — whose publication path is
//! *micro*: shorter reply window, no negative-result quota, distinct venue
//! mix. This crate owns that per-class config.
//!
//! ## Inputs
//!
//! Callers either:
//!
//! - Load YAML matching the shape of
//!   `contracts/scientia/finding-class-defaults.v1.yaml` via
//!   [`load_class_defaults_from_yaml`], or
//! - Construct [`ClassDefaults`] in code (e.g., for tests) and call
//!   [`recommended_venues_for`] / [`reply_window_days_for`] / etc.

pub mod defaults;
pub mod routing;

pub use defaults::{
    builtin_class_defaults, load_class_defaults_from_yaml, ClassDefaults,
    ClassPolicy, ClassRoutingError, FindingClass,
};
pub use routing::{
    atlas_gate_applies_to, recommended_venues_for, reply_window_days_for,
    negative_result_quota_for, critic_allowed_for,
};
