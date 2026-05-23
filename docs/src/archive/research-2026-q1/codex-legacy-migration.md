---
title: "Codex legacy migration"
description: "Official documentation for Codex legacy migration for the Vox language. Detailed technical reference, architecture guides, and implementa"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Codex legacy migration

Greenfield **Codex** releases do not rely on an unbounded chain of old SQL migrations as the primary story. Instead:

1. **Baseline schema** — Arca applies one manifest-defined DDL snapshot on Turso; `schema_version` holds the single maintained **`BASELINE_VERSION`** (see `crates/vox-db/src/schema/manifest.rs`). Any `MAX(schema_version)` not equal to that baseline is treated as non-baseline / legacy for normal opens. Legacy multi-row chains require export → fresh DB → import.
2. **Importers** — Rust modules read legacy exports or attached old DBs and write normalized rows into the new baseline.

## API surface (crate)

- `vox_db::codex_legacy` in `crates/vox-db/src/codex_legacy.rs` — `verify_legacy_store`, `LegacyImportSource`, JSONL export/import helpers.

## Shipped CLI (minimal `vox` binary)

- `vox codex verify` — connection + `schema_version` + manifest-derived reactivity tables + legacy-chain flag
- `vox codex export-legacy` — dump portable JSONL artifact (`LEGACY_EXPORT_TABLES` — full baseline user tables except `schema_version`)
- `vox codex import-legacy` — full snapshot restore: **DELETE** all `LEGACY_EXPORT_TABLES` on the target, then **INSERT** rows from JSONL (fresh baseline DB only; not a merge)
- `vox codex cutover` — **local** legacy file → timestamped `codex-cutover-*.jsonl` + `.sidecar.json`, new `--target-db`, import, verify

See [cli.md](../reference/cli.md).

## Training telemetry SQLite sidecar (not JSONL cutover)

When the **canonical** `vox.db` is still on a legacy chain, [`VoxDb::connect_default`](../../../crates/vox-db/src/facade/connect.rs) returns **`LegacySchemaChain`** until you export, re-init on baseline, and import. Mens training does not open a separate telemetry file automatically. After you migrate the main DB, all training rows use the canonical file.

Operator guide: [how-to-voxdb-canonical-store](../how-to/how-to-voxdb-canonical-store.md).

## Import sources

| Source | Notes |
|--------|--------|
| Turso file / remote `CodeStore` | Full relational + CAS |
| Orchestrator `memory/` files | `vox codex import-orchestrator-memory --dir … --agent-id …` |
| Skill bundles | `vox codex import-skill-bundle --file …` (JSON descriptor) |

See [Codex vNext schema](codex-vnext-schema.md) and [ADR 004](../adr/004-codex-arca-turso-ssot.md).


