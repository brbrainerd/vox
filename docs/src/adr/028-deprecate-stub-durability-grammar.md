---
title: "ADR-028: Remove Stub Durability/Scheduling Grammar from Public API"
description: "Proposes removing @scheduled, @durable, workflow, and activity from the public Vox grammar, retaining actor with documented limitations, following the 2026-05-01 durability runtime audit."
category: "Architecture Decisions (ADRs)"
status: "deprecated"
last_updated: "2026-05-23"
training_eligible: true
---
# ADR 028: Remove Stub Durability/Scheduling Grammar from Public API

## Status

**Superseded by [ADR-041](041-durable-functions-completion-2026.md) (2026-05-23).**

The 2026-05-01 audit found `@durable`/`workflow`/`activity` were parse-only stubs. Subsequent implementation work (Phases 1–6 of `docs/superpowers/plans/2026-05-23-durable-functions-completion.md`, completed 2026-05-23) closed the gap: codegen emits runtime calls; the runtime executes them with journal-backed replay; @scheduled has a persistent scheduler loop; actors auto-wire via main_boot. The grammar features are retained as public API. The original "Proposed" recommendation to remove them from the grammar is withdrawn.

The audit findings below remain historically accurate as of 2026-05-01 and are preserved for the record.

## Relationship to AGENTS.md (non-normative clarity)

[`AGENTS.md`](../../../AGENTS.md) Grammar Unification keeps `actor`, `workflow`, and `activity` as **supported bare keywords** (they lower to `HirFn { durability: Some(DurabilityKind::_) }`). **`@durable` and `@scheduled` remain valid decorator syntax** today. This ADR does **not** override that policy until moved to **Accepted** and the compiler/parser change ships.

**Separation of concerns:** “Syntax accepted by the compiler” and “durable execution implemented in the runtime” are different bars; see the audit below.

## Context

A durability runtime audit conducted on 2026-05-01 ([full findings](../architecture/durability-runtime-audit-2026.md)) established that four public grammar features — `@scheduled`, `@durable`, `workflow`, and `activity` — are **parse-only with zero runtime implementation**.

The audit findings in detail:

- **`@scheduled("1h")`** — Parsed into `ScheduledDecl { interval: String }` (`fundecl.rs:176-182`). HIR lowering preserves the string in `HirFn { schedule_interval: Some(interval) }` (`hir/lower/mod.rs:322-326`). Codegen has no reference to `schedule_interval` and emits a plain `async fn`. No scheduler loop, no `tokio::time::interval`, no cron wiring exists anywhere in `crates/vox-runtime/`.

- **`@durable` / `DurabilityKind::Workflow` / `DurabilityKind::Activity`** — All three variants are parsed and lowered to `HirFn { durability: Some(DurabilityKind::_) }` (`hir/lower/mod.rs:297-307`). The `durability` field is never read in `codegen_rust/`. All variants emit identical plain `async fn` Rust code. The `RuntimeProjectionModule` ignores durability metadata entirely.

- **`workflow` / `activity` keywords** — Parsed and lowered with correct `DurabilityKind` annotations. Generated Rust output is a plain `async fn`. The golden fixture `examples/golden/checkout_workflow.vox` does not use these keywords — it uses plain `fn`.

- **No passing integration tests** exist for any scheduler invocation, journal replay, or actor mailbox integration.

Shipping these features as public API would mislead users into expecting durable execution semantics — crash-safe replay, scheduled invocation loops, retry wrappers — that the runtime does not provide. Since Vox is pre-1.0 with zero known external production users, the cost of a breaking grammar change is low and the benefit is high: the public API accurately represents what the language can do.

The `actor` keyword is a partial exception. Handler splitting into per-handler `HirFn` entries works correctly at the HIR level (`hir/lower/mod.rs:305-314`), making it meaningfully distinct from a plain `fn`. The actor mailbox/spawn/dispatch logic in `crates/vox-runtime/src/scheduler.rs` is real, but the generated Rust functions do not yet connect to it automatically. This is an incomplete wiring problem, not a parse-only stub.

