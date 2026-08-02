use vox_db::{
    DbConfig, ExternalSubmissionAttemptParams, ExternalSubmissionJobUpsertParams,
    PublicationManifestParams, StoreError, VoxDb,
};
// Only used by the quarantine-gated test below (publication_external_links,
// publication_external_revisions, plus its ExternalStatusSnapshotParams
// call); unused when the feature is off.
#[cfg(feature = "quarantine")]
use vox_db::{
    ExternalStatusSnapshotParams, PublicationExternalLinkUpsertParams,
    PublicationExternalRevisionUpsertParams,
};

#[tokio::test]
async fn publication_manifest_upsert_and_dual_approval_are_digest_bound() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "paper-001",
        content_type: "scientia",
        source_ref: Some("docs/research/paper-001.md"),
        title: "Paper 001",
        author: "alice",
        abstract_text: Some("abstract"),
        body_markdown: "# body",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "digest-a",
        state: "draft",
    })
    .await
    .unwrap();

    db.record_publication_approval_for_digest("paper-001", "digest-a", "alice")
        .await
        .unwrap();
    assert!(
        !db.has_dual_publication_approval_for_digest("paper-001", "digest-a")
            .await
            .unwrap()
    );
    db.record_publication_approval_for_digest("paper-001", "digest-a", "bob")
        .await
        .unwrap();
    assert!(
        db.has_dual_publication_approval_for_digest("paper-001", "digest-a")
            .await
            .unwrap()
    );

    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "paper-001",
        content_type: "scientia",
        source_ref: Some("docs/research/paper-001.md"),
        title: "Paper 001 v2",
        author: "alice",
        abstract_text: Some("abstract"),
        body_markdown: "# body v2",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "digest-b",
        state: "draft",
    })
    .await
    .unwrap();
    assert!(
        !db.has_dual_publication_approval_for_digest("paper-001", "digest-b")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn scholarly_submission_upsert_tracks_status() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "paper-002",
        content_type: "scientia",
        source_ref: None,
        title: "Paper 002",
        author: "alice",
        abstract_text: None,
        body_markdown: "# body",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "digest-x",
        state: "approved",
    })
    .await
    .unwrap();

    db.upsert_scholarly_submission(
        "paper-002",
        "digest-x",
        "local_ledger",
        "local-abc",
        "submitted",
        Some("fp-1"),
        None,
    )
    .await
    .unwrap();
    db.upsert_scholarly_submission(
        "paper-002",
        "digest-x",
        "local_ledger",
        "local-abc",
        "accepted",
        Some("fp-2"),
        None,
    )
    .await
    .unwrap();

    let rows = db.list_scholarly_submissions("paper-002").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "accepted");
    let manifest = db
        .get_publication_manifest("paper-002")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(manifest.state, "accepted");
}

