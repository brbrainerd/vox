---
title: "Vox Language Rules — Phase 4: Runtime Monitors (2026-05-09)"
description: "Step-by-step plan to add runtime safety nets that the compiler can't reach: per-call fuel decremented per HIR step, allocation observer with cap, stack-depth cap, panic-trap boundary on every vox run invocation, runtime telemetry redactor for @secret-tagged fields, capability-violation runtime trap, idiom fingerprint telemetry export, deterministic-seed playground mode for examples, and a per-call sandbox under vox-bounded-fs. Defaults are CI-strict, end-user-friendly with --strict opt-in."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 4 child plan. Runtime monitors are Rust-side defense-in-depth; everything here is independent of compiler changes and can ship in parallel with Phase 2."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-eval: fuel, alloc-observer, stack-depth, panic-trap"
  - "vox-bounded-fs: per-call sandbox in CI mode"
  - "vox-capability-registry: runtime trap on builtin call outside declared effect set"
  - "vox-codegen: provenance ledger writes per emission"
  - "telemetry-trust-ssot: vox.runtime.* and vox.idiom.* events documented"
---

# Phase 4 — Runtime Monitors

> **Parent plan:** [`vox-language-rules-and-enforcement-plan-2026.md`](vox-language-rules-and-enforcement-plan-2026.md)
> **Depends on:** Phase 1 Task 8 (diagnostic catalog scaffolding) for trap-reason classification. Independent of Phase 2 and 3.
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans.

**Goal:** Ship the runtime safety nets the Vox compiler cannot enforce statically: per-call fuel decremented per HIR step, allocation observer with cap, stack-depth cap, panic-trap boundary, runtime telemetry redactor for `@secret`-tagged fields, capability-violation trap, idiom fingerprint export to the corpus pipeline, deterministic-seed playground mode for examples, and per-call sandbox under `vox-bounded-fs` in CI mode.

**Architecture:** The interpreter (`vox-eval`) gains a `RuntimeBudget` struct passed by reference through every step. `RuntimeBudget` carries `fuel: Option<u64>`, `alloc_bytes: Option<u64>`, `stack_depth: Option<u32>` — each independently set, each defaults to `None` (unbounded) for end-user `vox run` and to generous CI defaults when invoked from CI. Trap reasons emit as typed `RuntimeDiagnostic` values with stable IDs in the catalog (`vox/runtime/fuel-exhausted`, `vox/runtime/stack-overflow`, etc.).

The panic-trap boundary wraps every top-level `vox run` invocation in `std::panic::catch_unwind` and converts panics into `vox/runtime/host-panic` diagnostics. This is *defense in depth*: Vox's "panics are unrecoverable" language model is preserved; the host process simply doesn't go down.

**Out of scope for Phase 4:**
- Static effect inference (Phase 5 owns).
- Static taint type for `@secret` (Phase 5+; runtime redactor here is the cheap counterpart).
- Workflow journal verifier as a CLI (`vox workflow verify`) — belongs in `vox-orchestrator` work, not language-rules; cross-reference only.

---

## Verification setup

- `cargo test -p vox-eval --lib budget::` — fuel/alloc/stack accounting tests.
- `cargo test -p vox-eval --test runaway_traps` — synthetic infinite-loop, deep-recursion, alloc-blowup scripts each trigger the right diagnostic ID.
- `cargo test -p vox-eval --test panic_boundary` — synthetic panic-producing script returns a structured `vox/runtime/host-panic` diagnostic, not a process crash.
- `cargo run -p vox-cli -- run --fuel 1000 examples/golden/runaway.vox` — exit code is the trap-reason exit code, stderr is the structured diagnostic.

---

## Task 1: `RuntimeBudget` plumbed through interpreter

**Files:**
- Create: `crates/vox-eval/src/budget.rs`
- Modify: `crates/vox-eval/src/interpreter.rs` (or `step.rs`) — every step decrements `fuel`, checks alloc-observer hook, checks stack depth
- Modify: `crates/vox-eval/src/lib.rs` — re-export `RuntimeBudget`

**Why budget plumbing first:** Every later task depends on the budget surface existing. Land it as a no-op (all bounds `None`) first; subsequent tasks add the bound types.

**Code shape:**

```rust
pub struct RuntimeBudget {
    pub fuel: Option<u64>,
    pub alloc_bytes: Option<u64>,
    pub stack_depth: Option<u32>,
}

impl RuntimeBudget {
    pub const UNLIMITED: Self = Self { fuel: None, alloc_bytes: None, stack_depth: None };
    pub const CI_DEFAULT: Self = Self {
        fuel: Some(10_000_000),
        alloc_bytes: Some(512 * 1024 * 1024),
        stack_depth: Some(10_000),
    };

    pub fn tick(&mut self) -> Result<(), RuntimeTrap> { /* decrement fuel; trap on 0 */ }
}
```

