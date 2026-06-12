//! The hand-written `NoveltyEvidenceBundleV1` MUST serialize to JSON that the
//! contract schema accepts and that round-trips through the generated type.
//!
//! `vox-research-events` generates `ScientiaNoveltyEvidenceBundleV1` via typify
//! from `contracts/scientia/novelty-evidence-bundle.v1.schema.json`.  Nothing
//! currently keeps the two in sync.  This test makes the contract the de-facto
//! SSOT by asserting a lossless round-trip.

use vox_publisher::scientia_finding_ledger::{
    NormalizedPriorArtHit, NoveltyEvidenceBundleV1, NoveltyOverlapSummary, NoveltyQueryTrace,
    NoveltyRecencyBucket, PriorArtSource,
};
use vox_research_events::schema_types::generated::novelty_evidence_bundle_v1_schema::ScientiaNoveltyEvidenceBundleV1;

/// A 64-char all-lowercase-hex string that satisfies the `^[a-f0-9]{64}$` pattern
/// required by the contract schema.
const HEX64: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

/// Build a representative bundle that exercises every field.
fn representative_bundle() -> NoveltyEvidenceBundleV1 {
    NoveltyEvidenceBundleV1 {
        schema_version: 1,
        bundle_id: "nb.deadbeefdeadbeef".to_string(),
        candidate_id: "fc.test-candidate-001".to_string(),
        computed_at_ms: 1_700_000_000_000,
        query_digest_sha256: HEX64.to_string(),
        sources: vec![PriorArtSource::Openalex],
        normalized_hits: vec![NormalizedPriorArtHit {
            source: PriorArtSource::Openalex,
            work_uri: "https://openalex.org/W9999999999".to_string(),
            title: "Efficient batch inference for large language models".to_string(),
            year: Some(2023),
            lexical_score: Some(0.72),
            semantic_score: Some(0.65),
            overlap_note: Some("shares methodology section".to_string()),
            cited_by_count: Some(42),
        }],
        overlap_summary: Some(NoveltyOverlapSummary {
            max_lexical_score: Some(0.72),
            max_semantic_score: Some(0.65),
            recency_bucket: NoveltyRecencyBucket::Recent,
        }),
        query_traces: vec![NoveltyQueryTrace {
            source: "openalex".to_string(),
            request_fingerprint_sha256: HEX64.to_string(),
            http_status: Some(200),
            cached: Some(false),
        }],
    }
}

#[test]
fn v1_round_trips_through_generated_contract_type() {
    let bundle = representative_bundle();

    // Step 1: serialize hand-written type to a JSON Value.
    let json = serde_json::to_value(&bundle).expect("hand-written bundle serializes");

    // Step 2: deserialize into the generated contract type.
    // If this fails the hand-written type emits JSON the schema does not accept.
    let generated: ScientiaNoveltyEvidenceBundleV1 = serde_json::from_value(json.clone()).expect(
        "generated type should accept hand-written JSON; \
             deserialization failure = schema drift",
    );

    // Step 3: re-serialize and compare.
    let back = serde_json::to_value(&generated).expect("generated bundle re-serializes");

    assert_eq!(
        json, back,
        "lossy round-trip = schema drift: \n  original: {json}\n  round-tripped: {back}"
    );
}

#[test]
fn representative_bundle_validates_against_schema() {
    let bundle = representative_bundle();
    let json = serde_json::to_value(&bundle).expect("serialize");

    let schema_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/scientia/novelty-evidence-bundle.v1.schema.json");
    let schema_raw =
        std::fs::read_to_string(&schema_path).expect("read novelty-evidence-bundle schema");
    let schema: serde_json::Value = serde_json::from_str(&schema_raw).expect("parse schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("compile schema");

    validator
        .validate(&json)
        .expect("representative bundle JSON must satisfy the contract schema");
}
