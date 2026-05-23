---
title: "Crate and build-lane migration map"
description: "Official documentation for Crate and build-lane migration map for the Vox language. Detailed technical reference, architecture guides, an"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Crate and build-lane migration map

Single map for **where code lives**, **which Cargo feature turns it on**, and **naming drift** we are correcting. Pair with [vox-cli-build-feature-inventory](vox-cli-build-feature-inventory.md) and [CLI scope policy](cli-scope-policy.md).

## Nomenclature (canonical)

| Concept | Canonical Rust / docs name | Avoid |
|--------|----------------------------|--------|
| Unified DB facade type | `vox_db::VoxDb` or alias `vox_db::Codex` | Confusing `vox_codex::` in new code (use `vox-codex` crate only for legacy shims) |
| Arca store / schema | `vox_pm`, `CodeStore` | Mixing “Arca” and “Codex” without context |
| Mens corpus + runtime (no STT, no native train) | feature `mens-base` | Assuming Oratio or **`vox-populi`** ML is always on when you enable **`gpu`** |
| Oratio STT CLI | feature `oratio` | Shipping `vox-oratio` in every default `vox-cli` build |
| Native train / QLoRA | feature `gpu` (alias `mens-qlora`) | Expecting CUDA without `mens-candle-cuda` |
| Repo layout / `repository_id` | `vox-repository` | Scattering repo-root logic in CLI ad hoc |

## Build lanes (what CI and `vox ci build-timings` measure)

| Lane id | Command sketch | Purpose |
|---------|----------------|---------|
| `check_vox_cli_default` | `cargo check -p vox-cli` | Default contributor loop (`mens-base`, no Oratio, no **`vox-populi`** / **`gpu`**) |
| `check_vox_cli_no_default_features` | `cargo check -p vox-cli --no-default-features` | Compiler + `vox-db` shell only |
| `check_vox_cli_gpu_stub` | `… --features gpu,mens-qlora,stub-check` | ML + TOESTUB integration |
| `check_vox_cli_gpu_populi_candle_cuda` | `… --features gpu,mens-candle-cuda` | CUDA compile gate (when `nvcc` on `PATH`) |
| `check_vox_db` | `cargo check -p vox-db` | Data-plane baseline |
| `check_vox_oratio` | `cargo check -p vox-oratio` | STT crate isolation |
| `check_vox_mens_train` | `cargo check -p vox-populi --features mens-train` | Native training stack without linking full CLI |
| `check_vox_cli_populi_oratio` | `cargo check -p vox-cli --features oratio` | STT / Oratio stack on top of default `mens-base` |
| `check_vox_mcp` | `cargo check -p vox-mcp` | MCP host binary (orchestrator + publisher + skills + Oratio rerank) |

Run: `vox ci build-timings` and `vox ci build-timings --crates` (`--json` for CI artifacts). Soft budgets: **`docs/ci/build-timings/budgets.json`** only (loaded by the CLI — no second copy in Rust). Env: `VOX_BUILD_TIMINGS_BUDGET_WARN=1` (missing lane keys + over cap), `VOX_BUILD_TIMINGS_BUDGET_FAIL=1` (fail on over cap; warn not required).

## Aggressive per-crate compile pressure (model, not a guarantee)

Rough **cold** `cargo check -p …` on a typical dev machine (order-of-magnitude):

| Crate / lane | Cold check (indicative) | Notes |
|--------------|-------------------------|--------|
| `vox-cli` `--no-default-features` | 2–6 min | Lex/parser/typeck/codegen + `vox-db` |
| `vox-cli` default | 4–10 min | + `vox-corpus`, `vox-runtime` |
| `vox-cli` + `oratio` | +3–8 min delta | + `vox-oratio` / Candle transformers |
| `vox-cli` + `gpu` | +6–18 min delta | + `vox-populi` mens-train + `vox-tensor` |
| `vox-cli` + `mens-candle-cuda` | +10–30 min delta | nvcc / MSVC sensitive |
| `vox-populi` `--features mens-train` | 8–20 min | Burn + Candle + qlora-rs |
| `vox-oratio` | 5–15 min | Whisper / Candle path |
| `vox-db` | 1–4 min | Turso stack |

