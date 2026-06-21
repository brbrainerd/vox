//! Live integration smoke test for vox_agy_delegate.
//!
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp agy_delegate_smoke -- --ignored
//!
//! Prerequisites:
//!   - `agy` v1.0.9+ on PATH, interactive Google login complete
//!   - Run from the repo root (a git work tree with committed HEAD)

use vox_orchestrator_mcp::agy_doctor::{AgyStatus, detect};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn smoke_delegate_trivial_task() {
    let status = detect();
    assert!(
        matches!(status, AgyStatus::Ready { .. }),
        "agy must be ready before running smoke test: {status:?}"
    );

    let repo_root = std::env::current_dir().expect("cwd must be set");
    let slug = "smoke-test-00";
    let wt = DelegationWorktree::create(&repo_root, slug)
        .await
        .expect("worktree creation failed");

    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec {
        task: "echo 'smoke-ok' into a new file named .vox/agy-smoke.txt. \
               No other files may be touched."
            .to_string(),
        model: None,
        timeout_secs: 120,
    };
    let out = exec.run(&spec).await.expect("agy spawn failed");
    eprintln!("stdout={}", &out.stdout[..out.stdout.len().min(500)]);
    eprintln!("stderr={}", &out.stderr[..out.stderr.len().min(500)]);
    eprintln!(
        "exit_code={} timed_out={} elapsed_ms={}",
        out.exit_code, out.timed_out, out.elapsed_ms
    );

    assert!(!out.timed_out, "smoke task timed out");
    assert_eq!(
        out.exit_code,
        0,
        "agy exited non-zero: {}",
        &out.stderr[..out.stderr.len().min(300)]
    );

    let (diff, files_changed) = wt.capture().await.expect("capture failed");
    eprintln!(
        "files_changed={files_changed}\ndiff_head={}…",
        &diff[..diff.len().min(400)]
    );
    assert!(
        files_changed > 0,
        "expected ≥1 changed file after delegation"
    );

    wt.cleanup(&repo_root).await.expect("cleanup failed");
}
