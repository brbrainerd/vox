# Antigravity Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the four-stage, merge-gated Claude-Code↔Gemini pipeline — a thin deterministic `vox_agy_pipeline` MCP tool (delegate → capture spend+diff → run gates as written → classify → write ledger → verdict report) plus an `antigravity-pipeline` protocol skill — on top of the agy delegation primitives.

**Architecture:** Hybrid (brainstorming Approach C). The deterministic tool does the verification-critical mechanical work (running the plan's gates inside the worktree jail and classifying the result, which is the net-new defence against the ledger's "green ≠ correct" failure). The in-repo skill carries the LLM-judgment work (Stage-1 authoring, Stage 3-4 correct-and-fix). Net-new code reuses `agy_exec`, `agy_worktree`, `agy_ledger`, and `ToolResult`.

**Tech Stack:** Rust (tokio process spawn with `kill_on_drop` + timeout), `serde_json`, the `vox-orchestrator-mcp` crate, the MCP tool-registry SSOT, and a Markdown skill.

**Spec:** [docs/superpowers/specs/2026-06-19-antigravity-pipeline-design.md](../specs/2026-06-19-antigravity-pipeline-design.md)

---

## File Structure

| File | Responsibility | New/Modified |
|---|---|---|
| `crates/vox-orchestrator-mcp/src/agy_gates.rs` | Gate-runner: spawn a plan-specified gate command inside the jail, capture pass/fail + output tail, with timeout + `kill_on_drop` (no pipe — avoids the cargo-orphan leak). | **New** |
| `crates/vox-orchestrator-mcp/src/agy_pipeline.rs` | `classify_outcome` (pure) + `vox_agy_pipeline` tool handler (doctor→jail→delegate→capture→gates→classify→ledger→report). | **New** |
| `crates/vox-orchestrator-mcp/src/agy_ledger.rs` | Add optional real verification block (`with_verification`). | Modify |
| `crates/vox-orchestrator-mcp/src/lib.rs` | Declare the two new modules. | Modify |
| `crates/vox-orchestrator-mcp/src/dispatch.rs` | Route `vox_agy_pipeline`. | Modify |
| `crates/vox-orchestrator-mcp/src/input_schemas.rs` | Input schema for `vox_agy_pipeline`. | Modify |
| `contracts/mcp/tool-registry.canonical.yaml` | Register `vox_agy_pipeline`. | Modify |
| `crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md` | The 4-stage protocol skill. | **New** |
| `crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs` | `#[ignore]` live end-to-end smoke. | **New** |

---

## Task 1: Gate-runner (`agy_gates.rs`)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/agy_gates.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs`

- [ ] **Step 1: Declare the module in lib.rs**

In `crates/vox-orchestrator-mcp/src/lib.rs`, find the block:

```rust
pub mod agy_doctor;
pub mod agy_exec;
pub mod agy_ledger;
pub mod agy_worktree;
pub mod agy_tools;
```

Replace it with:

```rust
pub mod agy_doctor;
pub mod agy_exec;
pub mod agy_gates;
pub mod agy_ledger;
pub mod agy_pipeline;
pub mod agy_worktree;
pub mod agy_tools;
```

- [ ] **Step 2: Write the failing test** (create `agy_gates.rs` with the test module first)

Create `crates/vox-orchestrator-mcp/src/agy_gates.rs`:

