---
title: "How to use the canonical VoxDB / Codex store"
description: "Single source of truth for user-global vox.db, project-local store, and training telemetry fallback."
category: "How-To Guides"
last_updated: "2026-03-27"
training_eligible: false

schema_type: "HowTo"
---

# Canonical VoxDB / Codex store

## What is canonical?

**Authoritative relational data** (Codex, publication, research, default training telemetry) lives in the **user-global** database resolved by:

- [`DbConfig::resolve_canonical`](../../../crates/vox-db/src/config.rs) (same as `resolve_standalone`), then
- [`VoxDb::connect`](../../../crates/vox-db/src/facade/connect.rs).

Typical local path: `<VOX_DATA_DIR or platform default>/vox/vox.db` via [`default_db_path`](../../../crates/vox-config/src/paths.rs). Override with `VOX_DB_PATH` or use `VOX_DB_URL` + `VOX_DB_TOKEN` for remote Turso.

## What is not canonical?

| Location | Role |
|----------|------|
| **`.vox/store.db`** (repo) | Optional project cache: snippets, share, LSP — [`open_project_db`](../../../crates/vox-db/src/project_store.rs). Do not treat as cross-repo SSOT. |
| **`vox_training_telemetry.db`** | **Temporary** fallback when `vox.db` is still on a legacy `schema_version` chain. See [Training telemetry sidecar](#training-telemetry-sidecar). |

## migrating off a legacy chain

If `vox codex verify` or normal `connect` reports a non-baseline schema:

1. `vox codex export-legacy backup.jsonl`
2. Point `VOX_DB_PATH` at a **new** file (or delete the old file after backup).
3. `vox codex verify` (applies current baseline).
4. `vox codex import-legacy backup.jsonl`

Details: [codex-legacy-migration](../archive/research-2026-q1/codex-legacy-migration.md).

## Historical `vox_training_telemetry.db`

Mens training uses [`VoxDb::connect_default`](../../../crates/vox-db/src/facade/connect.rs) on the **canonical** store. If `vox.db` is still on a legacy `schema_version` chain, connect fails with `LegacySchemaChain` until you complete export / fresh baseline / import (see [codex-legacy-migration](../archive/research-2026-q1/codex-legacy-migration.md)). A leftover `vox_training_telemetry.db` from older releases can be archived after primary cutover.

## Deprecation stance

- **Canonical:** one maintained `BASELINE_VERSION` in [`manifest.rs`](../../../crates/vox-db/src/schema/manifest.rs).
- **Legacy:** multi-version `schema_version` chains — export/import only, not incremental SQL bridges.

## Related

- [Codex / Arca compatibility boundaries](../archive/research-2026-q1/codex-arca-compatibility-boundaries.md)
- [Forward migration charter](../archive/research-2026-q1/forward-migration-charter.md)
- [Mens training](../reference/mens-training.md)

