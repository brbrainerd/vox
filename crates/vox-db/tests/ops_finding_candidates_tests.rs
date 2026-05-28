//! Phase A — store ops for `scientia_finding_candidates`.
//!
//! Verifies insert / list / get round-trips plus the
//! `(producer_name, signal_fingerprint)` idempotency contract.

use vox_db::store::{FindingCandidateClass, FindingCandidateRow, InsertOutcome};
use vox_db::{DbConfig, VoxDb};

fn make_row(candidate_id: &str, fp: &str, created_at_ms: i64) -> FindingCandidateRow {
    FindingCandidateRow {
        candidate_id: candidate_id.into(),
        candidate_class: FindingCandidateClass::AlgorithmicImprovement,
        publication_id: None,
        title_hint: Some("perf delta".into()),
        internal_signals_json: r#"[{"code":"p95","summary":"latency drop","strength":"strong","family":"benchmark_pair","provenance":{"origin":"test"}}]"#.into(),
        novelty_evidence_bundle_id: None,
        worthiness_decision_ref: None,
        confidence_json: Some(r#"{"signal_strength":0.81}"#.into()),
        repository_id: Some("vox".into()),
        producer_name: "test_producer".into(),
        signal_fingerprint: fp.into(),
        created_at_ms,
        updated_at_ms: created_at_ms,
    }
}

#[tokio::test]
async fn insert_then_get_round_trip() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let row = make_row("alg-001", "fp-001", 1_747_000_000_000);
    let outcome = db.insert_finding_candidate(&row).await.expect("insert");
    assert_eq!(outcome, InsertOutcome::Inserted);

    let got = db
        .get_finding_candidate("alg-001")
        .await
        .expect("get")
        .expect("row present");
    assert_eq!(got, row);
}

#[tokio::test]
async fn insert_then_list_round_trip() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    db.insert_finding_candidate(&make_row("a", "fp-a", 1_000))
        .await
        .expect("insert a");
    db.insert_finding_candidate(&make_row("b", "fp-b", 2_000))
        .await
        .expect("insert b");

    let all = db.list_finding_candidates(None).await.expect("list all");
    assert_eq!(all.len(), 2);
    // Newest first.
    assert_eq!(all[0].candidate_id, "b");
    assert_eq!(all[1].candidate_id, "a");
}

#[tokio::test]
async fn duplicate_fingerprint_is_idempotent_already_seen() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let row1 = make_row("first", "fp-shared", 100);
    let mut row2 = make_row("second", "fp-shared", 200);
    row2.producer_name = row1.producer_name.clone();

    let o1 = db.insert_finding_candidate(&row1).await.expect("insert 1");
    assert_eq!(o1, InsertOutcome::Inserted);
    let o2 = db.insert_finding_candidate(&row2).await.expect("insert 2");
    assert_eq!(
        o2,
        InsertOutcome::AlreadySeen,
        "(producer_name, signal_fingerprint) must be idempotent"
    );

    let all = db.list_finding_candidates(None).await.expect("list");
    assert_eq!(all.len(), 1, "second insert must not create a new row");
    assert_eq!(all[0].candidate_id, "first");
}

#[tokio::test]
async fn list_filtered_by_class() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let mut a = make_row("a", "fp-a", 1_000);
    a.candidate_class = FindingCandidateClass::AlgorithmicImprovement;
    let mut b = make_row("b", "fp-b", 2_000);
    b.candidate_class = FindingCandidateClass::TelemetryTrust;
    db.insert_finding_candidate(&a).await.unwrap();
    db.insert_finding_candidate(&b).await.unwrap();

    let algs = db
        .list_finding_candidates(Some(FindingCandidateClass::AlgorithmicImprovement))
        .await
        .expect("list");
    assert_eq!(algs.len(), 1);
    assert_eq!(algs[0].candidate_id, "a");

    let trust = db
        .list_finding_candidates(Some(FindingCandidateClass::TelemetryTrust))
        .await
        .expect("list");
    assert_eq!(trust.len(), 1);
    assert_eq!(trust[0].candidate_id, "b");
}

#[tokio::test]
async fn get_returns_none_when_absent() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let got = db
        .get_finding_candidate("nonexistent")
        .await
        .expect("get");
    assert!(got.is_none());
}

#[test]
fn class_round_trips_through_sql_string() {
    for c in [
        FindingCandidateClass::AlgorithmicImprovement,
        FindingCandidateClass::ReproducibilityInfra,
        FindingCandidateClass::PolicyGovernance,
        FindingCandidateClass::TelemetryTrust,
        FindingCandidateClass::Other,
    ] {
        assert_eq!(FindingCandidateClass::from_sql(c.as_sql()), Some(c));
    }
    assert_eq!(FindingCandidateClass::from_sql("nope"), None);
}
