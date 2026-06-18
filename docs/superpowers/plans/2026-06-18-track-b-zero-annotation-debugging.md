# Track B — Zero-Annotation Severity-Graded Debugging Plan (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `crates/vox-skills/skills/superpowers/test-driven-development.skill.md`. Steps use checkbox (`- [ ]`).

> **🤖 EXECUTION TARGET — READ FIRST.** Run end-to-end by **Gemini 3.5 Flash in Google Antigravity**. Same reliability constraints as Track A (≈48% completion; no mid-task checkpoint; quota hard-cutoff; API hallucination; weak long-context recall). Obey the Operating Rules. Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

> **✅ FEASIBILITY — RESOLVED (2026-06-18).** Re-verification confirmed the capture core (Pernosco/rr) and the gap-fill confirmed the supporting prior art: **OTel eBPF** (zero-code capture, 1–5% CPU, but structural-only — semantics need hints), **Log2** (budget-governed runtime "whether-to-log" selectivity), **DeepLV** (severity inference AUC ≈83.7 → *advise, don't gate*), **OpenRCA** (LLM-RCA solves a minority; telemetry *quality* dominates). Tasks 1–6 un-gated; Task 7 (LLM context) keeps a light A/B gate. See [auto-debugging research](../../src/architecture/auto-debugging-zero-annotation-research-2026-06-18.md) + [error-surfacing](../../src/architecture/error-surfacing-dual-audience-research-2026-06-18.md).

## Operating Rules (apply to EVERY task)
1. **Atomic + green + committed** — a crash between tasks leaves a compiling, tested tree.
2. **Verify-before-use** — confirm every symbol/path with the task's `rg`/read step before coding; never invent APIs.
3. **Self-contained** — don't rely on remembering earlier tasks.
4. **Two-strike circuit breaker** — fail twice → STOP + handoff note; don't loop.
5. **Parallel dispatch** — honor `[PARALLEL-SAFE]`/`[SEQUENTIAL]`; never two subagents on one file (handoff §3/§5.2).
6. **Vox house rules** — no `cargo fmt --all`; automation is `.vox`; `docs/src/` `.md` needs frontmatter.
7. **Verification ritual** before commit (use `verification-before-completion` skill): `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`, paste output. Self-review with `requesting-code-review` skill.
8. **Rollback on broken tree.** If a task aborts mid-edit leaving a non-compiling tree, `git reset --hard HEAD` to the last green commit and re-attempt that one task. Never build forward on a broken tree.
9. **Skill references.** Native skills under `crates/vox-skills/skills/superpowers/`: `brainstorming`, `dispatching-parallel-agents`, `using-git-worktrees`, `verification-before-completion`, `requesting-code-review` (handoff §4).
10. **Rust implementation constraints** — obey design §5b: additive struct fields (`tracer`, `execution_context`) break every construction site → fix each (`Option`+`None`); **no allocation/`format!` in the per-step hot path** and **cap event volume** (budget); no `.unwrap()` in library code; `cargo run -p vox-arch-check` must pass (new crate one-directional dep).

**Goal:** Add an opt-in interpreter execution-event stream with automatically inferred severity, surfaced to humans via a CLI timeline and to LLMs via the existing diagnostic envelope — no new language syntax, zero source annotations, selective-by-default.

**Architecture:** New L2 crate `vox-execution-tracer` owns `ExecutionEvent` + a pure rules-based `infer_severity` (DeepLV-style ML deferred; severity *advises*). The interpreter (`vox-compiler::eval`) gains an optional tracer emitting at existing step/error boundaries (OBI lesson: capture is structural; severity adds meaning). A CLI flag renders a Notice+ timeline (Log2 selectivity). The LLM path reuses the existing diagnostic envelope (OpenRCA lesson: feed curated, ranked context).

**Tech Stack:** Rust; new `vox-execution-tracer`; `vox-telemetry` (`record_event!`, `TelemetryEvent`); `vox-compiler::eval::Interpreter`; `vox-cli`.

**Design:** [`../../src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md`](../../src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md) §3.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-execution-tracer/Cargo.toml` + `src/lib.rs` | `Severity`, `ExecutionEvent`, `infer_severity`, `Tracer` | Create (T1,2,4) |
| `crates/vox-execution-tracer/src/timeline.rs` | severity-filtered human timeline | Create (T3) |
| `crates/vox-compiler/src/eval/mod.rs:242` + `:583` | optional `tracer` field + emit at `track_step` | Modify (T4) |
| `crates/vox-compiler/src/eval/{expr,stmt}.rs` | accurate event kinds | Modify (T5) |
| `vox run` command handler | `--trace` flag | Modify (T6) |
| `crates/vox-compiler/src/typeck/diagnostics.rs` | `execution_context` field | Modify (T7) |
| `docs/.../where-things-live.md`, `layers.toml` | register crate, L2 | Modify (T1, T8) |

**Pre-flight (paste output; NOT code):**
- `rg -n "members = \[" Cargo.toml` — workspace members array.
- `rg -n "macro_rules! record_event|pub enum TelemetryEvent" crates/vox-telemetry/src/*.rs` — confirm telemetry API.
- `rg -n "pub struct Interpreter|pub fn track_step|Self \{" crates/vox-compiler/src/eval/mod.rs | head` — confirm struct + `track_step` (~line 583) + initializer (~242).
- `cargo run -p vox-arch-check` — baseline.

---

## Task 1 `[SEQUENTIAL]`: Scaffold crate + `Severity`

**Files:** Create `crates/vox-execution-tracer/Cargo.toml`, `src/lib.rs`; Modify root `Cargo.toml` members; maybe `layers.toml`.

- [ ] **Step 1 (verify-before-use):** Run `rg -n "^serde" Cargo.toml`. Note the exact workspace serde line to mirror.

- [ ] **Step 2: Create `src/lib.rs`:**

```rust
//! Zero-annotation execution tracing + severity inference. Design §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Severity { Trace, Debug, Notice, Warn, Error }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn severity_is_ordered() {
        assert!(Severity::Error > Severity::Warn);
        assert!(Severity::Notice > Severity::Debug);
        assert!(Severity::Debug > Severity::Trace);
    }
}
```

Create `Cargo.toml`:

```toml
[package]
name = "vox-execution-tracer"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true, features = ["derive"] }
```

Add `"crates/vox-execution-tracer"` to the root `Cargo.toml` `members`.

- [ ] **Step 3: Run → PASS (compiles).** `cargo test -p vox-execution-tracer severity_is_ordered`. If it does not compile, fix the manifest until green.

- [ ] **Step 4: arch-check.** `cargo run -p vox-arch-check`. If a fan-in/layer violation appears, add an L2 entry for `vox-execution-tracer` in `docs/src/architecture/layers.toml` allowing `vox-telemetry`.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-execution-tracer/ Cargo.toml docs/src/architecture/layers.toml
git commit -m "feat(execution-tracer): scaffold L2 crate with ordered Severity"
```

---

## Task 2 `[SEQUENTIAL]` (same lib.rs): `ExecutionEvent` + `infer_severity`

The selectivity core. Severity **advises** (DeepLV ceiling ≈84% → never gate). Selective-by-default echoes Log2.

**Files:** Modify `crates/vox-execution-tracer/src/lib.rs`.

- [ ] **Step 1: Failing tests.** Add types + tests:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EventKind { FunctionCall, Return, VariableBind, Branch, Loop, EffectPerformed { failed: bool }, Error }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionEvent {
    pub kind: EventKind,
    pub span: (usize, usize),
    pub label: String,
    pub scope_depth: u32,
    pub elapsed_ns: u64,
}
```

```rust
#[cfg(test)]
fn ev(kind: EventKind) -> ExecutionEvent {
    ExecutionEvent { kind, span: (0,0), label: "x".into(), scope_depth: 0, elapsed_ns: 0 }
}
#[test]
fn errors_and_failed_effects_are_error() {
    assert_eq!(infer_severity(&ev(EventKind::Error)), Severity::Error);
    assert_eq!(infer_severity(&ev(EventKind::EffectPerformed { failed: true })), Severity::Error);
}
#[test]
fn successful_effects_are_notice() {
    assert_eq!(infer_severity(&ev(EventKind::EffectPerformed { failed: false })), Severity::Notice);
}
#[test]
fn ordinary_steps_not_surfaced_by_default() {
    assert!(infer_severity(&ev(EventKind::VariableBind)) <= Severity::Debug);
    assert!(infer_severity(&ev(EventKind::Return)) <= Severity::Debug);
}
#[test]
fn deep_recursion_escalates_to_notice() {
    let mut e = ev(EventKind::Branch); e.scope_depth = 64;
    assert_eq!(infer_severity(&e), Severity::Notice);
}
```

- [ ] **Step 2: Run → FAIL** (`infer_severity` missing). `cargo test -p vox-execution-tracer infer_severity`.

- [ ] **Step 3: Implement.**

```rust
const DEEP_SCOPE_THRESHOLD: u32 = 32;
/// Rules-based, selective-by-default (Log2). Advises, never gates (DeepLV ~84% ceiling).
/// v2 may swap for a learned model behind this signature.
pub fn infer_severity(event: &ExecutionEvent) -> Severity {
    match &event.kind {
        EventKind::Error | EventKind::EffectPerformed { failed: true } => Severity::Error,
        EventKind::EffectPerformed { failed: false } => Severity::Notice,
        EventKind::Branch | EventKind::Loop if event.scope_depth >= DEEP_SCOPE_THRESHOLD => Severity::Notice,
        EventKind::FunctionCall => Severity::Debug,
        EventKind::Branch | EventKind::Loop | EventKind::Return | EventKind::VariableBind => Severity::Trace,
    }
}
```

- [ ] **Step 4: Run → PASS**, Rule 7, commit.

```bash
git add crates/vox-execution-tracer/src/lib.rs
git commit -m "feat(execution-tracer): ExecutionEvent + rules-based infer_severity"
```

---

## Task 3 `[PARALLEL-SAFE]` (new file timeline.rs): human timeline renderer

Default threshold Notice (Log2 selectivity — keep ordinary steps out). New file → parallel-safe after Task 2.

**Files:** Create `crates/vox-execution-tracer/src/timeline.rs`; Modify `src/lib.rs` (add `pub mod timeline;`).

- [ ] **Step 1: Create file + failing test.**

```rust
//! Render execution events into a human-readable CLI timeline.
use crate::{infer_severity, ExecutionEvent, Severity};
pub fn render_timeline(events: &[ExecutionEvent], threshold: Severity) -> String {
    let _ = (events, threshold); String::new()
}
#[cfg(test)]
mod tests {
    use super::*; use crate::EventKind;
    fn ev(kind: EventKind, label: &str) -> ExecutionEvent {
        ExecutionEvent { kind, span: (0,0), label: label.into(), scope_depth: 0, elapsed_ns: 0 }
    }
    #[test]
    fn default_threshold_hides_trace_shows_errors() {
        let events = vec![
            ev(EventKind::VariableBind, "x"),
            ev(EventKind::Error, "boom"),
            ev(EventKind::EffectPerformed { failed: false }, "db.write"),
        ];
        let out = render_timeline(&events, Severity::Notice);
        assert!(!out.contains("x"), "trace leaked:\n{out}");
        assert!(out.contains("boom"), "error:\n{out}");
        assert!(out.contains("db.write"), "notice:\n{out}");
        assert!(out.contains("[ERROR]"), "tag:\n{out}");
    }
}
```

Add `pub mod timeline;` to `lib.rs`.

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-execution-tracer default_threshold_hides_trace`.

- [ ] **Step 3: Implement.**

```rust
pub fn render_timeline(events: &[ExecutionEvent], threshold: Severity) -> String {
    let mut out = String::new();
    for e in events {
        let sev = infer_severity(e);
        if sev < threshold { continue; }
        let tag = match sev {
            Severity::Error => "[ERROR]", Severity::Warn => "[WARN] ",
            Severity::Notice => "[NOTE] ", Severity::Debug => "[DEBUG]", Severity::Trace => "[TRACE]",
        };
        out.push_str(&format!("{tag} {}\n", e.label));
    }
    out
}
```

- [ ] **Step 4: Run → PASS**, Rule 7, commit.

```bash
git add crates/vox-execution-tracer/src/lib.rs crates/vox-execution-tracer/src/timeline.rs
git commit -m "feat(execution-tracer): severity-filtered human timeline renderer"
```

---

## Task 4 `[SEQUENTIAL]`: `Tracer` collector + interpreter hook (opt-in)

⚠ Interpreter-touching. Add an optional collector; emit at the existing `track_step`. `None` default = zero cost.

**Files:** Modify `vox-execution-tracer/src/lib.rs`; Modify `crates/vox-compiler/src/eval/mod.rs` (struct + initializer ~:242, `track_step` ~:583); Modify `crates/vox-compiler/Cargo.toml`.

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub struct Interpreter|pub fn track_step|self.steps" crates/vox-compiler/src/eval/mod.rs`. Confirm the struct fields and that `track_step` increments `self.steps`. Confirm the `Self { ... }` initializer fields (scope, module_scope, step_limit, steps, caps, source_path, loaded_imports, db, repo).

- [ ] **Step 2: Add `Tracer` + test (in tracer crate).**

**Budgeted by construction (Log2 principle, design §5b.5):** memory is bounded; once the budget is hit, further events are counted but not stored — no unbounded growth on long runs.

```rust
/// Default max events retained when tracing is enabled (bounds memory on long runs).
pub const DEFAULT_TRACE_BUDGET: usize = 50_000;

#[derive(Debug)]
pub struct Tracer { events: Vec<ExecutionEvent>, budget: usize, dropped: usize }

impl Default for Tracer {
    fn default() -> Self { Self::with_budget(DEFAULT_TRACE_BUDGET) }
}

impl Tracer {
    pub fn with_budget(budget: usize) -> Self { Self { events: Vec::new(), budget, dropped: 0 } }
    /// Record within budget; over budget, increment `dropped` (cheap, no alloc, no panic).
    pub fn record(&mut self, e: ExecutionEvent) {
        if self.events.len() < self.budget { self.events.push(e); } else { self.dropped += 1; }
    }
    pub fn events(&self) -> &[ExecutionEvent] { &self.events }
    pub fn dropped(&self) -> usize { self.dropped }
}
```

```rust
#[test]
fn tracer_collects_within_budget_then_drops() {
    let mut t = Tracer::with_budget(1);
    let ev = || ExecutionEvent { kind: EventKind::Error, span: (1,2), label: "e".into(), scope_depth: 0, elapsed_ns: 0 };
    t.record(ev()); t.record(ev());
    assert_eq!(t.events().len(), 1, "budget caps stored events");
    assert_eq!(t.dropped(), 1, "over-budget events are counted not stored");
}
```

Run → PASS; commit the tracer-crate change before touching the compiler (keeps commits atomic):

```bash
git add crates/vox-execution-tracer/src/lib.rs
git commit -m "feat(execution-tracer): Tracer event collector"
```

- [ ] **Step 3: Wire into the compiler — failing integration test first.** Add `vox-execution-tracer` to `crates/vox-compiler/Cargo.toml` deps (one-directional; confirm `cargo run -p vox-arch-check` stays green). Add `pub tracer: Option<vox_execution_tracer::Tracer>,` to the `Interpreter` struct, and `tracer: None,` to the `Self { ... }` initializer (~:242 — the only construction site per the audit; `cargo build` will flag any other, e.g. in tests). Find an existing eval test pattern: `rg -n "#\[test\]" crates/vox-compiler/src/eval/mod.rs | head`. Add a test that builds an interpreter **with a tiny `step_limit`** and `tracer = Some(Tracer::default())`, runs a runaway loop module, asserts it returns `Err(EvalError::StepLimitExceeded)` **and** `events()` contains one `EventKind::Error`.

- [ ] **Step 4: Emit at the step-limit boundary (~:583) — NOT per step.** Per design §5b.5, do **not** allocate/`format!` or push an event on every step (unbounded + hot-path alloc). Record only the meaningful, rare boundary; Task 5 adds sparse call/error events at real sites.

```rust
    pub fn track_step(&mut self) -> Result<(), EvalError> {
        self.steps += 1;
        if self.steps >= self.step_limit {
            if let Some(t) = self.tracer.as_mut() {
                t.record(vox_execution_tracer::ExecutionEvent {
                    kind: vox_execution_tracer::EventKind::Error,
                    span: (0, 0),
                    label: "step limit exceeded".into(), // single rare event; no per-step alloc
                    scope_depth: 0,
                    elapsed_ns: 0,
                });
            }
            return Err(EvalError::StepLimitExceeded);
        }
        Ok(())
    }
```

This **replaces** the original `track_step` body wholesale (the old `self.steps += 1; if self.steps >= self.step_limit { Err(..) } else { Ok(()) }`). Behavior is identical when `tracer` is `None` (zero cost); the only addition is the single rare event on the limit branch.

- [ ] **Step 5: Run → PASS + regression.** `cargo test -p vox-compiler` (the `None` default keeps existing behavior identical).

- [ ] **Step 6: Rule 7 + commit.**

```bash
git add crates/vox-compiler/src/eval/mod.rs crates/vox-compiler/Cargo.toml
git commit -m "feat(eval): opt-in execution tracer hook at track_step (None = zero cost)"
```

---

## Task 5 `[SEQUENTIAL]`: accurate event kinds at eval sites

⚠ Interpreter-touching. Replace the coarse default with real `FunctionCall`/`Error` events.

**Files:** Modify `crates/vox-compiler/src/eval/expr.rs`, `crates/vox-compiler/src/eval/stmt.rs`.

- [ ] **Step 1 (verify-before-use):** `rg -n "HirExpr::Call|fn eval_expr|fn eval_stmt|EvalError::" crates/vox-compiler/src/eval/expr.rs crates/vox-compiler/src/eval/stmt.rs | head -30`. Note the call-eval site and error-return sites.

- [ ] **Step 2: Failing test.** Add an eval test: run a module that calls a function then errors; assert the collected events contain ≥1 `EventKind::FunctionCall` and ≥1 `EventKind::Error` (filter `tracer.events()` by `kind`).

- [ ] **Step 3: Implement.** At the call-eval site, when `self.tracer.is_some()`, push a `FunctionCall` event with the callee name as `label`. Add a helper `fn trace_error(&mut self, label: &str)` that pushes an `Error` event when a tracer is present, and call it right before constructing real runtime `Err(EvalError::...)` returns. Keep ALL emission behind `if let Some(t) = self.tracer.as_mut()`.

- [ ] **Step 4: Run → PASS + regression.** `cargo test -p vox-compiler`.

- [ ] **Step 5: Rule 7 + commit.**

```bash
git add crates/vox-compiler/src/eval/expr.rs crates/vox-compiler/src/eval/stmt.rs
git commit -m "feat(eval): emit accurate ExecutionEvent kinds at call/error sites"
```

---

## Task 6 `[SEQUENTIAL]`: CLI `--trace` prints the timeline

**Files:** Modify the `vox run` command handler.

- [ ] **Step 1 (verify-before-use):** `rg -n "Commands::Run|fn .*run|Interpreter::new|\"run\"" crates/vox-cli/src/ | head`. Find the run subcommand struct + where the interpreter is built + run.

- [ ] **Step 2: Failing test.** Add `--trace` (bool) to the run subcommand. When set: install `Tracer::default()` before running; after, print `render_timeline(events, Severity::Notice)` to stderr. Add a CLI integration test (mirror one in `crates/vox-cli/tests/`) asserting stderr contains `[NOTE]`/`[ERROR]` for a script that performs an effect/error with `--trace`, and contains no timeline without it.

- [ ] **Step 3: Run → FAIL → implement → PASS.** `cargo test -p vox-cli trace`.

- [ ] **Step 4: Rule 7 + commit.**

```bash
git add crates/vox-cli/
git commit -m "feat(cli): vox run --trace prints severity-graded execution timeline"
```

---

## Task 7 `[SEQUENTIAL]` (LIGHT-GATED): LLM execution_context in the diagnostic envelope

⚠ Validate the dual-audience benefit with an A/B before investing ([error-surfacing §4](../../src/architecture/error-surfacing-dual-audience-research-2026-06-18.md); OpenRCA: telemetry quality dominates). Then add optional runtime context to the LLM payload.

**Files:** Modify `crates/vox-compiler/src/typeck/diagnostics.rs`.

- [ ] **Step 1 (verify-before-use):** `rg -n "struct VoxCompilerDiagnosticPayload|minimal_repro|fn from_diagnostic" crates/vox-compiler/src/typeck/diagnostics.rs`. Confirm the payload struct + its constructors.

- [ ] **Step 2: Failing test.** Assert the payload serializes an optional `execution_context: Option<Vec<String>>` (last N timeline lines around the error), omitted when `None`.

- [ ] **Step 3: Implement.** Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub execution_context: Option<Vec<String>>,`. Default it `None` in every existing constructor (the compiler enumerates the sites). Add a setter accepting `render_timeline(...)` split into lines.

- [ ] **Step 4: Run → PASS + envelope regression.** `cargo test -p vox-compiler`; `cargo test -p vox-cli check_for_llm_envelope` (existing `--for-llm` test stays green; new field optional).

- [ ] **Step 5: Rule 7 + commit.**

```bash
git add crates/vox-compiler/src/typeck/diagnostics.rs
git commit -m "feat(diagnostics): optional execution_context in LLM diagnostic envelope"
```

---

## Task 8 `[PARALLEL-SAFE]` (docs): register crate

**Files:** `docs/src/architecture/where-things-live.md` (+ confirm `layers.toml` from T1). Docs-only → parallel-safe.

- [ ] **Step 1:** Add row `| Zero-annotation execution tracing + severity inference | crates/vox-execution-tracer/ |` (match table columns).

- [ ] **Step 2: Commit.**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register vox-execution-tracer"
```

---

## Parallelization summary (for the Antigravity orchestrator)

- **T1→T2 SEQUENTIAL** (both `lib.rs`).
- **T3 PARALLEL-SAFE after T2** (own file `timeline.rs`) — may run beside T8.
- **T4→T5 SEQUENTIAL** (both interpreter; T5 depends on T4's field). T4 depends on T2 (uses `ExecutionEvent`).
- **T6 SEQUENTIAL after T4** (needs `Tracer`+`render_timeline`).
- **T7 SEQUENTIAL, light-gated** (independent file but logically after T3's renderer exists).
- **T8 PARALLEL-SAFE** (docs) — any time after T1.
- **Recommended waves:** Wave 1 = T1→T2. Wave 2 = {T3, T8} parallel + T4 starting. Wave 3 = T5→T6. Wave 4 = T7 (after A/B). Never put two agents on `lib.rs` or on the interpreter simultaneously.

---

## Self-Review

- **Spec coverage (design §3):** event stream (T2,4,5); selective severity (T2 + T3 Notice default, Log2-aligned); dual-audience (T6 human + T7 LLM). GUI timeline surface = **Deferred** (compose with Track A once it lands).
- **Deferred (YAGNI):** GUI heatmap; DeepLV-style learned severity (swap behind `infer_severity`); **Log2 per-run event budget** (refine `infer_severity` with run-level budget context); record/replay (non-goal); frequency-baseline anomaly severity.
- **Placeholder scan:** T5/T6/T7 use verify-then-implement steps with exact `rg` + explicit instructions (must match live eval/CLI/payload code). Pure core (T1–T3) fully specified.
- **Type consistency:** `Severity`/`EventKind`/`ExecutionEvent`/`Tracer`/`infer_severity`/`render_timeline` consistent across tasks; `ExecutionEvent.span` is `(usize,usize)` everywhere (no `vox-ast` dep → keeps crate L2-clean); interpreter field `tracer: Option<Tracer>` consistent T4–6.
- **Antigravity fit:** atomic commits; verify-before-use; circuit breaker; parallel waves documented; T4 split so the tracer-crate change commits before the compiler change.

## Execution Handoff

Track B only; independent of Track A. Tasks 1–6 ready; Task 7 after the dual-audience A/B. Missing skills: see [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md) §5.
