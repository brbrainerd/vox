# 2026-05-19 — v1.0 execution plan (post-design-decisions)

**Status:** superseded 2026-05-21 by [`2026-05-21-v1-honest-completion-plan.md`](2026-05-21-v1-honest-completion-plan.md).

> **Why superseded.** This plan treated "code lands + unit tests pass" as
> the v1.0 acceptance bar. An adversarial audit on 2026-05-21 against
> `docs/src/architecture/v1-release-criteria.md` showed every CR-L
> gate that requires LLM-panel measurement was still in
> corpus-inventory mode, and CR-L8's telemetry pipeline was never
> wired. The honest-completion plan adds measurement-evidence gating
> (`vox audit --gate all --strict-block-ga`, evidence ledger,
> arch-check Rule 14) and a real list of what's still open.
>
> **What this plan is still useful for.** The language/codegen items
> (HirEndpointKind::Stream, workflow/activity unlock, SSE codegen,
> primary-key enforcement, polymorphism fixes) all landed and are
> still the canonical reference for what shipped between 2026-05-17
> and 2026-05-21. Treat the doc body below as a historical changelog,
> not as an action item.

**Owner (historical):** Claude (in-session) for Phases A–C; Phase D needs a council checkpoint.
**Supersedes:** the "pending" list in `2026-05-18-v1-completion-plan.md` (which now lives as historical context). This doc captures the decisions made 2026-05-19 and the implementation slices to execute them.

## Decisions locked 2026-05-19

| Question | Decision | Rationale (paraphrased) |
|---|---|---|
| Streaming model | **A + C** (tick-on-schedule + actor subscribe) | Best-of-both: A unblocks dashboards immediately, C is the natural fit for chat. Decorator vs body-content are orthogonal so the codegen branches don't collide. |
| `@table` primary key | **Explicit-required (Option iii)** | No magic, no forgotten primary keys. `@table type T {}` errors unless author writes `id: int` or uses `@table(pk: <field>)`. |
| Raw SQL escape hatch | **Reject; typed DSL only** | Forces the typed query DSL to be expressive enough for real-world queries. Any "I can't express X" report becomes a DSL-coverage bug, not an escape-hatch waiver. |
| `workflow` / `activity` (ADR-028) | **v1.0 must-have** | Differentiator vs plain web frameworks. Reserved-but-rejected parsing already exists; the lift is the runtime + replay machinery. |

---

## Phase A — Parallel grind (no decisions needed)

These ship in any order. None requires further design.

### A1 — `vox deploy` reference docs
**Path:** `docs/src/reference/cli-vox-deploy.md` (new)
**Scope:**
- One section per deploy target (`container`, `compose`, `kubernetes`, `bare-metal`, `fly`, `coolify`).
- For each: what it does in plain English, the `Vox.toml` block that activates it, an example, and the `vox deploy --dry-run` output shape.
- A "choosing a target" table at top: latency / portability / ops-complexity per target.
**Acceptance:** every `DeployTarget` variant from `vox-deploy-codegen` has a docs section; the page renders in mdbook without warnings.

### A2 — CR-L2 MENS-on-distribution runner
**Path:** `crates/vox-audit/src/subcommands/mens_on_distribution.rs` (already exists; currently partial).
**Plain-English goal:** measure whether code our model emits "looks like" the kind of code humans write in Vox. Catches drift where the model invents weird patterns nobody actually uses.
**Approach:**
- Build a small distribution profile from `examples/golden/` + `apps/marquee/` (extracted via existing HIR walker): which constructs appear at what frequency.
- For each candidate code snippet (passed in via the CR-L runner harness), compute the same profile.
- Distance metric: 1 − cosine-sim across construct-frequency vectors. Threshold at ≥0.95 = on-distribution.
**Acceptance:** runner produces a real number; test fixture proves a known-good snippet scores ≥0.95 and a known-weird one (e.g. emoji identifiers, heavy macro abuse) scores <0.5.

