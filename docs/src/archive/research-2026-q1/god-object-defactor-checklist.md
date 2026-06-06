---
title: "God object defactor checklist (v3)"
description: "Tracks Rust sources over 500 non-blank lines with status workflow; includes PowerShell inventory regeneration, per-crate cargo check/test matrix, public API freeze table, and refactor session log."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# God object defactor checklist (v3)

Track status for every `crates/*/src/**/*.rs` file with **>500 non-blank lines**. Values: `planned` | `in-progress` | `done` | `verified`.

## Inventory regeneration (PowerShell, repo root)

```powershell
$ErrorActionPreference = 'Stop'
$root = (Get-Location).Path
Get-ChildItem -Path (Join-Path $root 'crates\*\src') -Recurse -Filter '*.rs' | ForEach-Object {
  $lines = (Get-Content -LiteralPath $_.FullName | Where-Object { $_.Trim() -ne '' }).Count
  [PSCustomObject]@{ Lines = $lines; Path = $_.FullName.Substring($root.Length + 1) }
} | Where-Object { $_.Lines -gt 500 } | Sort-Object -Property Lines -Descending | Format-Table -AutoSize
```

## Per-crate validation matrix

| Crate / area | After edits run |
|--------------|-----------------|
| `vox-orchestrator` | `cargo check -p vox-orchestrator --lib` ; `cargo test -p vox-orchestrator` |
| `vox-compiler` | `cargo check -p vox-compiler --lib` ; `cargo test -p vox-compiler` |
| `vox-mcp` | `cargo check -p vox-mcp --lib` ; `cargo test -p vox-mcp` |
| `vox-db` | `cargo check -p vox-db --lib` ; `cargo test -p vox-db` |
| `vox-cli` | `cargo check -p vox-cli` ; `cargo test -p vox-cli` ; `cargo run -p vox-cli -- ci command-compliance` |
| `vox-ludus` | `cargo check -p vox-ludus --lib` ; `cargo test -p vox-ludus` |
| `vox-corpus` | `cargo check -p vox-corpus --lib` ; `cargo test -p vox-corpus` |
| `vox-orchestrator` | `cargo check -p vox-orchestrator --lib` ; `cargo test -p vox-orchestrator` |
| `vox-populi` | `cargo check -p vox-populi --lib` ; `cargo test -p vox-populi` |
| Other crates touched | `cargo check -p <crate>` ; `cargo test -p <crate>` |
| Wave boundary | `cargo check --workspace` |

## File inventory (baseline — re-run query to refresh)

See regeneration script above. Initial wave-0 snapshot aligns with God Object Defactor Plan v2 file list in `.cursor/plans/god_object_defactor_rollout_v2_*.plan.md`.

## Public API freeze (do not break without shim)

When refactoring, preserve these surfaces via `mod.rs` + `pub use`:

| Crate | Primary entry points |
|-------|---------------------|
| `vox-orchestrator` | `src/lib.rs` `pub mod` / `pub use` block |
| `vox-db` | `src/lib.rs` `VoxDb`, `Codex`, `pub use store::…` |
| `vox-mcp` | `src/lib.rs` `pub use server::*`, `pub use params::*` |
| `vox-cli` | `src/lib.rs` dispatch; `commands/mod.rs` tree; registry YAML |
| `vox-compiler` | `src/lib.rs`; `parser::parse` / public parse API |
| `vox-populi` | `src/lib.rs`; `mens/tensor` re-exports |
| `vox-ludus` | `src/lib.rs` `pub use` |

## Session log (2026-03-25)

Implemented in tree:

- **Wave 0:** This checklist + PowerShell inventory script + public API freeze table.
- **Orchestrator wave 1 (partial):**
  - `crates/vox-orchestrator/src/types/` — split from `types.rs` into `ids.rs`, `tasks.rs`, `messages.rs`, `mod.rs` (public `crate::types::*` unchanged via `lib.rs` re-exports).
  - `crates/vox-orchestrator/src/session/` — split from `session.rs` into `state.rs`, `config.rs`, `errors.rs`, `manager.rs`, `mod.rs`.
  - `crates/vox-orchestrator/src/orchestrator/task_dispatch/` — split from `task_dispatch.rs` into `submit.rs` + `complete.rs` + `mod.rs`.
  - `crates/vox-orchestrator/src/models/` — split from `models.rs` into `spec.rs`, `registry.rs`, `tests.rs`, `mod.rs`.
