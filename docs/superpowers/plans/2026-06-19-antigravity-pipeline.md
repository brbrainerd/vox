# Antigravity Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **Execute INLINE** in this repo (subagents are read-only in this sandbox).

**Goal:** Build the four-stage, merge-gated Claude-Code↔Gemini pipeline — a deterministic `vox_agy_pipeline` tool (delegate → capture spend+diff → run gates in the jail → classify → provisional ledger → report), an automated Claude-side adversarial review recorded as a ledger review-addendum (`vox_agy_review`), a flywheel digest that feeds historical Gemini failures into the next launch statement (`vox_agy_ledger_digest`), the `antigravity-pipeline` skill, and the full process-skill set ported in-repo.

**Architecture:** Hybrid (brainstorming Approach C). Deterministic tools do verification-critical mechanics (running the plan's gates inside the worktree jail, classifying by effect, recording the ledger); the in-repo skill carries LLM judgment (authoring, adversarial review, correction). Reuses `agy_exec`, `agy_worktree`, `agy_ledger`, `ToolResult`, and the `superpowers:code-reviewer` agent.

**Tech Stack:** Rust (tokio process spawn, `kill_on_drop`+timeout), `serde_json`, `vox-orchestrator-mcp`, the MCP tool-registry SSOT, Markdown skills.

**Spec:** [docs/superpowers/specs/2026-06-19-antigravity-pipeline-design.md](../specs/2026-06-19-antigravity-pipeline-design.md)

---

## File Structure

| File | Responsibility | New/Modified |
|---|---|---|
| `crates/vox-orchestrator-mcp/src/agy_gates.rs` | Gate-runner: spawn a plan gate in the jail (timeout+kill_on_drop+optional env), structured pass/fail. | **New** |
| `crates/vox-orchestrator-mcp/src/agy_pipeline.rs` | `classify_outcome` (pure), `vox_agy_pipeline`, `vox_agy_review`, `vox_agy_ledger_digest`. | **New** |
| `crates/vox-orchestrator-mcp/src/agy_ledger.rs` | `with_verification`, `ReviewRecord`+`append_review_locked`, `ledger_digest`. | Modify |
| `crates/vox-orchestrator-mcp/src/lib.rs` | Declare new modules. | Modify |
| `crates/vox-orchestrator-mcp/src/dispatch.rs` | Route the three new tools. | Modify |
| `crates/vox-orchestrator-mcp/src/input_schemas.rs` | Schemas for the three new tools. | Modify |
| `contracts/mcp/tool-registry.canonical.yaml` | Register the three new tools. | Modify |
| `crates/vox-skills/skills/superpowers/{writing-plans,executing-plans,subagent-driven-development,test-driven-development}.skill.md` | Port process skills in-repo so all agents (incl. Antigravity) mount them. | **New** |
| `crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md` | The 4-stage protocol skill. | **New** |
| `crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs` | `#[ignore]` live end-to-end smoke. | **New** |

---

## Task 1: Gate-runner (`agy_gates.rs`)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/agy_gates.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs`

- [ ] **Step 1: Declare the modules in lib.rs**

In `crates/vox-orchestrator-mcp/src/lib.rs`, find:

```rust
pub mod agy_doctor;
pub mod agy_exec;
pub mod agy_ledger;
pub mod agy_worktree;
pub mod agy_tools;
```

Replace with:

```rust
pub mod agy_doctor;
pub mod agy_exec;
pub mod agy_gates;
pub mod agy_ledger;
pub mod agy_pipeline;
pub mod agy_worktree;
pub mod agy_tools;
```

- [ ] **Step 2: Write the file with the failing tests**

Create `crates/vox-orchestrator-mcp/src/agy_gates.rs`:

```rust
//! Runs a plan-specified verification gate (build/test/arch-check/...) inside
//! the agy worktree jail and captures a structured pass/fail. This is the
//! deterministic defence against the ledger's "green gates ≠ correct code":
//! the pipeline proves the EFFECT instead of asserting it.
//!
//! Spawns mirror `agy_exec`: `kill_on_drop(true)` + timeout + CREATE_NO_WINDOW,
//! and NO pipe-to-head (a closed pipe orphans cargo workers on Windows).
//! `env` lets cargo gates set CARGO_TARGET_DIR to the main repo's target so a
//! worktree build reuses the cache instead of a cold rebuild.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Gate {
    pub name: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
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
        let gate = Gate { name: "probe".into(), program: "git".into(), args: vec!["--version".into()], ..Default::default() };
        let r = run_gate(std::env::temp_dir().as_path(), &gate, 30).await;
        assert!(r.passed, "git --version should pass: {}", r.output_tail);
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.name, "probe");
    }

    #[tokio::test]
    async fn failing_gate_reports_fail() {
        let gate = Gate { name: "bad".into(), program: "git".into(), args: vec!["rev-parse".into(), "--definitely-not-a-flag".into()], ..Default::default() };
        let r = run_gate(std::env::temp_dir().as_path(), &gate, 30).await;
        assert!(!r.passed);
        assert_ne!(r.exit_code, 0);
    }

    #[tokio::test]
    async fn missing_program_is_a_failed_gate_not_a_panic() {
        let gate = Gate { name: "nope".into(), program: "definitely-no-such-binary-xyz".into(), ..Default::default() };
        let r = run_gate(std::env::temp_dir().as_path(), &gate, 30).await;
        assert!(!r.passed);
    }

    #[tokio::test]
    async fn run_gates_runs_all_in_order() {
        let g = |n: &str| Gate { name: n.into(), program: "git".into(), args: vec!["--version".into()], ..Default::default() };
        let results = run_gates(std::env::temp_dir().as_path(), &[g("a"), g("b")], 30).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "a");
        assert_eq!(results[1].name, "b");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p vox-orchestrator-mcp agy_gates::tests`
Expected: FAIL — `cannot find function run_gate` / `run_gates`.

- [ ] **Step 4: Implement `run_gate` + `run_gates`** (insert above the `#[cfg(test)]` line)

```rust
/// Spawn one gate inside `cwd`; pass = exit 0. A spawn error or timeout is a
/// FAILED gate (never a panic).
pub async fn run_gate(cwd: &Path, gate: &Gate, timeout_secs: u64) -> GateResult {
    let started = Instant::now();
    let mut cmd = tokio::process::Command::new(&gate.program);
    cmd.current_dir(cwd)
        .args(&gate.args)
        .envs(&gate.env)
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
/// cargo would contend). One result per gate, in order.
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
Expected: PASS — 5 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_gates.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(agy): gate-runner — run plan gates in the jail (timeout+kill_on_drop+env)"
```

---

## Task 2: Outcome classifier (`agy_pipeline.rs`)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/agy_pipeline.rs`

- [ ] **Step 1: Write the file with classifier + tests**

Create `crates/vox-orchestrator-mcp/src/agy_pipeline.rs`:

```rust
//! Stage-2/3 deterministic harness: the pure outcome classifier plus the
//! vox_agy_pipeline / vox_agy_review / vox_agy_ledger_digest tools.

use crate::agy_gates::GateResult;

/// green   = files changed AND every specified gate passed.
/// partial = files changed but a gate failed, OR no gates specified (unverified).
/// failed  = timed out or no files changed.
///
/// agy's own exit code is intentionally NOT used — it's an agent wrapper whose
/// exit code doesn't reliably reflect correctness; the EFFECT is the signal (B-9).
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

- [ ] **Step 2: Run the tests to verify they pass** (module declared in Task 1, Step 1)

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline::tests`
Expected: PASS — 5 tests.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_pipeline.rs
git commit -m "feat(agy): pure outcome classifier (green/partial/failed by EFFECT)"
```

---

## Task 3: Ledger carries a real verification block

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_ledger.rs`

- [ ] **Step 1: Write the failing test** (inside `mod tests`)

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
Expected: FAIL — `no method named with_verification`.

- [ ] **Step 3: Add the field + builder + render wiring**

In the `LedgerEntry` struct, add a field after `pub date: String,`:

```rust
    /// Real verification summary (e.g. "build: pass, test: pass"). None ⇒ the
    /// legacy "n/a" default is rendered, so existing callers are unaffected.
    pub verification: Option<String>,
```

In `new(...)`, change the returned `Self { ... }` to append `, verification: None` before the closing brace, and add this method right after `new`:

```rust
    /// Attach a real verification summary; overrides the "n/a" default in render.
    pub fn with_verification(mut self, v: impl Into<String>) -> Self {
        self.verification = Some(v.into());
        self
    }
```

In `render_entry`, just above the `format!(` call add:

```rust
    let verification = e.verification.clone().unwrap_or_else(|| {
        format!("{{ tests: \"n/a\", clippy: \"n/a\", arch_check: \"n/a\", smoke: \"exit {}\" }}", e.exit_code)
    });
```

Then in the `format!` template replace the literal verification line:

```
verification: {{ tests: \"n/a\", clippy: \"n/a\", arch_check: \"n/a\", smoke: \"exit {code}\" }}
```

with `verification: {verification}` and add `verification = verification,` to the argument list (keep `code = e.exit_code,` only if `{code}` still appears elsewhere in the template — it does, in the `errors` default block, so keep it).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests`
Expected: PASS — new test + the 3 existing ledger tests (default path byte-identical).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_ledger.rs
git commit -m "feat(agy): ledger entries carry a real verification block (with_verification)"
```

---

## Task 4: Ledger review-addendum (`append_review_locked`)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_ledger.rs`

- [ ] **Step 1: Write the failing test** (inside `mod tests`)

```rust
    #[tokio::test]
    async fn append_review_writes_keyed_addendum() {
        let dir = std::env::temp_dir().join(format!("agyrev-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs/superpowers")).unwrap();
        let p = dir.join(LEDGER_REL);
        std::fs::write(&p, "## §C\n# --- AGH-0007 ---\nid: AGH-0007\n").unwrap();
        let rec = ReviewRecord {
            verdict: "request-changes".into(),
            categories: vec!["hallucinated-api".into(), "scope-creep".into()],
            findings: "Invented useQuery(api.x) with no import".into(),
            lessons: vec!["Verify framework primitives in-repo".into()],
            date: "2026-06-19".into(),
        };
        append_review_locked(&dir, "AGH-0007", &rec).await.unwrap();
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("# --- AGH-0007-review ---"));
        assert!(body.contains("verdict: request-changes"));
        assert!(body.contains("hallucinated-api"));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests::append_review_writes_keyed_addendum`
Expected: FAIL — `cannot find type ReviewRecord` / `function append_review_locked`.

- [ ] **Step 3: Implement `ReviewRecord`, `render_review`, `append_review_locked`** (add near `append_entry_locked`)

```rust
/// The Claude-side adversarial review outcome for one handoff.
#[derive(Debug, Clone)]
pub struct ReviewRecord {
    pub verdict: String,         // approve | approve-with-followups | request-changes
    pub categories: Vec<String>, // from the stable §B vocabulary
    pub findings: String,
    pub lessons: Vec<String>,
    pub date: String,
}

fn yaml_inline(s: &str) -> String {
    s.replace('"', "'").replace(['\n', '\r'], " ")
}

pub fn render_review(id: &str, r: &ReviewRecord) -> String {
    let cats = r.categories.iter().map(|c| yaml_inline(c)).collect::<Vec<_>>().join(", ");
    let lessons = if r.lessons.is_empty() {
        "  []".to_string()
    } else {
        r.lessons.iter().map(|l| format!("  - \"{}\"", yaml_inline(l))).collect::<Vec<_>>().join("\n")
    };
    format!(
        "```yaml\n# --- {id}-review ---\nreview_of: {id}\ndate: {date}\nverdict: {verdict}\ncategories: [{cats}]\nreview_findings: \"{findings}\"\nprompt_lessons:\n{lessons}\n```\n",
        id = id, date = r.date, verdict = yaml_inline(&r.verdict), cats = cats,
        findings = yaml_inline(&r.findings), lessons = lessons,
    )
}