// Touches publication_external_links, which is quarantined (DORMANT, no
// confirmed caller outside vox-db even via wrapper-call detection — Task 4,
// VoxDB audit condensation plan) — off by default, see
// crates/vox-db/src/schema/domains/quarantine.rs. Not caught by the
// test-census scan because most of this test exercises still-live tables
// (external_submission_jobs/attempts, external_status_snapshots); only its
// final section references publication_external_links.
#[tokio::test]
#[cfg(feature = "quarantine")]
async fn external_submission_job_attempt_snapshot_and_links_roundtrip() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "ext-job-1",
        content_type: "scientia",
        source_ref: None,
        title: "T",
        author: "a",
        abstract_text: None,
        body_markdown: "b",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "digest-z",
        state: "draft",
    })
    .await
    .unwrap();

    let idem = "zenodo:ext-job-1:digest-z:create";
    let job_id = db
        .upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
            publication_id: "ext-job-1",
            content_sha3_256: "digest-z",
            adapter: "zenodo",
            operation: "create_deposition",
            idempotency_key: idem,
            status: "queued",
            lock_owner: None,
            lock_expires_at_ms: None,
            next_retry_at_ms: None,
            attempt_count: 0,
            last_error_class: None,
            last_error_message: None,
            metadata_json: None,
        })
        .await
        .unwrap();

    let j2 = db
        .upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
            publication_id: "ext-job-1",
            content_sha3_256: "digest-z",
            adapter: "zenodo",
            operation: "create_deposition",
            idempotency_key: idem,
            status: "running",
            lock_owner: Some("worker-a"),
            lock_expires_at_ms: Some(9_999_999),
            next_retry_at_ms: None,
            attempt_count: 0,
            last_error_class: None,
            last_error_message: None,
            metadata_json: Some(r#"{"k":1}"#),
        })
        .await
        .unwrap();
    assert_eq!(job_id, j2);

    let fetched = db
        .get_external_submission_job_by_idempotency_key(idem)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.status, "running");
    assert_eq!(fetched.lock_owner.as_deref(), Some("worker-a"));

    db.record_external_submission_attempt(ExternalSubmissionAttemptParams {
        job_id,
        http_status: Some(429),
        error_class: Some("rate_limit"),
        retryable: true,
        request_fingerprint: Some("req-a"),
        response_fingerprint: Some("res-a"),
        detail_json: Some(r#"{"retry_after":1}"#),
    })
    .await
    .unwrap();

    let attempts = db
        .list_external_submission_attempts_for_job(job_id)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 1);
    assert!(attempts[0].retryable);

    let jobs = db
        .list_external_submission_jobs_for_publication_digest("ext-job-1", "digest-z")
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].attempt_count, 1);

    db.insert_external_status_snapshot(ExternalStatusSnapshotParams {
        adapter: "zenodo",
        external_submission_id: "dep-1",
        publication_id: "ext-job-1",
        content_sha3_256: "digest-z",
        snapshot_json: r#"{"state":"draft"}"#,
    })
    .await
    .unwrap();

    let snap = db
        .get_latest_external_status_snapshot("zenodo", "dep-1")
        .await
        .unwrap()
        .unwrap();
    assert!(snap.snapshot_json.contains("draft"));

    db.upsert_publication_external_link(PublicationExternalLinkUpsertParams {
        publication_id: "ext-job-1",
        content_sha3_256: "digest-z",
        adapter: "zenodo",
        link_kind: "doi",
        link_value: "10.5281/zenodo.123",
        metadata_json: None,
    })
    .await
    .unwrap();

    let links = db
        .list_publication_external_links("ext-job-1", "digest-z")
        .await
        .unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].link_value, "10.5281/zenodo.123");

    db.upsert_publication_external_revision(PublicationExternalRevisionUpsertParams {
        publication_id: "ext-job-1",
        content_sha3_256: "digest-z",
        adapter: "zenodo",
        external_revision: "42",
        metadata_json: Some(r#"{"concept_record_id":"123"}"#),
    })
    .await
    .unwrap();

    let rev = db
        .get_publication_external_revision("ext-job-1", "digest-z", "zenodo")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rev.external_revision, "42");

    db.upsert_publication_external_revision(PublicationExternalRevisionUpsertParams {
        publication_id: "ext-job-1",
        content_sha3_256: "digest-z",
        adapter: "zenodo",
        external_revision: "43",
        metadata_json: None,
    })
    .await
    .unwrap();

    let rev2 = db
        .get_publication_external_revision("ext-job-1", "digest-z", "zenodo")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rev2.external_revision, "43");
    assert!(rev2.metadata_json.is_none());
}

#[tokio::test]
async fn patch_scholarly_submission_status_updates_row_only() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "p-patch",
        content_type: "scientia",
        source_ref: None,
        title: "t",
        author: "a",
        abstract_text: None,
        body_markdown: "b",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "dig-patch",
        state: "approved",
    })
    .await
    .unwrap();
    db.upsert_scholarly_submission(
        "p-patch",
        "dig-patch",
        "zenodo",
        "dep-9",
        "submitted",
        None,
        None,
    )
    .await
    .unwrap();
    let man_after_submit = db
        .get_publication_manifest("p-patch")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(man_after_submit.state, "submitted");

    let n = db
        .patch_scholarly_submission_status("p-patch", "zenodo", "dep-9", "published", None)
        .await
        .unwrap();
    assert_eq!(n, 1u64);
    let rows = db.list_scholarly_submissions("p-patch").await.unwrap();
    assert_eq!(rows[0].status, "published");
    let man = db
        .get_publication_manifest("p-patch")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        man.state, "submitted",
        "patch_scholarly_submission_status must not rewrite publication_manifests.state"
    );
}

#[tokio::test]
async fn list_external_submission_jobs_due_finds_queued_and_retryable() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "due-pub",
        content_type: "scientia",
        source_ref: None,
        title: "t",
        author: "a",
        abstract_text: None,
        body_markdown: "b",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "digest-due",
        state: "draft",
    })
    .await
    .unwrap();

    db.upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
        publication_id: "due-pub",
        content_sha3_256: "digest-due",
        adapter: "zenodo",
        operation: "submit",
        idempotency_key: "k-due-queued",
        status: "queued",
        lock_owner: None,
        lock_expires_at_ms: None,
        next_retry_at_ms: None,
        attempt_count: 0,
        last_error_class: None,
        last_error_message: None,
        metadata_json: None,
    })
    .await
    .unwrap();

    db.upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
        publication_id: "due-pub",
        content_sha3_256: "digest-due",
        adapter: "openreview",
        operation: "submit",
        idempotency_key: "k-due-retry",
        status: "retryable_failed",
        lock_owner: None,
        lock_expires_at_ms: None,
        next_retry_at_ms: Some(1),
        attempt_count: 2,
        last_error_class: Some("transient"),
        last_error_message: Some("timeout"),
        metadata_json: None,
    })
    .await
    .unwrap();

    let jobs = db
        .list_external_submission_jobs_due(i64::MAX, 10)
        .await
        .unwrap();
    let adapters: Vec<&str> = jobs.iter().map(|j| j.adapter.as_str()).collect();
    assert!(adapters.contains(&"zenodo"));
    assert!(adapters.contains(&"openreview"));
}

