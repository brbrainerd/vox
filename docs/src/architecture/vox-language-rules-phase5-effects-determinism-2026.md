---
title: "Vox Language Rules — Phase 5: Effect System & Workflow Determinism (2026-05-09)"
description: "Multi-quarter step-by-step plan to land the @uses(net | fs | time | random | secret) effect system on every public fn, prove @pure transitively, forbid non-deterministic builtins inside workflow bodies (ADR-019 promised this), enforce @uses(fs(read:'./data/**')) glob declarations against literal paths, and lock the closed bare-keyword table from Phase 1. Each effect ships as warning over two minor versions, then escalates to error. Effect-row foundations land in Task 1; per-effect rules follow as one task each so partial completion is still a usable improvement."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 5 child plan. Largest single language win in the series. Sized for multi-quarter execution; child plan per-effect. Symmetric error/fix pairs (vox/effect/missing-X-decl ↔ vox/effect/unjustified-X-decl) deliberately designed so LLM training picks up the inverse rule at the same time."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-compiler/src/typeck/effects/: new module for effect inference and checking"
  - "vox-actor-runtime/src/builtins/manifest.rs: every builtin gains effect-set metadata"
  - "vox-capability-registry: consumed by effect checker, not advisory anymore"
  - "vox-bounded-fs: glob declarations in source (@uses(fs(read:'./data/**'))) checked at compile time against literal call sites"
  - "ADR-019 (workflow determinism): cashes the check it promised"
---

# Phase 5 — Effect System & Workflow Determinism

> **Parent plan:** [`vox-language-rules-and-enforcement-plan-2026.md`](vox-language-rules-and-enforcement-plan-2026.md)
> **Depends on:** Phases 1, 2, 3, and the precursor warning rule from Phase 3 Task 7 (`vox/effect/missing-net-decl`).
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to break each post-Task-1 effect into its own child plan; superpowers:executing-plans to execute each.

**Goal:** Land the language's effect system. Public functions declare `@uses(<effect-set>)`, the compiler proves the closure (every transitive callee is in the closure), `@pure` is provable rather than asserted, workflow bodies forbid non-deterministic builtins, `@uses(fs(read: "./data/**"))` glob declarations are checked at compile time against literal paths, and the closed bare-keyword table from Phase 1 Task 12 is structurally enforced. Lock the language's strongest "wrong programs unrepresentable" surface.

**Architecture:** A new `crates/vox-compiler/src/typeck/effects/` module owns:

1. **Effect set definition.** A closed enum `Effect = Net | Fs(FsCap) | Time | Random | Secret | Auth(Role)`. Builtin manifest (Phase 1 Task 3) gains an `effects: &'static [Effect]` field per entry.
2. **Effect inference.** Bottom-up over the call graph. Pure leaves (no impure builtin calls, no effect-having callees) infer to empty set. Functions calling impure builtins or impure callees inherit the union.
3. **Effect checking.** A function declared `@uses(S)` must have its inferred set `⊆ S`. A function declared `@pure` must have inferred set `= ∅`. Mismatch is a typed error.
4. **Workflow determinism rule.** A function decorated `workflow` may not transitively call any builtin in `{Time, Random, Net}`. Those must be hoisted into `activity`-decorated callees.
5. **`@uses(fs(read: "<glob>"))` checking.** Glob declaration in source becomes a constraint; literal-path call sites are matched against the glob set; non-literal paths are rejected (with an escape hatch: `@uses(fs(read: "*"))`).
6. **Closed keyword table runtime check.** Any code path that registers a new keyword at runtime (plugins, dyn loading) checks against the closed table from Phase 1 Task 12; mismatch traps.

This is sized for multi-quarter execution. Each effect ships independently as `warning` for two minor versions, then escalates to `error`. Partial completion is a usable improvement.

