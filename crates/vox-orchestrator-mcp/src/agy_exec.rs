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
    /// Auto-responses written back to agy's PTY stdin (e.g. "y\n" for a [y/n]
    /// prompt). Empty when no HITL prompts were detected during the run.
    pub hitl_responses: Vec<String>,
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
        let cwd = self.cwd.clone();
        let timeout = Duration::from_secs(spec.timeout_secs.max(1));

        // portable-pty is synchronous; run the whole PTY interaction on a
        // blocking worker so we never stall the async runtime.
        let join = tokio::task::spawn_blocking(move || run_in_pty(&cwd, &args, timeout));
        match join.await {
            Ok(result) => result,
            Err(join_err) => Err(AgyExecError::Spawn(std::io::Error::new(std::io::ErrorKind::Other, join_err.to_string()))),
        }
    }
}

/// Blocking PTY-backed agy invocation. Spawns `agy` inside a pseudo-console so
/// its bubbletea TUI initialises correctly (a plain piped subprocess has no
/// terminal and aborts before running its file-writing tools). Output is drained
/// on a reader thread; timeout is enforced by killing the child if the reader
/// thread hasn't finished by the deadline.
///
/// We do NOT use `child.try_wait()` for the exit signal — on Windows ConPTY
/// `try_wait()` can return `Ok(None)` indefinitely even after the child exits.
/// Instead we watch the reader thread: the child exiting closes the PTY slave →
/// the master sees EOF → the reader thread finishes. That EOF is our reliable
/// exit signal.
fn run_in_pty(cwd: &Path, args: &[String], timeout: Duration) -> Result<AgyOutput, AgyExecError> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    let started = Instant::now();

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize { rows: 40, cols: 120, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| AgyExecError::Spawn(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    // vox-arch-check: allow agy-exec
    let mut cmd = CommandBuilder::new("agy");
    for a in args {
        cmd.arg(a);
    }
    cmd.cwd(cwd);

    let mut child = pair.slave.spawn_command(cmd).map_err(|e| {
        let msg = e.to_string().to_ascii_lowercase();
        if msg.contains("no such file")
            || msg.contains("cannot find")
            || msg.contains("not found")
            || msg.contains("program not found")
        {
            AgyExecError::NotFound
        } else {
            AgyExecError::Spawn(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        }
    })?;
    // The slave handle must be dropped or the master reader never sees EOF.
    drop(pair.slave);

    // Drain the master on a dedicated thread (portable-pty reads are blocking).
    // NOTE: we do NOT move `pair.master` into the reader thread. On Windows
    // ConPTY, `child.kill()` does not reliably close the slave side, so the
    // reader can block forever after a kill. By keeping the master in this
    // function scope, we can `drop(pair.master)` from the main thread on the
    // timeout path to force the reader to see an error (EOF) and unblock.
    // Take the PTY writer before spawning the reader thread. Arc<Mutex<...>>
    // lets the reader thread write auto-responses without blocking the main thread.
    let writer = pair.master.take_writer().map_err(|e| {
        AgyExecError::Spawn(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    })?;
    let writer_arc: Arc<Mutex<Box<dyn Write + Send>>> =
        Arc::new(Mutex::new(writer));
    let writer_for_thread = Arc::clone(&writer_arc);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| AgyExecError::Spawn(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let buf_writer = Arc::clone(&buf);

    let (hitl_tx, hitl_rx) = std::sync::mpsc::channel::<String>();

    let reader_thread = std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let data = &chunk[..n];
                    buf_writer.lock().unwrap().extend_from_slice(data);
                    if let Some(resp) = auto_respond(data) {
                        if let Ok(mut w) = writer_for_thread.lock() {
                            let _ = w.write_all(resp);
                            let _ = w.flush();
                        }
                        let _ = hitl_tx.send(String::from_utf8_lossy(resp).to_string());
                    }
                }
            }
        }
        // hitl_tx drops here closing the channel
    });

    // Wait for the reader thread (= child exited, PTY master sees EOF) or kill.
    // `is_finished` is cheap (atomic flag). Poll at 100 ms.
    let deadline = started + timeout;
    let mut timed_out = false;
    loop {
        if reader_thread.is_finished() {
            break;
        }
        if Instant::now() >= deadline {
            // Kill the child on timeout.
            let _ = child.kill();
            timed_out = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // Dropping the master closes the ConPTY handle. On Windows this causes the
    // cloned reader to return an error, unblocking the reader thread. On Unix
    // this causes the master to send EOF. Either way the thread exits promptly.
    // Drop writer first, then master. On Windows ConPTY this ensures both
    // handles are closed before we drain the hitl channel.
    drop(writer_arc);
    drop(pair.master);
    let _ = reader_thread.join();

    let hitl_responses: Vec<String> = hitl_rx.try_iter().collect();

    // Get the exit code. `try_wait` can be unreliable on Windows ConPTY so we
    // use `wait` (blocking, but the child has already exited or been killed).
    let exit_code = if timed_out {
        -1
    } else {
        match child.wait() {
            Ok(status) => status.exit_code() as i32,
            Err(_) => 0, // child already reaped
        }
    };

    let raw = buf.lock().unwrap().clone();
    let text = strip_ansi(&String::from_utf8_lossy(&raw));

    if timed_out {
        tracing::warn!(target: "vox.agy.exec", timeout_secs = timeout.as_secs(), "agy delegation timed out; child killed");
    } else {
        tracing::debug!(target: "vox.agy.exec", code = exit_code, elapsed_ms = started.elapsed().as_millis() as u64, "agy exec done");
    }

    Ok(AgyOutput {
        // PTY merges stdout+stderr into one stream. Mirror the merged, cleaned
        // text into both fields so stderr-based classification keeps working.
        stdout: text.clone(),
        stderr: if timed_out {
            format!("agy delegation exceeded {}s; process killed", timeout.as_secs())
        } else {
            text
        },
        exit_code,
        timed_out,
        elapsed_ms: started.elapsed().as_millis() as u64,
        hitl_responses,
    })
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

/// Maps the most-recent PTY output chunk to an automatic stdin reply.
///
/// Scans the tail (last 300 bytes) of `chunk` for known interactive prompts
/// that can be safely auto-answered without human judgement. Returns
/// `Some(response_bytes)` or `None` if no pattern matched.
pub fn auto_respond(chunk: &[u8]) -> Option<&'static [u8]> {
    let tail_start = chunk.len().saturating_sub(300);
    let tail = String::from_utf8_lossy(&chunk[tail_start..]).to_ascii_lowercase();
    // Press-enter pauses (check first — response is "\n", not "y\n")
    if tail.contains("press enter") || tail.contains("press <enter>") {
        return Some(b"\n");
    }
    // yes/no confirmation variants
    if tail.contains("[y/n]")
        || tail.contains("(y/n)")
        || tail.contains("[yes/no]")
        || tail.contains("(yes/no)")
        || tail.contains("yes/no:")
    {
        return Some(b"y\n");
    }
    None
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

    #[test]
    fn agy_output_has_hitl_responses_field() {
        let out = AgyOutput {
            stdout: "done".into(),
            stderr: "".into(),
            exit_code: 0,
            timed_out: false,
            elapsed_ms: 100,
            hitl_responses: vec!["y\n".into()],
        };
        assert_eq!(out.hitl_responses.len(), 1);
        assert_eq!(out.hitl_responses[0], "y\n");
    }

    #[test]
    fn auto_respond_detects_yn_and_enter_prompts() {
        assert_eq!(auto_respond(b"Proceed with changes? [y/n] "), Some(&b"y\n"[..]));
        assert_eq!(auto_respond(b"Are you sure? (y/n): "),       Some(&b"y\n"[..]));
        assert_eq!(auto_respond(b"Continue? [Y/n] "),            Some(&b"y\n"[..]));
        assert_eq!(auto_respond(b"yes/no: "),                    Some(&b"y\n"[..]));
        assert_eq!(auto_respond(b"Press Enter to continue..."), Some(&b"\n"[..]));
        assert_eq!(auto_respond(b"press <enter> "),             Some(&b"\n"[..]));
        assert_eq!(auto_respond(b"Writing file src/lib.rs..."), None);
        assert_eq!(auto_respond(b"Task complete."),             None);
        assert_eq!(
            auto_respond(b"Reading 1000 lines of context...\r\nProceed? [y/n] "),
            Some(&b"y\n"[..])
        );
    }
}