#[tokio::test]
async fn list_external_submission_jobs_failed_lists_terminal_failed() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "dl-pub",
        content_type: "scientia",
        source_ref: None,
        title: "t",
        author: "a",
        abstract_text: None,
        body_markdown: "b",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "digest-dl",
        state: "draft",
    })
    .await
    .unwrap();

    db.upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
        publication_id: "dl-pub",
        content_sha3_256: "digest-dl",
        adapter: "zenodo",
        operation: "submit",
        idempotency_key: "k-dl-failed-1",
        status: "failed",
        lock_owner: None,
        lock_expires_at_ms: None,
        next_retry_at_ms: None,
        attempt_count: 3,
        last_error_class: Some("fatal"),
        last_error_message: Some("blocked"),
        metadata_json: None,
    })
    .await
    .unwrap();

    db.upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
        publication_id: "dl-pub",
        content_sha3_256: "digest-dl",
        adapter: "zenodo",
        operation: "submit",
        idempotency_key: "k-dl-retry",
        status: "retryable_failed",
        lock_owner: None,
        lock_expires_at_ms: None,
        next_retry_at_ms: Some(1),
        attempt_count: 1,
        last_error_class: Some("transient"),
        last_error_message: Some("timeout"),
        metadata_json: None,
    })
    .await
    .unwrap();

    let jobs = db.list_external_submission_jobs_failed(10).await.unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].idempotency_key, "k-dl-failed-1");
    assert_eq!(jobs[0].status, "failed");

    let replayed = db
        .replay_failed_external_submission_job_to_queued(jobs[0].id)
        .await
        .unwrap();
    assert_eq!(replayed.status, "queued");
    assert!(replayed.lock_owner.is_none());
    assert!(replayed.last_error_class.is_none());
    assert!(
        db.list_external_submission_jobs_failed(10)
            .await
            .unwrap()
            .is_empty()
    );

    let err = db
        .replay_failed_external_submission_job_to_queued(replayed.id)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("replay only allowed"), "{err}");
}

#[tokio::test]
async fn list_publication_ids_with_scholarly_submissions_orders_by_recency() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    for (pid, digest) in [("sync-batch-old", "dig-old"), ("sync-batch-new", "dig-new")] {
        db.upsert_publication_manifest(PublicationManifestParams {
            publication_id: pid,
            content_type: "scientia",
            source_ref: None,
            title: "t",
            author: "a",
            abstract_text: None,
            body_markdown: "b",
            citations_json: None,
            metadata_json: None,
            revision_history_json: None,
            content_sha3_256: digest,
            state: "draft",
        })
        .await
        .unwrap();
    }
    db.upsert_scholarly_submission(
        "sync-batch-old",
        "dig-old",
        "local_ledger",
        "ext-old",
        "submitted",
        None,
        None,
    )
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    db.upsert_scholarly_submission(
        "sync-batch-new",
        "dig-new",
        "local_ledger",
        "ext-new",
        "submitted",
        None,
        None,
    )
    .await
    .unwrap();

    let ids = db
        .list_publication_ids_with_scholarly_submissions(10)
        .await
        .unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], "sync-batch-new");
    assert_eq!(ids[1], "sync-batch-old");
}

#[tokio::test]
async fn append_publication_status_event_does_not_touch_manifest_state() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "handoff-p1",
        content_type: "scientia",
        source_ref: None,
        title: "t",
        author: "a",
        abstract_text: None,
        body_markdown: "b",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "digest-h1",
        state: "draft",
    })
    .await
    .unwrap();
    db.append_publication_status_event(
        "handoff-p1",
        "arxiv_handoff:operator_ack",
        Some(r#"{"note":"ok"}"#),
    )
    .await
    .unwrap();
    let row = db
        .get_publication_manifest("handoff-p1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "draft");
    let evs = db
        .list_publication_status_events("handoff-p1")
        .await
        .unwrap();
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].status, "arxiv_handoff:operator_ack");
}

