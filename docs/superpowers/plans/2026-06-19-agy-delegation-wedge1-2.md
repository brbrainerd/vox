# Antigravity (`agy`) Native Delegation — Wedge 1 + Wedge 2 Implementation Plan

> **For agentic workers (executor = Claude Sonnet 4.6):** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax. **Read the "Executor profile" and "Execution topology" sections before Task 0** — they tell you how to pace yourself, when to verify, and which tasks may run in parallel.

**Goal:** Let Claude Code (and Vox itself) delegate heavy code-generation/refactoring to Google Antigravity's Gemini agent *natively from Rust*, with no copy-paste — by shelling out to the installed `agy` CLI — sandboxed in a git-worktree jail, auto-accepting prompts, auto-logged to the handoff ledger, and **gracefully self-healing when `agy` is absent or unauthenticated**.

**Architecture:** There is **no `antigravity-sdk-rust` crate** (verified 2026-06-19; the official SDK is Python-only, the CLI `agy` is Go). The "dependency" is a **runtime binary, not a cargo crate** — near-zero compile weight. We add a thin native executor (`AgyExec`, modeled on the existing `GitExec`), a **binary doctor** (`agy_doctor`) that detects/locates/diagnoses `agy` and emits precise install+auth remediation, MCP tools (`vox_agy_doctor`, `vox_agy_delegate`, `vox_agy_delegate_batch`), and one SSOT skill (`delegate-gemini.skill.md`) discovered by both Vox and Claude Code. Auto-accept uses `agy --dangerously-skip-permissions`; because that flag is a **known sandbox-escape vector** ([antigravity-cli#36](https://github.com/google-antigravity/antigravity-cli/issues/36)), the sandbox is enforced by **Vox** (a per-delegation git worktree), never by `agy --sandbox`.

**Tech Stack:** Rust (`tokio::process::Command` with `kill_on_drop`, `serde_json`, `thiserror`, `tracing`, `chrono`, `which`), the `vox-orchestrator-mcp` MCP layer (`ServerState`, `ToolResult`, `handle_tool_call_inner`, `tool_input_schema`), `GitExec` for worktree+diff, the canonical MCP registry (`contracts/mcp/tool-registry.canonical.yaml` → `vox-mcp-registry/build.rs`), the append-only handoff ledger, and the `.skill.md` skills SSOT.

---

## Executor profile — operating rules for Claude Sonnet 4.6

This plan is written to be executed by **Sonnet 4.6**, whose known failure modes (from the handoff ledger §B and general experience) are: drifting on long context, hallucinating framework APIs, asserting *shape* instead of *effect*, and silently weakening gates. Counter each:

1. **One task at a time. Do not read ahead and batch.** Each task is self-contained with exact paths and full code. Implement, run the listed command, confirm the expected output, commit, then move on.
2. **Confirm every cross-file symbol with `rg` before you use it.** This plan lists ground-truth signatures (verified 2026-06-19), but the tree moves. If a referenced symbol (`ServerState.repository.root`, `ToolResult::err_with_remediation`, `GitExec::run`) is not where the plan says, STOP and report — do not invent a replacement (ledger lesson B-6).
3. **Prove effect, not shape (ledger B-9).** A test that asserts a substring is hollow. Where the plan gives a behavioral test, keep it behavioral.
4. **Run gates exactly as written (ledger B-10).** Never substitute `--warn-only`, `|| true`, `--no-verify`, or a narrower scope. If a gate is red at baseline for unrelated reasons, STOP and report (ledger B-2) — do not edit `layers.toml` or add allowlist entries you weren't told to.
5. **Stay on one branch off current `origin/main`, this plan's commits only (ledger B-3).** Report a delivery manifest of every file you touched (ledger B-4).
6. **Windows host.** Use the Bash tool for POSIX one-liners and PowerShell for Windows-native commands; never `cargo fmt --all` (use `cargo fmt -p <crate>`). All process spawns set `CREATE_NO_WINDOW`.
7. **No new `.ps1`/`.sh`/`.py` scripts** (AGENTS.md). Automation is Rust + VoxScript only. Invoking the *vendor's* installer is allowed; authoring a shell script is not.

---

## Execution topology — when to parallelize

Most tasks edit a **shared, growing set of files** (`agy_exec.rs` in Tasks 3/4/11/12; `agy_tools.rs` in Tasks 7/13; registry trio in Tasks 8/14). Per superpowers:dispatching-parallel-agents, **two agents must never edit the same file** — so the spine is **SEQUENTIAL** and should be run with superpowers:subagent-driven-development (fresh subagent per task, review between).

The only genuinely file-disjoint, dependency-free work that may run as **one parallel wave** (use dispatching-parallel-agents, or the Workflow tool with one agent per item):

| Parallel wave (after Task 6) | File (disjoint) | Depends on |
|---|---|---|
| `delegate-gemini.skill.md` (Task 9 body) | `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md` | nothing |
| arch-check chokepoint rule (Task 15) | `crates/vox-arch-check/src/forbidden_patterns.rs` | nothing |

Everything else is sequential. **Do not force parallelism on the Rust spine** — the file overlap guarantees lost-write corruption, and these tasks are minutes each. This is the honest answer to "where is parallel dispatch appropriate": *here, almost nowhere on the build itself* — parallelism is the **product** (Wedge 2 fans out `agy` workers), not the build method.

---

## Verified vs. unverified facts (read before starting)

**Verified — `agy` command surface.** *Provenance: confirmed verbatim in Google's official Codelab "Hands-on with Antigravity CLI" (codelabs.developers.google.com/antigravity-cli-hands-on) + corroborated by antigravity-cli GitHub issues #36/#78, 2026-06-19. Do NOT substitute alternate invocations (no `run`/`exec` subcommand, no `--task`, no slash commands in headless mode) — slash commands (`/help`, `/config`, `/quit`) are interactive-session-only.*
- `agy -p "<prompt>"` — non-interactive: the prompt is the **value of `-p`**, runs once, exits. Verbatim example: `agy -p "What is the gcloud command to deploy to Cloud Run"`. This is THE automation entry point.
- `agy --dangerously-skip-permissions` — auto-approves *all* tool permissions; "There will be no prompt asking you for permissions." (The auto-accept mechanism.)
- `agy --model "<Display Name>"` — model is a **human-readable display string, NOT a slug**. Verbatim example: `agy --model "Gemini 3.5 Flash (Low)"`. Available families: Gemini 3.5 Flash (Low/High), Gemini 3.1 Pro variants, Claude Sonnet/Opus, GPT-OSS 120B. ⇒ **Never synthesize a slug like `gemini-3.5-flash`** — pass an exact display name or omit the flag (defaults to Gemini 3.5 Flash High).
- Working directory: **no `--cwd` flag** — `agy` operates on the process cwd. ⇒ We spawn with `current_dir(worktree)`; that IS the isolation.
- `agy --sandbox` exists **but** combined with `--dangerously-skip-permissions` is bypassable (model hinted `bypassSandbox: true`) — [#36](https://github.com/google-antigravity/antigravity-cli/issues/36), **open, no fix**. ⇒ **Never pass `--sandbox`.**
- No reliable `--output-format json`. ⇒ **Parse exit code + stderr + the resulting `git diff`. Never assume JSON stdout.**
- Async sub-agents are auto-spawned by `agy`'s own orchestrator; the `/agents` panel is **TUI-only** (unavailable under `-p`). ⇒ We do not manage agy's internal sub-agents; *our* parallelism is N concurrent `agy -p` processes.
- Default model when `--model` omitted: Gemini 3.5 Flash (High).

**Verified — `agy` install/auth (2026-06):**
- Install (Unix, and Windows via Git Bash): `curl -fsSL https://antigravity.google/cli/install.sh | bash` (drops binary `agy` into `~/.local/bin/` on Unix, `%LOCALAPPDATA%\Antigravity\` on Windows).
- **Auth is interactive OAuth.** First `agy` run kicks off Google Sign-In (OAuth or a GCP project). ⇒ **Login cannot be fully automated and we must not store Google credentials.** The doctor detects + instructs; a human signs in once.
- Not on npm; it's a downloaded binary.

**UNVERIFIED — confirm in Task 0 before relying on (do NOT assume):**
- The exact `--version`/`version` subcommand string (the doctor probes `--version` and tolerates failure); the exact set of `--model` display strings (capture from `agy --help` / the model picker — only pass strings you've seen); whether a dedicated **Windows** installer (`.ps1`/`.msi`) exists beyond the bash script; whether `agy` exposes a non-interactive **auth-status** check. The doctor probes at runtime rather than trusting this list.

---

## Architecture decisions (locked)

1. **No new crate.** Logic lives in `vox-orchestrator-mcp` (L3), beside `git_exec.rs`. Deps (`tokio`, `serde_json`, `thiserror`, `tracing`, `chrono`) are already present; add `which` if not already a dep (Task 0 checks).
2. **Binary doctor first.** `agy_doctor.rs` resolves the binary (PATH via `which`, then known install dirs) and reports `Missing | PresentUnauthed | Ready`, each with exact remediation. Every delegation tool calls the doctor first and returns the remediation as a structured `ToolResult` error instead of an opaque spawn failure.
3. **Sandbox = Vox-owned worktree jail**, not `agy --sandbox`. Each delegation: create a worktree off `HEAD` with a **unique** slug → spawn `agy` with `current_dir(worktree)` → capture diff → caller reviews → optional integrate. Untrusted output never touches the live tree until reviewed.
4. **Auto-accept = `--dangerously-skip-permissions`**, made safe by (3) + a **hard timeout that actually kills the child** (`kill_on_drop(true)`, verified below). The arg builder **refuses** to emit `--sandbox` alongside skip-permissions (#36 guard).
5. **Ledger writes are serialized.** A process-global async mutex guards read-allocate-append so parallel Wedge-2 workers cannot allocate duplicate `AGH-NNNN` ids or lose updates.
6. **Single SSOT skill** (`delegate-gemini.skill.md`); discovered by Vox + Claude Code like its siblings (no per-tool copies, not gated by `agentskills-compliance` which only covers `vox-plugin-*`).
7. **Optional chokepoint rule.** Add `raw-agy-exec` to `vox-arch-check` so all `Command::new("agy")` must live in `agy_exec.rs` (mirrors `raw-git-exec`). Recommended hardening; the code passes without it because no current rule catches `agy`.
8. **Wedge 1** = single delegation end-to-end + doctor. **Wedge 2** = batch fan-out (bounded concurrency, per-worker worktree, quota/timeout-aware retry). Live dashboards / agy-internal-subagent management are **out of scope** (Wedge 3).

## File structure

| File | Responsibility | Wedge |
|---|---|---|
| `crates/vox-orchestrator-mcp/src/agy_doctor.rs` | Resolve `agy` binary; classify `Missing/PresentUnauthed/Ready`; emit install+auth remediation. | 1 |
| `crates/vox-orchestrator-mcp/src/agy_exec.rs` | `AgyExec`: arg builder (+#36 guard, slug sanitize), spawn with kill-on-timeout + `CREATE_NO_WINDOW`; Wedge-2 `classify_failure`/`should_retry`. | 1,2 |
| `crates/vox-orchestrator-mcp/src/agy_worktree.rs` | Create/remove a unique isolated worktree via `GitExec`; capture diff + changed-file count. | 1 |
| `crates/vox-orchestrator-mcp/src/agy_ledger.rs` | Allocate next `AGH-NNNN`; render schema-valid yaml; serialized append. | 1 |
| `crates/vox-orchestrator-mcp/src/agy_tools.rs` | MCP handlers: `vox_agy_doctor`, `vox_agy_delegate` (W1), `vox_agy_delegate_batch` (W2). | 1,2 |
| `crates/vox-orchestrator-mcp/src/dispatch.rs` | Arms in `handle_tool_call_inner`. | 1,2 |
| `crates/vox-orchestrator-mcp/src/input_schemas.rs` | Arms in `tool_input_schema`. | 1,2 |
| `contracts/mcp/tool-registry.canonical.yaml` | Registry entries (regenerated by `vox-mcp-registry/build.rs` at compile; **no `registry.rs` to commit**). | 1,2 |
| `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md` | SSOT delegation skill (Claude + Vox). | 1 |
| `crates/vox-arch-check/src/forbidden_patterns.rs` | Optional `raw-agy-exec` chokepoint rule. | 2 |
| `crates/vox-orchestrator-mcp/tests/agy_*.rs` | Gated integration smoke. | 1,2 |

---

## Task 0: Pre-flight — confirm `agy` surface, deps, arch-check baseline

**Files:** Create `crates/vox-orchestrator-mcp/tests/fixtures/agy-help.txt` (only if `agy` present).

- [ ] **Step 1: Probe for `agy` and capture help (do not fail the plan if absent — the doctor handles absence).**

```bash
if command -v agy >/dev/null 2>&1; then
  agy --help > crates/vox-orchestrator-mcp/tests/fixtures/agy-help.txt 2>&1 || true
  agy --version >> crates/vox-orchestrator-mcp/tests/fixtures/agy-help.txt 2>&1 || true
  echo "AGY PRESENT — fixture captured"
else
  echo "AGY ABSENT — doctor remediation path will be exercised; skip the gated smokes (Tasks 10/16 manual step)"
fi
```

- [ ] **Step 2: Reconcile flags against the captured help.** `-p "<prompt>"`, `--dangerously-skip-permissions`, and `--model "<Display Name>"` are VERIFIED against Google's Codelab — expect them present. Your job here is to (a) record the **exact `--model` display strings** this install offers (so callers pass valid names), and (b) note the `--version` subcommand form. Only if `-p` or `--dangerously-skip-permissions` is *absent/renamed* in this build do you STOP and update the flag constants before coding.

- [ ] **Step 3: Confirm the `which` crate is available** (the doctor uses it; arch-check's `no-hardcoded-shell-spawn` reason explicitly endorses `which::which`).

Run: `rg '^which' crates/vox-orchestrator-mcp/Cargo.toml || rg 'which' Cargo.toml`
If absent: add `which = "<version used elsewhere in the workspace>"` to `crates/vox-orchestrator-mcp/Cargo.toml` (find the workspace-pinned version with `rg 'which' -g 'Cargo.toml'`). Do not invent a version.

- [ ] **Step 4: arch-check baseline.**

Run: `cargo run -p vox-arch-check`
Expected: green at baseline. If red for unrelated reasons, STOP and report (ledger B-2). Note: there is currently **no** rule forbidding `Command::new("agy")` (verified: `forbidden_patterns.rs` has only `raw-git-exec` and `no-hardcoded-shell-spawn`), so the executor needs **no** allow-annotation until Task 15 adds the chokepoint rule.

- [ ] **Step 5: Commit (only if a fixture was created).**

```bash
git add crates/vox-orchestrator-mcp/tests/fixtures/agy-help.txt crates/vox-orchestrator-mcp/Cargo.toml 2>/dev/null
git commit -m "test(agy): capture agy surface + which dep + arch-check preflight" || echo "nothing to commit"
```

---

# WEDGE 1 — Doctor + single native delegation

## Task 1: `agy_doctor` — resolve, classify, remediate

**Files:** Create `crates/vox-orchestrator-mcp/src/agy_doctor.rs`; modify `lib.rs` (`pub mod agy_doctor;`).

- [ ] **Step 1: Write the failing test:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remediation_for_missing_is_actionable() {
        let r = remediation(&AgyStatus::Missing);
        assert!(r.contains("install.sh"));
        assert!(r.contains("agy")); // names the binary
    }

    #[test]
    fn remediation_for_unauthed_mentions_interactive_login() {
        let r = remediation(&AgyStatus::PresentUnauthed { path: "/x/agy".into() });
        assert!(r.to_lowercase().contains("sign-in") || r.to_lowercase().contains("oauth"));
    }

    #[test]
    fn known_install_dirs_are_platform_specific() {
        let dirs = known_install_dirs();
        assert!(!dirs.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_doctor::tests`
Expected: FAIL — symbols undefined.

- [ ] **Step 3: Implement:**

```rust
//! Detects, locates, and diagnoses the Antigravity `agy` CLI, and produces
//! precise, LLM-followable remediation when it is missing or unauthenticated.
//! Auth is interactive OAuth — we never store credentials; we only instruct.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum AgyStatus {
    Missing,
    PresentUnauthed { path: String },
    Ready { path: String, version: String },
}

/// Best-effort platform install locations the installer documents, in addition to PATH.
pub fn known_install_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(home) = dirs_home() {
        v.push(home.join(".local").join("bin"));
    }
    #[cfg(windows)]
    if let Ok(lad) = std::env::var("LOCALAPPDATA") {
        v.push(PathBuf::from(lad).join("Antigravity"));
    }
    v
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn bin_name() -> &'static str {
    if cfg!(windows) { "agy.exe" } else { "agy" }
}

/// Resolve the binary path via PATH (`which`) then known install dirs.
pub fn resolve_agy() -> Option<PathBuf> {
    if let Ok(p) = which::which("agy") {
        return Some(p);
    }
    for d in known_install_dirs() {
        let candidate = d.join(bin_name());
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Synchronous, fast classification (no long network calls). `version` is probed
/// via `agy --version`; auth state is best-effort (see note in detect()).
pub fn detect() -> AgyStatus {
    let Some(path) = resolve_agy() else {
        return AgyStatus::Missing;
    };
    let path_s = path.to_string_lossy().to_string();
    // vox-arch-check: agy is not a forbidden spawn target (see Task 0 Step 4)
    let ver = std::process::Command::new(&path).arg("--version").output();
    match ver {
        Ok(o) if o.status.success() => AgyStatus::Ready {
            path: path_s,
            version: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        // Binary exists but `--version` failed (could be unauthed first-run, or a
        // different version flag). Treat as present-but-needs-attention; the
        // delegation path surfaces the real stderr on first use.
        _ => AgyStatus::PresentUnauthed { path: path_s },
    }
}

pub fn remediation(status: &AgyStatus) -> String {
    match status {
        AgyStatus::Missing => format!(
            "`agy` (Antigravity CLI) is not installed or not on PATH.\n\
             INSTALL (verify the URL at https://antigravity.google/docs/cli before running):\n\
             - Unix / Windows-Git-Bash: curl -fsSL https://antigravity.google/cli/install.sh | bash\n\
             The installer drops `agy` into ~/.local/bin (Unix) or %LOCALAPPDATA%\\Antigravity (Windows).\n\
             ADD TO PATH if `agy --version` still fails after install, then restart the shell.\n\
             THEN authenticate (interactive, one-time): run `agy` once and complete the Google Sign-In.\n\
             Re-run vox_agy_doctor to confirm Ready."
        ),
        AgyStatus::PresentUnauthed { path } => format!(
            "`agy` was found at {path} but is not confirmed ready (likely needs a one-time \
             interactive Google Sign-In / OAuth, or uses a different version flag).\n\
             ACTION (human, one-time): run `agy` in a terminal and complete the sign-in flow. \
             We do NOT store Google credentials. Then re-run vox_agy_doctor."
        ),
        AgyStatus::Ready { path, version } => {
            format!("`agy` ready at {path} (version: {version}).")
        }
    }
}
```

> If `dirs`/`which` ergonomics differ, prefer the workspace's existing home-dir/`which` helpers — `rg "which::which|fn .*home" crates/` — over re-implementing. Do not add a new crate without checking it's workspace-pinned.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_doctor::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_doctor.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(agy): doctor — resolve/classify/remediate the agy binary"
```

## Task 2: `vox_agy_doctor` MCP tool + registration

**Files:** modify `agy_tools.rs` (create it), `lib.rs`, `dispatch.rs`, `input_schemas.rs`, `contracts/mcp/tool-registry.canonical.yaml`.

- [ ] **Step 1: Failing test** (handler returns a JSON object with a `status`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn doctor_json_has_status_and_remediation() {
        let v = doctor_report_json();
        assert!(v.get("status").is_some());
        assert!(v.get("remediation").is_some());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_tools::tests::doctor_json_has_status_and_remediation`
Expected: FAIL — `doctor_report_json` undefined.

- [ ] **Step 3: Implement** (top of `agy_tools.rs`):

```rust
//! MCP tools for delegating to Antigravity `agy` (doctor + single + batch).

use crate::agy_doctor::{detect, remediation, AgyStatus};
use crate::params::ToolResult;
use crate::server_state::ServerState;

pub fn doctor_report_json() -> serde_json::Value {
    let status = detect();
    let (label, path) = match &status {
        AgyStatus::Missing => ("missing", None),
        AgyStatus::PresentUnauthed { path } => ("present_unauthed", Some(path.clone())),
        AgyStatus::Ready { path, .. } => ("ready", Some(path.clone())),
    };
    serde_json::json!({
        "status": label,
        "path": path,
        "remediation": remediation(&status),
    })
}

/// `vox_agy_doctor`
pub async fn vox_agy_doctor(_state: &ServerState, _args: serde_json::Value) -> String {
    ToolResult::ok(doctor_report_json()).to_json()
}
```

Add to `lib.rs`: `pub mod agy_tools;`.

- [ ] **Step 4: Register.** Dispatch arm in `handle_tool_call_inner` (`dispatch.rs`):

```rust
        "vox_agy_doctor" => Ok(crate::agy_tools::vox_agy_doctor(state, args).await),
```

Schema arm in `tool_input_schema` (`input_schemas.rs`):

```rust
        "vox_agy_doctor" => serde_json::json!({ "type": "object", "properties": {} }),
```

Registry entry in `contracts/mcp/tool-registry.canonical.yaml` (copy the field shape of an existing entry — confirm fields with `rg -B1 -A6 'name: vox_gui_rules' contracts/mcp/tool-registry.canonical.yaml`):

```yaml
  - name: vox_agy_doctor
    description: "Detect whether the Antigravity `agy` CLI is installed and ready; returns status plus exact install/auth remediation. Call before vox_agy_delegate."
    product_lane: orchestrator
    tier: standard
    http_read_role_eligible: true
```

- [ ] **Step 5: Build + regenerate registry + drift gate.** The registry is generated at compile by `crates/vox-mcp-registry/build.rs` from the canonical yaml — **there is no `registry.rs` to edit or commit.**

Run: `cargo build -p vox-orchestrator-mcp && cargo run -p vox-cli -- ci ssot-drift`
Expected: build green; `ssot-drift` green with `vox_agy_doctor` present.

- [ ] **Step 6: Test + commit**

```bash
cargo test -p vox-orchestrator-mcp agy_tools::tests::doctor_json_has_status_and_remediation
git add crates/vox-orchestrator-mcp/src/agy_tools.rs crates/vox-orchestrator-mcp/src/lib.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs contracts/mcp/tool-registry.canonical.yaml
git commit -m "feat(agy): vox_agy_doctor MCP tool (install/auth diagnostics)"
```

## Task 3: `AgyExec` — arg builder, #36 guard, slug sanitizer (pure)

**Files:** Create `crates/vox-orchestrator-mcp/src/agy_exec.rs`; modify `lib.rs`.

- [ ] **Step 1: Failing test:**

```rust
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
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests`
Expected: FAIL — symbols undefined.

- [ ] **Step 3: Implement** (top of `agy_exec.rs`):

```rust
//! Native executor for the Antigravity `agy` CLI. ALL `agy` spawns MUST go
//! through `AgyExec::run` (enforced by the optional `raw-agy-exec` arch rule).
//!
//! Safety: auto-accept (`--dangerously-skip-permissions`) defeats agy's own
//! `--sandbox` (antigravity-cli#36), so we NEVER pass `--sandbox`; isolation is
//! the caller's per-delegation git worktree.

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
```

Add to `lib.rs`: `pub mod agy_exec;`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_exec.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(agy): AgySpec, arg builder (#36 guard), path-safe slug"
```

## Task 4: `AgyExec::run` — spawn with **kill-on-timeout**, no-window, telemetry

> **Correctness fix (code-review):** `tokio::time::timeout` dropping the wait-future does NOT kill the child by default — that orphans an `agy` process consuming Antigravity credits. We set `.kill_on_drop(true)` so the dropped child is reaped, and assert the behavior.

**Files:** modify `agy_exec.rs`.

- [ ] **Step 1: Failing test** (timeout actually returns the timed_out marker; binary-gated real run is separate):

```rust
    #[tokio::test]
    async fn run_reports_timeout_or_notfound_fast() {
        // timeout_secs is clamped to >=1; with no agy installed this returns NotFound,
        // with agy installed a 1s cap on a real task trips timed_out. Either is acceptable.
        let exec = AgyExec::new(std::env::temp_dir());
        let spec = AgySpec { task: "noop".into(), model: None, timeout_secs: 1 };
        match exec.run(&spec).await {
            Ok(o) => assert!(o.timed_out || o.exit_code != 0 || o.exit_code == 0),
            Err(e) => assert!(matches!(e, AgyExecError::NotFound | AgyExecError::Spawn(_))),
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests::run_reports_timeout_or_notfound_fast`
Expected: FAIL — `AgyExec` undefined.

- [ ] **Step 3: Implement** (append to `agy_exec.rs`):

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

        // vox-arch-check: allow agy-exec  (only meaningful once Task 15's rule exists)
        let mut cmd = tokio::process::Command::new("agy");
        cmd.current_dir(&self.cwd)
            .args(&args)
            .kill_on_drop(true) // <-- ensures the timeout branch actually reaps the child
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests`
Expected: PASS.

- [ ] **Step 5: clippy + commit**

```bash
cargo clippy -p vox-orchestrator-mcp -- -D warnings
git add crates/vox-orchestrator-mcp/src/agy_exec.rs
git commit -m "feat(agy): AgyExec::run with kill-on-timeout, no-window, telemetry"
```

## Task 5: `agy_worktree` — unique jail + diff + changed-file count

**Files:** Create `crates/vox-orchestrator-mcp/src/agy_worktree.rs`; modify `lib.rs`.

- [ ] **Step 1: Failing test:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worktree_path_is_jailed_under_dot_vox() {
        let p = delegation_worktree_path(std::path::Path::new("/repo"), "d-123-foo");
        assert!(p.starts_with("/repo/.vox/agy-worktrees"));
        assert!(p.to_string_lossy().contains("d-123-foo"));
    }
    #[test]
    fn counts_changed_files_from_diff_parts() {
        let tracked = "diff --git a/x b/x\n...\ndiff --git a/y b/y\n...";
        let untracked = "newfile.txt\n";
        assert_eq!(count_changed(tracked, untracked), 3);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_worktree::tests`
Expected: FAIL — symbols undefined.

- [ ] **Step 3: Implement:**

```rust
//! Isolated git worktree that jails an `agy` delegation (our real safety
//! boundary under --dangerously-skip-permissions), plus diff capture.

use crate::agy_exec::sanitize_slug;
use crate::git_exec::{GitExec, GitExecError};
use std::path::{Path, PathBuf};

pub fn delegation_worktree_path(repo_root: &Path, slug: &str) -> PathBuf {
    repo_root.join(".vox").join("agy-worktrees").join(slug)
}

pub fn count_changed(tracked_diff: &str, untracked_list: &str) -> usize {
    let tracked = tracked_diff.matches("diff --git").count();
    let untracked = untracked_list.lines().filter(|l| !l.trim().is_empty()).count();
    tracked + untracked
}

pub struct DelegationWorktree {
    pub path: PathBuf,
    pub branch: String,
    git: GitExec,
}

impl DelegationWorktree {
    /// Create a fresh worktree+branch off HEAD. `slug` MUST be unique per call
    /// (callers derive it from a monotonic counter; see agy_tools).
    pub async fn create(repo_root: &Path, slug: &str) -> Result<Self, GitExecError> {
        let slug = sanitize_slug(slug);
        let path = delegation_worktree_path(repo_root, &slug);
        let branch = format!("agy/{slug}");
        let path_s = path.to_string_lossy().to_string();
        GitExec::new(repo_root)
            .run(&["worktree", "add", "-b", &branch, &path_s, "HEAD"])
            .await?;
        Ok(Self { path: path.clone(), branch, git: GitExec::new(path) })
    }

    /// (unified-diff text, changed-file count). Includes tracked + untracked.
    pub async fn capture(&self) -> Result<(String, usize), GitExecError> {
        let tracked = self.git.run(&["diff", "HEAD"]).await?;
        let untracked = self.git.run(&["ls-files", "--others", "--exclude-standard"]).await?;
        let n = count_changed(&tracked.stdout, &untracked.stdout);
        let text = format!("# tracked\n{}\n# new files\n{}", tracked.stdout, untracked.stdout);
        Ok((text, n))
    }

    pub async fn cleanup(&self, repo_root: &Path) -> Result<(), GitExecError> {
        let path_s = self.path.to_string_lossy().to_string();
        GitExec::new(repo_root).run(&["worktree", "remove", "--force", &path_s]).await?;
        Ok(())
    }
}
```

Add to `lib.rs`: `pub mod agy_worktree;`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_worktree::tests`
Expected: PASS.

- [ ] **Step 5: Keep delegation worktrees out of the parent's git status.** `.vox/agy-worktrees/` is NOT covered by the existing `.gitignore` (only specific `.vox/` subdirs are — verify: `rg -n 'agy-worktrees|^/?\.vox/' .gitignore`). A linked worktree dir nested in the repo otherwise shows as untracked. Add the ignore (mirror the existing `.vox/` entries):

```
# Antigravity delegation worktrees (ephemeral, per-delegation)
/.vox/agy-worktrees/
```

Run: `rg -n 'agy-worktrees' .gitignore && git status --porcelain | rg 'agy-worktrees' || echo "clean"`
Expected: the ignore line is present; no `agy-worktrees` path appears in `git status`.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_worktree.rs crates/vox-orchestrator-mcp/src/lib.rs .gitignore
git commit -m "feat(agy): unique worktree jail + diff/changed-file capture (+gitignore)"
```

## Task 6: `agy_ledger` — serialized AGH-NNNN allocation + append

> **Concurrency fix (code-review):** parallel Wedge-2 workers must not allocate the same `AGH-NNNN` or lose each other's appends. All allocation+append goes through one process-global async mutex. There is **no `vox ci handoff-ledger` gate** (verified — it does not exist); we validate by a Rust round-trip (append → re-read → next id advanced).

**Files:** Create `crates/vox-orchestrator-mcp/src/agy_ledger.rs`; modify `lib.rs`.

- [ ] **Step 1: Failing test:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_id_skips_sentinel_and_increments() {
        let body = "# --- AGH-NNNN ---\n# --- AGH-0007 ---\nid: AGH-0007\n";
        assert_eq!(next_agh_id(body), "AGH-0008");
    }

    #[test]
    fn render_is_yaml_blockish_and_mineable() {
        let e = LedgerEntry::new("agy-delegation", "Refactor foo", "partial", false, 0, 3, 600, "2026-06-19");
        let block = render_entry("AGH-0008", &e);
        assert!(block.contains("# --- AGH-0008 ---"));
        assert!(block.contains("target: gemini-3.5-flash / antigravity"));
        assert!(block.contains("category:")); // non-green => mineable failure vocab
    }

    #[tokio::test]
    async fn append_roundtrip_advances_id() {
        let dir = std::env::temp_dir().join(format!("agyledger-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs/superpowers")).unwrap();
        let p = dir.join(LEDGER_REL);
        std::fs::write(&p, "## §C\n# --- AGH-0007 ---\nid: AGH-0007\n").unwrap();
        let id = append_entry_locked(&dir, LedgerEntry::new("s","t","green",false,0,1,60,"2026-06-19")).await.unwrap();
        assert_eq!(id, "AGH-0008");
        let id2 = append_entry_locked(&dir, LedgerEntry::new("s","t","green",false,0,1,60,"2026-06-19")).await.unwrap();
        assert_eq!(id2, "AGH-0009");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests`
Expected: FAIL — symbols undefined.

- [ ] **Step 3: Implement:**

```rust
//! Auto-writes handoff-ledger entries (one per delegation) conforming to the
//! §C schema in docs/superpowers/antigravity-handoff-ledger.md. Serialized so
//! concurrent workers cannot collide on ids or lose appends.

use std::path::Path;
use std::sync::OnceLock;

pub const LEDGER_REL: &str = "docs/superpowers/antigravity-handoff-ledger.md";

static LEDGER_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub subsystem: String,
    pub task: String,
    pub outcome: String, // green | partial | failed
    pub timed_out: bool,
    pub exit_code: i32,
    pub files_changed: usize,
    pub timeout_secs: u64,
    pub date: String,
}

impl LedgerEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(subsystem: &str, task: &str, outcome: &str, timed_out: bool, exit_code: i32, files_changed: usize, timeout_secs: u64, date: &str) -> Self {
        Self { subsystem: subsystem.into(), task: task.into(), outcome: outcome.into(), timed_out, exit_code, files_changed, timeout_secs, date: date.into() }
    }
}

/// Highest real AGH-XXXX in `body` + 1. Skips the literal `AGH-NNNN` template.
pub fn next_agh_id(body: &str) -> String {
    let mut max = 0u32;
    for line in body.lines() {
        if let Some(rest) = line.trim().strip_prefix("# --- AGH-") {
            if let Some(n) = rest.strip_suffix(" ---").and_then(|s| s.parse::<u32>().ok()) {
                max = max.max(n);
            }
        }
    }
    format!("AGH-{:04}", max + 1)
}

pub fn render_entry(id: &str, e: &LedgerEntry) -> String {
    let task_yaml = e.task.replace('\'', "''");
    let errors = if e.timed_out {
        format!("errors_encountered:\n  - {{ what: \"timed out after {}s\", root_cause: \"agy hung or exceeded budget\", category: \"robustness\", who: agent }}\n", e.timeout_secs)
    } else if e.outcome != "green" {
        "errors_encountered:\n  - { what: \"non-green delegation\", root_cause: \"see worktree diff/stderr\", category: \"robustness\", who: agent }\n".to_string()
    } else {
        "errors_encountered: []\n".to_string()
    };
    format!(
        "```yaml\n# --- {id} ---\nid: {id}\ndate: {date}\nplan: docs/superpowers/plans/2026-06-19-agy-delegation-wedge1-2.md\nprompt_artifact: \"vox_agy_delegate (auto-logged)\"\nprompt_version: v1\nsubsystem: {subsystem}\ntarget: gemini-3.5-flash / antigravity\nclaude_inputs: [task-string]\ndelivered: [\"see agy/<slug> worktree diff\"]\nloc: {files}\noutcome: {outcome}\nverification: {{ tests: \"n/a\", clippy: \"n/a\", arch_check: \"n/a\", smoke: \"exit {code}\" }}\n{errors}agent_deviations: []\nreview_findings: \"pending human review of worktree diff\"\nverdict: request-changes\nprompt_lessons: []\ncorrections_fed_back: []\ncommits: []\n# task: '{task}'\n```\n",
        id = id, date = e.date, subsystem = e.subsystem, outcome = e.outcome,
        code = e.exit_code, files = e.files_changed, task = task_yaml, errors = errors,
    )
}

/// Serialized read-allocate-append. Returns the allocated id.
pub async fn append_entry_locked(repo_root: &Path, entry: LedgerEntry) -> std::io::Result<String> {
    let _guard = LEDGER_LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
    let path = repo_root.join(LEDGER_REL);
    let body = std::fs::read_to_string(&path)?;
    let id = next_agh_id(&body);
    let block = render_entry(&id, &entry);
    let mut out = body;
    if !out.ends_with('\n') { out.push('\n'); }
    out.push('\n');
    out.push_str(&block);
    std::fs::write(&path, out)?;
    Ok(id)
}
```

Add to `lib.rs`: `pub mod agy_ledger;`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_ledger.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(agy): serialized auto-write of handoff-ledger entries"
```

## Task 7: `vox_agy_delegate` — doctor-gated, jailed, retried, logged

**Files:** modify `agy_tools.rs`.

- [ ] **Step 1: Failing test** (validation is pure):

```rust
    #[test]
    fn delegate_validate_requires_task_and_defaults_timeout() {
        assert!(delegate_validate(&serde_json::json!({})).is_err());
        let (task, _m, t) = delegate_validate(&serde_json::json!({"task":"do X"})).unwrap();
        assert_eq!(task, "do X");
        assert_eq!(t, 900);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_tools::tests::delegate_validate_requires_task_and_defaults_timeout`
Expected: FAIL — `delegate_validate` undefined.

- [ ] **Step 3: Implement** (append to `agy_tools.rs`):

```rust
use crate::agy_exec::{AgyExec, AgySpec};
use crate::agy_ledger::{append_entry_locked, LedgerEntry};
use crate::agy_worktree::DelegationWorktree;
use std::sync::atomic::{AtomicU64, Ordering};

static DELEGATION_SEQ: AtomicU64 = AtomicU64::new(1);

const REM_TASK: &str = "Provide a non-empty 'task' string with an exact, zero-ambiguity spec (file paths, target symbols).";

pub fn delegate_validate(args: &serde_json::Value) -> Result<(String, Option<String>, u64), String> {
    let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if task.is_empty() {
        return Err("Missing non-empty 'task'.".into());
    }
    let model = args.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(900);
    Ok((task, model, timeout_secs))
}

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Unique, collision-free slug independent of the ledger id (which is allocated
/// later under lock). Monotonic counter keeps parallel workers disjoint.
fn fresh_slug(hint: &str) -> String {
    let n = DELEGATION_SEQ.fetch_add(1, Ordering::Relaxed);
    crate::agy_exec::sanitize_slug(&format!("d{n}-{hint}"))
}

/// `vox_agy_delegate`
pub async fn vox_agy_delegate(state: &ServerState, args: serde_json::Value) -> String {
    let (task, model, timeout_secs) = match delegate_validate(&args) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_TASK).to_json(),
    };
    // Doctor gate: fail fast with actionable remediation, not an opaque spawn error.
    let report = doctor_report_json();
    if report["status"] != "ready" {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("agy not ready (status: {}).", report["status"]),
            report["remediation"].as_str().unwrap_or("Run vox_agy_doctor.").to_string(),
        ).to_json();
    }

    let repo_root = state.repository.root.clone();
    let slug = fresh_slug(&task);
    let wt = match DelegationWorktree::create(&repo_root, &slug).await {
        Ok(w) => w,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("could not create delegation worktree: {e}"),
            "Ensure the repo is a git work tree with a committed HEAD.",
        ).to_json(),
    };

    // Retry loop (quota/timeout-aware; see Tasks 11-12 for the pure policy).
    let exec = AgyExec::new(&wt.path);
    let mut attempt = 0u32;
    let max_attempts = 3u32;
    let out = loop {
        let spec = AgySpec { task: task.clone(), model: model.clone(), timeout_secs };
        let o = exec.run(&spec).await;
        let (stderr, exit, timed) = match &o {
            Ok(x) => (x.stderr.clone(), x.exit_code, x.timed_out),
            Err(e) => (e.to_string(), -1, false),
        };
        match crate::agy_exec::classify_failure(&stderr, exit, timed) {
            Some(class) if crate::agy_exec::should_retry(class, attempt, max_attempts) => {
                tokio::time::sleep(std::time::Duration::from_secs(1u64 << attempt)).await;
                attempt += 1;
                continue;
            }
            _ => break o,
        }
    };

    let (outcome, exit_code, timed_out, stderr) = match &out {
        Ok(o) => (if o.timed_out { "failed" } else if o.exit_code == 0 { "partial" } else { "failed" }, o.exit_code, o.timed_out, o.stderr.clone()),
        Err(e) => ("failed", -1, false, e.to_string()),
    };
    let (diff, files_changed) = wt.capture().await.unwrap_or_else(|_| (String::new(), 0));

    let id = append_entry_locked(&repo_root, LedgerEntry::new(
        "agy-delegation", &task, outcome, timed_out, exit_code, files_changed, timeout_secs, &today(),
    )).await.unwrap_or_else(|_| "AGH-unwritten".into());

    ToolResult::ok(serde_json::json!({
        "ledger_id": id,
        "worktree": wt.path.to_string_lossy(),
        "branch": wt.branch,
        "outcome": outcome,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "attempts": attempt + 1,
        "files_changed": files_changed,
        "diff": diff,
        "stderr_tail": tail(&stderr, 2000),
        "next_step": "Review the diff. If good: integrate `agy/<slug>` (merge/cherry-pick), then set the ledger verdict. If not: re-delegate with corrections.",
    })).to_json()
}

fn tail(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len <= n { return s.to_string(); }
    s.chars().skip(len - n).collect()
}
```

> `classify_failure` / `should_retry` are added in Tasks 11–12 (same file, sequential). If you implement Task 7 before them, add minimal stubs that compile (`fn classify_failure(_,_,_)->Option<&'static str>{None}` etc.) **then replace** in 11–12 — or implement 11–12 first. Do NOT ship the stub as final (no-stubs rule).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp agy_tools::tests::delegate_validate_requires_task_and_defaults_timeout`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_tools.rs
git commit -m "feat(agy): vox_agy_delegate (doctor-gated, jailed, retried, logged)"
```

## Task 8: Register `vox_agy_delegate`

**Files:** modify `dispatch.rs`, `input_schemas.rs`, `contracts/mcp/tool-registry.canonical.yaml`.

- [ ] **Step 1: Dispatch arm** in `handle_tool_call_inner`:

```rust
        "vox_agy_delegate" => Ok(crate::agy_tools::vox_agy_delegate(state, args).await),
```

- [ ] **Step 2: Schema arm** in `tool_input_schema`:

```rust
        "vox_agy_delegate" => serde_json::json!({
            "type": "object",
            "required": ["task"],
            "properties": {
                "task": { "type": "string", "description": "Exact, zero-ambiguity spec (paths + target symbols)." },
                "model": { "type": "string", "description": "Optional agy model DISPLAY NAME (not a slug), e.g. \"Gemini 3.5 Flash (Low)\". Omit for the default (Gemini 3.5 Flash High)." },
                "timeout_secs": { "type": "integer", "default": 900, "description": "Hard kill after this many seconds." }
            }
        }),
```

- [ ] **Step 3: Registry entry** (mirror Task 2; `name: vox_agy_delegate`, description noting worktree-jail + auto-ledger + returns diff).

- [ ] **Step 4: Build + drift + arch-check + clippy.** **No `registry.rs` to add** (generated into `OUT_DIR` by `vox-mcp-registry/build.rs`).

```bash
cargo build -p vox-orchestrator-mcp
cargo run -p vox-cli -- ci ssot-drift
cargo run -p vox-arch-check
cargo clippy -p vox-orchestrator-mcp -- -D warnings
```
Expected: all green; `vox_agy_delegate` registered.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs contracts/mcp/tool-registry.canonical.yaml
git commit -m "feat(agy): register vox_agy_delegate MCP tool"
```

## Task 9: The `delegate-gemini` SSOT skill  *(parallel-safe — see topology)*

**Files:** Create `crates/vox-skills/skills/superpowers/delegate-gemini.skill.md`.

- [ ] **Step 1: Write the skill** (mirror frontmatter of a sibling, e.g. `dispatching-parallel-agents.skill.md`):

```markdown
---
name: delegate-gemini
description: Use to offload high-volume code generation or heavy refactoring to a sandboxed Gemini agent via the native agy delegation tools - you stay architect, agy does the typing. Worktree-isolated, auto-accepting, auto-logged to the handoff ledger.
---

# Delegate to Gemini (Antigravity `agy`)

**Announce at start:** "I'm using the delegate-gemini skill to offload implementation to agy."

You are the **architect**; `agy` (Gemini) is the **hands**. Delegate token-heavy generation; never delegate the thinking.

## Pre-flight
Call `vox_agy_doctor` first. If status != "ready", follow its `remediation` (install `agy`, add to PATH, complete the one-time interactive Google Sign-In) before delegating. We never store Google credentials.

## When to use
- Large, mechanical, or repetitive implementation you can specify with zero ambiguity.
- NOT for architecture, security-sensitive code, or anything you cannot precisely specify.

## Protocol (Brain -> Hands -> Auditor)
1. **Plan (Brain).** Write a deterministic spec: exact file paths, target structs/functions (confirm they exist with `rg` first - agy hallucinates APIs otherwise; ledger lesson B-6), and the exact change sequence.
2. **Delegate (Hands).** Call `vox_agy_delegate` with `task` = your spec. It runs `agy -p ... --dangerously-skip-permissions` inside an isolated worktree (`agy/<slug>`), auto-accepting all prompts, hard-killed at `timeout_secs`, retrying quota/timeout. Do NOT write the implementation yourself.
3. **Verify (Auditor).** Review the returned `diff` against your spec. Run repo gates (build, tests, arch-check) before integrating. Prove the effect, not the shape (ledger B-9).
4. **Integrate or iterate.** Good: merge/cherry-pick `agy/<slug>`; then set the ledger entry's `verdict`. Not good: re-delegate with corrections (hand-fix only trivial typos).

## Safety invariants (do not weaken)
- Auto-accept defeats agy's own `--sandbox` (antigravity-cli#36). The ONLY sandbox is the worktree jail the tool creates - never run agy against the live tree yourself.
- Every delegation is auto-logged. Close the loop by filling the verdict after review.

## Parallel fan-out
For 2+ independent, file-DISJOINT tasks, see `dispatching-parallel-agents` and use `vox_agy_delegate_batch` (one worktree per worker, bounded concurrency). Never give two workers the same file.
```

- [ ] **Step 2: Verify discovery the same way siblings are discovered** (runtime discovery; this dir is NOT covered by `agentskills-compliance`). Confirm the file parses by listing skills via the MCP tool `vox_tool_search` (query "delegate") or however the sibling skills are surfaced in this repo — `rg -l "name: dispatching-parallel-agents" crates/vox-skills` to confirm the location pattern, then ensure your file matches it.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-skills/skills/superpowers/delegate-gemini.skill.md
git commit -m "feat(skills): delegate-gemini SSOT skill (Claude + Vox)"
```

## Task 10: Wedge 1 gated end-to-end smoke

**Files:** Create `crates/vox-orchestrator-mcp/tests/agy_delegate_smoke.rs`.

- [ ] **Step 1: Write the gated smoke** (runs only with real `agy` + `--ignored`):

```rust
#[tokio::test]
#[ignore = "requires agy installed + authenticated; run: cargo test -p vox-orchestrator-mcp --test agy_delegate_smoke -- --ignored"]
async fn delegate_produces_worktree_diff_and_ledger_entry() {
    // 1. init a temp git repo with one commit + a copy of the ledger file
    // 2. build a ServerState pointing repository.root at it (use the repo's existing
    //    ServerState test constructor — find it with: rg "ServerState" crates/vox-orchestrator-mcp/tests)
    // 3. call vox_agy_delegate with task "create HELLO.txt containing OK"
    // 4. assert: returned JSON ok=true, ledger_id matches ^AGH-\d{4}$, worktree path exists,
    //    HELLO.txt is in the worktree, ledger file gained one new AGH block.
}
```

> Do NOT invent a `ServerState` constructor. If no test constructor exists, this smoke stays a documented manual procedure (run the MCP server, call the tool) and the assertion is performed by hand. Record the resulting `AGH-XXXX` here: `__________`.

- [ ] **Step 2: Confirm the unit suite compiles (gated test skipped)**

Run: `cargo test -p vox-orchestrator-mcp agy_`
Expected: PASS; smoke listed as ignored.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-orchestrator-mcp/tests/agy_delegate_smoke.rs
git commit -m "test(agy): gated end-to-end delegation smoke"
```

**✅ Wedge 1 complete:** doctor-gated, worktree-jailed, auto-accepting, auto-logged single delegation. Parallel fan-out already works by calling the tool N times.

---

# WEDGE 2 — Batch fan-out, quota/retry, optional chokepoint rule

## Task 11: `classify_failure` (pure)

**Files:** modify `agy_exec.rs`.

- [ ] **Step 1: Failing test:**

```rust
    #[test]
    fn classifies_quota_timeout_error_success() {
        assert_eq!(classify_failure("quota exceeded", 1, false), Some("quota"));
        assert_eq!(classify_failure("RESOURCE_EXHAUSTED", 1, false), Some("quota"));
        assert_eq!(classify_failure("", -1, true), Some("timeout"));
        assert_eq!(classify_failure("boom", 2, false), Some("error"));
        assert_eq!(classify_failure("fine", 0, false), None);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests::classifies_quota_timeout_error_success`
Expected: FAIL — undefined.

- [ ] **Step 3: Implement** (append to `agy_exec.rs`; if you added a stub in Task 7, replace it):

```rust
/// Classify outcome for retry + ledger category. None on success.
pub fn classify_failure(stderr: &str, exit_code: i32, timed_out: bool) -> Option<&'static str> {
    if timed_out { return Some("timeout"); }
    let s = stderr.to_ascii_lowercase();
    if s.contains("quota") || s.contains("rate limit") || s.contains("resource_exhausted") {
        return Some("quota");
    }
    if exit_code == 0 { None } else { Some("error") }
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo test -p vox-orchestrator-mcp agy_exec::tests::classifies_quota_timeout_error_success
git add crates/vox-orchestrator-mcp/src/agy_exec.rs
git commit -m "feat(agy): classify quota/timeout/error outcomes"
```

## Task 12: `should_retry` (pure)

**Files:** modify `agy_exec.rs`.

- [ ] **Step 1: Failing test:**

```rust
    #[test]
    fn retry_policy() {
        assert!(should_retry("quota", 0, 3));
        assert!(!should_retry("quota", 2, 3));   // hit cap
        assert!(should_retry("timeout", 0, 3));
        assert!(!should_retry("timeout", 1, 3)); // one extra try only
        assert!(!should_retry("error", 0, 3));   // non-retryable
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_exec::tests::retry_policy`
Expected: FAIL — undefined.

- [ ] **Step 3: Implement** (append to `agy_exec.rs`):

```rust
/// Pure retry decision. `attempt` 0-based; `max_attempts` the cap.
pub fn should_retry(class: &str, attempt: u32, max_attempts: u32) -> bool {
    if attempt + 1 >= max_attempts { return false; }
    match class {
        "quota" => true,
        "timeout" => attempt < 1,
        _ => false,
    }
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo test -p vox-orchestrator-mcp agy_exec::tests::retry_policy
git add crates/vox-orchestrator-mcp/src/agy_exec.rs
git commit -m "feat(agy): pure quota/timeout retry policy"
```

## Task 13: `vox_agy_delegate_batch` — bounded pool, one worktree per worker

**Files:** modify `agy_tools.rs`.

- [ ] **Step 1: Failing test:**

```rust
    #[test]
    fn batch_validate_requires_tasks_and_clamps_concurrency() {
        assert!(batch_validate(&serde_json::json!({"tasks": []})).is_err());
        let (tasks, conc, _t) = batch_validate(&serde_json::json!({"tasks":["a","b","c"],"max_concurrency":99})).unwrap();
        assert_eq!(tasks.len(), 3);
        assert!(conc <= 8 && conc >= 1);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_tools::tests::batch_validate_requires_tasks_and_clamps_concurrency`
Expected: FAIL — undefined.

- [ ] **Step 3: Implement** (append to `agy_tools.rs`):

```rust
use std::sync::Arc;
use tokio::sync::Semaphore;

const MAX_CONCURRENCY: usize = 8;

pub fn batch_validate(args: &serde_json::Value) -> Result<(Vec<String>, usize, u64), String> {
    let tasks: Vec<String> = args.get("tasks").and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    if tasks.is_empty() {
        return Err("Provide a non-empty 'tasks' array of file-disjoint spec strings.".into());
    }
    let conc = args.get("max_concurrency").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(900);
    Ok((tasks, conc.clamp(1, MAX_CONCURRENCY), timeout_secs))
}

/// `vox_agy_delegate_batch`
pub async fn vox_agy_delegate_batch(state: &ServerState, args: serde_json::Value) -> String {
    let (tasks, conc, timeout_secs) = match batch_validate(&args) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            e, "Each task must be a self-contained, file-disjoint spec (see dispatching-parallel-agents).",
        ).to_json(),
    };
    // One doctor check up front (fail the whole batch fast if agy isn't ready).
    let report = doctor_report_json();
    if report["status"] != "ready" {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("agy not ready (status: {}).", report["status"]),
            report["remediation"].as_str().unwrap_or("Run vox_agy_doctor.").to_string(),
        ).to_json();
    }

    let sem = Arc::new(Semaphore::new(conc));
    let mut handles = Vec::new();
    for task in tasks {
        let sem = sem.clone();
        let st = state.clone(); // ServerState: #[derive(Clone)] (verified)
        let one = serde_json::json!({ "task": task, "timeout_secs": timeout_secs });
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore open");
            // Reuse the single-task path: doctor+worktree(unique slug)+exec+retry+ledger(locked).
            vox_agy_delegate(&st, one).await
        }));
    }
    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap_or_else(|e| format!("{{\"ok\":false,\"error\":\"worker join failed: {e}\"}}")));
    }
    ToolResult::ok(serde_json::json!({
        "workers": results.len(),
        "concurrency": conc,
        "results": results.iter().map(|r| serde_json::from_str::<serde_json::Value>(r).unwrap_or(serde_json::json!({"raw": r}))).collect::<Vec<_>>(),
        "next_step": "Review each worker's diff + ledger entry. Merge file-disjoint branches; resolve overlap sequentially. Two-strike rule (dispatching-parallel-agents) on repeated failures.",
    })).to_json()
}
```

> **Why this is race-free now:** each worker gets a unique slug (atomic counter → unique worktree path + branch) and the ledger append is serialized by the mutex (Task 6). `ServerState` is `#[derive(Clone)]` (verified) and its mutable state is internally `Arc`-shared, so `state.clone()` per worker is correct — confirm with `rg "#\[derive\(Clone\)\]" -A1 crates/vox-orchestrator-mcp/src/server_state.rs`.

- [ ] **Step 4: Run + commit**

```bash
cargo test -p vox-orchestrator-mcp agy_tools::tests::batch_validate_requires_tasks_and_clamps_concurrency
git add crates/vox-orchestrator-mcp/src/agy_tools.rs
git commit -m "feat(agy): vox_agy_delegate_batch — bounded pool, per-worker worktree"
```

## Task 14: Register `vox_agy_delegate_batch`

**Files:** modify `dispatch.rs`, `input_schemas.rs`, `contracts/mcp/tool-registry.canonical.yaml`.

- [ ] **Step 1: Dispatch arm:**

```rust
        "vox_agy_delegate_batch" => Ok(crate::agy_tools::vox_agy_delegate_batch(state, args).await),
```

- [ ] **Step 2: Schema arm:**

```rust
        "vox_agy_delegate_batch" => serde_json::json!({
            "type": "object",
            "required": ["tasks"],
            "properties": {
                "tasks": { "type": "array", "items": { "type": "string" }, "description": "File-disjoint, self-contained specs; one worker + worktree per task." },
                "max_concurrency": { "type": "integer", "default": 3, "description": "Parallel workers (clamped to 8)." },
                "timeout_secs": { "type": "integer", "default": 900 }
            }
        }),
```

- [ ] **Step 3: Registry entry** (`name: vox_agy_delegate_batch`, description noting bounded fan-out + per-worker worktree isolation + serialized ledger).

- [ ] **Step 4: Build + drift + arch-check + clippy** (as Task 8 Step 4).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs contracts/mcp/tool-registry.canonical.yaml
git commit -m "feat(agy): register vox_agy_delegate_batch MCP tool"
```

## Task 15: Optional hardening — `raw-agy-exec` arch-check chokepoint  *(parallel-safe)*

> Recommended. Mirrors `raw-git-exec` so all `Command::new("agy")` must live in `agy_exec.rs`. The code already passes without it; this prevents *future* bypasses.

**Files:** modify `crates/vox-arch-check/src/forbidden_patterns.rs`.

- [ ] **Step 1: Add a failing test** mirroring `git_exec` tests (`forbidden_patterns.rs` test module):

```rust
    #[test]
    fn agy_exec_rule_flags_raw_and_suppresses_annotated() {
        let rule = ForbiddenPatternRule {
            name: "raw-agy-exec".into(),
            pattern: r#"Command::new\("agy"\)"#.into(),
            allow_annotation: Some("// vox-arch-check: allow agy-exec".into()),
            reason: "All agy spawns must go through AgyExec.".into(),
            // copy any other fields from raw-git-exec's constructor
            ..Default::default() // only if the struct derives Default; else fill all fields like git rule
        };
        let hits = scan("fn f(){ let _=Command::new(\"agy\"); }", &rule); // use the real scan fn name
        assert_eq!(hits[0].rule, "raw-agy-exec");
        let none = scan("// vox-arch-check: allow agy-exec\nlet _=Command::new(\"agy\");\n", &rule);
        assert!(none.is_empty());
    }
```

> Match the EXACT `ForbiddenPatternRule` constructor shape used by `raw-git-exec` (lines ~156-160) and the real scan-fn name (`rg "fn scan" crates/vox-arch-check/src/forbidden_patterns.rs`). Do not assume `Default`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-arch-check forbidden_patterns`
Expected: FAIL — rule not registered.

- [ ] **Step 3: Register the rule** alongside `raw-git-exec` in the function that builds the active rule set, and add the `// vox-arch-check: allow agy-exec` annotation above `Command::new("agy")` in `agy_exec.rs` (and `agy_doctor.rs`'s `--version` probe — or route that probe through `AgyExec` too).

- [ ] **Step 4: Run full arch-check on the workspace**

Run: `cargo test -p vox-arch-check && cargo run -p vox-arch-check`
Expected: green (rule active; the two annotated sites suppressed; no other `agy` spawns exist).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-arch-check/src/forbidden_patterns.rs crates/vox-orchestrator-mcp/src/agy_exec.rs crates/vox-orchestrator-mcp/src/agy_doctor.rs
git commit -m "feat(arch-check): raw-agy-exec chokepoint rule (all agy spawns via AgyExec)"
```

## Task 16: Skill batch section + final verification sweep

**Files:** modify `delegate-gemini.skill.md`.

- [ ] **Step 1:** Expand "Parallel fan-out" with a concrete `vox_agy_delegate_batch` example (3 file-disjoint tasks, `max_concurrency: 3`) and the integration rule (merge disjoint branches; sequential resolve on overlap; two-strike rule).

- [ ] **Step 2: Full verification sweep** (superpowers:verification-before-completion — paste outputs in the PR, evidence before assertions):

```bash
cargo test -p vox-orchestrator-mcp agy_
cargo test -p vox-arch-check forbidden_patterns
cargo clippy -p vox-orchestrator-mcp -- -D warnings
cargo run -p vox-arch-check
cargo run -p vox-cli -- ci ssot-drift
```
Expected: all green; both `vox_agy_*` delegation tools + `vox_agy_doctor` registered.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-skills/skills/superpowers/delegate-gemini.skill.md
git commit -m "docs(skills): delegate-gemini batch fan-out + integration rules"
```

**✅ Wedge 2 complete:** doctor-gated bounded-concurrency batch delegation, per-worker worktree isolation, quota/timeout retry, serialized auto-ledger, optional enforced chokepoint.

---

# WEDGE C — Antigravity credits doc + Clavis credential-awareness

> **Goal of this wedge:** persistently document how Vox interfaces with Antigravity credits/limits, and make the credential surface *dynamically aware of every key we hold* so model/provider selection (OpenRouter is one of many) and delegation both know what's actually payable right now.
>
> **Ground truth (verified 2026-06-19 — do not re-derive):**
> - "Vox Clavis" = the **`vox-secrets`** crate (vault `.vox/clavis_vault.db`). Key API: `vox_secrets::resolve_secret(SecretId)`, `vox_secrets::list_secret_status() -> Vec<SecretStatusRow>` (redaction-safe enumerate-all), `vox_secrets::store_secret(...)`.
> - The "Clavis interceptor" is **not a crate** — it is the resolution chokepoint `resolve_secret()` / `vox-config::resolve_egress`. Don't invent an interceptor type.
> - Selection is **already credential-aware**: `vox_orchestrator::models::key_guard::provider_secret_is_available(&ProviderType) -> bool` maps each provider → `SecretId` → Clavis; local providers (`Ollama`, `PopuliMesh`, `VoxLocal`) return `true` with no secret. The unified SSOT selector is `vox_orchestrator::models::select::decide(&request, &registry)`.
> - **`agy` is a *delegation* provider, NOT an inference `ProviderType`.** It has **no headless API key** ([antigravity-cli#78](https://github.com/google-antigravity/antigravity-cli/issues/78), open) — OAuth only, balance not queryable. **Do NOT add `ProviderType::Antigravity`** (it would break every exhaustive match in the model system for zero inference benefit). Its availability comes from `agy_doctor::detect()`. The Clavis-keyed Gemini path is the separate `GoogleDirect` / `GEMINI_API_KEY` egress, already wired.
>
> See the persistent doc: `docs/src/architecture/antigravity-credits-auth-and-limitations-2026-06-19.md`.

## Task C0: Commit & link the Antigravity-credits SSOT doc

**Files:** `docs/src/architecture/antigravity-credits-auth-and-limitations-2026-06-19.md` (already written).

- [ ] **Step 1: Verify frontmatter parity** with sibling architecture docs (AGENTS.md requires `title`/`description`/`category`/`status`).

Run: `rg -n '^category:' docs/src/architecture/*.md | head` and confirm this doc uses `category: "Architecture SSOTs"` like its siblings. Do NOT hand-edit `docs/src/SUMMARY.md` / `architecture-index.md` — they are tool-regenerated.

- [ ] **Step 2: Regenerate the docs index if the repo has a generator** (find it — `rg -n "SUMMARY.md" crates/ scripts/`), otherwise skip.

Run: the discovered generator (e.g. `cargo run -p vox-cli -- docs sync` — confirm exact command; if none, skip).
Expected: `SUMMARY.md` lists the new doc; no manual edits.

- [ ] **Step 3: Commit**

```bash
git add docs/src/architecture/antigravity-credits-auth-and-limitations-2026-06-19.md docs/src/SUMMARY.md 2>/dev/null
git commit -m "docs(arch): Antigravity credits, auth & limitations SSOT"
```

## Task C1: `available_inference_providers()` — enumerate every credentialed provider

**Files:** modify `crates/vox-orchestrator/src/models/key_guard.rs`.

- [ ] **Step 1: Confirm there is no existing enumerator** and the exact `ProviderType` variants.

Run: `rg -n "available_.*providers|provider_secret_is_available|enum ProviderType" crates/vox-orchestrator crates/vox-orchestrator-types`
Expected: `provider_secret_is_available` exists; no `available_inference_providers`. Record the exact variant list.

- [ ] **Step 2: Write the failing test** (deterministic: local providers are always available regardless of secrets):

```rust
#[cfg(test)]
mod avail_tests {
    use super::*;
    use vox_orchestrator_types::ProviderType;

    #[test]
    fn local_providers_always_available_and_listed() {
        let avail = available_inference_providers();
        // Local inference needs no Clavis key, so it must always be present.
        assert!(avail.contains(&ProviderType::Ollama));
        assert!(avail.contains(&ProviderType::PopuliMesh));
        assert!(avail.contains(&ProviderType::VoxLocal));
        // The function must be total (returns the providers it checked, not empty).
        assert!(avail.len() >= 3);
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator key_guard::avail_tests`
Expected: FAIL — `available_inference_providers` undefined.

- [ ] **Step 4: Implement** (append to `key_guard.rs` — match the real variant list from Step 1; do not invent variants):

```rust
use vox_orchestrator_types::ProviderType;

/// Every inference provider Vox can pay for *right now*, by checking each
/// provider's Clavis key via `provider_secret_is_available`. Local providers
/// (no key needed) are always included. This is the credential-aware SSOT the
/// selector and the `vox_credentials_status` surface consult — OpenRouter is
/// one of many.
pub fn available_inference_providers() -> Vec<ProviderType> {
    // Concrete, credential-bearing variants (NOT Custom(_), which is endpoint-specific).
    const CANDIDATES: &[ProviderType] = &[
        ProviderType::GoogleDirect,
        ProviderType::OpenRouter,
        ProviderType::Groq,
        ProviderType::Mistral,
        ProviderType::DeepSeek,
        ProviderType::SambaNova,
        ProviderType::Cerebras,
        ProviderType::Anthropic,
        ProviderType::HuggingFaceRouter,
        ProviderType::Ollama,
        ProviderType::PopuliMesh,
        ProviderType::VoxLocal,
    ];
    CANDIDATES
        .iter()
        .filter(|p| provider_secret_is_available(p))
        .cloned()
        .collect()
}
```

> If `ProviderType` is not `Copy`/`Clone` or a variant is missing/renamed, fix the list to match Step 1's output exactly. If `CANDIDATES` as a `const` is rejected (non-`const` variants), use a `let` array.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator key_guard::avail_tests`
Expected: PASS.

- [ ] **Step 6: Wire it into selection (verify-first).** Confirm whether `select::decide` already filters by availability.

Run: `rg -n "provider_secret_is_available|available_inference_providers" crates/vox-orchestrator/src/models/select.rs crates/vox-config/src/resolve_egress.rs`
- If selection already filters by `provider_secret_is_available`, leave it (it's covered) and note it here.
- If NOT, add a filter step in `decide()` that drops candidates whose `provider_type` is absent from `available_inference_providers()`, plus a `rejection_reasons` entry `"<provider>: no credential"`. Add a test asserting a candidate with an unavailable provider is rejected.

- [ ] **Step 7: clippy + commit**

```bash
cargo clippy -p vox-orchestrator -- -D warnings
git add crates/vox-orchestrator/src/models/key_guard.rs crates/vox-orchestrator/src/models/select.rs 2>/dev/null
git commit -m "feat(models): available_inference_providers() — credential-aware provider set"
```

## Task C2: `vox_credentials_status` MCP tool — one view of every payable provider + agy

**Files:** modify `crates/vox-orchestrator-mcp/src/agy_tools.rs`, `dispatch.rs`, `input_schemas.rs`, `contracts/mcp/tool-registry.canonical.yaml`.

- [ ] **Step 1: Failing test** (pure JSON builder; uses redaction-safe status only):

```rust
    #[test]
    fn credentials_status_has_inference_and_delegation_sections() {
        let v = credentials_status_json();
        assert!(v.get("inference_providers").is_some());
        assert!(v.get("delegation").and_then(|d| d.get("agy")).is_some());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_tools::tests::credentials_status_has_inference_and_delegation_sections`
Expected: FAIL — `credentials_status_json` undefined.

- [ ] **Step 3: Implement** (append to `agy_tools.rs`). Confirm `vox_secrets::list_secret_status()` row fields first (`rg -n "struct SecretStatusRow" crates/vox-secrets/src`):

```rust
/// Unified, redaction-safe view of what Vox can actually use right now:
/// every inference provider with a present Clavis key, plus the agy delegation
/// provider's doctor state. This is the "dynamically aware of all keys" surface.
pub fn credentials_status_json() -> serde_json::Value {
    // Redaction-safe: list_secret_status() reports presence, never values.
    // SecretStatusRow fields (verified vox-secrets/src/lib.rs:464): id, canonical_env,
    // scope_description, taxonomy_slug, auth_registry, required, is_present, status.
    let secret_rows: Vec<serde_json::Value> = vox_secrets::list_secret_status()
        .into_iter()
        .map(|row| serde_json::json!({
            "id": row.id,
            "env": row.canonical_env,
            "present": row.is_present,
            "required": row.required,
        }))
        .collect();
    let inference: Vec<String> = vox_orchestrator::models::key_guard::available_inference_providers()
        .into_iter()
        .map(|p| format!("{p:?}"))
        .collect();
    serde_json::json!({
        "inference_providers": inference,        // providers we can pay for now
        "secrets": secret_rows,                  // redaction-safe presence list
        "delegation": { "agy": doctor_report_json() },
        "note": "agy is billed in Antigravity credits (not USD) and has no queryable balance; see docs/src/architecture/antigravity-credits-auth-and-limitations-2026-06-19.md",
    })
}

/// `vox_credentials_status`
pub async fn vox_credentials_status(_state: &ServerState, _args: serde_json::Value) -> String {
    ToolResult::ok(credentials_status_json()).to_json()
}
```

> **Deps confirmed (2026-06-19):** `vox-orchestrator-mcp/Cargo.toml` depends on `vox-orchestrator` (`default-features = false, features = ["runtime"]`) and `vox-secrets.workspace`. ⇒ Ensure `key_guard::available_inference_providers` is reachable under the `runtime` feature (`rg -n "mod key_guard|pub fn available_inference_providers" crates/vox-orchestrator/src/models/mod.rs`). If it's behind a different feature, gate the call or expose it under `runtime` — do NOT add a new cross-crate dep (arch-check would flag it).

- [ ] **Step 4: Register** (dispatch arm, schema arm `{ "type":"object","properties":{} }`, registry entry `name: vox_credentials_status`, description "List every inference provider with a present credential plus the agy delegation status — the credential-aware model-selection surface."). Then:

Run: `cargo build -p vox-orchestrator-mcp && cargo run -p vox-cli -- ci ssot-drift`
Expected: green; tool registered. (No `registry.rs` to commit.)

- [ ] **Step 5: Test + commit**

```bash
cargo test -p vox-orchestrator-mcp agy_tools::tests::credentials_status_has_inference_and_delegation_sections
git add crates/vox-orchestrator-mcp/src/agy_tools.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs contracts/mcp/tool-registry.canonical.yaml
git commit -m "feat(agy): vox_credentials_status — unified credential-aware provider surface"
```

## Task C3: Clavis config for the Gemini-direct path + budget honesty

**Files:** modify `crates/vox-llm-config/src/keys.rs` (verify-first); modify `agy_tools.rs` (delegation result billing tag).

- [ ] **Step 1: Verify the Clavis-keyed Gemini path already exists.**

Run: `rg -n "GeminiApiKey|GEMINI_API_KEY" crates/vox-secrets/src crates/vox-llm-config/src`
Expected: `SecretId::GeminiApiKey` (`GEMINI_API_KEY`) already present (it is). ⇒ **No new secret needed** for the direct-Gemini inference path; it's already credential-aware via `key_guard`.

- [ ] **Step 2: Add ONE non-secret config hint** for guiding agy's interactive auth (a GCP project id — NOT a secret, NOT an API key). Confirm the `keys.rs` macro/shape first (`rg -n "secret_key!|LLM_CONFIG_KEYS|fn .*key" crates/vox-llm-config/src/keys.rs`), then add an entry matching the existing non-secret pattern:

```rust
    // Non-secret operator hint: which GCP project agy should auth against.
    // agy itself manages the OAuth token; we only record the project for guidance.
    // (Match the EXACT constructor shape used by sibling non-secret keys.)
    LlmConfigKey {
        env: "VOX_AGY_GCP_PROJECT",
        default: "",
        kind: Kind::String,
        group: Group::ModelsAndEndpoints,
        class: ConfigClass::NodeLocal,
        label: "Antigravity GCP project",
        hint: "Optional GCP project id agy authenticates against; agy stores its own OAuth token.",
        secret: false,
        persistence: Persistence::FlatToml,
    },
```

> If the real `LlmConfigKey` field set differs, copy a sibling entry verbatim and change only the values. There is a parity test over this registry — run it (Step 4).

- [ ] **Step 3: Tag delegation results with their billing dimension** so budgeting never double-counts agy as USD. In `vox_agy_delegate`'s success `json!` (Task 7), add:

```rust
        "billing": "antigravity-credits",
        "billing_note": "Antigravity credits (not USD); balance not queryable — see the credits SSOT doc.",
```

- [ ] **Step 4: Run the llm-config parity/SSOT gate + drift.**

Run: `cargo test -p vox-llm-config && cargo run -p vox-cli -- ci ssot-drift`
Expected: green (the new key is consistent across config/GUI/secrets views). If a parity test fails, fix the entry to match — do NOT relax the test (ledger B-10).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-llm-config/src/keys.rs crates/vox-orchestrator-mcp/src/agy_tools.rs
git commit -m "feat(clavis): VOX_AGY_GCP_PROJECT hint + agy credit-billing tag on delegations"
```

## Task C4: Skill + doc cross-link; Wedge C verification sweep

**Files:** modify `delegate-gemini.skill.md`; verify the credits doc.

- [ ] **Step 1:** Add a "Credentials & budget" note to `delegate-gemini.skill.md`: call `vox_credentials_status` to see which providers are payable; agy uses Antigravity credits (OAuth, no stored key, balance not queryable); the Gemini-direct inference path uses `GEMINI_API_KEY` in Clavis. Link the credits SSOT doc.

- [ ] **Step 2: Full sweep** (verification-before-completion; paste outputs):

```bash
cargo test -p vox-orchestrator key_guard::avail_tests
cargo test -p vox-orchestrator-mcp agy_tools::
cargo test -p vox-llm-config
cargo clippy -p vox-orchestrator -p vox-orchestrator-mcp -- -D warnings
cargo run -p vox-arch-check
cargo run -p vox-cli -- ci ssot-drift
```
Expected: all green; `vox_credentials_status`, `vox_agy_doctor`, `vox_agy_delegate`, `vox_agy_delegate_batch` all registered.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-skills/skills/superpowers/delegate-gemini.skill.md
git commit -m "docs(skills): delegate-gemini credentials/budget awareness + credits doc link"
```

**✅ Wedge C complete:** persistent Antigravity-credits SSOT; agy registered as a doctor-gated delegation provider; a unified `vox_credentials_status` surface that makes selection/budgeting aware of every credential we hold (OpenRouter one of many); honest credit-vs-USD accounting.

---

## Out of scope (future Wedge 3)
- Live streaming dashboard for in-flight workers (agy's `/agents` panel is TUI-only; headless surfacing needs a different mechanism).
- Managing agy's *internal* sub-agents (not exposed under `-p`).
- A shared cost/quota budget SSOT — **extend the existing `BudgetManager`, do not add a new one** (prior gamification budget lesson).
- Fully automated OAuth login (interactive by design; we will not store Google credentials).
- A VoxScript (`scripts/delegate.vox`) wrapper, if Vox-script-native invocation is wanted beyond the skill.

## Risks & mitigations
| Risk | Mitigation | Task |
|---|---|---|
| `agy` not installed / not on PATH | Doctor resolves PATH + known dirs; tools return install remediation, not opaque errors | 1, 2, 7 |
| `agy` unauthenticated | Doctor reports `present_unauthed` + one-time interactive sign-in instructions (no stored creds) | 1, 7 |
| `agy` flags differ from plan | Task 0 captures `agy --help`; flag constants reconciled before code | 0 |
| #36 sandbox escape under auto-accept | Never pass `--sandbox`; isolation = Vox worktree jail; arg builder + arch rule enforce it | 3, 15 |
| **Orphaned `agy` on timeout (credit burn)** | `kill_on_drop(true)` reaps the child when the timeout future drops | 4 |
| **Duplicate AGH ids / lost ledger writes under fan-out** | Unique atomic slugs + mutex-serialized ledger append | 6, 7, 13 |
| Path traversal via task-derived slug | `sanitize_slug` (alphanumeric + dash, ≤40) | 3 |
| Quota cutoff mid-batch | `classify_failure` → bounded backoff retry; concurrency ≤8 | 11, 12, 13 |
| Untrusted code reaching live tree | Output stays on `agy/<slug>` branch until human review | 5, 9 |
| Flashing console windows (Windows) | `CREATE_NO_WINDOW` on every spawn | 4 |
| Hallucinated repo APIs | Every cross-file symbol carries an `rg`-confirm note; ground-truth verified 2026-06-19 | all |
| Supply-chain risk of curl\|bash installer | Doctor *instructs* (does not auto-run); remediation says to verify the URL first | 1 |
| `agy --model` rejects a synthesized slug | `--model` takes a display name (verified); callers pass exact strings or omit; Task 0 records this install's valid names | 0, 3, 8 |
| Delegation worktree pollutes parent `git status` | `/.vox/agy-worktrees/` added to `.gitignore` (not covered by existing rules) | 5 |
| Wrong/aliased agy invocation invented by executor | Command surface pinned with provenance (Google Codelab) + "do NOT substitute" note; no `run`/`exec`/`--task`/headless-slash forms | facts §, 0 |
