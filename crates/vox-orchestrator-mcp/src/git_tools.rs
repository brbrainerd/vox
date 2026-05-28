//! Git integration tool handlers for the Vox MCP server.
//!
//! Covers: log, diff, status, blame.
//!
//! Uses [`tokio::process::Command`] so the async MCP dispatcher does not block the runtime
//! while waiting on `git` subprocess I/O.

use std::path::PathBuf;

use crate::git_exec::{GitExec, GitExecError};
use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_GIT_EXEC: &str = "Ensure `git` is on PATH and the MCP workspace points at a repository with a usable `git_root`.";

fn git_cwd(state: &ServerState) -> PathBuf {
    state
        .repository
        .git_root
        .clone()
        .unwrap_or_else(|| state.repository.root.clone())
}

/// Run `git log` to show recent commits.
pub async fn git_log(state: &ServerState, max_commits: Option<usize>) -> String {
    let n = max_commits.unwrap_or(10).to_string();
    let exec = GitExec::new(git_cwd(state));
    match exec.run(&["log", "--oneline", "-n", &n]).await {
        Ok(o) => ToolResult::ok(o.stdout).to_json(),
        Err(GitExecError::NonZero { stdout, .. }) => ToolResult::ok(stdout).to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("git log failed: {e}"), REM_GIT_EXEC)
                .to_json()
        }
    }
}

/// Run `git diff` for a file or the whole working tree.
pub async fn git_diff(state: &ServerState, path: Option<&str>) -> String {
    let mut cmd = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git");
    cmd.current_dir(git_cwd(state));
    cmd.arg("diff");
    if let Some(p) = path {
        cmd.arg(p);
    }

    match cmd.output().await {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            ToolResult::ok(text).to_json()
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("git diff failed: {e}"),
            REM_GIT_EXEC,
        )
        .to_json(),
    }
}

/// Run `git status` to see working tree status.
pub async fn git_status(state: &ServerState) -> String {
    let output = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(git_cwd(state))
        .args(["status", "--short"])
        .output()
        .await;

    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            ToolResult::ok(text).to_json()
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("git status failed: {e}"),
            REM_GIT_EXEC,
        )
        .to_json(),
    }
}

/// Run `git blame` for a specific file.
pub async fn git_blame(state: &ServerState, path: &str) -> String {
    let output = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(git_cwd(state))
        .args(["blame", path])
        .output()
        .await;

    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            ToolResult::ok(text).to_json()
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("git blame failed: {e}"),
            REM_GIT_EXEC,
        )
        .to_json(),
    }
}
