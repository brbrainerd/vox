//! Live end-to-end smoke for the classifier path against a REAL agy run.
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp smoke_pipeline_classifies_green -- --ignored --nocapture
//! Prereqs: agy authenticated; run from the repo root (committed HEAD).

use vox_orchestrator_mcp::agy_doctor::{AgyStatus, detect};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_gates::{Gate, run_gates};
use vox_orchestrator_mcp::agy_pipeline::classify_outcome;
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn smoke_pipeline_classifies_green() {
    assert!(matches!(detect(), AgyStatus::Ready { .. }), "agy must be ready");

    let repo_root = std::env::current_dir().expect("cwd");
    let slug = format!("pipe-{}", uuid::Uuid::new_v4().simple());
    let wt = DelegationWorktree::create(&repo_root, &slug).await.expect("worktree");

    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec {
        task: "Create a file named pipeline-proof.txt in the current directory \
               containing exactly the single line: pipeline-ok\n\
               Use your file-writing tools. Do not run any git commands."
            .into(),
        model: None,
        timeout_secs: 180,
    };
    let out = exec.run(&spec).await.expect("agy spawn");

    let (_diff, files_changed) = wt.capture().await.expect("capture");
    // A trivially-passing gate so a green run is fully exercised.
    let gates = vec![Gate {
        name: "probe".into(),
        program: "git".into(),
        args: vec!["--version".into()],
        ..Default::default()
    }];
    let results = run_gates(&wt.path, &gates, 60).await;
    let outcome = classify_outcome(files_changed, &results, out.timed_out);

    // Clean up before asserting.
    wt.cleanup(&repo_root).await.expect("cleanup");

    eprintln!("exit={} files_changed={files_changed} gate_passed={} outcome={outcome}",
        out.exit_code, results[0].passed);

    assert_eq!(out.exit_code, 0, "agy should exit 0");
    assert!(files_changed > 0, "agy must have written a file");
    assert!(results[0].passed, "git --version probe gate must pass");
    assert_eq!(outcome, "green", "files changed + gate passed ⇒ green");
}
