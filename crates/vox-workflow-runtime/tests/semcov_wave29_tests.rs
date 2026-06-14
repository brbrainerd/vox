//! Adversarial tests for vox-workflow-runtime modules not covered by semcov_wave7_tests.
//! Targets: file_journal, journal/execute, workflow/return_extract, workflow/plan (plan.rs).

#![cfg(feature = "test-support")]

use serde_json::json;
use std::sync::Mutex;
use vox_workflow_runtime::{journal, workflow::extract_terminal_return};

mod semcov_wave29_tests {
    use super::*;
    use vox_workflow_runtime::FileJournalTracker;
    use vox_workflow_runtime::WorkflowTracker;

    // ── journal::execute ───────────────────────────────────────────────────────

    // Serialise all journal-execute tests: in-memory state is process-global.
    static JLOCK: Mutex<()> = Mutex::new(());

    // Catches: seed_completed silently shadowed by a second call with a different
    // value — the last write should win and replay must return the latest seed.
    #[tokio::test]
    async fn seed_overwrite_replays_latest_value() {
        let _g = JLOCK.lock().unwrap_or_else(|p| p.into_inner());
        journal::test_support::reset();
        journal::test_support::seed_completed("act-overwrite", json!(1i64));
        journal::test_support::seed_completed("act-overwrite", json!(99i64));
        let result: Result<i64, anyhow::Error> =
            journal::execute("act-overwrite", async { Ok(0i64) }).await;
        assert_eq!(result.unwrap(), 99, "second seed should shadow the first");
    }

    // Catches: body executing and also recording when a seed is present — body
    // must be completely bypassed on replay.
    #[tokio::test]
    async fn seeded_activity_never_executes_body() {
        let _g = JLOCK.lock().unwrap_or_else(|p| p.into_inner());
        journal::test_support::reset();
        journal::test_support::seed_completed("act-bypass", json!(7i64));

        let body_ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = body_ran.clone();
        let _: i64 = journal::execute("act-bypass", async move {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(0i64)
        })
        .await
        .unwrap();

        assert!(
            !body_ran.load(std::sync::atomic::Ordering::SeqCst),
            "body must not run when a seed exists"
        );
    }

    // Catches: failed body leaking a partial journal entry — recorded_for must
    // stay empty even after multiple failing attempts.
    #[tokio::test]
    async fn repeated_body_failure_never_records() {
        let _g = JLOCK.lock().unwrap_or_else(|p| p.into_inner());
        journal::test_support::reset();

        for _ in 0..3 {
            let result: Result<i64, anyhow::Error> =
                journal::execute("act-fail", async { Err(anyhow::anyhow!("boom")) }).await;
            assert!(result.is_err());
        }
        let recorded = journal::test_support::recorded_for("act-fail");
        assert_eq!(
            recorded.len(),
            0,
            "failed bodies must never produce journal entries"
        );
    }

    // Catches: execute recording duplicate entries for the same activity_id on
    // successive successful calls (double-record in the append path).
    #[tokio::test]
    async fn successful_body_records_exactly_once_per_call() {
        let _g = JLOCK.lock().unwrap_or_else(|p| p.into_inner());
        journal::test_support::reset();

        let _: i64 = journal::execute("act-once", async { Ok(1i64) })
            .await
            .unwrap();
        let _: i64 = journal::execute("act-once", async { Ok(2i64) })
            .await
            .unwrap();
        // Two independent calls — both ran the body (no seeded replay) and both
        // should have recorded.
        let recorded = journal::test_support::recorded_for("act-once");
        assert_eq!(
            recorded.len(),
            2,
            "each successful body execution records one entry"
        );
    }

    // Catches: seed with wrong JSON type silently coercing instead of returning
    // a deserialise error.
    #[tokio::test]
    async fn seed_type_mismatch_returns_error() {
        let _g = JLOCK.lock().unwrap_or_else(|p| p.into_inner());
        journal::test_support::reset();
        // Seed a string but caller expects i64.
        journal::test_support::seed_completed("act-type", json!("not-a-number"));
        let result: Result<i64, anyhow::Error> =
            journal::execute("act-type", async { Ok(0i64) }).await;
        assert!(
            result.is_err(),
            "type-mismatched seed must produce a deserialize error"
        );
    }

    // Catches: reset() not clearing the recorded map — stale entries bleed into
    // the next test.
    #[tokio::test]
    async fn reset_clears_both_seeded_and_recorded_state() {
        let _g = JLOCK.lock().unwrap_or_else(|p| p.into_inner());
        journal::test_support::reset();
        journal::test_support::seed_completed("act-reset", json!(42i64));
        let _: i64 = journal::execute("act-reset", async { Ok(0i64) })
            .await
            .unwrap();

        journal::test_support::reset();

        // Seeded state should be gone — body runs normally.
        let body_ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = body_ran.clone();
        let val: i64 = journal::execute("act-reset", async move {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(77i64)
        })
        .await
        .unwrap();
        assert_eq!(val, 77);
        assert!(body_ran.load(std::sync::atomic::Ordering::SeqCst));

        // Recorded state from previous run must also be gone.
        let prev = journal::test_support::recorded_for("act-reset");
        assert_eq!(
            prev.len(),
            1,
            "only the fresh run's entry should be present"
        );
    }

