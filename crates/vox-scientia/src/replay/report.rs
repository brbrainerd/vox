//! Replay report and error types.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Structured outcome of a replay attempt. Writes to `publication_status_events`
/// (or whatever event surface the caller chooses) as a JSON payload, and
/// drives the measured value of `artifact_replayability_measured` in the
/// worthiness rubric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayReport {
    pub outcome: ReplayOutcome,
    /// Wall-clock duration of the sandboxed run, in milliseconds.
    pub wall_ms: u64,
    /// Measured replayability in `[0.0, 1.0]`. Binary today (1.0 pass /
    /// 0.0 fail); statistical-equivalence scoring is a follow-up.
    pub measured_score: f64,
    pub diagnostics: Vec<String>,
    /// Captured stdout, truncated per `MainEntity::max_stdout_bytes`.
    pub stdout: String,
    /// Captured stderr, truncated per `MainEntity::max_stderr_bytes`.
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReplayOutcome {
    /// All declared output hashes matched and the entry-point exited 0.
    Pass,
    /// At least one declared output hash did not match.
    HashMismatch,
    /// Entry-point exited with a non-zero status code. Struct variant
    /// (not newtype) so serde's internally-tagged enum representation can
    /// serialize the payload — tagged newtype variants over primitives
    /// fail at runtime with `cannot serialize tagged newtype variant`.
    NonZeroExit { exit_code: i32 },
    /// Sandbox exceeded the contract's wall-clock cap.
    TimedOut,
}

#[derive(Debug, Error)]
pub enum ReplayError {
    /// `MainEntity` violated its internal invariants (e.g., path/hash
    /// vector length mismatch).
    #[error("contract malformed: {0}")]
    ContractMalformed(String),
    /// Filesystem I/O failure while staging or hashing outputs.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_outcome_serializes_with_kind_tag() {
        let json = serde_json::to_string(&ReplayOutcome::Pass).unwrap();
        assert!(json.contains("\"kind\":\"pass\""));
    }

    #[test]
    fn non_zero_exit_serializes_with_payload() {
        let json = serde_json::to_string(&ReplayOutcome::NonZeroExit { exit_code: 42 }).unwrap();
        assert!(json.contains("\"kind\":\"non_zero_exit\""));
        assert!(json.contains("42"));
    }

    #[test]
    fn report_round_trips_through_json() {
        let r = ReplayReport {
            outcome: ReplayOutcome::Pass,
            wall_ms: 123,
            measured_score: 1.0,
            diagnostics: vec![],
            stdout: "ok".into(),
            stderr: "".into(),
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: ReplayReport = serde_json::from_str(&j).unwrap();
        assert_eq!(back, r);
    }
}
