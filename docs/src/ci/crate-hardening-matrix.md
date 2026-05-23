---
title: "Crate hardening matrix (rolling)"
description: "Official documentation for Crate hardening matrix (rolling) for the Vox language. Detailed technical reference, architecture guides, and "
category: "CI & Quality"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# Crate hardening matrix (rolling)

Minimal **four-check** row per critical crate: compile, unit tests, lint (when enabled in CI), and doc/SSOT touchpoint. Expand rows as ownership grows; this is not an exhaustive 140-task matrix.

| Crate | `cargo check -p …` | `cargo test -p …` | Clippy / policy | SSOT / notes |
|-------|--------------------|--------------------|-----------------|--------------|
| `vox-db` | default + `local` where CI uses DB | `--lib` (+ `local`) | workspace `-D warnings` when run | [Codex boundaries](../archive/research-2026-q1/codex-arca-compatibility-boundaries.md), ADR 004 |
| `vox-package` | default | unit + `schema::migration_chain_tests` + `schema::manifest::tests` | same | Arca **manifest** (`SCHEMA_FRAGMENTS` → baseline V1); `execute_batch` only |
| `vox-codex` | default | via `vox-db` / consumers | same | Facade over `vox_db` — SQL lives in `vox-package` |
| `vox-codex-api` | default | manual / dashboard smoke | same | `/health`, `/ready` (baseline V1 + required tables + digest), `/api/search/status`; Codex SSE + Oratio |
| `vox-actor-runtime` | `database` feature if touching db | targeted | same | Optional `crate::db` behind feature |
| `vox-tensor` | `--features gpu` when touching Burn stack | `--lib` + `vox_nn::` subset under `gpu` | same | [vox_nn.rs](../../../crates/vox-tensor/src/lib.rs); legacy `nn.rs` removed |
| `vox-compiler` | default | `--test golden_examples_strict_parse` (with `VOX_EXAMPLES_STRICT_PARSE=1`) + unit / parity tests | same | Parser/HIR/typeck/codegen monolith; golden examples under `examples/golden/` |
| `vox-integration-tests` | N/A (integration) | full crate; env tests serialized | same | `venv_detection` mutex for `VIRTUAL_ENV` |
| `vox-cli` | default + `--bins` (`vox` + `vox-compilerd` + `vox-mens` shim when `mens-base`) + `--features gpu` for Mens train/merge tests + `script-execution` / `execution-api` when touching serve | targeted (`--lib` / `merge_` Mens tests incl. `merge_qlora_cli_roundtrip_lm_head_subset`, needs `--features gpu`) | `clippy -p vox-cli --features execution-api -- -D warnings` for HTTP path | [ref-cli.md](../reference/cli.md), [vox-cli build feature inventory](../archive/research-2026-q1/vox-cli-build-feature-inventory.md) |
| `vox-populi` | `cargo check -p vox-populi --features mens-train` (pulls `candle-qlora` + `qlora-rs`) | `execution_planner`; `hf_keymap`; `training_text`; `preflight_strict_rejects_missing_o_proj`; `burn_full_graph_smoke`; `merge_v2` (see CI + [acceptance runbook](../archive/research-2026-q1/mens-finetune-acceptance-runbook.md)) | workspace clippy when touched | [mens-training.md](../reference/mens-training.md), [mens-lora-ownership.md](../reference/mens-lora-ownership.md), ADR 006/007 |
| `vox-orchestrator-mcp` | default | targeted (`dispatch` / `input_schemas` ↔ **`vox ci operations-verify`**) | same | MCP tool host; catalog SSOT in **`contracts/operations/catalog.v1.yaml`** |

**Runner labels** for CI: see [runner contract](runner-contract.md).

**Rust pattern modernization (rolling):** [Wave 0 baseline](rust-modernization-baseline.md) (lint manifest + pilot file list; aligns with `.cursor/plans/rust-pattern-modernization-master_*.plan.md`).