## Decision

1. **Remove `@scheduled`, `@durable`, `workflow`, and `activity` from the public grammar** in the next minor release.

   - These tokens are retained as **reserved identifiers**: the parser recognizes and rejects them with a clear diagnostic, e.g.:
     ```text
     error: `@scheduled` is not yet implemented — see tracking issue #<N>
     ```
   - The rejection happens at parse time so that users of future versions who add these features back do not silently compile to no-ops if they accidentally target an older toolchain.

2. **Retain the `actor` keyword** in the public grammar.

   - Add an explicit documentation note to the language reference stating that auto-mailbox wiring is not yet connected: actor handlers are dispatched manually, not through the generated code. This will be removed when the wiring is complete.

3. **Retain internal HIR fields** `HirFn::schedule_interval` and `HirFn::durability` (`DurabilityKind` variants `Workflow`, `Activity`, `Actor`).

   - These will be needed when the runtime implementations land. Removing them now would create churn with no benefit.

4. **Provide a mechanical migration command**: `vox migrate drop-durability-stubs` rewrites source files that use the removed decorators/keywords to plain `fn`, allowing any internal or experimental code to be updated without manual effort.

## Consequences

- **Breaking change:** Any code using `@scheduled`, `@durable`, `workflow`, or `activity` will fail to compile after this change. Based on the audit, there are **zero known external users** of these features (the language is pre-1.0 and the golden examples do not use these keywords in runtime-exercised tests).

- **Internal examples:** `examples/golden/scheduled_tick.vox` and related files that currently parse-and-lower (but do not have runtime tests) will need to be rewritten or removed. `vox migrate drop-durability-stubs` handles this mechanically.

- **Honest public API:** After this change, the Vox grammar surface accurately reflects what the runtime can execute. Users who reach for scheduling or workflow durability receive a clear error pointing to the tracking issue rather than silently compiling to a plain async function that will never be scheduled.

- **Re-introduction path:** Each feature is re-introduced in a separate minor release when its runtime is ready: the scheduler loop for `@scheduled`, the retry wrapper for `@durable Activity`, and the journal/replay engine for `@durable Workflow`. The HIR metadata required for all three is preserved.

- **`actor` documentation debt:** The retained `actor` keyword incurs a documentation obligation. The language reference and any tutorial content using `actor` must be updated to describe the current limitation before this ADR is considered closed.

## Alternatives Considered

### (a) Retain as `#[experimental]` or `#[unstable]`

Mark the features with an experimental or unstable attribute that emits a compiler warning, keeping them in the grammar.

**Rejected.** The Vox grammar has no experimental marker today. Introducing one solely to preserve stubs would add complexity without fixing the underlying problem: users would still compile `@scheduled` functions and observe no scheduling behavior. A warning is easily missed; a hard error pointing to a tracking issue is not.

### (b) Implement the runtime now

Implement the scheduler loop, journal engine, and retry wrapper before removing anything from the grammar.

**Deferred, not rejected.** Each of these is a separate sub-project of substantial scope. The scheduler loop requires a persistent timer store and process restart semantics. The journal/replay engine requires a write-ahead log, deterministic re-execution, and side-effect suppression. The retry wrapper requires failure classification, backoff policy, and idempotency keying. Bundling all three into a single release would significantly delay the next Vox minor release. This ADR records the intent to implement them; it does not permanently abandon the features.

## Related

- [Durability runtime audit (2026-05-01)](../architecture/durability-runtime-audit-2026.md)
- [ADR 019: Durable workflow journal contract v1](019-durable-workflow-journal-contract-v1.md)
- [ADR 021: Generated workflow durability parity](021-generated-workflow-durability-parity.md)
- `crates/vox-compiler/src/hir/nodes/durability.rs` — `DurabilityKind` variants
- `crates/vox-compiler/src/hir/lower/mod.rs:297-326` — Durability lowering
- `crates/vox-runtime/src/scheduler.rs` — Actor mailbox (partial)
