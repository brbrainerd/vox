//! Git integration tool handlers for the Vox MCP server.
//!
//! Covers: log, diff, status, blame.

use crate::params::ToolResult;

/// Run `git log` to show recent commits.
pub fn git_log(max_commits: Option<usize>) -> String {
    let n = max_commits.unwrap_or(10).to_string();
    let output = std::process::Command::new("git")
        .args(["log", "--oneline", "-n", &n])
        .output();

    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            ToolResult::ok(text).to_json()
        }
        Err(e) => ToolResult::<String>::err(format!("git log failed: {e}")).to_json(),
    }
}

/// Run `git diff` for a file or the whole working tree.
pub fn git_diff(path: Option<&str>) -> String {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("diff");
    if let Some(p) = path {
        cmd.arg(p);
    }

    match cmd.output() {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            ToolResult::ok(text).to_json()
        }
        Err(e) => ToolResult::<String>::err(format!("git diff failed: {e}")).to_json(),
    }
}

/// Run `git status` to see working tree status.
pub fn git_status() -> String {
    let output = std::process::Command::new("git")
        .args(["status", "--short"])
        .output();

    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            ToolResult::ok(text).to_json()
        }
        Err(e) => ToolResult::<String>::err(format!("git status failed: {e}")).to_json(),
    }
}

/// Run `git blame` for a specific file.
pub fn git_blame(path: &str) -> String {
    let output = std::process::Command::new("git")
        .args(["blame", path])
        .output();

    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            ToolResult::ok(text).to_json()
        }
        Err(e) => ToolResult::<String>::err(format!("git blame failed: {e}")).to_json(),
    }
}
