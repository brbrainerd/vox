---
title: "Vox Pipeline Program — Sub-Spec 1: Placement Model"
description: "A compiler-enforced placement property (native / shared / gui) for every Vox declaration, inferred from the effect system and overridable with @place, making the perf-vs-GUI boundary a checked contract instead of convention."
category: "architecture"
status: "proposed"
---

# Vox Pipeline Program — Sub-Spec 1: Placement Model

## Program context (1A — decomposition)

This is the first of three sub-specs that together harden the Vox end-to-end pipeline
(`.vox` → TypeScript-via-Vite **and** Rust-via-cargo), each shipped on its own spec → plan → implementation cycle:

| # | Sub-spec | Owns | Depends on |
|---|----------|------|-----------|
| **1** | **Placement model** (this doc) | Which tier — `native` / `shared` / `gui` — each declaration belongs to | — (keystone) |
| 2 | Speed tiers | interp-default dev loop + warm/incremental cargo broker | placement (knows what *must* be native) |
| 3 | Parity oracle + dashboard | behavioral `interp ≡ Rust ≡ TS` oracle, required-fixture gate, Vox Axis panel | placement labels + feature matrix |

Sub-specs 2 and 3 are deliberately **out of scope** here and will be brainstormed separately.

## Problem

Today the perf-heavy/native-vs-GUI boundary is **convention plus CLI flag**, not a language
property:

- `@native` exists as a token but is `Unverified` on every `Target` in
  [`feature_matrix.rs`](../../../crates/vox-compiler/src/feature_matrix.rs) — it carries no semantics.
- The real split is decided by decorator habit (`@server`/`@table` → Rust; `component`/`@reactive` → TS)
  and by `--target server|client`. Nothing stops a GUI component from calling a `db`-touching
  function directly, or a perf function from being expected on the TS arm.
- `feature_matrix.rs` already knows, *per feature*, that JSX is `PARITY_FRONTEND_ONLY` and `Spawn`
  is `PARITY_BACKEND_ONLY`. That knowledge is never lifted to the **per-declaration** level where
  authors actually make placement mistakes.

## Goal

Give every declaration exactly one **placement**, computed by the compiler and enforced at compile
time, so that:

1. The author rarely annotates — placement is **inferred** from the existing effect/capability system.
2. When inference can't decide (a function pulled to two incompatible tiers), the compiler emits a
   **hard error with a fix-it**, not a silent default.
3. Crossing the boundary (`gui` calling `native`) is only legal through the existing RPC decorators
   (`@server`/`@query`/`@mutation`) — the client/server split becomes a **checked contract**.

## The placement lattice

Three placements, ordered by how many tiers can emit the declaration:

| Placement | Emits to | Meaning | Seeded by |
|-----------|----------|---------|-----------|
| `shared` | **native + gui + interp** (everywhere) | Pure, tier-agnostic logic | `@pure`; body calls only `shared`/pure builtins; types & ADTs |
| `native` | Rust (Axum/Tauri) + interp | Perf / server / system | `db`, `fs`, `net`, `spawn`; `@server`/`@query`/`@mutation`/`@table`/`@scheduled`; `actor`; `workflow` |
| `gui` | TypeScript | Browser/UI | `component`; `@reactive`; JSX; DOM/browser-only builtins |

`shared` is the top of the lattice (decision **C**: it emits **everywhere**, interp included — interp
is simply the native tier without AOT). `native` and `gui` are the two incompatible specializations;
a declaration forced toward **both** is the conflict case.

## How placement is computed

A new analysis pass in `vox-compiler`, running **after typeck** (so it can see resolved calls and
effects), in three steps:

1. **Seed.** Each declaration gets an initial placement from its own decorators, effects, and syntax
   (the "Seeded by" column above). Absent any signal → `shared`.
2. **Propagate.** Walk the call graph; a callee constrains its callers. A `shared` function that
   calls a `native`-only builtin is pulled to `native`; one that renders JSX is pulled to `gui`.
   Iterate to a fixed point.
3. **Resolve / conflict.** If a declaration is pulled toward both `native`-only and `gui`-only, it is
   a **placement conflict** — `E-PLACE-CONFLICT`, with a fix-it offering: (a) split the function,
   (b) move the cross-tier call behind an endpoint, or (c) add an explicit `@place`.

### The typed boundary

A direct call from a `gui` declaration into a `native` declaration is rejected
(`E-PLACE-BOUNDARY`) with a fix-it: *"wrap the callee in `@query fn` / `@server fn` and call it over
the client."* Legal crossings go through the existing RPC decorators, which already emit a Rust
handler + a typed TS client — so the boundary mechanism is **reused, not invented**.

### The `@place` override (decision A)

`@place(native | shared | gui)` is a single orthogonal decorator (one new
`DecoratorFeature::Place`), not three bare keywords. It overrides inference, and the compiler then
**verifies the override is satisfiable**:

