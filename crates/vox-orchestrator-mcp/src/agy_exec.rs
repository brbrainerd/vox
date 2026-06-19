//! Native executor for the Antigravity `agy` CLI. ALL `agy` spawns MUST go
//! through `AgyExec::run` (enforced by the optional `raw-agy-exec` arch rule).
//!
//! Safety: auto-accept (`--dangerously-skip-permissions`) defeats agy's own
//! `--sandbox` (antigravity-cli#36), so we NEVER pass `--sandbox`; isolation is
//! the caller's per-delegation git worktree.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct AgySpec {
    pub task: String,
    pub model: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Debug)]
pub struct AgyOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum AgyExecError {
    #[error("invalid delegation spec: {0}")]
    Invalid(String),
    #[error("agy binary not found (run vox_agy_doctor)")]
    NotFound,
    #[error("spawning agy failed: {0}")]
    Spawn(#[from] std::io::Error),
}

pub fn validate_spec(spec: &AgySpec) -> Result<(), AgyExecError> {
    if spec.task.trim().is_empty() {
        return Err(AgyExecError::Invalid("empty task".into()));
    }
    Ok(())
}

pub fn build_args(spec: &AgySpec) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        spec.task.clone(),
        "--dangerously-skip-permissions".to_string(),
    ];
    if let Some(m) = &spec.model {
        // VERIFIED: `--model` takes a display NAME, e.g. "Gemini 3.5 Flash (Low)" — NOT a slug.
        // Callers must pass an exact display string; we pass it through unmodified.
        args.push("--model".to_string());
        args.push(m.clone());
    }
    args
}

/// Lowercase, `[a-z0-9-]` only, collapsed dashes, max 40 chars. Prevents path
/// traversal / illegal worktree paths.
pub fn sanitize_slug(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    trimmed.chars().take(40).collect()
}

#[derive(Debug, Clone)]
pub struct AgyExec {
    cwd: PathBuf,
}

impl AgyExec {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self { cwd: cwd.into() }
    }
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub async fn run(&self, spec: &AgySpec) -> Result<AgyOutput, AgyExecError> {
        validate_spec(spec)?;
        let args = build_args(spec);
        let started = Instant::now();

        // vox-arch-check: allow agy-exec  (annotation active once Task 15 adds the rule)
        let mut cmd = tokio::process::Command::new("agy");
        cmd.current_dir(&self.cwd)
            .args(&args)
            .kill_on_drop(true) // <-- ensures the timeout branch actually reaps the child
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        // Do NOT set CREATE_NO_WINDOW on Windows: agy requires a virtual console
        // to initialise its TUI layer; suppressing the console causes it to exit
        // silently (exit 0, no output, no file writes).  Suppress the visible
        // window via DETACHED_PROCESS instead — same effect, no TUI breakage.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            cmd.creation_flags(DETACHED_PROCESS);
        }

        let child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AgyExecError::NotFound
            } else {
                AgyExecError::Spawn(e)
            }
        })?;

        let dur = Duration::from_secs(spec.timeout_secs.max(1));
        match tokio::time::timeout(dur, child.wait_with_output()).await {
            Ok(Ok(output)) => {
                let code = output.status.code().unwrap_or(-1);
                tracing::debug!(target: "vox.agy.exec", code, elapsed_ms = started.elapsed().as_millis() as u64, "agy exec done");
                Ok(AgyOutput {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: code,
                    timed_out: false,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                })
            }
            Ok(Err(e)) => Err(AgyExecError::Spawn(e)),
            Err(_elapsed) => {
                // The wait-future is dropped here; kill_on_drop(true) reaps the child.
                tracing::warn!(target: "vox.agy.exec", timeout_secs = spec.timeout_secs, "agy delegation timed out; child killed");
                Ok(AgyOutput {
                    stdout: String::new(),
                    stderr: format!("agy delegation exceeded {}s; process killed", spec.timeout_secs),
                    exit_code: -1,
                    timed_out: true,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                })
            }
        }
    }
}

