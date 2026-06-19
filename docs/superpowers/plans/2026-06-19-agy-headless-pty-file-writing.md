# Headless agy File-Writing via ConPTY Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `agy` (the Antigravity CLI) reliably write files when driven headlessly from the Rust harness, so Vox skills can delegate real code tasks to it through `vox_agy_pipeline`.

**Architecture:** Replace the `tokio::process` spawn in `AgyExec::run` with a `portable-pty` pseudo-console (ConPTY on Windows, openpty on Unix). `agy` is a bubbletea TUI app — under piped stdio it has no terminal and either bails (`could not open TTY`) or returns before its agent loop finishes its file-writing tool calls. A pseudo-console gives it a fully functional but invisible terminal, exactly what made manual `agy --print` runs succeed. Two hardening fixes ride along: the delegation worktree cleanup must delete its throwaway branch (it currently leaks `agy/<slug>` refs), and the delegation prompt template must stop asking agy to "commit" or "find the repo root" (those tangents burn the timeout).

**Tech Stack:** Rust, `portable-pty = "0.9"` (already a workspace dep, used by `vox-gui`'s terminal), `regex` + `uuid` (already crate deps), `git` CLI via the existing `GitExec`.

---

## Background: Verified Facts (read before starting)

These were established empirically in the session that produced this plan. Do **not** re-litigate them:

1. `agy --print "<task>" --dangerously-skip-permissions` **DOES write files** when it has a real console. Proven twice: created `test123.txt` in a plain dir and `agy-smoke.txt` in a git worktree (agy even `git add`-staged it).
2. The failure was the **Rust subprocess invocation**, not agy. `CREATE_NO_WINDOW` → agy exits silently (empty output, no writes). `DETACHED_PROCESS` → agy prints a partial "I'll create the file…" then returns with 0 writes (broken console).
3. `agy -i` (`--prompt-interactive`) needs a real TTY and dies headless with `bubbletea: could not open TTY: open CONIN$: The handle is invalid.` — so `-p` (`--print`) is the only headless-capable flag, and it is sufficient once it has a pseudo-console.
4. The **capture logic is already correct**: `git diff HEAD` catches staged files, `git ls-files --others --exclude-standard` catches unstaged. Do not change capture.
5. The **worktree cleanup leaks the branch**: `cleanup()` runs `git worktree remove --force <path>` but never `git branch -D agy/<slug>`. Re-running any fixed-slug delegation fails with `fatal: a branch named 'agy/<slug>' already exists`.
6. The `vox-gui` PTY reference implementation lives at `crates/vox-gui/src/commands/pty.rs` (portable-pty is sync/blocking; read on a dedicated thread).

## File Structure

| File | Responsibility | Change |
|------|----------------|--------|
| `crates/vox-orchestrator-mcp/Cargo.toml` | crate deps | add `portable-pty = "0.9"` |
| `crates/vox-orchestrator-mcp/src/agy_exec.rs` | spawn agy, capture output, timeout/kill | replace `tokio::process` with PTY; add `strip_ansi`; mirror merged stream into both output fields |
| `crates/vox-orchestrator-mcp/src/agy_worktree.rs` | worktree jail create/capture/cleanup | `cleanup()` deletes the branch; add pure `cleanup_steps` helper |
| `crates/vox-orchestrator-mcp/tests/agy_delegate_smoke.rs` | gated live single-delegation smoke | restore real file-write assertion, unique slug, cleanup-before-assert |
| `crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs` | gated live classifier smoke | restore real `green` outcome assertion |
| `crates/vox-orchestrator-mcp/tests/agy_pipeline_e2e.rs` | gated live full-pipeline proof (NEW) | delegate→capture→gates→classify→ledger against the live repo |
| `crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md` | delegation playbook | add prompt-hygiene rules |
| `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md` | single-delegation playbook | add prompt-hygiene rules |

---

## Task 1: Add portable-pty dependency and ANSI-stripping helper

**Files:**
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml`
- Modify: `crates/vox-orchestrator-mcp/src/agy_exec.rs`

- [ ] **Step 1: Add the dependency**

In `crates/vox-orchestrator-mcp/Cargo.toml`, find the line `which = { workspace = true }` (added in a prior task) and add `portable-pty` right after it:

```toml
which = { workspace = true }
portable-pty = "0.9"
```

- [ ] **Step 2: Verify it resolves**

Run: `cargo metadata --format-version 1 --no-deps -q > /dev/null && cargo tree -p vox-orchestrator-mcp -i portable-pty 2>&1 | head -3`
Expected: shows `portable-pty v0.9.x` (no "package not found" error).

- [ ] **Step 3: Write the failing test for `strip_ansi`**

In `crates/vox-orchestrator-mcp/src/agy_exec.rs`, inside the existing `#[cfg(test)] mod tests { ... }` block (the one starting near line 171), add:

```rust
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
```

- [ ] **Step 4: Run it to confirm it fails to compile**

Run: `cargo test -p vox-orchestrator-mcp --lib strip_ansi 2>&1 | tail -5`
Expected: FAIL — `cannot find function strip_ansi in this scope`.

- [ ] **Step 5: Implement `strip_ansi`**

At the top of `crates/vox-orchestrator-mcp/src/agy_exec.rs`, the imports currently read:

```rust
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
```

Add `OnceLock`:

```rust
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
```

Then add this function immediately **before** the `#[cfg(test)]` module (after `should_retry`):

```rust
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
```

- [ ] **Step 6: Run the test to confirm it passes**

Run: `cargo test -p vox-orchestrator-mcp --lib strip_ansi 2>&1 | tail -5`
Expected: `test result: ok. 1 passed`.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/Cargo.toml crates/vox-orchestrator-mcp/src/agy_exec.rs
git commit -m "feat(agy): add portable-pty dep + strip_ansi helper for PTY output"
```

---

## Task 2: Replace the subprocess spawn with a pseudo-console (the core fix)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_exec.rs:91-148` (the `run` method)

**Why:** `agy` needs a terminal. `portable-pty` gives it an invisible one on every platform. The whole interaction is blocking (portable-pty is sync), so we run it inside `tokio::task::spawn_blocking` and enforce the timeout by polling `child.try_wait()` against a deadline, killing on overrun. The PTY merges stdout+stderr into one stream; we mirror that merged, ANSI-stripped text into **both** `AgyOutput.stdout` and `AgyOutput.stderr` so the two existing `classify_failure(&x.stderr, …)` call sites keep working unchanged.

- [ ] **Step 1: Replace the `run` method body**

In `crates/vox-orchestrator-mcp/src/agy_exec.rs`, replace the entire `run` method (currently lines 91-148, from `pub async fn run` through its closing `}`) with:

```rust
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
            Err(join_err) => Err(AgyExecError::Spawn(std::io::Error::other(join_err.to_string()))),
        }
    }
}

/// Blocking PTY-backed agy invocation. Spawns `agy` inside a pseudo-console so
/// its bubbletea TUI initialises correctly (a plain piped subprocess has no
/// terminal and aborts before running its file-writing tools). Output is drained
/// on a reader thread; the timeout is enforced by polling `try_wait` against a
/// deadline and killing the child on overrun.
fn run_in_pty(cwd: &Path, args: &[String], timeout: Duration) -> Result<AgyOutput, AgyExecError> {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::io::Read;
    use std::sync::{Arc, Mutex};

    let started = Instant::now();

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize { rows: 40, cols: 120, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| AgyExecError::Spawn(std::io::Error::other(e.to_string())))?;

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
            AgyExecError::Spawn(std::io::Error::other(e.to_string()))
        }
    })?;
    // The slave handle must be dropped or the master reader never sees EOF.
    drop(pair.slave);

    // Drain the master on a dedicated thread (portable-pty reads are blocking).
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| AgyExecError::Spawn(std::io::Error::other(e.to_string())))?;
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let buf_writer = Arc::clone(&buf);
    let reader_thread = std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf_writer.lock().unwrap().extend_from_slice(&chunk[..n]),
            }
        }
    });

    // Poll for exit; kill if the deadline passes.
    let mut timed_out = false;
    let exit_code: i32 = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.exit_code() as i32,
            Ok(None) => {}
            Err(e) => return Err(AgyExecError::Spawn(std::io::Error::other(e.to_string()))),
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break -1;
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    // Dropping the master closes the PTY so the reader thread hits EOF.
    drop(pair.master);
    let _ = reader_thread.join();

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
    })
```

> NOTE: the replacement INTRODUCES a `}` that closes `impl AgyExec` right after the `run` method, then defines the free function `run_in_pty`. Make sure the original closing `}` of the `impl` block (the one that was after the old `run`) is not duplicated — after editing, the structure must be: `impl AgyExec { ... pub async fn run(...) {...} }` then `fn run_in_pty(...) {...}`. If you see two consecutive `}` followed by `fn run_in_pty`, that is correct (one closes `run`, one closes `impl`).

- [ ] **Step 2: Build to confirm it compiles**

Run: `cargo build -p vox-orchestrator-mcp 2>&1 | grep -E "^error" | head; echo "exit ${PIPESTATUS[0]}"`
Expected: no lines starting with `error`. If `std::io::Error::other` is unavailable (older Rust), replace each `std::io::Error::other(x)` with `std::io::Error::new(std::io::ErrorKind::Other, x)`.

- [ ] **Step 3: Run the existing exec unit tests**

Run: `cargo test -p vox-orchestrator-mcp --lib agy_exec 2>&1 | tail -8`
Expected: all pass, including `run_reports_timeout_or_notfound_fast` (it now goes through the PTY; on a machine without `agy` it returns `NotFound`, on one with `agy` it times out at 1s — both are accepted by that test).

- [ ] **Step 4: Run the full lib suite to confirm no regressions**

Run: `cargo test -p vox-orchestrator-mcp --lib 2>&1 | grep "test result" | tail -1`
Expected: `test result: ok. <N> passed; 0 failed` (N is 279 or more).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_exec.rs
git commit -m "fix(agy): spawn agy inside a portable-pty pseudo-console for headless file writes"
```

---

## Task 3: Make worktree cleanup delete the throwaway branch

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_worktree.rs:47-51` (the `cleanup` method)
- Modify: `crates/vox-orchestrator-mcp/src/agy_worktree.rs` (tests module)

- [ ] **Step 1: Write the failing test for the pure step planner**

In `crates/vox-orchestrator-mcp/src/agy_worktree.rs`, inside the `#[cfg(test)] mod tests` block, add:

```rust
    #[test]
    fn cleanup_steps_removes_worktree_then_deletes_branch() {
        let steps = cleanup_steps("/repo/.vox/agy-worktrees/d-1", "agy/d-1");
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0], vec!["worktree", "remove", "--force", "/repo/.vox/agy-worktrees/d-1"]);
        assert_eq!(steps[1], vec!["branch", "-D", "agy/d-1"]);
    }
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-orchestrator-mcp --lib cleanup_steps 2>&1 | tail -5`
Expected: FAIL — `cannot find function cleanup_steps`.

- [ ] **Step 3: Add the pure helper**

In `crates/vox-orchestrator-mcp/src/agy_worktree.rs`, add this free function near `count_changed` (the other pure helper, near the top of the file, after the imports):

```rust
/// The git invocations `cleanup` runs, in order: remove the worktree dir, then
/// force-delete its throwaway branch. Pure so the sequence is unit-testable.
/// `-D` (force) is used because the delegation branch intentionally holds
/// un-reviewed commits agy may have created and is always safe to discard here.
pub fn cleanup_steps(worktree_path: &str, branch: &str) -> Vec<Vec<String>> {
    vec![
        vec!["worktree".into(), "remove".into(), "--force".into(), worktree_path.into()],
        vec!["branch".into(), "-D".into(), branch.into()],
    ]
}
```

- [ ] **Step 4: Run the test to confirm it passes**

Run: `cargo test -p vox-orchestrator-mcp --lib cleanup_steps 2>&1 | tail -5`
Expected: `test result: ok. 1 passed`.

- [ ] **Step 5: Rewrite `cleanup` to use the steps**

Replace the existing `cleanup` method (currently lines 47-51):

```rust
    pub async fn cleanup(&self, repo_root: &Path) -> Result<(), GitExecError> {
        let path_s = self.path.to_string_lossy().to_string();
        GitExec::new(repo_root).run(&["worktree", "remove", "--force", &path_s]).await?;
        Ok(())
    }
```

with:

```rust
    pub async fn cleanup(&self, repo_root: &Path) -> Result<(), GitExecError> {
        let path_s = self.path.to_string_lossy().to_string();
        let git = GitExec::new(repo_root);
        let steps = cleanup_steps(&path_s, &self.branch);
        // Step 0 (worktree remove) must succeed — propagate its error.
        let s0: Vec<&str> = steps[0].iter().map(|s| s.as_str()).collect();
        git.run(&s0).await?;
        // Step 1 (branch -D) is best-effort: the branch is gone the moment the
        // worktree is removed in some git versions, so a failure here is benign.
        let s1: Vec<&str> = steps[1].iter().map(|s| s.as_str()).collect();
        let _ = git.run(&s1).await;
        Ok(())
    }
```

- [ ] **Step 6: Build and run the worktree tests**

Run: `cargo test -p vox-orchestrator-mcp --lib agy_worktree 2>&1 | tail -6`
Expected: all pass (`cleanup_steps_removes_worktree_then_deletes_branch`, `worktree_path_is_jailed_under_dot_vox`, `counts_changed_files_from_diff_parts`).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_worktree.rs
git commit -m "fix(agy): delete the agy/<slug> branch on worktree cleanup (was leaking refs)"
```

---

## Task 4: Restore the real file-write assertion in the delegate smoke test

**Files:**
- Modify: `crates/vox-orchestrator-mcp/tests/agy_delegate_smoke.rs`

**Why:** Now that agy actually writes files via the PTY, the smoke must prove it — with a unique slug (no collisions) and cleanup-before-assert (so a failed assertion never leaks a worktree).

- [ ] **Step 1: Replace the whole test file**

Replace the entire contents of `crates/vox-orchestrator-mcp/tests/agy_delegate_smoke.rs` with:

```rust
//! Live integration smoke test for headless agy file-writing.
//!
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp smoke_delegate_writes_a_file -- --ignored --nocapture
//!
//! Prerequisites:
//!   - `agy` v1.0.9+ on PATH, interactive Google login complete
//!   - Run from the repo root (a git work tree with committed HEAD)

use vox_orchestrator_mcp::agy_doctor::{AgyStatus, detect};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn smoke_delegate_writes_a_file() {
    let status = detect();
    assert!(
        matches!(status, AgyStatus::Ready { .. }),
        "agy must be ready before running smoke test: {status:?}"
    );

    let repo_root = std::env::current_dir().expect("cwd must be set");
    // Unique slug → no collision with a prior leaked worktree/branch.
    let slug = format!("smoke-{}", uuid::Uuid::new_v4().simple());
    let wt = DelegationWorktree::create(&repo_root, &slug)
        .await
        .expect("worktree creation failed");

    let exec = AgyExec::new(&wt.path);
    // Tight, tangent-free prompt: write one file, no git, no repo hunting.
    let spec = AgySpec {
        task: "Create a file named delegate-proof.txt in the current directory \
               containing exactly the single line: PROOF-OK\n\
               Use your file-writing tools. Do not run any git commands."
            .to_string(),
        model: None,
        timeout_secs: 180,
    };
    let out = exec.run(&spec).await.expect("agy spawn failed");

    // Capture results BEFORE asserting, then clean up, THEN assert — so a failed
    // assertion can never leak the worktree/branch.
    let (diff, files_changed) = wt.capture().await.expect("capture failed");
    let proof_path = wt.path.join("delegate-proof.txt");
    let proof_contents = std::fs::read_to_string(&proof_path).unwrap_or_default();
    wt.cleanup(&repo_root).await.expect("cleanup failed");

    eprintln!("exit_code={} timed_out={} elapsed_ms={}", out.exit_code, out.timed_out, out.elapsed_ms);
    eprintln!("files_changed={files_changed}\ndiff_head={}…", &diff[..diff.len().min(300)]);
    eprintln!("proof_contents={proof_contents:?}");

    assert!(!out.timed_out, "smoke task timed out");
    assert_eq!(out.exit_code, 0, "agy exited non-zero");
    assert!(files_changed > 0, "expected ≥1 changed file after delegation");
    assert!(
        proof_contents.contains("PROOF-OK"),
        "delegate-proof.txt missing/empty: {proof_contents:?}"
    );
}
```

- [ ] **Step 2: Confirm it compiles (without running the live call)**

Run: `cargo test -p vox-orchestrator-mcp --test agy_delegate_smoke -- --list 2>&1 | tail -5`
Expected: lists `smoke_delegate_writes_a_file: test` and reports it as ignored; no compile errors.

- [ ] **Step 3: Run the live smoke (requires authenticated agy)**

Run: `cargo test -p vox-orchestrator-mcp smoke_delegate_writes_a_file -- --ignored --nocapture 2>&1 | grep -E "exit_code|files_changed|proof_contents|test result"`
Expected: `exit_code=0`, `files_changed>=1`, `proof_contents="PROOF-OK\n"` (or similar), `test result: ok. 1 passed`.

> If `files_changed=0`: re-read the captured `proof_contents`. If the file exists but `files_changed=0`, the capture is mis-counting (it is not — verified). If the file does not exist, the prompt was too vague — keep the task wording exactly as written above; it is tuned to avoid agy's "find repo root / commit" tangents.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator-mcp/tests/agy_delegate_smoke.rs
git commit -m "test(agy): delegate smoke proves real file write via PTY"
```

---

## Task 5: Restore the `green` outcome assertion in the pipeline smoke test

**Files:**
- Modify: `crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs`

**Why:** With real file writes, the classifier should now return `green` (files changed + all gates pass), proving the exec→capture→gates→classify chain end-to-end.

- [ ] **Step 1: Replace the whole test file**

Replace the entire contents of `crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs` with:

```rust
//! Live end-to-end smoke for the classifier path against a REAL agy run.
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp smoke_pipeline_classifies_green -- --ignored --nocapture
//! Prereqs: agy authenticated; run from the repo root (committed HEAD).

use vox_orchestrator_mcp::agy_doctor::{AgyStatus, detect};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_gates::{Gate, run_gates};
use vox_orchestrator_mcp::agy_pipeline::classify_outcome;
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn smoke_pipeline_classifies_green() {
    assert!(matches!(detect(), AgyStatus::Ready { .. }), "agy must be ready");

    let repo_root = std::env::current_dir().expect("cwd");
    let slug = format!("pipe-{}", uuid::Uuid::new_v4().simple());
    let wt = DelegationWorktree::create(&repo_root, &slug).await.expect("worktree");

    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec {
        task: "Create a file named pipeline-proof.txt in the current directory \
               containing exactly the single line: pipeline-ok\n\
               Use your file-writing tools. Do not run any git commands."
            .into(),
        model: None,
        timeout_secs: 180,
    };
    let out = exec.run(&spec).await.expect("agy spawn");

    let (_diff, files_changed) = wt.capture().await.expect("capture");
    // A trivially-passing gate so a green run is fully exercised.
    let gates = vec![Gate {
        name: "probe".into(),
        program: "git".into(),
        args: vec!["--version".into()],
        ..Default::default()
    }];
    let results = run_gates(&wt.path, &gates, 60).await;
    let outcome = classify_outcome(files_changed, &results, out.timed_out);

    // Clean up before asserting.
    wt.cleanup(&repo_root).await.expect("cleanup");

    eprintln!("exit={} files_changed={files_changed} gate_passed={} outcome={outcome}",
        out.exit_code, results[0].passed);

    assert_eq!(out.exit_code, 0, "agy should exit 0");
    assert!(files_changed > 0, "agy must have written a file");
    assert!(results[0].passed, "git --version probe gate must pass");
    assert_eq!(outcome, "green", "files changed + gate passed ⇒ green");
}
```

- [ ] **Step 2: Confirm it compiles**

Run: `cargo test -p vox-orchestrator-mcp --test agy_pipeline_smoke -- --list 2>&1 | tail -5`
Expected: lists `smoke_pipeline_classifies_green: test`; no compile errors.

- [ ] **Step 3: Run the live smoke**

Run: `cargo test -p vox-orchestrator-mcp smoke_pipeline_classifies_green -- --ignored --nocapture 2>&1 | grep -E "outcome|files_changed|test result"`
Expected: `outcome=green`, `files_changed>=1`, `test result: ok. 1 passed`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs
git commit -m "test(agy): pipeline smoke proves green classification on real file write"
```

---

## Task 6: Prompt-hygiene rules in the delegation skills

**Files:**
- Modify: `crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md`
- Modify: `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md`

**Why:** agy runs with `cwd` = the worktree and we capture diffs via git ourselves. Asking it to "commit" or "find the repo root" sends it on multi-minute tangents that burn the timeout and produce 0 writes. The skills that author delegation prompts must forbid this.

- [ ] **Step 1: Add the rules block to `antigravity-pipeline.skill.md`**

Open `crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md`. Find the section that describes how to phrase the delegated task (search for `task` or `prompt`). Immediately after that section's heading, insert this block verbatim:

```markdown
### Prompt hygiene (REQUIRED)

agy runs headlessly inside a pre-made git worktree at its current directory, and
Vox captures the resulting diff itself. Write the task as a **direct file-edit
instruction** and obey these rules:

- **DO** name exact files and exact contents/changes ("Edit `src/foo.rs`: add …").
- **DO** tell it to use its file-writing tools.
- **DO NOT** ask it to run `git` (no "commit", no "stage", no "push").
- **DO NOT** ask it to "find the repository root" or "navigate" — it is already
  in the right directory.
- **DO NOT** bundle multiple unrelated changes; one focused task delegates best.

Bad:  "Find the repo, create a file, and commit it."
Good: "Create `notes.txt` in the current directory containing the line: hello.
       Use your file-writing tools. Do not run any git commands."
```

- [ ] **Step 2: Add the same block to `delegate-gemini.skill.md`**

Open `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md` and insert the **identical** `### Prompt hygiene (REQUIRED)` block (copy it verbatim from Step 1) after its task-phrasing section.

- [ ] **Step 3: Verify the skill files still parse**

Run: `cargo test -p vox-orchestrator-mcp --lib skills 2>&1 | grep "test result" | tail -2`
Expected: existing skill tests still pass (`0 failed`). If there is no skills test in this crate, instead run `cargo build -p vox-skills 2>&1 | grep -E "^error" ; echo "build exit ${PIPESTATUS[0]}"` and expect no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md crates/vox-skills/skills/superpowers/delegate-gemini.skill.md
git commit -m "docs(agy): prompt-hygiene rules — no git/no repo-hunting in delegated tasks"
```

---

## Task 7: End-to-end full-pipeline proof (gated)

**Files:**
- Create: `crates/vox-orchestrator-mcp/tests/agy_pipeline_e2e.rs`

**Why:** Tasks 4–5 prove the building blocks. This proves the **whole pipeline a skill drives**: delegate a non-trivial code task → capture → run a real gate → classify → append a provisional ledger entry → clean up. It exercises the exact public functions `vox_agy_pipeline` calls (the only things it adds are `state.repository.root` and slug derivation, both trivial one-liners), so a green run here means a skill delegating through `vox_agy_pipeline` will work.

- [ ] **Step 1: Create the test file**

Create `crates/vox-orchestrator-mcp/tests/agy_pipeline_e2e.rs` with:

```rust
//! Gated end-to-end proof that a skill-style delegation flows through the entire
//! pipeline and yields a verified, ledgered result. Run with:
//!   cargo test -p vox-orchestrator-mcp e2e_pipeline_delegates_real_code_task -- --ignored --nocapture
//! Prereqs: agy authenticated; run from the repo root (committed HEAD).

use vox_orchestrator_mcp::agy_doctor::{AgyStatus, detect};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_gates::{Gate, run_gates};
use vox_orchestrator_mcp::agy_ledger::{LedgerEntry, append_entry_locked, ledger_digest};
use vox_orchestrator_mcp::agy_pipeline::classify_outcome;
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn e2e_pipeline_delegates_real_code_task() {
    assert!(matches!(detect(), AgyStatus::Ready { .. }), "agy must be ready");
    let repo_root = std::env::current_dir().expect("cwd");

    // Baseline ledger size so we can prove an entry was appended.
    let before = ledger_digest(&repo_root).map(|d| d.total_entries).unwrap_or(0);

    // 1) Delegate a non-trivial code task into a jailed worktree.
    let slug = format!("e2e-{}", uuid::Uuid::new_v4().simple());
    let wt = DelegationWorktree::create(&repo_root, &slug).await.expect("worktree");
    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec {
        task: "Create a new file `e2e_demo.py` in the current directory with a \
               function `add(a, b)` that returns a + b, and a `__main__` block \
               that prints add(2, 3). Use your file-writing tools. Do not run git."
            .into(),
        model: None,
        timeout_secs: 240,
    };
    let out = exec.run(&spec).await.expect("agy spawn");

    // 2) Capture the diff.
    let (_diff, files_changed) = wt.capture().await.expect("capture");

    // 3) Run a real verification gate: the generated python must be syntactically
    //    valid. `python -m py_compile` exits 0 on success.
    let gates = vec![Gate {
        name: "py_compile".into(),
        program: "python".into(),
        args: vec!["-m".into(), "py_compile".into(), "e2e_demo.py".into()],
        ..Default::default()
    }];
    let results = run_gates(&wt.path, &gates, 60).await;

    // 4) Classify.
    let outcome = classify_outcome(files_changed, &results, out.timed_out);

    // 5) Append a provisional ledger entry (what the tool does on every run).
    let entry = LedgerEntry::new(
        "agy-e2e-proof", &spec.task, outcome, out.timed_out, out.exit_code, files_changed, spec.timeout_secs, "2026-06-19",
    )
    .with_verification(&format!("py_compile: {}", if results[0].passed { "pass" } else { "fail" }));
    let ledger_id = append_entry_locked(&repo_root, entry).await.expect("ledger append");

    // 6) Clean up the worktree+branch.
    wt.cleanup(&repo_root).await.expect("cleanup");

    let after = ledger_digest(&repo_root).map(|d| d.total_entries).unwrap_or(0);

    eprintln!(
        "exit={} files_changed={files_changed} gate_passed={} outcome={outcome} ledger_id={ledger_id} entries {before}->{after}",
        out.exit_code, results[0].passed
    );

    assert_eq!(out.exit_code, 0, "agy should exit 0");
    assert!(files_changed > 0, "agy must have created e2e_demo.py");
    assert!(results[0].passed, "generated python must compile (py_compile gate)");
    assert_eq!(outcome, "green", "files changed + gate passed ⇒ green");
    assert!(ledger_id.starts_with("AGH-"), "a ledger id must be allocated");
    assert_eq!(after, before + 1, "exactly one ledger entry must be appended");
}
```

> NOTE: this test appends one real entry to `docs/superpowers/antigravity-handoff-ledger.md`. That is the intended behaviour (the pipeline always writes a provisional entry). After a successful run, review that entry and keep or revert it as you would any delegation record. If `python` is not on PATH, change the gate `program` to `python3`.

- [ ] **Step 2: Confirm it compiles**

Run: `cargo test -p vox-orchestrator-mcp --test agy_pipeline_e2e -- --list 2>&1 | tail -5`
Expected: lists `e2e_pipeline_delegates_real_code_task: test`; no compile errors.

- [ ] **Step 3: Run the live end-to-end proof**

Run: `cargo test -p vox-orchestrator-mcp e2e_pipeline_delegates_real_code_task -- --ignored --nocapture 2>&1 | grep -E "outcome|ledger_id|entries|test result"`
Expected: `outcome=green`, `ledger_id=AGH-####`, `entries N->N+1`, `test result: ok. 1 passed`.

- [ ] **Step 4: Review the appended ledger entry**

Run: `git -C . diff -- docs/superpowers/antigravity-handoff-ledger.md | tail -30`
Expected: one new `# --- AGH-#### ---` block with `outcome: green` and `verification: py_compile: pass`. Stage it with the commit (it is a real, valid delegation record).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/tests/agy_pipeline_e2e.rs docs/superpowers/antigravity-handoff-ledger.md
git commit -m "test(agy): gated end-to-end proof — real code delegation flows through full pipeline to green+ledger"
```

---

## Task 8: Final verification sweep

**Files:** none (verification only)

- [ ] **Step 1: Full lib test suite**

Run: `cargo test -p vox-orchestrator-mcp --lib 2>&1 | grep "test result" | tail -1`
Expected: `test result: ok. <N> passed; 0 failed` (N ≥ 281: original 278 + strip_ansi + cleanup_steps + any others).

- [ ] **Step 2: Confirm the three gated live tests are present and green**

Run (requires authenticated agy):
```bash
cargo test -p vox-orchestrator-mcp -- --ignored --nocapture 2>&1 | grep -E "smoke_delegate_writes_a_file|smoke_pipeline_classifies_green|e2e_pipeline_delegates_real_code_task|test result"
```
Expected: all three named tests appear and the final aggregate shows `0 failed`.

- [ ] **Step 3: Confirm no leaked worktrees or branches**

Run: `git worktree list | grep -E "agy/|agy-worktrees" ; git branch --list "agy/*"`
Expected: NO output (every gated test cleans up after itself). If anything remains, run `git worktree prune` and `git branch -D <leftover>`, then investigate which test failed to clean up.

- [ ] **Step 4: Confirm the arch-check still passes (raw-agy-exec rule)**

Run: `cargo build -p vox-orchestrator-mcp 2>&1 | grep -E "^error|^warning: unused" | head`
Expected: no errors. The new `agy` spawn in `run_in_pty` carries the `// vox-arch-check: allow agy-exec` annotation, so the `raw-agy-exec` rule (if active) does not fire.

- [ ] **Step 5: Final commit (if any verification fixups were needed)**

```bash
git add -A
git commit -m "chore(agy): verification sweep — headless PTY delegation green end-to-end"
```

---

## Self-Review Notes

- **Spec coverage:** ConPTY console (Tasks 1–2) ✓; smoke + real assertions (Tasks 4–5) ✓; prompt hygiene (Task 6) ✓; delegation hardening — branch cleanup (Task 3) + unique slugs + cleanup-before-assert (Tasks 4,5,7) ✓; skill-wiring proof (Task 7) ✓.
- **Cross-platform:** `portable-pty` gives ConPTY on Windows and openpty on Unix; the `#[cfg(windows)]` creation-flags block is removed entirely.
- **Type consistency:** `AgyOutput` fields unchanged (`stdout/stderr/exit_code/timed_out/elapsed_ms`); `classify_failure(&x.stderr, …)` call sites keep working because the merged stream is mirrored into both fields. `Gate{name,program,args,env}`, `classify_outcome(files_changed,&results,timed_out)`, `LedgerEntry::new(...)`/`.with_verification(...)`, `append_entry_locked(repo_root, entry) -> String`, `ledger_digest(repo_root) -> io::Result<LedgerDigest>` all match the current signatures in `agy_gates.rs`, `agy_pipeline.rs`, and `agy_ledger.rs`.
- **Known residual:** if a future agy version adds a true non-interactive agent flag, `build_args` can switch off `-p`; nothing else changes.