- `@place(gui)` on a function that touches `db`/`fs`/`net` → `E-PLACE-UNSAT` (GUI tier can't reach
  those; route through an endpoint).
- `@place(shared)` on a function with a tier-specific effect → `E-PLACE-UNSAT`.
- `@place(native)` is always satisfiable (native is the most capable tier).

### Retiring `@native` (decision A, persistent)

The dormant `@native` token is **removed**, not aliased:

- Delete the `DecoratorFeature::Native` arm and its lexer spelling in `feature_matrix.rs`.
- Add `@native` to the **Retired Surfaces** table in [`AGENTS.md`](../../../AGENTS.md) with canonical
  replacement `@place(native)`, and to any docs that mention it (`grep -ri "@native"` across
  `docs/`, `crates/*/src`, `examples/`).
- A `vox-code-audit` detector (`placement/retired-native-decorator`) flags any surviving `@native`
  at **Error** severity so it can never silently return.

## Enforcement severity (decision B — blocking from day one)

No warn-first ratchet. `E-PLACE-CONFLICT`, `E-PLACE-BOUNDARY`, `E-PLACE-UNSAT`, and the retired
`@native` detector are **blocking** the moment they land. Existing golden examples / fixtures that
break under the new rules are treated as **bugs in the fixtures** and fixed in the same body of work:

- A parallel sub-agent sweep runs the placement pass over `examples/golden/**`, every
  `crates/*/tests` Vox fixture, and the canonical ladder, collecting every new diagnostic.
- Each finding is triaged as **true positive** (real placement bug → fix the fixture) or **false
  positive** (inference is wrong → fix the inference rule, add a regression case). The same sweep
  watches for **false negatives** (a known cross-tier call that *should* error but doesn't).
- This triage is the implementation plan's first milestone — the rules don't merge until the corpus
  is green under them.

## Parity / matrix integration (5B forward-link)

- `@place` registers as a feature in `feature_matrix.rs`, so the existing **compile-time-exhaustive**
  `support(Feature × Target)` match forces every target to declare how it treats it — the same gate
  that already makes "add a feature → won't compile until every cell is filled."
- Placement is the per-declaration computation of the per-feature `FRONTEND_ONLY`/`BACKEND_ONLY`
  facts the matrix already holds; the two must agree, checked by a new parity test
  (`placement_matches_feature_matrix`).
- Sub-spec 3 surfaces each declaration's placement in the Vox Axis **Parity** panel (rendered from a
  generated `parity-report.v1.json`, same pattern as the gui-surface-registry reports).

## Components & boundaries

| Unit | Responsibility | Interface |
|------|----------------|-----------|
| `placement::infer` (new module, `vox-compiler`) | Seed → propagate → resolve; returns `PlacementMap` keyed by declaration id | `fn infer(hir: &HirModule, effects: &EffectTable) -> (PlacementMap, Vec<Diagnostic>)` |
| `placement::check` | Boundary + `@place` satisfiability checks over the `PlacementMap` | `fn check(map: &PlacementMap, hir: &HirModule) -> Vec<Diagnostic>` |
| `feature_matrix` (edit) | Drop `Native`; add `Place`; keep exhaustiveness | existing `support()` |
| `vox-code-audit` detector | `placement/retired-native-decorator` (Error) | existing detector ABI |
| Codegen consumers (`codegen_rust`, `codegen-ts`) | Read placement to decide what each backend emits | `PlacementMap` passed into `generate()` |

Each unit is independently testable: inference is a pure function of HIR+effects; checking is a pure
function of the map; the detector is a string/AST scan.

## Testing

- **Inference unit tests:** table-driven — input snippet → expected placement, covering each seed
  rule, call-graph propagation, and the fixed-point.
- **Conflict/boundary tests:** each new diagnostic code has a positive fixture (must fire) and a
  negative fixture (must not fire) — guards false positives and false negatives.
- **Matrix parity test:** `placement_matches_feature_matrix` asserts per-declaration placement is
  consistent with the per-feature matrix verdict.
- **Corpus gate:** the placement pass over the full golden + fixture corpus must be diagnostic-clean
  before merge (decision B).
- TDD per the Test-First Policy: failing test first for every new `pub fn`.

## Out of scope

Speed tiers / cargo broker (sub-spec 2), the behavioral differential oracle and the Vox Axis Parity
panel UI (sub-spec 3), and any change to the RPC/endpoint emission itself (reused as-is).

## Open risks

- **Inference precision.** Over-eager propagation could mislabel genuinely-shared helpers as
  `native`. Mitigation: `@place(shared)` escape hatch + the false-positive triage milestone.
- **Effect-table completeness.** Placement is only as good as the effect/capability data; gaps in
  `@uses(...)` coverage become placement blind spots. The corpus sweep is the detection mechanism.