Use `vox ci build-timings --crates` to replace guesses with wall-clock numbers on **your** runner.

### Measured sample (warm cache, not cold model)

Committed snapshot: `docs/ci/build-timings/latest.jsonl` (regenerate with `SKIP_CUDA_FEATURE_CHECK=1` when CUDA is unavailable). Example row from a **warm** Windows run (2026-03-21): all lanes **within** aggressive cold bands from the table above (same order of magnitude or better because of cache).

| Lane id | Wall-clock ms (sample) |
|---------|------------------------|
| `check_vox_cli_default` | 8845 |
| `check_vox_cli_gpu_stub` | 11376 |
| `check_vox_cli_no_default_features` | 4144 |
| `check_vox_db` | 3892 |
| `check_vox_oratio` | 826 |
| `check_vox_mens_train` | 2444 |
| `check_vox_cli_populi_oratio` | 9448 |

Treat these as **telemetry**, not SLA: refresh `latest.jsonl` after toolchain or dependency upgrades.

### Deviation vs aggressive cold model + soft budgets

Use **`docs/ci/build-timings/snapshot-metadata.json`** with each `latest.jsonl` commit so reviewers know **warm vs cold** methodology.

**Soft budgets** (`docs/ci/build-timings/budgets.json`) are *upper* cold-check guards, not targets. The committed warm sample uses a **tiny fraction** of each budget (example: `check_vox_cli_default` ≈ **1%** of its 600_000 ms cap) — expected when `target/` is warm.

**Vs cold time bands** (minutes, from the table above): a **warm** run that finishes in seconds does **not** contradict the cold model; it confirms incremental caching. **Regression triage:** compare **new cold** or **CI wall-clock** runs to bands, or enable `VOX_BUILD_TIMINGS_BUDGET_WARN=1` on a clean `CARGO_TARGET_DIR`.

## Migration matrix (aggressive reorg)

| Old name / path | New home / policy | Rationale | Compatibility | Deprecation |
|-----------------|-------------------|-----------|---------------|-------------|
| `vox_codex::…` imports in workspace | `vox_db::…` | Single data-plane mental model; `Codex` remains a **type alias** on `VoxDb` | Crate `vox-codex` re-exports `vox_db::*` | Retain facade until release notes removal |
| `vox-codex` crate | Stay as thin shim over `vox-db` | External crates / legacy paths | `pub use vox_db::*` in `crates/vox-codex/src/lib.rs` | Document-only; no date until downstreams audited |
| Oratio in default CLI | Feature `oratio` | Candle/Whisper compile cost | `vox-cli` default = `mens-base` only | Done |
| Native train / QLoRA in default CLI | Feature `gpu` (+ `mens-candle-cuda` for NVIDIA kernels) | Burn/Candle/qlora-rs blast radius | Aliases `mens-qlora` → `gpu` | Done |
| Ad-hoc repo root walks in new code | `vox_repository::…` | Stable `repository_id`, layout, scopes | N/A | Policy in `external-repositories.md` |
| `vox mens` without `mens-base` | Enable `mens-base` (default) or build **`vox-mens`** shim | Command surface gate | **`vox-mens`** binary prepends `mens` | Done |
| Shell timing scripts as SSOT | `vox ci build-timings` | Reproducible lanes in Rust | Scripts remain optional delegates | Done |

## Lateral moves already applied or targeted

| From | To / policy | Why |
|------|-------------|-----|
| `vox-oratio` on default `mens-base` | feature `oratio` | Cuts default `vox-cli` compile cost; STT is opt-in |
| `vox_codex::` in `vox-cli` / `vox-ludus` | `vox_db::` | One data-plane mental model |
| `vox-codex` crate | keep as thin re-export over `vox-db` | External/legacy `vox_codex` path without duplicating logic |
| Dead `vox-ludus` / `vox-codex` deps in `vox-lsp` | removed | Less atomization in tooling crate |

## Deliverables checklist

- [x] `oratio` feature split in `vox-cli`
- [x] `vox ci build-timings --crates`
- [x] This migration map + inventory doc updates
- [ ] Optional: deprecate `vox-codex` crate in a later release after downstreams migrate (breaking policy: allowed)


