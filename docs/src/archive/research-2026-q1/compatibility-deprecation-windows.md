---
title: "Compatibility and deprecation windows"
description: "Official documentation for Compatibility and deprecation windows for the Vox language. Detailed technical reference, architecture guides,"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Compatibility and deprecation windows

## Environment variables

| Name | Status |
|------|--------|
| `VOX_DB_URL`, `VOX_DB_TOKEN`, `VOX_DB_PATH` | **Canonical** for Codex / Turso configuration. |
| `TURSO_URL`, `TURSO_AUTH_TOKEN` | **Deprecated** aliases; may be accepted where documented (e.g. optional `vox-runtime` `database` feature) for migration only. |

New code must read **`VOX_DB_*`** first. Legacy aliases should log a one-time deprecation warning when feasible.

Full registry (orchestrator, repo root, CI knobs): [Environment variables (SSOT)](../reference/env-vars.md).

## Crates

| Crate | Role |
|-------|------|
| **`vox-db`** | Canonical database facade — prefer for all new code. |
| **`vox-codex`** | Re-export shim — avoid for new code; no sunset date fixed in repo (track in orphan inventory). |

## JSONL legacy import/export

`vox codex export-legacy` / `import-legacy` are **supported** migration tools for greenfield baselines. Retention of JSONL formats is tied to importer modules in `vox_db::codex_legacy`, not to indefinite SQL migration chains.

## Process

1. Document deprecation in [changelog.md](../reference/changelog.md) when behavior changes.
2. Keep [codex-legacy-migration.md](codex-legacy-migration.md) aligned with shipped CLI subcommands.