```rust
//! Runs a plan-specified verification gate (build/test/arch-check/...) inside
//! the agy worktree jail and captures a structured pass/fail. This is the
//! deterministic defence against the ledger's recurring "green gates ≠ correct
//! code": the pipeline proves the EFFECT (it compiles / tests pass) instead of
//! asserting it.
//!
//! Spawns mirror `agy_exec`: `kill_on_drop(true)` + timeout + CREATE_NO_WINDOW,
//! and NO pipe-to-head (a closed pipe orphans cargo workers on Windows).

use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Gate {
    pub name: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GateResult {
    pub name: String,
    pub passed: bool,
    pub exit_code: i32,
    pub output_tail: String,
    pub elapsed_ms: u64,
}

fn tail(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len <= n {
        return s.to_string();
    }
    s.chars().skip(len - n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_last_n_chars() {
        assert_eq!(tail("hello", 3), "llo");
        assert_eq!(tail("hi", 5), "hi");
    }

    #[tokio::test]
    async fn passing_gate_reports_pass() {
        // `git --version` is present in CI and exits 0 quickly.
        let gate = Gate { name: "probe".into(), program: "git".into(), args: vec!["--version".into()] };
        let r = run_gate(std::env::temp_dir().as_path(), &gate, 30).await;
        assert!(r.passed, "git --version should pass: {}", r.output_tail);
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.name, "probe");
    }

    #[tokio::test]
    async fn failing_gate_reports_fail() {
        let gate = Gate { name: "bad".into(), program: "git".into(), args: vec!["rev-parse".into(), "--definitely-not-a-flag".into()] };
        let r = run_gate(std::env::temp_dir().as_path(), &gate, 30).await;
        assert!(!r.passed);
        assert_ne!(r.exit_code, 0);
    }

    #[tokio::test]
    async fn missing_program_is_a_failed_gate_not_a_panic() {
        let gate = Gate { name: "nope".into(), program: "definitely-no-such-binary-xyz".into(), args: vec![] };
        let r = run_gate(std::env::temp_dir().as_path(), &gate, 30).await;
        assert!(!r.passed);
    }

    #[tokio::test]
    async fn run_gates_runs_all_in_order() {
        let gates = vec![
            Gate { name: "a".into(), program: "git".into(), args: vec!["--version".into()] },
            Gate { name: "b".into(), program: "git".into(), args: vec!["--version".into()] },
        ];
        let results = run_gates(std::env::temp_dir().as_path(), &gates, 30).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "a");
        assert_eq!(results[1].name, "b");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p vox-orchestrator-mcp agy_gates::tests`
Expected: FAIL — `cannot find function run_gate` / `run_gates` in this scope.

- [ ] **Step 4: Implement `run_gate` + `run_gates`**

In `crates/vox-orchestrator-mcp/src/agy_gates.rs`, insert this **above** the `#[cfg(test)]` line:

```rust
/// Spawn one gate inside `cwd`, capture combined stdout+stderr tail, classify
/// pass = exit 0. A spawn error or timeout is a FAILED gate (never a panic).
pub async fn run_gate(cwd: &Path, gate: &Gate, timeout_secs: u64) -> GateResult {
    let started = Instant::now();
    let mut cmd = tokio::process::Command::new(&gate.program);
    cmd.current_dir(cwd)
        .args(&gate.args)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return GateResult {
                name: gate.name.clone(),
                passed: false,
                exit_code: -1,
                output_tail: format!("gate '{}' failed to spawn '{}': {e}", gate.name, gate.program),
                elapsed_ms: started.elapsed().as_millis() as u64,
            };
        }
    };

    let dur = Duration::from_secs(timeout_secs.max(1));
    match tokio::time::timeout(dur, child.wait_with_output()).await {
        Ok(Ok(out)) => {
            let code = out.status.code().unwrap_or(-1);
            let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            GateResult {
                name: gate.name.clone(),
                passed: code == 0,
                exit_code: code,
                output_tail: tail(&combined, 2000),
                elapsed_ms: started.elapsed().as_millis() as u64,
            }
        }
        Ok(Err(e)) => GateResult {
            name: gate.name.clone(),
            passed: false,
            exit_code: -1,
            output_tail: format!("gate '{}' io error: {e}", gate.name),
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
        Err(_elapsed) => GateResult {
            name: gate.name.clone(),
            passed: false,
            exit_code: -1,
            output_tail: format!("gate '{}' exceeded {}s; process killed", gate.name, timeout_secs),
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
    }
}

/// Run gates sequentially (they often share one cargo target dir; parallel
/// cargo would contend). Returns one result per gate, in order.
pub async fn run_gates(cwd: &Path, gates: &[Gate], timeout_secs: u64) -> Vec<GateResult> {
    let mut out = Vec::with_capacity(gates.len());
    for g in gates {
        out.push(run_gate(cwd, g, timeout_secs).await);
    }
    out
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp agy_gates::tests`
Expected: PASS — 5 tests pass (`tail_keeps_last_n_chars`, `passing_gate_reports_pass`, `failing_gate_reports_fail`, `missing_program_is_a_failed_gate_not_a_panic`, `run_gates_runs_all_in_order`).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_gates.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(agy): gate-runner — run plan gates in the jail (timeout+kill_on_drop)"
```

---

## Task 2: Outcome classifier (`agy_pipeline.rs`)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/agy_pipeline.rs`

