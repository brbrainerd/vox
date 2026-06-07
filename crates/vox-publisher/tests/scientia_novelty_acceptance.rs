//! Acceptance-style tests for SCIENTIA prior-art merge, worthiness adjustments, and venue enforcement.

use chrono::Datelike;
use vox_publisher::publication_preflight::{
    PreflightConfidence, PreflightFinding, PreflightReport, PreflightSeverity,
};
use vox_publisher::publication_worthiness::{
    WorthinessInputs, apply_prior_art_to_worthiness_inputs, load_contract_from_str,
    machine_venue_profile_violations, validate_contract_invariants,
};
use vox_publisher::scientia_finding_ledger::{
    NormalizedPriorArtHit, NoveltyEvidenceBundleV1, NoveltyOverlapSummary, NoveltyQueryTrace,
    PriorArtSource, impact_readership_projection_v1, novelty_decision_calibration_v1,
};
use vox_publisher::scientia_heuristics::ScientiaHeuristics;
use vox_publisher::scientia_prior_art::{PriorArtQuery, empty_novelty_bundle, title_lexical_score};

fn default_contract() -> vox_publisher::publication_worthiness::PublicationWorthinessContract {
    let yaml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/scientia/publication-worthiness.default.yaml"
    ));
    let c = load_contract_from_str(yaml).expect("default contract");
    validate_contract_invariants(&c).expect("invariants");
    c
}

#[test]
fn scientia_impact_projection_assist_only_flags() {
    let q = PriorArtQuery {
        title: "Demo title".into(),
        abstract_text: None,
    };
    let bundle = empty_novelty_bundle("fc.demo", &q);
    let p = impact_readership_projection_v1(&bundle, &ScientiaHeuristics::default());
    assert!(p.assist_only);
    assert!(p.not_publish_gate);
}

#[test]
fn scientia_novelty_title_lexical_score_identity() {
    let s = title_lexical_score("Hello World Pipeline", "hello world pipeline");
    assert!(s > 0.9, "s={s}");
}

#[test]
fn scientia_novelty_calibration_validates_against_telemetry_schema() {
    let q = PriorArtQuery {
        title: "Test publication title".into(),
        abstract_text: None,
    };
    let bundle = empty_novelty_bundle("fc.demo", &q);
    let cal = novelty_decision_calibration_v1(
        "pub-1",
        "fc.demo",
        &bundle,
        42,
        true,
        "ask_for_evidence",
        0.71,
        true,
        Some(0.05),
    );
    let v = serde_json::to_value(&cal).expect("serialize");
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["publication_id"], "pub-1");
    assert_eq!(v["decision_latency_ms"], 42);
    assert_eq!(v["offline_prior_art_fetch"], true);
    assert_eq!(v["max_lexical_overlap"], 0.05);

    let schema_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/telemetry/scientia-novelty-decision-calibration.v1.schema.json");
    let schema_raw = std::fs::read_to_string(&schema_path).expect("read calibration schema");
    let schema: serde_json::Value = serde_json::from_str(&schema_raw).expect("parse schema");
    let validator = jsonschema::validator_for(&schema).expect("compile calibration schema");
    validator
        .validate(&v)
        .expect("calibration JSON should match SSOT schema");
}

