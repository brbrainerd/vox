//! Live end-to-end smoke for the classifier path against a REAL agy run.
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp agy_pipeline_smoke -- --ignored
//! Prereqs: agy authenticated; run from the repo root (committed HEAD).

use vox_orchestrator_mcp::agy_doctor::{detect, AgyStatus};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_gates::{run_gates, Gate};
use vox_orchestrator_mcp::agy_pipeline::classify_outcome;
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn smoke_pipeline_classifies_a_real_run() {
    assert!(matches!(detect(), AgyStatus::Ready { .. }), "agy must be ready");

    let repo_root = std::env::current_dir().expect("cwd");
    let wt = DelegationWorktree::create(&repo_root, "pipe-smoke-00").await.expect("worktree");

    // agy -p (--print) is chat-only on Windows — it never writes files.
    // The pipeline smoke validates: exec → capture → gates → classifier chain.
    // With 0 file changes and a passing gate the outcome is correctly "failed"
    // (files_changed == 0 → failed per the classifier, regardless of gates).
    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec {
        task: "Reply with the single word: pipeline-ok".into(),
        model: None,
        timeout_secs: 120,
    };
    let out = exec.run(&spec).await.expect("agy spawn");
    eprintln!("exit={} timed_out={} elapsed_ms={}", out.exit_code, out.timed_out, out.elapsed_ms);
    assert_eq!(out.exit_code, 0, "agy should exit 0");
    assert!(!out.stdout.trim().is_empty(), "agy response must be non-empty");

    let (_diff, files_changed) = wt.capture().await.expect("capture");
    let gates = vec![Gate { name: "probe".into(), program: "git".into(), args: vec!["--version".into()], ..Default::default() }];
    let results = run_gates(&wt.path, &gates, 60).await;
    assert!(results[0].passed, "git --version probe gate must pass");

    let outcome = classify_outcome(files_changed, &results, out.timed_out);
    eprintln!("files_changed={files_changed} outcome={outcome}");
    // -p mode makes no file changes → classifier correctly returns "failed"
    assert_eq!(outcome, "failed", "0 file changes → outcome must be 'failed'");

    wt.cleanup(&repo_root).await.expect("cleanup");
}
