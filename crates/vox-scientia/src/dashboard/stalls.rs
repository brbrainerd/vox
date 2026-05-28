//! Stall detection. A candidate is "stalled" when it has been in
//! `evidence_incomplete` for more than [`STALE_THRESHOLD_MS`] (30 days).
//!
//! Surfaced to the dashboard as a "needs attention" badge.

use serde::{Deserialize, Serialize};

use super::queue::CandidateRow;

/// 30 days in epoch milliseconds.
pub const STALE_THRESHOLD_MS: i64 = 30 * 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StallEntry {
    pub candidate_id: String,
    pub class: String,
    /// How long the candidate has been in `evidence_incomplete` (ms).
    pub stuck_for_ms: i64,
}

/// Return one [`StallEntry`] per stalled candidate.
pub fn detect_stalls(candidates: &[CandidateRow], now_ms: i64) -> Vec<StallEntry> {
    candidates
        .iter()
        .filter_map(|c| {
            if c.state != "evidence_incomplete" {
                return None;
            }
            let stuck_for_ms = now_ms.saturating_sub(c.updated_at_ms);
            if stuck_for_ms < STALE_THRESHOLD_MS {
                return None;
            }
            Some(StallEntry {
                candidate_id: c.candidate_id.clone(),
                class: c.candidate_class.clone(),
                stuck_for_ms,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::queue::CandidateRow;

    fn row(id: &str, state: &str, updated_at_ms: i64) -> CandidateRow {
        CandidateRow {
            candidate_id: id.into(),
            candidate_class: "algorithmic_improvement".into(),
            confidence: 0.5,
            state: state.into(),
            created_at_ms: 0,
            updated_at_ms,
        }
    }

    #[test]
    fn candidate_in_other_state_does_not_stall() {
        let now = 1_000_000_000_000;
        let rows = vec![row("a", "ready_for_prereg", 0)];
        let s = detect_stalls(&rows, now);
        assert!(s.is_empty());
    }

    #[test]
    fn evidence_incomplete_under_threshold_does_not_stall() {
        let now = 1_000_000_000_000;
        let one_day_ago = now - 24 * 60 * 60 * 1_000;
        let rows = vec![row("a", "evidence_incomplete", one_day_ago)];
        let s = detect_stalls(&rows, now);
        assert!(s.is_empty());
    }

    #[test]
    fn evidence_incomplete_over_threshold_stalls() {
        let now = 1_000_000_000_000;
        let thirty_one_days_ago = now - 31 * 24 * 60 * 60 * 1_000;
        let rows = vec![row("a", "evidence_incomplete", thirty_one_days_ago)];
        let s = detect_stalls(&rows, now);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].candidate_id, "a");
        assert!(s[0].stuck_for_ms >= STALE_THRESHOLD_MS);
    }

    #[test]
    fn stuck_for_ms_is_never_negative_even_when_clock_skews_backward() {
        let now = 1_000_000;
        let future = 2_000_000;
        let rows = vec![row("a", "evidence_incomplete", future)];
        let s = detect_stalls(&rows, now);
        // future > now → saturating_sub yields 0 → below threshold → not stalled.
        assert!(s.is_empty());
    }

    #[test]
    fn threshold_constant_is_thirty_days() {
        assert_eq!(STALE_THRESHOLD_MS, 30 * 86_400 * 1_000);
    }
}