#[test]
fn scientia_novelty_prior_art_lowers_worthiness_novelty_metric() {
    let bundle = NoveltyEvidenceBundleV1 {
        schema_version: 1,
        bundle_id: "nb.test".into(),
        candidate_id: "fc.x".into(),
        computed_at_ms: 0,
        query_digest_sha256: "a".repeat(64),
        sources: vec![],
        normalized_hits: vec![NormalizedPriorArtHit {
            source: PriorArtSource::Openalex,
            work_uri: "https://openalex.org/W1".into(),
            title: "Similar work about neural architecture".into(),
            year: Some(2024),
            lexical_score: Some(0.92),
            semantic_score: Some(0.9),
            overlap_note: None,
            cited_by_count: None,
        }],
        overlap_summary: Some(NoveltyOverlapSummary {
            max_lexical_score: Some(0.92),
            max_semantic_score: Some(0.9),
            recency_bucket: vox_publisher::scientia_finding_ledger::NoveltyRecencyBucket::Recent,
        }),
        query_traces: vec![],
    };
    let mut inputs = WorthinessInputs {
        red_line_violation_ids: vec![],
        repeated_unresolved_contradiction: false,
        claim_evidence_coverage: 0.95,
        artifact_replayability: 0.9,
        artifact_replayability_measured: None,
        before_after_pair_integrity: 0.95,
        metadata_completeness: 0.95,
        ai_disclosure_compliance: 1.0,
        epistemic: 0.9,
        reproducibility: 0.9,
        novelty: 0.95,
        reliability: 0.9,
        metadata_policy: 0.95,
        meaningful_advance: true,
    };
    let notes = apply_prior_art_to_worthiness_inputs(&mut inputs, Some(&bundle), None);
    assert!(inputs.novelty < 0.95, "novelty={}", inputs.novelty);
    assert!(
        notes
            .iter()
            .any(|n| n.contains("novelty_after_prior_art_min")),
        "{notes:?}"
    );
}

#[test]
fn scientia_novelty_double_blind_venue_machine_violation() {
    let c = default_contract();
    let report = PreflightReport {
        ok: false,
        readiness_score: 40,
        findings: vec![PreflightFinding {
            code: "double_blind_email_detected",
            severity: PreflightSeverity::Error,
            message: "email in manuscript".into(),
        }],
        manual_required: vec![],
        next_actions: vec![],
        confidence: PreflightConfidence::ManualRequired,
        destination_readiness: vec![],
        worthiness: None,
    };
    let v = machine_venue_profile_violations(&c, "tmlr_double_blind", &report);
    assert!(
        v.iter().any(|s| s.contains("double_blind_anonymization")),
        "{v:?}"
    );
}

// ---------------------------------------------------------------------------
// B5: gate publication novelty on the typed vox-scientia decision layer.
// ---------------------------------------------------------------------------

/// A high-novelty `WorthinessInputs` (novelty=0.95) so a *cap* is observable.
fn high_novelty_inputs() -> WorthinessInputs {
    WorthinessInputs {
        red_line_violation_ids: vec![],
        repeated_unresolved_contradiction: false,
        claim_evidence_coverage: 0.95,
        artifact_replayability: 0.9,
        artifact_replayability_measured: None,
        before_after_pair_integrity: 0.95,
        metadata_completeness: 0.95,
        ai_disclosure_compliance: 1.0,
        epistemic: 0.9,
        reproducibility: 0.9,
        novelty: 0.95,
        reliability: 0.9,
        metadata_policy: 0.95,
        meaningful_advance: true,
    }
}

/// (a) FALSE-POSITIVE REGRESSION: a bundle whose prior-art retrieval FAILED
/// (every queried source returned a non-success status) must NOT be treated as
/// novel. Before B5 the scalar heuristic saw "no hits" and left novelty maxed
/// (a false "novel"); the decision layer must map this to InsufficientEvidence
/// and cap novelty low.
#[test]
fn scientia_failed_retrieval_does_not_max_novelty() {
    let bundle = NoveltyEvidenceBundleV1 {
        schema_version: 1,
        bundle_id: "nb.fail".into(),
        candidate_id: "fc.x".into(),
        computed_at_ms: 0,
        query_digest_sha256: "a".repeat(64),
        sources: vec![PriorArtSource::Openalex, PriorArtSource::Crossref],
        normalized_hits: vec![],
        overlap_summary: None,
        query_traces: vec![
            NoveltyQueryTrace {
                source: "openalex".into(),
                request_fingerprint_sha256: "f".repeat(64),
                http_status: Some(503),
                cached: None,
            },
            NoveltyQueryTrace {
                source: "crossref".into(),
                request_fingerprint_sha256: "f".repeat(64),
                http_status: Some(500),
                cached: None,
            },
        ],
    };
    let mut inputs = high_novelty_inputs();
    let notes = apply_prior_art_to_worthiness_inputs(&mut inputs, Some(&bundle), None);
    assert!(
        inputs.novelty < 0.5,
        "failed retrieval must not stay novel: novelty={}",
        inputs.novelty
    );
    assert!(
        notes.iter().any(|n| n.contains("insufficient_evidence")),
        "expected insufficient_evidence note, got {notes:?}"
    );
}

