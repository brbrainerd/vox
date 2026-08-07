//! Synchronous, in-process heuristic scorer for repeated-correction patterns
//! within a single `run_agent_turn` tool-call loop. Pure logic, no I/O — kept
//! separate from the loop itself so it's unit-testable without a live LLM or DB.
//! Scoped per-turn (not per-session): this codebase has no per-session
//! shared-mutable-state mechanism at this layer, and adding one purely for a
//! same-turn-only detector would be over-built for what the detector actually
//! observes.
//!
//! `// ponytail: fixed threshold, revisit with a GUI-configurable slider only
//! if false-positive rate in practice warrants it — see the design's history
//! of over-built, never-wired auto-tuning engines in this codebase.`

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const THRESHOLD: u32 = 3;

#[derive(Debug, Default)]
pub struct HarnessIssueScorer {
    /// (tool_name, first-line-of-result hash) -> times seen this turn.
    error_signatures: HashMap<(String, u64), u32>,
    /// Args JSON of the immediately preceding call, and how many times in a
    /// row (including this one) the exact same (tool, args) pair has repeated.
    last_call: Option<(String, String)>,
    consecutive_repeats: u32,
    score: u32,
}

impl HarnessIssueScorer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one tool call's name, arguments (as a JSON string), and result
    /// string. Each call contributes at most one point to `score`, from
    /// whichever single signal fires first (repeated error signature, then
    /// consecutive identical retry) — a call is not double-counted by both.
    /// Returns `true` once the accumulated score first crosses [`THRESHOLD`].
    pub fn record(&mut self, tool_name: &str, args_json: &str, result: &str) -> bool {
        let mut hit = false;

        let is_error = result.starts_with("Error:") || result.contains("\"success\":false");
        if is_error {
            let first_line = result.lines().next().unwrap_or(result);
            let mut hasher = DefaultHasher::new();
            first_line.hash(&mut hasher);
            let key = (tool_name.to_string(), hasher.finish());
            let count = self.error_signatures.entry(key).or_insert(0);
            *count += 1;
            if *count >= 2 {
                hit = true;
            }
        }

        let call_key = (tool_name.to_string(), args_json.to_string());
        if self.last_call.as_ref() == Some(&call_key) {
            self.consecutive_repeats += 1;
        } else {
            self.last_call = Some(call_key);
            self.consecutive_repeats = 1;
        }
        if !hit && self.consecutive_repeats >= 3 {
            hit = true;
        }

        if hit {
            self.score += 1;
        }
        self.score >= THRESHOLD
    }

    /// Reset all accumulated state (called after a judge verdict, whether
    /// or not it produced a real issue, so one turn's noise can't leak into
    /// the next).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_error_signature_crosses_threshold_on_third_hit() {
        let mut scorer = HarnessIssueScorer::new();
        // 1st occurrence: count=1, not >=2, no hit.
        assert!(!scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
        // 2nd occurrence: count=2, >=2 -> hit -> score=1.
        assert!(!scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
        // 3rd occurrence: count=3 -> hit -> score=2.
        assert!(!scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
        // 4th occurrence: count=4 -> hit -> score=3 >= THRESHOLD.
        assert!(scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
    }

    #[test]
    fn retry_loop_alone_crosses_threshold_after_five_identical_calls() {
        let mut scorer = HarnessIssueScorer::new();
        let args = r#"{"path":"foo.vox"}"#;
        // Use a non-error result so only the retry signal is exercised in isolation.
        assert!(!scorer.record("validate_file", args, "ok: no diagnostics")); // streak=1
        assert!(!scorer.record("validate_file", args, "ok: no diagnostics")); // streak=2
        // 3rd identical consecutive call -> consecutive_repeats=3 -> hit -> score=1.
        // THRESHOLD is 3 total hits, and each call earns at most one hit, so
        // this is the FIRST hit, not the crossing point.
        assert!(!scorer.record("validate_file", args, "ok: no diagnostics")); // streak=3, score=1
        assert!(!scorer.record("validate_file", args, "ok: no diagnostics")); // streak=4, score=2
        assert!(scorer.record("validate_file", args, "ok: no diagnostics")); // streak=5, score=3 -> true
    }

    #[test]
    fn a_call_is_never_double_counted_by_both_signals_at_once() {
        let mut scorer = HarnessIssueScorer::new();
        let args = "{}";
        // Every call here is both a repeated error signature AND (from the
        // 3rd call on) a consecutive retry. If both signals fired per call,
        // this would cross THRESHOLD=3 on the 2nd or 3rd call; it must not,
        // because each call awards at most one point.
        assert!(!scorer.record("build_crate", args, "Error: E0502"));
        assert!(!scorer.record("build_crate", args, "Error: E0502")); // error-sig hit #1 (score=1)
        assert!(!scorer.record("build_crate", args, "Error: E0502")); // error-sig hit #2 (score=2)
        assert!(scorer.record("build_crate", args, "Error: E0502")); // error-sig hit #3 (score=3)
    }

    #[test]
    fn interleaving_a_different_call_resets_the_retry_streak() {
        let mut scorer = HarnessIssueScorer::new();
        scorer.record("build_crate", "{}", "ok"); // streak=1
        scorer.record("build_crate", "{}", "ok"); // streak=2 (one more would hit)
        scorer.record("lint_crate", "{}", "ok"); // different call -> streak resets to 1
        // If the streak had NOT reset, the next build_crate call would be the
        // 3rd-in-a-row and should hit immediately. Because it resets, it takes
        // two more identical calls (not one) before a hit occurs again — and
        // even that single hit only brings score to 1, still below
        // THRESHOLD=3, so every assertion below is `false`.
        assert!(!scorer.record("build_crate", "{}", "ok")); // streak=1, no hit, score=0
        assert!(!scorer.record("build_crate", "{}", "ok")); // streak=2, no hit, score=0
        assert!(!scorer.record("build_crate", "{}", "ok")); // streak=3, hit, score=1
    }

    #[test]
    fn distinct_successful_calls_never_cross_threshold() {
        let mut scorer = HarnessIssueScorer::new();
        for i in 0..10 {
            assert!(!scorer.record(
                "build_crate",
                &format!("{{\"n\":{i}}}"),
                "ok: build succeeded"
            ));
        }
    }

    #[test]
    fn reset_clears_accumulated_score() {
        let mut scorer = HarnessIssueScorer::new();
        scorer.record("build_crate", "{}", "Error: E0502");
        scorer.record("build_crate", "{}", "Error: E0502");
        scorer.reset();
        assert!(!scorer.record("build_crate", "{}", "Error: E0502"));
    }
}
