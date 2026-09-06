---
title: "True workflow durability: corrected design"
description: "Audit-corrected design for making Vox workflow replay real — execute the workflow body in the interpreter, intercept activity calls, journal real results."
category: "Architecture SSOTs"
status: "planned"
---

# True workflow durability: corrected design

**Date:** 2026-09-05
**Supersedes the framing of:** `docs/src/architecture/durability-runtime-audit-2026.md` (2026-05-01, stale)
**Constrained by:** [ADR-019](../adr/019-durable-workflow-journal-contract-v1.md), [ADR-021](../adr/021-generated-workflow-durability-parity.md), [ADR-041](../adr/041-durable-functions-completion-2026.md)
**Implementation plan:** `docs/superpowers/plans/2026-09-05-true-workflow-durability.md`

## 1. What "true durability" means here

A workflow is durable when all six hold:

1. The **orchestrator is deterministic** — no clocks, RNG, network, or filesystem in the workflow body; those live in activities.
2. **History is the source of truth** — an activity's result is persisted before the next step starts.
3. **Resume is replay** — restart with the same `run_id`, completed activities return their journalled value without re-running.
4. **Timers and signals outlive the process** — a 24-hour wait is a row with a wake time, not `tokio::sleep` in a process that may die.
5. **Code can evolve** — inserting a line must not rename every later `activity_id`.
6. **Side effects are at-least-once**, with `activity_id` exposed as an idempotency key. Exactly-once for arbitrary I/O is not offered.

Explicit non-goals (unchanged from ADR-021): process/VM checkpointing, CRIU, "every async fn is durable", exactly-once for `process.run` or arbitrary HTTP.

## 2. Verified state of the code (2026-09-05)

Confirmed by reading the tree, not by reading the old audit.

| # | Finding | Evidence |
|---|---|---|
| G1 | **Local activities never run user code.** Every non-mesh activity returns a canned `{"event":"LocalActivity","status":"executed"}`. | `workflow/run.rs::execute_local_activity_step` |
| G2 | **The workflow body is discarded by codegen.** `emit_workflow_body` never emits `func.body`; it emits a call to `interpret_workflow_durable` against the HIR. Generated activity Rust fns exist but nothing calls them. | `vox-codegen/src/codegen_rust/emit/durability_lower.rs` |
| G3 | **Two disagreeing identity schemes.** The interpreter derives `blake3(workflow \0 name \0 position)`; codegen's `journal::execute` uses `func.generated_hash` (per-function, not per-call-site). | `run.rs::derive_activity_id` vs `durability_lower.rs::emit_activity_body` |
| G4 | **Generated workflows use `DefaultTracker`** (RAM, always "not completed"). Crash ⇒ replay from zero. | `durability_lower.rs:57` |
| G5 | **`journal::execute` is a production no-op**; persistence is behind `cfg(test)`/`test-support`. | `journal/execute.rs` |
| G6 | **Timers sleep in-process.** `workflow_wait` → `tokio::time::sleep`, persisted only *after* the sleep. | `run.rs::execute_step_once` |
| G7 | **Signals fail instead of parking.** A missing signal row is `bail!`, then the step immediately emits `SignalWaitSatisfied` anyway. | `db_tracker.rs::on_activity_started`, `run.rs::execute_local_activity_step` |
| G8 | **Control flow is a compile-time linearizer.** `if` requires `eval_const_bool` (literals only); `match`/`while`/`loop`/`break` hard-error; `for` only unrolls literal lists. | `workflow/plan.rs` |
| G9 | **Planned arguments are always `vec![]`.** The 24h dedup cache hashes nothing. | `plan.rs`, all `arguments: vec![]` |
| G10 | **`WorkflowCompleted.return_value` is always `null`**, so `extract_terminal_return::<T>` in generated code fails for any non-nullable `T`. | `run.rs`, `return_extract.rs` |

Three findings the source audit **missed**:

