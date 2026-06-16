---
title: Affected-Crate Selective CI
description: PR-time CI builds only the crates a change affects; the merge-queue gate and nightly run the full workspace for soundness.
category: "CI & Quality"
---

# Affected-Crate Selective CI

> **Status (2026-06-16):** PR-lane scoping wired in `.github/workflows/ci.yml`; `merge_group`
> runs the full workspace. Tooling: `vox-cli-ci` (`affected`, `affected_cmd`); graph SSOT:
> `contracts/ci/crate-graph.v1.json`. Reusable consumer: `.github/workflows/compute-affected.yml`.

## The rule

- **`pull_request`:** build/test/clippy only changed crates + reverse-dep closure. Fast feedback.
- **`merge_group`:** full `--workspace`. Authoritative soundness gate (doctests, llvm-cov, slow partition, rustdoc).

## Intentional gaps (PR-time)

- PR-time affected runs do **not** run doctests, llvm-cov coverage gates, the slow `#[ignore]` partition, or workspace rustdoc. Those run only when `setup.outputs.full == true` (`merge_group` or sentinel paths).
- A doctest-only regression is invisible until the merge gate — accepted, because it cannot land on `main` without passing `merge_group`.
- Adding or removing a crate requires regenerating `crate-graph.v1.json` in the same PR; `vox ci ssot-drift` calls `affected-crates --check` and fails with a regenerate hint when the graph drifts.

## Sentinels (force full)

`Cargo.toml` (root), `Cargo.lock`, `.cargo/config.toml`, `rust-toolchain.toml`, `contracts/ci/crate-graph.v1.json`, `crates/workspace-hack/**`, `.config/hakari.toml`.

Non-graph `contracts/**` edits (anything under `contracts/` except `crate-graph.v1.json`) also force `full=true` because SSOT surfaces can affect the whole workspace.

## The graph

`contracts/ci/crate-graph.v1.json` — regenerate with `vox ci affected-crates --regen --out contracts/ci/crate-graph.v1.json`,
verify with `vox ci affected-crates --check`. Drift is enforced inside `vox ci ssot-drift` (blocking).

## Setup job outputs

| Output | Meaning |
|--------|---------|
| `full` | `true` on `merge_group`, sentinel paths, or fail-closed upgrade |
| `affected_crates` | space-separated reverse-dep closure (on merge_group, from `HEAD~1` for shadow) |
| `affected_p_args` | `-p crate1 -p crate2` for cargo scoping |
| `affects_compiler` | compiler-gates job (`vox-compiler` / `vox-codegen` / `vox-integration-tests` closure, golden-only PRs, or `full`) |
| `affects_golden` | `examples/golden/**` changed |
| `affects_contracts` | `contracts/**` changed |
| `affects_scripts` | `scripts/**` or `examples/mesh-compose.yml` changed |
| `affects_gui` | `crates/vox-gui/**` or `apps/editor/vox-vscode/**` |
| `affects_web` | web/vite/visualizer integration paths |
| `affects_plugins` | `crates/vox-plugin-*` |
| `rust_changed` / `docs_changed` | path-filter flags from the setup `filter` step |

The **setup → Log selective CI plan** step writes the same fields to the GitHub **job step summary**.

## Fail-closed

If `rust_changed=true` but `git diff` is empty or produces an empty affected set, setup upgrades to `full=true` rather than skipping Rust gates.

| Condition | Annotation | Job effect |
|-----------|------------|------------|
| `rust_changed` + empty diff | `::error::rust_changed=true but git diff produced no changed files` | `full=true` |
| `rust_changed` + empty affected | `::warning::rust_changed with empty affected set — upgrading to full=true` | `full=true` |
| Graph drift | `::error::crate-graph drift: …` | `ssot-drift` fails |

## Observability (grep these in CI logs)

| Title prefix | Severity | Meaning |
|--------------|----------|---------|
| `affected-ci shadow-miss::<crate>` | warning | Failure outside PR affected set |
| `affected-ci shadow::` | warning | Shadow skipped (empty affected / no junit) |
| `affected-ci shadow::junit has failures but affected set is empty` | warning | merge_group setup gap |

Shadow exits `1` on miss; CI shadow step uses `continue-on-error: true` until F1 (≥3 clean merge batches).

**Agents:** after CI edits run `cargo test -p vox-cli-ci shadow_junit` and `cargo test -p vox-cli --test ci_workflow_contract selective_ci`.

## Future work

- **F1:** Make shadow blocking after calibration.
- **F2:** `ci.yml` calls `compute-affected.yml` (dedupe setup bash).
- **F3:** `vox ci path-gate-plan` from `check-targets.v1.yaml`.
- **F4:** Complete `pr_scope` on all CI checks.
- **F5:** Update `rcicd-coverage-cost-matrix-2026.md`.

## Local parity

- `vox ci affected-crates --changed <file>` — same closure as CI.
- `vox ci pre-push --full --since origin/main` — mirrors PR scope (note: pre-push may use a broader fallback for docs/scripts locally).
- `vox ci pre-push --full --with-coverage --include-slow` — mirrors merge queue.