### A3 — CR-L4 plan-mode fidelity runner
**Path:** `crates/vox-audit/src/subcommands/plan_fidelity.rs` (currently real but minimal).
**Plain-English goal:** measure whether multi-step plans (e.g. "scaffold app, add table, add endpoint, run tests") actually succeed when executed step-by-step.
**Approach:** the 5 fixtures already in `contracts/eval/plan-fidelity/plans/001-005/plan.toml` are the corpus. For each plan, the runner executes its steps in order and reports per-step pass/fail. Aggregate: % of plans where ALL steps pass.
**Acceptance:** runner produces a per-plan + aggregate pass-rate; CI gate fails if pass-rate < 80% (initial threshold; tightens later).

### A4 — Compiler/syntax polish (small fixes I've noticed)

**A4a. Better "retired decorator" error messages.** Today `@server fn` produces:
```
Unexpected token at top level: query
```
Should be:
```
@server is retired — use @endpoint(kind: server) instead.
Same for @query → @endpoint(kind: query), @mutation → @endpoint(kind: mutation),
@health → plain fn, @metric → plain fn.
```
**Scope:** add a recovery path in `parser/descent/mod.rs` that peeks for `@server`/`@query`/`@mutation`/`@health`/`@metric` *as a name token* (not a real token), emits the actionable diagnostic, and skips to the next declaration.

**A4b. Polymorphic instantiation on field/method lookups.** The 2026-05-19 ident fix only covers `HirExpr::Ident`. Same drift likely affects `HirExpr::FieldAccess` and `HirExpr::MethodCall` for generic-typed receivers (e.g. `list.first()` returning `Option[T]`). Apply `uf.instantiate` at lookup sites.

**A4c. Expected-type propagation through `match` arms.** Today `let r: Result[X, MyErr] = match cond { ... => Ok(...), ... => Error(...) }` — does `Error` constructor pick up `MyErr` from the expected type? Likely no. Same fix shape: check_expr on each match arm with `expected` threaded through.

**Acceptance:** new pipeline tests for each item; existing tests still pass.

### A5 — HumanEval-Vox corpus growth (30 → ~60 this batch)
**Path:** `contracts/eval/humaneval-vox/problems/031-060/{spec.toml,reference.vox,tests.vox}`.
**Scope:** authoring work. Each fixture is a small programming problem (sort, filter, count, accumulate, string manipulation). Roughly 1 hour per fixture if you sequence them in order of difficulty.
**Acceptance:** `contracts/eval/humaneval-vox/manifest.v1.yaml` `count_current` bumps to 60; held-out manifest regenerated; HumanEvalRunner produces a pass-rate report against the expanded corpus.

**Note:** going from 30 → 164 is a separate, longer track. Phase A bites off the next 30; the rest is post-v1.0 unless we accelerate authoring.

---

## Phase B — Streaming (Option A + Option C)

### B1 — Parser: interval syntax on `@endpoint(kind: stream)`

```vox
@endpoint(kind: stream, every: "1s")
fn current_temperature() to int { return read_sensor() }
```

`every: "<duration>"` is optional. Duration parser: `"1s"`, `"500ms"`, `"5m"`. Accepts `s`/`ms`/`m`/`h` suffixes. Stored as `Option<Duration>` on `HirEndpointFn`.

### B2 — Parser/typeck: `subscribe(ActorName)` builtin

```vox
@endpoint(kind: stream)
fn watch_room() to Stream[Message] {
    return subscribe(ChatRoom)
}
```

`subscribe` is a built-in fn registered in `typeck::builtins` with signature:
```
fn subscribe<A: actor>(actor_type: A) -> Stream[A::BroadcastType]
```

The broadcast type comes from the actor declaration — for v1.0, this is `Ty::Str` (every actor broadcasts a string until per-actor message types ship). Stretch goal: type the broadcast via the actor's `on broadcast(...) to T` handler.

