#![allow(dead_code)] // eval-gate helpers, not yet wired
//! B7.4 — Planning/dispatch eval (base-only, records v2 evidence).
//!
//! NOTE: No planning spoke exists in v1 of the fine-tuning pipeline.
//! This module records metric evidence for a potential v2 planning spoke
//! decision. The structs and evaluator are compile-time verified and
//! tested, but are NOT wired into the v1 gate path.
#![allow(dead_code)]

/// Result of evaluating an actual tool sequence against an expected sequence.
///
/// NOTE: No planning spoke in v1 — this metric is evidence for v2 decision.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanningEvalResult {
    /// The actual tool sequence produced by the model.
    pub sequence: Vec<String>,
    /// The expected (reference) tool sequence.
    pub expected_sequence: Vec<String>,
    /// True if `sequence == expected_sequence` (exact order and length match).
    pub sequence_match: bool,
    /// Fraction of expected tools that appear anywhere in `actual` (order-independent).
    /// Range: 0.0–1.0.
    pub coverage: f64,
}

/// Evaluate a model-produced tool sequence against the expected reference.
///
/// - `sequence_match`: exact equality (order and length must match).
/// - `coverage`: fraction of expected tools that appear in actual
///   (each expected tool counted once; duplicate matching is counted correctly).
///
/// # Examples
/// ```
/// use vox_ml_cli::commands::mens::eval_gate::planning_eval::evaluate_plan_sequence;
/// let result = evaluate_plan_sequence(&["a", "b", "c"], &["a", "b", "c"]);
/// assert!(result.sequence_match);
/// assert_eq!(result.coverage, 1.0);
/// ```
///
/// NOTE: No planning spoke in v1 — this metric is evidence for v2 decision.
pub fn evaluate_plan_sequence(actual: &[&str], expected: &[&str]) -> PlanningEvalResult {
    let sequence_match =
        actual.len() == expected.len() && actual.iter().zip(expected.iter()).all(|(a, e)| a == e);

    // Coverage: for each expected tool, check if it appears in actual.
    // Use a "consume" approach: each actual element can satisfy at most one expected.
    let coverage = if expected.is_empty() {
        1.0 // vacuously true
    } else {
        let mut remaining: Vec<&str> = actual.to_vec();
        let mut matched = 0usize;
        for exp in expected {
            if let Some(pos) = remaining.iter().position(|a| a == exp) {
                matched += 1;
                remaining.remove(pos);
            }
        }
        matched as f64 / expected.len() as f64
    };

    PlanningEvalResult {
        sequence: actual.iter().map(|s| s.to_string()).collect(),
        expected_sequence: expected.iter().map(|s| s.to_string()).collect(),
        sequence_match,
        coverage,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_match() {
        let r = evaluate_plan_sequence(
            &["read_file", "grep_pattern", "write_file"],
            &["read_file", "grep_pattern", "write_file"],
        );
        assert!(
            r.sequence_match,
            "exact match should set sequence_match=true"
        );
        assert_eq!(r.coverage, 1.0, "all expected tools covered");
    }

    #[test]
    fn partial_match_coverage() {
        let r = evaluate_plan_sequence(
            &["read_file", "write_file"],                 // actual: 2 tools
            &["read_file", "grep_pattern", "write_file"], // expected: 3 tools
        );
        assert!(!r.sequence_match, "different lengths → no sequence match");
        // 2 of 3 expected tools appear in actual → coverage = 2/3
        assert!(
            (r.coverage - 2.0 / 3.0).abs() < 1e-9,
            "coverage should be 2/3, got {}",
            r.coverage
        );
    }

    #[test]
    fn wrong_order_no_sequence_match_but_full_coverage() {
        let r = evaluate_plan_sequence(
            &["write_file", "read_file", "grep_pattern"],
            &["read_file", "grep_pattern", "write_file"],
        );
        assert!(!r.sequence_match, "wrong order → sequence_match=false");
        assert_eq!(r.coverage, 1.0, "all tools present → coverage=1.0");
    }

    #[test]
    fn empty_actual_zero_coverage() {
        let r = evaluate_plan_sequence(&[], &["read_file", "write_file"]);
        assert!(!r.sequence_match);
        assert_eq!(r.coverage, 0.0);
    }

    #[test]
    fn empty_expected_vacuously_true() {
        let r = evaluate_plan_sequence(&["read_file"], &[]);
        // sequence_match: both empty? no — actual has 1 element
        assert!(!r.sequence_match);
        assert_eq!(r.coverage, 1.0, "vacuously true for empty expected");
    }

    #[test]
    fn both_empty_match() {
        let r = evaluate_plan_sequence(&[], &[]);
        assert!(r.sequence_match);
        assert_eq!(r.coverage, 1.0);
    }

    #[test]
    fn duplicate_tools_counted_once_each() {
        // expected has "read_file" twice; actual has it once
        let r = evaluate_plan_sequence(
            &["read_file", "write_file"],
            &["read_file", "read_file", "write_file"],
        );
        // 1 "read_file" + 1 "write_file" matched out of 3 expected → 2/3
        assert!(
            (r.coverage - 2.0 / 3.0).abs() < 1e-9,
            "duplicate consume-once: coverage should be 2/3, got {}",
            r.coverage
        );
    }

    #[test]
    fn result_fields_correct() {
        let r = evaluate_plan_sequence(&["a", "b"], &["a", "c"]);
        assert_eq!(r.sequence, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(r.expected_sequence, vec!["a".to_string(), "c".to_string()]);
        assert!(!r.sequence_match); // "b" ≠ "c"
        // "a" matches, "c" does not → 1/2
        assert!((r.coverage - 0.5).abs() < 1e-9);
    }
}