/// Append a `{id}-review` addendum under the same lock (append-only).
pub async fn append_review_locked(repo_root: &Path, id: &str, r: &ReviewRecord) -> std::io::Result<()> {
    let _guard = LEDGER_LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
    let path = repo_root.join(LEDGER_REL);
    let mut body = std::fs::read_to_string(&path)?;
    if !body.ends_with('\n') {
        body.push('\n');
    }
    body.push('\n');
    body.push_str(&render_review(id, r));
    std::fs::write(&path, body)?;
    Ok(())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests`
Expected: PASS — new test + all prior ledger tests.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_ledger.rs
git commit -m "feat(agy): append-only {id}-review ledger addendum (verdict/categories/lessons)"
```

---

## Task 5: Flywheel digest (`ledger_digest`)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_ledger.rs`

- [ ] **Step 1: Write the failing test** (inside `mod tests`)

```rust
    #[test]
    fn digest_counts_categories_and_entries() {
        let body = "\
# --- AGH-NNNN ---
# --- AGH-0001 ---
id: AGH-0001
errors_encountered:
  - { what: x, root_cause: y, category: \"hallucinated-api\", who: agent }
# --- AGH-0001-review ---
verdict: request-changes
categories: [hallucinated-api, scope-creep]
# --- AGH-0002 ---
id: AGH-0002
";
        let d = digest_from_body(body);
        assert_eq!(d.total_entries, 2); // AGH-0001 + AGH-0002 (sentinel + -review excluded)
        assert_eq!(*d.category_counts.get("hallucinated-api").unwrap(), 2);
        assert_eq!(*d.category_counts.get("scope-creep").unwrap(), 1);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests::digest_counts_categories_and_entries`
Expected: FAIL — `cannot find function digest_from_body` / type `LedgerDigest`.

- [ ] **Step 3: Implement the digest** (add near the other ledger functions)

```rust
use std::collections::BTreeMap;

/// Stable §B + exec category vocabulary the digest tallies.
pub const KNOWN_CATEGORIES: &[&str] = &[
    "hallucinated-api", "wrong-path", "wrong-crate", "arch-check-gate", "fmt-gate",
    "build-gate", "branch-hygiene", "scope-creep", "already-done", "perf", "robustness",
    "test-hygiene", "unplanned-shared-change", "ssot-fork", "unit-mismatch",
    "timeout", "quota", "error",
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct LedgerDigest {
    pub total_entries: usize,
    pub category_counts: BTreeMap<String, usize>,
}

/// Pure: tally entries + category frequencies from raw ledger text.
pub fn digest_from_body(body: &str) -> LedgerDigest {
    let mut total = 0usize;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("# --- AGH-") && !t.contains("-review") && !t.contains("AGH-NNNN") {
            total += 1;
        }
        if t.contains("categor") {
            for cat in KNOWN_CATEGORIES {
                if t.contains(cat) {
                    *counts.entry((*cat).to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    LedgerDigest { total_entries: total, category_counts: counts }
}

/// Read the on-disk ledger and digest it.
pub fn ledger_digest(repo_root: &Path) -> std::io::Result<LedgerDigest> {
    let body = std::fs::read_to_string(repo_root.join(LEDGER_REL))?;
    Ok(digest_from_body(&body))
}
```

> If `use std::collections::BTreeMap;` already exists at the top of the file, do not add a second one — keep one import.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-orchestrator-mcp agy_ledger::tests`
Expected: PASS — digest test + all prior ledger tests.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_ledger.rs
git commit -m "feat(agy): ledger_digest — failure-category flywheel input for Stage 1"
```

---

## Task 6: `vox_agy_pipeline` tool handler

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_pipeline.rs`

- [ ] **Step 1: Write the failing test** (inside `mod tests`)

```rust
    #[test]
    fn pipeline_validate_requires_task_and_parses_gates() {
        assert!(pipeline_validate(&serde_json::json!({})).is_err());
        let (task, model, t, gates) = pipeline_validate(&serde_json::json!({
            "task": "do X",
            "gates": [{"name": "build", "program": "cargo", "args": ["build", "-p", "foo"]}]
        })).unwrap();
        assert_eq!(task, "do X");
        assert!(model.is_none());
        assert_eq!(t, 900);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].program, "cargo");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline::tests::pipeline_validate_requires_task_and_parses_gates`
Expected: FAIL — `cannot find function pipeline_validate`.

- [ ] **Step 3: Add imports + implement the handler** (top-of-file imports below the existing `use crate::agy_gates::GateResult;`)

```rust
use crate::agy_doctor::{detect, remediation, AgyStatus};
use crate::agy_exec::{AgyExec, AgySpec};
use crate::agy_gates::{run_gates, Gate};
use crate::agy_ledger::{append_entry_locked, append_review_locked, ledger_digest, LedgerEntry, ReviewRecord};
use crate::params::ToolResult;
use crate::server_state::ServerState;
use std::sync::atomic::{AtomicU64, Ordering};
```

Above the `#[cfg(test)]` line, add:

