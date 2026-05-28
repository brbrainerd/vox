//! Queue snapshot for `/api/v2/scientia/queue`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::stalls::{detect_stalls, StallEntry};

/// Single row of the candidate ledger, projected for dashboard display.
/// Backend adapters lift this from `scientia_finding_candidates`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateRow {
    pub candidate_id: String,
    pub candidate_class: String,
    pub confidence: f64,
    /// Last-seen state label: e.g. `evidence_incomplete`, `ready_for_prereg`,
    /// `submitted`, `published`, `retracted`. Backend chooses these from
    /// existing `publication_status_events`.
    pub state: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaimsPendingSummary {
    pub verifiable: u64,
    pub abstained: u64,
    pub extraction_running: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplyWindowEntry {
    pub publication_id: String,
    pub opened_at_ms: i64,
    pub closes_at_ms: i64,
}

/// Inputs assembled by the backend from DB rows.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardInputs<'a> {
    pub candidates: &'a [CandidateRow],
    pub claims_pending: ClaimsPendingSummary,
    pub manifests_in_reply_window: &'a [ReplyWindowEntry],
    pub retraction_queue: &'a [String],
    /// Logical "now" (epoch ms). Tests inject a fixed value.
    pub now_ms: i64,
}

/// Response served at `GET /api/v2/scientia/queue`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueueSnapshot {
    pub candidates: CandidateSummary,
    pub claims_pending: ClaimsPendingSummary,
    pub manifests_in_reply_window: Vec<String>,
    pub retraction_queue: Vec<String>,
    pub stalls: Vec<StallEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateSummary {
    pub total: u64,
    /// Counts grouped by `candidate_class` string.
    pub by_class: BTreeMap<String, u64>,
    /// Top-5 candidates by confidence, newest first as tiebreaker.
    pub top_5_by_confidence: Vec<CandidateRow>,
}

/// Assemble a [`QueueSnapshot`] from inputs.
pub fn build_queue_snapshot(inputs: &DashboardInputs<'_>) -> QueueSnapshot {
    let mut by_class: BTreeMap<String, u64> = BTreeMap::new();
    for c in inputs.candidates {
        *by_class.entry(c.candidate_class.clone()).or_default() += 1;
    }

    let mut top: Vec<CandidateRow> = inputs.candidates.to_vec();
    top.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.created_at_ms.cmp(&a.created_at_ms))
    });
    top.truncate(5);

    let stalls = detect_stalls(inputs.candidates, inputs.now_ms);

    let reply_ids: Vec<String> = inputs
        .manifests_in_reply_window
        .iter()
        .map(|r| r.publication_id.clone())
        .collect();

    QueueSnapshot {
        candidates: CandidateSummary {
            total: inputs.candidates.len() as u64,
            by_class,
            top_5_by_confidence: top,
        },
        claims_pending: inputs.claims_pending.clone(),
        manifests_in_reply_window: reply_ids,
        retraction_queue: inputs.retraction_queue.to_vec(),
        stalls,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, class: &str, conf: f64, created_at_ms: i64) -> CandidateRow {
        CandidateRow {
            candidate_id: id.into(),
            candidate_class: class.into(),
            confidence: conf,
            state: "evidence_incomplete".into(),
            created_at_ms,
            updated_at_ms: created_at_ms,
        }
    }

    fn inputs<'a>(candidates: &'a [CandidateRow]) -> DashboardInputs<'a> {
        DashboardInputs {
            candidates,
            claims_pending: ClaimsPendingSummary {
                verifiable: 0,
                abstained: 0,
                extraction_running: 0,
            },
            manifests_in_reply_window: &[],
            retraction_queue: &[],
            now_ms: 2_000_000_000_000,
        }
    }

    #[test]
    fn total_count_matches_input_length() {
        let rows = vec![
            row("a", "algorithmic_improvement", 0.7, 100),
            row("b", "telemetry_trust", 0.5, 200),
        ];
        let snap = build_queue_snapshot(&inputs(&rows));
        assert_eq!(snap.candidates.total, 2);
    }

    #[test]
    fn by_class_counts_each_class_separately() {
        let rows = vec![
            row("a", "algorithmic_improvement", 0.7, 100),
            row("b", "algorithmic_improvement", 0.4, 200),
            row("c", "telemetry_trust", 0.5, 150),
        ];
        let snap = build_queue_snapshot(&inputs(&rows));
        assert_eq!(snap.candidates.by_class.get("algorithmic_improvement"), Some(&2));
        assert_eq!(snap.candidates.by_class.get("telemetry_trust"), Some(&1));
    }

    #[test]
    fn top_5_sorted_by_confidence_desc_then_created_at_desc() {
        let rows = vec![
            row("a", "x", 0.7, 100),
            row("b", "x", 0.9, 50),
            row("c", "x", 0.9, 300), // same conf as b, newer → ahead
            row("d", "x", 0.1, 400),
            row("e", "x", 0.8, 200),
        ];
        let snap = build_queue_snapshot(&inputs(&rows));
        let ids: Vec<&str> = snap
            .candidates
            .top_5_by_confidence
            .iter()
            .map(|r| r.candidate_id.as_str())
            .collect();
        assert_eq!(ids, vec!["c", "b", "e", "a", "d"]);
    }

    #[test]
    fn top_5_caps_at_five_rows_even_with_more_input() {
        let rows: Vec<CandidateRow> = (0..10)
            .map(|i| row(&format!("c{i}"), "x", i as f64 * 0.1, i as i64 * 100))
            .collect();
        let snap = build_queue_snapshot(&inputs(&rows));
        assert_eq!(snap.candidates.top_5_by_confidence.len(), 5);
    }

    #[test]
    fn reply_window_ids_are_lifted_into_snapshot() {
        let rows = vec![];
        let reply_window = vec![ReplyWindowEntry {
            publication_id: "pub-1".into(),
            opened_at_ms: 100,
            closes_at_ms: 200,
        }];
        let mut input = inputs(&rows);
        input.manifests_in_reply_window = &reply_window;
        let snap = build_queue_snapshot(&input);
        assert_eq!(snap.manifests_in_reply_window, vec!["pub-1".to_string()]);
    }

    #[test]
    fn snapshot_serializes_to_json_with_documented_top_level_keys() {
        let snap = build_queue_snapshot(&inputs(&[]));
        let j = serde_json::to_value(&snap).unwrap();
        for key in [
            "candidates",
            "claims_pending",
            "manifests_in_reply_window",
            "retraction_queue",
            "stalls",
        ] {
            assert!(
                j.get(key).is_some(),
                "QueueSnapshot JSON missing top-level `{key}`"
            );
        }
    }
}
