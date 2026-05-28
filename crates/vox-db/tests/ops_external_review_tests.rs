use vox_db::{
    DbConfig, VoxDb,
    store::types::{
        ExternalReviewDeadletterParams, ExternalReviewFindingParams,
        ExternalReviewFindingStateParams, ExternalReviewRunParams, ExternalReviewThreadParams,
    },
};

#[tokio::test]
async fn external_review_upsert_and_state_roundtrip() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");

    let run_id = db
        .insert_external_review_run(ExternalReviewRunParams {
            provider: "coderabbit",
            repository_id: "vox-foundation/vox",
            owner: "vox-foundation",
            repo: "vox",
            pr_number: 42,
            commit_sha: Some("abc123"),
            trigger_kind: "full_review",
            idempotency_key: Some("test-run-1"),
            item_count: 0,
            metadata_json: Some("{\"source\":\"test\"}"),
        })
        .await
        .expect("insert run");

    db.upsert_external_review_thread(ExternalReviewThreadParams {
        provider: "coderabbit",
        repository_id: "vox-foundation/vox",
        pr_number: 42,
        thread_identity: "thread:42:1",
        placement_kind: "inline",
        line_anchor_state: "current",
        file_path: Some("crates/vox-cli/src/main.rs"),
        line_start: Some(10),
        line_end: Some(10),
        source_comment_id: Some(9001),
        parent_comment_id: None,
        source_payload_hash: "payload-hash-1",
        raw_payload_json: "{\"body\":\"nit\"}",
    })
    .await
    .expect("upsert thread");

    let finding_id = db
        .upsert_external_review_finding(ExternalReviewFindingParams {
            run_id,
            provider: "coderabbit",
            repository_id: "vox-foundation/vox",
            pr_number: 42,
            finding_identity: "finding:42:1",
            thread_identity: Some("thread:42:1"),
            source_comment_id: Some(9001),
            placement_kind: "inline",
            line_anchor_state: "current",
            file_path: Some("crates/vox-cli/src/main.rs"),
            line_start: Some(10),
            line_end: Some(10),
            category: "style",
            anti_pattern_id: Some("review/style-nitpick"),
            severity: "info",
            title: "Prefer clearer naming",
            details: "Rename variable x to something clearer.",
            suggested_fix: Some("let clearer_name = x;"),
            extraction_confidence: Some(0.81),
            source_payload_hash: "payload-hash-1",
            fingerprint: "fp-42-1",
            status: "open",
        })
        .await
        .expect("upsert finding");

    db.append_external_review_finding_state(ExternalReviewFindingStateParams {
        finding_id,
        previous_state: Some("open"),
        new_state: "confirmed_true",
        reason: Some("fix merged"),
        confidence: Some(0.92),
        evidence_ref: Some("commit:deadbeef"),
    })
    .await
    .expect("append state");

    let latest = db
        .latest_external_review_run("vox-foundation/vox", 42)
        .await
        .expect("latest")
        .expect("row");
    assert_eq!(latest.id, run_id);

    let findings = db
        .list_external_review_findings_for_training_window("vox-foundation/vox", 50)
        .await
        .expect("list findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].fingerprint, "fp-42-1");
    assert_eq!(findings[0].status, "confirmed_true");

    assert_eq!(
        db.count_external_review_runs_for_repository("vox-foundation/vox")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        db.count_external_review_findings_for_repository("vox-foundation/vox")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        db.count_external_review_deadletters_pending_for_repository("vox-foundation/vox")
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn external_review_deadletter_retry_flow() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    db.insert_external_review_deadletter(ExternalReviewDeadletterParams {
        provider: "coderabbit",
        repository_id: "vox-foundation/vox",
        pr_number: 77,
        source_kind: "review",
        source_comment_id: Some(1234),
        source_payload_hash: "dl-hash-1",
        error_class: "parse_error",
        error_message: Some("unknown markdown shape"),
        raw_payload_json: "{\"body\":\"??\"}",
    })
    .await
    .expect("insert deadletter");

    let rows = db
        .list_external_review_deadletters("vox-foundation/vox", 77, 20)
        .await
        .expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].retry_state, "pending");

    db.mark_external_review_deadletter_retried(rows[0].id)
        .await
        .expect("mark retried");
    let rows2 = db
        .list_external_review_deadletters("vox-foundation/vox", 77, 20)
        .await
        .expect("list after");
    assert_eq!(rows2[0].retry_state, "retried");
    assert_eq!(
        db.count_external_review_deadletters_pending_for_repository("vox-foundation/vox")
            .await
            .unwrap(),
        0
    );
}
