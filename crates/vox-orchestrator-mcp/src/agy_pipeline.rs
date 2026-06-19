//! Stage-2/3 deterministic harness: the pure outcome classifier plus the
//! vox_agy_pipeline / vox_agy_review / vox_agy_ledger_digest tools.

use crate::agy_gates::GateResult;

/// green   = files changed AND every specified gate passed.
/// partial = files changed but a gate failed, OR no gates specified (unverified).
/// failed  = timed out or no files changed.
///
/// agy's own exit code is intentionally NOT used — it's an agent wrapper whose
/// exit code doesn't reliably reflect correctness; the EFFECT is the signal (B-9).
pub fn classify_outcome(files_changed: usize, gates: &[GateResult], timed_out: bool) -> &'static str {
    if timed_out || files_changed == 0 {
        return "failed";
    }
    if gates.is_empty() {
        return "partial";
    }
    if gates.iter().all(|g| g.passed) {
        "green"
    } else {
        "partial"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agy_gates::GateResult;

    fn gate(passed: bool) -> GateResult {
        GateResult { name: "g".into(), passed, exit_code: if passed { 0 } else { 1 }, output_tail: String::new(), elapsed_ms: 0 }
    }

    #[test]
    fn timeout_is_failed() {
        assert_eq!(classify_outcome(5, &[gate(true)], true), "failed");
    }
    #[test]
    fn no_changes_is_failed() {
        assert_eq!(classify_outcome(0, &[gate(true)], false), "failed");
    }
    #[test]
    fn changes_with_no_gates_is_partial_not_green() {
        assert_eq!(classify_outcome(3, &[], false), "partial");
    }
    #[test]
    fn changes_with_all_gates_passing_is_green() {
        assert_eq!(classify_outcome(3, &[gate(true), gate(true)], false), "green");
    }
    #[test]
    fn changes_with_a_failing_gate_is_partial() {
        assert_eq!(classify_outcome(3, &[gate(true), gate(false)], false), "partial");
    }
}