### B3 — Codegen: SSE response shape

`crates/vox-codegen/src/codegen_rust/emit/http.rs::emit_server_fn_handler` already handles `HirEndpointKind::Stream` by aliasing to Server. New branch:

```rust
HirEndpointKind::Stream => emit_sse_handler(sf, ...),
```

`emit_sse_handler` generates:
- An `async fn` returning `axum::response::sse::Sse<S>` where `S: Stream<Item = Result<Event, Infallible>>`.
- If `every: Some(duration)` is set: use `tokio_stream::wrappers::IntervalStream` + `StreamExt::map`. Each tick invokes the body (inlined) and pushes the result as a `data:` event.
- If body's tail expression is `subscribe(Actor)`: wire to the actor's broadcast channel via `vox-actor-runtime::SubscriptionManager::subscribe(name)`.
- Heartbeat: `KeepAlive::default()` so proxies don't drop idle connections.
- `Content-Type: text/event-stream` (axum handles this automatically).

New runtime deps in generated apps' Cargo.toml: `tokio-stream = "0.1"`, `futures-util = "0.3"`.

### B4 — Actor broadcast channel bridge

`crates/vox-actor-runtime/src/subscription.rs::SubscriptionManager` needs:
- `pub fn subscribe(&self, actor_name: &str) -> BroadcastStream<String>` — returns a tokio `BroadcastStream` over an actor's outgoing channel.
- The actor's `on broadcast(msg: str)` handler (when present) sends into this channel.

The actor body uses a stdlib-like `broadcast(msg)` call to push to subscribers (next phase: when actor body wants to broadcast, it calls `broadcast(...)` which `vox-actor-runtime` routes to the subscription channel).

### B5 — Tests
- Chat marquee `watch_room` streams a sequence of JOIN/MSG/LEAVE events when ChatRoom's handlers broadcast.
- Dashboard test: a `@endpoint(kind: stream, every: "100ms") fn counter() to int { return read_counter() }` streams an integer per tick.
- Integration test: HTTP client connects, receives N events, disconnects cleanly.

**Acceptance:** chat marquee no longer needs the "watch_room returns a static tick" placeholder — it actually streams events.

---

## Phase C — Explicit `@table` primary key

### C1 — Parser: `@table(pk: <field_name>) type Foo { ... }`

The `@table` decorator already accepts no-arg form. Add optional `(pk: <ident>)` argument:
```vox
@table(pk: ulid) type Order {
    ulid: str
    amount_cents: int
}
```

If `@table` (no args) is used, fall back to default: the table must contain a field named `id`.

### C2 — Typeck: enforce primary key existence

After `register_hir_table`:
1. If `@table(pk: X)`: assert field `X` exists; otherwise error `E1041 — primary key field 'X' not found on @table 'Foo'`.
2. If `@table` (no args): assert field `id` exists; otherwise error `E1042 — @table 'Foo' has no 'id' field. Either add 'id: int' or use @table(pk: <field>) to point at a different primary key.`

### C3 — Migrations
- Every existing `@table` in `examples/golden/`, `apps/marquee/`, `crates/vox-project-scaffold` templates, fixtures: audit & add `id: int` (or rename appropriate field and use `@table(pk: …)`).
- Doctor-green guards (already exist) will catch any missed migration.

### C4 — Codegen impact
Currently codegen treats every `@table` field as a regular column. With explicit pk, the codegen for table-create-DDL needs to emit `PRIMARY KEY (<pk_field>)`. The default `id` case: emit `PRIMARY KEY AUTOINCREMENT`.

**Acceptance:** new tests confirm both forms of `@table` are accepted; tables without a pk are rejected with a helpful diagnostic; SQL DDL emits the right `PRIMARY KEY` clause.

---

## Phase D — Workflow / Activity (the big one)

