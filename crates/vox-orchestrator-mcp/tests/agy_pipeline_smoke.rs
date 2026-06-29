//! Live end-to-end smoke for the classifier path against a REAL agy run.
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp agy_pipeline_smoke -- --ignored
//! Prereqs: agy authenticated; run from the repo root (committed HEAD).

use vox_orchestrator_mcp::agy_doctor::{AgyStatus, detect};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_gates::{Gate, run_gates};
use vox_orchestrator_mcp::agy_pipeline::classify_outcome;
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn smoke_pipeline_classifies_a_real_run() {
    assert!(
        matches!(detect(), AgyStatus::Ready { .. }),
        "agy must be ready"
    );

    let repo_root = std::env::current_dir().expect("cwd");
    let wt = DelegationWorktree::create(&repo_root, "pipe-smoke-00")
        .await
        .expect("worktree");

    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec {
        task: "Create a new file .vox/pipeline-smoke.txt containing 'pipeline-ok'. No other files."
            .into(),
        model: None,
        timeout_secs: 180,
    };
    let out = exec.run(&spec).await.expect("agy spawn");
    eprintln!(
        "exit={} timed_out={} elapsed_ms={}",
        out.exit_code, out.timed_out, out.elapsed_ms
    );

    let (_diff, files_changed) = wt.capture().await.expect("capture");
    let gates = vec![Gate {
        name: "probe".into(),
        program: "git".into(),
        args: vec!["--version".into()],
        ..Default::default()
    }];
    let results = run_gates(&wt.path, &gates, 60).await;

    let outcome = classify_outcome(files_changed, &results, out.timed_out);
    eprintln!("files_changed={files_changed} outcome={outcome}");
    assert_eq!(outcome, "green", "expected a verified green run");

    wt.cleanup(&repo_root).await.expect("cleanup");
}