/// (b) A future-dated "prior art" hit is dropped by ChronoFilter, so it cannot
/// suppress novelty. With the only hit filtered out, there is no prior-art
/// signal and novelty must remain high.
#[test]
fn scientia_future_dated_hit_dropped_by_chronofilter() {
    let future_year = chrono::Utc::now().date_naive().year() + 5;
    let bundle = NoveltyEvidenceBundleV1 {
        schema_version: 1,
        bundle_id: "nb.future".into(),
        candidate_id: "fc.x".into(),
        computed_at_ms: 0,
        query_digest_sha256: "a".repeat(64),
        sources: vec![PriorArtSource::Openalex],
        normalized_hits: vec![NormalizedPriorArtHit {
            source: PriorArtSource::Openalex,
            work_uri: "https://openalex.org/Wfuture".into(),
            title: "Work that cannot predate the claim".into(),
            year: Some(future_year),
            lexical_score: Some(0.95),
            semantic_score: Some(0.95),
            overlap_note: None,
            cited_by_count: None,
        }],
        overlap_summary: Some(NoveltyOverlapSummary {
            max_lexical_score: Some(0.95),
            max_semantic_score: Some(0.95),
            recency_bucket: vox_publisher::scientia_finding_ledger::NoveltyRecencyBucket::Recent,
        }),
        query_traces: vec![],
    };
    let mut inputs = high_novelty_inputs();
    let notes = apply_prior_art_to_worthiness_inputs(&mut inputs, Some(&bundle), None);
    assert!(
        inputs.novelty >= 0.9,
        "future-dated hit must be dropped and not suppress novelty: novelty={}",
        inputs.novelty
    );
    assert!(
        notes.iter().any(|n| n.contains("chrono_filtered")),
        "expected chrono_filtered note, got {notes:?}"
    );
}

/// (c) Opposing-polarity high-similarity hits (one supporting, one
/// contradicting the same claim direction) drive a Contradicted-style cap:
/// novelty is forced low and an explanatory contradiction note is emitted.
#[test]
fn scientia_opposing_polarity_drives_contradicted_cap() {
    let yr = chrono::Utc::now().date_naive().year() - 2;
    let bundle = NoveltyEvidenceBundleV1 {
        schema_version: 1,
        bundle_id: "nb.conflict".into(),
        candidate_id: "fc.x".into(),
        computed_at_ms: 0,
        query_digest_sha256: "a".repeat(64),
        sources: vec![PriorArtSource::Openalex],
        normalized_hits: vec![
            // Supporting hit: overlap_note marks polarity "support".
            NormalizedPriorArtHit {
                source: PriorArtSource::Openalex,
                work_uri: "https://openalex.org/Wsupport".into(),
                title: "Confirms the effect".into(),
                year: Some(yr),
                lexical_score: Some(0.9),
                semantic_score: Some(0.9),
                overlap_note: Some("polarity:support".into()),
                cited_by_count: None,
            },
            // Contradicting hit: overlap_note marks polarity "refute".
            NormalizedPriorArtHit {
                source: PriorArtSource::Openalex,
                work_uri: "https://openalex.org/Wrefute".into(),
                title: "Refutes the effect".into(),
                year: Some(yr),
                lexical_score: Some(0.9),
                semantic_score: Some(0.9),
                overlap_note: Some("polarity:refute".into()),
                cited_by_count: None,
            },
        ],
        overlap_summary: Some(NoveltyOverlapSummary {
            max_lexical_score: Some(0.9),
            max_semantic_score: Some(0.9),
            recency_bucket: vox_publisher::scientia_finding_ledger::NoveltyRecencyBucket::Recent,
        }),
        query_traces: vec![],
    };
    let mut inputs = high_novelty_inputs();
    let notes = apply_prior_art_to_worthiness_inputs(&mut inputs, Some(&bundle), None);
    assert!(
        inputs.novelty < 0.5,
        "contradiction must cap novelty: novelty={}",
        inputs.novelty
    );
    assert!(
        notes.iter().any(|n| n.contains("contradicted")),
        "expected contradicted note, got {notes:?}"
    );
}
