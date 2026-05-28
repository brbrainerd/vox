//! Central executor for every `git` invocation made from the orchestrator
//! process tree. All callers (MCP tools, CLI subcommands lifted into the
//! orchestrator) MUST go through `GitExec::run` so the banned-command
//! denylist and `vox.vcs.*` telemetry apply uniformly.

use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct GitExec {
    cwd: PathBuf,
}

#[derive(Debug)]
pub struct GitOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum GitExecError {
    #[error("banned git invocation: {0}")]
    Banned(String),
    #[error("spawning git failed: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git exited non-zero ({code}): {stderr}")]
    NonZero {
        code: i32,
        stdout: String,
        stderr: String,
    },
}

/// Each entry is a contiguous arg-vector window that, if matched **exactly**
/// anywhere in `args`, denies the call. This is positional exact-match — NOT
/// flag-prefix match. `["clean", "-f"]` does not subsume `["clean", "-fd"]`;
/// each variant must be listed explicitly. The window-match (rather than
/// strict-prefix) lets us catch banned subcommands that follow leading
/// `-c key=val` configuration overrides. See `is_banned` for the matcher.
///
/// Phase 2 will add an arch-check rule covering `// vox-arch-check: allow git-exec
///         Command::new("git")`
/// outside `git_exec.rs`, which removes the need to enumerate every
/// destructive variant here. Until then, additions to this list should
/// also include the `-ffd` / permuted-flag forms when the bypass risk is
/// real.
const BANNED_PREFIXES: &[&[&str]] = &[
    &["stash"],
    &["reset", "--hard"],
    &["clean", "-fd"],
    &["clean", "-f"],
    &["clean", "-fdx"],
    &["restore", "."],
    &["checkout", "."],
    &["checkout", "--", "."],
    &["checkout", "-f"],
];

/// Returns `Some(matched_phrase)` if `args` contains any banned prefix.
pub fn is_banned(args: &[&str]) -> Option<String> {
    for &pat in BANNED_PREFIXES {
        if args.windows(pat.len()).any(|w| w == pat) {
            return Some(pat.join(" "));
        }
    }
    None
}

impl GitExec {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self { cwd: cwd.into() }
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub async fn run(&self, args: &[&str]) -> Result<GitOutput, GitExecError> {
        if let Some(phrase) = is_banned(args) {
            tracing::warn!(target: "vox.vcs.exec", phrase = %phrase, "denied banned git invocation");
            return Err(GitExecError::Banned(phrase));
        }
        let started = Instant::now();
        let output = tokio::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .current_dir(&self.cwd)
            .args(args)
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);
        let elapsed_ms = started.elapsed().as_millis() as u64;
        tracing::debug!(
            target: "vox.vcs.exec",
            args = ?args,
            cwd = %self.cwd.display(),
            code = code,
            elapsed_ms = elapsed_ms,
            "git exec",
        );
        if code != 0 {
            return Err(GitExecError::NonZero {
                code,
                stdout,
                stderr,
            });
        }
        Ok(GitOutput {
            stdout,
            stderr,
            exit_code: code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_banned_catches_each_prefix() {
        assert_eq!(is_banned(&["stash"]).as_deref(), Some("stash"));
        assert_eq!(is_banned(&["stash", "pop"]).as_deref(), Some("stash"));
        assert_eq!(
            is_banned(&["reset", "--hard", "HEAD~1"]).as_deref(),
            Some("reset --hard")
        );
        assert_eq!(is_banned(&["clean", "-fd"]).as_deref(), Some("clean -fd"));
        assert_eq!(is_banned(&["restore", "."]).as_deref(), Some("restore ."));
        assert_eq!(is_banned(&["checkout", "."]).as_deref(), Some("checkout ."));
        assert_eq!(
            is_banned(&["checkout", "--", "."]).as_deref(),
            Some("checkout -- .")
        );
    }

    #[test]
    fn is_banned_passes_through_safe_calls() {
        assert!(is_banned(&["status", "--short"]).is_none());
        assert!(is_banned(&["log", "-n", "10"]).is_none());
        assert!(is_banned(&["commit", "-m", "wip: anything"]).is_none());
        assert!(is_banned(&["checkout", "main"]).is_none());
        assert!(is_banned(&["clean", "-n"]).is_none());
    }

    #[test]
    fn is_banned_catches_prefix_after_dash_c() {
        assert_eq!(
            is_banned(&["-c", "advice.detachedHead=false", "reset", "--hard", "abc"]).as_deref(),
            Some("reset --hard"),
        );
    }

    #[tokio::test]
    async fn run_rejects_banned_without_spawning() {
        let exec = GitExec::new(std::env::temp_dir());
        let err = exec.run(&["stash"]).await.unwrap_err();
        assert!(matches!(err, GitExecError::Banned(_)));
    }
}
