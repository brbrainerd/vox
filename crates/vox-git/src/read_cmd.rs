//! Audited read-only `git` invocations for callers that need output gix cannot
//! produce (unified diffs, numstat). This is the SINGLE sanctioned raw-git
//! location for read-only commands outside the GitExec write-gateway; it is
//! listed in layers.toml `raw-git-exec` exempt_files. WRITE commands are not
//! permitted here (use GitExec at the MCP layer for those).

use std::path::Path;
use std::process::Command;

/// Read-only git subcommands this helper will run. Anything else is rejected.
const READ_ONLY: &[&str] = &[
    "show",
    "diff",
    "log",
    "rev-parse",
    "status",
    "cat-file",
    "ls-files",
    "remote",
    "worktree",
    "branch",
];

/// Failure modes for [`read_only`].
#[derive(Debug, thiserror::Error)]
pub enum GitReadError {
    /// The requested subcommand is not on the read-only allowlist.
    #[error("non-read-only or disallowed git subcommand: {0}")]
    Disallowed(String),
    /// `git` could not be spawned (missing binary, permissions, etc).
    #[error("git spawn failed: {0}")]
    Spawn(#[from] std::io::Error),
    /// `git` ran but exited non-zero.
    #[error("git exited {code}: {stderr}")]
    NonZero {
        /// Process exit code (`-1` if terminated by signal / unknown).
        code: i32,
        /// Captured stderr (lossy UTF-8, trimmed).
        stderr: String,
    },
}

/// Run `git -C <repo> <args...>` for a read-only subcommand, returning stdout
/// (lossy UTF-8).
///
/// Rejects any subcommand not in `READ_ONLY` (defense against accidental
/// writes). The output is byte-for-byte the stdout of the underlying `git`
/// process, so callers feeding diffs to an LLM judge/router see identical
/// context to a direct invocation.
pub fn read_only(repo: &Path, args: &[&str]) -> Result<String, GitReadError> {
    let sub = args.first().copied().unwrap_or("");
    if !READ_ONLY.contains(&sub) {
        return Err(GitReadError::Disallowed(sub.to_string()));
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(GitReadError::NonZero {
            code: out.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Build a throwaway repo with one commit. Uses raw `git` write commands,
    /// but the dir is an isolated per-test tempdir (no shared-repo concurrency
    /// risk), so this is fixture setup, not production git access.
    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let run = |args: &[&str]| {
            let st = Command::new("git")
                .arg("-C")
                .arg(p)
                .args(args)
                .output()
                .unwrap();
            assert!(
                st.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&st.stderr)
            );
        };
        run(&["init", "--initial-branch=main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        std::fs::write(p.join("a.txt"), "hello\n").unwrap();
        run(&["add", "a.txt"]);
        run(&["commit", "-m", "init"]);
        dir
    }

    #[test]
    fn allowed_subcommand_returns_output() {
        let dir = init_repo();
        let out = read_only(dir.path(), &["log", "--format=%s"]).unwrap();
        assert!(out.contains("init"), "unexpected log output: {out:?}");
    }

    #[test]
    fn show_returns_diff_body() {
        let dir = init_repo();
        let out = read_only(dir.path(), &["show", "--no-color", "--format=", "HEAD"]).unwrap();
        assert!(
            out.contains("a.txt"),
            "expected diff to mention file: {out:?}"
        );
        assert!(out.contains("+hello"), "expected added line: {out:?}");
    }

    #[test]
    fn disallowed_write_subcommand_is_rejected() {
        let dir = init_repo();
        for bad in ["commit", "push", "init", "add"] {
            let err = read_only(dir.path(), &[bad]).unwrap_err();
            assert!(
                matches!(err, GitReadError::Disallowed(ref s) if s == bad),
                "expected Disallowed({bad}), got {err:?}"
            );
        }
    }
}