### D-context: what these keywords mean (plain English)

A **workflow** is a multi-step process where each step's result is saved to a log so the workflow can resume after a crash. An **activity** is one of those steps — typically something with side effects (call an API, write to a file, query an LLM).

Concrete example: a "process new order" workflow might be:
1. activity `charge_card(card_id, amount)` → returns `Result[ChargeReceipt]`
2. activity `decrement_inventory(product_id, qty)` → returns `Result[InventoryUpdate]`
3. activity `send_confirmation_email(order_id)` → returns `Result[EmailId]`

If the server crashes after step 1 succeeded but before step 2, on restart the workflow replays from the log: step 1 was successful → use the recorded receipt, don't charge the card again; then proceed to step 2.

Done well (Temporal does this in production at huge scale), workflows are how you build reliable systems that don't lose state. **For v1.0 we ship the minimum viable version of this.**

### D1 — Design: durable event log schema

New tables in `vox-db` (managed by the generated app, not hand-rolled per project):

```
workflow_runs:
    id: str  (ulid)
    workflow_name: str
    params_json: str
    status: str  ("running" | "completed" | "failed")
    result_json: str  (nullable; populated on completion)
    started_at: timestamp
    completed_at: timestamp (nullable)

workflow_events:
    workflow_run_id: str  (fk → workflow_runs.id)
    sequence: int  (monotonic per run, starts at 0)
    activity_name: str  (the user's `activity_id` in `with { ... }`)
    activity_result_json: str  (the recorded return value)
    recorded_at: timestamp
    PRIMARY KEY (workflow_run_id, sequence)
```

These tables are auto-generated as part of `vox build` when the module contains any `workflow` declaration. Schema migrations land via existing `vox-db` baseline mechanism.

### D2 — Parser: unblock `workflow` and `activity` keywords

The parser already parses these (the chatbot template used them pre-2026-05-19). The pipeline currently emits the ADR-028 reject diagnostic. Remove that reject; route `workflow` to `HirWorkflowFn` and `activity` to `HirActivityFn` (new HIR variants — or, reuse `HirEndpointFn` with a `kind: Workflow|Activity` discriminator).

### D3 — Typeck: `with { ... }` clause validation

```vox
let response = call_provider(prompt) with {
    retries: 3,
    timeout: "30s",
    activity_id: "provider-call"
}
```

Constraints:
- `retries: int` (≥ 0)
- `timeout: str` (parsed as duration; reuse Phase B duration parser)
- `activity_id: str` (must be non-empty; must be unique within a workflow body)
- Only valid on calls to functions declared `activity ...`

Diagnostic if missing fields or used on non-activity: `E1050 — 'with' clause requires activity_id, retries, and timeout; was applied to a non-activity function 'foo'`.

### D4 — Codegen: state-machine emit for workflows

For each `workflow foo(params) to T { body }`, emit (in pseudocode):

```rust
async fn vox_workflow_foo(db, params: Params) -> Result<T, WorkflowError> {
    let run_id = db.workflow_runs.insert(...).await?;
    let mut seq = 0;

    // Each `activity_call(...) with { ... }` in the body becomes:
    let result_for_step_N = {
        let existing = db.workflow_events.find(run_id, seq).await?;
        match existing {
            Some(event) => deserialize::<StepNType>(event.activity_result_json)?,
            None => {
                let result = run_activity_with_retries(activity_fn, params, retries, timeout).await?;
                db.workflow_events.insert(run_id, seq, "step_id", serialize(&result)).await?;
                result
            }
        }
    };
    seq += 1;
    // ... rest of body
    db.workflow_runs.complete(run_id, serialize(&final_result)).await?;
    Ok(final_result)
}
```

Activity calls translate to: invoke the activity fn directly (no log lookup), retry up to N times, return the result.

### D5 — Runtime: workflow start / resume entrypoints

