---
title: "Tier D — Orchestrator core-extraction plan (2026-05-15)"
description: "Assessment and implementation plan for extracting vox-orchestrator-core from vox-orchestrator. Post-A-12 (2026-05-24): 60,681 LoC with 13% headroom. dei_shim/ extracted to vox-dei-shim. C5 (orchestrator/ subdir) is the correct next wedge but Rule 13 has not fired."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-24"
training_eligible: false
---

# Tier D — Orchestrator core-extraction plan (2026-05-15, updated 2026-05-24)

**Companion to:** [`crate-structure-audit-2026-05-15.md`](crate-structure-audit-2026-05-15.md) §4.4  
**Prereq:** Tiers A + B + C landed (all on 2026-05-15). `vox-arch-check` reports clean.

> **2026-05-24 update (A-12 complete):** `dei_shim/` (5,016 LoC) was extracted to the new
> `vox-dei-shim` crate as planned in §4. `vox-orchestrator` is now at **60,681 LoC** — 13%
> headroom. Rule 13 (`>15% LoC growth since v0.5.0 tag`) has **not fired**. C5 remains deferred.

## TL;DR

- `vox-orchestrator` is at **60,681 LoC** vs `max_loc = 70_000` — **13.3% headroom** (~9,319 LoC). *(was 65,560 / 7% before A-12)*
- The audit's "vertical-slice" path (`agentos/` → `vox-orchestrator-policy`) is a dead end: `agentos/` contains only **358 LoC** across 8 stubs.
- The correct wedge is **C5 from the 2026-05-08 followup design**: extract `src/orchestrator/` (12.8K LoC) into a new `vox-orchestrator-core` crate. This is the densest subdir and the primary source of the +13K LoC regrowth.
- C5 requires co-moving the `Orchestrator` struct (Rust coherence: `impl` blocks for a type must live in the defining crate). The minimum co-move set is ~8 sibling modules; total displacement is ~20–25K LoC.
- Estimated post-split sizes: `vox-orchestrator-core` ~35K LoC, `vox-orchestrator` ~27K LoC.
- **Do not start C5 until `vox-orchestrator` Rule 13 fires (>15% LoC growth since last release tag).** The §4 interim (`dei_shim/` extraction) has landed; runway is now 13.3%.

---

## 1. Current state

```
vox-orchestrator  60,681 LoC  (budget: 70,000 — 13.3% headroom)   ← updated 2026-05-24 post A-12
```

Subdir breakdown (descending), original 2026-05-15 baseline:

| Dir | LoC | Notes |
|---|---:|---|
| `orchestrator/` | 12,825 | Task dispatch + Orchestrator inherent impls |
| `dei_shim/` | ~~5,005~~ → **0** | **Extracted to `vox-dei-shim` (A-12, 2026-05-24)** |
| `models/` | 3,448 | Model registry + selection |
| `a2a/` | 3,167 | A2A remote-worker protocol |
| `planning/` | 2,870 | Plan adequacy / continuation |
| `services/` | 2,848 | Request routing |
| `config/` | 2,673 | Env-sourced config |
| `preregistration/` | 1,456 | Agent pre-registration |
| `types/` | 1,426 | Shared internal types |
| `memory/` | 1,193 | Memory manager |
| `attention/` | 1,089 | Attention tracker |
| `session/` | 1,026 | Session lifecycle |
| `orch_daemon/` | 994 | Daemon entry point |
| `budget/` | 983 | Holistic budget manager |
| `queue/` | 837 | (thin, most queue logic is in `vox-orchestrator-queue`) |
| `hopper/` | 638 | Task hopper shim |
| `routing/` | 564 | Routing helpers |
| `legacy/` | 412 | Compatibility shims |
| `agentos/` | **358** | Policy stubs — **not an extraction target** |
| `context/` | 321 | Context store helpers |
| flat files | ~17K | `runtime.rs`, `catalog.rs`, `events.rs`, `usage.rs`, `handoff.rs`, … |

---

## 2. Why the `agentos/` path fails

The 2026-05-15 audit §4.4 listed two paths for Tier D:

> **Path 2 (vertical-slice):** split `agentos/` into `vox-orchestrator-policy`.

At audit-writing time `agentos/` was assumed to hold most of the +13K LoC regrowth. Measurement shows it is **358 lines** (8 stub files: `policy_runtime.rs`, `guardrail_kernel.rs`, `context_budget_manager.rs`, `risk_scoring.rs`, `intent_planner.rs`, `checkpoint_engine.rs`, `replay_fast_forward.rs`, `mutation_classifier.rs`). Creating a crate boundary for 358 LoC adds manifest overhead and compile-unit fragmentation with near-zero LoC benefit.

---

## 3. C5: `orchestrator/` → `vox-orchestrator-core`

### 3.1 What C5 extracts

`src/orchestrator/` contains the core `Orchestrator` inherent impl blocks — task dispatch submission, completion, agent lifecycle, scaling, persistence, VCS ops, campaigns, comms, safety, and the integration tests. At 12,825 LoC it is the single densest subdir and accounts for the bulk of the post-reorg regrowth.

