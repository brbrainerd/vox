---
title: "Repository cleanup ledger (2026 deep reorg)"
description: "Audit ledger for orphan artifact removal, runtime untracking, surface folder clustering, and docs/superpowers/plans sprawl reduction."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: false
---

# Repository cleanup ledger (2026)

Machine-readable intent: each row is executed as-is after reference updates.

## Orphan artifacts — remove from VCS

| Path | Evidence | Action |
|------|----------|--------|
| `artifacts/toestub-crates-audit.json` | No `rg` references | `git rm` |
| `docs/lychee_report.json` | Superseded by `docs-astro/lychee_report.json` in workflow | `git rm` |
| `marquee_app/build/**` | Built output; no source refs | `git rm` |
| `crates\vox-ml-cli\check_errors.txt` (historical path) | Captured compiler log | `git rm` |
| `mens/data/metadata.json` | Generated corpus sidecar | `git rm` |
| `mens/temp_stabilization/metadata.json` | Orphan experiment output | `git rm` |
| `vox-vscode/tmp/*.md` | Scratch planning | `git rm` |
| `.gemini/antigravity/scratch/test_yaml.vox` | IDE scratch | `git rm` |
| `.vox/artifacts/simulation/summary.md` | Ludus CLI output | `git rm` |

## Runtime / local state — untrack only (paths unchanged)

| Path | Action |
|------|--------|
| `.vox/store.db` | `git rm --cached` |
| `.vox/research-audit-codex.db` | `git rm --cached` |
| `.vox_modules/local_store.db` | `git rm --cached` |
| `.vox/memory/*.md` | `git rm --cached` (runtime daily stubs) |
| `crates/vox-orchestrator/.vox/memory/**` | `git rm --cached` |

Intentional tracked policy under `.vox/` (e.g. `.vox/agents/`, `.vox/bin/`) retained.

## Surface directories — move + update contracts/docs

| Source | Destination |
|--------|-------------|
| `marquee_app/` | `apps/interop/marquee_app/` |
| `tools/visualizer/` | `apps/experimental/visualizer/` |
| `vox-vscode/` | `apps/editor/vox-vscode/` |
| `test_app_bundle/` | `tests/fixtures/frontend/test_app_bundle/` |
| `test_app/` | `examples/sandboxes/test_app/` |

`tools/render-durable-animation/` moved to [`apps/build-tools/render-durable-animation/`](../../../apps/build-tools/render-durable-animation/) (Tier M consolidation; script path in [`scripts/render-durable-animation.vox`](../../../scripts/render-durable-animation.vox)).

## `docs/superpowers/plans` — domain subfolders

| Subfolder | Files moved |
|-----------|-------------|
| `orchestrator/` | reliability block, master plan, phase 1–11 |
| `language/` | language-\*, tracker index, codegen-ts-bugs-blocking-tracker |
| `handoff/` | handoff-\* |
| `ci/` | local-ci pre-push, dashboard design port |
| `data-audit/` | vox-db-\*, audit followup |
| `scientia/` | scientia-phase-\* |
| `telemetry/` | telemetry-phase-\* |
| `tooling/` | vuv roadmap, vox-share, drift-detection, astro migration |
| `mental-tracker/` | mobile-gui-correctness-and-tracker-ship |

## Ignore hardening

- `.vox/artifacts/` simulation summaries (complement existing `*.db` ignores).

## Verification

- `rg` must show zero stale paths for moved roots after sweep.
- Regenerate `docs/agents/doc-inventory.json` via `cargo run -p vox-cli -- ci check-docs-ssot` or documented generator.
