---
title: "Crate Build-Time × Dependency Model (2026-06-19)"
description: "Measured + semantic model of the cargo workspace: blast-radius build-cost, the 20-crate advisory dependency tangle, modularity Q=0.24 communities, and layer inversions — the target list and measurement spine for a crate-rearrangement / build-time / disentanglement plan."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Agents planning crate splits or build-time work need the blast-radius keystones, the dependency tangle, and the community structure in one searchable place."
---

# Crate Build-Time × Dependency Model (2026-06-19)

Joins **measured** build data (`graphify-out/crate_audit.json`, compile self-times) with the
**committed dependency graph** (`contracts/ci/crate-graph.v1.json`) and a **semantic** community
layer, to model how the workspace is arranged into crates and where to invest to cut build time and
disentangle dependencies. Machine artifact: `graphify-out/crate-build-map.json` (generated build output; not committed).
(graphify-shaped nodes/links with `compile_s`, `loc`, `layer`, `fan_in`, `blast_s`, `community`).

**Freshness:** dependency graph 2026-06-18; `compile_s` 2026-06-15 (directional, re-measure for exact);
cycle inventory from `vox ci dep-cycles` 2026-06-19. 113 crates, total self-compile **531s**.

## The key metric: blast-radius-seconds

`blast_s(X)` = `compile_s(X)` + Σ `compile_s` over all crates that transitively depend on X — the
wall-seconds of downstream rebuild forced by touching X. This, not raw self-time, is the build-time
lever.

| Crate | self_s | trans. dependents | blast_s | loc | layer | Reading |
|---|---|---|---|---|---|---|
| `workspace-hack` | 0.5 | 94 | **492** | 2 | 0 | Hakari hub; touching it rebuilds ~everything. Discipline its churn. |
| `vox-secrets` | 3.5 | 61 | 406 | 11309 | 1 | Foundation; huge fan-out of rebuilds. |
| `vox-config` | 2.5 | 60 | 402 | 5193 | 2 | Same; 40 direct dependents (over budget 20). |
| `vox-db` | 21.2 | 46 | **370** | 38211 | 3 | #1 real-code keystone: high self **and** blast **and** size. |
| `vox-compiler` | 14.5 | 46 | 364 | 48063 | 3 | Top split target (over LoC budget). |
| `vox-populi` | 17.1 | 46 | 366 | 21680 | 3 | Keystone. |
| `vox-orchestrator-mcp` | 63.6 | 5 | 128 | 40744 | 3 | Slowest self-compile but few dependents → incremental/internal-parallelism target, not a blast target. |
| `vox-cli` | 53.7 | 3 | 58 | 90279 | 5 | Same; leaf surface. |

Foundation L0/L1 crates (`vox-mesh-types`, `vox-scaling-policy`, `vox-bounded-fs`, `vox-crypto`,
`vox-http-client`) all carry ~400s blast despite tiny self-times — their **stability** matters most.

**Disentangle candidates** (blast × loc): `vox-compiler`, `vox-db`, `vox-orchestrator`, `vox-populi`,
`vox-codegen`.

## The dependency tangle (the disentanglement target)

`vox ci dep-cycles` (authoritative):
- **0 hard link-time cycles** (cargo enforces these — good).
- **1 advisory dev/build-dep back-edge cycle of 20 crates:** `vox-actor-runtime → vox-codegen →
  vox-codegen-ts → vox-compiler → vox-corpus → vox-db → vox-gamify → vox-openclaw-runtime →
  vox-orchestrator-queue → vox-package → vox-populi → vox-publisher → vox-scientia → vox-search →
  vox-skills → vox-sql → vox-tauri-codegen → vox-tensor → vox-test-harness → vox-workflow-runtime`.

Cargo-legal but terrible hygiene — a one-third-of-the-workspace tangle that defeats build parallelism
and clean mental models. It is anchored by **`vox-test-harness`** (a dev-dependency used widely that
itself depends on heavy crates, closing the loop). Inverting `vox-test-harness`'s heavy deps is the
highest-leverage cut.

## Semantic structure: modularity Q = 0.24 (weak)

CNM-greedy community detection yields **modularity Q = 0.240** — well below the Q ≥ 0.4 "well-separated"
threshold, quantitatively confirming weak crate boundaries (pervasive cross-cutting deps). Seven
communities emerge (full membership in `crate-build-map.json`):