| # | Finding | Evidence |
|---|---|---|
| G11 | **`VoxDbTracker` silently no-ops `record_workflow_patch` / `load_workflow_patch` and the dedup cache** — it doesn't override those trait methods, and no `workflow_patch_log` table exists. `FileJournalTracker` *does* implement them. So `workflow.version(...)` is durable on mobile and not durable on the server. | `db_tracker.rs` (no impl), `schema/domains/execution.rs` (no table), `file_journal.rs:323` |
| G12 | **`run.rs` emits events the v1 schema rejects** — `WorkflowPatch` and `ActivityCacheHit` are not in the schema's `event` enum. The contract test only validates hand-built retry events, so it never catches this. | `contracts/workflow/workflow-journal.v1.schema.json` |
| G13 | **The interpreter is `!Send`.** `VoxValue` holds `Rc`. An `Interpreter` cannot be held across an `.await` in the async runner. Any design that "evals the activity body from the async loop" is unimplementable as written. | `eval/value.rs` (`Rc<Vec<VoxValue>>`) |

G13 is the constraint that shapes everything.

## 3. The correction: one engine, not two

The source audit proposes executing planned activities first (its step 1), then — 800–1500 LoC later — executing the workflow body and intercepting calls (its step 5). That ordering is wrong, because step 5 **subsumes** step 1 and deletes most of step 1's machinery.

If the workflow body is executed and activity calls are intercepted:

