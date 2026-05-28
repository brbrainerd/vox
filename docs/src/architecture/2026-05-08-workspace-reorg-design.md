---
title: "Workspace Reorg — Build-Time + Layered Architecture (2026-05-08)"
description: "Design document for the 2026-05-08 workspace reorg: build-time wins, layered architecture, and phase plan."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Workspace Reorg — Build-Time + Layered Architecture (2026-05-08)

> **Phase numbering:** This plan uses the **workspace reorg** phase sequence (Phases 0–9). For the other two sequences, see [phase-numbering-index](phase-numbering-index.md).

## Problem

The workspace has grown to 86+ crates with these structural issues:

- **Build-time floor**: `workspace-hack` unifies 283 deps (tokio[full], reqwest, axum, hyper, wasmtime, abi_stable, …) — every crate, including pure-type leaves, pays the full floor.
- **Monolith crates**: `vox-orchestrator` (88K LoC, 106 deps), `vox-cli` (63K LoC, 156 deps), `vox-db` (32K LoC) — each compiles as one `rustc` invocation. Touching any of them is expensive.
- **Layering inversions**: `vox-plugin-host` (1.5K LoC infra) depends on `vox-db` (32K LoC domain). `vox-cli` always pulls `vox-orchestrator` even for compile-only commands. The daemon binary uses `mcp_tools::*` ungated, breaking slim builds.
- **No architectural enforcement**: layer rules are folklore; nothing prevents a new commit from re-introducing inversions.

## Goals

1. **Build-time wins** measured against a recorded baseline. Targets: ≥40% off clean slim build, ≥70% off incremental rebuild after touching `mcp_tools/`, ≥60% off CLI rebuild, ≥80% off plugin-edit cross-rebuild.
2. **Single source of truth**: each concept owned by exactly one crate; no duplicate definitions.
3. **Separation of concerns**: subsystems can be compiled, tested, and reasoned about in isolation.
4. **Architectural enforceability**: a CI guard fails the build on any layer inversion.

## Target architecture (6-layer model)

| Layer | Purpose | Crates (post-reorg) |
|---|---|---|
| **L0 — Pure types** | Data structs only. No tokio, no DB, no logic. | `vox-orchestrator-types`, `vox-db-types`, `vox-protocol`, `vox-mesh-types`, **`vox-plugin-types`** *(new)*, **`vox-skill-types`** *(new)* |
| **L1 — Primitives & utilities** | OS wrappers, crypto, FS, JSON helpers. | `vox-primitives`, `vox-bounded-fs`, `vox-crypto`, `vox-secrets`, `vox-jsonschema-util`, `vox-checksum-manifest`, `vox-identity` |
| **L2 — Domain libraries** | Pure-data domain logic over L0+L1. | `vox-config`, `vox-package`, `vox-repository`, `vox-search`, `vox-corpus` |
| **L3 — Heavy runtimes (split)** | Big monoliths, decomposed along feature boundaries. | `vox-db-core`, `vox-db-stores` *(new — extracted from `vox-db`)*, `vox-orchestrator-queue` *(new)*, `vox-orchestrator-mcp` *(new)*, `vox-orchestrator-runtime` *(new)*, `vox-orchestrator` *(slim coordinator, ~25K LoC)*, `vox-compiler`, `vox-codegen`, `vox-ml-cli`, `vox-publisher` |
| **L4 — Plugin infrastructure** | Plugins ship as cdylib; loaded at runtime. | `vox-plugin-api` *(slimmed)*, **`vox-plugin-host`** *(no `vox-db` dep — uses `PluginStateBackend` trait)*, plugin crates |
| **L5 — Surfaces** | Binaries and integration. | `vox-actor-runtime`, **`vox-cli-thin`** *(new)*, `vox-cli` *(full)*, `vox-code-audit`, `vox-orchestrator-d` *(new — daemon bin package)* |

### Workspace-hack 5-way decomposition

| Hack crate | Unifies | Consumers |
|---|---|---|
| `vox-hack-core` | `serde`, `serde_json`, `thiserror`, `tracing`, `anyhow` | All crates |
| `vox-hack-async` | `tokio[full]`, `futures-util`, `async-trait`, `parking_lot` | Async crates |
| `vox-hack-net` | `reqwest`, `axum`, `hyper`, `tower`, `tower-http`, `axum-extra` | Network surfaces |
| `vox-hack-codegen` | `syn`, `proc-macro2`, `wasmtime`, `wasmtime-wasi` | Compiler/wasm |
| `vox-hack-ml` | `candle-core`, `candle-nn`, `candle-transformers` | ML plugins |

### Layering inversions to fix

