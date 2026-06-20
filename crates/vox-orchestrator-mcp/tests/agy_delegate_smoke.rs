//! Live integration smoke test for headless agy file-writing.
//!
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp smoke_delegate_writes_a_file -- --ignored --nocapture
//!
//! Prerequisites:
//!   - `agy` v1.0.9+ on PATH, interactive Google login complete
//!   - Run from the repo root (a git work tree with committed HEAD)

use vox_orchestrator_mcp::agy_doctor::{AgyStatus, detect};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn smoke_delegate_writes_a_file() {
    let status = detect();
    assert!(
        matches!(status, AgyStatus::Ready { .. }),
        "agy must be ready before running smoke test: {status:?}"
    );

    let repo_root = std::env::current_dir().expect("cwd must be set");
    // Unique slug → no collision with a prior leaked worktree/branch.
    let slug = format!("smoke-{}", uuid::Uuid::new_v4().simple());
    let wt = DelegationWorktree::create(&repo_root, &slug)
        .await
        .expect("worktree creation failed");

    let exec = AgyExec::new(&wt.path);
    // Tight, tangent-free prompt: write one file, no git, no repo hunting.
    let spec = AgySpec {
        task: "Create a file named delegate-proof.txt in the current directory \
               containing exactly the single line: PROOF-OK\n\
               Use your file-writing tools. Do not run any git commands."
            .to_string(),
        model: None,
        timeout_secs: 180,
    };
    let out = exec.run(&spec).await.expect("agy spawn failed");

    // Capture results BEFORE asserting, then clean up, THEN assert — so a failed
    // assertion can never leak the worktree/branch.
    let (diff, files_changed) = wt.capture().await.expect("capture failed");
    let proof_path = wt.path.join("delegate-proof.txt");
    let proof_contents = std::fs::read_to_string(&proof_path).unwrap_or_default();
    wt.cleanup(&repo_root).await.expect("cleanup failed");

    eprintln!("exit_code={} timed_out={} elapsed_ms={}", out.exit_code, out.timed_out, out.elapsed_ms);
    eprintln!("files_changed={files_changed}\ndiff_head={}…", &diff[..diff.len().min(300)]);
    eprintln!("proof_contents={proof_contents:?}");

    assert!(!out.timed_out, "smoke task timed out");
    assert_eq!(out.exit_code, 0, "agy exited non-zero");
    assert!(files_changed > 0, "expected ≥1 changed file after delegation");
    assert!(
        proof_contents.contains("PROOF-OK"),
        "delegate-proof.txt missing/empty: {proof_contents:?}"
    );
}
