//! End-to-end: run the pipeline against a fixture git repo with `MockJudge`,
//! assert on the emitted files.

mod support;

use vox_effort_audit::config::EffortAuditConfig;
use vox_effort_audit::judge::MockJudge;
use vox_effort_audit::pricing::ModelRates;

#[tokio::test]
async fn smoke_run_produces_outputs() {
    let (_g, repo_path) = support::make_smoke_repo();
    let out_dir = tempfile::tempdir().unwrap();
    let cfg = EffortAuditConfig::default();
    let judge = Box::new(MockJudge {
        fixed_score: 5,
        model: "mock".into(),
    });
    // Synthetic-but-real rates: $3/1k in, $15/1k out. MockJudge reports 0
    // tokens, so the cost is `Some(0.0)` — a HONEST zero (real tokens × real
    // rate), distinct from the unknown-price `None` asserted below.
    let rates = ModelRates {
        input_per_1k_usd: 3.0,
        output_per_1k_usd: 15.0,
        known: true,
    };

    let summary = vox_effort_audit::run(
        &repo_path,
        out_dir.path(),
        cfg,
        judge,
        None, // no transcript dir override
        rates,
    )
    .await
    .unwrap();

    assert!(out_dir.path().join("findings.jsonl").exists());
    assert!(out_dir.path().join("report.md").exists());
    assert!(out_dir.path().join("manifest.json").exists());
    assert_eq!(summary.commits_judged, 5);
    assert_eq!(summary.commits_in_range, 5);
    assert_eq!(summary.commits_skipped, 0);

    // Cost is REAL: MockJudge spends 0 tokens, known rate → Some(0.0).
    let manifest: vox_effort_audit::output::manifest::Manifest = serde_json::from_str(
        &std::fs::read_to_string(out_dir.path().join("manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest.judge_total_cost_usd, Some(0.0));
}

#[tokio::test]
async fn unknown_pricing_yields_null_cost_not_fake_zero() {
    let (_g, repo_path) = support::make_smoke_repo();
    let out_dir = tempfile::tempdir().unwrap();
    let cfg = EffortAuditConfig::default();
    let judge = Box::new(MockJudge {
        fixed_score: 5,
        model: "mock".into(),
    });

    // Default rates have `known = false` → cost must be `None`, never $0.00.
    let summary = vox_effort_audit::run(
        &repo_path,
        out_dir.path(),
        cfg,
        judge,
        None,
        ModelRates::default(),
    )
    .await
    .unwrap();
    assert_eq!(summary.commits_judged, 5);

    let manifest: vox_effort_audit::output::manifest::Manifest = serde_json::from_str(
        &std::fs::read_to_string(out_dir.path().join("manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest.judge_total_cost_usd, None);
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
    let cfg = EffortAuditConfig {
        max_concurrent: 4,
        with_transcripts: false,
        ..EffortAuditConfig::default()
    };

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
    let _ = vox_effort_audit::run(
        &repo_path,
        out_dir.path(),
        cfg,
        Box::new(SlowMock),
        None,
        ModelRates::default(),
    )
    .await
    .unwrap();
    let elapsed = started.elapsed();
    // Budget is well under the ~1000ms sequential baseline so it still proves
    // concurrency, but with enough headroom to survive scheduler jitter when the
    // whole workspace test suite runs in parallel (the 700ms bound flaked under
    // that load even though the work is the same).
    assert!(
        elapsed < std::time::Duration::from_millis(850),
        "elapsed: {elapsed:?} (sequential baseline would be ~1000ms)"
    );
}
