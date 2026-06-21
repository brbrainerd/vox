//! `vox mens eval-gate` — check training/eval metrics against policy thresholds.
//!
//! Reads `mens/config/eval-gates.yaml` and validates run artifacts.
//! Exits non-zero if any blocking gate fails.

// Re-exports are consumed by feature-gated `populi` / `ai::train` call sites; default `cargo check` may not reference every name.
#![allow(unused_imports)]

pub mod baseline;
pub mod bfcl;
mod check_run;
mod io;
pub mod leakage;
mod legacy;
pub mod planning_eval;
mod policy;
mod run_gate;
#[cfg(test)]
mod tests;

pub use baseline::{BaselineEntry, BaselineReport, beat_base, load_baseline, save_baseline};
pub use bfcl::{BfclGate, check_bfcl};
pub use check_run::{GateResult, check_run};
pub use leakage::{SplitManifest, assert_no_leakage, load_split_manifest};
#[cfg(all(feature = "mens-dei", feature = "gpu"))]
pub(crate) use legacy::{
    LEGACY_TRAIN_POST_EVAL_DEFAULT_MIN_COVERAGE, LEGACY_TRAIN_POST_EVAL_DEFAULT_MIN_PARSE_RATE,
    run_legacy_train_post_eval_gate,
};
pub use planning_eval::{PlanningEvalResult, evaluate_plan_sequence};
pub use policy::{
    ContextGateEntry, EvalGatePolicy, EvalLocalGate, McpToolSchemaGate, ModalMixGate, PassAtKGate,
    PerplexityGate, ReviewRecurrenceGate, SupervisedRatioGate, ThroughputGate, TruncationGate,
    load_policy,
};
pub use run_gate::run_eval_gate;