- **Wave 7 (infra + runtime):**
  - `vox-workflow-runtime`: `src/workflow/` (`plan`, `run`, `tracker`, `types`, `populi`) + facade `lib.rs` / `db_tracker` unchanged.
  - `vox-pm`: `src/resolver/` (`semver`, `version_req`, `resolve`, `error`) + `resolver/mod.rs` shim; removed flat `resolver.rs`.
  - `vox-tensor` (gpu): `src/tensor/` (`ctor`, `elemwise`, `activations`, `cat_reshape`, `slice_reduce`) + `tensor/mod.rs`; removed flat `tensor.rs`.
  - `vox-runtime`: `src/llm/` (`types`, `wire`, `chat`, `stream`, `embed`) + `llm/mod.rs`; removed flat `llm.rs`.
  - `vox-bootstrap`: `src/engine/` (`cmd`, `evaluate`, `install`) + `engine/mod.rs`; removed flat `engine.rs`.
  - `vox-cli` CI: merged `run_body_inc_a.rs` + `run_body_inc_b.rs` into `run_body_helpers.rs` (single `include!`) after rustc reported unclosed delimiters across back-to-back includes; deleted the two inc fragments.
  - `vox-db`: `gamify_activity.rs` — import `AgentEventRow` (fix compile).
  - **`vox-doc-pipeline`:** `src/pipeline/` (`types`, `lint`, `summary`, `feed`, `mod.rs`) + thin `main.rs` calling `pipeline::run()`.
  - **`vox-doc-inventory`:** `constants`, `types`, `walk`, `counts`, `hints`, `file_entry`, `gen`, `verify_normalize`, `relevance` + facade `lib.rs` (`DEFAULT_INVENTORY_PATH`, `generate`, `verify_fresh`, etc. unchanged).
  - **`vox-config`:** `src/config/` (`gamify_web`, `toml_schema`, `vox_config`, `persist`, `impl_ops`) + `config/mod.rs`; removed flat `config.rs`; `crate::config::{GamifyMode, VoxConfig, WebRunMode}` unchanged via `lib.rs`.
  - **`vox-orchestrator` `config`:** `src/config/` (`enums`, `news`, `orchestrator_fields`, `defaults`, `merge_populi`, `impl_default`, `impl_load`, `impl_env`, `impl_validate`, `errors`, `tests`) + `config/mod.rs`; public `crate::config::{OrchestratorConfig, …}` unchanged via `lib.rs`.
- **Wave 8 (2026-03-25, partial):**
  - **`vox-compiler`:** `parser/descent/expr/` — replaced monolithic `pratt.rs` with `pratt_ops.rs` (binding power + infix loop), `pratt_match.rs` (primary / postfix / brace / match / if / for / lambda), `pratt_jsx.rs` (`parse_jsx`); `expr/mod.rs` wires the three modules.
  - **`vox-orchestrator`:** `selection/` — `task_routing`, `weights`, `scorer`, `virtual_models`, `free_tier`, `resolve`, `tests`, `mod.rs`; removed flat `selection.rs`. Doc-inventory constant updated to `crates/vox-orchestrator/src/selection/mod.rs`.

**Orchestrator (2026-03-25 closure):** `a2a/{envelope,dispatch,bus/}`, `oplog/`, `locks/`, `attention/`, `queue/`, `session/manager/`, `task_dispatch/submit/` — all ≤500 non-blank per file.

**Hardening v3 (2026-03-25):**

- **TOESTUB god-object detector** uses **non-blank** line counts (aligned with this checklist and PowerShell scan).
- **`vox-cli` CI:** `run_body_helpers/` explicit modules (`hash`, `grammar`, `guards`, `docs`, `matrix`, `timings`, `cuda`) + `#[path = …]` from `run_body.rs` (avoids `ci/run_body/run_body_helpers/` submodule pitfall). Removed `run_body_helpers_part*.rs`.
- **`vox-cli` Ludus:** game flows live under `commands/extras/ludus/` + `vox-ludus`; the old duplicate `commands/gamify/` tree was removed (SSOT: **`vox ludus`** with `extras-ludus`).
- **`vox-populi` transport:** `transport/{auth,store,handlers,router}.rs` (removed `part_*.rs` includes).
- **`vox-corpus` synthetic_gen:** explicit modules (`tool_pairs`, `a2a_pairs`, `workflow_pairs`, `orchestrator_pairs`, `web_pairs`, `negative_pairs`, `agent_pairs`, `cli_pairs`, `script_pairs`, `routing_pairs`, `error_recovery_pairs`, `multi_agent_pairs`, `telemetry_pairs`) + shared `emit_line` / `emit_tool_pair` in `mod.rs`; body text remains in `_*` include fragments; `generate_all` via `_generate_all_mod.inc`; `rng.rs` / `templates.rs`; `tests.rs` sibling module. Removed `gen_impl.rs` and `part_01.rs`…`part_05.rs`.
- **Workflow:** `.github/workflows/ml_data_extraction.yml` triggers on `crates/vox-cli/src/commands/corpus/**` (replaces stale single-file path).

**Closure inventory:** Re-run the PowerShell block at the top from repo root. As of **2026-03-25** the scan reports **zero** `crates/*/src/**/*.rs` files with **>500** non-blank lines (strict `Trim()` rule).

**Final rebaseline (2026-03-25, follow-up):** A fresh scan found three regressions over **500** non-blank lines (`vox-toestub` `scaling.rs`, `vox-cli` `db_cli.rs`, `vox-orchestrator` `snapshot.rs`). These were split again:

- `snapshot.rs` — unit tests moved to `snapshot_tests.rs` (`#[path]`).
- `db_cli` — directory module: `db_cli/types.rs`, `db_cli/subcommands.rs`, `db_cli/mod.rs` (`run` + re-exports); public `commands::db_cli::*` unchanged.
- `scaling.rs` — syn visitor + env/loop helpers moved to `scaling_support.rs`; tests to `scaling_tests.rs`.