    // ── return_extract::extract_terminal_return ────────────────────────────────

    // Catches: extract selecting the FIRST WorkflowCompleted event when there
    // should only ever be one — ensure it picks the LAST (backwards scan).
    #[test]
    fn extract_picks_last_completed_event_on_duplicate() {
        let journal = vec![
            json!({"event": "WorkflowCompleted", "return_value": 1i64}),
            json!({"event": "WorkflowCompleted", "return_value": 2i64}),
        ];
        let got: i64 = extract_terminal_return(&journal).expect("extract");
        assert_eq!(
            got, 2,
            "backwards scan must return the last WorkflowCompleted"
        );
    }

    // Catches: extract matching a case-variant event name (e.g. "workflowcompleted")
    // and accidentally treating it as terminal.
    #[test]
    fn extract_is_case_sensitive_on_event_name() {
        let journal = vec![
            json!({"event": "workflowcompleted", "return_value": 99i64}),
            json!({"event": "WORKFLOWCOMPLETED", "return_value": 99i64}),
        ];
        let result: Result<i64, _> = extract_terminal_return(&journal);
        assert!(
            result.is_err(),
            "event name matching must be case-sensitive"
        );
    }

    // Catches: empty journal panicking or returning wrong error variant.
    #[test]
    fn extract_returns_no_terminal_for_empty_journal() {
        let journal: Vec<serde_json::Value> = vec![];
        let result: Result<i64, _> = extract_terminal_return(&journal);
        assert!(result.is_err(), "empty journal must return an error");
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("WorkflowCompleted") || err_str.contains("terminal"),
            "error message should mention the missing terminal event; got: {err_str}"
        );
    }

    // Catches: null return_value silently deserializing to Option<T> when caller
    // expects a non-optional T.
    #[test]
    fn extract_null_return_value_fails_for_non_option_type() {
        let journal = vec![json!({"event": "WorkflowCompleted", "return_value": null})];
        let result: Result<i64, _> = extract_terminal_return(&journal);
        assert!(
            result.is_err(),
            "null return_value must not silently coerce to a non-optional integer"
        );
    }

    // Catches: extract working correctly when many non-terminal events precede
    // the terminal (regression: early-exit in the linear scan).
    #[test]
    fn extract_works_with_many_preceding_activity_events() {
        let mut journal: Vec<serde_json::Value> = (0..50)
            .map(|i| json!({"event": "ActivityCompleted", "activity": format!("step{i}"), "value": i}))
            .collect();
        journal.push(json!({"event": "WorkflowCompleted", "return_value": {"result": "ok"}}));

        let got: serde_json::Value = extract_terminal_return(&journal).expect("extract");
        assert_eq!(got, json!({"result": "ok"}));
    }

    // ── FileJournalTracker ─────────────────────────────────────────────────────

    fn tmp_path(tag: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let n = N.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("semcov_w29_{tag}_{pid}_{n}.jsonl"))
    }

    // Catches: load_activity_result returning Some for an activity_id that was
    // never recorded (key collision between different workflow names).
    #[tokio::test]
    async fn different_workflow_names_are_isolated() {
        let path = tmp_path("wf_isolation");
        let _ = std::fs::remove_file(&path);

        let mut t = FileJournalTracker::new(&path).expect("open");
        t.on_activity_completed("wf-A", "act", "id-1", &json!({"wf": "A"}))
            .await
            .unwrap();

        // wf-B should not see wf-A's entry under the same activity_id.
        let result = t.load_activity_result("wf-B", "id-1").await.unwrap();
        assert!(
            result.is_none(),
            "activity result must be scoped to (workflow_name, activity_id)"
        );
        std::fs::remove_file(&path).ok();
    }

    // Catches: is_activity_completed returning true for an activity_id that was
    // only recorded under a different workflow name.
    #[tokio::test]
    async fn is_completed_respects_workflow_name_boundary() {
        let path = tmp_path("is_completed_boundary");
        let _ = std::fs::remove_file(&path);

        let mut t = FileJournalTracker::new(&path).expect("open");
        t.on_activity_completed("wf-X", "act", "shared-id", &json!(1))
            .await
            .unwrap();

        let completed = t.is_activity_completed("wf-Y", "shared-id").await.unwrap();
        assert!(
            !completed,
            "is_activity_completed must not match across workflow names"
        );
        std::fs::remove_file(&path).ok();
    }

    // Catches: on_activity_completed not updating the in-memory index after
    // a successful journal write — is_completed returns false immediately after record.
    #[tokio::test]
    async fn in_memory_index_updated_immediately_after_record() {
        let path = tmp_path("inmem_index");
        let _ = std::fs::remove_file(&path);

        let mut t = FileJournalTracker::new(&path).expect("open");
        assert!(
            !t.is_activity_completed("wf", "act-1").await.unwrap(),
            "not completed before record"
        );
        t.on_activity_completed("wf", "act", "act-1", &json!(42))
            .await
            .unwrap();
        assert!(
            t.is_activity_completed("wf", "act-1").await.unwrap(),
            "must be completed immediately after record without reopen"
        );
        std::fs::remove_file(&path).ok();
    }

    // Catches: load_workflow_patch returning a stale version after a newer
    // record_workflow_patch for the same (workflow_name, change_id).
    #[tokio::test]
    async fn record_workflow_patch_overwrites_in_memory() {
        let path = tmp_path("patch_overwrite");
        let _ = std::fs::remove_file(&path);

        let mut t = FileJournalTracker::new(&path).expect("open");
        t.record_workflow_patch("wf", "cid-1", 1).await.unwrap();
        t.record_workflow_patch("wf", "cid-1", 2).await.unwrap();

        let v = t.load_workflow_patch("wf", "cid-1").await.unwrap();
        assert_eq!(
            v,
            Some(2),
            "in-memory patch version must reflect latest write"
        );
        std::fs::remove_file(&path).ok();
    }

    // Catches: recorded_count inflating on re-records for the same activity_id
    // (HashMap::insert should replace, not add).
    #[tokio::test]
    async fn recorded_count_does_not_inflate_on_re_record() {
        let path = tmp_path("count_stable");
        let _ = std::fs::remove_file(&path);

        let mut t = FileJournalTracker::new(&path).expect("open");
        t.on_activity_completed("wf", "act", "dup-id", &json!(1))
            .await
            .unwrap();
        t.on_activity_completed("wf", "act", "dup-id", &json!(2))
            .await
            .unwrap();

        // HashMap key is (workflow_name, activity_id); inserting twice must not
        // grow the count beyond 1.
        assert_eq!(
            t.recorded_count(),
            1,
            "re-recording same activity_id must not inflate recorded_count"
        );
        std::fs::remove_file(&path).ok();
    }

    // Catches: replay on reopen dropping entries that share workflow_name but
    // have different activity_ids — verifies the whole set survives a crash-cycle.
    #[tokio::test]
    async fn replay_preserves_all_activity_ids_under_same_workflow() {
        let path = tmp_path("replay_multi");
        let _ = std::fs::remove_file(&path);

        {
            let mut t = FileJournalTracker::new(&path).expect("open");
            for i in 0..10u32 {
                t.on_activity_completed("wf", "act", &format!("id-{i}"), &json!(i))
                    .await
                    .unwrap();
            }
        }

        let t2 = FileJournalTracker::new(&path).expect("reopen");
        assert_eq!(
            t2.recorded_count(),
            10,
            "all 10 entries must survive replay"
        );
        for i in 0..10u32 {
            let v = t2
                .load_activity_result("wf", &format!("id-{i}"))
                .await
                .unwrap();
            assert_eq!(v, Some(json!(i)), "entry id-{i} must replay correctly");
        }
        std::fs::remove_file(&path).ok();
    }

    // Catches: load_workflow_patch returning None after reopen when a patch was
    // written and the file was closed — patch entries not surviving replay.
    #[tokio::test]
    async fn workflow_patch_survives_reopen() {
        let path = tmp_path("patch_reopen");
        let _ = std::fs::remove_file(&path);

        {
            let mut t = FileJournalTracker::new(&path).expect("open");
            t.record_workflow_patch("wf", "change-reopen", 7)
                .await
                .unwrap();
        }

        let t2 = FileJournalTracker::new(&path).expect("reopen");
        assert_eq!(
            t2.load_workflow_patch("wf", "change-reopen").await.unwrap(),
            Some(7),
            "patch version must survive journal reopen"
        );
        std::fs::remove_file(&path).ok();
    }

    // Catches: path() returning a path that doesn't match the constructor argument
    // (e.g., some canonicalization or normalization bug that changes the path).
    #[test]
    fn path_matches_constructor_argument() {
        let path = tmp_path("path_check");
        let _ = std::fs::remove_file(&path);
        let t = FileJournalTracker::new(&path).expect("open");
        // Paths should refer to the same file even if representation differs.
        assert_eq!(
            t.path().file_name(),
            path.file_name(),
            "tracker path() must refer to the same file"
        );
        std::fs::remove_file(&path).ok();
    }
}
