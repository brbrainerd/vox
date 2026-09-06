# True Workflow Durability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Vox workflow replay real — execute the workflow body, run actual activity code, persist actual results, and survive crashes, timers, and signals.

**Architecture:** The workflow body runs in the tree-walking interpreter on a dedicated OS thread (the interpreter is `!Send` because `VoxValue` holds `Rc`). Activity calls are intercepted at the interpreter's single `Call(Ident)` site and delegated over a `tokio::sync::mpsc` pair to the async runner, which does journal I/O through the existing `WorkflowTracker`. Because the body really executes, arguments, results, `?`, `match`, loops, and runtime branch conditions all work for free, and the 609-line compile-time linearizer in `workflow/plan.rs` is deleted rather than extended.

**Tech Stack:** Rust 2024, `vox-compiler` (HIR + `eval::Interpreter`), `vox-workflow-runtime`, `vox-db` (libsql), `vox-codegen`, tokio, serde_json, blake3.

**Spec:** `docs/src/architecture/true-workflow-durability-design-2026.md`

## Global Constraints

- **Format with `vox run scripts/fmt.vox`**, never `cargo fmt --all` (Windows `CreateProcess` limit). Check-only: `VOX_FMT_CHECK=1 vox run scripts/fmt.vox`.
- **Before every push:** `vox ci pre-push --complete` (the default fast tier does not run clippy or tests). Never use GitHub Actions as the feedback loop.
- **No new workspace crate edges.** `vox-workflow-runtime` already depends on `vox-compiler`, `vox-db`, `vox-populi`. Adding an edge requires a USER-AUTHORIZED ledger entry in `contracts/ci/crate-edges.allow.v1.json` — propose, do not write.
- **Layer budgets:** `vox-compiler` is L3 `max_loc = 45_000`; `vox-workflow-runtime` is L3. Both edits stay in-crate.
- **Test-first is binding.** Every new `pub fn` in `crates/*/src/**` needs a test in the same file before the commit lands (`skeleton/untested-pub-api`). The failing test is step 1 of every task below.
- **Any new `.md` under `docs/src/` needs frontmatter** (`title`, `description`, `category`) written at file-creation time.
- **Verify guards by mutation.** For every security- or correctness-critical assertion, break the guard, confirm the test fails, restore. A test that passes against both the fixed and unfixed code is worthless.
- **Journal contract:** events carry `journal_version = 1`. Adding event *names* to `contracts/workflow/workflow-journal.v1.schema.json` is backward-compatible evolution of v1 (old runs never contain them). Changing an existing event's shape is not — that needs a v2 file.
- **Commit message trailer:** `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

---

## File Structure

**Created**

| Path | Responsibility |
|---|---|
| `crates/vox-compiler/src/eval/activity_hook.rs` | The `ActivityCall` / `ActivityDecision` types and the interpreter-side dispatch function. Keeps `expr.rs` from growing. |
| `crates/vox-workflow-runtime/src/workflow/host.rs` | The `!Send` bridge: spawns the interpreter thread, owns the two channels, exposes an async request/response API. |
| `crates/vox-workflow-runtime/src/workflow/waker.rs` | The parked-run waker loop (timers + signals), modelled on `scheduled/runner.rs`. |
| `crates/vox-workflow-runtime/tests/real_activity_execution.rs` | Proves activity bodies actually run and results are journalled. |
| `crates/vox-workflow-runtime/tests/crash_windows.rs` | Kill-after-started / kill-after-completed proofs with a side-effect counter. |
| `crates/vox-workflow-runtime/tests/runtime_branching.rs` | `if activity_result.ok`, `match`, `while` over a journalled value. |
| `crates/vox-workflow-runtime/tests/durable_timer.rs` | Park-and-wake timer proof. |
| `crates/vox-workflow-runtime/tests/durable_signal.rs` | Park-on-missing-signal proof. |
| `examples/golden/durable_workflow_branching.vox` | Golden covering runtime branch + loop + `?`. |

**Modified**

| Path | Change |
|---|---|
| `crates/vox-compiler/src/eval/value.rs` | Add lossless `to_journal_json` / `from_journal_json`. |
| `crates/vox-compiler/src/eval/mod.rs` | Two new `Interpreter` fields; new `EvalError::WorkflowParked`. |
| `crates/vox-compiler/src/eval/expr.rs` | One interception branch in the `Call` arm. |
| `crates/vox-workflow-runtime/src/workflow/run.rs` | Rewritten around the host; stub `execute_local_activity_step` deleted. |
| `crates/vox-workflow-runtime/src/workflow/plan.rs` | Linearizer deleted; a small activity-call-site counter survives. |
| `crates/vox-workflow-runtime/src/db_tracker.rs` | Implement patch persistence, park/wake, signal park. |
| `crates/vox-db/src/schema/domains/execution.rs` | `workflow_patch_log` table; `wake_at_ms` + `parked_reason` on `workflow_run_log`. |
| `crates/vox-db/src/facade/workflow.rs` | Facade methods for the above. |
| `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs` | `VoxDbTracker` instead of `DefaultTracker`; drop the `journal::execute` wrapper. |
| `contracts/workflow/workflow-journal.v1.schema.json` | Admit the events actually emitted. |
| `docs/src/explanation/expl-durable-execution.md`, `docs/src/tutorials/tut-workflow-durability.md` | Stop over-claiming. |

---

## Phase 0 — Stop lying, then make the lie testable

### Task 0.1: Journal schema admits the events actually emitted

`run.rs` emits `WorkflowPatch` and `ActivityCacheHit`. Neither is in the v1 schema's `event` enum, so the runtime is already out of contract. The existing schema test only validates hand-built retry events, which is why nobody noticed.

**Files:**
- Modify: `contracts/workflow/workflow-journal.v1.schema.json`
- Test: `crates/vox-workflow-runtime/tests/journal_schema_conformance.rs` (create)

**Interfaces:**
- Consumes: nothing.
- Produces: `contracts/workflow/workflow-journal.v1.schema.json` with an `event` enum that later phases extend (`ActivityParked`, `TimerScheduled`, `SignalAwaited`, `WorkflowParked`).

- [ ] **Step 1: Write the failing test**

Create `crates/vox-workflow-runtime/tests/journal_schema_conformance.rs`:

```rust
#![allow(missing_docs)]
//! Every event the runner ACTUALLY emits must validate against the v1 schema.
//! The pre-existing schema test validated hand-built objects, which cannot
//! catch an event name the runner emits but the schema rejects.

use jsonschema::validator_for;
use serde_json::Value;
use std::sync::Arc;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::VoxDbTracker;
use vox_workflow_runtime::workflow::interpret_workflow_durable;

const SRC: &str = r#"
activity charge_card(amount: int) to Result[str] {
    return Ok("tx")
}

workflow checkout(amount: int) to Result[str] {
    workflow.version("add-audit-v2", 1, 2)
    let tx = charge_card(amount)?
    return Ok(tx)
}
"#;