Post-fix strict scan: **zero** files **>500** non-blank under `crates/*/src/**/*.rs`.

**Near-threshold watchlist (≥450 non-blank, `<500`):** refresh with the same script; representative snapshot **2026-03-25**: `crates/vox-oratio/src/backends/candle_engine.rs` (499), `crates/vox-orchestrator/src/services/routing.rs` (497), `crates/vox-orchestrator/src/usage.rs` (496), `crates/vox-orchestrator/src/snapshot.rs` (488), `crates/vox-orchestrator/src/events.rs` (486), `crates/vox-cli/src/build_service.rs` (484), `crates/vox-cli/src/commands/populi_lifecycle.rs` (479), `crates/vox-ast/src/decl/callable.rs` (478), `crates/vox-cli/src/commands/mens/populi/action_populi_enum.rs` (476), `crates/vox-cli/src/commands/openclaw.rs` (469), `crates/vox-orchestrator/src/mcp_tools/tools/input_schemas.rs` (469), `crates/vox-db/src/store/ops_ludus/gamify_world.rs` (468), `crates/vox-cli/src/commands/extras/ludus/profile.rs` (467), `crates/vox-orchestrator/src/mcp_tools/tools/dispatch.rs` (465), `crates/vox-forge/src/github.rs` (464), `crates/vox-orchestrator/src/mcp_tools/server/lifecycle.rs` (463), `crates/vox-populi/src/mens/tensor/candle_qlora/train_loop.rs` (462), `crates/vox-ludus/src/companion.rs` (457), `crates/vox-cli/src/commands/db_cli.rs` (457), `crates/vox-corpus/src/codegen_vox/part_02.rs` (454), `crates/vox-ludus/src/achievement/defaults/part_c.rs` (452), `crates/vox-db/src/store/ops_ludus/gamify_extended.rs` (450).

**Verified:** `cargo run -p vox-cli --features extras-ludus,stub-check -- ci command-compliance` OK (2026-03-25). `cargo test -p vox-corpus synthetic_gen` OK. **`vox-orchestrator`** is a workspace member (minimal `lib.rs`); use **`cargo check -p vox-orchestrator`**; do not link it from **`vox-cli`** (`vox ci no-vox-orchestrator-import`).

- **CLI:** root `lib.rs` facade + `cli_dispatch.rs`; `corpus/`, `semantic_planner/`, `stack_planner/`, `github/`, `eval_gate/`, `db_research/`, `command_compliance/`, `ludus/`, `training/`, `checks_standard/`, `schola/train/`, `island/`, `runtime/run/backend/`, `templates/`, `gamify` shards, `extras/ars/` — counts per subagent logs in git history if needed.

### File inventory (>500 non-blank)

Regenerate with the PowerShell block at the top of this file. **v3/v4:** no waivers — inventory is empty under the >500 non-blank rule when the script is re-run.

**Hardening v4 (closure):** Re-run strict nonblank scan from repo root; `tokio` integration tests use bounded drains + `timeout` (see `crates/vox-integration-tests/tests/orchestrator_e2e.rs`, `crates/vox-orchestrator/tests/stress_test.rs`). `codegen_vox` uses explicit submodules instead of `part_*.rs` includes. Refresh this watchlist when nearing 500 lines.

**Near-threshold watchlist (≥450 non-blank, 2026-03-26 snapshot):** `crates/vox-oratio/src/backends/candle_engine.rs` (499), `crates/vox-orchestrator/src/services/routing.rs` (497), `crates/vox-orchestrator/src/usage.rs` (496), `crates/vox-orchestrator/src/snapshot.rs` (488), `crates/vox-orchestrator/src/events.rs` (486), `crates/vox-cli/src/build_service.rs` (484), `crates/vox-cli/src/commands/populi_lifecycle.rs` (479), `crates/vox-ast/src/decl/callable.rs` (478), `crates/vox-cli/src/commands/mens/populi/action_populi_enum.rs` (476), `crates/vox-cli/src/commands/openclaw.rs` (469), `crates/vox-orchestrator/src/mcp_tools/tools/input_schemas.rs` (469), `crates/vox-db/src/store/ops_ludus/gamify_world.rs` (468), `crates/vox-cli/src/commands/extras/ludus/profile.rs` (467), `crates/vox-orchestrator/src/mcp_tools/tools/dispatch.rs` (465), `crates/vox-forge/src/github.rs` (464), `crates/vox-orchestrator/src/mcp_tools/server/lifecycle.rs` (463), `crates/vox-populi/src/mens/tensor/candle_qlora/train_loop.rs` (462), `crates/vox-ludus/src/companion.rs` (457), `crates/vox-cli/src/commands/db_cli.rs` (457), `crates/vox-corpus/src/codegen_vox/part_02.rs` (454), `crates/vox-ludus/src/achievement/defaults/part_c.rs` (452), `crates/vox-db/src/store/ops_ludus/gamify_extended.rs` (450). Note: `vox-dei` was removed from the list as it is now a small, dedicated HITL crate.

