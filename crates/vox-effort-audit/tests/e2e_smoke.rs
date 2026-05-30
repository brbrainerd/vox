//! End-to-end: run the pipeline against a fixture git repo with `MockJudge`,
//! assert on the emitted files.

mod support;

use vox_effort_audit::config::EffortAuditConfig;
use vox_effort_audit::judge::MockJudge;

#[tokio::test]
async fn smoke_run_produces_outputs() {
    let (_g, repo_path) = support::make_smoke_repo();
    let out_dir = tempfile::tempdir().unwrap();
    let cfg = EffortAuditConfig::default();
    let judge = Box::new(MockJudge {
        fixed_score: 5,
        model: "mock".into(),
    });

    let summary = vox_effort_audit::run(
        &repo_path,
        out_dir.path(),
        cfg,
        judge,
        None, // no transcript dir override
    )
    .await
    .unwrap();

    assert!(out_dir.path().join("findings.jsonl").exists());
    assert!(out_dir.path().join("report.md").exists());
    assert!(out_dir.path().join("manifest.json").exists());
    assert_eq!(summary.commits_judged, 5);
    assert_eq!(summary.commits_in_range, 5);
    assert_eq!(summary.commits_skipped, 0);
}

/// E3 regression: with `max_concurrent=4` and a judge that sleeps 200ms/call,
/// 5 commits MUST finish well under the sequential 1000ms baseline. With ideal
/// concurrency the judges take `ceil(5/4) * 200ms = 400ms`; we assert <700ms
/// to leave headroom for the synchronous walk/shape/IO work the pipeline does
/// before and around the judge fan-out on a cold CI runner.
///
/// We disable transcript scanning explicitly so the timer measures the
/// concurrency primitive itself, not filesystem-bound transcript correlation.
#[tokio::test]
async fn concurrent_judge_completes_under_budget() {
    let (_g, repo_path) = support::make_smoke_repo();
    let out_dir = tempfile::tempdir().unwrap();
    let mut cfg = EffortAuditConfig::default();
    cfg.max_concurrent = 4;
    cfg.with_transcripts = false;

    struct SlowMock;
    #[async_trait::async_trait]
    impl vox_effort_audit::judge::Judge for SlowMock {
        async fn judge_one(
            &self,
            rec: &vox_effort_audit::walk::CommitRecord,
            shape: &vox_effort_audit::shape::ShapeFeatures,
        ) -> vox_effort_audit::judge::JudgeOutcome {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            vox_effort_audit::judge::MockJudge {
                fixed_score: 3,
                model: "slow".into(),
            }
            .judge_one(rec, shape)
            .await
        }
        fn model_id(&self) -> &str {
            "slow"
        }
    }

    let started = std::time::Instant::now();
    let _ = vox_effort_audit::run(&repo_path, out_dir.path(), cfg, Box::new(SlowMock), None)
        .await
        .unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(700),
        "elapsed: {elapsed:?} (sequential baseline would be ~1000ms)"
    );
}