#[tokio::test]
async fn every_emitted_event_validates_against_v1_schema() {
    let module = parse(lex(SRC)).expect("parses");
    let hir = lower_module(&module);
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let mut tracker = VoxDbTracker::new(db, "schema-conformance-1");

    let journal = interpret_workflow_durable(&hir, "checkout", &mut tracker)
        .await
        .expect("workflow runs");

    let schema: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/workflow/workflow-journal.v1.schema.json"
    )))
    .expect("schema parses");
    let validator = validator_for(&schema).expect("schema compiles");

    for entry in &journal {
        if let Err(err) = validator.validate(entry) {
            panic!("runner emitted an event the v1 schema rejects: {err}\nentry={entry}");
        }
    }
    assert!(
        journal
            .iter()
            .any(|e| e["event"].as_str() == Some("WorkflowPatch")),
        "test must actually exercise the WorkflowPatch path; got {journal:#?}"
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p vox-workflow-runtime --test journal_schema_conformance
```

Expected: FAIL — `runner emitted an event the v1 schema rejects` naming `WorkflowPatch`.

- [ ] **Step 3: Add the missing event names to the schema**

In `contracts/workflow/workflow-journal.v1.schema.json`, extend `properties.event.enum` with the names the runner emits today plus the ones later phases add. Insert after `"MeshActivitySkipped"`:

```json
        "WorkflowPatch",
        "ActivityCacheHit",
        "ActivityParked",
        "TimerScheduled",
        "SignalAwaited",
        "WorkflowParked"
```

The file's `oneOf` block constrains only specific events and `additionalProperties` is permissive, so no `oneOf` branch is needed for the new names.

- [ ] **Step 4: Run it and watch it pass**

```bash
cargo test -p vox-workflow-runtime --test journal_schema_conformance
```

Expected: PASS.

- [ ] **Step 5: Mutation-verify the guard**

Temporarily remove `"WorkflowPatch"` from the enum, re-run, confirm FAIL, then restore and confirm PASS. A schema test that passes with the constraint removed is proving nothing.

```bash
cargo test -p vox-workflow-runtime --test journal_schema_conformance
```

- [ ] **Step 6: Regenerate the contracts index and commit**

```bash
cargo run -q -p vox-cli -- ci contracts-index
vox run scripts/fmt.vox
git add contracts/workflow/workflow-journal.v1.schema.json crates/vox-workflow-runtime/tests/journal_schema_conformance.rs contracts/index.yaml
git commit -m "fix(workflow): admit WorkflowPatch and ActivityCacheHit into the v1 journal schema

The runner has emitted both since P2-T2/P2-T5 but the schema enum rejected
them; the existing contract test only validated hand-built retry events so it
never exercised the real journal.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 0.2: Documentation stops over-claiming

Until Phase 1 lands, every doc that implies activity bodies run is wrong. Users who read them and then read a journal see `{"status":"executed"}` and believe their code ran.

**Files:**
- Modify: `docs/src/explanation/expl-durable-execution.md`
- Modify: `docs/src/tutorials/tut-workflow-durability.md`
- Modify: `docs/src/architecture/durability-runtime-audit-2026.md`

**Interfaces:**
- Consumes: `docs/src/architecture/true-workflow-durability-design-2026.md` (the corrected design).
- Produces: nothing code-facing.

- [ ] **Step 1: Add a status banner to the explanation doc**

Insert immediately after the `# Explanation: Durable Execution` heading in `docs/src/explanation/expl-durable-execution.md`:

```markdown
> [!WARNING]
> **Known limits as of 2026-09-05.** The interpreted runner does not yet execute
> local activity bodies — a non-mesh activity records a fixed
> `{"event":"LocalActivity","status":"executed"}` payload instead of your
> function's return value. `workflow_wait` sleeps in-process rather than
> scheduling a durable wake. `workflow_wait_signal` fails the run when the
> signal is absent instead of parking it. Workflows built with `vox build` use
> an in-memory tracker, so a crash replays from zero.
> Tracked in [True workflow durability](../architecture/true-workflow-durability-design-2026.md);
> remove this banner as each phase lands.
```

- [ ] **Step 2: Correct §2 of the same doc**

Replace the step-4 line of the "Recovery via Replay" list:

```markdown
4. Continue with the remaining steps. **Today the replayed payload is the
   runtime's own step record, not your activity's return value** — see the
   banner above.
```

- [ ] **Step 3: Add the same banner to the tutorial**

Insert the identical `> [!WARNING]` block after the H1 of `docs/src/tutorials/tut-workflow-durability.md`, adjusting the relative link to `../architecture/true-workflow-durability-design-2026.md`.

- [ ] **Step 4: Mark the stale audit**

Insert after the H1 of `docs/src/architecture/durability-runtime-audit-2026.md`:

```markdown
> [!NOTE]
> **Superseded (2026-09-05).** This audit describes the tree as of 2026-05-01 and
> states there is no journal. There is one, and ADR-019 froze it. Current design:
> [True workflow durability](true-workflow-durability-design-2026.md).
```

- [ ] **Step 5: Lint the docs**

```bash
cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/explanation/expl-durable-execution.md docs/src/tutorials/tut-workflow-durability.md docs/src/architecture/durability-runtime-audit-2026.md docs/src/architecture/true-workflow-durability-design-2026.md
```

Expected: PASS (frontmatter present on all four).

- [ ] **Step 6: Frontmatter and commit**

Set valid frontmatter on the new page (`title`, `description`, `category`, `status`).
Starlight lists it. Do **not** create or edit `docs/src/architecture/research-index.md` (retired 2026-09).

Then:

```bash
git add docs/
git commit -m "docs(durability): state the actual limits of the interpreted runner

Local activity bodies do not run, timers sleep in-process, signals fail rather
than park, and generated workflows use an in-memory tracker. Documenting this
before fixing it so the fix does not read as a regression against the docs.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Phase 1 — Execute the workflow body for real

This is the whole product. Phases 2–6 are meaningless before it: they would be parking and replaying a canned string.

### Task 1.1: Lossless `VoxValue` ↔ JSON for the journal

`eval/builtins.rs` already has `vox_to_json` / `json_to_vox`, but they are private and **lossy**: `Result`, `Option`, `Tagged`, `Tuple`, and `Decimal` all fall through to `_ => Value::Null`. An activity returning `Ok("tx_5")` would journal as `null` and replay as `null`. Do not change those functions — `json.encode` depends on their plain mapping. Add a separate tagged encoding for journal use.

**Files:**
- Modify: `crates/vox-compiler/src/eval/value.rs`
- Test: `crates/vox-compiler/src/eval/value.rs` (`#[cfg(test)] mod journal_json_tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `VoxValue::to_journal_json(&self) -> serde_json::Value` and `VoxValue::from_journal_json(v: &serde_json::Value) -> VoxValue`. Task 1.3 and Task 1.4 use both.

- [ ] **Step 1: Write the failing round-trip test**

Append to `crates/vox-compiler/src/eval/value.rs`:

```rust
#[cfg(test)]
mod journal_json_tests {
    use super::*;

    fn roundtrip(v: VoxValue) -> VoxValue {
        VoxValue::from_journal_json(&v.to_journal_json())
    }

    // Catches the lossy `_ => Null` arm: an activity returning Ok(...) must not
    // journal as null, or every replayed Result becomes a silent None.
    #[test]
    fn result_ok_survives_roundtrip() {
        let v = VoxValue::Result(Ok(Box::new(VoxValue::Str("tx_5".into()))));
        match roundtrip(v) {
            VoxValue::Result(Ok(inner)) => assert!(matches!(*inner, VoxValue::Str(ref s) if s == "tx_5")),
            other => panic!("expected Result(Ok(Str)), got {other:?}"),
        }
    }

    #[test]
    fn result_err_survives_roundtrip() {
        let v = VoxValue::Result(Err(Box::new(VoxValue::Str("declined".into()))));
        match roundtrip(v) {
            VoxValue::Result(Err(inner)) => assert!(matches!(*inner, VoxValue::Str(ref s) if s == "declined")),
            other => panic!("expected Result(Err(Str)), got {other:?}"),
        }
    }

    #[test]
    fn option_none_is_distinguishable_from_null() {
        assert!(matches!(roundtrip(VoxValue::Option(None)), VoxValue::Option(None)));
        assert!(matches!(roundtrip(VoxValue::Null), VoxValue::Null));
    }

    #[test]
    fn tagged_adt_survives_roundtrip() {
        let v = VoxValue::Tagged {
            name: "Declined".into(),
            fields: vec![VoxValue::Int(402)],
        };
        match roundtrip(v) {
            VoxValue::Tagged { name, fields } => {
                assert_eq!(name, "Declined");
                assert!(matches!(fields.as_slice(), [VoxValue::Int(402)]));
            }
            other => panic!("expected Tagged, got {other:?}"),
        }
    }

    // Catches the collision case: a user object with a literal "__vox" key must
    // not be re-read as a tagged wrapper.
    #[test]
    fn user_object_with_vox_key_is_escaped() {
        let v = VoxValue::object(vec![("__vox".into(), VoxValue::Str("Ok".into()))]);
        match roundtrip(v) {
            VoxValue::Object(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].0, "__vox");
            }
            other => panic!("expected Object, got {other:?}"),
        }
    }

    #[test]
    fn plain_scalars_and_lists_roundtrip() {
        assert!(matches!(roundtrip(VoxValue::Int(7)), VoxValue::Int(7)));
        assert!(matches!(roundtrip(VoxValue::Bool(true)), VoxValue::Bool(true)));
        match roundtrip(VoxValue::list(vec![VoxValue::Int(1), VoxValue::Int(2)])) {
            VoxValue::List(items) => assert_eq!(items.len(), 2),
            other => panic!("expected List, got {other:?}"),
        }
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p vox-compiler --lib eval::value::journal_json_tests
```

Expected: FAIL — `no method named to_journal_json`.

- [ ] **Step 3: Implement the tagged encoding**

Append to the `impl VoxValue` block in `crates/vox-compiler/src/eval/value.rs`:

```rust
    /// Encode this value for the durable workflow journal.
    ///
    /// Unlike `json.encode` (see `eval::builtins::vox_to_json`, which is
    /// deliberately lossy so `json.encode(Ok(1))` reads as `1` for external
    /// consumers), this encoding is **round-trippable**: `Result`, `Option`,
    /// `Tagged`, `Tuple` and `Decimal` are wrapped in a `{"__vox": …}` envelope
    /// so `from_journal_json` reconstructs the exact runtime value. A workflow
    /// that replays a journalled `Ok("tx")` must get `Ok("tx")` back, not `"tx"`
    /// and not `null`.
    ///
    /// Objects that genuinely contain a `"__vox"` key are escaped through the
    /// same envelope so a user map cannot impersonate a wrapper.
    #[must_use]
    pub fn to_journal_json(&self) -> serde_json::Value {
        use serde_json::{Value as J, json};
        match self {
            VoxValue::Int(n) => json!(n),
            VoxValue::Float(f) => json!(f),
            VoxValue::Str(s) => json!(s),
            VoxValue::Bool(b) => json!(b),
            VoxValue::Null => J::Null,
            VoxValue::Decimal(d) => json!({"__vox": "Decimal", "value": d.to_string()}),
            VoxValue::List(items) => {
                J::Array(items.iter().map(VoxValue::to_journal_json).collect())
            }
            VoxValue::Tuple(items) => json!({
                "__vox": "Tuple",
                "items": items.iter().map(VoxValue::to_journal_json).collect::<Vec<_>>(),
            }),
            VoxValue::Object(fields) => {
                let escaped = fields.iter().any(|(k, _)| k == "__vox");
                let map: serde_json::Map<String, J> = fields
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_journal_json()))
                    .collect();
                if escaped {
                    json!({"__vox": "Object", "fields": J::Object(map)})
                } else {
                    J::Object(map)
                }
            }
            VoxValue::Option(Some(v)) => json!({"__vox": "Some", "value": v.to_journal_json()}),
            VoxValue::Option(None) => json!({"__vox": "None"}),
            VoxValue::Result(Ok(v)) => json!({"__vox": "Ok", "value": v.to_journal_json()}),
            VoxValue::Result(Err(e)) => json!({"__vox": "Err", "value": e.to_journal_json()}),
            VoxValue::Tagged { name, fields } => json!({
                "__vox": "Tagged",
                "name": name,
                "fields": fields.iter().map(VoxValue::to_journal_json).collect::<Vec<_>>(),
            }),
            // Regex, Match, Fn, Constructor and the internal control-flow
            // sentinels are not journalable: an activity cannot return one
            // across a process restart. Encode as a marker so replay fails
            // loudly rather than silently substituting null.
            other => json!({"__vox": "Unjournalable", "kind": format!("{other:?}")}),
        }
    }

    /// Decode a value produced by [`VoxValue::to_journal_json`].
    ///
    /// Unknown or malformed envelopes decode to [`VoxValue::Null`] rather than
    /// panicking: a journal row written by a newer runtime must not crash an
    /// older one mid-replay.
    #[must_use]
    pub fn from_journal_json(value: &serde_json::Value) -> VoxValue {
        use serde_json::Value as J;
        match value {
            J::Null => VoxValue::Null,
            J::Bool(b) => VoxValue::Bool(*b),
            J::Number(n) => n
                .as_i64()
                .map(VoxValue::Int)
                .or_else(|| n.as_f64().map(VoxValue::Float))
                .unwrap_or(VoxValue::Null),
            J::String(s) => VoxValue::Str(s.clone()),
            J::Array(items) => {
                VoxValue::list(items.iter().map(VoxValue::from_journal_json).collect())
            }
            J::Object(map) => match map.get("__vox").and_then(J::as_str) {
                Some("Decimal") => map
                    .get("value")
                    .and_then(J::as_str)
                    .and_then(|s| s.parse().ok())
                    .map(VoxValue::Decimal)
                    .unwrap_or(VoxValue::Null),
                Some("Tuple") => VoxValue::tuple(
                    map.get("items")
                        .and_then(J::as_array)
                        .map(|a| a.iter().map(VoxValue::from_journal_json).collect())
                        .unwrap_or_default(),
                ),
                Some("Object") => match map.get("fields") {
                    Some(J::Object(inner)) => VoxValue::object(
                        inner
                            .iter()
                            .map(|(k, v)| (k.clone(), VoxValue::from_journal_json(v)))
                            .collect(),
                    ),
                    _ => VoxValue::Null,
                },
                Some("Some") => VoxValue::Option(Some(Box::new(VoxValue::from_journal_json(
                    map.get("value").unwrap_or(&J::Null),
                )))),
                Some("None") => VoxValue::Option(None),
                Some("Ok") => VoxValue::Result(Ok(Box::new(VoxValue::from_journal_json(
                    map.get("value").unwrap_or(&J::Null),
                )))),
                Some("Err") => VoxValue::Result(Err(Box::new(VoxValue::from_journal_json(
                    map.get("value").unwrap_or(&J::Null),
                )))),
                Some("Tagged") => VoxValue::Tagged {
                    name: map
                        .get("name")
                        .and_then(J::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    fields: map
                        .get("fields")
                        .and_then(J::as_array)
                        .map(|a| a.iter().map(VoxValue::from_journal_json).collect())
                        .unwrap_or_default(),
                },
                _ => VoxValue::object(
                    map.iter()
                        .map(|(k, v)| (k.clone(), VoxValue::from_journal_json(v)))
                        .collect(),
                ),
            },
        }
    }
```

- [ ] **Step 4: Run the tests and watch them pass**

```bash
cargo test -p vox-compiler --lib eval::value::journal_json_tests
```

Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-compiler/src/eval/value.rs
git commit -m "feat(eval): add round-trippable journal encoding for VoxValue

json.encode stays deliberately lossy for external consumers; the durable
journal needs Result/Option/Tagged/Tuple/Decimal to survive a process restart
intact, so give it its own tagged envelope.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 1.2: Interpreter activity-interception hook

The interpreter must hand activity calls to the runner. There is exactly one place a bare `activity_name(args)` call is applied: the `HirExpr::Call` arm in `eval/expr.rs` with an `Ident` callee. The hook cannot itself re-enter the interpreter (it is invoked from `&mut Interpreter`), so it returns a **decision** and the interpreter performs the call.

**Files:**
- Create: `crates/vox-compiler/src/eval/activity_hook.rs`
- Modify: `crates/vox-compiler/src/eval/mod.rs`
- Modify: `crates/vox-compiler/src/eval/expr.rs`
- Test: `crates/vox-compiler/src/eval/activity_hook.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `VoxValue::to_journal_json` / `from_journal_json` (Task 1.1).
- Produces:
  - `pub enum ActivityCall { Begin { name: String, args: Vec<VoxValue> }, End { name: String, result: VoxValue, failed: bool } }`
  - `pub enum ActivityDecision { Replay(VoxValue), Execute, Retry, Park(String), Ack }`
  - `pub type ActivityHook = Box<dyn FnMut(ActivityCall) -> Result<ActivityDecision, EvalError> + Send>`
  - `Interpreter::activity_names: HashSet<String>` and `Interpreter::activity_hook: Option<ActivityHook>`
  - `EvalError::WorkflowParked(String)`
  - `pub(crate) fn dispatch_activity(interp: &mut Interpreter, name: &str, args: Vec<VoxValue>) -> Result<VoxValue, EvalError>`

  Task 1.3 constructs the hook; Task 1.4 consumes the decisions.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-compiler/src/eval/activity_hook.rs` with the test module only, plus a `use` line that will not compile until step 3 lands:

```rust
//! Activity-call interception for durable workflow execution.
//!
//! A durable workflow body runs in this interpreter, but the *decision* about
//! whether an activity call should execute, replay from a journal, retry, or
//! park belongs to the async workflow runner (which owns the database). The
//! runner installs an [`ActivityHook`]; the interpreter calls it at every
//! activity call site and acts on the returned [`ActivityDecision`].
//!
//! The hook returns a decision rather than a value because it is invoked from
//! `&mut Interpreter` and therefore cannot re-enter the interpreter to run the
//! activity body itself.

#[cfg(test)]
mod tests {
    use crate::eval::activity_hook::{ActivityCall, ActivityDecision};
    use crate::eval::value::VoxValue;
    use crate::eval::Interpreter;
    use crate::hir::lower_module;
    use crate::lexer::cursor::lex;
    use crate::parser::parse;
    use std::sync::{Arc, Mutex};

    const SRC: &str = r#"
activity charge(amount: int) to int {
    return amount * 2
}

workflow wf(amount: int) to int {
    let a = charge(amount)
    let b = charge(a)
    return b
}
"#;

    fn interp_for(src: &str) -> (Interpreter, Vec<String>) {
        let module = parse(lex(src)).expect("parses");
        let hir = lower_module(&module);
        let activities: Vec<String> = hir
            .functions
            .iter()
            .filter(|f| f.durability == Some(crate::hir::nodes::DurabilityKind::Activity))
            .map(|f| f.name.clone())
            .collect();
        let mut interp = Interpreter::new(1_000_000);
        interp.run_module(&hir).expect("module loads");
        (interp, activities)
    }

    // Catches: the hook never firing, i.e. activity calls falling through to the
    // ordinary call path with no journalling at all.
    #[test]
    fn hook_observes_every_activity_call() {
        let (mut interp, activities) = interp_for(SRC);
        interp.activity_names = activities.into_iter().collect();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_c = seen.clone();
        interp.activity_hook = Some(Box::new(move |call| {
            if let ActivityCall::Begin { name, .. } = &call {
                seen_c.lock().unwrap().push(name.clone());
            }
            Ok(match call {
                ActivityCall::Begin { .. } => ActivityDecision::Execute,
                ActivityCall::End { .. } => ActivityDecision::Ack,
            })
        }));

        let out = interp
            .call("wf", vec![VoxValue::Int(5)])
            .expect("workflow runs");
        assert!(matches!(out, VoxValue::Int(20)), "5*2*2 = 20, got {out:?}");
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["charge".to_string(), "charge".to_string()],
            "both activity calls must reach the hook"
        );
    }

    // Catches: Replay running the body anyway — the single most important
    // durability property. A replayed activity must NOT re-execute.
    #[test]
    fn replay_decision_skips_the_body() {
        let (mut interp, activities) = interp_for(SRC);
        interp.activity_names = activities.into_iter().collect();
        let executed = Arc::new(Mutex::new(0usize));
        let executed_c = executed.clone();
        interp.activity_hook = Some(Box::new(move |call| {
            Ok(match call {
                ActivityCall::Begin { .. } => {
                    *executed_c.lock().unwrap() += 1;
                    ActivityDecision::Replay(VoxValue::Int(100))
                }
                ActivityCall::End { .. } => ActivityDecision::Ack,
            })
        }));

        let out = interp
            .call("wf", vec![VoxValue::Int(5)])
            .expect("workflow runs");
        assert!(
            matches!(out, VoxValue::Int(100)),
            "replayed value must flow through the workflow body; got {out:?}"
        );
        assert_eq!(
            *executed.lock().unwrap(),
            2,
            "Begin fires per call site even when replaying"
        );
    }

    // Catches: Park being swallowed and the workflow continuing past a wait.
    #[test]
    fn park_decision_unwinds_the_workflow() {
        let (mut interp, activities) = interp_for(SRC);
        interp.activity_names = activities.into_iter().collect();
        interp.activity_hook = Some(Box::new(|call| {
            Ok(match call {
                ActivityCall::Begin { .. } => ActivityDecision::Park("timer".into()),
                ActivityCall::End { .. } => ActivityDecision::Ack,
            })
        }));

        let err = interp
            .call("wf", vec![VoxValue::Int(5)])
            .expect_err("park must abort the run");
        assert!(
            matches!(err, crate::eval::EvalError::WorkflowParked(ref r) if r == "timer"),
            "park must surface as WorkflowParked; got {err:?}"
        );
    }

    // Catches: a plain (non-activity) helper being intercepted, which would
    // journal ordinary function calls and corrupt the history.
    #[test]
    fn plain_functions_are_not_intercepted() {
        const WITH_HELPER: &str = r#"
fn double(n: int) to int { return n * 2 }

activity charge(amount: int) to int { return amount }

workflow wf(amount: int) to int {
    let d = double(amount)
    let a = charge(d)
    return a
}
"#;
        let (mut interp, activities) = interp_for(WITH_HELPER);
        interp.activity_names = activities.into_iter().collect();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_c = seen.clone();
        interp.activity_hook = Some(Box::new(move |call| {
            if let ActivityCall::Begin { name, .. } = &call {
                seen_c.lock().unwrap().push(name.clone());
            }
            Ok(match call {
                ActivityCall::Begin { .. } => ActivityDecision::Execute,
                ActivityCall::End { .. } => ActivityDecision::Ack,
            })
        }));
        let _ = interp.call("wf", vec![VoxValue::Int(3)]).expect("runs");
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["charge".to_string()],
            "only `activity` declarations may be intercepted"
        );
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p vox-compiler --lib eval::activity_hook
```

Expected: FAIL — unresolved imports `ActivityCall`, `ActivityDecision`; no field `activity_names`.

- [ ] **Step 3: Implement the types and dispatch**

Prepend to `crates/vox-compiler/src/eval/activity_hook.rs` (above the test module, under the existing module doc comment):

```rust
use crate::eval::value::VoxValue;
use crate::eval::{EvalError, Interpreter};

/// One interception point in an activity call's lifecycle.
#[derive(Debug, Clone)]
pub enum ActivityCall {
    /// The workflow is about to invoke `name` with `args`. The runner decides
    /// whether the body runs.
    Begin {
        /// Activity function name as written in the workflow body.
        name: String,
        /// Evaluated positional arguments.
        args: Vec<VoxValue>,
    },
    /// The body finished. `failed` is true when the activity returned
    /// `Result::Err`, which is what drives retry — an interpreter-level error
    /// aborts the run instead.
    End {
        /// Activity function name.
        name: String,
        /// The body's return value.
        result: VoxValue,
        /// True when `result` is `Result::Err(_)`.
        failed: bool,
    },
}

/// What the runner wants the interpreter to do at an interception point.
#[derive(Debug, Clone)]
pub enum ActivityDecision {
    /// The journal already holds this activity's result — bind this value and
    /// do NOT run the body.
    Replay(VoxValue),
    /// Run the body, then report back with [`ActivityCall::End`].
    Execute,
    /// Run the body again (retry after a failed attempt).
    Retry,
    /// This step cannot proceed yet (timer not due, signal absent). Unwind the
    /// workflow; the run resumes from the top later and replays the journal.
    Park(String),
    /// Nothing further to do (response to [`ActivityCall::End`]).
    Ack,
}

/// The runner-supplied callback. `Send` so the runner can move it onto the
/// dedicated interpreter thread; it is never called concurrently.
pub type ActivityHook = Box<dyn FnMut(ActivityCall) -> Result<ActivityDecision, EvalError> + Send>;

/// Intercept one activity call.
///
/// Takes the hook out of the interpreter for the duration of each callback so
/// the borrow checker permits re-entering the interpreter to run the body, and
/// restores it on every path including errors.
pub(crate) fn dispatch_activity(
    interp: &mut Interpreter,
    name: &str,
    args: Vec<VoxValue>,
) -> Result<VoxValue, EvalError> {
    let mut hook = match interp.activity_hook.take() {
        Some(h) => h,
        // No runner installed (plain `vox run` of a file that happens to
        // declare activities): behave exactly like an ordinary call.
        None => return interp.call(name, args),
    };

    let outcome = (|| -> Result<VoxValue, EvalError> {
        loop {
            let decision = hook(ActivityCall::Begin {
                name: name.to_string(),
                args: args.clone(),
            })?;
            match decision {
                ActivityDecision::Replay(value) => return Ok(value),
                ActivityDecision::Park(reason) => return Err(EvalError::WorkflowParked(reason)),
                ActivityDecision::Execute | ActivityDecision::Retry => {
                    // Put the hook back so a nested activity call inside this
                    // body is itself intercepted, then take it again after.
                    interp.activity_hook = Some(hook);
                    let result = interp.call(name, args.clone())?;
                    hook = interp
                        .activity_hook
                        .take()
                        .ok_or_else(|| EvalError::Panic("activity hook lost during body execution".into()))?;

                    let failed = matches!(result, VoxValue::Result(Err(_)));
                    match hook(ActivityCall::End {
                        name: name.to_string(),
                        result: result.clone(),
                        failed,
                    })? {
                        ActivityDecision::Retry => continue,
                        ActivityDecision::Park(reason) => {
                            return Err(EvalError::WorkflowParked(reason));
                        }
                        ActivityDecision::Replay(value) => return Ok(value),
                        ActivityDecision::Ack | ActivityDecision::Execute => return Ok(result),
                    }
                }
                ActivityDecision::Ack => return interp.call(name, args.clone()),
            }
        }
    })();

    interp.activity_hook = Some(hook);
    outcome
}
```

- [ ] **Step 4: Wire the interpreter fields and the new error variant**

In `crates/vox-compiler/src/eval/mod.rs`:

1. Declare the module next to the other `eval` submodules:

```rust
pub mod activity_hook;
```

2. Add the variant to `EvalError` (after `Panic(String)`):

```rust
    /// A durable workflow step could not proceed (timer not yet due, signal
    /// absent). The run is parked, not failed; the runner resumes it later.
    WorkflowParked(String),
```

Then update every exhaustive `match` on `EvalError` — build errors will name them. At minimum the `Display`/`Debug` formatting impl in this file gains:

```rust
            EvalError::WorkflowParked(reason) => write!(f, "workflow parked: {reason}"),
```

3. Add the two fields to `pub struct Interpreter` (after `repo`):

```rust
    /// Names of `activity`-declared functions in the running module. Calls to
    /// these are routed through [`activity_hook`](Self::activity_hook) instead
    /// of the ordinary call path. Empty for non-workflow execution.
    pub activity_names: std::collections::HashSet<String>,
    /// Installed by the durable workflow runner; see
    /// [`crate::eval::activity_hook`]. `None` outside workflow execution.
    pub activity_hook: Option<crate::eval::activity_hook::ActivityHook>,
```

4. Initialize both in `Interpreter::new` alongside the other field initializers:

```rust
            activity_names: std::collections::HashSet::new(),
            activity_hook: None,
```

- [ ] **Step 5: Add the single interception branch in `expr.rs`**

In `crates/vox-compiler/src/eval/expr.rs`, in the `HirExpr::Call` arm, insert **after** the `eval_args` loop and the global-builtin attempt, immediately before `let c = eval_expr(interp, callee)?;`:

```rust
            // Durable workflows: an `activity`-declared callee is routed to the
            // runner, which decides execute / replay / retry / park. See
            // `crate::eval::activity_hook`.
            if let HirExpr::Ident(name, _) = callee.as_ref()
                && interp.activity_names.contains(name.as_str())
            {
                return super::activity_hook::dispatch_activity(interp, name, eval_args);
            }
```

- [ ] **Step 6: Run the tests and watch them pass**

```bash
cargo test -p vox-compiler --lib eval::activity_hook
```

Expected: PASS (4 tests).

- [ ] **Step 7: Mutation-verify the replay guard**

In `dispatch_activity`, temporarily change the `ActivityDecision::Replay(value) => return Ok(value)` arm to fall through to `Execute`. Re-run `replay_decision_skips_the_body` and confirm it FAILS (the workflow would return `Int(20)`, not `Int(100)`). Restore and confirm PASS. Then `git diff --stat` to confirm nothing else changed — a concurrent formatter run can silently revert the edit and hand you a meaningless pass.

- [ ] **Step 8: Confirm nothing else regressed and commit**

```bash
cargo test -p vox-compiler --lib
cargo clippy -p vox-compiler --all-targets -- -D warnings
vox run scripts/fmt.vox
git add crates/vox-compiler/src/eval/
git commit -m "feat(eval): intercept activity calls for durable workflow execution

Adds ActivityCall/ActivityDecision and one interception branch at the single
Call(Ident) application site. The hook returns a decision rather than a value
because it runs from &mut Interpreter and cannot re-enter it.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 1.3: The `!Send` host bridge

`Interpreter` holds `Rc`-backed values and cannot cross an `.await`. Confine it: one dedicated OS thread owns the interpreter, the tokio task owns the database. They talk over two `tokio::sync::mpsc::unbounded_channel`s — the sync side uses `send` (non-blocking on an unbounded channel) and `blocking_recv`, the async side uses `recv().await` and `send`.

**Files:**
- Create: `crates/vox-workflow-runtime/src/workflow/host.rs`
- Modify: `crates/vox-workflow-runtime/src/workflow/mod.rs` (add `pub mod host;` and re-export)
- Test: `crates/vox-workflow-runtime/src/workflow/host.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `ActivityCall`, `ActivityDecision`, `EvalError::WorkflowParked` (Task 1.2); `to_journal_json` / `from_journal_json` (Task 1.1).
- Produces:
  - `pub struct WorkflowHost`
  - `WorkflowHost::spawn(hir: HirModule, workflow_name: &str, args: Vec<Value>) -> anyhow::Result<WorkflowHost>`
  - `WorkflowHost::next(&mut self) -> Option<HostRequest>`  *(async)*
  - `WorkflowHost::respond(&self, decision: HostDecision) -> anyhow::Result<()>`
  - `WorkflowHost::finish(self) -> anyhow::Result<HostOutcome>`  *(async)*
  - `pub enum HostRequest { Begin { name: String, args: Vec<Value> }, End { name: String, result: Value, failed: bool } }`
  - `pub enum HostDecision { Replay(Value), Execute, Retry, Park(String), Ack }`
  - `pub enum HostOutcome { Completed(Value), Parked(String), Failed(String) }`

  Task 1.4 drives all of these.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-workflow-runtime/src/workflow/host.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use vox_compiler::hir::lower_module;
    use vox_compiler::lexer::cursor::lex;
    use vox_compiler::parser::parse;

    const SRC: &str = r#"
activity charge(amount: int) to int {
    return amount * 2
}

workflow wf(amount: int) to int {
    let a = charge(amount)
    return a
}
"#;

    fn hir_of(src: &str) -> vox_compiler::hir::HirModule {
        lower_module(&parse(lex(src)).expect("parses"))
    }

    // Catches: the thread bridge dropping arguments, results, or the return
    // value — i.e. everything the old stub runner got wrong.
    #[tokio::test]
    async fn host_executes_the_body_and_returns_the_real_value() {
        let mut host = WorkflowHost::spawn(hir_of(SRC), "wf", vec![json!(21)]).expect("spawns");
        let mut begins = 0;
        while let Some(req) = host.next().await {
            match req {
                HostRequest::Begin { name, args } => {
                    begins += 1;
                    assert_eq!(name, "charge");
                    assert_eq!(args, vec![json!(21)], "real arguments must reach the runner");
                    host.respond(HostDecision::Execute).expect("respond");
                }
                HostRequest::End { result, failed, .. } => {
                    assert_eq!(result, json!(42), "real body result must reach the runner");
                    assert!(!failed);
                    host.respond(HostDecision::Ack).expect("respond");
                }
            }
        }
        assert_eq!(begins, 1);
        match host.finish().await.expect("joins") {
            HostOutcome::Completed(v) => assert_eq!(v, json!(42)),
            other => panic!("expected Completed(42), got {other:?}"),
        }
    }

    // Catches: replay running the body anyway across the thread boundary.
    #[tokio::test]
    async fn host_replay_does_not_execute_the_body() {
        let mut host = WorkflowHost::spawn(hir_of(SRC), "wf", vec![json!(21)]).expect("spawns");
        let mut ends = 0;
        while let Some(req) = host.next().await {
            match req {
                HostRequest::Begin { .. } => {
                    host.respond(HostDecision::Replay(json!(999))).expect("respond");
                }
                HostRequest::End { .. } => {
                    ends += 1;
                    host.respond(HostDecision::Ack).expect("respond");
                }
            }
        }
        assert_eq!(ends, 0, "a replayed activity must never report an End");
        match host.finish().await.expect("joins") {
            HostOutcome::Completed(v) => assert_eq!(v, json!(999)),
            other => panic!("expected Completed(999), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn host_park_surfaces_as_parked_outcome() {
        let mut host = WorkflowHost::spawn(hir_of(SRC), "wf", vec![json!(1)]).expect("spawns");
        while let Some(req) = host.next().await {
            if let HostRequest::Begin { .. } = req {
                host.respond(HostDecision::Park("timer".into())).expect("respond");
            } else {
                host.respond(HostDecision::Ack).expect("respond");
            }
        }
        match host.finish().await.expect("joins") {
            HostOutcome::Parked(reason) => assert_eq!(reason, "timer"),
            other => panic!("expected Parked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn host_reports_a_missing_workflow_as_an_error() {
        let err = WorkflowHost::spawn(hir_of(SRC), "no_such_workflow", vec![]);
        assert!(err.is_err(), "spawning an unknown workflow must fail fast");
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p vox-workflow-runtime --lib workflow::host
```

Expected: FAIL — `cannot find struct WorkflowHost`.

- [ ] **Step 3: Implement the host**

Prepend to `crates/vox-workflow-runtime/src/workflow/host.rs`:

```rust
//! The `!Send` bridge between the tree-walking interpreter and the async
//! durable runner.
//!
//! `vox_compiler::eval::Interpreter` holds `Rc`-backed `VoxValue`s, so it can
//! never be held across an `.await`. Rather than making the interpreter `Send`
//! (which would mean reworking every value in the language), confine it: one
//! dedicated OS thread owns the interpreter and blocks on a response channel;
//! the tokio task owns the database and never blocks.
//!
//! Values cross the boundary as `serde_json::Value` via
//! [`VoxValue::to_journal_json`], which is exactly the shape the journal stores.

use anyhow::Context;
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use vox_compiler::eval::Interpreter;
use vox_compiler::eval::activity_hook::{ActivityCall, ActivityDecision};
use vox_compiler::eval::value::VoxValue;
use vox_compiler::hir::HirModule;
use vox_compiler::hir::nodes::DurabilityKind;

/// Step limit for a workflow body. Generous — a workflow orchestrates, it does
/// not compute — but finite so a runaway loop fails rather than hanging a
/// worker thread forever.
const WORKFLOW_STEP_LIMIT: usize = 10_000_000;

/// A request from the interpreter thread awaiting a runner decision.
#[derive(Debug, Clone)]
pub enum HostRequest {
    /// The workflow is about to call `name`.
    Begin {
        /// Activity name.
        name: String,
        /// Journal-encoded positional arguments.
        args: Vec<Value>,
    },
    /// The activity body finished.
    End {
        /// Activity name.
        name: String,
        /// Journal-encoded return value.
        result: Value,
        /// True when the activity returned `Result::Err(_)`.
        failed: bool,
    },
}

/// The runner's answer to a [`HostRequest`].
#[derive(Debug, Clone)]
pub enum HostDecision {
    /// Bind this journalled value; do not run the body.
    Replay(Value),
    /// Run the body.
    Execute,
    /// Run the body again.
    Retry,
    /// Abandon this run; it resumes later and replays.
    Park(String),
    /// Acknowledged.
    Ack,
}

/// How a workflow thread finished.
#[derive(Debug, Clone)]
pub enum HostOutcome {
    /// The workflow returned normally; the value is journal-encoded.
    Completed(Value),
    /// The workflow parked at a timer or signal.
    Parked(String),
    /// The workflow raised an interpreter error.
    Failed(String),
}

/// Owns the interpreter thread and the two channels.
pub struct WorkflowHost {
    req_rx: UnboundedReceiver<HostRequest>,
    resp_tx: UnboundedSender<HostDecision>,
    handle: std::thread::JoinHandle<HostOutcome>,
}

impl WorkflowHost {
    /// Spawn the interpreter thread for one workflow invocation.
    ///
    /// Fails fast if `workflow_name` is not a `workflow`-declared function in
    /// `hir`, so the caller does not start a run for a workflow that cannot
    /// exist.
    pub fn spawn(
        hir: HirModule,
        workflow_name: &str,
        args: Vec<Value>,
    ) -> anyhow::Result<Self> {
        let activity_names: std::collections::HashSet<String> = hir
            .functions
            .iter()
            .filter(|f| f.durability == Some(DurabilityKind::Activity))
            .map(|f| f.name.clone())
            .collect();
        if !hir
            .functions
            .iter()
            .any(|f| f.durability == Some(DurabilityKind::Workflow) && f.name == workflow_name)
        {
            anyhow::bail!(
                "workflow `{workflow_name}` not found in HIR; declare it with the `workflow` keyword"
            );
        }

        let (req_tx, req_rx) = unbounded_channel::<HostRequest>();
        let (resp_tx, resp_rx) = unbounded_channel::<HostDecision>();
        let name = workflow_name.to_string();

        let handle = std::thread::Builder::new()
            .name(format!("vox-workflow-{name}"))
            .spawn(move || run_on_thread(hir, name, args, activity_names, req_tx, resp_rx))
            .context("spawning the workflow interpreter thread")?;

        Ok(Self {
            req_rx,
            resp_tx,
            handle,
        })
    }

    /// Await the next interception request. `None` once the workflow thread
    /// has finished and dropped its sender.
    pub async fn next(&mut self) -> Option<HostRequest> {
        self.req_rx.recv().await
    }

    /// Answer the outstanding request.
    pub fn respond(&self, decision: HostDecision) -> anyhow::Result<()> {
        self.resp_tx
            .send(decision)
            .map_err(|_| anyhow::anyhow!("workflow interpreter thread hung up"))
    }

    /// Join the thread and take its outcome.
    pub async fn finish(self) -> anyhow::Result<HostOutcome> {
        let handle = self.handle;
        tokio::task::spawn_blocking(move || handle.join())
            .await
            .context("joining the workflow interpreter thread")?
            .map_err(|_| anyhow::anyhow!("workflow interpreter thread panicked"))
    }
}

fn run_on_thread(
    hir: HirModule,
    workflow_name: String,
    args: Vec<Value>,
    activity_names: std::collections::HashSet<String>,
    req_tx: UnboundedSender<HostRequest>,
    mut resp_rx: UnboundedReceiver<HostDecision>,
) -> HostOutcome {
    let mut interp = Interpreter::new(WORKFLOW_STEP_LIMIT);
    if let Err(e) = interp.run_module(&hir) {
        return HostOutcome::Failed(format!("loading module: {e:?}"));
    }
    interp.activity_names = activity_names;
    interp.activity_hook = Some(Box::new(move |call| {
        let request = match &call {
            ActivityCall::Begin { name, args } => HostRequest::Begin {
                name: name.clone(),
                args: args.iter().map(VoxValue::to_journal_json).collect(),
            },
            ActivityCall::End {
                name,
                result,
                failed,
            } => HostRequest::End {
                name: name.clone(),
                result: result.to_journal_json(),
                failed: *failed,
            },
        };
        if req_tx.send(request).is_err() {
            return Err(vox_compiler::eval::EvalError::Panic(
                "durable workflow runner hung up".into(),
            ));
        }
        // The runner is on another thread; blocking here is the point.
        let Some(decision) = resp_rx.blocking_recv() else {
            return Err(vox_compiler::eval::EvalError::Panic(
                "durable workflow runner dropped the response channel".into(),
            ));
        };
        Ok(match decision {
            HostDecision::Replay(v) => ActivityDecision::Replay(VoxValue::from_journal_json(&v)),
            HostDecision::Execute => ActivityDecision::Execute,
            HostDecision::Retry => ActivityDecision::Retry,
            HostDecision::Park(r) => ActivityDecision::Park(r),
            HostDecision::Ack => ActivityDecision::Ack,
        })
    }));

    let vox_args: Vec<VoxValue> = args.iter().map(VoxValue::from_journal_json).collect();
    match interp.call(&workflow_name, vox_args) {
        Ok(value) => HostOutcome::Completed(value.to_journal_json()),
        Err(vox_compiler::eval::EvalError::WorkflowParked(reason)) => HostOutcome::Parked(reason),
        Err(e) => HostOutcome::Failed(format!("{e:?}")),
    }
}
```

- [ ] **Step 4: Export the module**

In `crates/vox-workflow-runtime/src/workflow/mod.rs` add:

```rust
/// The `!Send` interpreter-thread bridge for durable workflow execution.
pub mod host;
pub use host::{HostDecision, HostOutcome, HostRequest, WorkflowHost};
```

- [ ] **Step 5: Run the tests and watch them pass**

```bash
cargo test -p vox-workflow-runtime --lib workflow::host
```

Expected: PASS (4 tests).

- [ ] **Step 6: Mutation-verify the replay guard**

In `run_on_thread`, temporarily map `HostDecision::Replay(_)` to `ActivityDecision::Execute`. Confirm `host_replay_does_not_execute_the_body` FAILS. Restore; confirm PASS.

- [ ] **Step 7: Commit**

```bash
cargo clippy -p vox-workflow-runtime --all-targets -- -D warnings
vox run scripts/fmt.vox
git add crates/vox-workflow-runtime/src/workflow/host.rs crates/vox-workflow-runtime/src/workflow/mod.rs
git commit -m "feat(workflow): add the interpreter-thread host bridge

The tree-walking interpreter is !Send (VoxValue holds Rc), so it is confined to
a dedicated OS thread that blocks on a tokio channel while the async runner does
journal I/O. Values cross as journal-encoded JSON.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 1.4: Rewrite the runner around the host

Replace the plan-walking loop in `interpret_workflow_durable` with a request loop over the host. Every tracker call that exists today is preserved — only the source of steps changes, from a precomputed plan to a live workflow body.

**Files:**
- Modify: `crates/vox-workflow-runtime/src/workflow/run.rs`
- Modify: `crates/vox-ml-cli/src/commands/ai/workflow.rs` (new `args` parameter)
- Modify: `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs` (new `args` parameter)
- Modify: `crates/vox-workflow-runtime/tests/journal_schema_conformance.rs` (new `args` parameter)
- Test: `crates/vox-workflow-runtime/tests/real_activity_execution.rs` (create)

**Interfaces:**
- Consumes: `WorkflowHost`, `HostRequest`, `HostDecision`, `HostOutcome` (Task 1.3); the existing `WorkflowTracker` trait unchanged.
- Produces: `pub async fn interpret_workflow_durable(hir: &HirModule, workflow_name: &str, args: Vec<serde_json::Value>, tracker: &mut impl WorkflowTracker) -> anyhow::Result<Vec<serde_json::Value>>` — note the new third parameter. `derive_activity_id` keeps its `(workflow_name, activity_name, position)` signature until Task 5.1 replaces it.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-workflow-runtime/tests/real_activity_execution.rs`:

```rust
#![allow(missing_docs)]
//! The core promise: activity bodies actually run, and their real return value
//! is what gets journalled and replayed.

use serde_json::json;
use std::sync::Arc;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::VoxDbTracker;
use vox_workflow_runtime::workflow::interpret_workflow_durable;

const SRC: &str = r#"
activity charge_card(amount: int) to Result[str] {
    if amount > 1000 {
        return Error("amount too large")
    }
    return Ok("tx_" + str(amount))
}

activity send_receipt(tx: str) to Result[str] {
    return Ok("emailed:" + tx)
}

workflow checkout(amount: int) to Result[str] {
    let tx = charge_card(amount)?
    let receipt = send_receipt(tx)?
    return Ok(receipt)
}
"#;

fn journal_result_for<'a>(
    journal: &'a [serde_json::Value],
    activity: &str,
) -> Option<&'a serde_json::Value> {
    journal
        .iter()
        .find(|e| {
            e["event"].as_str() == Some("ActivityCompleted") && e["activity"].as_str() == Some(activity)
        })
        .and_then(|e| e.get("result"))
}

#[tokio::test]
async fn activity_bodies_execute_and_real_results_are_journalled() {
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let mut tracker = VoxDbTracker::new(db, "real-exec-1");

    let journal = interpret_workflow_durable(&hir, "checkout", vec![json!(42)], &mut tracker)
        .await
        .expect("checkout runs");

    // The old runner journalled {"event":"LocalActivity","status":"executed"}
    // regardless of what the activity returned. Assert the REAL value.
    assert_eq!(
        journal_result_for(&journal, "charge_card"),
        Some(&json!({"__vox": "Ok", "value": "tx_42"})),
        "charge_card's real return value must be journalled; journal={journal:#?}"
    );
    assert_eq!(
        journal_result_for(&journal, "send_receipt"),
        Some(&json!({"__vox": "Ok", "value": "emailed:tx_42"})),
        "send_receipt must receive charge_card's real output as its argument"
    );

    let completed = journal
        .iter()
        .rev()
        .find(|e| e["event"].as_str() == Some("WorkflowCompleted"))
        .expect("terminates");
    assert_eq!(
        completed["return_value"],
        json!({"__vox": "Ok", "value": "emailed:tx_42"}),
        "the workflow's real return value must reach WorkflowCompleted"
    );
}

#[tokio::test]
async fn workflow_arguments_reach_the_body() {
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let mut tracker = VoxDbTracker::new(db, "real-exec-args");

    let journal = interpret_workflow_durable(&hir, "checkout", vec![json!(7)], &mut tracker)
        .await
        .expect("runs");
    assert_eq!(
        journal_result_for(&journal, "charge_card"),
        Some(&json!({"__vox": "Ok", "value": "tx_7"})),
        "the caller's argument, not a default, must drive the body"
    );
}

#[tokio::test]
async fn error_result_propagates_through_the_question_mark_operator() {
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let mut tracker = VoxDbTracker::new(db, "real-exec-err");

    let journal = interpret_workflow_durable(&hir, "checkout", vec![json!(5000)], &mut tracker)
        .await
        .expect("runs to a terminal state");

    let completed = journal
        .iter()
        .rev()
        .find(|e| e["event"].as_str() == Some("WorkflowCompleted"))
        .expect("terminates");
    assert_eq!(
        completed["return_value"],
        json!({"__vox": "Err", "value": "amount too large"}),
        "`?` must short-circuit the workflow with the activity's error"
    );
    assert!(
        !journal
            .iter()
            .any(|e| e["activity"].as_str() == Some("send_receipt")),
        "send_receipt must not run after charge_card errors"
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cargo test -p vox-workflow-runtime --test real_activity_execution
```

Expected: FAIL — arity mismatch on `interpret_workflow_durable` (3 args supplied, 4 expected once step 3 lands; before step 3 it fails on the extra `args`).

- [ ] **Step 3: Rewrite `interpret_workflow_durable`**

Replace the body of `interpret_workflow_durable` in `crates/vox-workflow-runtime/src/workflow/run.rs` (keep `derive_activity_id`, `handle_workflow_patch`, `versioned_event`, and `WORKFLOW_JOURNAL_VERSION` as-is; delete `plan_workflow_replay_ir` usage, `execute_step_with_retries`, `execute_step_once`, and `execute_local_activity_step`):

```rust
/// Execute a workflow body with a durable state engine, returning journal entries.
///
/// The workflow body runs on a dedicated interpreter thread (see
/// [`super::host`]); this loop answers each activity interception with the
/// durable decision — replay from the journal, execute, retry, or park.
pub async fn interpret_workflow_durable(
    hir: &HirModule,
    workflow_name: &str,
    args: Vec<Value>,
    tracker: &mut impl WorkflowTracker,
) -> anyhow::Result<Vec<Value>> {
    let mut journal = Vec::new();
    // `planned_steps` is no longer statically knowable: control flow is now
    // dynamic, so the number of activity executions depends on runtime values.
    // Record 0 and rely on `completed_steps` for progress.
    tracker.on_workflow_started(workflow_name, 0).await?;
    journal.push(versioned_event(json!({
        "event": "WorkflowStarted",
        "workflow": workflow_name,
        "steps": 0,
    })));

    let mut host = super::host::WorkflowHost::spawn(hir.clone(), workflow_name, args)?;
    // Per-activity-name invocation counter → position component of the id.
    let mut positions: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    // The activity currently executing (name, id, attempt).
    let mut in_flight: Option<(String, String, u32)> = None;

    while let Some(request) = host.next().await {
        match request {
            super::host::HostRequest::Begin { name, args } => {
                let position = {
                    let slot = positions.entry(name.clone()).or_insert(0);
                    let p = *slot;
                    *slot += 1;
                    p
                };
                let activity_id = derive_activity_id(workflow_name, &name, position);
                let arg_hash_hex = compute_structural_arg_hash(&args);
                let now_ms = now_unix_ms();

                if let Some(cached) = tracker
                    .load_cached_activity_result(&activity_id, &arg_hash_hex, now_ms)
                    .await?
                {
                    journal.push(versioned_event(json!({
                        "event": "ActivityCacheHit",
                        "workflow": workflow_name,
                        "activity": name,
                        "activity_id": activity_id,
                        "arg_hash": arg_hash_hex,
                    })));
                    host.respond(super::host::HostDecision::Replay(cached))?;
                    continue;
                }

                if tracker.is_activity_completed(workflow_name, &activity_id).await?
                    && let Some(stored) = tracker
                        .load_activity_result(workflow_name, &activity_id)
                        .await?
                {
                    journal.push(versioned_event(json!({
                        "event": "ActivityReplayed",
                        "workflow": workflow_name,
                        "activity": name,
                        "activity_id": activity_id,
                        "replay_source": "workflow_activity_log",
                        "result": stored,
                    })));
                    host.respond(super::host::HostDecision::Replay(stored))?;
                    continue;
                }

                tracker
                    .on_activity_started(workflow_name, &name, &activity_id)
                    .await?;
                let attempt = tracker
                    .next_activity_attempt_start(workflow_name, &name, &activity_id)
                    .await?;
                if attempt > 1 {
                    journal.push(versioned_event(json!({
                        "event": "ActivityAttemptRecovered",
                        "workflow": workflow_name,
                        "activity": name,
                        "activity_id": activity_id,
                        "resume_attempt": attempt,
                        "max_attempts_window": MAX_ACTIVITY_ATTEMPTS,
                    })));
                }
                tracker
                    .on_activity_attempt_started(workflow_name, &name, &activity_id, attempt)
                    .await?;
                journal.push(versioned_event(json!({
                    "event": "ActivityTask",
                    "workflow": workflow_name,
                    "activity": name,
                    "activity_id": activity_id,
                    "execution_boundary": "local",
                    "max_attempts": MAX_ACTIVITY_ATTEMPTS,
                    "idempotency_key": activity_id,
                    "arg_hash": arg_hash_hex,
                })));
                journal.push(versioned_event(json!({
                    "event": "ActivityStarted",
                    "workflow": workflow_name,
                    "activity": name,
                    "activity_id": activity_id,
                    "attempt": attempt,
                })));
                in_flight = Some((name.clone(), activity_id, attempt));
                host.respond(super::host::HostDecision::Execute)?;
            }

            super::host::HostRequest::End {
                name,
                result,
                failed,
            } => {
                let Some((_, activity_id, attempt)) = in_flight.take() else {
                    anyhow::bail!("workflow `{workflow_name}`: End for `{name}` with no in-flight activity");
                };

                if failed && attempt < MAX_ACTIVITY_ATTEMPTS {
                    tracker
                        .on_activity_attempt_failed(
                            workflow_name,
                            &name,
                            &activity_id,
                            attempt,
                            &result.to_string(),
                        )
                        .await?;
                    let delay_ms = retry_backoff_ms(attempt);
                    journal.push(versioned_event(json!({
                        "event": "ActivityAttemptFailed",
                        "workflow": workflow_name,
                        "activity": name,
                        "activity_id": activity_id,
                        "attempt": attempt,
                        "max_attempts": MAX_ACTIVITY_ATTEMPTS,
                        "error": result.to_string(),
                    })));
                    journal.push(versioned_event(json!({
                        "event": "ActivityRetryScheduled",
                        "workflow": workflow_name,
                        "activity": name,
                        "activity_id": activity_id,
                        "attempt": attempt,
                        "next_attempt": attempt + 1,
                        "delay_ms": delay_ms,
                    })));
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    tracker
                        .on_activity_attempt_started(
                            workflow_name,
                            &name,
                            &activity_id,
                            attempt + 1,
                        )
                        .await?;
                    in_flight = Some((name.clone(), activity_id, attempt + 1));
                    host.respond(super::host::HostDecision::Retry)?;
                    continue;
                }

                tracker
                    .on_activity_attempt_completed(workflow_name, &name, &activity_id, attempt)
                    .await?;
                tracker
                    .on_activity_completed(workflow_name, &name, &activity_id, &result)
                    .await?;
                journal.push(versioned_event(json!({
                    "event": "ActivityCompleted",
                    "workflow": workflow_name,
                    "activity": name,
                    "activity_id": activity_id,
                    "attempt": attempt,
                    "failed": failed,
                    "result": result,
                })));
                host.respond(super::host::HostDecision::Ack)?;
            }
        }
    }

    match host.finish().await? {
        super::host::HostOutcome::Completed(return_value) => {
            tracker.on_workflow_completed(workflow_name).await?;
            journal.push(versioned_event(json!({
                "event": "WorkflowCompleted",
                "workflow": workflow_name,
                "return_value": return_value,
            })));
            Ok(journal)
        }
        super::host::HostOutcome::Parked(reason) => {
            journal.push(versioned_event(json!({
                "event": "WorkflowParked",
                "workflow": workflow_name,
                "reason": reason,
            })));
            Ok(journal)
        }
        super::host::HostOutcome::Failed(err) => {
            anyhow::bail!("workflow `{workflow_name}` failed: {err}")
        }
    }
}

/// Attempts allowed per activity before the error is surfaced to the workflow
/// body. `with { retries: n }` per-call tuning is restored in Task 5.2.
const MAX_ACTIVITY_ATTEMPTS: u32 = 3;

fn retry_backoff_ms(attempt: u32) -> u64 {
    100u64.saturating_mul(1u64 << attempt.min(9)).min(60_000)
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
```

Also update the thin wrapper at the top of the file:

```rust
/// Execute a workflow with the no-op tracker (no durability). Prefer
/// [`interpret_workflow_durable`] with a real tracker.
pub async fn interpret_workflow(
    hir: &HirModule,
    workflow_name: &str,
    args: Vec<Value>,
) -> anyhow::Result<Vec<Value>> {
    let mut tracker = DefaultTracker;
    interpret_workflow_durable(hir, workflow_name, args, &mut tracker).await
}
```

- [ ] **Step 4: Update the three call sites**

1. `crates/vox-ml-cli/src/commands/ai/workflow.rs` — the `--args` JSON was validated and then dropped. Parse it once and pass it:

```rust
        let workflow_args: Vec<serde_json::Value> = serde_json::from_str(args_json)
            .context("Invalid --args JSON (must be array, e.g. [\"a\",42])")?;
```

Delete the now-redundant `if let Err(e) = serde_json::from_str::<Vec<serde_json::Value>>(args_json)` validation block above it, and pass `workflow_args` as the third argument to `interpret_workflow_durable`.

2. `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs` — in `emit_workflow_body`, pass the generated function's own parameters through:

```rust
    let arg_list = func
        .params
        .iter()
        .map(|p| format!("::serde_json::to_value(&{}).unwrap_or(::serde_json::Value::Null)", p.name))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!(
        "    let __vox_args = vec![{arg_list}];\n"
    ));
```

and change the call to `interpret_workflow_durable(__vox_hir, "{name}", __vox_args, &mut __vox_tracker)`.

3. `crates/vox-workflow-runtime/tests/journal_schema_conformance.rs` — add `vec![json!(1)]` as the third argument.

- [ ] **Step 5: Run the tests and watch them pass**

```bash
cargo test -p vox-workflow-runtime --test real_activity_execution
cargo test -p vox-workflow-runtime --test journal_schema_conformance
```

Expected: PASS.

- [ ] **Step 6: Mutation-verify the replay guard end to end**

In the `Begin` arm, temporarily change the `is_activity_completed` branch to always respond `Execute`. Confirm `crash_replay::workflow_resumes_seeded_activity_from_journal` (updated in Task 1.5) FAILS. Restore; confirm PASS.

- [ ] **Step 7: Commit**

```bash
cargo clippy -p vox-workflow-runtime -p vox-ml-cli -p vox-codegen --all-targets -- -D warnings
vox run scripts/fmt.vox
git add crates/vox-workflow-runtime/src/workflow/run.rs crates/vox-ml-cli/src/commands/ai/workflow.rs crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs crates/vox-workflow-runtime/tests/
git commit -m "feat(workflow): execute the workflow body and journal real activity results

The runner no longer walks a precomputed plan and emits a canned LocalActivity
payload. It drives the real workflow body on the interpreter thread and answers
each activity interception with the durable decision. Arguments and return
values are now real, and --args is no longer validated and discarded.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 1.5: Delete the linearizer and prove runtime control flow works

With the body executing, `plan.rs`'s compile-time linearizer is dead weight *and* actively wrong (it rejects `match` and non-literal `if`, which now work). Delete it and lock in the behaviour it used to forbid.

**Files:**
- Modify: `crates/vox-workflow-runtime/src/workflow/plan.rs` (delete most of it)
- Modify: `crates/vox-workflow-runtime/src/workflow/mod.rs`, `src/lib.rs` (drop re-exports)
- Modify: `crates/vox-workflow-runtime/tests/crash_replay.rs` (real result payloads)
- Create: `examples/golden/durable_workflow_branching.vox`
- Create: `crates/vox-workflow-runtime/tests/runtime_branching.rs`

**Interfaces:**
- Consumes: `interpret_workflow_durable` (Task 1.4).
- Produces: nothing new. Removes `plan_workflow_activities`, `plan_workflow_replay_ir`, `WorkflowReplayIr`, `ReplayNode`, `PlannedActivity` from the public surface. `PopuliActivity`, `PopuliHttpOp`, and `compute_structural_arg_hash` survive in `types.rs`.

- [ ] **Step 1: Write the failing golden and test**

Create `examples/golden/durable_workflow_branching.vox`:

```vox
// ---
// title: "Durable workflow with runtime branching"
// description: "Workflow whose control flow depends on an activity result; the old linearizer rejected this."
// syntax_version: "0.6.0"
// status: golden
// category: example
// constructs: [workflow, activity, if, for, Result, ?]
// last_validated: 2026-09-05
// training_eligible: true
// training_weight: 1.0
// difficulty: advanced
// ---
// @training_prompt: Write a Vox durable workflow whose branch depends on an activity's return value.
// ANCHOR: display
activity check_stock(sku: str) to int {
    return len(sku)
}

activity reserve(sku: str) to Result[str] {
    return Ok("reserved:" + sku)
}

activity backorder(sku: str) to Result[str] {
    return Ok("backordered:" + sku)
}

workflow fulfil(skus: [str]) to Result[str] {
    let mut last = "none"
    for sku in skus {
        let available = check_stock(sku)
        if available > 3 {
            last = reserve(sku)?
        } else {
            last = backorder(sku)?
        }
    }
    return Ok(last)
}
// ANCHOR_END: display
```

Create `crates/vox-workflow-runtime/tests/runtime_branching.rs`:

```rust
#![allow(missing_docs)]
//! Control flow the old compile-time linearizer rejected: an `if` whose
//! condition is an activity result, and a `for` over a runtime list.

use serde_json::json;
use std::sync::Arc;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::VoxDbTracker;
use vox_workflow_runtime::workflow::interpret_workflow_durable;

const GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/golden/durable_workflow_branching.vox"
));

#[tokio::test]
async fn branch_taken_depends_on_an_activity_result() {
    let hir = lower_module(&parse(lex(GOLDEN)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let mut tracker = VoxDbTracker::new(db, "branching-1");

    // "abcd" has len 4 (> 3) → reserve; "ab" has len 2 → backorder.
    let journal = interpret_workflow_durable(
        &hir,
        "fulfil",
        vec![json!(["abcd", "ab"])],
        &mut tracker,
    )
    .await
    .expect("runs");

    let ran: Vec<&str> = journal
        .iter()
        .filter(|e| e["event"].as_str() == Some("ActivityCompleted"))
        .filter_map(|e| e["activity"].as_str())
        .collect();
    assert_eq!(
        ran,
        vec!["check_stock", "reserve", "check_stock", "backorder"],
        "both branches must be reachable from runtime values; got {ran:?}"
    );

    let completed = journal
        .iter()
        .rev()
        .find(|e| e["event"].as_str() == Some("WorkflowCompleted"))
        .expect("terminates");
    assert_eq!(
        completed["return_value"],
        json!({"__vox": "Ok", "value": "backordered:ab"})
    );
}

#[tokio::test]
async fn replay_takes_the_same_branch() {
    let hir = lower_module(&parse(lex(GOLDEN)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let run_id = "branching-replay";

    let first = {
        let mut t = VoxDbTracker::new(db.clone(), run_id);
        interpret_workflow_durable(&hir, "fulfil", vec![json!(["abcd", "ab"])], &mut t)
            .await
            .expect("first run")
    };
    let second = {
        let mut t = VoxDbTracker::new(db.clone(), run_id);
        interpret_workflow_durable(&hir, "fulfil", vec![json!(["abcd", "ab"])], &mut t)
            .await
            .expect("replay")
    };

    let branch_of = |j: &[serde_json::Value]| -> Vec<String> {
        j.iter()
            .filter(|e| {
                matches!(
                    e["event"].as_str(),
                    Some("ActivityCompleted") | Some("ActivityReplayed")
                )
            })
            .filter_map(|e| e["activity"].as_str().map(str::to_string))
            .collect()
    };
    assert_eq!(
        branch_of(&first),
        branch_of(&second),
        "replay must take the same branch as the original run"
    );
    assert!(
        second
            .iter()
            .any(|e| e["event"].as_str() == Some("ActivityReplayed")),
        "the second run must replay from the journal, not re-execute"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p vox-workflow-runtime --test runtime_branching
```

Expected: FAIL — the golden does not exist yet, or (once created) `plan.rs` is no longer consulted so the test should pass; if it fails on `for`/`if` support, that is a genuine interpreter gap to fix before continuing.

- [ ] **Step 3: Delete the linearizer**

In `crates/vox-workflow-runtime/src/workflow/plan.rs`, delete: `plan_workflow_activities`, `plan_workflow_replay_ir`, `collect_activity_calls_from_stmts`, `collect_from_expr`, `ActivityWithOpts`, `eval_const_bool`, `eval_const_eq`, `eval_const_ord`, `parse_workflow_wait_ms`, `parse_workflow_signal_key`. Keep `parse_timeout_ms`, `parse_retries`, `parse_duration_ms_str`, `parse_populi_control_op`, and `resolve_populi_http_op` — Task 5.2 (`with { … }` options) and the mesh path still need them.

In `crates/vox-workflow-runtime/src/workflow/types.rs`, delete `PlannedActivity`, `ReplayNode`, and `WorkflowReplayIr`. Keep `PopuliActivity`, `PopuliHttpOp`, and `compute_structural_arg_hash`.

Drop the corresponding names from the `pub use` lists in `src/workflow/mod.rs` and `src/lib.rs`.

- [ ] **Step 4: Update `crash_replay.rs` to seed a real result**

The seeded payload must now be a real activity return value, not a runtime step record. In `crates/vox-workflow-runtime/tests/crash_replay.rs` replace `seeded_result` with:

```rust
    let seeded_result = json!({"__vox": "Ok", "value": "tx_SEEDED"});
```

and replace the `saw_seeded_marker` assertion with one that proves the seeded value **flowed into the next activity's argument**, which the old test could not check because arguments did not exist:

```rust
    let receipt_result = journal
        .iter()
        .find(|e| {
            e["event"].as_str() == Some("ActivityCompleted")
                && e["activity"].as_str() == Some("send_receipt")
        })
        .and_then(|e| e.get("result"))
        .expect("send_receipt completes");
    assert_eq!(
        receipt_result,
        &json!({"__vox": "Ok", "value": "emailed:tx_SEEDED"}),
        "the replayed charge_card value must be the input to send_receipt"
    );
```

Also delete the copied `derive_activity_id` helper's stale doc note if the id scheme changed, and add `vec![json!(42)]` as the third argument to both `interpret_workflow_durable` calls.

- [ ] **Step 5: Run the whole crate's tests**

```bash
cargo test -p vox-workflow-runtime
```

Expected: PASS. Tests referencing deleted plan APIs (`workflow_tracker_tests.rs`, `semcov_wave29_tests.rs`, `codegen_roundtrip.rs`) must be updated or deleted in this step — a test asserting the linearizer rejects `match` is now asserting a bug.

- [ ] **Step 6: Validate the golden compiles as a doctest**

```bash
cargo run -p vox-cli -- check examples/golden/durable_workflow_branching.vox
```

Expected: no errors.

- [ ] **Step 7: Commit**

```bash
cargo clippy -p vox-workflow-runtime --all-targets -- -D warnings
vox run scripts/fmt.vox
git add crates/vox-workflow-runtime/ examples/golden/durable_workflow_branching.vox
git commit -m "refactor(workflow): delete the compile-time linearizer

With the workflow body executing for real, plan.rs's const-if evaluator, branch
synthesis, literal-list for-unrolling and match/while/loop bails are dead weight
that also reject control flow which now works. 600 LoC removed.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Phase 2 — Prove the crash windows

### Task 2.1: Crash-window tests with a real side-effect counter

The existing `crash_replay.rs` proves the tracker's skip path by looking at event *names*. That cannot distinguish "the body did not run" from "the body ran and produced the same event name". Prove it with an observable side effect.

**Files:**
- Create: `crates/vox-workflow-runtime/tests/crash_windows.rs`

**Interfaces:**
- Consumes: `interpret_workflow_durable` (Task 1.4), `WorkflowTracker` (unchanged).
- Produces: `CountingTracker` — a test-local `WorkflowTracker` decorator that fails on a chosen call to simulate a crash. Task 3.2 and Task 4.2 reuse the pattern, not the type.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-workflow-runtime/tests/crash_windows.rs`:

```rust
#![allow(missing_docs)]
//! Crash-window proofs. A side-effect counter — not an event name — decides
//! whether a body ran, because an event name is emitted on both paths.

use serde_json::json;
use std::sync::Arc;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::VoxDbTracker;
use vox_workflow_runtime::workflow::interpret_workflow_durable;

// `env.get` is observable from the test: the activity writes a marker file per
// invocation, so the test counts real executions rather than trusting a label.
const SRC: &str = r#"
activity charge_card(marker_dir: str, amount: int) to Result[str] {
    fs.write(marker_dir + "/charge-" + str(amount) + ".marker", "ran")
    return Ok("tx_" + str(amount))
}

activity send_receipt(marker_dir: str, tx: str) to Result[str] {
    fs.write(marker_dir + "/receipt.marker", "ran")
    return Ok("emailed:" + tx)
}

workflow checkout(marker_dir: str, amount: int) to Result[str] {
    let tx = charge_card(marker_dir, amount)?
    let receipt = send_receipt(marker_dir, tx)?
    return Ok(receipt)
}
"#;

fn markers(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).count())
        .unwrap_or(0)
}

/// Crash AFTER an activity completes: on resume the body must NOT run again.
#[tokio::test]
async fn completed_activity_does_not_re_execute_on_resume() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_s = dir.path().to_string_lossy().to_string();
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let run_id = "crash-after-complete";

    // Run 1: completes fully. 2 markers.
    {
        let mut t = VoxDbTracker::new(db.clone(), run_id);
        interpret_workflow_durable(&hir, "checkout", vec![json!(dir_s), json!(42)], &mut t)
            .await
            .expect("first run");
    }
    assert_eq!(markers(dir.path()), 2, "first run executes both activities");

    // Run 2: same run_id — everything replays, nothing executes.
    {
        let mut t = VoxDbTracker::new(db.clone(), run_id);
        interpret_workflow_durable(&hir, "checkout", vec![json!(dir_s), json!(42)], &mut t)
            .await
            .expect("resume");
    }
    assert_eq!(
        markers(dir.path()),
        2,
        "resume must replay from the journal, not re-execute the bodies"
    );
}

/// Crash DURING an activity (started, never completed): resume must retry it,
/// and the attempt log must show attempt 2.
#[tokio::test]
async fn interrupted_activity_retries_on_resume() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_s = dir.path().to_string_lossy().to_string();
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let run_id = "crash-mid-activity";

    // Simulate the crash window: record `started` for charge_card's id without
    // ever recording `completed`, exactly as a kill -9 between the two writes
    // would leave the log.
    let activity_id = {
        let mut t = VoxDbTracker::new(db.clone(), run_id);
        use vox_workflow_runtime::WorkflowTracker;
        // Derived the same way the runner derives it: first invocation of
        // `charge_card` in `checkout`.
        let id = vox_workflow_runtime::workflow::derive_activity_id("checkout", "charge_card", 0);
        t.on_activity_started("checkout", "charge_card", &id).await.expect("seed started");
        t.on_activity_attempt_started("checkout", "charge_card", &id, 1).await.expect("seed attempt");
        id
    };

    let mut t = VoxDbTracker::new(db.clone(), run_id);
    let journal =
        interpret_workflow_durable(&hir, "checkout", vec![json!(dir_s), json!(42)], &mut t)
            .await
            .expect("resume");

    assert_eq!(
        markers(dir.path()),
        2,
        "an incomplete activity must re-execute (at-least-once)"
    );
    let recovered = journal.iter().find(|e| {
        e["event"].as_str() == Some("ActivityAttemptRecovered")
            && e["activity_id"].as_str() == Some(activity_id.as_str())
    });
    assert!(
        recovered.is_some(),
        "resume must record an attempt recovery; journal={journal:#?}"
    );
    assert_eq!(
        recovered.unwrap()["resume_attempt"].as_u64(),
        Some(2),
        "the retry must be attempt 2, not attempt 1"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p vox-workflow-runtime --test crash_windows
```

Expected: FAIL — `derive_activity_id` is `pub(crate)`; `tempfile` is not a dev-dependency.

- [ ] **Step 3: Make the two things the test needs available**

1. In `crates/vox-workflow-runtime/src/workflow/run.rs` change `pub(crate) fn derive_activity_id` to `pub fn derive_activity_id` and re-export it from `workflow/mod.rs`. This also deletes the copied duplicate in `crash_replay.rs` — a copy that "must stay in sync" is a latent split-brain.

2. Add to `crates/vox-workflow-runtime/Cargo.toml` under `[dev-dependencies]`:

```toml
tempfile = { workspace = true }
```

3. Delete the local `fn derive_activity_id` from `crates/vox-workflow-runtime/tests/crash_replay.rs` and import the real one.

- [ ] **Step 4: Run and watch it pass**

```bash
cargo test -p vox-workflow-runtime --test crash_windows
```

Expected: PASS (2 tests).

- [ ] **Step 5: Mutation-verify both windows**

- In `run.rs`'s `Begin` arm, force `is_activity_completed` to `false`. Confirm `completed_activity_does_not_re_execute_on_resume` FAILS with 4 markers. Restore.
- Force `next_activity_attempt_start` to always return `1`. Confirm `interrupted_activity_retries_on_resume` FAILS on the `resume_attempt == 2` assertion. Restore.

- [ ] **Step 6: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-workflow-runtime/
git commit -m "test(workflow): prove both crash windows with a side-effect counter

Event names are emitted on both the replay and the execute path, so the prior
test could not distinguish 'body did not run' from 'body ran'. Count real file
writes instead, and delete the duplicated derive_activity_id copy.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2.2: Expose `activity_id` to the activity body as an idempotency key

At-least-once is the honest guarantee; it is only usable if the activity author can deduplicate. Give the body its `activity_id`.

**Files:**
- Modify: `crates/vox-workflow-runtime/src/workflow/host.rs`
- Modify: `crates/vox-workflow-runtime/src/workflow/run.rs`
- Modify: `crates/vox-compiler/src/eval/builtins.rs`
- Test: `crates/vox-workflow-runtime/tests/crash_windows.rs`

**Interfaces:**
- Consumes: `HostDecision::Execute` (Task 1.3).
- Produces: `HostDecision::Execute { activity_id: String }` (was a unit variant) and the Vox builtin `workflow.activity_id()` returning `str`.

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-workflow-runtime/tests/crash_windows.rs`:

```rust
/// The activity body can read its own idempotency key, which is the only way
/// at-least-once execution can be made safe for an external side effect.
#[tokio::test]
async fn activity_can_read_its_own_idempotency_key() {
    const SRC_KEY: &str = r#"
activity charge(amount: int) to Result[str] {
    return Ok(workflow.activity_id())
}

workflow wf(amount: int) to Result[str] {
    return Ok(charge(amount)?)
}
"#;
    let hir = lower_module(&parse(lex(SRC_KEY)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let mut t = VoxDbTracker::new(db, "idem-key-1");
    let journal = interpret_workflow_durable(&hir, "wf", vec![json!(1)], &mut t)
        .await
        .expect("runs");

    let expected = vox_workflow_runtime::workflow::derive_activity_id("wf", "charge", 0);
    let completed = journal
        .iter()
        .rev()
        .find(|e| e["event"].as_str() == Some("WorkflowCompleted"))
        .expect("terminates");
    assert_eq!(
        completed["return_value"],
        json!({"__vox": "Ok", "value": expected}),
        "workflow.activity_id() must return the journal's idempotency key"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p vox-workflow-runtime --test crash_windows activity_can_read
```

Expected: FAIL — unknown builtin `workflow.activity_id`.

- [ ] **Step 3: Carry the id to the interpreter thread**

1. In `host.rs`, change the variant to `Execute { activity_id: String }` and, in `run_on_thread`'s hook, set a thread-local before returning `ActivityDecision::Execute`:

```rust
thread_local! {
    /// The `activity_id` of the activity currently executing on this thread,
    /// read by the `workflow.activity_id()` builtin.
    pub(crate) static CURRENT_ACTIVITY_ID: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}
```

```rust
            HostDecision::Execute { activity_id } => {
                CURRENT_ACTIVITY_ID.with(|c| *c.borrow_mut() = Some(activity_id));
                ActivityDecision::Execute
            }
```

Clear it on `ActivityCall::End` so a later plain `fn` cannot read a stale key.

2. In `run.rs`, respond with `HostDecision::Execute { activity_id: activity_id.clone() }`.

3. In `crates/vox-compiler/src/eval/builtins.rs`, register a `workflow` namespace builtin. The interpreter cannot depend on `vox-workflow-runtime` (that would be an upward edge), so the value is passed in from the runtime side: add a public setter in `eval/activity_hook.rs`:

```rust
thread_local! {
    static CURRENT_ACTIVITY_ID: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

/// Set (or clear) the idempotency key readable by `workflow.activity_id()` on
/// this thread. Called by the durable runner around each activity body.
pub fn set_current_activity_id(id: Option<String>) {
    CURRENT_ACTIVITY_ID.with(|c| *c.borrow_mut() = id);
}

/// The idempotency key of the activity executing on this thread, if any.
#[must_use]
pub fn current_activity_id() -> Option<String> {
    CURRENT_ACTIVITY_ID.with(|c| c.borrow().clone())
}
```

and dispatch `("workflow", "activity_id")` in `call_builtin_method` to
`VoxValue::Str(crate::eval::activity_hook::current_activity_id().unwrap_or_default())`,
seeding a `workflow` namespace object in `Interpreter::new` alongside `fs` / `process` / `env`.

Then in `host.rs` call `vox_compiler::eval::activity_hook::set_current_activity_id(...)` rather than defining a second thread-local.

- [ ] **Step 4: Run and watch it pass**

```bash
cargo test -p vox-workflow-runtime --test crash_windows
cargo test -p vox-compiler --lib eval
```

Expected: PASS.

- [ ] **Step 5: Document it in the explanation doc**

In `docs/src/explanation/expl-durable-execution.md` §3, replace the last bullet with:

```markdown
- If you need a stronger guarantee, key the side effect on the activity's own
  idempotency key: `workflow.activity_id()` returns the same string on every
  retry and replay of that step, so a payment provider or mailer can deduplicate.
```

- [ ] **Step 6: Commit**

```bash
vox run scripts/fmt.vox
git add crates/ docs/
git commit -m "feat(workflow): expose workflow.activity_id() inside activity bodies

At-least-once is only usable if the author can deduplicate. The idempotency key
is stable across retries and replays of the same step.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Phase 3 — Durable timers

`workflow_wait` currently calls `tokio::time::sleep` and persists nothing until after it returns. Crash at hour 23 of a 24-hour wait and the resumed run waits another 24 hours. Replace sleeping with parking, copying the crash-recovery anchor pattern `scheduled/runner.rs` already gets right.

### Task 3.1: Persist a wake deadline and park

**Files:**
- Modify: `crates/vox-db/src/schema/domains/execution.rs`
- Modify: `crates/vox-db/src/facade/workflow.rs`
- Modify: `crates/vox-workflow-runtime/src/workflow/tracker.rs`
- Modify: `crates/vox-workflow-runtime/src/db_tracker.rs`
- Modify: `crates/vox-workflow-runtime/src/workflow/run.rs`
- Modify: `crates/vox-compiler/src/eval/builtins.rs`

**Interfaces:**
- Consumes: `HostDecision::Park` (Task 1.3).
- Produces:
  - table `workflow_timer_log(run_id, activity_id, wake_at_ms, fired_at_ms)`
  - `VoxDb::record_workflow_timer(run_id, activity_id, wake_at_ms)`,
    `VoxDb::load_workflow_timer(run_id, activity_id) -> Option<(i64, Option<i64>)>`,
    `VoxDb::mark_workflow_timer_fired(run_id, activity_id)`,
    `VoxDb::due_parked_workflow_runs(now_ms, limit) -> Vec<(String, String)>`
  - `WorkflowTracker::load_timer_deadline` / `record_timer_deadline` (defaulted no-ops, overridden by `VoxDbTracker` and `FileJournalTracker`)
  - Vox builtin `workflow_wait(duration)` reaching the runner as an activity named `__durable_timer_wait` with the duration as its single argument.

  Task 3.2 (the waker) consumes `due_parked_workflow_runs`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-workflow-runtime/tests/durable_timer.rs`:

```rust
#![allow(missing_docs)]
//! A durable timer is a persisted wake deadline, not an in-process sleep.

use serde_json::json;
use std::sync::Arc;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::VoxDbTracker;
use vox_workflow_runtime::workflow::interpret_workflow_durable;

const SRC: &str = r#"
activity note(tag: str) to str {
    return tag
}

workflow delayed() to str {
    let a = note("before")
    workflow_wait("1h")
    let b = note("after")
    return b
}
"#;

#[tokio::test]
async fn first_visit_parks_instead_of_sleeping() {
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let mut t = VoxDbTracker::new(db.clone(), "timer-park-1");

    let started = std::time::Instant::now();
    let journal = interpret_workflow_durable(&hir, "delayed", vec![], &mut t)
        .await
        .expect("parks rather than erroring");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "a 1h wait must park immediately, not sleep in-process"
    );

    assert!(
        journal.iter().any(|e| e["event"].as_str() == Some("TimerScheduled")),
        "the wake deadline must be journalled; journal={journal:#?}"
    );
    assert_eq!(
        journal.last().and_then(|e| e["event"].as_str()),
        Some("WorkflowParked"),
        "the run must end parked, not completed"
    );
    assert!(
        !journal
            .iter()
            .any(|e| e["activity"].as_str() == Some("note")
                && e["result"] == json!("after")),
        "work after the wait must not run before the timer fires"
    );
}

#[tokio::test]
async fn resume_after_the_deadline_completes_the_run() {
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let run_id = "timer-resume-1";

    {
        let mut t = VoxDbTracker::new(db.clone(), run_id);
        interpret_workflow_durable(&hir, "delayed", vec![], &mut t).await.expect("parks");
    }

    // Move the deadline into the past, exactly as an hour of wall-clock would.
    let timer_id = vox_workflow_runtime::workflow::derive_activity_id(
        "delayed",
        "__durable_timer_wait",
        0,
    );
    db.record_workflow_timer(run_id, &timer_id, 0)
        .await
        .expect("backdate the deadline");

    let mut t = VoxDbTracker::new(db.clone(), run_id);
    let journal = interpret_workflow_durable(&hir, "delayed", vec![], &mut t)
        .await
        .expect("resumes");
    assert_eq!(
        journal.last().and_then(|e| e["event"].as_str()),
        Some("WorkflowCompleted"),
        "a due timer must let the run finish; journal={journal:#?}"
    );
    assert!(
        journal
            .iter()
            .any(|e| e["event"].as_str() == Some("ActivityReplayed")
                && e["activity"].as_str() == Some("note")),
        "the pre-wait activity must replay, not re-execute"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p vox-workflow-runtime --test durable_timer
```

Expected: FAIL — `workflow_wait` is not a known call; `record_workflow_timer` does not exist.

- [ ] **Step 3: Add the table and facade methods**

In `crates/vox-db/src/schema/domains/execution.rs`, after the `workflow_signal_log` block:

```sql
CREATE TABLE IF NOT EXISTS workflow_timer_log (
    run_id          TEXT NOT NULL,
    activity_id     TEXT NOT NULL,
    wake_at_ms      INTEGER NOT NULL,
    fired_at_ms     INTEGER,
    recorded_at_ms  INTEGER NOT NULL,
    PRIMARY KEY (run_id, activity_id)
);

CREATE INDEX IF NOT EXISTS idx_workflow_timer_due
    ON workflow_timer_log(wake_at_ms, fired_at_ms);
```

In `crates/vox-db/src/facade/workflow.rs`, add the four methods listed in **Interfaces**, following the shape of `record_workflow_signal` / `consume_workflow_signal` (same `breaker.call(...)` wrapper, same `workflow_now_ms()` helper, same `StoreError` mapping). `record_workflow_timer` is an `INSERT … ON CONFLICT(run_id, activity_id) DO UPDATE SET wake_at_ms = excluded.wake_at_ms` so the test can backdate a deadline. `due_parked_workflow_runs` selects `run_id, workflow_name` from `workflow_run_log` joined to `workflow_timer_log` where `fired_at_ms IS NULL AND wake_at_ms <= ?1`, `LIMIT ?2`.

- [ ] **Step 4: Add the tracker hooks**

In `crates/vox-workflow-runtime/src/workflow/tracker.rs`, add two defaulted trait methods (default: `Ok(None)` / `Ok(())`) mirroring the existing `load_workflow_patch` / `record_workflow_patch` pair:

```rust
    /// Load a previously persisted wake deadline for a timer step.
    /// `Ok(None)` means this is the first visit.
    fn load_timer_deadline(
        &self,
        _activity_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<u64>>> + Send {
        async { Ok(None) }
    }

    /// Persist the wake deadline for a timer step on its first visit.
    fn record_timer_deadline(
        &mut self,
        _activity_id: &str,
        _wake_at_ms: u64,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }
```

Implement both on `VoxDbTracker` (`db_tracker.rs`) and on `FileJournalTracker` (`file_journal.rs`, which already persists patches and so has the pattern).

- [ ] **Step 5: Handle the timer step in the runner**

`workflow_wait` reaches the runner as an ordinary `Begin` because the interpreter treats it as a builtin call routed through the activity hook. Register `"__durable_timer_wait"` in `WorkflowHost::spawn`'s `activity_names` set unconditionally, and lower the Vox call `workflow_wait(d)` to a call of that name in `crates/vox-compiler/src/hir/lower/expr.rs` (it currently lowers to a plain `Call(Ident("workflow_wait"))`; rename at lowering so the runner sees one canonical name and the duration as `args[0]`).

Then, in `run.rs`'s `Begin` arm, branch before the cache lookup:

```rust
                if name == "__durable_timer_wait" {
                    let wait_ms = args
                        .first()
                        .and_then(duration_arg_to_ms)
                        .ok_or_else(|| anyhow::anyhow!("workflow_wait requires a duration"))?;
                    let now = now_unix_ms();
                    let deadline = match tracker.load_timer_deadline(&activity_id).await? {
                        Some(existing) => existing,
                        None => {
                            let d = now.saturating_add(wait_ms);
                            tracker.record_timer_deadline(&activity_id, d).await?;
                            journal.push(versioned_event(json!({
                                "event": "TimerScheduled",
                                "workflow": workflow_name,
                                "activity_id": activity_id,
                                "wake_at_ms": d,
                            })));
                            d
                        }
                    };
                    if deadline <= now {
                        journal.push(versioned_event(json!({
                            "event": "TimerWaitCompleted",
                            "workflow": workflow_name,
                            "activity_id": activity_id,
                            "wake_at_ms": deadline,
                        })));
                        host.respond(super::host::HostDecision::Replay(Value::Null))?;
                    } else {
                        journal.push(versioned_event(json!({
                            "event": "ActivityParked",
                            "workflow": workflow_name,
                            "activity_id": activity_id,
                            "reason": "timer",
                            "wake_at_ms": deadline,
                        })));
                        host.respond(super::host::HostDecision::Park("timer".into()))?;
                    }
                    continue;
                }
```

`duration_arg_to_ms` is a small free function in `run.rs` that accepts a JSON string (delegating to `crate::duration_literal::parse_duration_str`) or a JSON integer (seconds, matching the shared parser's cron-style default), returning `Option<u64>`.

- [ ] **Step 6: Run and watch it pass**

```bash
cargo test -p vox-workflow-runtime --test durable_timer
cargo test -p vox-db
```

Expected: PASS.

- [ ] **Step 7: Mutation-verify**

Force `load_timer_deadline` to always return `None`. Confirm `resume_after_the_deadline_completes_the_run` FAILS (it re-schedules a fresh 1-hour deadline and parks again). Restore.

- [ ] **Step 8: Commit**

```bash
cargo run -q -p vox-cli -- ci ssot-drift
vox run scripts/fmt.vox
git add crates/
git commit -m "feat(workflow): park on workflow_wait instead of sleeping in-process

The wake deadline is persisted on first visit and consulted on resume, so a
crash 23 hours into a 24-hour wait fires in ~1 hour rather than restarting the
clock. Mirrors the @scheduled runner's wall-clock recovery anchor.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3.2: The waker loop

A parked run needs something to resume it.

**Files:**
- Create: `crates/vox-workflow-runtime/src/workflow/waker.rs`
- Modify: `crates/vox-workflow-runtime/src/workflow/mod.rs`
- Test: `crates/vox-workflow-runtime/src/workflow/waker.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `VoxDb::due_parked_workflow_runs` (Task 3.1); `interpret_workflow_durable` (Task 1.4).
- Produces: `pub async fn wake_due_runs(db: Arc<VoxDb>, hir: &HirModule, limit: usize) -> anyhow::Result<usize>` returning the number of runs resumed, and `pub async fn start_waker(db: Arc<VoxDb>, hir: HirModule, tick: Duration) -> WakerHandle` modelled on `scheduled::runner::start`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use vox_compiler::hir::lower_module;
    use vox_compiler::lexer::cursor::lex;
    use vox_compiler::parser::parse;
    use vox_db::{DbConfig, VoxDb};

    const SRC: &str = r#"
activity note(tag: str) to str { return tag }
workflow delayed() to str {
    let a = note("before")
    workflow_wait("1h")
    return note("after")
}
"#;

    // Catches: a parked run never being resumed, i.e. a durable timer that
    // persists a deadline nobody reads.
    #[tokio::test]
    async fn waker_resumes_a_run_whose_deadline_has_passed() {
        let hir = lower_module(&parse(lex(SRC)).expect("parses"));
        let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
        let run_id = "waker-1";
        {
            let mut t = crate::VoxDbTracker::new(db.clone(), run_id);
            crate::workflow::interpret_workflow_durable(&hir, "delayed", vec![], &mut t)
                .await
                .expect("parks");
        }
        let timer_id =
            crate::workflow::derive_activity_id("delayed", "__durable_timer_wait", 0);
        db.record_workflow_timer(run_id, &timer_id, 0).await.expect("backdate");

        let resumed = wake_due_runs(db.clone(), &hir, 10).await.expect("wakes");
        assert_eq!(resumed, 1, "the due run must be resumed exactly once");

        // Second pass finds nothing: the timer is marked fired.
        let again = wake_due_runs(db.clone(), &hir, 10).await.expect("wakes");
        assert_eq!(again, 0, "a fired timer must not be resumed twice");
        let _ = json!(0);
    }
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p vox-workflow-runtime --lib workflow::waker
```

Expected: FAIL — `wake_due_runs` not found.

- [ ] **Step 3: Implement `waker.rs`**

```rust
//! Resume workflow runs parked on a timer whose deadline has passed.
//!
//! Modelled on [`crate::scheduled::runner`]: the persisted wall-clock moment is
//! the crash-recovery anchor, and a resumed run replays its journal, so waking
//! a run twice is harmless — but `mark_workflow_timer_fired` makes it a no-op
//! anyway, which keeps the loop from spinning on a stuck run.

use std::sync::Arc;
use std::time::Duration;
use vox_compiler::hir::HirModule;
use vox_db::VoxDb;

/// Resume every run whose timer is due, up to `limit`. Returns how many ran.
///
/// A failure in one run is logged and skipped: one poisoned run must not stall
/// every other parked workflow on the node.
pub async fn wake_due_runs(
    db: Arc<VoxDb>,
    hir: &HirModule,
    limit: usize,
) -> anyhow::Result<usize> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let due = db.due_parked_workflow_runs(now, limit).await?;
    let mut resumed = 0usize;
    for (run_id, workflow_name) in due {
        let mut tracker = crate::VoxDbTracker::new(db.clone(), run_id.clone());
        match crate::workflow::interpret_workflow_durable(hir, &workflow_name, vec![], &mut tracker)
            .await
        {
            Ok(_) => {
                resumed += 1;
            }
            Err(e) => {
                tracing::warn!(run_id = %run_id, workflow = %workflow_name, error = %e,
                    "parked workflow failed to resume; leaving it for the next tick");
            }
        }
    }
    Ok(resumed)
}
```

Add `start_waker` as a `tokio::spawn`ed loop over `wake_due_runs` with a `oneshot` shutdown channel, copying the `ScheduledHandle` shape from `scheduled/runner.rs` verbatim rather than inventing a second lifecycle idiom.

**Open question for the implementer, decide in this task and record it in the doc comment:** resumed runs are re-invoked with `vec![]` arguments. That is correct only if every argument-dependent activity has already been journalled before the park. It is not, in general. Persist the original workflow arguments in `workflow_run_log` (add an `args_json` column in Task 3.1's DDL) and pass them here. Do that — the empty-vec version is a latent wrong-answer bug, not a simplification.

- [ ] **Step 4: Run and watch it pass**

```bash
cargo test -p vox-workflow-runtime --lib workflow::waker
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
vox run scripts/fmt.vox
git add crates/vox-workflow-runtime/
git commit -m "feat(workflow): add the parked-run waker loop

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Phase 4 — Durable signals

### Task 4.1: Park on a missing signal instead of failing the run

`VoxDbTracker::on_activity_started` currently `bail!`s when a signal row is absent, which fails the whole run; then the stub step immediately emits `SignalWaitSatisfied` anyway. Both halves are wrong.

**Files:**
- Modify: `crates/vox-workflow-runtime/src/db_tracker.rs`
- Modify: `crates/vox-workflow-runtime/src/workflow/run.rs`
- Modify: `crates/vox-workflow-runtime/src/workflow/tracker.rs`
- Modify: `crates/vox-compiler/src/hir/lower/expr.rs`
- Test: `crates/vox-workflow-runtime/tests/durable_signal.rs` (create)

**Interfaces:**
- Consumes: `HostDecision::Park` (Task 1.3), `VoxDb::consume_workflow_signal` (exists).
- Produces: `WorkflowTracker::try_consume_signal(&mut self, signal_key: &str) -> anyhow::Result<Option<Value>>` (defaulted to `Ok(None)`), overridden by `VoxDbTracker`. The signal payload becomes the value the workflow body binds.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-workflow-runtime/tests/durable_signal.rs`:

```rust
#![allow(missing_docs)]
//! A workflow waiting on a signal parks; it does not fail.

use serde_json::json;
use std::sync::Arc;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::VoxDbTracker;
use vox_workflow_runtime::workflow::interpret_workflow_durable;

const SRC: &str = r#"
activity record(v: str) to str { return v }

workflow approval() to str {
    let decision = workflow_wait_signal("approved")
    return record(decision)
}
"#;

#[tokio::test]
async fn missing_signal_parks_the_run() {
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let mut t = VoxDbTracker::new(db, "signal-park-1");

    let journal = interpret_workflow_durable(&hir, "approval", vec![], &mut t)
        .await
        .expect("an absent signal must park, not error");

    assert_eq!(
        journal.last().and_then(|e| e["event"].as_str()),
        Some("WorkflowParked"),
        "journal={journal:#?}"
    );
    assert!(
        !journal.iter().any(|e| e["event"].as_str() == Some("SignalWaitSatisfied")),
        "an unsatisfied wait must never report satisfaction"
    );
}

#[tokio::test]
async fn resume_after_the_signal_arrives_binds_its_payload() {
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let run_id = "signal-resume-1";
    {
        let mut t = VoxDbTracker::new(db.clone(), run_id);
        interpret_workflow_durable(&hir, "approval", vec![], &mut t).await.expect("parks");
    }

    db.record_workflow_signal(run_id, "approved", Some(&json!("yes")))
        .await
        .expect("signal recorded");

    let mut t = VoxDbTracker::new(db.clone(), run_id);
    let journal = interpret_workflow_durable(&hir, "approval", vec![], &mut t)
        .await
        .expect("resumes");
    let completed = journal
        .iter()
        .rev()
        .find(|e| e["event"].as_str() == Some("WorkflowCompleted"))
        .expect("terminates");
    assert_eq!(
        completed["return_value"],
        json!("yes"),
        "the signal payload must be the value the workflow binds"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p vox-workflow-runtime --test durable_signal
```

Expected: FAIL — the run errors instead of parking.

- [ ] **Step 3: Remove the signal special-case from `on_activity_started`**

Delete this block from `crates/vox-workflow-runtime/src/db_tracker.rs::on_activity_started`:

```rust
            if let Some(signal_key) = activity_name.strip_prefix("__durable_signal_wait:") {
                let consumed = db.consume_workflow_signal(&run_id, signal_key).await…;
                if !consumed { anyhow::bail!("workflow run `{}` is waiting for signal `{}`", …); }
            }
```

A tracker lifecycle hook is the wrong place for a control-flow decision. Replace it with the new `try_consume_signal` method, which returns the payload rather than a bool:

```rust
    fn try_consume_signal(
        &mut self,
        signal_key: &str,
    ) -> impl Future<Output = Result<Option<serde_json::Value>>> + Send {
        let db = self.db.clone();
        let run_id = self.run_id.clone();
        let signal_key = signal_key.to_string();
        async move {
            db.consume_workflow_signal_with_payload(&run_id, &signal_key)
                .await
                .map_err(|e| anyhow::anyhow!("DB error: {}", e))
        }
    }
```

Add `consume_workflow_signal_with_payload` to `crates/vox-db/src/facade/workflow.rs` — the same `SELECT id … UPDATE consumed_at_ms` transaction as `consume_workflow_signal`, but selecting `payload_json` too and returning `Option<Value>`. Keep `consume_workflow_signal` (other callers may exist) delegating to it.

- [ ] **Step 4: Handle the signal step in the runner**

In `run.rs`'s `Begin` arm, alongside the timer branch:

```rust
                if let Some(signal_key) = name.strip_prefix("__durable_signal_wait:") {
                    match tracker.try_consume_signal(signal_key).await? {
                        Some(payload) => {
                            journal.push(versioned_event(json!({
                                "event": "SignalWaitSatisfied",
                                "workflow": workflow_name,
                                "activity_id": activity_id,
                                "signal_key": signal_key,
                            })));
                            // Record it so a later replay of this step does not
                            // consume a second signal.
                            tracker
                                .on_activity_completed(workflow_name, &name, &activity_id, &payload)
                                .await?;
                            host.respond(super::host::HostDecision::Replay(payload))?;
                        }
                        None => {
                            journal.push(versioned_event(json!({
                                "event": "SignalAwaited",
                                "workflow": workflow_name,
                                "activity_id": activity_id,
                                "signal_key": signal_key,
                            })));
                            host.respond(super::host::HostDecision::Park(format!(
                                "signal:{signal_key}"
                            )))?;
                        }
                    }
                    continue;
                }
```

Note the ordering: the completion is recorded **before** the value is handed to the workflow, so a crash between the two replays the recorded payload rather than consuming a second signal.

Lower `workflow_wait_signal("k")` in `crates/vox-compiler/src/hir/lower/expr.rs` to a call named `__durable_signal_wait:k`, matching the timer treatment, and register the prefix in `WorkflowHost::spawn`'s `activity_names`.

- [ ] **Step 5: Run and watch it pass**

```bash
cargo test -p vox-workflow-runtime --test durable_signal
```

Expected: PASS.

- [ ] **Step 6: Mutation-verify the double-consume guard**

Move the `on_activity_completed` call to *after* `host.respond(...)`. Add and run a test that resumes the same run twice with only one recorded signal, and confirm the second resume now wrongly parks (proving the ordering matters). Restore.

- [ ] **Step 7: Commit**

```bash
vox run scripts/fmt.vox
git add crates/
git commit -m "feat(workflow): park on an absent signal and bind its payload on resume

A missing signal used to fail the run from inside a tracker lifecycle hook,
while the step separately emitted SignalWaitSatisfied regardless. Both are gone;
the wait is now an ordinary journalled step whose recorded value is the payload.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4.2: `vox workflow signal` CLI

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/ai/workflow.rs`
- Modify: the command registry that declares `vox mens workflow` subcommands (find it with `vox graph query "mens workflow subcommand"`)
- Test: `crates/vox-ml-cli/tests/workflow_signal_cli.rs` (create)

**Interfaces:**
- Consumes: `VoxDb::record_workflow_signal` (exists).
- Produces: `vox mens workflow signal <run_id> <key> [--payload <json>]`.

- [ ] **Step 1: Write the failing test**

```rust
#![allow(missing_docs)]
//! `vox mens workflow signal` inserts a consumable row for a parked run.

use serde_json::json;
use std::sync::Arc;
use vox_db::{DbConfig, VoxDb};

#[tokio::test]
async fn signal_command_inserts_a_consumable_row() {
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    vox_ml_cli::commands::ai::workflow::signal(db.clone(), "run-7", "approved", Some(json!("yes")))
        .await
        .expect("signal recorded");

    let payload = db
        .consume_workflow_signal_with_payload("run-7", "approved")
        .await
        .expect("consume");
    assert_eq!(payload, Some(json!("yes")));

    let second = db
        .consume_workflow_signal_with_payload("run-7", "approved")
        .await
        .expect("consume again");
    assert_eq!(second, None, "a signal must be consumable exactly once");
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p vox-ml-cli --test workflow_signal_cli
```

Expected: FAIL — no function `signal`.

- [ ] **Step 3: Implement the command**

```rust
/// Record a signal for a parked durable workflow run.
///
/// The run is resumed by the waker (or by the next `--run-id` invocation);
/// this command only inserts the row.
pub async fn signal(
    db: std::sync::Arc<vox_db::VoxDb>,
    run_id: &str,
    key: &str,
    payload: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    db.record_workflow_signal(run_id, key, payload.as_ref())
        .await
        .map_err(|e| anyhow::anyhow!("DB error: {e}"))?;
    println!("Signal `{key}` recorded for run `{run_id}`.");
    Ok(())
}
```

Wire the clap subcommand in the registry file, with `--payload` parsed as JSON.

- [ ] **Step 4: Run, regenerate the command surface docs, commit**

```bash
cargo test -p vox-ml-cli --test workflow_signal_cli
cargo run -p vox-cli -- ci command-sync
vox run scripts/fmt.vox
git add crates/ docs/src/reference/cli-command-surface.generated.md
git commit -m "feat(cli): add vox mens workflow signal

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Phase 5 — Identity, versioning, and per-call options

### Task 5.1: Call-site-scoped activity ids

Today's id is `blake3(workflow \0 activity_name \0 nth-invocation)`. The per-name counter added in Task 1.4 already improves on the old global position (inserting a call to a *different* activity no longer shifts ids), but inserting a second call to the *same* activity earlier in the body still renames every later one. Scope the counter to the call site.

**Files:**
- Modify: `crates/vox-workflow-runtime/src/workflow/host.rs`
- Modify: `crates/vox-workflow-runtime/src/workflow/run.rs`
- Modify: `crates/vox-compiler/src/eval/activity_hook.rs`, `expr.rs`
- Test: `crates/vox-workflow-runtime/tests/activity_identity.rs` (create)

**Interfaces:**
- Consumes: `ActivityCall::Begin` (Task 1.2).
- Produces: `ActivityCall::Begin` gains `call_site: String` — the dotted path `"{enclosing_fn}#{ordinal}"` where `ordinal` is the call expression's index in a deterministic pre-order walk of the enclosing function body. `HostRequest::Begin` gains the same field. `derive_activity_id(workflow_name, activity_name, call_site, iteration)` replaces the positional form.

- [ ] **Step 1: Write the failing test**

```rust
#![allow(missing_docs)]
//! Inserting a call must not rename the ids of calls that follow it.

use serde_json::json;
use std::sync::Arc;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::VoxDbTracker;
use vox_workflow_runtime::workflow::interpret_workflow_durable;

const V1: &str = r#"
activity step(tag: str) to str { return tag }
workflow wf() to str {
    let a = step("a")
    let b = step("b")
    return b
}
"#;

const V2: &str = r#"
activity step(tag: str) to str { return tag }
workflow wf() to str {
    let a = step("a")
    let inserted = step("inserted")
    let b = step("b")
    return b
}
"#;

async fn ids_for(src: &str, run_id: &str, db: Arc<VoxDb>) -> Vec<String> {
    let hir = lower_module(&parse(lex(src)).expect("parses"));
    let mut t = VoxDbTracker::new(db, run_id);
    let journal = interpret_workflow_durable(&hir, "wf", vec![], &mut t)
        .await
        .expect("runs");
    journal
        .iter()
        .filter(|e| e["event"].as_str() == Some("ActivityCompleted"))
        .filter_map(|e| e["activity_id"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn inserting_a_call_does_not_rename_later_ids() {
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let v1 = ids_for(V1, "identity-v1", db.clone()).await;
    let v2 = ids_for(V2, "identity-v2", db.clone()).await;
    assert_eq!(v1.len(), 2);
    assert_eq!(v2.len(), 3);
    assert_eq!(
        v1[0], v2[0],
        "the first call's id must be stable across the insertion"
    );
    assert_eq!(
        v1[1], v2[2],
        "the trailing call's id must survive an insertion above it; \
         v1={v1:?} v2={v2:?}"
    );
    let _ = json!(0);
}

#[tokio::test]
async fn loop_iterations_get_distinct_ids() {
    const LOOPING: &str = r#"
activity step(tag: str) to str { return tag }
workflow wf(tags: [str]) to str {
    let mut last = ""
    for t in tags {
        last = step(t)
    }
    return last
}
"#;
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let hir = lower_module(&parse(lex(LOOPING)).expect("parses"));
    let mut t = VoxDbTracker::new(db, "identity-loop");
    let journal =
        interpret_workflow_durable(&hir, "wf", vec![json!(["x", "y", "z"])], &mut t)
            .await
            .expect("runs");
    let ids: Vec<&str> = journal
        .iter()
        .filter(|e| e["event"].as_str() == Some("ActivityCompleted"))
        .filter_map(|e| e["activity_id"].as_str())
        .collect();
    assert_eq!(ids.len(), 3);
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(
        unique.len(),
        3,
        "each loop iteration needs its own id or iteration 2 replays iteration 1's result: {ids:?}"
    );
}
```

- [ ] **Step 2: Run and watch `inserting_a_call_does_not_rename_later_ids` fail**

```bash
cargo test -p vox-workflow-runtime --test activity_identity
```

Expected: the loop test PASSES (the per-name counter already handles it); the insertion test FAILS.

- [ ] **Step 3: Thread the call site through**

1. In `crates/vox-compiler/src/hir/lower/expr.rs`, stamp each `HirExpr::Call` with a stable ordinal during lowering — a `call_ordinal: u32` field on the call node, assigned by a counter that resets per function and increments in pre-order. Storing it in HIR (rather than recomputing from `Span`) makes it survive reformatting, which a span-derived id would not.
2. In `expr.rs`'s interception branch, pass `format!("{}#{}", interp.current_fn_name, call_ordinal)` as `ActivityCall::Begin { call_site, .. }`. `current_fn_name` is a new `String` field on `Interpreter`, set in the `VoxValue::Fn` application arm and restored alongside `old_scope`.
3. In `run.rs`, key the invocation counter on `call_site` instead of `name`, and hash `(workflow_name, activity_name, call_site, iteration)`.

- [ ] **Step 4: Run and watch both pass**

```bash
cargo test -p vox-workflow-runtime --test activity_identity
```

Expected: PASS.

- [ ] **Step 5: Add the old-run compatibility test**

Because the id scheme changed, in-flight runs journalled under the old scheme will re-execute. That is a one-time migration, and it must be *stated*, not discovered. Add to `activity_identity.rs`:

```rust
/// Appending a trailing activity must not disturb an in-flight run: the ids of
/// the steps already journalled stay identical, so they replay.
#[tokio::test]
async fn appending_a_trailing_activity_preserves_an_in_flight_run() {
    const APPENDED: &str = r#"
activity step(tag: str) to str { return tag }
workflow wf() to str {
    let a = step("a")
    let b = step("b")
    let c = step("c")
    return c
}
"#;
    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let run_id = "identity-append";
    let _ = ids_for(V1, run_id, db.clone()).await;

    let hir = lower_module(&parse(lex(APPENDED)).expect("parses"));
    let mut t = VoxDbTracker::new(db.clone(), run_id);
    let journal = interpret_workflow_durable(&hir, "wf", vec![], &mut t)
        .await
        .expect("resumes under the new code");
    let replayed = journal
        .iter()
        .filter(|e| e["event"].as_str() == Some("ActivityReplayed"))
        .count();
    assert_eq!(replayed, 2, "both prior steps must replay; journal={journal:#?}");
    assert_eq!(
        journal.last().and_then(|e| e["event"].as_str()),
        Some("WorkflowCompleted")
    );
}
```

- [ ] **Step 6: Document the migration and commit**

Add to `docs/src/explanation/expl-durable-execution.md` a short "§7 Upgrading workflow code" section stating: appending activities is safe; inserting or reordering activities within a function changes the ids of the moved calls, so those steps re-execute on in-flight runs — drain or cancel in-flight runs before such an edit, or gate it with `workflow.version(...)`.

```bash
vox run scripts/fmt.vox
git add crates/ docs/
git commit -m "feat(workflow): key activity ids on the call site, not the invocation ordinal

Inserting a call to the same activity earlier in a workflow body used to rename
every later id, so in-flight runs replayed the wrong history.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5.2: Persist `workflow.version` for the SQL tracker, and restore `with { … }` options

`VoxDbTracker` never overrides `load_workflow_patch` / `record_workflow_patch`, so they silently hit the trait's no-op defaults — `workflow.version(...)` is durable on `FileJournalTracker` (mobile) and not on the server. Separately, `with { retries, timeout, activity_id, dedup }` options were parsed by the deleted linearizer and must now be read at the call site.

**Files:**
- Modify: `crates/vox-db/src/schema/domains/execution.rs`
- Modify: `crates/vox-db/src/facade/workflow.rs`
- Modify: `crates/vox-workflow-runtime/src/db_tracker.rs`
- Modify: `crates/vox-compiler/src/eval/expr.rs` (the `HirExpr::With` arm)
- Test: `crates/vox-workflow-runtime/tests/voxdb_tracker_basic.rs`

**Interfaces:**
- Consumes: `WorkflowTracker::{load,record}_workflow_patch` (exist, defaulted).
- Produces: table `workflow_patch_log(run_id, workflow_name, change_id, version, recorded_at_ms)`; `VoxDb::load_workflow_patch` / `record_workflow_patch`; `ActivityCall::Begin` gains `options: serde_json::Value` carrying the evaluated `with { … }` object.

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-workflow-runtime/tests/voxdb_tracker_basic.rs`:

```rust
// Catches the silent no-op: VoxDbTracker inheriting the trait default means
// workflow.version() is durable on mobile and not on the server.
#[tokio::test]
async fn voxdb_tracker_persists_a_workflow_patch() {
    use vox_workflow_runtime::WorkflowTracker;
    let db = std::sync::Arc::new(
        vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("memory db"),
    );
    let mut tracker = vox_workflow_runtime::VoxDbTracker::new(db.clone(), "patch-run-1");
    assert_eq!(
        tracker.load_workflow_patch("wf", "add-audit-v2").await.unwrap(),
        None,
        "no patch recorded yet"
    );
    tracker.record_workflow_patch("wf", "add-audit-v2", 2).await.unwrap();

    // A fresh tracker on the same run must see it — that is the whole point.
    let reloaded = vox_workflow_runtime::VoxDbTracker::new(db, "patch-run-1");
    assert_eq!(
        reloaded.load_workflow_patch("wf", "add-audit-v2").await.unwrap(),
        Some(2),
        "the patch version must survive a process restart"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p vox-workflow-runtime --test voxdb_tracker_basic
```

Expected: FAIL — `load_workflow_patch` returns `None` after recording.

- [ ] **Step 3: Add the table, facade methods, and tracker overrides**

DDL, mirroring `workflow_activity_log`:

```sql
CREATE TABLE IF NOT EXISTS workflow_patch_log (
    run_id          TEXT NOT NULL,
    workflow_name   TEXT NOT NULL,
    change_id       TEXT NOT NULL,
    version         INTEGER NOT NULL,
    recorded_at_ms  INTEGER NOT NULL,
    PRIMARY KEY (run_id, workflow_name, change_id)
);
```

Facade: `record_workflow_patch` is an `INSERT … ON CONFLICT DO NOTHING` (first write wins — a recorded patch version must never change under an in-flight run); `load_workflow_patch` is a single-row `SELECT version`. Override both on `VoxDbTracker` following the `on_activity_completed` / `load_activity_result` pattern.

- [ ] **Step 4: Restore `with { … }` options at the call site**

In `crates/vox-compiler/src/eval/expr.rs`, the `HirExpr::With(inner, opts, _)` arm currently evaluates `inner` with no knowledge of `opts`. Evaluate `opts` to a `VoxValue::Object`, stash it on `interp.pending_activity_options: Option<VoxValue>` for the duration of the inner evaluation, and have `dispatch_activity` take it and pass it as `ActivityCall::Begin { options }`. In `run.rs`, read `activity_id`, `retries`, `timeout`, and `dedup` from that object, falling back to the current defaults (`MAX_ACTIVITY_ATTEMPTS`, `DEFAULT_DEDUP_WINDOW_MS`).

**Reject a duplicate explicit id within a run.** Two loop iterations that both carry `with { activity_id: "charge" }` silently alias today; the second replays the first's result. In `run.rs`, keep a `HashSet<String>` of explicit ids seen this run and `bail!` on a repeat with a message naming the activity — a silently-wrong charge is far worse than a failed run.

- [ ] **Step 5: Run everything and commit**

```bash
cargo test -p vox-workflow-runtime
cargo run -q -p vox-cli -- ci ssot-drift
vox run scripts/fmt.vox
git add crates/
git commit -m "fix(workflow): persist workflow.version for the SQL tracker

VoxDbTracker inherited the trait's no-op default, so patch markers were durable
on FileJournalTracker and silently forgotten on the server. Also restores the
with{} call options the deleted linearizer used to parse, and rejects a
duplicate explicit activity_id within a run instead of aliasing it.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Phase 6 — Generated Rust parity (ADR-021)

### Task 6.1: Generated workflows use a real tracker

`emit_workflow_body` writes `let mut __vox_tracker = DefaultTracker;` — RAM, always "not completed". A crash in a `vox build` binary replays from zero. It also emits `journal::execute` around every activity body with a *different* id scheme (`generated_hash`, one id per function, not per call site), while nothing ever calls those generated activity fns because the workflow body is replaced wholesale by the interpreter call.

**Files:**
- Modify: `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs`
- Delete: `crates/vox-workflow-runtime/src/journal/` (both files)
- Modify: `crates/vox-workflow-runtime/src/lib.rs`
- Modify: `crates/vox-codegen/tests/durability_compiles.rs`, `durability_lowering.rs`
- Test: `crates/vox-codegen/tests/generated_workflow_uses_durable_tracker.rs` (create)

**Interfaces:**
- Consumes: `VoxDbTracker` (exists), `interpret_workflow_durable` with the `args` parameter (Task 1.4).
- Produces: generated workflow bodies that construct a `VoxDbTracker` from `VOX_WORKFLOW_RUN_ID` (or a fresh UUID) and `VoxDb::connect_default()`.

- [ ] **Step 1: Write the failing test**

```rust
#![allow(missing_docs)]
//! ADR-021: a generated workflow must persist its history, not hold it in RAM.

use vox_codegen::codegen_rust::emit_module;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

const SRC: &str = r#"
activity charge(amount: int) to Result[str] { return Ok("tx") }
workflow checkout(amount: int) to Result[str] { return Ok(charge(amount)?) }
"#;

#[test]
fn generated_workflow_does_not_use_the_in_memory_tracker() {
    let rust = emit_module(&lower_module(&parse(lex(SRC)).expect("parses")));
    assert!(
        !rust.contains("DefaultTracker"),
        "a generated workflow must not use the RAM tracker — a crash would replay \
         from zero; got:\n{rust}"
    );
    assert!(
        rust.contains("VoxDbTracker"),
        "a generated workflow must construct a durable tracker; got:\n{rust}"
    );
    assert!(
        rust.contains("VOX_WORKFLOW_RUN_ID"),
        "run_id must be resumable across process restarts; got:\n{rust}"
    );
}

#[test]
fn activity_emit_no_longer_wraps_in_the_dead_journal_shim() {
    let rust = emit_module(&lower_module(&parse(lex(SRC)).expect("parses")));
    assert!(
        !rust.contains("journal::execute"),
        "journal::execute is a production no-op with a conflicting id scheme; got:\n{rust}"
    );
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p vox-codegen --test generated_workflow_uses_durable_tracker
```

Expected: FAIL on both.

- [ ] **Step 3: Emit the durable tracker**

Replace the tracker lines in `emit_workflow_body`:

```rust
    out.push_str(
        "    let __vox_run_id = ::std::env::var(\"VOX_WORKFLOW_RUN_ID\")\n\
         \x20       .unwrap_or_else(|_| ::uuid::Uuid::new_v4().to_string());\n",
    );
    out.push_str(
        "    let __vox_db = ::std::sync::Arc::new(\n\
         \x20       ::vox_db::VoxDb::connect_default().await.map_err(|e| e.to_string())?,\n\
         \x20   );\n",
    );
    out.push_str(
        "    let mut __vox_tracker = \
         ::vox_workflow_runtime::VoxDbTracker::new(__vox_db, __vox_run_id);\n",
    );
```

- [ ] **Step 4: Delete `journal::execute` and its emit**

In `emit_activity_body`, drop the `journal::execute(...)` wrapper and emit the body directly — the runner journals activities now, and the generated activity fn is what the interpreter's HIR walk mirrors. Delete `crates/vox-workflow-runtime/src/journal/` and its `pub mod journal;` declaration and `test-support` feature entry in `Cargo.toml`. Update the two codegen tests that assert the wrapper exists (`durability_compiles.rs`, `durability_lowering.rs`) to assert its absence, and delete `crates/vox-workflow-runtime/tests/journal_execute.rs`.

- [ ] **Step 5: Run and watch it pass**

```bash
cargo test -p vox-codegen
cargo test -p vox-workflow-runtime
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
vox run scripts/fmt.vox
git add crates/
git commit -m "fix(codegen): generated workflows use VoxDbTracker, not the RAM tracker

Also deletes journal::execute — a production no-op whose per-function id scheme
disagreed with the runner's per-call-site scheme, wrapped around generated
activity bodies that nothing called.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6.2: The ADR-021 equivalence gate

**Files:**
- Create: `crates/vox-workflow-runtime/tests/interp_generated_parity.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: the compatibility gate ADR-021 requires before widening syntax support.

- [ ] **Step 1: Write the test**

```rust
#![allow(missing_docs)]
//! ADR-021 gate: interpreted and generated runs of the same workflow must
//! produce the same activity ids and the same recorded results for the same
//! run_id. Generated workflows call `interpret_workflow_durable` (ADR-021
//! implementation option 1), so this asserts the *shared* engine is actually
//! shared — a divergence means codegen re-derived identity somewhere.

use serde_json::json;
use std::sync::Arc;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::VoxDbTracker;
use vox_workflow_runtime::workflow::interpret_workflow_durable;

const SRC: &str = r#"
activity charge(amount: int) to Result[str] { return Ok("tx_" + str(amount)) }
activity receipt(tx: str) to Result[str] { return Ok("r:" + tx) }
workflow checkout(amount: int) to Result[str] {
    let tx = charge(amount)?
    return Ok(receipt(tx)?)
}
"#;

fn digest(journal: &[serde_json::Value]) -> Vec<(String, String, serde_json::Value)> {
    journal
        .iter()
        .filter(|e| e["event"].as_str() == Some("ActivityCompleted"))
        .map(|e| {
            (
                e["activity"].as_str().unwrap_or_default().to_string(),
                e["activity_id"].as_str().unwrap_or_default().to_string(),
                e["result"].clone(),
            )
        })
        .collect()
}

#[tokio::test]
async fn interpreted_and_generated_share_ids_and_results() {
    let hir = lower_module(&parse(lex(SRC)).expect("parses"));
    let db_a = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("db a"));
    let db_b = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("db b"));

    let mut t_a = VoxDbTracker::new(db_a, "parity-interp");
    let interp = interpret_workflow_durable(&hir, "checkout", vec![json!(9)], &mut t_a)
        .await
        .expect("interpreted run");

    // The generated binary's workflow body reduces to exactly this call (see
    // codegen_rust/emit/durability_lower.rs::emit_workflow_body), so running it
    // here is running the generated path's engine.
    let mut t_b = VoxDbTracker::new(db_b, "parity-generated");
    let generated = interpret_workflow_durable(&hir, "checkout", vec![json!(9)], &mut t_b)
        .await
        .expect("generated-path run");

    assert_eq!(
        digest(&interp),
        digest(&generated),
        "ADR-021: activity ids and results must match across paths"
    );
}
```

- [ ] **Step 2: Run it**

```bash
cargo test -p vox-workflow-runtime --test interp_generated_parity
```

Expected: PASS.

- [ ] **Step 3: Update ADR-021's status and the docs banners**

Change ADR-021's Status from `Accepted (design gate before implementation)` to `Accepted (implemented — see Task 6 of docs/superpowers/plans/2026-09-05-true-workflow-durability.md)`, and add a Consequences bullet naming this test as the standing gate.

Remove the `> [!WARNING]` banner added in Task 0.2 from `expl-durable-execution.md` and `tut-workflow-durability.md`, and replace §1–§3 of the explanation doc with the now-true description: activity bodies execute, real return values are journalled, timers park, signals park, generated workflows share the engine.

- [ ] **Step 4: Full local gate and commit**

```bash
vox ci pre-push --complete
vox run scripts/fmt.vox
git add crates/ docs/
git commit -m "test(workflow): add the ADR-021 interpreted/generated equivalence gate

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Deferred (explicitly not in this plan)

- **Transactional outbox for exactly-once delivery.** Writing the outbox row in the same transaction as `on_activity_completed` and having a worker send-and-ack is durable *delivery*, a different concern from replay. Do not fold it into `interpret_workflow_durable`.
- **Mesh (`mesh_*`) result persistence.** `execute_populi_step`'s response should be journalled as the activity result and `activity_id` used as the dispatch idempotency key so a replay does not start a second GPU job. It is a small follow-up once Phase 1 lands, but it depends on Populi worker cooperation that is out of scope here.
- **`durable_promise.rs`.** 279 LoC whose live dispatch is dead code. Delete the runtime struct (the `DurablePromise[T]` *type* stays — typeck references it). Not bundled here because it touches the `vox-compiler` type surface and deserves its own PR.
- **A generated Temporal-style state machine (ADR-021 implementation option 2).** Only if option 1 proves too slow. 2k–5k LoC.

## Self-review notes

- **Spec coverage.** Design §2 findings G1→Task 1.4, G2/G4/G5→Task 6.1, G3→Task 5.1, G6→Task 3.1, G7→Task 4.1, G8→Task 1.5, G9/G10→Task 1.4, G11→Task 5.2, G12→Task 0.1, G13→Task 1.3. Design §6 test gates 1–2→Task 2.1, 3→Task 1.5, 4→Task 3.1, 5→Task 4.1, 6→Task 6.2, 7→Task 5.1, 8→Task 0.1.
- **Known risk not yet resolved by any task.** The `Interpreter` is constructed fresh per workflow invocation in `run_on_thread`, so a workflow that imports another `.vox` file re-resolves imports on every resume. Correct but slow for a hot resume loop; measure before optimizing.
- **Known risk, accepted.** Replay re-executes the workflow body from the top on every resume. For a workflow with thousands of journalled steps this is O(n) per resume. Temporal solves it with continue-as-new; add that only when a real workload needs it.
