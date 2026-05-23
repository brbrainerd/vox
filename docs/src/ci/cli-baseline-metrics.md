---
title: "CLI baseline metrics"
description: "Checklist for local and CI timing (cargo check --timings, vox ci build-timings), vox-cli dependency graph review, and command-surface diffs against fixtures and vox ci command-compliance when changing the CLI surface or registry."
category: "CI & Quality"

schema_type: "TechArticle"
---

# CLI baseline metrics

Use this checklist when changing `vox-cli` command surface, registry, or compile time.

## Before / after a change

1. **Timing (local):** `cargo check -p vox-cli --timings` — open the HTML report; compare wall time to the previous run.
2. **Workspace guard:** `vox ci build-timings` (budgets in `docs/ci/build-timings/budgets.json`).
3. **Dependency graph:** `cargo tree -p vox-cli -e normal,build` — spot unexpected always-on crates after edits.
4. **Command surface:** `cargo run -p vox-cli -- commands --format json --include-nested` — diff against the prior output, or rely on `cargo test -p vox-cli --test command_catalog_paths_baseline` (sorted path fixture under `crates/vox-cli/tests/fixtures/`) plus `vox ci command-compliance` (embed + catalog vs registry).
5. **Build analytics (VoxDB):** query `build_*` projections via MCP (`vox_benchmark_list` with
   `source=build_health|build_regressions|build_warnings|dependency_shape`) and compare with prior
   runs before deciding module refactor vs feature-gate vs crate split.

## Single source of truth

- **Registry:** `contracts/cli/command-registry.yaml` (embedded in `vox-cli` for catalog metadata).
- **Generated table:** `docs/src/reference/cli-command-surface.generated.md` — refresh with `vox ci command-sync --write` after registry edits.
- **Compliance:** `vox ci command-compliance` before merge.
