---
title: "Durability & Scheduling Runtime Audit (2026)"
description: "Definitive audit of @scheduled, @durable, DurabilityKind, actor/workflow/activity keywords — what parses vs. what executes. Verdict: zero runtime implementation across all features."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Audit establishing that all Vox durability/scheduling features are syntax-only with no runtime semantics; required reading before any Phase 4 implementation work."
---

# Durability & Scheduling Runtime Audit (2026)

**Conducted:** 2026-05-01  
**Scope:** `@scheduled`, `@durable`, `DurabilityKind`, `actor`/`workflow`/`activity` keywords, `vox-orchestrator`

## Verdict Summary

| Feature | Status | Risk |
|---------|--------|------|
| `@scheduled("1h")` | **Parses only** | High — users may expect a cron loop |
| `@durable` decorator | **Parses only** | High — durability semantics not implemented |
| `DurabilityKind::Workflow` | **Parses only** | High — no journal, saga log, or replay |
| `DurabilityKind::Activity` | **Parses only** | High — no retry/checkpoint wrapper |
| `DurabilityKind::Actor` | **Partially wired** | Medium — handler splitting works; mailbox dispatch doesn't connect to generated code |
| `actor`/`workflow`/`activity` keywords | **Syntax unified** | Medium — compile identically to `fn` |
| `vox-orchestrator` | **Active, orthogonal** | Low — operates at agent-session level, not function durability |

**No passing integration tests exist** for any scheduler invocation, journal replay, or actor mailbox integration.

---

## Detailed Findings

### `@scheduled("1h")` — Parses Only

- **AST:** `ScheduledDecl { interval: String }` in `crates/vox-ast/src/decl/fundecl.rs:176-182`.
- **HIR lowering:** `hir/lower/mod.rs:322-326` sets `HirFn { schedule_interval: Some(interval) }`. The string is preserved.
- **Codegen:** No reference to `schedule_interval` anywhere in `crates/vox-compiler/src/codegen_rust/`. Emits a plain `async fn`.
- **Runtime:** No scheduler loop, no `tokio::time::interval`, no cron wiring anywhere in `crates/vox-actor-runtime/`.
- **Golden:** `examples/golden/scheduled_tick.vox` compiles and lowers (counted in `golden_vox_examples_test.rs:97`), but no invocation test exists.

**Conclusion:** The interval metadata travels from parse to HIR and stops. It is dead code beyond the HIR.

---

### `@durable` / `DurabilityKind` — Parses, Lowers, No Engine

- **Variants** (`crates/vox-compiler/src/hir/nodes/durability.rs:13-23`):
  - `Workflow` — "survives crashes; calls to activity functions replayed via journal"
  - `Activity` — "retried on failure, never replayed mid-execution"
  - `Actor` — "stateful entity with message handlers — isolated memory, serialised handler dispatch"
- **HIR attachment:** `HirFn { durability: Option<DurabilityKind> }` set at `hir/lower/mod.rs:297, 302, 307`.
- **Codegen:** The `durability` field is never read in `codegen_rust/`. All three variants emit identical plain `async fn` Rust code.
- **Runtime projection:** `RuntimeProjectionModule` in `runtime_projection.rs:31-45` captures DB planning but ignores durability metadata entirely.

**Conclusion:** The HIR accurately describes *what these features should do*. Nothing downstream implements those semantics.

---

### `actor` / `workflow` / `activity` Keywords — Phase 2 Syntax Unification

Per [AGENTS.md §Grammar Unification](../../../AGENTS.md): "They lower to `HirFn { durability: Some(DurabilityKind::_) }` — no separate HIR node types."

- **Actor:** Handler splitting into per-handler `HirFn` entries works at HIR level (`hir/lower/mod.rs:305-314`). `vox-actor-runtime/src/scheduler.rs` has actor mailbox/spawn/dispatch logic, but the generated Rust functions do not connect to it — the wiring is manual.
- **Workflow / Activity:** Parsed, lowered with correct `DurabilityKind`. Generated Rust is a plain `async fn`. `examples/golden/checkout_workflow.vox` does **not** use the `workflow`/`activity` keywords — it uses plain `fn`.
- **No integration tests** for any of the three execution models.

---

### `vox-orchestrator` — Active but Orthogonal

The crate (~200 `.rs` files) handles model selection, context injection, A2A message bus, remote dispatch, and agent session management. It is the canonical replacement for the retired `vox-dei` crate.

It has **zero references** to `DurabilityKind`, `schedule_interval`, or workflow journals. It operates at the AI agent session level, not at the function-durability level. It is unrelated to the scheduling and durability gaps above.

---

## Consequences for Phase 4

The interop plan's Phase 4 requires an honest resolution of these stubs before documenting or shipping them. The options, per the plan:

1. **Fix the wiring** — implement the scheduler loop for `@scheduled`, a retry wrapper for `@durable Activity`, and a journal engine for `@durable Workflow`. This is substantial work; each is roughly a separate sub-project.
2. **Remove the decorators** from the grammar (with a deprecation cycle) until each can be re-introduced with a real implementation.

**Recommended path:** Remove `@scheduled` and `@durable` from the public grammar in the next release, retaining them as internal/reserved identifiers. Re-introduce each when its runtime is ready. The `actor` keyword can stay since the handler-splitting HIR work is real; document the current limitation (no auto-mailbox integration) explicitly.

The HIR `DurabilityKind` variants and `schedule_interval` field are worth keeping as internal compiler metadata — they will be needed when the runtime implementations land.

---

## Files Referenced

| File | Relevance |
|------|-----------|
| `crates/vox-ast/src/decl/fundecl.rs:176-182` | `ScheduledDecl` AST node |
| `crates/vox-compiler/src/hir/nodes/durability.rs:13-23` | `DurabilityKind` variants |
| `crates/vox-compiler/src/hir/lower/mod.rs:297-326` | Lowering for all three kinds |
| `crates/vox-compiler/src/codegen_rust/` | Absent: no durability codegen |
| `crates/vox-actor-runtime/src/scheduler.rs` | Actor mailbox (partial); no cron loop |
| `examples/golden/scheduled_tick.vox` | Parses/lowers; no runtime test |
| `examples/golden/counter_actor.vox` | Notes: "persistent state inside actor blocks not parsed yet" |