- real arguments appear for free (no HIR arg-capture pass — G9 dissolves),
- real results appear for free (G1 dissolves),
- `if result.ok`, `match`, `while`, `?`, and non-literal `for` all work because they are ordinary interpreter control flow (G8 dissolves — `plan.rs`'s 609-line linearizer is deleted, not extended),
- the return value is the workflow function's return value (G10 dissolves).

Doing step 1 first means building an HIR argument-capture pass and a per-step body-eval shim that step 5 then throws away. Skip it.

### 3.1 The `!Send` bridge

`Interpreter` is `!Send`; journal I/O is `async`. Resolve by confinement, not by making the interpreter `Send`:

```
async runner (tokio task)              workflow thread (std::thread, owns Interpreter)
─────────────────────────              ──────────────────────────────────────────────
spawn thread with HirModule (Clone,    interp.run_module(&hir)
  Send + Sync — already stored in a    interp.call(workflow_name, args)
  OnceLock static today)                 └─ activity hook fires on each activity call
loop {                                        req_tx.send(Begin { name, args })
  req = req_rx.recv().await                   ← blocks on resp_rx.recv()
  journal I/O via WorkflowTracker
  resp_tx.send(decision)
}
join → final VoxValue → return_value
```

The hook must not re-enter the interpreter (it is called from `&mut Interpreter`), so it returns a **decision**, not a value:

```rust
enum ActivityDecision {
    Replay(VoxValue),   // journal had a completed result — use it, don't run the body
    Execute,            // run the real body; the interpreter then reports the result back
    Park(ParkReason),   // timer not yet due / signal absent — unwind the thread
}
```

`Execute` runs the body through the interpreter's ordinary call path, then a second hook call (`after_activity`) hands the result to the async side for persistence. `Park` unwinds via a dedicated `EvalError` variant.

### 3.2 Parking replaces sleeping

A parked run **abandons its thread**. Resume re-runs the workflow from the top; every completed activity replays from the journal, so re-execution is cheap and side-effect-free. This is Temporal's model and it makes timers and signals ordinary journal entries rather than special cases:

- `workflow_wait(d)` → first visit persists `wake_at_ms = now + d` and parks. A waker resumes the run when due. Resume finds `wake_at_ms <= now`, completes the step, continues.
- `workflow_wait_signal(k)` → consume-or-park. A parked run releases its lease. `vox workflow signal <run_id> <key>` inserts the row and wakes it.

Both reuse the `@scheduled` runner's existing wall-clock-anchor pattern (`scheduled/runner.rs`), which already gets crash recovery right.

### 3.3 Identity

Position-based ids (`blake3(workflow \0 name \0 nth-activity-call-in-file-order)`) break on insertion. Replace with a **call-site path**: the `Span` of the call expression is stable under edits elsewhere in the file but not under reformatting, so use the lexical path instead — `(enclosing fn, call ordinal within that fn's body by AST walk order, invocation counter for loop iterations)`. Explicit `with { activity_id: "…" }` always wins and is checked for duplicates within a run (today two loop iterations with the same explicit id silently alias).

## 4. Ordering

| Phase | Delivers | Rough size |
|---|---|---|
| 0 | Docs stop over-claiming; schema admits the events actually emitted; the lie is testable | ~150 LoC + doc edits |
| 1 | **Workflow body executes; activities really run; results journalled; args and return value real** | ~600 LoC net (−600 deleted) |
| 2 | Crash-window proof: kill-after-started retries, kill-after-completed replays, `activity_id` reaches the activity as an idempotency key | ~150 LoC + tests |
| 3 | Durable timers (park + `wake_at_ms` + waker) | ~350 LoC |
| 4 | Durable signals (park, don't fail) + `vox workflow signal` | ~250 LoC |
| 5 | Stable call-site ids, duplicate-id detection, `workflow_patch_log` for `VoxDbTracker` (G11) | ~250 LoC |
| 6 | Generated Rust parity: `VoxDbTracker` in codegen, `run_id` from env, interp≡generated equivalence test, delete `journal::execute` (G5) | ~200 LoC + tests |
| 7 *(deferred)* | Transactional outbox for exactly-once *delivery* — a separate concern from replay | not in this plan |

Phases 3–6 are meaningless before phase 1: they would be parking and replaying `{"event":"LocalActivity","status":"executed"}`.

## 5. What is deleted, not extended

- `workflow/plan.rs` linearizer (609 LoC): `eval_const_bool`, branch-decision synthesis, `for`-unrolling, the `match`/`while`/`loop` bails. `plan_workflow_activities` has no consumers outside the crate.
- `durable_promise.rs` (279 LoC): staged, dispatch is dead code. Keep the *type* (typeck references `DurablePromise[T]`); delete the unused runtime struct.
- `journal/execute.rs` + `journal/test_support` (~120 LoC): the production path is a no-op and the id scheme disagrees with the runner (G3). Phase 6 removes the codegen emit that calls it.

## 6. Test gates

A phase is not done until its gate passes, and each guard is verified **by mutation** (break it, confirm the test fails, restore) per AGENTS.md §PR & Review Discipline.

1. **Crash after complete** — kill after `on_activity_completed`; resume; that activity's body is *not* invoked (proved by a side-effect counter, not by an event name); later activities run.
2. **Crash during activity** — kill after `started`, before `completed`; resume retries; attempt log increments.
3. **Runtime branch** — `if charge.ok { … } else { … }` where `charge` is an activity result; both branches reachable; replay takes the same branch.
4. **Timer** — wait 5 s, crash at 2 s, resume; total wall-clock ≈ 5 s, not ≈ 7 s.
5. **Signal** — start with an empty signal log; the process exits `waiting` (not `failed`); insert the signal; resume completes.
6. **Interp ≡ generated** — same `run_id`, same `activity_id` set, same `result_json` for a linear workflow (ADR-021 gate).
7. **Code upgrade** — append a trailing activity; an old `run_id` still completes without renaming prior ids.
8. **Schema** — every event the runner actually emits validates against `workflow-journal.v1.schema.json`, asserted over a real journal, not a hand-built one (closes G12).

## 7. Related

- [ADR-019: Durable workflow journal contract v1](../adr/019-durable-workflow-journal-contract-v1.md)
- [ADR-021: Generated workflow durability parity](../adr/021-generated-workflow-durability-parity.md)
- [ADR-041: Durable functions completion](../adr/041-durable-functions-completion-2026.md)
- [Explanation: Durable Execution](../explanation/expl-durable-execution.md)
