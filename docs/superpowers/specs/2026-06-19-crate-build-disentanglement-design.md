---
title: "Crate Build-Time & Dependency Disentanglement — Design (2026-06-19)"
description: "Design for a 4-track program that makes the crate build/dependency model native + re-runnable + LLM-searchable, breaks the 20-crate advisory cycle, shrinks blast-radius keystones, and gates regressions — to be implemented by Gemini 3.5 Flash in Antigravity."
category: "Architecture SSOTs"
status: "design"
training_eligible: true
training_rationale: "Single design SSOT for the crate-rearrangement / build-time program; agents need the track decomposition, interfaces, and scope boundaries before planning."
---

# Crate Build-Time & Dependency Disentanglement — Design (2026-06-19)

**Grounding (the output that informs this design):**
[`../../src/architecture/crate-build-dependency-model-2026-06-19.md`](../../src/architecture/crate-build-dependency-model-2026-06-19.md)
+ machine artifact `graphify-out/crate-build-map.json`. Read it first — it has the blast-radius
keystones, the 20-crate advisory cycle, the Q=0.24 community structure, and the validation/gaps.

**Reuse, don't duplicate:** this extends two already-planned efforts — the build-time measurement
program (`vox ci build-bench` + committed baseline) and the plugin-maturity Track A (arch-check Tarjan
cycles R17, closure-size budget R19). Where those define a gate, this design *wires the crate-build
model into it* rather than re-inventing it.

**Execution target:** Gemini 3.5 Flash in Antigravity — plans will carry the Operating Rules block,
pre-flight `rg`, `[PARALLEL-SAFE]`/`[SEQUENTIAL]` tags, atomic green+commit.

## Goal

Turn the one-off offline crate model into a **native, re-runnable, searchable capability**, then use it
to **break the dependency tangle, shrink the build-time keystones, and gate regressions** — measured
against a committed baseline.

## Track 1 — Native crate-map capability (foundation; build first)

**Purpose:** Replace the offline Python/CNM computation with a native, re-runnable command, so every
later track is scored against a live model and future LLM tool calls can search it.

**Design:**
- New `vox graphify crate-map` (in `crates/vox-cli/src/commands/graphify/mod.rs`, or `vox ci crate-map`
  — decide in plan): reads `contracts/ci/crate-graph.v1.json` (deps) + `graphify-out/crate_audit.json`
  (`compile_s`), or regenerates both from `cargo metadata` + cargo-timings.
- Build a graphify-shaped graph: nodes = crates with `compile_s`/`loc`/`layer`/`fan_in`/`blast_s`
  attributes; links = dependency edges.
- **Reuse native Leiden** — `vox_graphify_reader::cluster::cluster_nodes` (already exists) for
  communities. **New:** `blast_s` = transitive-dependent compile-sum (a reverse-BFS over the
  adjacency); add as `vox_graphify_reader::crate_model::blast_radius_seconds(adj, self_s)`.
- Persist to `.vox/cache/graphify/crate-map/graph.json` + manifest via the existing corpus machinery;
  register a `crate-map` corpus in `contracts/retrieval/graphify-corpora.v1.yaml` so
  `vox_graphify_search`/`query` and the GUI corpus panel surface it for free.
- Emit `graphify-out/CRATE_BUILD_MAP.md` (keystones, communities, cycle, inversions) deterministically.

**Success:** `vox graphify crate-map` reproduces the model in this design (blast-radius top-N, Q,
communities) natively; the `crate-map` corpus is queryable via MCP; output is deterministic.

## Track 2 — Disentangle the 20-crate advisory cycle

**Purpose:** Dissolve the dev/build-dep tangle (`vox-actor-runtime … vox-test-harness …
vox-workflow-runtime`) that defeats build parallelism.

**Design:**
- Root-cause the back-edges (pre-flight: `vox ci dep-cycles` + `cargo metadata` dep-kind inspection).
  Hypothesis from the model: `vox-test-harness` is a widely-used dev-dependency that itself depends on
  heavy crates (`vox-db`, `vox-compiler`, …), closing the loop.
- **Invert** `vox-test-harness`: it should depend only on type-only / L0–L1 crates; move heavy
  dependencies behind a trait/interface the harness consumes, or split the harness so test fixtures
  don't pull production heavyweights.
- **Regression gate:** extend `dep_cycles.rs` (currently *inventory-only* for advisory cycles) with a
  `--deny-new` mode that fails CI when a *new* advisory back-edge appears vs a committed allowlist
  (`contracts/ci/dep-backedges.allow.json`). Wire into `vox ci` non-blocking → blocking once the cycle
  is broken. (Coordinate with plugin-maturity Track A R17.)

**Success:** the 20-crate advisory cycle is gone (or reduced to a small committed allowlist); the gate
prevents new back-edges.

## Track 3 — Blast-radius splits (heavy; last, conservatively scoped)

**Purpose:** Shrink the 370s+ blast radii of `vox-db` / `vox-compiler` / `vox-populi`.

**Design (scoped for a fast model — NOT "split a 38k-loc crate" in one task):**
- Each split is a **pure-type / interface extraction to L0–L1**, isolated and measured. Example:
  extend `vox-db-types` (exists) by moving more pure data types out of `vox-db` so dependents that only
  need types stop recompiling on `vox-db` body changes.
- Every extraction task: (1) measure `blast_s` before (Track 1), (2) move a *named, type-only* module,
  (3) update dependents, (4) measure `blast_s` after, (5) assert a non-trivial reduction. One module
  per task; atomic; reversible.
- Fix the 2 layer inversions (`vox-runtime→vox-config`, `vox-arch-check→vox-test-harness`) and bring
  `vox-config`/`vox-secrets`/`vox-db` fan-in under their `layers.toml` budgets.

**Success:** measured blast-radius reduction on at least the top-3 keystones; layer inversions resolved.
**Out of scope:** wholesale re-architecture of `vox-db`/`vox-compiler`; only incremental, measured
extractions.

## Track 4 — Measurement spine + gating

**Purpose:** Make every change scoreable and prevent regressions.

**Design:**
- Refresh the zeroed `build-bench` baseline (`vox ci build-bench --write contracts/ci/build-bench-baseline.v1.json`)
  and the `crate_audit` compile-times.
- Add a `vox ci crate-budget` gate (or extend arch-check) that fails when blast-radius-seconds of a
  keystone, or workspace modularity Q, regresses beyond a committed threshold in `crate-build-map`.
- Commit the baseline so phase deltas are reproducible (the build-time-program measurement spine).

**Success:** a committed baseline; CI reports blast-radius/Q deltas per change.

## Sequencing & parallelism

`T1 (foundation) → then T2 ∥ T4 → then T3`. T1 must land first (the re-runnable model everything is
measured against). T2 and T4 are independent of each other. T3 (heavy refactors) comes last, each
extraction gated by T1's `blast_s` measurement.

## Open decisions (resolve in planning)
- `vox graphify crate-map` vs `vox ci crate-map` placement (graphify owns the corpus/cluster machinery →
  lean graphify, but `crate-graph.v1.json`/build-bench live under `vox ci`). **Recommend:** the model
  build lives in `vox-graphify-reader` (`crate_model.rs`), exposed under `vox graphify crate-map`,
  reading the `vox ci` artifacts.
- Whether the regression gates start blocking or advisory (recommend advisory until T2/T3 land).
