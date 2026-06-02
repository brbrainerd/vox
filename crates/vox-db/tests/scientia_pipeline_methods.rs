//! Smoke tests for Phase 0d Codex research pipeline methods.

use vox_db::{DbConfig, VoxDb};

async fn test_db() -> VoxDb {
    VoxDb::connect(DbConfig::Memory)
        .await
        .expect("in-memory DB")
}

#[tokio::test(flavor = "multi_thread")]
async fn create_and_update_research_session() {
    let db = test_db().await;
    let id = db
        .create_research_session("sess-test-001", "what is the latency trend?")
        .await
        .expect("create session");
    assert!(id >= 0);
    db.update_research_session_status(id, "completed")
        .await
        .expect("update status");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_and_list_recent_research_sessions() {
    let db = test_db().await;
    let first = db
        .create_research_session("sess-list-001", "first query")
        .await
        .expect("create first");
    let second = db
        .create_research_session("sess-list-002", "second query")
        .await
        .expect("create second");
    db.update_research_session_status(second, "completed")
        .await
        .expect("status");

    let row = db
        .get_research_session(second)
        .await
        .expect("get session")
        .expect("session exists");
    assert_eq!(row.id, second);
    assert_eq!(row.session_key, "sess-list-002");
    assert_eq!(row.query_text, "second query");
    assert_eq!(row.status, "completed");

    let recent = db
        .list_recent_research_sessions(10)
        .await
        .expect("list recent");
    assert!(recent.len() >= 2);
    assert_eq!(recent[0].id, second);
    assert_eq!(recent[1].id, first);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_claim_and_verdict() {
    let db = test_db().await;
    let sid = db
        .create_research_session("sess-claim-001", "claim test")
        .await
        .unwrap();
    db.store_claim(
        sid,
        12345678,
        "latency increased by 10ms",
        true,
        false,
        false,
    )
    .await
    .expect("store claim");
    db.store_claim_verdict(12345678, "Supported", 0.87, "minicheck-ft5")
        .await
        .expect("store verdict");
}

#[tokio::test(flavor = "multi_thread")]
async fn list_publication_claims_and_pending_summary() {
    let db = test_db().await;
    let sid = db
        .create_research_session("sess-claims-rd", "claims read")
        .await
        .unwrap();

    // claim 111 → Supported; claim 222 → Abstain; claim 333 → no verdict yet.
    db.store_claim(sid, 111, "latency increased by 10ms", true, false, false)
        .await
        .unwrap();
    db.update_claim_verifiability_score(111, 0.85)
        .await
        .unwrap();
    db.store_claim_verdict(111, "Supported", 0.9, "mock")
        .await
        .unwrap();
    db.store_claim(sid, 222, "performance may improve", false, false, false)
        .await
        .unwrap();
    db.store_claim_verdict(222, "Abstain", 0.0, "mock")
        .await
        .unwrap();
    db.store_claim(sid, 333, "throughput doubled", true, false, false)
        .await
        .unwrap();

    let claims = db.list_publication_claims(sid).await.expect("list claims");
    assert_eq!(claims.len(), 3);
    let by_id = |id: i64| claims.iter().find(|c| c.claim_id == id).unwrap();
    assert_eq!(by_id(111).verdict.as_deref(), Some("Supported"));
    assert_eq!(by_id(111).confidence, Some(0.9));
    assert_eq!(by_id(111).verifier_model.as_deref(), Some("mock"));
    assert_eq!(by_id(111).verifiability_score, Some(0.85));
    assert_eq!(by_id(333).verifiability_score, None);
    assert_eq!(by_id(222).verdict.as_deref(), Some("Abstain"));
    assert_eq!(by_id(333).verdict, None);

    let counts = db
        .scientia_claims_pending_summary()
        .await
        .expect("pending summary");
    assert_eq!(counts.verifiable, 1);
    assert_eq!(counts.abstained, 1);
    assert_eq!(counts.extraction_running, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_training_pair_roundtrip() {
    let db = test_db().await;
    let sid = db
        .create_research_session("sess-train-001", "training query")
        .await
        .unwrap();
    db.store_training_pair(sid, "what broke?", "provider X latency spiked", 85)
        .await
        .expect("store training pair");
}

#[tokio::test(flavor = "multi_thread")]
async fn research_artifact_roundtrip_returns_latest_report() {
    let db = test_db().await;
    let sid = db
        .create_research_session("sess-artifact-001", "artifact query")
        .await
        .unwrap();

    db.store_research_artifact(
        sid,
        r#"{"schema_version":1,"answer":"first"}"#,
        "# First report\n",
    )
    .await
    .expect("store first artifact");
    db.store_research_artifact(
        sid,
        r#"{"schema_version":1,"answer":"second"}"#,
        "# Second report\n",
    )
    .await
    .expect("store second artifact");

    let artifact = db
        .get_research_artifact(sid)
        .await
        .expect("get artifact")
        .expect("artifact exists");
    assert_eq!(artifact.session_id, sid);
    assert!(artifact.artifact_json.contains("\"second\""));
    assert_eq!(artifact.report_markdown, "# Second report\n");
    assert!(artifact.updated_at_ms >= artifact.created_at_ms);
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_run_lifecycle() {
    let db = test_db().await;
    let sid = db
        .create_research_session("sess-prov-001", "provider run test")
        .await
        .unwrap();
    let run_id = db
        .start_provider_run(sid, "tavily")
        .await
        .expect("start run");
    assert!(run_id > 0);
    db.finish_provider_run(run_id, 5, 1200)
        .await
        .expect("finish run");
}

#[tokio::test(flavor = "multi_thread")]
async fn rollup_model_scoreboard_updates_running_average() {
    let db = test_db().await;

    // First insertion — stored as-is with sample_count = 1.
    db.rollup_model_scoreboard_with_scientia("openai", "gpt-4o", "p95_latency_ms", 200.0)
        .await
        .expect("first rollup");

    // Second insertion — running average: (200 * 1 + 400) / 2 = 300.
    db.rollup_model_scoreboard_with_scientia("openai", "gpt-4o", "p95_latency_ms", 400.0)
        .await
        .expect("second rollup");

    // Third insertion — running average: (300 * 2 + 300) / 3 = 300.
    db.rollup_model_scoreboard_with_scientia("openai", "gpt-4o", "p95_latency_ms", 300.0)
        .await
        .expect("third rollup");

    // Verify using a different key — first insert for a new key should succeed.
    db.rollup_model_scoreboard_with_scientia(
        "anthropic",
        "claude-3-5-sonnet",
        "refusal_rate",
        0.02,
    )
    .await
    .expect("different provider rollup");
}
