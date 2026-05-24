---
title: "ADR 041: Durable functions completion (workflow, activity, actor, @scheduled)"
description: "Records the closure of the parse-only stub gap identified in ADR-028. The grammar features are now backed by working runtime, codegen, journal-backed replay, and a scheduler loop."
category: "Architecture Decisions (ADRs)"
status: "accepted"
last_updated: "2026-05-23"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 041: Durable functions completion

## Status

**Accepted (2026-05-23).** Supersedes [ADR-028](028-deprecate-stub-durability-grammar.md).

(Numbering note: this ADR was originally drafted as "ADR-029" in
`docs/superpowers/plans/2026-05-23-durable-functions-completion.md` Task 7.1,
but ADR-029 was already taken by `029-formal-intent.md` — itself renumbered
from 024 on 2026-05-02 — so this entry takes the next free slot, 041.)

## Context

ADR-028 (proposed 2026-05-01) recommended removing `@durable`, `@scheduled`,
`workflow`, and `activity` from the public grammar because the 2026-05-01
durability audit found them to be parse-only with no runtime backing.

Between 2026-05-01 and 2026-05-23, the implementation work was completed:

- `codegen_rust/emit/durability_lower.rs` emits real runtime calls
  (`interpret_workflow_durable`, `journal::execute`, `spawn_process`).
- The three runtime symbols the codegen referenced (`current_hir_module`,
  `extract_terminal_return`, `journal::execute`) were implemented (Phase 1).
- End-to-end and crash-replay tests prove the integration works
  (Phases 2–3). The interpreter walks user-defined activity calls via a
  newly-added `HirExpr::Try` arm in `workflow/plan.rs:collect_from_expr`
  (caught and fixed during Phase 2 testing).
- `@scheduled` got a persistent scheduler loop with crash-safe state
  via a `scheduled_runs` DB table (Phase 4).
- Actor handlers auto-wire from generated `main()` via a new
  `ActorRegistry` (Phase 5.2) and the `emit_main_boot` codegen
  (Phase 5.1) — note: `emit_main_boot` carries `phase5-followup` TODOs
  for HIR embedding and HTTP server boot that remain future work.
- A determinism lint blocks non-deterministic ops
  (`std.time.now_ms`, `std.random`, `std.uuid`, `std.process.spawn`)
  in workflow bodies (Phase 6).

## Decision

1. Retain `@durable`, `@scheduled`, `workflow`, `activity`, and `actor` as
   public grammar features.
2. The durable runtime is **Stable** for the supported subset documented
   in ADR-019 (linear activity execution, deterministic `if` branches,
   `workflow_wait` timer replay).
3. Out-of-subset features (arbitrary `match` replay, unbounded loops,
   non-deterministic conditions inside workflows) remain explicit
   non-goals as ADR-019 §5 specifies. The determinism lint enforces this.
4. The `vox check` CLI gate at the user-facing layer that rejected
   `workflow`/`activity` with `E0001`/`E0002` must be removed in this
   ADR's enactment (Task 7.2/7.3 will surface it; the gate is in
   `crates/vox-cli` and emits an ADR-028-style reservation error).
5. Migration of golden examples (`checkout_workflow.vox`) from plain
   `fn` to `workflow`/`activity` lands alongside this ADR (Task 7.3).
6. **Three tracked follow-ups** from the 2026-05-23 final code review:

   a. **Scheduler restart deadline derivation.** When `vox-workflow-runtime/src/scheduled/runner.rs` (lines 88-116) restarts and a registered function's persisted `next_due_at_ms` is in the future, the in-memory `Instant` deadline is currently seeded to `now + interval` instead of `now + (next_due_at_ms - now)`. This means a crash 23 hours into a `@scheduled("1d")` interval re-arms a full day instead of firing in ~1 hour. The DB row is correct; only the in-memory derivation is the gap. Fix: add `scheduled_runs_next_due_at_ms(name)` to the DB facade; consult it before falling back to `interval`.

   b. **HIR embedding for `current_hir_module()`.** `emit_main_boot` currently emits the registration as a TODO comment, not a real call. Generated binaries link but workflow bodies panic at runtime calling `current_hir_module()` (the OnceLock is unset). Interpreter path (`vox mens workflow run`) is unaffected. The tier-table row in the README scopes the Stable claim to the interpreter path until this lands.

   c. **HTTP server boot in generated `main()`.** No `vox-http-runtime::serve()` symbol exists today; the HTTP wiring still lives inside `emit_main` in `http.rs`. Factoring this into a reusable runtime function is the path forward.

These follow-ups do NOT block this ADR's acceptance.

## Consequences

- The README, the docs site landing page (`index.mdx`), and the design
  system kit can claim durable execution truthfully for the supported
  subset (Tasks 7.2 and 7.4).
- The "Durable Runtime" stability tier moves from 🟡 Preview to 🔵 Stable
  for the supported subset (Task 7.2 reflects this in the tier table).
- Future durability work (HIR embedding so generated workflows can call
  `current_hir_module()` for activity body lookup; HTTP server-runtime
  integration for combined boot; unrestricted control-flow replay;
  mesh-distributed workflow dispatch) is tracked under separate ADRs
  and follow-up plans.

## Related

- [ADR-019: Durable workflow journal contract v1](019-durable-workflow-journal-contract-v1.md) — replay contract (Accepted)
- [ADR-021: Generated workflow durability parity](021-generated-workflow-durability-parity.md) — design gate (Accepted)
- [ADR-028: Remove stub durability/scheduling grammar](028-deprecate-stub-durability-grammar.md) — superseded
- `docs/superpowers/plans/2026-05-23-durable-functions-completion.md` — implementation plan
- `docs/src/architecture/durability-runtime-audit-2026.md` — the 2026-05-01 audit (historical)
