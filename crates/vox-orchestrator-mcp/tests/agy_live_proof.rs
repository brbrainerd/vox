//! Live end-to-end proof: launch agy, write a file, prove tokens were spent.
//!
//! Prerequisites:
//!   - `agy` v1.0.9+ on PATH, interactive Google Sign-In complete once
//!   - Run from the repo root (a git work tree with a committed HEAD)
//!
//! Run with:
//!   cargo test -p vox-orchestrator-mcp --test agy_live_proof -- --ignored --nocapture

use vox_orchestrator_mcp::agy_doctor::{AgyStatus, detect};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

const PROOF_TASK: &str = "\
Create a file named PROOF.md in the current directory containing exactly \
this markdown content:\n\n\
# Proof\n\n\
This file was written by agy to prove end-to-end delegation works.\n\n\
Use your file-writing tool. Do not run any git commands. — no other files.";

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits; run manually: cargo test -p vox-orchestrator-mcp --test agy_live_proof -- --ignored --nocapture"]
async fn live_agy_writes_a_file_and_exits_zero() {
    match detect() {
        AgyStatus::Ready { .. } => {}
        other => panic!(
            "agy not ready ({:?}). Complete Google Sign-In first.",
            other
        ),
    }

    let repo_root = std::env::current_dir().expect("cwd");
    let slug = format!(
        "live-proof-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let wt = DelegationWorktree::create(&repo_root, &slug)
        .await
        .expect("worktree creation failed — repo must have a committed HEAD");

    eprintln!("worktree: {}", wt.path.display());
    eprintln!("branch:   {}", wt.branch);

    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec {
        task: PROOF_TASK.into(),
        model: None,
        timeout_secs: 180,
    };
    let out = exec.run(&spec).await.expect("agy spawn failed");

    let (diff, files_changed) = wt.capture().await.expect("capture failed");
    let proof_contents = std::fs::read_to_string(wt.path.join("PROOF.md")).unwrap_or_default();

    // Clean up BEFORE asserting so worktree is always removed on failure.
    wt.cleanup(&repo_root).await.expect("cleanup failed");

    eprintln!("--- agy stdout (first 2000 chars) ---");
    eprintln!("{}", &out.stdout.chars().take(2000).collect::<String>());
    eprintln!("--- hitl_responses: {:?}", out.hitl_responses);
    eprintln!("--- diff (first 500 chars) ---");
    eprintln!("{}", &diff.chars().take(500).collect::<String>());
    eprintln!(
        "exit={} timed_out={} files_changed={}",
        out.exit_code, out.timed_out, files_changed
    );

    assert!(
        !out.timed_out,
        "agy timed out — increase timeout_secs or simplify the task"
    );
    assert_eq!(out.exit_code, 0, "agy exited non-zero");
    assert!(files_changed > 0, "agy ran but wrote no files");
    assert!(
        proof_contents.contains("Proof"),
        "PROOF.md content unexpected: {}",
        proof_contents
    );
}