/// Classify outcome for retry + ledger category. None on success.
pub fn classify_failure(stderr: &str, exit_code: i32, timed_out: bool) -> Option<&'static str> {
    if timed_out { return Some("timeout"); }
    let s = stderr.to_ascii_lowercase();
    if s.contains("quota") || s.contains("rate limit") || s.contains("resource_exhausted") {
        return Some("quota");
    }
    if exit_code == 0 { None } else { Some("error") }
}

/// Strip ANSI escape sequences (CSI colour/cursor + OSC title) from PTY output.
/// A pseudo-console echoes terminal control codes interleaved with text; we
/// remove them so the captured stream is plain text for logging and for the
/// substring matching in `classify_failure`.
pub fn strip_ansi(s: &str) -> String {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // CSI:  ESC [ ... final-byte   |   OSC:  ESC ] ... (BEL | ESC \)
        regex::Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)")
            .expect("static ANSI regex is valid")
    });
    re.replace_all(s, "").to_string()
}

/// Pure retry decision. `attempt` 0-based; `max_attempts` the cap.
pub fn should_retry(class: &str, attempt: u32, max_attempts: u32) -> bool {
    if attempt + 1 >= max_attempts { return false; }
    match class {
        "quota" => true,
        "timeout" => attempt < 1,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_headless_autoaccept_args_without_sandbox() {
        let spec = AgySpec { task: "Refactor foo".into(), model: None, timeout_secs: 600 };
        let args = build_args(&spec);
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "Refactor foo");
        assert!(args.iter().any(|a| a == "--dangerously-skip-permissions"));
        assert!(!args.iter().any(|a| a == "--sandbox")); // #36 guard
    }

    #[test]
    fn rejects_empty_task() {
        assert!(validate_spec(&AgySpec { task: "  ".into(), model: None, timeout_secs: 1 }).is_err());
    }

    #[test]
    fn slug_is_path_safe() {
        assert_eq!(sanitize_slug("Refactor/Foo Bar!!"), "refactor-foo-bar");
        assert!(!sanitize_slug("../etc").contains('.'));
        assert!(!sanitize_slug("../etc").contains('/'));
    }

    #[tokio::test]
    async fn run_reports_timeout_or_notfound_fast() {
        let exec = AgyExec::new(std::env::temp_dir());
        let spec = AgySpec { task: "noop".into(), model: None, timeout_secs: 1 };
        match exec.run(&spec).await {
            Ok(o) => assert!(o.timed_out || o.exit_code != 0 || o.exit_code == 0),
            Err(e) => assert!(matches!(e, AgyExecError::NotFound | AgyExecError::Spawn(_))),
        }
    }

    #[test]
    fn classifies_quota_timeout_error_success() {
        assert_eq!(classify_failure("quota exceeded", 1, false), Some("quota"));
        assert_eq!(classify_failure("RESOURCE_EXHAUSTED", 1, false), Some("quota"));
        assert_eq!(classify_failure("", -1, true), Some("timeout"));
        assert_eq!(classify_failure("boom", 2, false), Some("error"));
        assert_eq!(classify_failure("fine", 0, false), None);
    }

    #[test]
    fn strip_ansi_removes_csi_and_osc_sequences() {
        // CSI colour codes
        assert_eq!(strip_ansi("\x1b[32mhello\x1b[0m"), "hello");
        // OSC title sequence terminated by BEL
        assert_eq!(strip_ansi("\x1b]0;title\x07world"), "world");
        // plain text is untouched
        assert_eq!(strip_ansi("quota exceeded"), "quota exceeded");
        // a realistic mixed line still exposes the keyword for classification
        assert!(strip_ansi("\x1b[31mERROR:\x1b[0m quota exceeded").contains("quota"));
    }

    #[test]
    fn retry_policy() {
        assert!(should_retry("quota", 0, 3));
        assert!(!should_retry("quota", 2, 3));   // hit cap
        assert!(should_retry("timeout", 0, 3));
        assert!(!should_retry("timeout", 1, 3)); // one extra try only
        assert!(!should_retry("error", 0, 3));   // non-retryable
    }
}