**Out of scope for Phase 5:**
- Higher-rank effect polymorphism (ML/Koka-style row variables) — not needed; effect sets are concrete.
- `@no_panic` reachability proof (research-tier; descoped earlier).
- Full taint type system for `@secret` — Phase 6+ research; this phase ships the *capability* aspect (`@uses(secret)`) without taint propagation.
- Effect-system rules for plugin-loaded code that bypasses the static checker (Phase 4 Task 7's runtime trap covers).

---

## Verification setup

- `cargo test -p vox-compiler --lib typeck::effects::` — per-rule unit tests.
- `cargo test -p vox-compiler --test effect_inference_golden` — input-`.vox`/expected-inferred-effect-set snapshots across many small cases.
- `cargo test -p vox-compiler --test workflow_determinism_golden` — workflow-violating examples.
- `cargo run -p vox-cli -- check examples/golden/` — should pass after burn-down PRs (Phase 3 Task 7 already started this).
- `cargo run -p vox-cli -- check --explain vox/effect/missing-net-decl` — must succeed; same for every paired ID.

---

## Task 1: Effect-set foundations + builtin manifest annotations

**Files:**
- Create: `crates/vox-compiler/src/typeck/effects/mod.rs`
- Create: `crates/vox-compiler/src/typeck/effects/inference.rs`
- Create: `crates/vox-compiler/src/typeck/effects/check.rs`
- Modify: `crates/vox-actor-runtime/src/builtins/manifest.rs` (Phase 1 Task 3 schema) — populate `effects:` field for every builtin
- Modify: `crates/vox-actor-runtime/src/builtins/builtin_registry.rs` — gain effect annotations as a new column in the registry table
- Re-run: `cargo run -p xtask -- gen-builtins` to propagate annotations to all generated outputs

**Why first:** Every later task depends on the inference/check infrastructure. This task lands the closed `Effect` enum, the inference walker (returns inferred set per fn), and the check rule (errors on declared/inferred mismatch). At land time the *check* is gated to `Effect::Net` only — Tasks 2–6 add other effects one at a time.

**Effect classification of existing builtins** (curated in this task; ~150 builtins to annotate):

- `populi.*` — `Net`, `Time` (latency observation)
- `fs.read*`, `fs.write*` — `Fs(...)`
- `time.now`, `time.sleep` — `Time`
- `random.*` — `Random`
- `vox_secrets.resolve` — `Secret`, `Net` (some sources are remote)
- `std.http.*` — `Net`
- `std.print`, `std.println` — empty (modulo a `Stdout` sub-effect we can defer)

**Verify:** `cargo test -p vox-compiler --lib typeck::effects::inference::leaf_purity` — a fn calling no impure builtins infers `∅`.

---

## Task 2: `vox/effect/missing-net-decl` escalation + `vox/effect/unjustified-net-decl` (the symmetric pair)

**Files:**
- Modify: `crates/vox-compiler/src/typeck/effects/check.rs` — wire `Net`-effect checks
- Modify: `crates/vox-code-audit/src/diagnostics/catalog.rs` — escalate `vox/effect/missing-net-decl` (Phase 3 Task 7) from `note` to `warning`; add `vox/effect/unjustified-net-decl` (warning at land)
- Create: `docs/src/reference/diagnostics/effect-unjustified-net-decl.md`
- Create: `examples/golden/anti/effect-unjustified-net-decl.vox`

**Symmetric pair design:**
- `vox/effect/missing-net-decl` — fn transitively calls a `Net`-effect builtin but lacks `@uses(net)`. Suggested fix: insert `@uses(net)`.
- `vox/effect/unjustified-net-decl` — fn declares `@uses(net)` but transitively makes no `Net`-effect call. Suggested fix: remove the decorator.

**Why symmetric:** The audit's [A.5] rule is one direction; the inverse is just as important for keeping `@uses(...)` declarations honest. LLM training on this corpus picks up the inverse rule "for free." This is the LLM-target principle "symmetric error/fix pairs in diagnostic IDs" applied.

**Severity ramp for both:**
- Land both as `warning`.
- After one minor version of corpus burn-down, `missing` escalates to `error` and `unjustified` stays as `warning`.
- After one further minor, `unjustified` escalates to `error`.

**Verify:** Symmetric golden tests for both directions; corpus burn-down PR fixes existing `missing` violations before escalation.

---

## Task 3: Workflow determinism hard rule

**Diagnostic:** `vox/workflow/non-deterministic-builtin` (warning at land; error after one minor — high priority because ADR-019 promised this).

**Files:**
- Create: `crates/vox-compiler/src/typeck/rules/workflow_determinism.rs`
- Modify: `crates/vox-compiler/src/typeck/effects/check.rs` — wire workflow-context check
- Create: `docs/src/reference/diagnostics/workflow-non-deterministic-builtin.md`
- Create: `examples/golden/anti/workflow-non-deterministic-builtin.vox`

**Rule:** Inside a `workflow`-decorated body, no transitive call to a builtin in `{Time, Random, Net}` is permitted. The non-deterministic operation must be hoisted into an `activity`-decorated callee.

**Suggested fix:** Cannot autofix the design. Diagnostic emits a worked example showing the hoist:

```vox
// vox:skip — workflow keyword is reserved (ADR-028); documents future syntax
// BAD: time.now() inside workflow body
workflow fn process_order(id: OrderId) -> Result[Receipt, Err] {
    let started_at = time.now()        // <-- vox/workflow/non-deterministic-builtin
    // ...
}

// GOOD: hoisted into activity
@activity
fn record_started_at() -> Instant { time.now() }

workflow fn process_order(id: OrderId) -> Result[Receipt, Err] {
    let started_at = record_started_at()  // activity is journalled; replay-deterministic
    // ...
}
```

**Why high priority:** ADR-019's replay-determinism contract depends on this. Today it's advisory; this task makes it law.

**Verify:** Synthetic workflow with `time.now()` fires; after hoist, passes.

---

## Task 4: `@uses(fs(read: "<glob>"))` source-side declarations

**Diagnostic:** `vox/effect/fs-path-not-in-glob` (warning at land; error after one minor).

**Files:**
- Modify: `crates/vox-compiler/src/typeck/effects/inference.rs` — `Fs` effect carries `FsCap { read: Vec<Glob>, write: Vec<Glob> }`
- Modify: `crates/vox-compiler/src/lower/decorators/uses.rs` — parse `@uses(fs(read: "./data/**", write: "./out/**"))`
- Modify: `crates/vox-bounded-fs/src/lib.rs` — runtime checker becomes the *fallback* for non-literal paths; literal paths checked at compile time
- Create: `docs/src/reference/diagnostics/effect-fs-path-not-in-glob.md`

**Rule:** A fn declared `@uses(fs(read: "./data/**"))` may only call `fs.read` with literal-or-derived paths matching the glob. Non-literal paths (e.g., from a runtime parameter) require either:
- An explicit escape: `@uses(fs(read: "*"))` — wide open, requires reviewer sign-off via the suppression mechanism, OR
- A constructor-prove pattern: the path is built from validated components.

**Why a Vox-only win [A.60]:** Rust can have a bounded-fs *crate*; only Vox can attach the policy *to the function* so the compiler proves which functions can violate it.

**Suggested fix:** None for missing decl (need human design). For literal path that doesn't match the declared glob, suggest widening the glob or moving the file.

**Verify:** Synthetic fn `@uses(fs(read: "./data/**"))` calling `fs.read("./other/x.txt")` fires; calling `fs.read("./data/users.json")` passes.

---

## Task 5: `@pure` proven transitively

**Diagnostic:** `vox/effect/pure-violated` (warning at land; error after one minor).

**Files:**
- Modify: `crates/vox-compiler/src/typeck/effects/check.rs` — `@pure` is the same rule as `@uses()` (empty set), implemented as a checked predicate over the inferred set

**Rule:** `@pure fn f()` requires inferred-effect-set `= ∅`. Any transitive call to an effecting builtin or effecting callee fires.

**Why:** Today `@pure` is a tag with no enforcement. After this task, the tag *means* something.

**Suggested fix:** Remove `@pure` (likely the right choice if the fn legitimately needs the effect), or hoist the impure call out.

**Verify:** Synthetic `@pure fn f() { populi.complete(...) }` fires; `@pure fn f() { 2 + 2 }` passes.

---

## Task 6: Per-effect rules for `Time`, `Random`, `Secret`

**Files:** One detector module per effect, mirroring Task 2's symmetric-pair pattern.
- `vox/effect/missing-time-decl` ↔ `vox/effect/unjustified-time-decl`
- `vox/effect/missing-random-decl` ↔ `vox/effect/unjustified-random-decl`
- `vox/effect/missing-secret-decl` ↔ `vox/effect/unjustified-secret-decl`

**Sequencing:** One effect at a time, in the order above. Each lands as warning, has a corpus burn-down, escalates to error. Effort: ~2 weeks per effect, sequenced (not parallel — each consumes the previous one's lessons).

**Why this order:**
1. `Time` first — smallest fan-out (few fns currently call `time.*`).
2. `Random` next — even smaller fan-out.
3. `Secret` last — largest fan-out (many fns call `vox_secrets.resolve`); benefits from lessons learned in (1) and (2).

**Verify:** Per-effect, per-direction symmetric golden tests.

---

## Task 7: Closed keyword table runtime enforcement

**Files:**
- Modify: `crates/vox-compiler/src/lexer/keywords.rs` — `KEYWORDS` is already const after Phase 1 Task 12; this task adds the runtime check that any plugin attempting to register a new keyword via dynamic loading hits a typed trap.
- Modify: `crates/vox-plugin-host/src/loader.rs` — runtime keyword-registration calls check against `KEYWORDS`
- Create: diagnostic `vox/runtime/closed-keyword-table-violation`

**Why:** Phase 1 Task 12 closes the table at compile time for in-tree code. This task closes it at runtime for dynamically-loaded plugins.

**Verify:** Synthetic plugin attempting to add a keyword → host traps with the diagnostic.

---

## Task 8: ADR landing + AGENTS.md backlinks + where-things-live update

**Files:**
- Create: `docs/src/adr/0NN-effect-system.md` — the design ADR for `@uses(...)`
- Create: `docs/src/adr/0NN-workflow-determinism-enforcement.md` — refs ADR-019, lands the structural check
- Modify: `AGENTS.md` — add §"Effect System (Required)" with the rules + backlinks
- Modify: `docs/src/architecture/where-things-live.md` — add row for `crates/vox-compiler/src/typeck/effects/`

**Verify:** ADRs render via `vox-doc-pipeline --check`; AGENTS.md update has backlinks to this phase plan and to the new ADRs.

---

## Risks specific to this phase

| Risk | Mitigation |
|---|---|
| Effect inference is too permissive (allows impure calls in `@pure`) | Snapshot tests cover edge cases (recursion, mutual recursion, dyn dispatch — n/a since Vox lacks dyn dispatch today, which is *why* this phase is cheap *now*). |
| Effect inference is too restrictive (rejects legitimate code via false-positive transitive inheritance) | Each effect ships warning for two minor versions; collect FP reports via `vox check --report-false-positive`; iterate. The two-minor warning window is non-negotiable. |
| Corpus burn-down is harder than expected (many fns need `@uses(...)`) | Codemod `vox migrate add-effect-decls` walks call graphs and inserts the smallest sufficient `@uses(...)` per fn. Conservative: never widens; if it can't prove a tight set, it asks for human input. |
| `@uses(fs(...))` glob enforcement too strict for legitimate dynamic-path code | The `@uses(fs(read: "*"))` escape is honest about the wide capability; reviewer sign-off via suppression mechanism. Document the trade-off prominently. |
| Workflow determinism rule (Task 3) breaks existing workflows | Workflows in `examples/golden/` are audited *before* the rule lands; the corpus burn-down PR is a Task 3 prerequisite, not a follow-on. |
| Plugin authors don't know about closed keyword table runtime check (Task 7) | Document in `docs/src/contributors/plugin-authors.md`; the runtime trap message points to the doc. |
| Per-effect rule sequencing (Task 6) gets de-prioritized after `Net` lands | Each effect's child plan is its own TASK with its own retrospective; stalled work is visible in the top-level plan's status table. |

---

## Phase 5 acceptance gate

A phase this large doesn't have a single completion gate. Each effect (Net, Time, Random, Secret, Fs glob) has its own gate:

- [ ] **Net effect (Task 1+2):** Symmetric pair shipped; `missing` is `error`; `unjustified` is `warning`.
- [ ] **Workflow determinism (Task 3):** `vox/workflow/non-deterministic-builtin` is `error`; ADR-019's check is now structurally enforced.
- [ ] **Fs glob (Task 4):** `@uses(fs(read: "<glob>"))` checked at compile time; `vox-bounded-fs` is fallback for non-literal paths only.
- [ ] **`@pure` proven (Task 5):** `vox/effect/pure-violated` is `error`; `@pure` is provable, not asserted.
- [ ] **Time/Random/Secret effects (Task 6):** All three have shipped symmetric pairs; all three `missing` are `error`.
- [ ] **Closed keyword table runtime check (Task 7):** Plugin loader rejects new keyword registration.
- [ ] **ADRs landed (Task 8):** Effect-system ADR + workflow-determinism ADR.
- [ ] **AGENTS.md §"Effect System (Required)" landed.**
- [ ] **`where-things-live.md` updated.**
- [ ] **Per-effect retrospectives appended at each gate.**

Phase 5 closes when all six gates above are met. Realistic horizon: 6–9 months from Phase 1 land, depending on Net-effect burn-down velocity.

---

## Retrospective (per gate)

_Appended within 5 working days of each gate's completion. Capture: actual vs estimated effort per effect, scope changes, what surprised the team, whether the symmetric-pair design (Task 2) helped LLM corpus learning measurably._
