//! Phase D wiring acceptance — when
//! `metadata_json.scientia_evidence.extracted_claims` carries a measured
//! support ratio, `WorthinessInputs::claim_evidence_coverage` MUST be
//! sourced from that measurement rather than the citation-presence
//! heuristic.

use vox_publisher::publication::PublicationManifest;
use vox_publisher::publication_preflight::{
    run_preflight, worthiness_inputs_from_manifest_and_preflight, PreflightProfile,
};
use vox_publisher::scientia_heuristics::ScientiaHeuristics;

fn derive_worthiness_inputs(
    manifest: &PublicationManifest,
    heuristics: &ScientiaHeuristics,
) -> vox_publisher::publication_worthiness::WorthinessInputs {
    let report = run_preflight(manifest, PreflightProfile::Default);
    worthiness_inputs_from_manifest_and_preflight(manifest, &report, Some(heuristics))
}

fn base_manifest() -> PublicationManifest {
    PublicationManifest {
        publication_id: "p1".into(),
        content_type: "scientia".into(),
        source_ref: None,
        title: "Demo".into(),
        author: "Alice".into(),
        abstract_text: Some("Short abstract.".into()),
        body_markdown: "# Hello\nThis is a test paper.".into(),
        citations_json: None,
        metadata_json: None,
    }
}

fn metadata_with_extracted_claims(total: u32, supported: u32) -> String {
    serde_json::json!({
        "scientia_evidence": {
            "extracted_claims": {
                "schema_version": 1,
                "total_atomic": total,
                "supported": supported,
                "refuted": 0,
                "abstained": total.saturating_sub(supported),
                "verifier_model": "mock",
                "abstain_threshold": 0.3,
                "promotion_threshold": 0.7,
                "extracted_at_ms": 1_747_000_000_000_i64
            }
        }
    })
    .to_string()
}

#[test]
fn no_extracted_claims_falls_back_to_citation_heuristic() {
    let mut m = base_manifest();
    // No citations, no extracted_claims → low coverage via heuristic.
    m.citations_json = None;
    let h = ScientiaHeuristics::default();
    let inputs = derive_worthiness_inputs(&m, &h);
    assert!(
        inputs.claim_evidence_coverage < 0.5,
        "no signals at all → low coverage; got {}",
        inputs.claim_evidence_coverage
    );
}

#[test]
fn extracted_claims_high_support_ratio_drives_high_coverage() {
    let mut m = base_manifest();
    m.metadata_json = Some(metadata_with_extracted_claims(10, 9));
    let h = ScientiaHeuristics::default();
    let inputs = derive_worthiness_inputs(&m, &h);
    // 9/10 = 0.9 measured; must dominate the citation-presence heuristic.
    assert!(
        (inputs.claim_evidence_coverage - 0.9).abs() < 1e-9,
        "expected exactly 0.9; got {}",
        inputs.claim_evidence_coverage
    );
}

#[test]
fn extracted_claims_low_support_ratio_drives_low_coverage_even_with_citations() {
    let mut m = base_manifest();
    // Even with non-empty citations_json (which would push the heuristic
    // high), the measured ratio MUST win.
    m.citations_json = Some(r#"[{"title":"A","doi":"10.0/x"}]"#.into());
    m.metadata_json = Some(metadata_with_extracted_claims(10, 1));
    let h = ScientiaHeuristics::default();
    let inputs = derive_worthiness_inputs(&m, &h);
    assert!(
        (inputs.claim_evidence_coverage - 0.1).abs() < 1e-9,
        "measured 0.1 must override citation heuristic; got {}",
        inputs.claim_evidence_coverage
    );
}

#[test]
fn extracted_claims_with_zero_total_atomic_falls_back_to_heuristic() {
    // `total_atomic = 0` means "nothing extractable" — not "fully unsupported".
    // The rubric falls back to the citation-presence heuristic in that case.
    let mut m = base_manifest();
    m.citations_json = Some(r#"[{"title":"A","doi":"10.0/x"}]"#.into());
    m.metadata_json = Some(metadata_with_extracted_claims(0, 0));
    let h = ScientiaHeuristics::default();
    let inputs = derive_worthiness_inputs(&m, &h);
    // Citation heuristic with non-empty array → at least 0.55.
    assert!(
        inputs.claim_evidence_coverage >= 0.55,
        "should fall back to citation heuristic; got {}",
        inputs.claim_evidence_coverage
    );
}
