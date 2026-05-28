//! Shared helper for structured observability around persistence-layer failures.
//!
//! Use this in place of `let _ = db.something().await;` whenever the discarded
//! `Result` represents a meaningful write (audit trail, reliability observation,
//! budget accounting, lineage event). The helper logs at `tracing::error!` with
//! a stable structured `op` field so operators can detect ongoing loss.
//!
//! Event-bus emission of a `PersistenceFailed` event is intentionally deferred
//! to a follow-up — this initial helper is log-only so it can be applied
//! without threading new fields through service constructors.
//!
//! Refs: docs/src/architecture/semantic-gap-audit-2026.md F2–F6.

use std::error::Error;
use std::fmt::Display;

/// Log a persistence-layer failure with structured context.
///
/// `op_id` is a short stable identifier for the call site (e.g.
/// `"reliability.endpoint_observation"`, `"lineage.task_submitted"`,
/// `"budget.exec_time"`). It feeds the `op` field in the tracing event so
/// operators can grep / filter persistence failures by site.
///
/// `err` is anything `Display` — most commonly an `anyhow::Error` or a `&str`.
pub fn log_persistence_failure(op_id: &'static str, err: impl Display) {
    tracing::error!(
        target: "vox.orchestrator.persistence",
        op = op_id,
        error = %err,
        "persistence write failed; data not recorded"
    );
}

/// Variant that accepts a `&dyn Error` for callers that already have one in hand.
#[allow(dead_code)] // available for future callers; current call sites use the Display variant.
pub fn log_persistence_failure_err(op_id: &'static str, err: &(dyn Error + 'static)) {
    log_persistence_failure(op_id, err);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: full log-capture assertions (via `tracing_test::traced_test`) are
    // incompatible with `complexity_based_routing_test` in the same binary, which
    // installs a global tracing subscriber via `try_init()`. These smoke tests
    // verify the functions accept their inputs and complete without panicking;
    // the structural correctness (tracing target, field names) is validated by
    // reading the source.
    #[test]
    fn log_persistence_failure_emits_structured_error_event() {
        log_persistence_failure("reliability.endpoint_observation", "simulated db failure");
    }

    #[test]
    fn log_persistence_failure_err_variant_works_for_dyn_error() {
        let e: Box<dyn std::error::Error + 'static> = "boxed error".into();
        log_persistence_failure_err("budget.exec_time", &*e);
    }
}
