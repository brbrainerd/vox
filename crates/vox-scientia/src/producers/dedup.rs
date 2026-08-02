//! Producer-output deduplication.
//!
//! Producers may legitimately emit overlapping signals (e.g.,
//! `CommitGraphProducer` flags a perf-improving merge, and
//! `BenchHistoryProducer` flags the same merge from the bench-CI side).
//! We collapse on `finding_id`, which producers construct deterministically
//! from a content fingerprint — same root cause → same id → one event.
//!
//! We also collapse near-duplicate findings whose `finding_id`s differ but
//! whose `finding_candidate.title_hint` text is lexically near-identical
//! (4-gram shingle Jaccard similarity), using
//! `super::novelty_lexical::lexical_similarity`. This catches the case where
//! two producers (or the same producer on a slightly-reworded pass) generate
//! different fingerprints for what is substantively the same finding.

use std::collections::HashSet;
use vox_research_events::ResearchEvent;

use super::novelty_lexical::lexical_similarity;

/// Findings whose title-hint text is at least this lexically similar to a
/// previously-accepted finding are treated as near-duplicates and dropped.
///
/// Lowered from an initially-considered 0.85: a real reworded-restatement
/// pair ("...post-hoc citation audit" vs "...post hoc citation audit")
/// measures ≈0.846 Jaccard on 4-gram shingles, which would fall just under
/// 0.85 and go undetected. 0.8 catches that case with a comfortable margin
/// above the "clearly different text" range (well under 0.1 in testing).
const LEXICAL_DUPLICATE_THRESHOLD: f64 = 0.8;

/// Extract the `title_hint` text (if any) from a `FindingCandidateProposed`
/// event's optional `finding_candidate` JSON payload. A present-but-`null`
/// `title_hint` (a valid serialization of `FindingCandidateV1`'s
/// `Option<String>` field) is intentionally treated the same as an absent
/// field — `.as_str()` returns `None` for `Value::Null`, so this falls
/// through to the exact-`finding_id`-only dedup path below, not a bug.
fn title_hint_text(finding_candidate: &Option<serde_json::Value>) -> Option<String> {
    finding_candidate
        .as_ref()?
        .get("title_hint")?
        .as_str()
        .map(str::to_owned)
}

/// Drop later `FindingCandidateProposed` events whose `finding_id` already
/// appeared in the input, or whose `finding_candidate.title_hint` is
/// lexically near-identical to a previously-accepted finding's title hint.
/// Other event variants pass through unchanged.
pub fn dedup_finding_candidates(events: Vec<ResearchEvent>) -> Vec<ResearchEvent> {
    let mut seen_ids = HashSet::new();
    let mut accepted_texts: Vec<String> = Vec::new();
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        match &ev {
            ResearchEvent::FindingCandidateProposed {
                finding_id,
                finding_candidate,
                ..
            } => {
                if !seen_ids.insert(finding_id.clone()) {
                    continue;
                }

                if let Some(text) = title_hint_text(finding_candidate) {
                    // O(n^2) in accepted findings per dedup pass; fine at
                    // expected per-run finding-candidate volume (tens, not
                    // thousands) — revisit with an LSH/bucketing index if
                    // that assumption ever changes.
                    let is_near_duplicate = accepted_texts.iter().any(|prior| {
                        lexical_similarity(prior, &text) >= LEXICAL_DUPLICATE_THRESHOLD
                    });
                    if is_near_duplicate {
                        continue;
                    }
                    accepted_texts.push(text);
                }

                out.push(ev);
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
            finding_candidate: None,
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
                ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
                    Some(finding_id.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(dedup_finding_candidates(vec![]).is_empty());
    }

    fn fc_with_text(id: &str, text: &str) -> ResearchEvent {
        ResearchEvent::FindingCandidateProposed {
            finding_id: id.into(),
            claim_ids: vec![],
            worthiness_score: 0.5,
            session_id: "s".into(),
            finding_candidate: Some(serde_json::json!({ "title_hint": text })),
        }
    }

    #[test]
    fn collapses_near_duplicate_finding_text_even_with_different_ids() {
        let events = vec![
            fc_with_text(
                "id-1",
                "The synthesis stage lacks a post-hoc citation audit",
            ),
            fc_with_text(
                "id-2",
                "The synthesis stage lacks a post hoc citation audit",
            ), // near-identical, different id
        ];
        let deduped = dedup_finding_candidates(events);
        assert_eq!(
            deduped.len(),
            1,
            "near-duplicate finding text should collapse despite different finding_ids"
        );
    }
}
