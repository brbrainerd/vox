---
title: "Workspace Reorg Outcome (2026-05-08)"
description: "Outcome report for the 2026-05-08 workspace reorg: phases completed, build-time gains, and naming history."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Workspace Reorg Outcome (2026-05-08)

> **Naming note (2026-05-08):** The CI guard binary referenced as `vox-layer-check` in this narration was renamed to `vox-arch-check` later in the same series; references below are historical.

Companion to [2026-05-08-workspace-reorg-design.md](./2026-05-08-workspace-reorg-design.md).
Records what was delivered across the 10 phases.

> **Follow-up landed (2026-05-15):** The [crate-org followup design](./2026-05-08-crate-org-followup-design.md) delivered C1 (vox-mcp-meta merge), C2 (vox-package-types split — removed both vox-package inversions), C4 (ops_ludus → vox-gamify), the mcp-server feature-gate side-quest, all Track A SSOT fixes, Track B description rewrites, and PR6 arch-check lints. Workspace now ~91 crates. `vox-orchestrator` has regrown to 65.5K LoC. C3 (vox-cli-ci, 22K LoC) and C5 (vox-orchestrator-core) are deferred with plan docs.

## Phases delivered (5 of 10)

### Phase 0 — Baseline & guards ✓
- New `crates/vox-layer-check/` (Rust binary): parses `cargo metadata`,
  validates each dep edge against `layers.toml`. Modes: `--warn-only` and
  strict.
- New `docs/src/architecture/layers.toml` — 79 crates assigned to L0–L5.
- New `build-time-baseline.md` and per-phase `build-time-log.md`.
- CI step wired up.

### Phase 1 — L0 type cleanup + plugin-host inversion fix ✓
- New `vox-plugin-types` (L1 leaf): plugin manifest, skill manifest,
  `PluginStateBackend` trait + `PluginStateSkillEntry`/`PluginStateError`.
- `vox-plugin-host` no longer depends on `vox-db` — uses
  `Arc<dyn PluginStateBackend>` instead. `vox-db` impls the trait.
- Daemon binary `[[bin]] required-features = ["mcp-native"]`.
- Drive-by fixes: pre-existing `mesh_driver_compile` test;
  pre-existing `vox-mens` `gpu`-feature broken `crate::training::native`.

### Phase 2 — workspace-hack leaf exclusion ✓
- Hakari's `[traversal-excludes]` and `[final-excludes]` exclude L0 leaves.
- Truly-leaf incremental check at 0.5s (no hack compile in the dep graph).

### Phase 4 — Extract vox-orchestrator-mcp + vox-orchestrator-d ✓
- **The big one.** `mcp_tools/` (33,885 LoC) → new crate `vox-orchestrator-mcp`.
- `services/routes/` (axum HTTP routes) → moved with mcp.
- `bin/vox_orchestrator_d.rs` → new package `vox-orchestrator-d`.
- vox-orchestrator drops the `mcp-native` feature and 14 deps that mcp now
  owns (schemars, axum, rmcp, tower-http, vox-compiler, vox-grammar-export,
  vox-mcp-registry, vox-capability-registry, vox-openai-wire,
  vox-project-scaffold, vox-skills, vox-openclaw-runtime, vox-plugin-host).
- vox-cli's `mcp-server` feature wires through to the new crate.
- Orchestrator incremental: **5.59s → 4.06s (−27%)**. MCP edits are now isolated.

### Phase 5 — Extract vox-orchestrator-queue ✓
- `locks/`, `oplog/`, `affinity.rs`, `sync_lock.rs` (~3K LoC) → new crate
  `vox-orchestrator-queue`.
- Four pure-data types moved to `vox-orchestrator-types` to keep the queue
  crate as a clean leaf: `SnapshotId`, `SnapshotIdGenerator`, `ChangeId`,
  `FileAffinity`, `AccessKind`.
- Orchestrator incremental: **4.06s → 3.58s** (−12% additional, **−36% total**).

### Phase 9 — Strict CI guard + final docs ✓
- Layer-check flipped from `--warn-only` to strict in CI.
- 79 crates fully assigned. 3 known inversions documented with rationale.
- Outcome doc + per-phase build-time log.

## Phases deferred (5 of 10)