### 3.2 The Rust coherence constraint

**Problem:** All files in `src/orchestrator/` contain `impl crate::orchestrator::Orchestrator { … }` blocks. Rust's coherence rules require that inherent `impl` blocks for a type live in the **same crate** as the type's definition. You cannot add inherent methods on `ExternalCrate::Orchestrator` from a downstream crate.

**Consequence:** Extracting `src/orchestrator/` requires moving the `Orchestrator` struct definition with it, into `vox-orchestrator-core`.

### 3.3 Sibling deps that must co-move

`src/orchestrator/**/*.rs` (excluding tests) imports these sibling modules via `crate::`:

```
crate::affinity      crate::attention    crate::budget
crate::bulletin      crate::catalog      crate::config
crate::context       crate::groups       crate::locks
crate::models        crate::oplog        crate::orchestrator
crate::planning      crate::queue        crate::scope
crate::services      crate::snapshot     crate::socrates
crate::topology      crate::types
```

Not all of these need to co-move — some can be extracted as `vox-orchestrator-core` **dependencies** instead. The break-even question is: does the module have its own Orchestrator `impl` blocks, or is it a pure utility consumed by those blocks?

| Module | LoC | Can stay in `vox-orchestrator` as a dep? | Notes |
|---|---:|---|---|
| `types/` | 1,426 | **No — must move.** Used by the Orchestrator struct fields. | |
| `config/` | 2,673 | **No — must move.** `OrchestratorConfig` is in `Orchestrator::new`. | |
| `budget/` | 983 | **No — must move.** `BudgetManager` is a struct field. | |
| `locks/` | ~400 | **No — must move.** `FileLockManager` is a struct field. | |
| `bulletin/` | ~200 | **No — must move.** `BulletinBoard` is a struct field. | |
| `scope/` | ~150 | **No — must move.** `ScopeGuard` used by task dispatch. | |
| `groups/` | ~180 | **No — must move.** `AffinityGroupRegistry` is a struct field. | |
| `affinity/` | ~250 | **No — must move.** `FileAffinityMap` is a struct field. | |
| `context/` | 321 | **No — must move.** `ContextStore` is a struct field. | |
| `snapshot/` | ~150 | Possibly, if accessed via trait. | |
| `oplog/` | ~300 | Possibly, if accessed via trait. | |
| `attention/` | 1,089 | Possibly, if accessed via a `dyn AttentionSink`. | |
| `planning/` | 2,870 | Likely must move (plan adequacy checks are in completion handlers). | |
| `services/` | 2,848 | Likely must move (routing is called in task dispatch). | |
| `catalog/` | 853 | Likely must move (catalog refresh is an impl block). | |
| `models/` | 3,448 | Likely must move (model selection called in dispatch). | |
| `socrates/` | ~546 | Possibly, via trait injection. | |
| `topology/` | ~200 | Small — probably must move. | |
| `queue/` | 837 | Already thin; most logic in `vox-orchestrator-queue`. | |

**Estimated minimum co-move set:** types + config + budget + locks + bulletin + scope + groups + affinity + context ≈ **~6,600 LoC**. With planning + services + catalog + models (likely must move), the co-move set grows to **~17K LoC**.

### 3.4 Estimated post-split sizes

| Crate | Post-split LoC | Budget |
|---|---:|---:|
| `vox-orchestrator-core` | ~35,000 | suggest `max_loc = 40_000` |
| `vox-orchestrator` | ~32,000 | suggest `max_loc = 35_000` (down from 70K) |

The split roughly halves both crates and leaves `vox-orchestrator` holding: `orch_daemon/`, `dei_shim/`, `a2a/`, `runtime.rs`, `preregistration/`, `session/`, `hopper/`, `routing/`, `legacy/`, and the integration glue.

---

## 4. Interim option: `dei_shim/` extraction

If C5 is too large to scope right now, `dei_shim/` (5,005 LoC) is a viable intermediate:

- It is the research pipeline shim (`vox dei`, MENS pipeline dispatch, `pipeline.rs` at 930 LoC).
- Its main consumer is `orch_daemon/dei_dispatch.rs` (323 LoC).
- Probable dep set: `vox-orchestrator` types, `vox-db`, `vox-ml-cli`, `vox-compiler`.
- If it can accept `Orchestrator` via a thin trait (5–10 methods: task_submit, attention_report, etc.), it becomes extractable without moving the struct.
- Estimated gain: −5K LoC from `vox-orchestrator`, buying ~3 more months of headroom at the observed growth rate.

**Recommended:** do `dei_shim/` extraction **only if** Rule 13 fires before C5 is staffed, as a holding action.

---

## 5. Task breakdown for C5

> **Prerequisite:** Rule 13 must confirm `vox-orchestrator` has grown >15% since last release tag before starting. If the budget is still comfortable, defer.

### D1 — Dep audit (1–2h)

For each module in the co-move candidate list (§3.3), run:

```powershell
grep -rn "crate::<module>" crates/vox-orchestrator/src/orchestrator/ --include="*.rs" | wc -l
```

