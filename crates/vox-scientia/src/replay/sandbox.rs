//! Sandboxed subprocess execution.
//!
//! Phase B is *not* trying to implement a security-grade sandbox — we run
//! the child as the host user under a fresh working directory with a hard
//! wall-clock timeout and `kill_on_drop`. Container / seccomp / namespace
//! isolation is a follow-up phase (gated on a feature flag).
//!
//! The runner is platform-aware: POSIX hosts dispatch the entry-point via
//! `sh -c <entry_point>`; Windows hosts use `cmd /C <entry_point>`.

use std::path::Path;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct SandboxOutcome {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub wall_ms: u64,
}

/// Spawn `entry_point` as a shell command inside `cwd` with a `kill_on_drop`
/// child and a hard `timeout_dur` wall-clock budget. Returns captured
/// stdout/stderr (truncation is the caller's job; see `lib::truncate`).
pub async fn run_in_sandbox(
    cwd: &Path,
    entry_point: &str,
    timeout_dur: Duration,
) -> std::io::Result<SandboxOutcome> {
    let start = Instant::now();
    let mut cmd = build_shell_command(entry_point);
    cmd.current_dir(cwd).kill_on_drop(true);

    let exec = cmd.output();
    let res = timeout(timeout_dur, exec).await;
    let wall_ms = start.elapsed().as_millis() as u64;

    match res {
        Ok(Ok(out)) => Ok(SandboxOutcome {
            exit_code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            timed_out: false,
            wall_ms,
        }),
        Ok(Err(e)) => Err(e),
        Err(_elapsed) => Ok(SandboxOutcome {
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: true,
            wall_ms,
        }),
    }
}

#[cfg(windows)]
fn build_shell_command(entry_point: &str) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", entry_point]);
    cmd
}

#[cfg(not(windows))]
fn build_shell_command(entry_point: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", entry_point]);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_exits_zero_and_captures_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let out = run_in_sandbox(dir.path(), "echo ok", Duration::from_secs(5))
            .await
            .expect("spawn");
        assert_eq!(out.exit_code, Some(0));
        assert!(
            out.stdout.contains("ok"),
            "expected 'ok' in stdout, got {:?}",
            out.stdout
        );
        assert!(!out.timed_out);
    }

    #[tokio::test]
    async fn nonzero_exit_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        // `exit 7` is portable across sh/cmd.
        let out = run_in_sandbox(dir.path(), "exit 7", Duration::from_secs(5))
            .await
            .expect("spawn");
        assert_eq!(out.exit_code, Some(7));
    }

    /// Slow: waits ~30s for a long-running child process to be killed by the sandbox timeout.
    /// Excluded from default local `vox ci pre-push --full`; use `--include-slow` or CI.
    #[tokio::test]
    #[ignore = "slow; waits ~30s for process timeout (~29s elapsed); owner: scientia sunset: never; use --include-slow or CI"]
    async fn timeout_kills_long_running_child() {
        let dir = tempfile::tempdir().unwrap();
        // Sleep a long time on POSIX; ping on Windows is the portable analog.
        #[cfg(windows)]
        let cmd = "ping -n 30 127.0.0.1 > nul";
        #[cfg(not(windows))]
        let cmd = "sleep 30";
        let out = run_in_sandbox(dir.path(), cmd, Duration::from_millis(500))
            .await
            .expect("spawn");
        assert!(out.timed_out, "expected timeout, got {:?}", out);
        assert_eq!(out.exit_code, None);
    }
}