**Verify:** Add `#[test] fn budget_unlimited_runs_forever_until_done()` and `#[test] fn budget_zero_fuel_traps_immediately()`.

---

## Task 2: Per-call fuel + diagnostic `vox/runtime/fuel-exhausted`

**Files:**
- Modify: `crates/vox-eval/src/interpreter.rs` — call `budget.tick()` per step
- Modify: `crates/vox-cli/src/run.rs` — add `--fuel <N>` flag (default `None`; `auto` resolves to `RuntimeBudget::CI_DEFAULT.fuel` when `$CI` env var is set)
- Modify: `crates/vox-code-audit/src/diagnostics/catalog.rs` — register `vox/runtime/fuel-exhausted` with `severity = "error"`, `since = "0.6.0"`
- Create: `docs/src/reference/diagnostics/runtime-fuel-exhausted.md`
- Create: `examples/golden/anti/runtime-fuel-exhausted.vox`

**Default behavior:**
- `vox run script.vox` → unbounded (no fuel check overhead).
- `vox run --fuel 10000000 script.vox` → bounded; trap emits the diagnostic.
- `vox run --fuel auto script.vox` → resolves to `CI_DEFAULT` if `$CI=true`, else unbounded.
- CI calls `vox run --fuel auto` everywhere (set in `lefthook.yml` and `.github/workflows/*`).

**Diagnostic shape (when emitted to stderr):**

```json
{
  "id": "vox/runtime/fuel-exhausted",
  "severity": "error",
  "trap_at_step": 10000000,
  "fn": "Vec[T]::shuffle",
  "trace": ["main", "process_batch", "Vec[T]::shuffle"],
  "rationale": "Fuel cap reached. The script ran for 10M HIR steps without completing. Either the script has unbounded recursion / infinite loop, or the fuel cap is too low for the workload.",
  "suggested_action": "Re-run with --fuel <larger>, or fix the runaway loop. See ADR-021 for fuel calibration guidance."
}
```

**LLM-target note:** The `trap_at_step`, `fn`, and `trace` fields are exactly what an LLM agent needs to identify the runaway loop without re-running the script.

**Verify:** Synthetic `loop { }` script, `--fuel 10` exits with the diagnostic and exit code 124 (matching POSIX `timeout` convention).

---

## Task 3: Allocation observer + diagnostic `vox/runtime/alloc-cap-exceeded`

**Files:**
- Modify: `crates/vox-eval/src/budget.rs` — wire allocation tracking
- Modify: `crates/vox-eval/src/runtime/values.rs` (or wherever `Value` allocation happens) — call `budget.observe_alloc(size)` on every `Value` construction

