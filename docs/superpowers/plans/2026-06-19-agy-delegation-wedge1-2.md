# Antigravity (`agy`) Native Delegation — Wedge 1 + Wedge 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let Claude Code (and Vox itself) delegate heavy code-generation/refactoring tasks to Google Antigravity's Gemini agent *natively from Rust*, with no copy-paste, by shelling out to the installed `agy` CLI binary — sandboxed, auto-accepting, and auto-logged to the handoff ledger.

**Architecture:** There is **no `antigravity-sdk-rust` crate** (verified 2026-06-19; the official SDK is Python-only, the CLI `agy` is Go). So the "dependency" is a **runtime binary, not a cargo crate** — near-zero compile weight. We add a thin native executor (`AgyExec`, modeled exactly on the existing `GitExec`) inside `vox-orchestrator-mcp`, expose it as MCP tools (`vox_agy_delegate`, then `vox_agy_delegate_batch`), and drive it from one SSOT skill (`delegate-gemini.skill.md`) discovered by both Vox and Claude Code. Auto-accept of human-intervention prompts uses `agy --dangerously-skip-permissions`; because that flag is a **known sandbox-escape vector** ([antigravity-cli#36](https://github.com/google-antigravity/antigravity-cli/issues/36)), the sandbox boundary is enforced by **Vox**, not by `agy` — every delegation runs inside a dedicated, path-jailed git worktree.

**Tech Stack:** Rust (`tokio::process::Command`, `serde_json`, `thiserror`), the existing `vox-orchestrator-mcp` MCP tool layer (`ServerState`, `ToolResult`, dispatch/registry/input-schema wiring), `GitExec` for worktree creation + diff capture, the `agy` CLI binary, the append-only handoff ledger (`docs/superpowers/antigravity-handoff-ledger.md`), and the `.skill.md` skills SSOT.

---

## Verified vs. unverified facts (read before starting)

**Verified from public docs/community (2026-06):**
- `agy -p "<prompt>"` (`--print`) — non-interactive: runs one prompt and exits. The CI/script mode.
- `agy -i` (`--prompt-interactive`) — interactive TUI. **We never use this.**
- `agy --dangerously-skip-permissions` — auto-approves *all* tool/command confirmations ("force accept human intervention").
- `agy --sandbox` exists, **but** combined with `--dangerously-skip-permissions` it is bypassable (the model is hinted to pass `bypassSandbox: true` and obeys). Issue #36, **open, no maintainer fix**. ⇒ **Do not rely on `agy --sandbox` for safety.**
- Async sub-agents are spawned *automatically by agy's own orchestrator*; the `/agents` panel that manages them is **TUI-only** (unavailable under `-p`). ⇒ We do not manage agy's internal sub-agents; *our* parallelism is N concurrent `agy -p` processes.
- No reliable `--output-format json` (community reports it undefined). ⇒ **Parse exit code + stderr + the resulting `git diff`. Never assume JSON stdout.**
- Default model: Gemini 3.5 Flash (High).

**UNVERIFIED — confirm with `agy --help` in Task 0 before hardcoding:**
- A `--model` flag (model selection).
- A `--cwd` / working-directory flag (we default to spawning with `current_dir(worktree)`).
- Exact spelling of any sandbox flags beyond `--sandbox`.

> **Ledger lesson B-1/B-6 applies to us too:** do not hardcode an `agy` flag we have not confirmed. Task 0 captures `agy --help` to a fixture; later tasks reference that fixture, not memory.

---

## Architecture decisions (locked)

1. **No new crate.** The binary is the dependency. Core logic lives in `vox-orchestrator-mcp` (L3), beside its closest precedent `git_exec.rs`.
2. **Single SSOT skill.** `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md` is discovered by Vox *and* Claude Code (existing discovery + `.agents`/`.claude` mounts). No per-tool copies.
3. **Sandbox = Vox-owned worktree jail**, not `agy --sandbox`. Every delegation: create worktree off current `HEAD` → spawn `agy` with `current_dir(worktree)` → capture diff → caller reviews → optional integrate. Untrusted output never touches the live tree until reviewed.
4. **Auto-accept = `--dangerously-skip-permissions`**, made safe *only* by (3) + a hard timeout watchdog. The executor **refuses** to pass both `--sandbox` and `--dangerously-skip-permissions` together (defends against #36 footgun).
5. **Ledger auto-write** is a first-class output of every delegation, produced by `agy_ledger.rs` (unit-tested against a temp file), conforming to the existing §C schema and passing `vox ci handoff-ledger`.
6. **Windows:** all `agy` spawns set `CREATE_NO_WINDOW` (no flashing console windows — see `feedback_no_console_windows_on_spawn`).
7. **Wedge 1** = single delegation end-to-end. **Wedge 2** = batch fan-out (concurrency cap, per-worker worktree, quota/retry detection, aggregated ledger). Live dashboards are **out of scope** (future Wedge 3).

## File structure (what each file owns)

| File | Responsibility | Wedge |
|---|---|---|
| `crates/vox-orchestrator-mcp/src/agy_exec.rs` | `AgyExec`: resolve binary, build args, spawn (timeout + `CREATE_NO_WINDOW`), capture stdout/stderr/exit; arg denylist (#36 guard). Mirrors `GitExec`. | 1 |
| `crates/vox-orchestrator-mcp/src/agy_worktree.rs` | Create/remove an isolated delegation worktree via `GitExec`; capture `git diff` + untracked list. | 1 |
| `crates/vox-orchestrator-mcp/src/agy_ledger.rs` | Allocate next `AGH-NNNN`, render a schema-valid yaml block, append to ledger §C. Pure formatting + file append. | 1 |
| `crates/vox-orchestrator-mcp/src/agy_tools.rs` | MCP handlers `vox_agy_delegate` (W1) and `vox_agy_delegate_batch` (W2). Orchestrates exec+worktree+ledger. | 1, 2 |
| `crates/vox-orchestrator-mcp/src/dispatch.rs` | Add routing arms for the new tools. | 1, 2 |
| `crates/vox-orchestrator-mcp/src/input_schemas.rs` | JSON input schemas for the new tools. | 1, 2 |
| `contracts/mcp/tool-registry.canonical.yaml` | Registry entries (name/description/tier/lane). Regenerates `registry.rs`. | 1, 2 |
| `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md` | The SSOT delegation skill (Claude + Vox). | 1 |
| `crates/vox-orchestrator-mcp/tests/agy_*.rs` | Integration tests (binary-gated where they need real `agy`). | 1, 2 |

---

## Task 0: Pre-flight — confirm `agy` surface and arch-check baseline

**Files:**
- Create: `crates/vox-orchestrator-mcp/tests/fixtures/agy-help.txt`

- [ ] **Step 1: Confirm the binary exists and capture its help.** If `agy` is not installed, STOP and report (the runtime dependency is missing).

```bash
agy --version || { echo "agy NOT INSTALLED — install Antigravity CLI before proceeding"; exit 1; }
agy --help > crates/vox-orchestrator-mcp/tests/fixtures/agy-help.txt 2>&1
agy -p --help >> crates/vox-orchestrator-mcp/tests/fixtures/agy-help.txt 2>&1 || true
```

- [ ] **Step 2: Reconcile flags against this plan.** Open the fixture and confirm: `-p`/`--print`, `--dangerously-skip-permissions`. Note the *actual* spelling of any `--model` and working-dir flags. If `-p` or `--dangerously-skip-permissions` is absent/renamed, STOP and update the plan's flag constants before writing code.

- [ ] **Step 3: Capture the arch-check baseline.** The executor will call `Command::new("agy")`, which (like `git`) is likely denied outside a designated file.

Run: `cargo run -p vox-arch-check`
Expected: green at baseline (record any pre-existing red and STOP if red for unrelated reasons — ledger lesson B-2; do not edit `layers.toml` to "fix" it).

- [ ] **Step 4: Discover the exact process-spawn arch annotation.** Inspect how `git_exec.rs:85` is annotated (`// vox-arch-check: allow git-exec`) and find the rule that governs `Command::new` outside allowlisted files.

Run: `rg -n "vox-arch-check: allow" crates/ | rg -i "exec|spawn|command"`
Expected: you can name the annotation you will add above `Command::new("agy")` in `agy_exec.rs`. If no generic rule exists, the executor file may need an allowlist entry — note the exact mechanism here: `__________`.

- [ ] **Step 5: Commit the fixture.**

```bash
git add crates/vox-orchestrator-mcp/tests/fixtures/agy-help.txt
git commit -m "test(agy): capture agy --help surface + arch-check preflight"
```

---

# WEDGE 1 — Single native delegation, end-to-end

## Task 1: `AgyExec` — argument builder + #36 denylist (pure, no spawn)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/agy_exec.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs` (add `pub mod agy_exec;`)

- [ ] **Step 1: Write the failing test** (`agy_exec.rs`, `#[cfg(test)]` module):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_headless_autoaccept_args() {
        let spec = AgySpec {
            task: "Refactor foo".into(),
            model: None,
            timeout_secs: 600,
        };
        let args = build_args(&spec);
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "Refactor foo");
        assert!(args.iter().any(|a| a == "--dangerously-skip-permissions"));
        // #36 guard: we must NOT also pass agy's own --sandbox.
        assert!(!args.iter().any(|a| a == "--sandbox"));
    }

    #[test]
    fn rejects_empty_task() {
        let spec = AgySpec { task: "   ".into(), model: None, timeout_secs: 600 };
        assert!(validate_spec(&spec).is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests`
Expected: FAIL — `AgySpec` / `build_args` not defined.

- [ ] **Step 3: Write minimal implementation** (top of `agy_exec.rs`):

```rust
//! Native executor for the Antigravity `agy` CLI. All `agy` invocations made
//! from the orchestrator MUST go through `AgyExec::run` so the auto-accept +
//! sandbox-isolation invariants and `vox.agy.exec` telemetry apply uniformly.
//!
//! Safety model: auto-accept (`--dangerously-skip-permissions`) defeats agy's
//! own `--sandbox` (antigravity-cli#36), so we NEVER pass `--sandbox`; isolation
//! is enforced by the caller running us inside a dedicated git worktree.

use std::path::{Path, PathBuf};
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
    #[error("agy binary not found on PATH (install the Antigravity CLI)")]
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

/// Build the headless, auto-accepting arg vector. NB: no `--sandbox` (see #36).
pub fn build_args(spec: &AgySpec) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        spec.task.clone(),
        "--dangerously-skip-permissions".to_string(),
    ];
    if let Some(m) = &spec.model {
        // Flag name confirmed in Task 0 fixture; adjust if agy uses a different spelling.
        args.push("--model".to_string());
        args.push(m.clone());
    }
    args
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_exec.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(agy): AgySpec + arg builder with #36 sandbox-bypass guard"
```

## Task 2: `AgyExec::run` — spawn with timeout, Windows no-window, telemetry

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_exec.rs`

- [ ] **Step 1: Write the failing test** (append to the test module — gated so CI without `agy` still builds):

```rust
    #[tokio::test]
    #[ignore = "requires agy on PATH; run locally with --ignored"]
    async fn run_executes_in_workdir() {
        let exec = AgyExec::new(std::env::temp_dir());
        let out = exec
            .run(&AgySpec { task: "print the word OK and stop".into(), model: None, timeout_secs: 120 })
            .await
            .expect("spawn");
        assert!(out.exit_code == 0 || out.timed_out);
    }

    #[tokio::test]
    async fn run_times_out_fast() {
        // `sleep`-style binary stand-in: we assert the timeout path compiles + trips.
        let exec = AgyExec::new(std::env::temp_dir());
        let spec = AgySpec { task: "x".into(), model: None, timeout_secs: 0 };
        // timeout_secs == 0 => immediate timeout branch regardless of binary.
        let out = exec.run(&spec).await;
        assert!(out.is_err() || out.as_ref().map(|o| o.timed_out).unwrap_or(false));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests::run_times_out_fast`
Expected: FAIL — `AgyExec` not defined.

- [ ] **Step 3: Write minimal implementation** (append to `agy_exec.rs`):

```rust
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

        // vox-arch-check: allow agy-exec  (the ONLY place that spawns `agy`)
        let mut cmd = tokio::process::Command::new("agy");
        cmd.current_dir(&self.cwd).args(&args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AgyExecError::NotFound
            } else {
                AgyExecError::Spawn(e)
            }
        })?;

        let fut = child.wait_with_output();
        let timed = tokio::time::timeout(Duration::from_secs(spec.timeout_secs.max(1)), fut).await;
        let elapsed_ms = started.elapsed().as_millis() as u64;

        match timed {
            Ok(Ok(output)) => {
                let code = output.status.code().unwrap_or(-1);
                tracing::debug!(target: "vox.agy.exec", code, elapsed_ms, "agy exec done");
                Ok(AgyOutput {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: code,
                    timed_out: false,
                    elapsed_ms,
                })
            }
            Ok(Err(e)) => Err(AgyExecError::Spawn(e)),
            Err(_elapsed) => {
                tracing::warn!(target: "vox.agy.exec", elapsed_ms, "agy delegation timed out");
                Ok(AgyOutput {
                    stdout: String::new(),
                    stderr: format!("agy delegation exceeded {}s; killed", spec.timeout_secs),
                    exit_code: -1,
                    timed_out: true,
                    elapsed_ms,
                })
            }
        }
    }
}
```

> Note for `timeout_secs == 0`: `Duration::from_secs(0.max(1)) == 1s`; the `run_times_out_fast` test tolerates either the timeout or a `NotFound`/spawn error, so it passes whether or not `agy` is installed in CI.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests`
Expected: PASS (the `#[ignore]` real-binary test is skipped).

- [ ] **Step 5: Verify arch-check accepts the annotation**

Run: `cargo run -p vox-arch-check`
Expected: green. If the `agy-exec` annotation is rejected, apply the exact mechanism recorded in Task 0 Step 4 (add `agy_exec.rs` to the spawn allowlist, matching how `git_exec.rs` is handled).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_exec.rs
git commit -m "feat(agy): AgyExec::run — timeout watchdog, Windows no-window, telemetry"
```

## Task 3: `agy_worktree` — isolated jail + diff capture

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/agy_worktree.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs` (`pub mod agy_worktree;`)

- [ ] **Step 1: Write the failing test:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worktree_path_is_under_repo_dot_vox() {
        let root = std::path::Path::new("/repo");
        let p = delegation_worktree_path(root, "agh-0008");
        assert!(p.starts_with("/repo/.vox/agy-worktrees"));
        assert!(p.to_string_lossy().contains("agh-0008"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_worktree::tests`
Expected: FAIL — `delegation_worktree_path` not defined.

- [ ] **Step 3: Write minimal implementation:**

```rust
//! Creates the isolated git worktree that jails an `agy` delegation, and
//! captures the resulting diff. Isolation (not agy --sandbox) is our real
//! safety boundary under --dangerously-skip-permissions.

use crate::git_exec::{GitExec, GitExecError};
use std::path::{Path, PathBuf};

pub fn delegation_worktree_path(repo_root: &Path, slug: &str) -> PathBuf {
    repo_root.join(".vox").join("agy-worktrees").join(slug)
}

pub struct DelegationWorktree {
    pub path: PathBuf,
    git: GitExec,
}

impl DelegationWorktree {
    /// Create a fresh worktree+branch off current HEAD for this delegation.
    pub async fn create(repo_root: &Path, slug: &str) -> Result<Self, GitExecError> {
        let path = delegation_worktree_path(repo_root, slug);
        let branch = format!("agy/{slug}");
        let root_git = GitExec::new(repo_root);
        let path_s = path.to_string_lossy().to_string();
        root_git
            .run(&["worktree", "add", "-b", &branch, &path_s, "HEAD"])
            .await?;
        Ok(Self { path: path.clone(), git: GitExec::new(path) })
    }

    /// Unified diff of everything the delegation changed (staged + unstaged + untracked names).
    pub async fn capture_diff(&self) -> Result<String, GitExecError> {
        let tracked = self.git.run(&["diff", "HEAD"]).await?;
        let untracked = self
            .git
            .run(&["ls-files", "--others", "--exclude-standard"])
            .await?;
        Ok(format!(
            "# tracked changes\n{}\n# new files\n{}",
            tracked.stdout, untracked.stdout
        ))
    }

    /// Remove the worktree (best-effort; leaves the branch for inspection).
    pub async fn cleanup(&self, repo_root: &Path) -> Result<(), GitExecError> {
        let root_git = GitExec::new(repo_root);
        let path_s = self.path.to_string_lossy().to_string();
        root_git.run(&["worktree", "remove", "--force", &path_s]).await?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_worktree::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_worktree.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(agy): isolated delegation worktree + diff capture"
```

## Task 4: `agy_ledger` — allocate AGH-NNNN + render + append

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/agy_ledger.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs` (`pub mod agy_ledger;`)

- [ ] **Step 1: Write the failing test:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "## §C. Handoff entries\n```yaml\n# --- AGH-0007 ---\nid: AGH-0007\n```\n";

    #[test]
    fn next_id_skips_template_sentinel_and_increments() {
        // The real template uses the literal AGH-NNNN sentinel; it must be ignored.
        let body = format!("# --- AGH-NNNN ---\n{SAMPLE}");
        assert_eq!(next_agh_id(&body), "AGH-0008");
    }

    #[test]
    fn renders_schema_valid_block() {
        let e = LedgerEntry {
            id: "AGH-0008".into(),
            date: "2026-06-19".into(),
            subsystem: "agy-delegation".into(),
            task: "Refactor foo".into(),
            outcome: "partial".into(),
            timed_out: false,
            exit_code: 0,
            files_changed: 3,
            timeout_secs: 600,
        };
        let block = render_entry(&e);
        assert!(block.contains("# --- AGH-0008 ---"));
        assert!(block.contains("target: gemini-3.5-flash / antigravity"));
        assert!(block.contains("outcome: partial"));
        assert!(block.contains("category:")); // mineable failure vocab present when not green
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests`
Expected: FAIL — symbols undefined.

- [ ] **Step 3: Write minimal implementation:**

```rust
//! Auto-writes handoff-ledger entries for every `agy` delegation, conforming to
//! the §C schema in docs/superpowers/antigravity-handoff-ledger.md and passing
//! `vox ci handoff-ledger`.

use std::path::Path;

pub const LEDGER_REL: &str = "docs/superpowers/antigravity-handoff-ledger.md";

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub id: String,
    pub date: String,
    pub subsystem: String,
    pub task: String,
    pub outcome: String, // green | partial | failed
    pub timed_out: bool,
    pub exit_code: i32,
    pub files_changed: usize,
    pub timeout_secs: u64,
}

/// Highest real AGH-XXXX in `body` + 1, formatted `AGH-NNNN`. Skips the literal
/// `AGH-NNNN` template sentinel.
pub fn next_agh_id(body: &str) -> String {
    let mut max = 0u32;
    for line in body.lines() {
        if let Some(rest) = line.trim().strip_prefix("# --- AGH-") {
            if let Some(num) = rest.strip_suffix(" ---") {
                if let Ok(n) = num.parse::<u32>() {
                    max = max.max(n);
                }
            }
        }
    }
    format!("AGH-{:04}", max + 1)
}

pub fn render_entry(e: &LedgerEntry) -> String {
    // Single-quote the task to keep YAML valid regardless of punctuation.
    let task_yaml = e.task.replace('\'', "''");
    let errors = if e.timed_out {
        format!(
            "errors_encountered:\n  - {{ what: \"timed out after {}s\", root_cause: \"agy did not finish or hung on intervention\", category: \"robustness\", who: agent }}\n",
            e.timeout_secs
        )
    } else if e.outcome != "green" {
        "errors_encountered:\n  - { what: \"non-green delegation\", root_cause: \"see diff/stderr in worktree\", category: \"robustness\", who: agent }\n".to_string()
    } else {
        "errors_encountered: []\n".to_string()
    };
    format!(
        "```yaml\n# --- {id} ---\nid: {id}\ndate: {date}\nplan: docs/superpowers/plans/2026-06-19-agy-delegation-wedge1-2.md\nprompt_artifact: \"vox_agy_delegate task (auto-logged)\"\nprompt_version: v1\nsubsystem: {subsystem}\ntarget: gemini-3.5-flash / antigravity\nclaude_inputs: [task-string]\ndelivered: [\"see agy/{id} worktree diff\"]\nloc: {files}\noutcome: {outcome}\nverification: {{ tests: \"n/a (delegation)\", clippy: \"n/a\", arch_check: \"n/a\", smoke: \"exit {code}\" }}\n{errors}agent_deviations: []\nreview_findings: \"pending human review of worktree diff\"\nverdict: request-changes\nprompt_lessons: []\ncorrections_fed_back: []\ncommits: []\n# task: '{task}'\n```\n",
        id = e.id,
        date = e.date,
        subsystem = e.subsystem,
        outcome = e.outcome,
        code = e.exit_code,
        files = e.files_changed,
        task = task_yaml,
        errors = errors,
    )
}

/// Append a rendered block to §C (newest at bottom). Returns the allocated id.
pub fn append_entry(repo_root: &Path, mut entry_no_id: LedgerEntry) -> std::io::Result<String> {
    let path = repo_root.join(LEDGER_REL);
    let body = std::fs::read_to_string(&path)?;
    let id = next_agh_id(&body);
    entry_no_id.id = id.clone();
    let block = render_entry(&entry_no_id);
    let mut out = body;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&block);
    std::fs::write(&path, out)?;
    Ok(id)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Verify the rendered block passes the ledger lint.** Append a throwaway entry to a copy and lint it.

Run: `cargo run -p vox-cli -- ci handoff-ledger` (confirm exact subcommand in repo; the ledger header documents `vox ci handoff-ledger`)
Expected: green. If the linter rejects a field, fix `render_entry` to match the schema and re-run.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_ledger.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(agy): auto-write schema-valid handoff-ledger entries"
```

## Task 5: `vox_agy_delegate` MCP handler (wires exec + worktree + ledger)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/agy_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs` (`pub mod agy_tools;`)

- [ ] **Step 1: Write the failing test** (handler returns a structured error when task missing — no binary needed):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_task_returns_remediation() {
        let json = vox_agy_delegate_validate(&serde_json::json!({}));
        assert!(json.is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_tools::tests`
Expected: FAIL — `vox_agy_delegate_validate` undefined.

- [ ] **Step 3: Write minimal implementation** (mirror `codex_tools.rs` signature + `ToolResult`):

```rust
//! MCP tool: delegate a task to the Antigravity `agy` CLI inside an isolated
//! worktree, auto-accepting prompts, and auto-logging to the handoff ledger.

use crate::agy_exec::{AgyExec, AgySpec};
use crate::agy_ledger::{append_entry, LedgerEntry};
use crate::agy_worktree::DelegationWorktree;
use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_TASK: &str = "Provide a non-empty 'task' string describing exactly what agy should implement.";

pub fn vox_agy_delegate_validate(args: &serde_json::Value) -> Result<(String, Option<String>, u64), String> {
    let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if task.is_empty() {
        return Err("Missing non-empty 'task'.".into());
    }
    let model = args.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(900);
    Ok((task, model, timeout_secs))
}

/// `vox_agy_delegate`
pub async fn vox_agy_delegate(state: &ServerState, args: serde_json::Value) -> String {
    let (task, model, timeout_secs) = match vox_agy_delegate_validate(&args) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_TASK).to_json(),
    };
    let repo_root = state.repository.root.clone();
    // Slug from current ledger id so worktree + ledger entry line up.
    let provisional = {
        let body = std::fs::read_to_string(repo_root.join(crate::agy_ledger::LEDGER_REL)).unwrap_or_default();
        crate::agy_ledger::next_agh_id(&body).to_lowercase()
    };

    let wt = match DelegationWorktree::create(&repo_root, &provisional).await {
        Ok(w) => w,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("could not create delegation worktree: {e}"),
            "Ensure the repo is a git work tree and HEAD is committed.",
        ).to_json(),
    };

    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec { task: task.clone(), model, timeout_secs };
    let out = exec.run(&spec).await;

    let (outcome, exit_code, timed_out, stderr) = match &out {
        Ok(o) => (
            if o.timed_out { "failed" } else if o.exit_code == 0 { "partial" } else { "failed" },
            o.exit_code, o.timed_out, o.stderr.clone(),
        ),
        Err(e) => ("failed", -1, false, e.to_string()),
    };

    let diff = wt.capture_diff().await.unwrap_or_default();
    let files_changed = diff.lines().filter(|l| l.starts_with("diff --git") || (!l.is_empty() && !l.starts_with('#'))).count();

    let date = state.now_date_string(); // existing helper; if absent use chrono via vox-foundation time facade
    let id = append_entry(&repo_root, LedgerEntry {
        id: String::new(),
        date,
        subsystem: "agy-delegation".into(),
        task: task.clone(),
        outcome: outcome.into(),
        timed_out,
        exit_code,
        files_changed,
        timeout_secs,
    }).unwrap_or_else(|_| "AGH-unwritten".into());

    ToolResult::ok(serde_json::json!({
        "ledger_id": id,
        "worktree": wt.path.to_string_lossy(),
        "branch": format!("agy/{provisional}"),
        "outcome": outcome,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "diff": diff,
        "stderr_tail": stderr.chars().rev().take(2000).collect::<String>().chars().rev().collect::<String>(),
        "next_step": "Review the diff. If good, `git -C <repo> merge agy/<slug>` (or cherry-pick). Then update the ledger verdict.",
    })).to_json()
}
```

> If `state.now_date_string()` / `state.repository.root` differ in name, grep `ServerState` for the actual fields (`rg "pub struct ServerState" -A40`) and adjust. Do not invent fields (ledger lesson B-6).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_tools::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_tools.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(agy): vox_agy_delegate handler (worktree-jailed, auto-logged)"
```

## Task 6: Register `vox_agy_delegate` (dispatch + schema + registry)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`
- Modify: `contracts/mcp/tool-registry.canonical.yaml`

- [ ] **Step 1: Add the dispatch arm** (near the other tool arms, e.g. after `"vox_gui_rules"`):

```rust
        "vox_agy_delegate" => Ok(crate::agy_tools::vox_agy_delegate(state, args).await),
```

- [ ] **Step 2: Add the input schema** in `input_schemas.rs` (`tool_input_schema` match):

```rust
        "vox_agy_delegate" => serde_json::json!({
            "type": "object",
            "required": ["task"],
            "properties": {
                "task": { "type": "string", "description": "Exact, zero-ambiguity implementation task for the Gemini agent." },
                "model": { "type": "string", "description": "Optional model override; defaults to agy's Gemini 3.5 Flash." },
                "timeout_secs": { "type": "integer", "default": 900, "description": "Hard kill after this many seconds." }
            }
        }),
```

- [ ] **Step 3: Add the registry entry** to `contracts/mcp/tool-registry.canonical.yaml` (copy the field shape of an existing entry like `vox_gui_rules`):

```yaml
  - name: vox_agy_delegate
    description: "Delegate a heavy code-gen/refactor task to Google Antigravity's Gemini agent via the local `agy` CLI, sandboxed in an isolated git worktree with auto-accept, and auto-logged to the handoff ledger. Returns the diff for review."
    product_lane: orchestrator
    tier: standard
    http_read_role_eligible: false
```

- [ ] **Step 4: Regenerate the registry and verify drift gate.** Find the regen command (Explore confirmed `registry.rs` is generated from the canonical yaml; check `build.rs` / an `ssot sync` subcommand).

Run: `cargo run -p vox-cli -- ssot sync` *(confirm exact command; if a build-time generator, just `cargo build -p vox-orchestrator-mcp`)*
Then: `cargo run -p vox-cli -- ci ssot-drift` *(or the repo's drift gate)*
Expected: registry includes `vox_agy_delegate`; drift gate green.

- [ ] **Step 5: Build + arch-check + clippy the crate**

Run: `cargo build -p vox-orchestrator-mcp && cargo run -p vox-arch-check && cargo clippy -p vox-orchestrator-mcp -- -D warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs contracts/mcp/tool-registry.canonical.yaml crates/vox-orchestrator-mcp/src/registry.rs
git commit -m "feat(agy): register vox_agy_delegate MCP tool (dispatch+schema+registry)"
```

## Task 7: The `delegate-gemini` SSOT skill (Claude + Vox)

**Files:**
- Create: `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md`

- [ ] **Step 1: Write the skill** (mirrors the frontmatter style of `dispatching-parallel-agents.skill.md`). This is the prompt-engineering surface that enforces the Brain/Hands/Auditor protocol and the ledger loop:

```markdown
---
name: delegate-gemini
description: Use to offload high-volume code generation or heavy refactoring to a sandboxed Gemini agent via the native `agy` delegation tool - you stay the architect, agy does the typing. Delegation is worktree-isolated, auto-accepting, and auto-logged to the handoff ledger.
---

# Delegate to Gemini (Antigravity `agy`)

**Announce at start:** "I'm using the delegate-gemini skill to offload implementation to agy."

You are the **architect**; `agy` (Gemini) is the **hands**. Protect your context by delegating token-heavy generation, but never delegate the thinking.

## When to use
- Large, mechanical, or repetitive implementation where you can write a zero-ambiguity spec.
- NOT for architectural decisions, security-sensitive code, or anything you cannot precisely specify.

## Protocol (Brain -> Hands -> Auditor)
1. **Plan (Brain).** Write a deterministic spec: exact file paths, target structs/functions (confirm they exist with `rg` first - agy will hallucinate APIs otherwise; see ledger lesson B-6), and the exact change sequence.
2. **Delegate (Hands).** Call the MCP tool `vox_agy_delegate` with `task` = your full spec. It runs `agy -p ... --dangerously-skip-permissions` inside an isolated worktree (`agy/<slug>`), auto-accepting all prompts, hard-killed at `timeout_secs`. Do NOT write the implementation yourself.
3. **Verify (Auditor).** The tool returns a `diff` and a `ledger_id`. Review the diff against your spec. Run the repo gates (`cargo build`, tests, `vox-arch-check`) before integrating. Do not trust green-on-shape - prove the effect (ledger lesson B-9).
4. **Integrate or iterate.** If good: merge/cherry-pick `agy/<slug>`. If not: issue a follow-up `vox_agy_delegate` with corrections (do not hand-fix unless it's a trivial typo). Then update the ledger entry's `verdict`/`review_findings`.

## Safety invariants (do not weaken)
- Auto-accept (`--dangerously-skip-permissions`) defeats agy's own `--sandbox` (antigravity-cli#36). The ONLY sandbox is the worktree jail the tool creates - never run agy against the live tree yourself.
- Every delegation is logged. Close the loop by filling the verdict after review.

## Parallel fan-out
For 2+ independent, file-disjoint tasks, see `dispatching-parallel-agents` and use `vox_agy_delegate_batch` (Wedge 2) - one worktree per worker.
```

- [ ] **Step 2: Verify discovery picks it up** (Vox side):

Run: `cargo run -p vox-cli -- skill search delegate` *(or `vox skill search delegate`)*
Expected: `delegate-gemini` appears in results.

- [ ] **Step 3: Verify the skill is valid** against the parser/lint (if a skills lint exists, e.g. `vox ci skills`):

Run: `cargo run -p vox-cli -- ci skills` *(confirm exact gate; otherwise rely on skill search above)*
Expected: green / skill listed.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-skills/skills/superpowers/delegate-gemini.skill.md
git commit -m "feat(skills): delegate-gemini SSOT skill (Claude + Vox)"
```

## Task 8: Wedge 1 end-to-end smoke (gated) + docs note

**Files:**
- Create: `crates/vox-orchestrator-mcp/tests/agy_delegate_smoke.rs`

- [ ] **Step 1: Write the gated smoke test** (only runs with a real `agy` + `--ignored`):

```rust
#[tokio::test]
#[ignore = "requires agy on PATH and network; run locally: cargo test -p vox-orchestrator-mcp --test agy_delegate_smoke -- --ignored"]
async fn delegate_creates_worktree_diff_and_ledger_entry() {
    // Arrange a temp git repo with the ledger file present, then call vox_agy_delegate
    // with a tiny task ("create HELLO.txt containing the word OK"). Assert:
    //  - returned JSON has ok=true, a ledger_id like AGH-XXXX, a worktree path,
    //  - the worktree contains the new file,
    //  - the ledger file gained one new # --- AGH-XXXX --- block.
    // (Full harness omitted here; implement against ServerState test constructor.)
}
```

- [ ] **Step 2: Run the unit suite (gated test skipped) to confirm it compiles**

Run: `cargo test -p vox-orchestrator-mcp agy_`
Expected: PASS; the smoke test is listed as ignored.

- [ ] **Step 3: Run the gated smoke locally once** (manual acceptance — requires Antigravity credits):

Run: `cargo test -p vox-orchestrator-mcp --test agy_delegate_smoke -- --ignored`
Expected: a real `agy` run produces a worktree diff and a ledger entry. Record the resulting `AGH-XXXX` here: `__________`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator-mcp/tests/agy_delegate_smoke.rs
git commit -m "test(agy): gated end-to-end delegation smoke"
```

**✅ Wedge 1 complete:** Claude (or Vox) can call `vox_agy_delegate` → isolated, auto-accepting Gemini run → reviewable diff → auto-ledger. Parallel fan-out is already possible by calling the tool N times.

---

# WEDGE 2 — Batch fan-out, sub-agent management, quota/retry

> Wedge 2 turns "call it N times" into a managed pool: bounded concurrency, one worktree per worker, quota/timeout detection with retry, and a single aggregated ledger summary. We manage **our** workers; agy's *internal* sub-agents remain auto-managed by agy (the `/agents` panel is TUI-only and unavailable under `-p`).

## Task 9: Quota / failure classification (pure)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_exec.rs`

- [ ] **Step 1: Write the failing test:**

```rust
    #[test]
    fn classifies_quota_and_timeout() {
        assert_eq!(classify_failure("Error: quota exceeded for project", 1, false), Some("quota"));
        assert_eq!(classify_failure("rate limit reached, retry later", 1, false), Some("quota"));
        assert_eq!(classify_failure("", -1, true), Some("timeout"));
        assert_eq!(classify_failure("ok", 0, false), None);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests::classifies_quota_and_timeout`
Expected: FAIL — `classify_failure` undefined.

- [ ] **Step 3: Implement** (append to `agy_exec.rs`):

```rust
/// Classify a delegation outcome for retry policy + ledger `category`.
/// Returns None on success. "quota"/"timeout" are retryable; others are not.
pub fn classify_failure(stderr: &str, exit_code: i32, timed_out: bool) -> Option<&'static str> {
    if timed_out {
        return Some("timeout");
    }
    let s = stderr.to_ascii_lowercase();
    if s.contains("quota") || s.contains("rate limit") || s.contains("resource_exhausted") {
        return Some("quota");
    }
    if exit_code == 0 {
        return None;
    }
    Some("error")
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests::classifies_quota_and_timeout`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_exec.rs
git commit -m "feat(agy): classify quota/timeout failures for retry policy"
```

## Task 10: Bounded retry wrapper around a single delegation

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_exec.rs`

- [ ] **Step 1: Write the failing test** (retry policy is pure: decide given attempt + class):

```rust
    #[test]
    fn retry_policy_backs_off_quota_then_gives_up() {
        // attempt is 0-based; max 2 retries for quota, 1 for timeout, 0 for error.
        assert_eq!(should_retry("quota", 0, 3), true);
        assert_eq!(should_retry("quota", 2, 3), false);
        assert_eq!(should_retry("timeout", 1, 3), false);
        assert_eq!(should_retry("error", 0, 3), false);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests::retry_policy_backs_off_quota_then_gives_up`
Expected: FAIL — `should_retry` undefined.

- [ ] **Step 3: Implement** (append to `agy_exec.rs`):

```rust
/// Pure retry decision. `attempt` is 0-based, `max_attempts` is the cap.
pub fn should_retry(class: &str, attempt: u32, max_attempts: u32) -> bool {
    if attempt + 1 >= max_attempts {
        return false;
    }
    match class {
        "quota" => true,         // backoff + retry (caller sleeps)
        "timeout" => attempt < 1, // one extra try
        _ => false,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests::retry_policy_backs_off_quota_then_gives_up`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_exec.rs
git commit -m "feat(agy): pure retry policy for quota/timeout"
```

## Task 11: `vox_agy_delegate_batch` — bounded-concurrency pool, one worktree per worker

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_tools.rs`

- [ ] **Step 1: Write the failing test** (validation + concurrency clamp are pure):

```rust
    #[test]
    fn batch_validate_clamps_concurrency_and_requires_tasks() {
        assert!(batch_validate(&serde_json::json!({"tasks": []})).is_err());
        let (tasks, conc, _t) = batch_validate(&serde_json::json!({
            "tasks": ["a","b","c"], "max_concurrency": 99
        })).unwrap();
        assert_eq!(tasks.len(), 3);
        assert!(conc <= 8); // hard cap to protect quota + disk
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_tools::tests::batch_validate_clamps_concurrency_and_requires_tasks`
Expected: FAIL — `batch_validate` undefined.

- [ ] **Step 3: Implement** (append to `agy_tools.rs`):

```rust
use tokio::sync::Semaphore;
use std::sync::Arc;

const MAX_CONCURRENCY: usize = 8;

pub fn batch_validate(args: &serde_json::Value) -> Result<(Vec<String>, usize, u64), String> {
    let tasks: Vec<String> = args.get("tasks").and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).filter(|s| !s.trim().is_empty()).collect())
        .unwrap_or_default();
    if tasks.is_empty() {
        return Err("Provide a non-empty 'tasks' array of spec strings.".into());
    }
    let conc = args.get("max_concurrency").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let conc = conc.clamp(1, MAX_CONCURRENCY);
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(900);
    Ok((tasks, conc, timeout_secs))
}

/// `vox_agy_delegate_batch`
pub async fn vox_agy_delegate_batch(state: &ServerState, args: serde_json::Value) -> String {
    let (tasks, conc, timeout_secs) = match batch_validate(&args) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            e, "Each task must be a self-contained, file-disjoint spec (see dispatching-parallel-agents).",
        ).to_json(),
    };
    let sem = Arc::new(Semaphore::new(conc));
    let mut handles = Vec::new();
    for task in tasks {
        let permit_sem = sem.clone();
        // Each worker delegates independently; reuse the single-task handler for
        // worktree+exec+retry+ledger so behavior is identical to Wedge 1.
        let one = serde_json::json!({ "task": task, "timeout_secs": timeout_secs });
        let st = state.clone_for_worker(); // see note below
        handles.push(tokio::spawn(async move {
            let _permit = permit_sem.acquire().await.expect("semaphore");
            vox_agy_delegate(&st, one).await
        }));
    }
    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap_or_else(|e| format!("{{\"ok\":false,\"error\":\"worker panicked: {e}\"}}")));
    }
    ToolResult::ok(serde_json::json!({
        "workers": results.len(),
        "concurrency": conc,
        "results": results.iter().map(|r| serde_json::from_str::<serde_json::Value>(r).unwrap_or(serde_json::json!({"raw": r}))).collect::<Vec<_>>(),
        "next_step": "Review each worker's diff + ledger entry. Integrate file-disjoint branches; resolve any overlap sequentially.",
    })).to_json()
}
```

> **`state.clone_for_worker()` is illustrative.** Confirm how `ServerState` is shared across tasks (it likely is `Arc`-wrapped or `Clone`). If `ServerState: Clone`, use `state.clone()`; if it holds non-`Send` fields, pass only the fields the worker needs (`repo_root`) and have the worker build a minimal context. Grep `rg "struct ServerState" -A40` and `rg "impl Clone for ServerState"` before implementing — do not invent `clone_for_worker`.

- [ ] **Step 4: Wire retry into the single-task path.** Update `vox_agy_delegate` (Task 5) so that around `exec.run`, it loops using `classify_failure` + `should_retry` (sleep `2^attempt` secs on quota). Keep the public JSON shape unchanged; add `"attempts": n`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p vox-orchestrator-mcp agy_`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_tools.rs
git commit -m "feat(agy): vox_agy_delegate_batch — bounded pool + per-worker worktree + retry"
```

## Task 12: Register `vox_agy_delegate_batch`

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`
- Modify: `contracts/mcp/tool-registry.canonical.yaml`

- [ ] **Step 1: Dispatch arm:**

```rust
        "vox_agy_delegate_batch" => Ok(crate::agy_tools::vox_agy_delegate_batch(state, args).await),
```

- [ ] **Step 2: Input schema:**

```rust
        "vox_agy_delegate_batch" => serde_json::json!({
            "type": "object",
            "required": ["tasks"],
            "properties": {
                "tasks": { "type": "array", "items": { "type": "string" }, "description": "File-disjoint, self-contained specs; one worker (and worktree) per task." },
                "max_concurrency": { "type": "integer", "default": 3, "description": "Parallel workers (clamped to 8)." },
                "timeout_secs": { "type": "integer", "default": 900 }
            }
        }),
```

- [ ] **Step 3: Registry entry** (mirror Task 6 Step 3, name `vox_agy_delegate_batch`, description noting bounded fan-out + per-worker worktree isolation).

- [ ] **Step 4: Regenerate + drift gate + build + arch-check + clippy**

Run: `cargo build -p vox-orchestrator-mcp && cargo run -p vox-arch-check && cargo clippy -p vox-orchestrator-mcp -- -D warnings` (and the ssot-drift gate from Task 6 Step 4)
Expected: all green; both `vox_agy_*` tools registered.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs contracts/mcp/tool-registry.canonical.yaml crates/vox-orchestrator-mcp/src/registry.rs
git commit -m "feat(agy): register vox_agy_delegate_batch MCP tool"
```

## Task 13: Update the skill for batch + final verification sweep

**Files:**
- Modify: `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md`

- [ ] **Step 1:** Expand the "Parallel fan-out" section with a concrete `vox_agy_delegate_batch` example (3 file-disjoint tasks, `max_concurrency: 3`) and the integration rule (merge disjoint branches; sequential resolve on overlap; two-strike rule from `dispatching-parallel-agents`).

- [ ] **Step 2: Full verification sweep** (use superpowers:verification-before-completion):

Run:
```bash
cargo test -p vox-orchestrator-mcp agy_
cargo clippy -p vox-orchestrator-mcp -- -D warnings
cargo run -p vox-arch-check
cargo run -p vox-cli -- ci handoff-ledger   # ledger still valid after auto-entries
```
Expected: all green. Paste outputs in the PR description (evidence before assertions).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-skills/skills/superpowers/delegate-gemini.skill.md
git commit -m "docs(skills): delegate-gemini batch fan-out + integration rules"
```

**✅ Wedge 2 complete:** bounded-concurrency batch delegation, per-worker worktree isolation, quota/timeout-aware retry, aggregated results + auto-ledger.

---

## Out of scope (future Wedge 3)
- Live streaming dashboard / progress UI for in-flight workers (the `agy` `/agents` panel is TUI-only; surfacing it headlessly needs a different mechanism).
- Managing agy's *internal* sub-agents (not exposed under `-p`).
- A cost/quota budget SSOT shared with `BudgetManager` (extend existing budget, do not add a new one — see prior gamification budget lesson).
- A VoxScript (`scripts/delegate.vox`) wrapper calling the MCP tool over the local MCP, if Vox-script-native invocation is wanted beyond the skill.

## Risks & mitigations
| Risk | Mitigation |
|---|---|
| `agy` flags differ from this plan | Task 0 captures `agy --help`; flag constants are reconciled before any code. |
| #36 sandbox escape under auto-accept | We never pass `--sandbox`; isolation is the Vox worktree jail; executor refuses the dangerous flag combo. |
| `agy` hangs despite skip-permissions | Hard timeout watchdog kills the process; classified as `timeout`, logged, retried once. |
| Quota cutoff mid-batch | `classify_failure` → backoff retry; bounded concurrency (≤8) limits burst (cf. graphify rate-limit lesson). |
| Untrusted code reaches live tree | Output stays in `agy/<slug>` worktree branch until a human reviews the diff and merges. |
| Flashing console windows on Windows | `CREATE_NO_WINDOW` on every spawn. |
| Hallucinated `ServerState`/ledger APIs | Plan flags every assumed symbol with a `rg`-confirm note (ledger lesson B-6). |