1. `vox-plugin-host` → `vox-db`: replaced with `PluginStateBackend` trait in `vox-plugin-types` (L0); concrete impl injected from L5.
2. `vox-cli` → `vox-orchestrator` (always-on): introduce `OrchestratorClient` trait facade; `vox-cli-thin` uses the trait, `vox-cli` (full) wires the concrete orchestrator.
3. Daemon binary uses ungated `mcp_tools`: relocate the bin into `vox-orchestrator-d` package that depends on `vox-orchestrator-mcp` directly.

## CI architectural guard

A small Rust binary at `tools/layer-check/` parses `cargo metadata` and reads a hand-maintained `docs/architecture/layers.toml`. For each dep edge, it checks the source crate's layer is ≥ the target's layer (within-layer deps allowed). Inversions fail CI.

The guard runs in **warn-only** mode through Phase 8 (so refactor commits don't get blocked) and flips to **error** in Phase 9.

## Phasing

| # | Phase | Sessions | Output |
|---|---|---|---|
| 0 | Baseline & guards | 1 | `build-time-baseline.md`, `layers.toml`, `tools/layer-check/` (warn) |
| 1 | L0 type cleanup + plugin-host inversion | 1–2 | `vox-plugin-types`, `vox-skill-types`, `PluginStateBackend` trait, daemon bin gating |
| 2 | workspace-hack 5-way split | 2 | 5 new hack crates, monolithic hack deleted |
| 3 | vox-db split (parallel-eligible) | 1 | `vox-db-stores` extracted |
| 4 | Orchestrator MCP split | 1–2 | `vox-orchestrator-mcp`, `vox-orchestrator-d` package |
| 5 | Orchestrator queue split | 1–2 | `vox-orchestrator-queue` |
| 6 | Orchestrator runtime split + slim core | 1–2 | `vox-orchestrator-runtime`; orchestrator ~25K LoC |
| 7 | vox-cli decoupling | 2 | `OrchestratorClient` trait, `vox-cli-thin` |
| 8 | Plugin family flattening (parallel-eligible) | 1 | All plugin Cargo.tomls normalized |
| 9 | Harden guards + final docs | 1 | Layer-check flipped to error; old re-export shims removed |

## Acceptance criteria (every phase)

1. `cargo check --workspace --all-features` green.
2. Existing tests pass; no `#[ignore]` smuggled in.
3. Build-time delta measured and recorded in `docs/architecture/build-time-log.md`.
4. No new layer inversions per `tools/layer-check/`.

## Backwards-compat strategy

When a module moves to a new crate, the original module path becomes a `pub use new_crate::*;` shim for at least one phase of grace period. Phase 9 removes shims that have been quiet for ≥1 phase. Every commit is shippable.

## Estimated build-time payoff

| Scenario | Today | After reorg | Win |
|---|---|---|---|
| Clean slim build | full hack floor | `vox-hack-core` only | ~40–50% |
| Edit `mcp_tools/` → check | full 88K orchestrator | `vox-orchestrator-mcp` only | ~70% |
| Edit CLI surface | re-link cli + orchestrator | `vox-cli-thin` only | ~60% |
| Edit one plugin | many plugins rebuild | only that plugin | ~80% |
| CI per-crate matrix | every job pays full hack | per-job hack subset | ~30–40% |

## Risk register

| Risk | Mitigation |
|---|---|
| Phase boundaries leak — a crate ends up needing its old siblings | Re-export shims; can revert a split if it doesn't hold |
| Workspace-hack split breaks incremental builds (the original purpose of hakari) | Keep hakari config; regenerate per-hack-crate; measure incrementally during Phase 2 |
| The 88K orchestrator has hidden circular dep loops we discover mid-split | We already saw this for `messages.rs`/`tasks.rs`; budget for in-place untangling before extraction |
| Test suite needs orchestrator internals across new crate boundaries | Use `pub(crate)` → `pub` widening as needed; add `#[cfg(test)]` re-exports |
| Plan stalls mid-way leaving a half-refactored workspace | Each phase ends green and shippable; partial completion is acceptable |

## Out of scope

- Touching plugin source code (only their Cargo.tomls)
- Migrating off turso, axum, or any external dep
- Vox language changes
- Cross-platform build issues unrelated to dep layout

## Decommissioned items

The following pre-existing followups in the in-session todo list are folded into this plan and removed as separate work items:

- "Deeper vox-db extractions" → Phase 3
- "Switch vox-cli type-only sites to vox-db-types" → Phase 1
- "Feature-gate vox-db itself in vox-cli" → Phase 7
- "vox-orchestrator audit + potential split" → Phases 4–6
