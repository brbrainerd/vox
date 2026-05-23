---
title: "RCICD coverage and cost matrix (2026)"
description: "Maps CI workflows and jobs to risk coverage, local equivalents, and optimization notes. Companion to runner-contract and local pre-push docs."
category: "CI & Quality"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Explains where CI spend goes and what belongs in GitHub Actions vs local gates."
schema_type: "TechArticle"
---

# RCICD coverage and cost matrix (2026)

This document implements the RCICD audit plan: what is covered where, what belongs in CI versus `vox ci pre-push`, known gaps, and cost hotspots.

## Principles

- **Merge gate:** [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) on `pull_request` and push to `main`.
- **Shift-left:** Prefer [`vox ci pre-push`](../contributors/local-ci-pre-push.md) (and crate-scoped tests) for deterministic checks developers can run before push; keep environment-heavy work in Actions.
- **Single bundle:** `vox ci ssot-drift` runs `check-docs-ssot`, `check-codex-ssot`, `command-compliance`, SQL/query guards, operations verify, contracts index, docs-reality-audit verify, exec policy, completion audit (verify path), scientia contracts, and data SSOT guards — see `run_ssot_drift` in [`crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs`](../../../crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs).

## Workflow summary

| Workflow | Trigger (summary) | Role | Cost tier |
|----------|-------------------|------|-----------|
| `ci.yml` | PR + push `main` | Build, lint, guards, nextest, llvm-cov, audits, integration lanes | High |
| `docs-quality.yml` | Path-filtered PR/`main` | Doc lint, doctest-md, Starlight build | Medium |
| `docs-deploy.yml` | Path-filtered push `main` | Site deploy | Medium |
| `link_checker.yml` | PR + push `main` | External links | Medium |
| `ssot-drift.yml` | PR + push `main` | Crate version / dashboard SSOT (overlaps theme with `ci.yml`, not identical steps) | Low–medium |
| `mutation-pr.yml` | Path-filtered PR (`vox-compiler`, `vox-codegen`) | `cargo mutants` | High |
| `mutation-nightly.yml` / `bench-nightly.yml` / `qwen35-native-nightly.yml` | Schedule | Nightly quality / perf | High (scheduled) |
| `mobile-e2e-android.yml` | Path-filtered PR/push (`apps/vox-mental-tracker/**`) | Android emulator E2E | High |
| `deploy-hetzner.yml` | Push `main`, `workflow_dispatch` | Coolify deploy + health probes; Gate 1 is minimal ubuntu build only | Low (smoke) + deploy wall time |
| Tag/release workflows | Tags / `release` | Artifacts | Variable |

For runner labels and exceptions, see [runner-contract.md](runner-contract.md) and [github-hosted-exceptions.md](github-hosted-exceptions.md).

## `ci.yml` job → coverage → local parity

| Area | CI location (typical) | Local / test-suite equivalent |
|------|------------------------|-------------------------------|
| Line endings, manifest, fmt, deny | `guards-fast` | `vox ci pre-push` (fast), `vox ci manifest`, `cargo deny` |
| Docs/codex SSOT + registry parity | `guards-fast` → `ssot-drift` | `vox ci ssot-drift` or `vox ci pre-push --complete` |
| Retired symbols | `guards-fast` | `vox ci retired-symbol-check` |
| Data / telemetry SSOT (inside `ssot-drift`) + secrets | `guards-fast` → `ssot-drift` + later secrets steps | `vox ci ssot-drift`; `vox ci secrets-parity` |
| Clippy / rustdoc / drift | `lints` | `cargo clippy`, `vox ci pre-push --complete` |
| Workspace tests + coverage | `tests` | `vox ci pre-push --full`, `cargo llvm-cov nextest` |
| Compiler gates (golden strict-parse, `@test` runner, WebIR) | `compiler-gates` | See crate tests under `vox-compiler`, `vox-integration-tests` |
| Audits (TOESTUB, mens-gate, build-timings, all-features matrix) | `audits`, matrices | Partial local; GPU/time budgets stay CI |

## Coverage gaps addressed in-repo

- **Recursive golden strict-parse:** [`golden_examples_strict_parse`](../../../crates/vox-compiler/tests/golden_examples_strict_parse.rs) now walks `examples/golden/**/*.vox`, matching [`golden_vox_test_runner`](../../../crates/vox-integration-tests/tests/golden_vox_test_runner.rs).

## Ongoing gaps / debt (monitor)

- **Ignored tests:** Inventory in `contracts/reports/test-inventory.v1.json`; governance via `vox ci ignored-test-age`, `test-inventory`. Large ignored counts hide regressions if ignored-only lanes are skipped.
- **Mutation scope:** PR mutation gate is limited to compiler/codegen paths; other crates rely on unit/integration coverage only.
- **GitLab vs GitHub:** `.gitlab-ci.yml` **`vox-ci-guards`** uses the same **`retired-symbol-check`** then **`ssot-drift`** bundle as GitHub **`guards-fast`** (plus GitLab-only extras such as **`data-storage-guard`**). Full parity with the entire **`ci.yml`** matrix is still not guaranteed — track drift when adding GitHub jobs.

## Cost optimizations applied

1. **`guards-fast`:** Removed standalone `check-codex-ssot`, `check-docs-ssot`, and `command-compliance` before `ssot-drift` (they run inside `run_ssot_drift`).
2. **`compiler-gates`:** Removed redundant first `web_ir_lower_emit_test` nextest invocation; the `VOX_WEBIR_VALIDATE=1` ignored-only run executes the full ignored set including the former filtered test.
3. **`mobile-e2e-android.yml`:** Path filters so macOS emulator jobs run only when `apps/vox-mental-tracker/` or the workflow file changes.
4. **`guards-fast`:** Removed standalone **`data-ssot-guards`** after **`ssot-drift`** (already invoked at end of `run_ssot_drift`).
5. **`deploy-hetzner.yml` Gate 1:** Dropped duplicate **`cargo fmt`** / **`cargo clippy`**; kept **`cargo build -p vox-cli --locked`** on **`ubuntu-latest`** only (merge already validated by **`ci.yml`**).
6. **`.gitlab-ci.yml`:** Replaced separate **`check-codex-ssot`**, **`check-docs-ssot`**, **`command-compliance`** with **`retired-symbol-check`** + **`ssot-drift`** to match GitHub bundling.

## Rollback

- Re-add explicit `ci command-compliance` (and codex/docs steps) in `ci.yml` if `ssot-drift` is split or reordered without preserving those calls.
- Re-add standalone **`ci data-ssot-guards`** in **`guards-fast`** only if **`ssot-drift`** stops calling **`run_data_ssot_guards`**.
- Restore **`deploy-hetzner`** fmt/clippy smoke if branch protection no longer requires green **`ci.yml`** before merge.
- Restore global `on: [push, pull_request]` on mobile workflow only if cross-repo coupling requires every PR to exercise Android.

## References

- [Local CI pre-push](../contributors/local-ci-pre-push.md)
- [Runner contract](runner-contract.md)
- [Command compliance](../reference/command-compliance.md)