Classify each as: must-move / trait-injectable / stays-as-dep. Produce a final co-move manifest and target sizes. Update this doc with the findings.

### D2 — Create `vox-orchestrator-core` skeleton (2h)

```
crates/vox-orchestrator-core/
  Cargo.toml  (name = "vox-orchestrator-core", layer = 3, max_loc = 40_000)
  src/
    lib.rs
```

- Add to `Cargo.toml` workspace members (via glob, no manual edit needed).
- Add to `layers.toml` and `where-things-live.md`.
- Add `workspace-hack` dep.
- Add to hakari `traversal-excludes` if it ends up smaller than 2K LoC (it won't — skip).

### D3 — Move co-move modules (1–2d)

For each module in the co-move manifest:

1. `git mv crates/vox-orchestrator/src/<mod> crates/vox-orchestrator-core/src/<mod>`
2. Rewrite `use crate::<mod>` → `use crate::<mod>` (stays `crate::` within `vox-orchestrator-core`).
3. Update `vox-orchestrator/src/lib.rs` to `pub use vox_orchestrator_core::<mod>;` for any type that's in the public API.
4. Add `vox-orchestrator-core = { path = … }` to `vox-orchestrator/Cargo.toml`.

### D4 — Move `orchestrator/` subdir (4–8h)

1. `git mv crates/vox-orchestrator/src/orchestrator crates/vox-orchestrator-core/src/orchestrator`
2. Fix all `crate::` refs within `orchestrator/` (they now point to `vox-orchestrator-core` root — should still resolve after D3 moved the deps).
3. Move the `Orchestrator` struct definition from `vox-orchestrator/src/lib.rs` to `vox-orchestrator-core/src/lib.rs`.
4. Re-export in `vox-orchestrator/src/lib.rs`: `pub use vox_orchestrator_core::Orchestrator;`
5. Fix any remaining compilation errors.

### D5 — Integration glue (2–4h)

- `vox-orchestrator/src/runtime.rs` and `orch_daemon/` create `Orchestrator` instances — they now import from `vox-orchestrator-core`. Verify `Orchestrator::new` is accessible.
- `vox-cli` → `vox-orchestrator` inversion: the known inversion does not change (still `vox-cli` → `vox-orchestrator`). The build-time benefit comes from `vox-orchestrator` now being thinner.

### D6 — Tests (2h)

- Integration tests in `src/orchestrator/tests/` stay with the struct in `vox-orchestrator-core`.
- `cargo test -p vox-orchestrator-core` must pass.
- `cargo test -p vox-orchestrator` must pass.
- `cargo run -p vox-arch-check` must report clean.

### D7 — Cleanup (1h)

- Update `layers.toml`: add `vox-orchestrator-core` entry, lower `vox-orchestrator` `max_loc` from 70K to 35K.
- Update `where-things-live.md`: add `vox-orchestrator-core` row in L3 section.
- Update this plan doc: mark complete, record actual post-split LoC counts.

---

## 6. Decision checklist before starting

- [ ] `vox-arch-check` Rule 13 has fired for `vox-orchestrator` (>15% LoC growth since last tag), OR headroom has dropped below 5% (~3,500 LoC).
- [ ] D1 dep audit is complete and the co-move manifest is finalized.
- [ ] No other active large PR touches `vox-orchestrator/src/` (merge conflicts would be severe).
- [ ] CI is green on `main` before branching.

---

## 7. Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `Orchestrator` struct has hidden dependencies not caught in D1 | Medium | High — compile failure mid-D4 | D1 must use `cargo check` iteration, not grep alone |
| Re-export changes break the `vox-cli` → `vox-orchestrator` inversion | Low | Medium — arch-check flags it | Verify `known_inversions` in `layers.toml` still covers the edge |
| `dei_shim/` has transitive dep on `Orchestrator` impl methods (not just the type) | Medium | Medium — blocks dei_shim interim option | Check with `cargo check -p vox-orchestrator-core` excluding dei_shim |
| Integration tests in `orchestrator/tests/` require types from `vox-orchestrator` (post-split) | Medium | Low — test-only fix | Move to `vox-orchestrator` `tests/` as integration test using both crates |
| Merge conflicts if MENS/mesh work lands while Tier D is in progress | High | High | Coordinate with MENS Mn-T1 timeline; Tier D should not overlap |

---

## 8. Appendix: commands for re-measuring

```powershell
# Total LoC
(Get-ChildItem crates/vox-orchestrator/src -Recurse -Filter *.rs | Get-Content | Measure-Object -Line).Lines

# Per-subdir
Get-ChildItem crates/vox-orchestrator/src -Directory | ForEach-Object {
  $loc = (Get-ChildItem "$($_.FullName)" -Recurse -Filter *.rs | Get-Content | Measure-Object -Line).Lines
  "$loc  $($_.Name)"
} | Sort-Object { [int]($_ -split '\s+')[0] } -Descending

# agentos/ specifically
(Get-ChildItem crates/vox-orchestrator/src/agentos -Recurse -Filter *.rs | Get-Content | Measure-Object -Line).Lines
```