- [ ] **Step 1: Write the failing test** (create the file with classifier + test)

Create `crates/vox-orchestrator-mcp/src/agy_pipeline.rs`:

```rust
//! Stage-2 deterministic harness: the `vox_agy_pipeline` MCP tool plus the pure
//! outcome classifier. The classifier turns (files_changed, gate results,
//! timed_out) into green/partial/failed — the verdict the ledger and the human
//! merge gate consume.

use crate::agy_gates::GateResult;

/// green  = the agent changed files AND every specified gate passed.
/// partial = files changed but a gate failed, OR no gates were specified
///           (changes are unverified — never call that green).
/// failed = the agent timed out or changed nothing.
///
/// NOTE: agy's own process exit code is intentionally NOT used — it is an agent
/// wrapper whose exit code does not reliably reflect code correctness. The
/// EFFECT (files changed + gates) is the signal (ledger lesson B-9).
pub fn classify_outcome(files_changed: usize, gates: &[GateResult], timed_out: bool) -> &'static str {
    if timed_out || files_changed == 0 {
        return "failed";
    }
    if gates.is_empty() {
        return "partial";
    }
    if gates.iter().all(|g| g.passed) {
        "green"
    } else {
        "partial"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agy_gates::GateResult;

    fn gate(passed: bool) -> GateResult {
        GateResult { name: "g".into(), passed, exit_code: if passed { 0 } else { 1 }, output_tail: String::new(), elapsed_ms: 0 }
    }

    #[test]
    fn timeout_is_failed() {
        assert_eq!(classify_outcome(5, &[gate(true)], true), "failed");
    }

    #[test]
    fn no_changes_is_failed() {
        assert_eq!(classify_outcome(0, &[gate(true)], false), "failed");
    }

    #[test]
    fn changes_with_no_gates_is_partial_not_green() {
        assert_eq!(classify_outcome(3, &[], false), "partial");
    }

    #[test]
    fn changes_with_all_gates_passing_is_green() {
        assert_eq!(classify_outcome(3, &[gate(true), gate(true)], false), "green");
    }

    #[test]
    fn changes_with_a_failing_gate_is_partial() {
        assert_eq!(classify_outcome(3, &[gate(true), gate(false)], false), "partial");
    }
}
```

- [ ] **Step 2: Run the tests to verify they pass** (classifier is self-contained, so they pass immediately)

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline::tests`
Expected: PASS — 5 tests (`timeout_is_failed`, `no_changes_is_failed`, `changes_with_no_gates_is_partial_not_green`, `changes_with_all_gates_passing_is_green`, `changes_with_a_failing_gate_is_partial`).

> The module is already declared in `lib.rs` from Task 1, Step 1.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_pipeline.rs
git commit -m "feat(agy): pure outcome classifier (green/partial/failed by EFFECT)"
```

---

## Task 3: Ledger carries the real verification block

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_ledger.rs`

- [ ] **Step 1: Write the failing test**

In `crates/vox-orchestrator-mcp/src/agy_ledger.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn with_verification_overrides_the_default_block() {
        let e = LedgerEntry::new("agy-pipeline", "Do X", "green", false, 0, 2, 600, "2026-06-19")
            .with_verification("build: pass, test: pass");
        let block = render_entry("AGH-0010", &e);
        assert!(block.contains("build: pass, test: pass"));
        assert!(!block.contains("tests: \"n/a\""));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests::with_verification_overrides_the_default_block`
Expected: FAIL — `no method named with_verification found for struct LedgerEntry`.

- [ ] **Step 3: Add the optional field + builder, and wire it into render**

In `agy_ledger.rs`, find the struct:

```rust
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
```

Replace it with (adds one field):

```rust
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
    /// Real verification summary (e.g. "build: pass, test: pass"). None ⇒ the
    /// legacy "n/a" default is rendered, so existing callers are unaffected.
    pub verification: Option<String>,
}
```

In the `impl LedgerEntry` block, find the end of `new(...)` where it returns `Self { ... }`:

```rust
        Self { subsystem: subsystem.into(), task: task.into(), outcome: outcome.into(), timed_out, exit_code, files_changed, timeout_secs, date: date.into() }
    }