- `vox-orchestrator` (existing crate) gets a `WorkflowRunner` that:
  - On `start(workflow_name, params)`: spawn the workflow fn in a tokio task; return the run_id.
  - On boot / re-attach: query `workflow_runs WHERE status = 'running'`, re-spawn each.
- Workflow body code from D4 handles the "replay vs first-time" branching transparently.

### D6 — Tests
- Unit: `workflow_runs` and `workflow_events` schema round-trip via vox-db.
- Integration: a 3-step workflow runs to completion, log has 3 events, final status = completed.
- **Crash test:** workflow runs step 1, simulated process kill, restart, workflow resumes — step 1's activity is NOT re-invoked (verified by side-effect counter), steps 2 and 3 complete.
- Retry test: activity returns Err 2 times, succeeds on 3rd; final workflow result is Ok.
- Failure test: activity exhausts retries; workflow status = failed, error recorded.

### D7 — Documentation
- `docs/src/reference/workflows.md` — plain-English workflow primer with the order-processing example, retry/timeout semantics, what determinism means and why workflow bodies should be deterministic outside activity calls.
- `docs/src/reference/cli-vox-workflows.md` — `vox workflow list`, `vox workflow inspect <run_id>` CLI surface (stretch — could be cut for v1.0).

**Acceptance:**
- The chatbot template (currently using `fn` placeholders) gets migrated back to use `activity` + `workflow`.
- Crash test passes.
- `vox doctor --project` flags any workflow body that calls non-deterministic builtins (current-time, random, env-var-read) outside of an activity — warning, not error, for v1.0.

---

## Effort budget

| Phase | Estimated effort | Calendar (focused) |
|---|---|---|
| Phase A — Parallel grind | 3 days | 1 week |
| Phase B — Streaming A+C | 2-3 days | 1 week |
| Phase C — Primary key | 1-2 days | mid-week |
| Phase D — Workflow MVP | 5-7 days | 2 weeks |
| **Total** | **~2 weeks focused** | **~3-4 calendar weeks** |

## Execution order

I'll work them in this sequence to minimize cross-dependency churn:

1. **A4** (compiler polish) — small, fast wins, helps every subsequent phase by improving diagnostics.
2. **C** (primary key) — small, but touches many fixtures; doing this early means later phases write `@table` correctly from the start.
3. **A1, A2, A3** (docs, runners) in parallel — independent of other work.
4. **B** (streaming) — chat marquee can finally stream.
5. **D** (workflow) — the biggest lift; chatbot template re-migrates to `activity`/`workflow` at the end.
6. **A5** (corpus growth) — interleaved opportunistically.

## What's explicitly OUT of scope for v1.0

- Raw SQL escape hatch (decided no).
- Chained query DSL (`.where().limit().select()`) — typed-only means we'll expand the typed DSL piecewise as gaps are reported, not ship a parallel chained API.
- WebSocket-specific emit shape (Option B for streaming) — SSE handles all the v1.0 use cases. WebSocket waits for first user that genuinely needs bidirectional streaming.
- `yield` keyword — superseded by A+C combination above.
- HumanEval growth past 60 — content authoring grind for v1.1+.

## Open risks

1. **Phase D's "determinism in workflow bodies" is unenforced** — relies on author discipline + the v1.0 advisory warning. Real enforcement requires effect tracking (Hopper-track territory).
2. **Phase B Option C's broadcast type is `Str` until per-actor message types ship.** Acceptable v1.0 trade-off; flagged in `docs/src/reference/actors.md`.
3. **Phase C breaks existing `@table` callers** that lack an `id` field. Migration is mechanical (add `id: int` to every existing table). Doctor-green CI guards catch the misses.
4. **No "stub" detection** in the workflow event-log lane — if a workflow body uses a non-activity side effect (e.g. calls `db.X.insert(...)` directly without wrapping in an activity), replays will double-execute. Documented; advisory linter for v1.1.
