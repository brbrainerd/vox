---
title: "Build-Time Log"
description: "Per-phase build-time measurements for the 2026-05-08 workspace reorg."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Build-Time Log

Per-phase measurements for the workspace reorg. See [build-time-baseline.md](./build-time-baseline.md).

## Phase 0 — Baseline established (2026-05-08)

| Scenario | Time |
|---|---|
| Orchestrator incremental (lib.rs) | 5.59s |
| Orchestrator incremental (mcp_tools/) | 5.06s |
| CLI incremental | 26.76s |
| L0 leaf (vox-orchestrator-types) | 0.36s |

## Phase 1 — L0 type cleanup + plugin-host inversion fix (2026-05-08)

`vox-plugin-types` extracted (manifest + skill_manifest + state-backend trait).
`vox-plugin-host` no longer depends on `vox-db`. Daemon binary gated via
`required-features=mcp-native`.

| Scenario | Time | vs baseline |
|---|---|---|
| Orchestrator (touch lib.rs) | 6.24s | +0.65s (added plugin-types edge) |
| CLI incremental | 7.60s | **−72%** (−19.16s) |

## Phase 2 — workspace-hack leaf exclusion (2026-05-08)

Configured hakari's `[traversal-excludes]` and `[final-excludes]` so L0 leaves
don't pull in workspace-hack.

| Scenario | Time |
|---|---|
| L0 leaf (vox-plugin-types, true leaf) | 0.53s |

## Phase 3 — vox-db split (2026-05-08)

Audited; deferred. Orphan rule forces extension-trait migration for 67 impl
blocks (~50 callers). vox-compiler dep is structural (used for `@table`
parsing). Cost > benefit at current crate size.

## Phase 4 — Extract vox-orchestrator-mcp + vox-orchestrator-d (2026-05-08)

The 88K-LoC vox-orchestrator splits along its biggest internal seam:
- `mcp_tools/*` (33,885 LoC) → new crate `vox-orchestrator-mcp`
- `services/routes/` (axum HTTP routes) → moved with mcp
- `bin/vox_orchestrator_d.rs` → new crate `vox-orchestrator-d`

vox-orchestrator drops mcp-native feature and 14 deps that mcp owns
(schemars, axum, rmcp, tower-http, vox-compiler, vox-grammar-export,
vox-mcp-registry, vox-capability-registry, vox-openai-wire,
vox-project-scaffold, vox-skills, vox-openclaw-runtime, vox-plugin-host).

| Scenario | Time | vs baseline |
|---|---|---|
| Orchestrator (touch lib.rs) | **4.06s** | **−27%** |
| MCP isolated (touch mcp lib.rs) | 5.15s | new |
| CLI incremental | 7.60s | (unchanged from Phase 1) |

## Phase 5 — Extract vox-orchestrator-queue (2026-05-08)

Move locks/, oplog/, affinity.rs, sync_lock.rs (~3K LoC) to a new crate.
Also moved 4 pure-data types (SnapshotId, ChangeId, FileAffinity, AccessKind)
to `vox-orchestrator-types` so the queue crate has only L0 deps.

| Scenario | Time | vs baseline |
|---|---|---|
| Orchestrator (touch lib.rs) | **3.58s** | **−36%** |
| Queue isolated (touch lib.rs) | 6.84s* | new (*first build) |
| CLI incremental | 6.99s | **−74%** (cumulative from Phase 1) |

## Phase 6 — Orchestrator runtime split (deferred → C5 / Tier D) (2026-05-08)

`runtime.rs`, `orch_daemon/`, `dei_shim/` form the orchestrator's CORE — they
reference the `Orchestrator` struct directly along with `events`, `models`,
`services`, `types`. Extracting requires either a trait abstraction over the
Orchestrator type itself (huge — covers ~40 method surfaces) or moving the
Orchestrator struct out (which empties the parent crate). Either approach
exceeds reasonable scope.

The previously-completed Phase 4 + Phase 5 already cut 36K LoC out of the
orchestrator. The remaining core is dominated by the runtime/daemon layer,
which is the part that genuinely benefits from co-location.

> **Superseded (2026-05-15):** The correct extraction wedge is `src/orchestrator/`
> (12,825 LoC, the densest subdir) — not the full runtime split. The Rust coherence
> constraint (all `impl Orchestrator { }` blocks must live in the defining crate)
> requires co-moving the struct. Full analysis and 7-task plan at
> [`2026-05-15-orchestrator-tier-d-plan.md`](2026-05-15-orchestrator-tier-d-plan.md).
> Start when Rule 13 fires (>15% LoC growth since last release tag).

## Phase 7 — vox-cli decoupling (partial) (2026-05-08)

Most orchestrator-using commands in vox-cli (`attention`, `dei`, `safety`,
`visus`, `live`, `mcp_server/*`, `extras/ludus/hud`) are **already** gated
behind features (`dei`, `live`, `mcp-server`, `ludus-hud`). Only `generate`,
`model/*`, `ci/*` are unconditional, and the cumulative win from gating them
is small (~0.5s at most) given Phase 4+5 already trimmed 36K LoC from the
orchestrator transitive compile.

The originally-planned `OrchestratorClient` trait facade (covering ~40
methods) is large refactor work for limited additional payoff and is not
pursued.

| Scenario | Time | vs baseline |
|---|---|---|
| CLI incremental | 6.99s | **−74%** (cumulative — bulk came from Phase 1 + Phase 4) |

## Phase 8 — Plugin family flattening (no action) (2026-05-08)

Audited; structurally clean. vox-cli/vox-orchestrator don't compile-time
depend on any plugin (cdylib delivery). L4 → L3 plugin → vox-db deps are
allowed by the layer model.

## Phase 9 — Strict CI guard + final docs (2026-05-08)

Layer-check flipped from `--warn-only` to strict in CI. Three known
inversions documented in `layers.toml`:
- `vox-cli → vox-orchestrator` (deliberate; runtime/observability surfaces)
- `vox-pm → vox-compiler`, `vox-pm → vox-db` (transitional; future re-tier)

> **Update (2026-05-15):** The `vox-pm` inversions were removed when C2 from the
> followup design split `vox-package` → `vox-package-types` (L1 pure-data leaf)
> + `vox-package` (L3 build/registry). Current known inversions: `vox-cli →
> vox-orchestrator`, `vox-arch-check → vox-compiler` (dev-dep only),
> `vox-ml-cli → vox-cli` (optional mens-dei workflow). See `layers.toml`
> `[[known_inversions]]` for the authoritative list.

## Headline outcome

| Scenario | Baseline | Final | Win |
|---|---|---|---|
| Orchestrator incremental | 5.59s | 3.58s | **−36%** |
| CLI incremental | 26.76s | 6.99s | **−74%** |
| L0 leaf (true leaf) | n/a | 0.53s | new floor |
| MCP isolated | (in 5s orch) | 5.15s | newly parallel |
| Queue isolated | (in 5s orch) | <2s warm | newly parallel |

vox-orchestrator went from 88K LoC to 52K LoC. The 36K-LoC reduction
came from extracting `vox-orchestrator-mcp` (33K) and `vox-orchestrator-queue`
(3K). Editing files in those subsystems no longer triggers a full
orchestrator recompile.