```

Replace it with:

```rust
        Self { subsystem: subsystem.into(), task: task.into(), outcome: outcome.into(), timed_out, exit_code, files_changed, timeout_secs, date: date.into(), verification: None }
    }

    /// Attach a real verification summary; overrides the "n/a" default in render.
    pub fn with_verification(mut self, v: impl Into<String>) -> Self {
        self.verification = Some(v.into());
        self
    }
```

In `render_entry`, find the line that builds the format string (it currently embeds `verification: {{ tests: \"n/a\", ... smoke: \"exit {code}\" }}`). Just **above** the `format!(` call, add:

```rust
    let verification = e.verification.clone().unwrap_or_else(|| {
        format!("{{ tests: \"n/a\", clippy: \"n/a\", arch_check: \"n/a\", smoke: \"exit {}\" }}", e.exit_code)
    });
```

Then in the `format!(...)` template string, replace the literal verification line:

```
verification: {{ tests: \"n/a\", clippy: \"n/a\", arch_check: \"n/a\", smoke: \"exit {code}\" }}
```

with:

```
verification: {verification}
```

and add `verification = verification,` to the `format!` argument list (and remove the now-unused `code = e.exit_code,` argument **only if** `{code}` no longer appears anywhere else in the template — it is also used in the `errors` block default; if `{code}` still appears, keep `code = e.exit_code,`).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests`
Expected: PASS — the new test plus the 3 existing ledger tests (`next_id_skips_sentinel_and_increments`, `render_is_yaml_blockish_and_mineable`, `append_roundtrip_advances_id`) all pass (the default path is byte-identical to before).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_ledger.rs
git commit -m "feat(agy): ledger entries carry a real verification block (with_verification)"
```

---

## Task 4: `vox_agy_pipeline` tool handler

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_pipeline.rs`

- [ ] **Step 1: Write the failing test** (validation + doctor-gate behaviour, no live agy)

In `agy_pipeline.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn validate_requires_task_and_defaults() {
        assert!(pipeline_validate(&serde_json::json!({})).is_err());
        let (task, model, t, gates) =
            pipeline_validate(&serde_json::json!({"task": "do X"})).unwrap();
        assert_eq!(task, "do X");
        assert!(model.is_none());
        assert_eq!(t, 900);
        assert!(gates.is_empty());
    }

    #[test]
    fn validate_parses_gates() {
        let v = serde_json::json!({
            "task": "do X",
            "gates": [{"name": "build", "program": "cargo", "args": ["build", "-p", "foo"]}]
        });
        let (_t, _m, _to, gates) = pipeline_validate(&v).unwrap();
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].name, "build");
        assert_eq!(gates[0].program, "cargo");
        assert_eq!(gates[0].args, vec!["build", "-p", "foo"]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline::tests::validate_requires_task_and_defaults`
Expected: FAIL — `cannot find function pipeline_validate`.

- [ ] **Step 3: Implement validation + the tool handler**

In `agy_pipeline.rs`, add these imports at the top (below the existing `use crate::agy_gates::GateResult;`):

```rust
use crate::agy_doctor::{detect, remediation, AgyStatus};
use crate::agy_exec::{AgyExec, AgySpec};
use crate::agy_gates::{run_gates, Gate};
use crate::agy_ledger::{append_entry_locked, LedgerEntry};
use crate::agy_worktree::DelegationWorktree;
use crate::params::ToolResult;
use crate::server_state::ServerState;
use std::sync::atomic::{AtomicU64, Ordering};
```

Then, **above** the `#[cfg(test)]` line, add:

```rust
static PIPELINE_SEQ: AtomicU64 = AtomicU64::new(1);

const REM_TASK: &str =
    "Provide a non-empty 'task' with an exact, zero-ambiguity spec, and 'gates' \
     scoped to the touched crate (e.g. cargo build -p <crate>) so the result is verified.";

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

fn fresh_slug(hint: &str) -> String {
    let n = PIPELINE_SEQ.fetch_add(1, Ordering::Relaxed);
    crate::agy_exec::sanitize_slug(&format!("p{n}-{hint}"))
}

/// (task, model, timeout_secs, gates). Gates may be empty (⇒ unverified/partial).
pub fn pipeline_validate(
    args: &serde_json::Value,
) -> Result<(String, Option<String>, u64, Vec<Gate>), String> {
    let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if task.is_empty() {
        return Err("Missing non-empty 'task'.".into());
    }
    let model = args.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
    let timeout_secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(900);
    let gates: Vec<Gate> = match args.get("gates") {
        Some(g) => serde_json::from_value(g.clone())
            .map_err(|e| format!("'gates' must be [{{name, program, args}}]: {e}"))?,
        None => Vec::new(),
    };
    Ok((task, model, timeout_secs, gates))
}

fn doctor_status_label() -> (&'static str, String) {
    match detect() {
        AgyStatus::Missing => ("missing", remediation(&AgyStatus::Missing)),
        s @ AgyStatus::PresentUnauthed { .. } => ("present_unauthed", remediation(&s)),
        s @ AgyStatus::Ready { .. } => ("ready", remediation(&s)),
    }
}

/// `vox_agy_pipeline` — Stage 2 of the Antigravity pipeline.
pub async fn vox_agy_pipeline(state: &ServerState, args: serde_json::Value) -> String {
    let (task, model, timeout_secs, gates) = match pipeline_validate(&args) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_TASK).to_json(),
    };

    // Doctor gate.
    let (label, rem) = doctor_status_label();
    if label != "ready" {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("agy not ready (status: {label})."),
            rem,
        )
        .to_json();
    }

    let repo_root = state.repository.root.clone();
    let slug = fresh_slug(&task);
    let wt = match DelegationWorktree::create(&repo_root, &slug).await {
        Ok(w) => w,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("could not create delegation worktree: {e}"),
                "Ensure the repo is a git work tree with a committed HEAD.",
            )
            .to_json()
        }
    };

    // Delegate (retry quota/timeout exactly like vox_agy_delegate).
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
    let (exit_code, timed_out, elapsed_ms) = match &out {
        Ok(o) => (o.exit_code, o.timed_out, o.elapsed_ms),
        Err(_) => (-1, false, 0),
    };

    // Capture the EFFECT: diff + gate results.
    let (diff, files_changed) = wt.capture().await.unwrap_or_else(|_| (String::new(), 0));
    let gate_results: Vec<GateResult> = run_gates(&wt.path, &gates, timeout_secs).await;
    let outcome = classify_outcome(files_changed, &gate_results, timed_out);

    // Real verification summary for the ledger.
    let gate_summary = if gate_results.is_empty() {
        "unverified (no gates specified)".to_string()
    } else {
        gate_results
            .iter()
            .map(|g| format!("{}: {}", g.name, if g.passed { "pass" } else { "fail" }))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let id = append_entry_locked(
        &repo_root,
        LedgerEntry::new(
            "agy-pipeline", &task, outcome, timed_out, exit_code, files_changed, timeout_secs, &today(),
        )
        .with_verification(gate_summary.clone()),
    )
    .await
    .unwrap_or_else(|_| "AGH-unwritten".into());

    ToolResult::ok(serde_json::json!({
        "ledger_id": id,
        "worktree": wt.path.to_string_lossy(),
        "branch": wt.branch,
        "outcome": outcome,
        "files_changed": files_changed,
        "gates": gate_results,
        "verification": gate_summary,
        "spend_proxy": {
            "elapsed_ms": elapsed_ms,
            "attempts": attempt + 1,
            "timed_out": timed_out,
            "exit_code": exit_code,
            "billing": "antigravity-credits",
            "note": "Credits are not queryable headlessly; this is a proxy, not a USD/credit balance."
        },
        "diff": diff,
        "next_step": match outcome {
            "green" => "Review the jailed diff (prove the effect), then approve the merge of `agy/<slug>` to main and set the ledger verdict.",
            "partial" => "Some gate failed or no gate ran. Review the gate output_tail; distill a correction and re-delegate ONCE (two-strike), or add scoped gates.",
            _ => "Delegation made no changes or timed out. Re-author the launch statement (smaller/atomic task) and re-delegate ONCE.",
        },
    }))
    .to_json()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline::tests`
Expected: PASS — the classifier tests from Task 2 plus `validate_requires_task_and_defaults` and `validate_parses_gates`.

- [ ] **Step 5: Verify the crate builds**

Run: `cargo build -p vox-orchestrator-mcp`
Expected: `Finished` with no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_pipeline.rs
git commit -m "feat(agy): vox_agy_pipeline handler — delegate→capture→gates→classify→ledger→report"
```

---

## Task 5: Register `vox_agy_pipeline`

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`
- Modify: `contracts/mcp/tool-registry.canonical.yaml`

- [ ] **Step 1: Route the tool in dispatch.rs**

In `crates/vox-orchestrator-mcp/src/dispatch.rs`, find:

```rust
        "vox_agy_delegate_batch" => Ok(crate::agy_tools::vox_agy_delegate_batch(state, args).await),
```

Add immediately after it:

```rust
        "vox_agy_pipeline" => Ok(crate::agy_pipeline::vox_agy_pipeline(state, args).await),
```

- [ ] **Step 2: Add the input schema**

In `crates/vox-orchestrator-mcp/src/input_schemas.rs`, find the `"vox_agy_delegate_batch" => parse_obj(...)` block and add immediately after it:

```rust
        "vox_agy_pipeline" => parse_obj(r#"{
            "type": "object",
            "required": ["task"],
            "properties": {
                "task": { "type": "string", "description": "Exact, zero-ambiguity launch statement (paths + target symbols), hardened with the ledger §B lessons." },
                "model": { "type": "string", "description": "Optional agy model DISPLAY NAME (not a slug)." },
                "timeout_secs": { "type": "integer", "default": 900, "description": "Hard kill for the agy delegation AND each gate." },
                "gates": {
                    "type": "array",
                    "description": "Verification gates run inside the jail. Scope to the touched crate (e.g. cargo build -p <crate>). Empty ⇒ outcome is 'partial' (unverified).",
                    "items": {
                        "type": "object",
                        "required": ["name", "program"],
                        "properties": {
                            "name": { "type": "string" },
                            "program": { "type": "string" },
                            "args": { "type": "array", "items": { "type": "string" } }
                        }
                    }
                }
            }
        }"#),
```

- [ ] **Step 3: Register in the canonical tool registry**

In `contracts/mcp/tool-registry.canonical.yaml`, find the `vox_agy_delegate_batch` entry and add immediately after it:

```yaml
- name: vox_agy_pipeline
  description: "Run one Antigravity delegation end-to-end with deterministic verification: delegate via agy in a worktree jail, capture the diff + spend proxy, run the plan-specified gates INSIDE the jail, classify green/partial/failed by effect, and auto-log the ledger. Returns a verdict-ready report for the human merge gate. Use this (not vox_agy_delegate) when you want the result proven, not asserted."
  product_lane: ai
  http_read_role_eligible: false
  tier: standard
```

- [ ] **Step 4: Build to verify registry + dispatch compile**

Run: `cargo build -p vox-orchestrator-mcp`
Expected: `Finished` — the `vox-mcp-registry` build script accepts the new entry (valid `product_lane: ai`).

- [ ] **Step 5: Run the registry/dispatch tests**

Run: `cargo test -p vox-orchestrator-mcp`
Expected: PASS — all tests, including any registry-parity test, pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs contracts/mcp/tool-registry.canonical.yaml
git commit -m "feat(agy): register vox_agy_pipeline (dispatch + schema + registry)"
```

---

## Task 6: The `antigravity-pipeline` skill

**Files:**
- Create: `crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md`

- [ ] **Step 1: Write the skill file**

Create `crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md`:

```markdown
---
name: antigravity-pipeline
description: Use to run a full hardened Claude-Code→Gemini delegation loop - author a verify-before-use plan, delegate it to agy with deterministic gate-checking via vox_agy_pipeline, run a two-strike correct-and-fix loop, and stop at the human merge gate. One level above delegate-gemini (which delegates a single task without the authoring + correction protocol).
---

# Antigravity Pipeline (Claude Code ↔ Gemini)

**Announce at start:** "I'm using the antigravity-pipeline skill to run the delegation loop."

You are the **architect**; Gemini (via `agy`) is the **hands**. This skill is the full loop;
`delegate-gemini` is just the single-task primitive underneath it. Merge is ALWAYS
human-gated — the pipeline never merges to main.

## Stage 1 — Author (you, Claude Code)
1. **Codebase audit (anti-hallucination).** Confirm EVERY symbol/path/API the launch
   statement references with `rg`/Grep; inline exact signatures. No "assume the API."
2. **Targeted web research** only if external/current knowledge is needed (2-3 WebFetch, not
   mass fan-out).
3. **Plan-engineer the launch statement**, baking in:
   - Ledger §B lessons (docs/superpowers/antigravity-handoff-ledger.md): verify-before-use,
     don't weaken gates, branch isolation, full delivery manifest, prove the effect.
   - Gemini limitations §5 (docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md):
     atomic green-committed tasks, self-contained tasks, one-decision-per-step,
     PARALLEL-SAFE/SEQUENTIAL tags.

## Stage 2 — Execute & verify (vox_agy_pipeline)
Call `vox_agy_pipeline` with `task` = the launch statement and `gates` scoped to the touched
crate so the result is PROVEN, not asserted. Example:

​```json
{
  "task": "Add pub fn parse_config(path:&Path)->Result<Config> to crates/vox-config/src/lib.rs (confirmed present) — no other files.",
  "gates": [
    {"name": "build", "program": "cargo", "args": ["build", "-p", "vox-config"]},
    {"name": "test",  "program": "cargo", "args": ["test",  "-p", "vox-config"]}
  ],
  "timeout_secs": 900
}
​```

The tool returns `outcome` (green/partial/failed), the `gates` results, a `spend_proxy`, the
`diff`, and the `ledger_id`. Pre-flight: if you have not confirmed `agy` is authenticated, call
`vox_agy_doctor` first and follow its remediation.

## Stage 3-4 — Correct-and-fix + report (two-strike)
- **green:** review the jailed diff yourself (prove the effect, not the shape — ledger B-9).
- **partial/failed:** read the gate `output_tail`, distill the failure into a corrected launch
  statement, append the lesson to ledger §B, and re-delegate **ONCE**. A second failure → STOP
  and hand off with a note. Never loop indefinitely (Gemini self-correction is poor).
- **Report** "to what extent implemented": which tasks landed green/partial/failed, the spend
  proxy, and the ledger trail.

## Human merge gate (always)
Present the jailed `agy/<slug>` branch + report and ask the human to approve the merge to main.
Set the ledger entry's `verdict` after the merge decision.

## Safety invariants (do not weaken)
- Never run `agy` against the live tree; the worktree jail is the only sandbox
  (`--dangerously-skip-permissions` defeats `agy --sandbox`, antigravity-cli#36).
- Never store Google credentials anywhere; `agy` owns its own OAuth token.
- Gates run exactly as specified — never substitute `--warn-only`/`|| true`/`--no-verify`.
```

> Note: the three `​```json`/`​```` fences inside the skill use a zero-width space before the
> backticks in this plan only to keep the outer code block intact. When you create the file,
> write normal triple-backtick fences with no zero-width space.

- [ ] **Step 2: Verify the skill is well-formed (frontmatter + name)**

Run: `cargo test -p vox-skills`
Expected: PASS — the SKILL.md YAML-frontmatter parser accepts the new file (name/description present).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md
git commit -m "feat(skill): antigravity-pipeline — full author→delegate→verify→correct→merge loop"
```

---

## Task 7: Live end-to-end smoke (gated)

**Files:**
- Create: `crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs`

- [ ] **Step 1: Write the gated smoke test**

Create `crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs`:

```rust
//! Live end-to-end smoke for the pipeline classifier path against a REAL agy run.
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp agy_pipeline_smoke -- --ignored
//!
//! Prereqs: `agy` authenticated; run from the repo root (git work tree, committed HEAD).

use vox_orchestrator_mcp::agy_doctor::{detect, AgyStatus};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_gates::{run_gates, Gate};
use vox_orchestrator_mcp::agy_pipeline::classify_outcome;
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn smoke_pipeline_classifies_a_real_run() {
    assert!(matches!(detect(), AgyStatus::Ready { .. }), "agy must be ready");

    let repo_root = std::env::current_dir().expect("cwd");
    let wt = DelegationWorktree::create(&repo_root, "pipe-smoke-00").await.expect("worktree");

    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec {
        task: "Create a new file .vox/pipeline-smoke.txt containing the text 'pipeline-ok'. No other files.".into(),
        model: None,
        timeout_secs: 180,
    };
    let out = exec.run(&spec).await.expect("agy spawn");
    eprintln!("exit={} timed_out={} elapsed_ms={}", out.exit_code, out.timed_out, out.elapsed_ms);

    let (_diff, files_changed) = wt.capture().await.expect("capture");
    // A trivially-true gate proves the gate-runner path end-to-end.
    let gates = vec![Gate { name: "probe".into(), program: "git".into(), args: vec!["--version".into()] }];
    let results = run_gates(&wt.path, &gates, 60).await;

    let outcome = classify_outcome(files_changed, &results, out.timed_out);
    eprintln!("files_changed={files_changed} outcome={outcome}");
    assert_eq!(outcome, "green", "expected a verified green run");

    wt.cleanup(&repo_root).await.expect("cleanup");
}
```

- [ ] **Step 2: Verify it compiles and is correctly ignored**

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline_smoke`
Expected: PASS with `1 ignored` (the test compiles but does not run without `--ignored`).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs
git commit -m "test(agy): gated live end-to-end pipeline smoke"
```

---

## Task 8: Full verification sweep

**Files:** none (verification only)

- [ ] **Step 1: Run the full crate test suite**

Run: `cargo test -p vox-orchestrator-mcp`
Expected: PASS — all tests including `agy_gates::tests` (5), `agy_pipeline::tests` (7), `agy_ledger::tests` (4), and the existing suite; smoke tests show as `ignored`.

- [ ] **Step 2: Run arch-check (full output to a file — never pipe cargo to head/grep on Windows)**

Run: `cargo run -p vox-arch-check -- > arch-check.out 2>&1; echo "exit $?"`
Then read `arch-check.out`.
Expected: exit 0; no new forbidden-pattern violations (the `raw-agy-exec` rule is satisfied — all `agy` spawns go through `AgyExec`; the gate-runner spawns `cargo`/`git`, which are not guarded).

- [ ] **Step 3: Run ssot-drift to confirm the registry is consistent**

Run: `cargo run -p vox-cli -- ci ssot-drift > ssot.out 2>&1; echo "exit $?"`
Then read `ssot.out`.
Expected: no NEW drift attributable to `vox_agy_pipeline` (a pre-existing unrelated `query-all-guard` failure on `crates/vox-gui/src/commands/activity.rs` is a known baseline — do not fix it here, just note it).

- [ ] **Step 4: Clean up scratch files and commit if anything is outstanding**

```bash
rm -f arch-check.out ssot.out
git status --short
```
Expected: clean working tree (all task commits already made).

---

## Self-Review (completed by plan author)

**Spec coverage:**
- Stage 1 authoring → Task 6 (skill encodes the discipline). ✓
- Stage 2 `vox_agy_pipeline` (doctor→jail→delegate→capture→**run gates**→classify→ledger→report) → Tasks 1-5. ✓
- Stage 3-4 correct-and-fix + report + human merge gate → Task 6 (skill) + the handler's `next_step`. ✓
- "Gate-execution moves into the tool" (kill hollow-green) → Tasks 1, 2, 4. ✓
- Ledger carries real verification → Task 3. ✓
- Error handling (timeout/quota/no-change) → `classify_outcome` + retry loop (Tasks 2, 4). ✓
- Testing (unit classifier, gate-runner, `#[ignore]` live smoke) → Tasks 1, 2, 4, 7. ✓
- Merge-gate only / never auto-merge → handler returns a report and stops; skill states it. ✓
- Spend proxy (credits not queryable) → handler `spend_proxy` block. ✓

**Placeholder scan:** no TBD/TODO; every code step has complete code and an exact run command. ✓

**Type consistency:** `Gate{name,program,args}`, `GateResult{name,passed,exit_code,output_tail,elapsed_ms}`, `classify_outcome(usize,&[GateResult],bool)`, `LedgerEntry::with_verification`, and `pipeline_validate -> (String, Option<String>, u64, Vec<Gate>)` are used identically across Tasks 1, 2, 4, 7. ✓