```rust
static PIPELINE_SEQ: AtomicU64 = AtomicU64::new(1);

const REM_TASK: &str =
    "Provide a non-empty 'task' with an exact, zero-ambiguity spec, and 'gates' \
     scoped to the touched crate (e.g. cargo build -p <crate>, with env CARGO_TARGET_DIR \
     set to the main target) so the result is verified.";

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

fn fresh_slug(hint: &str) -> String {
    let n = PIPELINE_SEQ.fetch_add(1, Ordering::Relaxed);
    crate::agy_exec::sanitize_slug(&format!("p{n}-{hint}"))
}

fn doctor_status_label() -> (&'static str, String) {
    match detect() {
        AgyStatus::Missing => ("missing", remediation(&AgyStatus::Missing)),
        s @ AgyStatus::PresentUnauthed { .. } => ("present_unauthed", remediation(&s)),
        s @ AgyStatus::Ready { .. } => ("ready", remediation(&s)),
    }
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
            .map_err(|e| format!("'gates' must be [{{name, program, args, env}}]: {e}"))?,
        None => Vec::new(),
    };
    Ok((task, model, timeout_secs, gates))
}

/// `vox_agy_pipeline` — Stage 2.
pub async fn vox_agy_pipeline(state: &ServerState, args: serde_json::Value) -> String {
    let (task, model, timeout_secs, gates) = match pipeline_validate(&args) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_TASK).to_json(),
    };

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
    let wt = match crate::agy_worktree::DelegationWorktree::create(&repo_root, &slug).await {
        Ok(w) => w,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("could not create delegation worktree: {e}"),
                "Ensure the repo is a git work tree with a committed HEAD.",
            )
            .to_json()
        }
    };

    // Delegate with quota/timeout retry (same policy as vox_agy_delegate).
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

    // Capture the EFFECT.
    let (diff, files_changed) = wt.capture().await.unwrap_or_else(|_| (String::new(), 0));
    let gate_results: Vec<GateResult> = run_gates(&wt.path, &gates, timeout_secs).await;
    let outcome = classify_outcome(files_changed, &gate_results, timed_out);

    let gate_summary = if gate_results.is_empty() {
        "unverified (no gates specified)".to_string()
    } else {
        gate_results
            .iter()
            .map(|g| format!("{}: {}", g.name, if g.passed { "pass" } else { "fail" }))
            .collect::<Vec<_>>()
            .join(", ")
    };

    // PROVISIONAL ledger entry (verdict pending; render already sets request-changes
    // + "pending human review"). Recorded even on failure for the flywheel.
    let id = append_entry_locked(
        &repo_root,
        LedgerEntry::new(
            "agy-pipeline", &task, outcome, timed_out, exit_code, files_changed, timeout_secs, &today(),
        )
        .with_verification(gate_summary.clone()),
    )
    .await
    .unwrap_or_else(|_| "AGH-unwritten".into());

    // Cleanup the jail only when nothing was produced (no dead worktrees pile up).
    if files_changed == 0 {
        let _ = wt.cleanup(&repo_root).await;
    }

    ToolResult::ok(serde_json::json!({
        "ledger_id": id,
        "worktree": if files_changed == 0 { String::new() } else { wt.path.to_string_lossy().to_string() },
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
            "note": "Credits are not queryable headlessly; this is a proxy, not a balance."
        },
        "diff": diff,
        "next_step": match outcome {
            "green" => "Run the Stage-3 adversarial review (code-reviewer agent vs the jailed diff), record it with vox_agy_review, then take the jailed branch to the human merge gate.",
            "partial" => "A gate failed or no gate ran. Review the gate output_tail; distill a correction and re-delegate ONCE (two-strike), or add scoped gates.",
            _ => "No changes / timeout (jail cleaned). Re-author a smaller atomic launch statement and re-delegate ONCE.",
        },
    }))
    .to_json()
}
```

