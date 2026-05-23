---
title: "VoxScript as Universal Glue Code — Implementation Plan 2026"
description: "Phased roadmap for decommissioning legacy shell/PowerShell technical debt and standardizing on native .vox automation."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-17"
training_eligible: false
training_rationale: "Unifies automation surface and reduces environment-specific failure modes."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# VoxScript as Universal Glue Code — Implementation Plan 2026

## Executive Summary

The Vox project is standardizing on native `.vox` scripts for all project automation (CI, training, migrations, benchmarks). This plan formalizes the decommissioning of legacy shell/PowerShell technical debt, ensuring cross-platform reproducibility and agentic reliability.

## Strategic Objectives

1. **Unify Automation Surface**: All non-bootstrap glue code must be written in Vox.
2. **Cross-Platform Determinism**: Eliminate `bash` vs. `pwsh` divergence.
3. **Agentic Safety**: Scripts are type-checked by `vox check` and governed by the capability-permissions model.
4. **MENS Training Data**: Automation scripts serve as high-quality training examples for the Vox language itself.

## Wave 0: Foundation (Completed)

- [x] Implement `vox run` command with script-mode builtins (`fs`, `process`, `env`, `clavis`).
- [x] Establish the "Bootstrap Exception": `scripts/windows/vox-dev.ps1` and `scripts/vox-dev.sh` as thin launchers.
- [x] Implement `vox check` for static script validation.
- [x] Basic `std` namespace for automation primitives.

## Wave 1: Release & Training Gates (100% Complete)

- [x] Migrate `mens-full-pipeline.ps1` → `scripts/mens/full-pipeline.vox`.
- [x] Implement `vox mens watch-telemetry` to replace legacy polling scripts.
- [x] Migrate `scripts/populi/release_training_gate.sh` → `scripts/populi/release-training-gate.vox`. (Done)
- [x] Migrate `scripts/unlock.ps1` → `scripts/quality/unlock-resources.vox`. (Done)

## Wave 2: CI & Quality Gates

- [ ] Implement `vox ci` subcommands as native `.vox` scripts (e.g., `vox ci ssot-drift`, `vox ci policy-smoke`).
- [ ] Standardize `scripts/quality/` for linting and architectural guards.
- [ ] Integrate `vox check` into the pre-commit hook to ensure all scripts remain valid.

## Wave 3: Installer & Bootstrap Hardening

- [ ] Hardening `scripts/install.ps1` and `scripts/install.sh` as the *only* remaining shell scripts.
- [ ] Ensure the installer correctly handles the bypass of MSVC requirements when `VOX_CANDLE_DEVICE=cpu` is set.
- [ ] Implement a `vox setup` command in native code to handle post-install configuration.

## Wave 4: Total Decommissioning

- [ ] Final sweep: Delete all remaining `.ps1`, `.sh`, and `.py` files in `scripts/` (except bootstrap).
- [ ] Update all documentation to use `vox run scripts/foo.vox`.
- [ ] Enforce "Zero Shell" policy in CI (reject PRs with new shell glue).

## Migration Table

| Legacy Script | Status | Native Replacement |
| --- | --- | --- |
| `scripts/mens/full-pipeline.ps1` | Done | `scripts/mens/full-pipeline.vox` |
| `scripts/telemetry_watch.ps1` | Done | `vox mens watch-telemetry` (builtin) |
| `scripts/populi/release_training_gate.sh` | Done | `scripts/populi/release-training-gate.vox` |
| `scripts/unlock.ps1` | Done | `scripts/quality/unlock-resources.vox` |
| `scripts/mens/run_4080_experiment_cycles.ps1` | Done | `scripts/mens/run_4080_cycles.vox` |

## Execution Checklist for Agents

1. **Commit before Execute**: All `.vox` scripts must be committed to VCS before an agent executes them in a project-critical context.
2. **Use `vox check`**: Always validate scripts before running them.
3. **Prefer `run --interp`**: For simple computation/logic scripts without high performance requirements.
4. **Standardize on `std`**: Do not use `shell_exec` (not implemented/blocked); use `std.fs` and `std.process`.

## Verification

- `vox ci script-hygiene` (to be implemented) will check all `.vox` files for syntax and capability compliance.
- All documents in `docs/` must link to the new native paths.


