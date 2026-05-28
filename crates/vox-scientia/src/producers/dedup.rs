//! Producer-output deduplication.
//!
//! Producers may legitimately emit overlapping signals (e.g.,
//! `CommitGraphProducer` flags a perf-improving merge, and
//! `BenchHistoryProducer` flags the same merge from the bench-CI side).
//! We collapse on `finding_id`, which producers construct deterministically
//! from a content fingerprint — same root cause → same id → one event.

use std::collections::HashSet;
use vox_research_events::ResearchEvent;

/// Drop later `FindingCandidateProposed` events whose `finding_id` already
/// appeared in the input. Other event variants pass through unchanged.
pub fn dedup_finding_candidates(events: Vec<ResearchEvent>) -> Vec<ResearchEvent> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        match &ev {
            ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
                if seen.insert(finding_id.clone()) {
                    out.push(ev);
                }
            }
            _ => out.push(ev),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fc(id: &str) -> ResearchEvent {
        ResearchEvent::FindingCandidateProposed {
            finding_id: id.into(),
            claim_ids: vec![],
            worthiness_score: 0.5,
            session_id: "s".into(),
        }
    }

    #[test]
    fn collapses_duplicate_finding_ids() {
        let out = dedup_finding_candidates(vec![fc("x"), fc("x"), fc("y")]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn preserves_first_occurrence_order() {
        let out = dedup_finding_candidates(vec![fc("a"), fc("b"), fc("a"), fc("c")]);
        let ids: Vec<&str> = out
            .iter()
            .filter_map(|e| match e {
                ResearchEvent::FindingCandidateProposed { finding_id, .. } => Some(finding_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(dedup_finding_candidates(vec![]).is_empty());
    }
}