- [ ] **Step 4: Run the tests + build**

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline::tests` then `cargo build -p vox-orchestrator-mcp`
Expected: tests PASS; build `Finished`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_pipeline.rs
git commit -m "feat(agy): vox_agy_pipeline — delegate→capture→gates→classify→provisional ledger→report"
```

---

## Task 7: `vox_agy_review` + `vox_agy_ledger_digest` tools

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/agy_pipeline.rs`

- [ ] **Step 1: Write the failing test** (inside `mod tests`)

```rust
    #[test]
    fn review_validate_requires_id_and_verdict() {
        assert!(review_validate(&serde_json::json!({"verdict": "approve"})).is_err()); // no id
        assert!(review_validate(&serde_json::json!({"ledger_id": "AGH-0007"})).is_err()); // no verdict
        let (id, rec) = review_validate(&serde_json::json!({
            "ledger_id": "AGH-0007",
            "verdict": "request-changes",
            "categories": ["hallucinated-api"],
            "findings": "no import emitted",
            "lessons": ["verify framework primitive"]
        })).unwrap();
        assert_eq!(id, "AGH-0007");
        assert_eq!(rec.verdict, "request-changes");
        assert_eq!(rec.categories, vec!["hallucinated-api"]);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline::tests::review_validate_requires_id_and_verdict`
Expected: FAIL — `cannot find function review_validate`.

- [ ] **Step 3: Implement validation + both tool handlers** (above the `#[cfg(test)]` line)

```rust
/// (ledger_id, ReviewRecord) from a vox_agy_review call.
pub fn review_validate(args: &serde_json::Value) -> Result<(String, ReviewRecord), String> {
    let id = args.get("ledger_id").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if id.is_empty() {
        return Err("Missing 'ledger_id' (the AGH id returned by vox_agy_pipeline).".into());
    }
    let verdict = args.get("verdict").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if verdict.is_empty() {
        return Err("Missing 'verdict' (approve | approve-with-followups | request-changes).".into());
    }
    let str_list = |key: &str| -> Vec<String> {
        args.get(key)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default()
    };
    let rec = ReviewRecord {
        verdict,
        categories: str_list("categories"),
        findings: args.get("findings").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        lessons: str_list("lessons"),
        date: today(),
    };
    Ok((id, rec))
}

/// `vox_agy_review` — Stage 3: record the Claude-side adversarial review.
pub async fn vox_agy_review(state: &ServerState, args: serde_json::Value) -> String {
    let (id, rec) = match review_validate(&args) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e,
                "Run the code-reviewer agent vs the jailed diff first; pass its verdict + §B categories here.",
            )
            .to_json()
        }
    };
    match append_review_locked(&state.repository.root, &id, &rec).await {
        Ok(()) => ToolResult::ok(serde_json::json!({
            "recorded": format!("{id}-review"),
            "verdict": rec.verdict,
            "categories": rec.categories,
            "next_step": "Take the jailed branch to the human merge gate. The flywheel will surface these categories via vox_agy_ledger_digest.",
        }))
        .to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
            format!("could not append review addendum: {e}"),
            "Confirm docs/superpowers/antigravity-handoff-ledger.md exists and is writable.",
        )
        .to_json(),
    }
}

/// `vox_agy_ledger_digest` — Stage 1 flywheel input.
pub async fn vox_agy_ledger_digest(state: &ServerState, _args: serde_json::Value) -> String {
    match ledger_digest(&state.repository.root) {
        Ok(d) => ToolResult::ok(serde_json::json!({
            "total_entries": d.total_entries,
            "category_counts": d.category_counts,
            "guidance": "Inject the top recurring categories as explicit avoid-rules into the next launch statement (Stage 1); read §B of the ledger for the matching lessons.",
        }))
        .to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
            format!("could not read ledger: {e}"),
            "Confirm docs/superpowers/antigravity-handoff-ledger.md exists.",
        )
        .to_json(),
    }
}
```

- [ ] **Step 4: Run the tests + build**

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline::tests` then `cargo build -p vox-orchestrator-mcp`
Expected: tests PASS; build `Finished`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/agy_pipeline.rs
git commit -m "feat(agy): vox_agy_review (record adversarial review) + vox_agy_ledger_digest (flywheel)"
```

---

## Task 8: Register the three tools

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`
- Modify: `contracts/mcp/tool-registry.canonical.yaml`

- [ ] **Step 1: Route in dispatch.rs** (after the `"vox_agy_delegate_batch" => ...` arm)

```rust
        "vox_agy_pipeline" => Ok(crate::agy_pipeline::vox_agy_pipeline(state, args).await),
        "vox_agy_review" => Ok(crate::agy_pipeline::vox_agy_review(state, args).await),
        "vox_agy_ledger_digest" => Ok(crate::agy_pipeline::vox_agy_ledger_digest(state, args).await),
```

- [ ] **Step 2: Add `vox_agy_ledger_digest` to the no-arg schema group**

In `input_schemas.rs`, find the group ending `| "vox_credentials_status" => {` and add `vox_agy_ledger_digest` to it:

```rust
        "vox_gui_components" | "vox_gui_tokens" | "vox_gui_rules" | "vox_agy_doctor"
        | "vox_credentials_status" | "vox_agy_ledger_digest" => {
            parse_obj(r#"{"type":"object","additionalProperties":false}"#)
        }
```

- [ ] **Step 3: Add schemas for `vox_agy_pipeline` and `vox_agy_review`** (after the `"vox_agy_delegate_batch" => parse_obj(...)` block)

```rust
        "vox_agy_pipeline" => parse_obj(r#"{
            "type": "object",
            "required": ["task"],
            "properties": {
                "task": { "type": "string", "description": "Exact, zero-ambiguity launch statement (paths + target symbols), hardened with ledger §B lessons + the digest's top categories." },
                "model": { "type": "string", "description": "Optional agy model DISPLAY NAME (not a slug)." },
                "timeout_secs": { "type": "integer", "default": 900, "description": "Hard kill for the agy delegation AND each gate." },
                "gates": {
                    "type": "array",
                    "description": "Verification gates run inside the jail. Scope to the touched crate and set env.CARGO_TARGET_DIR to the main target. Empty ⇒ outcome 'partial' (unverified).",
                    "items": {
                        "type": "object",
                        "required": ["name", "program"],
                        "properties": {
                            "name": { "type": "string" },
                            "program": { "type": "string" },
                            "args": { "type": "array", "items": { "type": "string" } },
                            "env": { "type": "object", "additionalProperties": { "type": "string" } }
                        }
                    }
                }
            }
        }"#),
        "vox_agy_review" => parse_obj(r#"{
            "type": "object",
            "required": ["ledger_id", "verdict"],
            "properties": {
                "ledger_id": { "type": "string", "description": "The AGH id returned by vox_agy_pipeline." },
                "verdict": { "type": "string", "enum": ["approve", "approve-with-followups", "request-changes"] },
                "categories": { "type": "array", "items": { "type": "string" }, "description": "Stable §B vocabulary, e.g. hallucinated-api, scope-creep." },
                "findings": { "type": "string" },
                "lessons": { "type": "array", "items": { "type": "string" }, "description": "1-3 prompt-hardening lessons fed to the flywheel." }
            }
        }"#),
```

- [ ] **Step 4: Register in `contracts/mcp/tool-registry.canonical.yaml`** (after the `vox_agy_delegate_batch` entry)

```yaml
- name: vox_agy_pipeline
  description: "Run one Antigravity delegation end-to-end with deterministic verification: delegate via agy in a worktree jail, capture diff + spend proxy, run the plan-specified gates INSIDE the jail, classify green/partial/failed by effect, write a provisional ledger entry, and return a verdict-ready report for the human merge gate. Use this (not vox_agy_delegate) when you want the result proven, not asserted."
  product_lane: ai
  http_read_role_eligible: false
  tier: standard
- name: vox_agy_review
  description: "Record the Claude-side adversarial code review of a delegation as an append-only {id}-review ledger addendum (verdict + §B failure categories + findings + prompt lessons). Closes the review phase before the human merge gate and feeds the flywheel."
  product_lane: ai
  http_read_role_eligible: false
  tier: standard
- name: vox_agy_ledger_digest
  description: "Read the handoff ledger and return historical Gemini failure-category frequencies + entry count — the flywheel input injected as explicit avoid-rules into the next launch statement (Stage 1)."
  product_lane: ai
  http_read_role_eligible: true
  tier: standard
```

- [ ] **Step 5: Build + full test + arch-check**

Run: `cargo build -p vox-orchestrator-mcp` then `cargo test -p vox-orchestrator-mcp`
Expected: build `Finished`; all tests PASS (registry build-script accepts the three `product_lane: ai` entries).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs contracts/mcp/tool-registry.canonical.yaml
git commit -m "feat(agy): register vox_agy_pipeline + vox_agy_review + vox_agy_ledger_digest"
```

---

## Task 9: Port the process skills in-repo

Antigravity mounts only in-repo skills (limitations §4), so the authoring/execution/TDD skills the pipeline references must live under `crates/vox-skills/skills/superpowers/`. These are copy-and-adapt tasks: copy each upstream `SKILL.md`, keep its YAML frontmatter (`name`, `description`), and adapt any Claude-Code-only tool names to Vox equivalents exactly as the sibling ported skills already do (compare `dispatching-parallel-agents.skill.md`).

**Files:**
- Create: `crates/vox-skills/skills/superpowers/writing-plans.skill.md`
- Create: `crates/vox-skills/skills/superpowers/executing-plans.skill.md`
- Create: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md`
- Create: `crates/vox-skills/skills/superpowers/test-driven-development.skill.md`

- [ ] **Step 1: Copy each upstream SKILL.md to its in-repo flat file**

Sources (this machine):
- `C:/Users/Owner/.claude/plugins/cache/superpowers-marketplace/superpowers/5.0.7/skills/writing-plans/SKILL.md`
- `.../skills/executing-plans/SKILL.md`
- `.../skills/subagent-driven-development/SKILL.md`
- `.../skills/test-driven-development/SKILL.md`

For each: read the source, write the content verbatim to the matching `crates/vox-skills/skills/superpowers/<name>.skill.md`, preserving frontmatter. Then adapt tool-name references to Vox conventions where any differ (the reference mapping is `references/copilot-tools.md` / `references/codex-tools.md` in the upstream `using-superpowers` skill).

- [ ] **Step 2: Verify the skills parse**

Run: `cargo test -p vox-skills`
Expected: PASS — the SKILL.md YAML-frontmatter parser accepts all four new files.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-skills/skills/superpowers/writing-plans.skill.md crates/vox-skills/skills/superpowers/executing-plans.skill.md crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md crates/vox-skills/skills/superpowers/test-driven-development.skill.md
git commit -m "feat(skills): port writing-plans/executing-plans/subagent-driven/TDD in-repo for all agents"
```

---

## Task 10: The `antigravity-pipeline` skill

**Files:**
- Create: `crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md`

- [ ] **Step 1: Write the skill** (write normal triple-backtick fences; the `​```` markers below carry a zero-width space ONLY to keep this plan's outer code block intact — remove it when creating the file)

```markdown
---
name: antigravity-pipeline
description: Use to run a full hardened Claude-Code→Gemini delegation loop - read the flywheel digest, author a verify-before-use launch statement, delegate with deterministic gate-checking via vox_agy_pipeline, run an automated adversarial review (code-reviewer agent) recorded via vox_agy_review, two-strike correct-and-fix, then stop at the human merge gate. One level above delegate-gemini (single-task primitive).
---

# Antigravity Pipeline (Claude Code ↔ Gemini)

**Announce at start:** "I'm using the antigravity-pipeline skill to run the delegation loop."

You are the **architect + adversarial reviewer**; Gemini (via agy) is the **hands**. Merge is
ALWAYS human-gated — the pipeline never merges to main. In-repo sub-skills:
crates/vox-skills/skills/superpowers/{brainstorming,writing-plans,executing-plans,
subagent-driven-development,test-driven-development,dispatching-parallel-agents,
requesting-code-review,verification-before-completion,delegate-gemini}.skill.md.

## Stage 1 — Author
0. **Flywheel.** Call vox_agy_ledger_digest. Inject the top recurring failure categories as
   explicit "avoid this" rules in the launch statement; read §B of the handoff ledger for the
   matching lessons.
1. **Codebase audit.** Confirm EVERY symbol/path/API with rg/Grep; inline exact signatures.
2. **Targeted web research** only if needed (2-3 fetches).
3. **Plan-engineer** with writing-plans + §B + Gemini limitations §5 (atomic green-committed
   tasks, self-contained, one-decision-per-step, PARALLEL-SAFE/SEQUENTIAL).

## Stage 2 — Execute & verify
Call vox_agy_pipeline with task = the launch statement and gates scoped to the touched crate,
with env CARGO_TARGET_DIR pointing at the main target so cargo reuses cache:

​```json
{
  "task": "Add pub fn parse_config(path:&Path)->Result<Config> to crates/vox-config/src/lib.rs (confirmed present) — no other files.",
  "gates": [
    {"name": "build", "program": "cargo", "args": ["build", "-p", "vox-config"], "env": {"CARGO_TARGET_DIR": "C:/Users/Owner/vox/target"}},
    {"name": "test",  "program": "cargo", "args": ["test",  "-p", "vox-config"], "env": {"CARGO_TARGET_DIR": "C:/Users/Owner/vox/target"}}
  ]
}
​```

Pre-flight: if agy auth is unconfirmed, call vox_agy_doctor and follow its remediation.

## Stage 3-4 — Adversarial review, correct-and-fix, learn + merge
- **Adversarial review (automated).** Dispatch the superpowers:code-reviewer agent against the
  jailed diff with a template that hunts the known Gemini failures: hallucinated-api,
  hollow-green (tests assert shape not behavior), unplanned-shared-change, scope-creep,
  gate-weakening, effect-vs-shape. Prove the EFFECT (ledger B-9).
- **Record it.** Call vox_agy_review with ledger_id + verdict + §B categories + findings +
  1-3 lessons. This writes the {id}-review addendum the flywheel mines.
- **Two-strike.** If outcome != green OR verdict = request-changes: distill the failure into a
  corrected launch statement and re-delegate ONCE. Second failure → STOP + hand off. Never loop.
- **Report** "to what extent implemented" + the ledger trail.

## Human merge gate (always)
Present the jailed agy/<slug> branch + report + review addendum; ask the human to approve the
merge to main.

## Safety invariants (do not weaken)
- Never run agy against the live tree; the worktree jail is the only sandbox.
- Never store Google credentials anywhere; agy owns its OAuth token.
- Gates run exactly as specified — never substitute --warn-only/|| true/--no-verify.
```

- [ ] **Step 2: Verify it parses**

Run: `cargo test -p vox-skills`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-skills/skills/superpowers/antigravity-pipeline.skill.md
git commit -m "feat(skill): antigravity-pipeline — flywheel→author→delegate→verify→review→merge loop"
```

---

## Task 11: Live end-to-end smoke (gated)

**Files:**
- Create: `crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs`

- [ ] **Step 1: Write the gated smoke test**

Create `crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs`:

```rust
//! Live end-to-end smoke for the classifier path against a REAL agy run.
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp agy_pipeline_smoke -- --ignored
//! Prereqs: agy authenticated; run from the repo root (committed HEAD).

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
        task: "Create a new file .vox/pipeline-smoke.txt containing 'pipeline-ok'. No other files.".into(),
        model: None,
        timeout_secs: 180,
    };
    let out = exec.run(&spec).await.expect("agy spawn");
    eprintln!("exit={} timed_out={} elapsed_ms={}", out.exit_code, out.timed_out, out.elapsed_ms);

    let (_diff, files_changed) = wt.capture().await.expect("capture");
    let gates = vec![Gate { name: "probe".into(), program: "git".into(), args: vec!["--version".into()], ..Default::default() }];
    let results = run_gates(&wt.path, &gates, 60).await;

    let outcome = classify_outcome(files_changed, &results, out.timed_out);
    eprintln!("files_changed={files_changed} outcome={outcome}");
    assert_eq!(outcome, "green", "expected a verified green run");

    wt.cleanup(&repo_root).await.expect("cleanup");
}
```

- [ ] **Step 2: Verify it compiles and is ignored**

Run: `cargo test -p vox-orchestrator-mcp agy_pipeline_smoke`
Expected: PASS with `1 ignored`.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-orchestrator-mcp/tests/agy_pipeline_smoke.rs
git commit -m "test(agy): gated live end-to-end pipeline smoke"
```

---

## Task 12: Full verification sweep

**Files:** none (verification only)

- [ ] **Step 1: Full crate test suite**

Run: `cargo test -p vox-orchestrator-mcp`
Expected: PASS — `agy_gates::tests` (5), `agy_pipeline::tests` (7: 5 classifier + `pipeline_validate` + `review_validate`), `agy_ledger::tests` (6), plus the existing suite; smokes `ignored`.

- [ ] **Step 2: Skills parse**

Run: `cargo test -p vox-skills`
Expected: PASS — all ported + new skills parse.

- [ ] **Step 3: arch-check (full output to a file — never pipe cargo to head/grep on Windows)**

Run: `cargo run -p vox-arch-check -- > arch-check.out 2>&1; echo "exit $?"` then read `arch-check.out`.
Expected: exit 0; no new forbidden-pattern violations (`raw-agy-exec` satisfied — all agy spawns go through `AgyExec`; gate-runner spawns `cargo`/`git`, not guarded).

- [ ] **Step 4: ssot-drift**

Run: `cargo run -p vox-cli -- ci ssot-drift > ssot.out 2>&1; echo "exit $?"` then read `ssot.out`.
Expected: no NEW drift from the three tools (pre-existing `query-all-guard` failure on `crates/vox-gui/src/commands/activity.rs` is a known baseline — do not fix here).

- [ ] **Step 5: Clean up scratch files**

```bash
rm -f arch-check.out ssot.out
git status --short
```
Expected: clean working tree.

---

## Self-Review (completed by plan author)

**Spec coverage:**
- Stage 1 author + flywheel digest → Tasks 5, 7 (`vox_agy_ledger_digest`), 10 (skill Stage 1.0). ✓
- Stage 2 deterministic verify (gates in jail, classify, provisional ledger, jail cleanup) → Tasks 1, 2, 3, 6. ✓
- Stage 3-4 automated adversarial review + addendum recording → Tasks 4, 7 (`vox_agy_review`), 10 (skill). ✓
- Flywheel learn-forward (failures → categories → next launch) → Tasks 5, 7, 10. ✓
- Ledger storage = flat file + digest reader (vox-db rejected) → Tasks 5; spec §1/§9. ✓
- Skills for every stage available to all agents (in-repo) → Task 9 + 10. ✓
- Cargo cold-rebuild mitigation (`Gate.env` CARGO_TARGET_DIR) → Task 1 + skill example. ✓
- Merge-gate only / never auto-merge → handler `next_step` + skill. ✓
- Spend proxy (credits not queryable) → Task 6 report. ✓

**Placeholder scan:** no TBD/TODO; every code step has complete code + an exact command. Task 9 is a precise copy-and-adapt with exact source/dest paths (not a placeholder). ✓

**Type consistency:** `Gate{name,program,args,env}` (Default-derived), `GateResult{name,passed,exit_code,output_tail,elapsed_ms}`, `classify_outcome(usize,&[GateResult],bool)`, `LedgerEntry::with_verification`, `ReviewRecord{verdict,categories,findings,lessons,date}`, `append_review_locked(&Path,&str,&ReviewRecord)`, `digest_from_body(&str)->LedgerDigest`, `ledger_digest(&Path)`, `pipeline_validate -> (String,Option<String>,u64,Vec<Gate>)`, `review_validate -> (String,ReviewRecord)` are used identically across Tasks 1-11. ✓
