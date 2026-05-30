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
