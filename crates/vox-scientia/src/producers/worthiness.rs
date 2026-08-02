//! Populates `WorthinessSignalsV2` hard/soft gates from `TrustScorer`
//! (retraction status, venue reputation).
//!
//! These functions have no production callers yet — wiring into the real
//! SCIENTIA finding-promotion pipeline is deliberate future work (see
//! docs/src/architecture/deep-research-trust-novelty-scoring-landscape-2026-08-01.md §5).
//! Do not delete as "unused"; this is tested infrastructure awaiting its
//! integration point.

use vox_research_events::schema_types::{WorthinessProfile, WorthinessSignalItem};
use vox_research_events::WorthinessSignalsV2;

/// Builds the hard-gate retraction signal for a candidate finding, given
/// whether its primary source DOI is confirmed retracted.
///
/// Callers passing through `TrustScorer::check_retraction`'s `Option<bool>`
/// should collapse `None` (lookup failed/unknown) to `false` here — i.e.
/// fail-open, matching `vox_search::trust::score_hit_trust`'s existing
/// convention of never penalizing on an unresolved lookup. Only a
/// confirmed `Some(true)` should map to `is_retracted: true`.
pub fn hard_gate_retraction_signal(is_retracted: bool) -> WorthinessSignalItem {
    WorthinessSignalItem {
        id: "hg-retraction".to_string(),
        passed: !is_retracted,
        score: if is_retracted { 0.0 } else { 1.0 },
        reason_code: if is_retracted {
            "source_retracted".to_string()
        } else {
            "no_retraction_detected".to_string()
        },
        details: None,
    }
}

/// Builds the soft-gate peer-review-status signal from an OpenAlex venue
/// type string. Takes the venue type directly so it stays independently
/// testable without live HTTP.
pub fn soft_gate_peer_review_signal(
    venue_type: Option<&str>,
) -> (WorthinessSignalItem, WorthinessProfile) {
    let (profile, passed, score, reason) = match venue_type {
        Some("journal") => (WorthinessProfile::Journal, true, 1.0, "peer_reviewed_journal"),
        Some("repository") => (
            WorthinessProfile::Repository,
            true,
            0.7,
            "institutional_repository",
        ),
        Some("preprint") => (
            WorthinessProfile::Preprint,
            true,
            0.5,
            "preprint_not_peer_reviewed",
        ),
        Some(_) => (
            WorthinessProfile::Social,
            false,
            0.2,
            "unrecognized_venue_type",
        ),
        None => (WorthinessProfile::Social, false, 0.2, "unverified_venue"),
    };
    (
        WorthinessSignalItem {
            id: "sg-peer-review".to_string(),
            passed,
            score,
            reason_code: reason.to_string(),
            details: None,
        },
        profile,
    )
}

/// Assembles a `WorthinessSignalsV2` from the individual signal builders
/// above. `next_actions` and `diagnostic` are intentionally left empty here
/// — populating them requires a statcheck-style numeric-claim recheck,
/// which is out of scope for this task (see
/// docs/src/architecture/deep-research-trust-novelty-scoring-landscape-2026-08-01.md §5
/// for that follow-up).
pub fn build_worthiness_signals(
    version: &str,
    is_retracted: bool,
    venue_type: Option<&str>,
) -> WorthinessSignalsV2 {
    let hard = hard_gate_retraction_signal(is_retracted);
    let (soft, profile) = soft_gate_peer_review_signal(venue_type);
    WorthinessSignalsV2 {
        version: version.to_string(),
        profile,
        hard_gate: vec![hard],
        soft_gate: vec![soft],
        diagnostic: vec![],
        next_actions: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retracted_source_fails_hard_gate() {
        let signal = hard_gate_retraction_signal(true);
        assert!(!signal.passed);
        assert_eq!(signal.reason_code, "source_retracted");
    }

    #[test]
    fn clean_source_passes_hard_gate() {
        let signal = hard_gate_retraction_signal(false);
        assert!(signal.passed);
        assert_eq!(signal.reason_code, "no_retraction_detected");
    }

    #[test]
    fn journal_venue_passes_soft_gate_with_full_score() {
        let (signal, profile) = soft_gate_peer_review_signal(Some("journal"));
        assert!(signal.passed);
        assert_eq!(signal.score, 1.0);
        assert_eq!(profile, WorthinessProfile::Journal);
    }

    #[test]
    fn unknown_venue_fails_soft_gate() {
        let (signal, profile) = soft_gate_peer_review_signal(None);
        assert!(!signal.passed);
        assert_eq!(profile, WorthinessProfile::Social);
    }

    #[test]
    fn unrecognized_venue_type_string_fails_soft_gate_with_distinct_reason() {
        let (signal, profile) = soft_gate_peer_review_signal(Some("dataset"));
        assert!(!signal.passed);
        assert_eq!(signal.reason_code, "unrecognized_venue_type");
        assert_eq!(profile, WorthinessProfile::Social);
    }

    #[test]
    fn build_worthiness_signals_assembles_both_gates() {
        let bundle = build_worthiness_signals("v2", false, Some("journal"));
        assert_eq!(bundle.hard_gate.len(), 1);
        assert_eq!(bundle.soft_gate.len(), 1);
        assert!(bundle.hard_gate[0].passed);
        assert!(bundle.soft_gate[0].passed);
    }
}