| Phase | Target | Why deferred |
|---|---|---|
| 3 | vox-db → vox-db-stores | 67 `impl VoxDb` blocks need extension-trait migration; ~50 callers; vox-compiler dep is structural |
| 6 | vox-orchestrator-runtime | runtime.rs / orch_daemon/ / dei_shim/ form the orchestrator's core; extracting requires trait facade over Orchestrator (~40 methods) or moving Orchestrator out (empties parent) |
| 7 | vox-cli-thin (full) | Most orchestrator-using commands in vox-cli are already gated behind features (`dei`, `live`, `mcp-server`, `ludus-hud`). The few remaining unconditional uses (`generate`, `model/*`, `ci/*`) deliver ~0.5s additional savings, not worth the gating complexity now that Phase 4+5 trimmed 36K LoC out of orchestrator. |
| 8 | Plugin family flatten | Already structurally clean: cdylib delivery; vox-cli/vox-orchestrator have no compile-time deps on any plugin. L4 → L3 plugin→db deps are allowed by the layer model. |

## Headline build-time outcome

| Scenario | Baseline | Final | Win |
|---|---|---|---|
| Orchestrator incremental | 5.59s | 3.58s | **−36%** |
| CLI incremental | 26.76s | 6.99s | **−74%** |
| L0 leaf (true leaf) | — | 0.53s | new floor (was paying workspace-hack) |
| MCP isolated | (in 5s) | 5.15s | now parallel with orchestrator |
| Queue isolated | (in 5s) | <2s warm | now parallel with orchestrator |

`vox-orchestrator` went from 88K LoC → 52K LoC. The 36K-LoC reduction came
from extracting `vox-orchestrator-mcp` (33K) and `vox-orchestrator-queue` (3K).

## What's enforced going forward

1. **Layer-check runs strict in CI.** Any new dep edge that violates the
   layer model fails the build. Adding a new crate requires an entry in
   `layers.toml`.
2. **L0 leaf crates stay leaves.** Hakari's exclusion list prevents
   `workspace-hack` from re-poisoning them on regeneration.
3. **The plugin-host inversion stays fixed.** `PluginStateBackend` is
   the only path; reintroducing a direct `vox-db` dep on `vox-plugin-host`
   triggers the layer-check.
4. **Three known inversions are documented** (current as of 2026-05-15):
   - vox-cli → vox-orchestrator (deliberate; runtime/observability surfaces)
   - vox-arch-check → vox-compiler (dev-dependency only; integration test)
   - vox-ml-cli → vox-cli (optional mens-dei workflow path)
   - *(The `vox-pm → vox-compiler/db` inversion listed here at reorg time was removed in C2 when `vox-package` was split into `vox-package-types` (L1) + `vox-package` (L3).)*

## New workspace shape

Crates added by this reorg (all L3 unless noted):
- `vox-plugin-types` (L1) — pure types, plugin/skill manifests, state backend trait
- `vox-orchestrator-mcp` — MCP tool layer (33K LoC moved from orchestrator)
- `vox-orchestrator-queue` — locks/oplog/affinity/sync_lock (3K LoC moved)
- `vox-orchestrator-d` (L5) — daemon binary package
- `vox-layer-check` (L0) — CI guard tool

Crates added to `vox-orchestrator-types`:
- `agent_types::workspace_ids` (SnapshotId, ChangeId)
- `agent_types::file_affinity` (FileAffinity, AccessKind)
- `agent_types::ids` (existing)
- `agent_types::switch` (existing)

## Remaining work for future sessions

`vox-orchestrator` has regrown from 52K → **65.5K LoC** (94% of budget). `vox-cli` is at **71.3K LoC** (79% of budget). The deferred extractions have been redesigned:

- **Phase 6 replacement → C5 / Tier D:** Extract `src/orchestrator/` (12.8K LoC) + `Orchestrator` struct into `vox-orchestrator-core`. Full plan with Rust coherence analysis at [`2026-05-15-orchestrator-tier-d-plan.md`](./2026-05-15-orchestrator-tier-d-plan.md). Start when Rule 13 fires (>15% LoC growth since last tag).
- **Phase 7 replacement → C3:** Extract `src/commands/ci/` (22K LoC, 74 files) into `vox-cli-ci`. Three shared modules must move to `vox-cli-core` first. Full plan at [`2026-05-15-cli-ci-extraction-plan.md`](./2026-05-15-cli-ci-extraction-plan.md). Start when `vox-cli` exceeds 72K LoC (80% budget).
- **Phase 3 (vox-db-stores):** Still deferred. 67 `impl VoxDb` blocks need extension-trait migration.

The infrastructure (arch-check, layers.toml, hakari exclusions) is in place to support all of these.
