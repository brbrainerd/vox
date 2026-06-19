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
    // agy -p (--print) is the only headless mode on Windows — it submits the prompt and
    // returns a text response but does not invoke file-write tools in this mode.
    // The smoke validates: agy is reachable + authenticated + returns a non-empty response.
    // File-write verification requires interactive mode (TTY) which is not available headlessly.
    let spec = AgySpec {
        task: "Reply with the single word: smoke-ok".to_string(),
        model: None,
        timeout_secs: 120,
    };
    let out = exec.run(&spec).await.expect("agy spawn failed");
    eprintln!("stdout={}", &out.stdout[..out.stdout.len().min(500)]);
    eprintln!("stderr={}", &out.stderr[..out.stderr.len().min(500)]);
    eprintln!("exit_code={} timed_out={} elapsed_ms={}", out.exit_code, out.timed_out, out.elapsed_ms);

    assert!(!out.timed_out, "smoke task timed out");
    assert_eq!(out.exit_code, 0, "agy exited non-zero: {}", &out.stderr[..out.stderr.len().min(300)]);
    assert!(!out.stdout.trim().is_empty(), "agy returned no output — auth or connectivity failure");

    // outcome is 'partial' (no file changes) because -p mode is chat-only.
    let (diff, files_changed) = wt.capture().await.expect("capture failed");
    eprintln!("files_changed={files_changed}\ndiff_head={}…", &diff[..diff.len().min(200)]);

    wt.cleanup(&repo_root).await.expect("cleanup failed");
}