#[tokio::test]
async fn summarize_scholarly_external_pipeline_metrics_rollup() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "metrics-pub",
        content_type: "scientia",
        source_ref: None,
        title: "t",
        author: "a",
        abstract_text: None,
        body_markdown: "b",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "dig-m",
        state: "draft",
    })
    .await
    .unwrap();

    let job_queued = db
        .upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
            publication_id: "metrics-pub",
            content_sha3_256: "dig-m",
            adapter: "zenodo",
            operation: "submit",
            idempotency_key: "idem-m-q",
            status: "queued",
            lock_owner: None,
            lock_expires_at_ms: None,
            next_retry_at_ms: None,
            attempt_count: 0,
            last_error_class: None,
            last_error_message: None,
            metadata_json: None,
        })
        .await
        .unwrap();

    db.upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
        publication_id: "metrics-pub",
        content_sha3_256: "dig-m",
        adapter: "openreview",
        operation: "submit",
        idempotency_key: "idem-m-ok",
        status: "succeeded",
        lock_owner: None,
        lock_expires_at_ms: None,
        next_retry_at_ms: None,
        attempt_count: 1,
        last_error_class: None,
        last_error_message: None,
        metadata_json: None,
    })
    .await
    .unwrap();

    db.record_external_submission_attempt(ExternalSubmissionAttemptParams {
        job_id: job_queued,
        http_status: Some(503),
        error_class: Some("transient"),
        retryable: true,
        request_fingerprint: None,
        response_fingerprint: None,
        detail_json: Some(r#"{"e":"retry"}"#),
    })
    .await
    .unwrap();

    db.record_publication_attempt("metrics-pub", "dig-m", "reddit", r#"{"dry_run":true}"#)
        .await
        .unwrap();

    let v = db
        .summarize_scholarly_external_pipeline_metrics(0)
        .await
        .unwrap();
    assert_eq!(v["metrics_schema_version"].as_i64(), Some(2));
    let jobs = v["external_submission_jobs"]["by_status"]
        .as_object()
        .expect("by_status object");
    assert_eq!(jobs.get("queued").and_then(|x| x.as_i64()), Some(1));
    assert_eq!(jobs.get("succeeded").and_then(|x| x.as_i64()), Some(1));

    let attempts = &v["external_submission_attempts"];
    assert_eq!(attempts["total_in_window"].as_i64(), Some(1));
    assert_eq!(attempts["retryable_in_window"].as_i64(), Some(1));
    let ec = attempts["by_error_class"].as_object().unwrap();
    assert_eq!(ec.get("transient").and_then(|x| x.as_i64()), Some(1));

    let ch = v["publication_attempts_in_window_by_channel"]
        .as_object()
        .unwrap();
    assert_eq!(ch.get("reddit").and_then(|x| x.as_i64()), Some(1));
    assert!(v["external_submission_jobs"]["by_status_in_window"].is_object());
    assert!(v["external_submission_jobs"]["terminal_latency_ms_percentiles_in_window"].is_object());
}

#[tokio::test]
async fn external_submission_job_upsert_rejects_identity_mismatch() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "idem-pub",
        content_type: "scientia",
        source_ref: None,
        title: "t",
        author: "a",
        abstract_text: None,
        body_markdown: "b",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "dig-a",
        state: "draft",
    })
    .await
    .unwrap();

    let idem = "stable-idem-key";
    db.upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
        publication_id: "idem-pub",
        content_sha3_256: "dig-a",
        adapter: "zenodo",
        operation: "submit",
        idempotency_key: idem,
        status: "queued",
        lock_owner: None,
        lock_expires_at_ms: None,
        next_retry_at_ms: None,
        attempt_count: 0,
        last_error_class: None,
        last_error_message: None,
        metadata_json: None,
    })
    .await
    .unwrap();

    let err = db
        .upsert_external_submission_job(ExternalSubmissionJobUpsertParams {
            publication_id: "idem-pub",
            content_sha3_256: "dig-b",
            adapter: "zenodo",
            operation: "submit",
            idempotency_key: idem,
            status: "queued",
            lock_owner: None,
            lock_expires_at_ms: None,
            next_retry_at_ms: None,
            attempt_count: 0,
            last_error_class: None,
            last_error_message: None,
            metadata_json: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::UpsertIdentityMismatch(_)));
}

#[tokio::test]
async fn scholarly_submission_upsert_rejects_identity_mismatch() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.upsert_publication_manifest(PublicationManifestParams {
        publication_id: "ss-pub-a",
        content_type: "scientia",
        source_ref: None,
        title: "t",
        author: "a",
        abstract_text: None,
        body_markdown: "b",
        citations_json: None,
        metadata_json: None,
        revision_history_json: None,
        content_sha3_256: "dig-a",
        state: "draft",
    })
    .await
    .unwrap();
    db.upsert_scholarly_submission("ss-pub-a", "dig-a", "zenodo", "dep-1", "draft", None, None)
        .await
        .unwrap();

    let err = db
        .upsert_scholarly_submission("ss-pub-b", "dig-a", "zenodo", "dep-1", "draft", None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::UpsertIdentityMismatch(_)));
}