**Approach:** Rather than a custom allocator (intrusive, slow), instrument Value allocation in the interpreter. Captures the 95% of allocations that come from script execution; misses allocations done by host builtins (acceptable trade-off — those are bounded by the builtin's own contract).

**Default cap:** 512 MB in CI, unbounded for end-user.

**Diagnostic includes:** `alloc_bytes_at_trap`, `largest_recent_value: { kind: "Vec[T]", size_bytes: ... }`, `trace`.

**Verify:** Synthetic `let huge = []; loop { huge.push(repeat("x", 1000000)) }` — traps at the cap with the `largest_recent_value` field pointing at `Vec[Str]`.

---

## Task 4: Stack-depth cap + diagnostic `vox/runtime/stack-overflow`

**Files:**
- Modify: `crates/vox-eval/src/interpreter.rs` — increment/decrement `budget.stack_depth_used` on every call/return
- Modify: `crates/vox-eval/src/budget.rs` — `RuntimeBudget::stack_check()`

**Default cap:** 10000 frames in CI, unbounded for end-user.

**Why cheap:** Pure counter; no native stack inspection.

**Verify:** Synthetic `fn rec() { rec() }` — traps with `stack_overflow` and depth at trap.

---

## Task 5: Panic-trap boundary

**Files:**
- Modify: `crates/vox-cli/src/run.rs` — wrap the interpreter call in `std::panic::catch_unwind`
- Modify: `crates/vox-eval/src/lib.rs` — `pub fn run_caught(...)` helper that handles the catch
- Create: diagnostic `vox/runtime/host-panic`

**Why:** Vox's language model is "panics are unrecoverable from the script's POV." That's correct *for the script*, but the host process should never go down because of a script bug. The catch boundary preserves the language semantics while protecting the host.

**Approach:** Run the interpreter on a dedicated thread; `catch_unwind` on that thread; surface the panic message + (where available) the script frame at the time of panic in the diagnostic.

**Verify:** Synthetic script that triggers a host-side panic via a buggy host builtin (test harness only; production builtins should not panic) — `vox run` exits cleanly with the diagnostic.

---

## Task 6: Runtime telemetry redactor for `@secret`-tagged fields

**Files:**
- Modify: `crates/vox-eval/src/telemetry.rs` (or wherever spans are emitted) — gate every span attribute write through a `redactor` that drops fields whose source struct field carried `@secret`
- Modify: `crates/vox-actor-runtime/src/builtins/manifest.rs` — already (Phase 1 Task 3) carries `@secret` flags; surface to the redactor
- Modify: `crates/vox-eval/src/values.rs` — `Value` carries an optional `secret: bool` flag propagated by the type checker

**Approach:** Two-layer defense. The Phase 5 type system (when it lands) will refuse `@secret` field reads in `tracing` span calls statically. *This task* adds the runtime redactor that drops the field even if the static check missed it (e.g., for plugin-loaded code not yet checked).

**Diagnostic:** Each redaction emits a `vox.runtime.redacted_attribute` event with the field name and the call site (no value). Frequent emissions are a signal of static-check coverage gaps to fix.

**Verify:** Synthetic struct with `@secret password: str`; emit a span with `password` as attribute; assert the emitted span has no `password` field; assert one redaction event was emitted.

---

## Task 7: Capability-violation runtime trap + idiom fingerprint export

**Files:**
- Modify: `crates/vox-eval/src/interpreter.rs` — at every builtin call, check the calling fn's declared `@uses(...)` set against the builtin's effect set; trap on mismatch
- Create: diagnostic `vox/runtime/capability-violation`
- Modify: `crates/vox-eval/src/idiom.rs` (new) — emit `vox.idiom.<fingerprint>` events for each construct executed
- Modify: `docs/src/architecture/telemetry-trust-ssot.md` — document `vox.runtime.*` and `vox.idiom.*` namespaces

**Why both at once:** Both walk the same call sites; sharing infra is cheaper.

**Why the runtime cap-violation trap when Phase 5 will check it statically:** Plugin-loaded scripts (mens skills, vox-plugin-host code) may not be statically checkable in all loading paths. Runtime trap is defense-in-depth.

**Idiom fingerprints:** Each construct (e.g., `Result.ok_or`, `for x in y { ... }`, `match x { Some(_) => ..., None => ... }`) emits a `vox.idiom.<short-id>` event with a count. Aggregated periodically (Task 9) to compute "what % of accepted code uses Vox-distinctive forms vs Python-shaped fallbacks." Direct LLM-target metric.

**Verify:** `@pure fn x()` calling `populi.complete` (which is in the `net` effect set) → runtime trap. Idiom export covers every construct in `examples/golden/`.

---

## Task 8: `vox playground` deterministic-seed mode

**Files:**
- Modify: `crates/vox-cli/src/run.rs` — add `--seed <u64>` and `--deterministic` flags
- Modify: `crates/vox-eval/src/builtins/time.rs` and `random.rs` — when `--deterministic` is set, `time.now()` returns a fixed instant and `random.*` is seeded from `--seed`
- Modify: `crates/vox-doc-pipeline/src/lib.rs` — every doctest runs with `--deterministic --seed 1`

**Why:** LLM-generated tests for Vox examples often have intermittent failures due to time/random noise. Fixed seeds eliminate that whole class. Doctests already get this treatment; the CLI flag exposes it for end-user use.

**Constraint:** `--deterministic` requires the script to declare `@pure` or `@deterministic` on top-level fns; mixing real time/random reads with deterministic mode would produce silently wrong results, so it's rejected at runtime.

**Verify:** A doctest with `random.choice([...])` is reproducible across CI runs; without `--deterministic`, the same script produces different outputs.

---

## Task 9: `vox.idiom.*` aggregation export to corpus pipeline

**Files:**
- Create: `crates/vox-cli/src/ci/idiom_export.rs` — `vox ci idiom-export` subcommand that reads `vox.idiom.*` events from the local telemetry sink and writes a periodic JSON to `contracts/reports/idiom-fingerprints.<date>.json`
- Modify: `mens/scripts/build-corpus.vox` (or equivalent) — consume idiom-fingerprints reports as a feature for example weighting

**Why a Vox-only LLM-target win:** This is the closing of the feedback loop. The corpus pipeline learns which Vox-distinctive idioms are *actually used* (not just "should be used") and biases training accordingly. Idiom adoption rate becomes a tracked metric.

**Verify:** Run `vox ci idiom-export` after a `vox run` of `examples/golden/`; output JSON contains a non-empty `fingerprint_counts` map.

---

## Task 10: Per-call sandbox under `vox-bounded-fs` (CI mode)

**Files:**
- Modify: `crates/vox-cli/src/run.rs` — when `--sandbox` is set (default-on in CI), wrap fs operations in `vox-bounded-fs` with a fresh tmpdir as root
- Modify: `crates/vox-bounded-fs/src/lib.rs` — gain `BoundedFsBuilder::with_call_root(path)` for per-call scoping
- Modify: `lefthook.yml` and CI workflows — set `VOX_SANDBOX=true` for `vox run` invocations in pre-commit hooks

**Why:** Today the eval sandbox is "deploy a Docker, isolate that way." Per-call sandbox protects local CI runs from script bugs that would otherwise touch real paths.

**Defaults:**
- `vox run script.vox` → no sandbox (end-user gets local fs access).
- `vox run --sandbox script.vox` → fresh tmpdir; no network unless script declares `@uses(net)` (Phase 5 surfaces this).
- CI runs `vox run --sandbox auto` everywhere; auto = on.

**Verify:** Synthetic `vox run` script that writes to `/tmp/foo` — without sandbox, file persists; with sandbox, file lives in the per-call tmpdir and is cleaned up on exit.

---

## Task 11: AGENTS.md backlinks + where-things-live + telemetry-trust-ssot updates

**Files:**
- Modify: `AGENTS.md` — add §"Runtime Safety Nets" describing fuel, alloc-cap, stack-depth, panic-trap, redactor, sandbox; backlink to this phase plan
- Modify: `docs/src/architecture/telemetry-trust-ssot.md` — document the new `vox.runtime.*` and `vox.idiom.*` namespaces
- Modify: `docs/src/architecture/where-things-live.md` — add rows for `vox-eval/budget.rs`, `vox-eval/idiom.rs`

**Verify:** All three doc updates pass `vox-doc-pipeline --check`.

---

## Risks specific to this phase

| Risk | Mitigation |
|---|---|
| Fuel-tick overhead slows interpreter measurably | Benchmark before/after; if regression > 5%, switch from per-step to per-loop-back-edge ticking. The protection is for runaway loops, not exact step counting. |
| `--deterministic` mode silently masks real time/random bugs | Mode is opt-in for `vox run`; doctests are explicit about why they're deterministic. End users won't accidentally turn it on. |
| Idiom fingerprint telemetry is high-volume in long-running scripts | Aggregate locally (in `vox-eval`) to per-construct counts; export aggregated counts, not per-execution events. |
| Per-call sandbox (Task 10) breaks scripts that intentionally touch real fs in CI | Whitelist via `Vox.toml [ci.sandbox.exempt-paths]`; document the safety trade-off. |
| Runtime cap-violation trap (Task 7) fires before Phase 5 effect declarations exist | Until Phase 5 lands, all fns have an empty effect set; the trap is gated behind a `--strict-capabilities` flag (default off) until Phase 5 ships. |
| Panic-trap boundary masks real host bugs from developers | Default verbose mode prints the panic message and host-side backtrace alongside the structured diagnostic; `--quiet` suppresses for production. |

---

## Phase 4 acceptance gate

- [ ] `RuntimeBudget` plumbed through `vox-eval` (Task 1).
- [ ] Fuel, alloc, stack-depth caps each have unit tests + a runaway-trap golden test.
- [ ] Panic-trap boundary in place; `vox run` cannot crash the host on a script panic.
- [ ] Runtime redactor drops `@secret` fields from spans; redaction event emitted.
- [ ] Capability-violation runtime trap is gated behind `--strict-capabilities` until Phase 5.
- [ ] `vox playground --deterministic --seed N` produces reproducible output; doctests use it by default.
- [ ] `vox ci idiom-export` produces a per-construct count JSON consumed by the corpus pipeline.
- [ ] `vox run --sandbox` works; CI sets it on by default.
- [ ] AGENTS.md §"Runtime Safety Nets" landed.
- [ ] `telemetry-trust-ssot.md` documents `vox.runtime.*` and `vox.idiom.*`.
- [ ] `where-things-live.md` updated.
- [ ] Retrospective appended.

---

## Retrospective

_Appended within 5 working days of phase completion._