| Community | n | self_s | Theme |
|---|---|---|---|
| C0 | 29 | 77 | plugin / container / runtime (wasm, host, sdk, package) |
| C1 | 27 | 128 | compiler / codegen / language (ast, compiler, codegen, sql, tensor, lsp) + config/db/secrets |
| C2 | 25 | 114 | orchestrator / mesh / populi (orchestrator*, populi*, mesh*, corpus, crypto) |
| C3 | 16 | 95 | CLI / audit / tooling (cli, arch-check, audit, git, graphify-reader, registries) |
| C4 | 14 | 113 | research / scientia / gui / llm-egress (scientia, search, research, gui, orchestrator-mcp) |
| — | 2 | — | loners: `vox-tauri-stt`, `vox-wire-format-validator` |

These are candidate *natural* boundaries; the low Q means the current crate split does not yet realize
them cleanly.

## Layer inversions

`vox-runtime(L1) → vox-config(L2)` (real upward dependency) and `vox-arch-check(L0) →
vox-test-harness(L3)` (dev-dep). Both violate the layered model in `layers.toml`.

## Validation findings (end-to-end through our tooling)

- **Native graphify works live:** `vox graphify status` reports a `repo-code-graph` corpus built
  2026-06-19 (570,691 nodes / 460,868 edges) with `manifest_git_sha` populated and `git_drift`
  correctly detected — Plan A + freshness model confirmed in production, not just tests.
- **GAP — no crate-level clustering:** native graphify clusters 570k *symbols*, but there is **no path
  to run Leiden on the 113-crate adjacency**. The semantic layer here is computed offline (CNM); making
  it native (`vox graphify crate-map` feeding the crate adjacency to `cluster::cluster_nodes`) is a plan
  deliverable.
- **GAP — stale toolchain check:** the prebuilt `vox.exe` predates the Plan B/C/D subcommands
  (`index`/`refresh`/`gc`); `vox ci` refuses stale-binary guards. End-to-end testing the new CLI needs
  a fresh `cargo build -p vox-cli`.

## Implications for the plan

1. **Targets:** split `vox-db` / `vox-compiler` / `vox-populi` to shrink the 370s+ blast radii; extract
   pure-type sub-crates to L0 to shrink dependents' compile closures.
2. **Disentangle:** break the 20-crate advisory cycle by inverting `vox-test-harness` (and the other
   dev/build back-edges) so the tangle dissolves.
3. **Layering:** fix the two inversions; bring `vox-config`/`vox-secrets`/`vox-db` fan-in under budget.
4. **Make the model native + re-runnable:** `vox graphify crate-map` (crate adjacency → Leiden
   communities + blast_s), persisted to a corpus + surfaced via MCP for future tool calls; gate
   regressions on blast-radius-seconds and modularity Q via `vox ci`.
5. **Measurement spine:** refresh `build-bench` (baseline is zeroed) and `crate_audit` so every split is
   scored against a committed baseline delta.

## blast_s semantics & keystone selection (added 2026-06-19)

`blast_s(c) = compile_s(c) + Σ compile_s(d)` over all transitive dependents `d` of `c`
(reverse-BFS over the dep graph). It answers: "if `c` changes, how many compile-seconds of
downstream rebuild does that trigger?"

**Known limitation — churn-blindness.** `blast_s` weights by *fan-out × compile time*, not by
*how often a crate actually changes*. Stable pure-type leaf crates therefore rank high:
`vox-mesh-types` (419s) and `vox-crypto` (410s) outrank `vox-db`/`vox-compiler`/`vox-populi`
(349s each) despite changing far less. The three heavyweights are identical (349s) because they
share the same transitive-dependent closure — a cluster signature of the dependency tangle.

**Consequence for gating.** `contracts/ci/crate-budget.v1.json` gates only the heavy,
frequently-changed L3 crates (`vox-db`, `vox-compiler`, `vox-populi`, `workspace-hack`) — NOT
high-blast_s leaf type crates, which would produce false pressure to split stable code.

**SSOT + parity.** `contracts/ci/crate-build-map.v1.json` is the committed gate input
(`compile_s` from the audit + derived `dependents`/`blast_s`/`fan_in`). `vox ci crate-build-map-parity`
recomputes the derived fields from `crate-graph.v1.json` + embedded `compile_s` and fails on drift.
Refresh `compile_s` periodically via the runbook in the measurement-spine plan.

**Follow-on (out of scope):** a churn-weighted `blast_s_weighted = blast_s × commits_90d` mined
from `git log` would rank by real rebuild cost. Tracked, not yet built.
