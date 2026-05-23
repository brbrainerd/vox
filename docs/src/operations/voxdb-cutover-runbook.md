---
title: "VoxDB data cutover and telemetry sidecar runbook"
description: "Operator runbook for legacy schema_version migration via export/import, historical training telemetry file cleanup, aligning telemetry consumers with Populi envelopes, publication/news tables, and rollback guidance."
category: "Operations"
last_updated: "2026-04-12"

schema_type: "TechArticle"
---

# VoxDB data cutover & telemetry sidecar runbook

Operator-facing sequence for converging on **canonical `vox.db`**, telemetry contracts, and retiring reliance on **`vox_training_telemetry.db`**.

## Stage 0 — Preconditions

- Read `docs/src/architecture/voxdb-connect-policy.md` (strict vs degraded vs legacy primary).
- Ensure `vox ci ssot-drift` and `vox ci data-ssot-guards` pass on main.

## Contributors / local tooling — fresh canonical DB (preferred when data is disposable)

If you **do not** need to keep existing Codex rows (for example `stub-check`, repro scripts, or CI-style checks), **do not** rely on an old user-default `vox.db` that may still be on a legacy `schema_version` chain.

**Use a fresh file:** set **`VOX_DB_PATH`** to a scratch path. When that file is missing, the next normal open (`VoxDb::open` / `connect_default` path) creates it and runs **`migrate`** to the current repository baseline — no export/import loop.

- **PowerShell:** `$scratch = Join-Path $env:TEMP "vox-scratch-$(Get-Date -Format yyyyMMddHHmmss).db"; Remove-Item $scratch -ErrorAction SilentlyContinue; $env:VOX_DB_PATH = $scratch` then run your command (repeat with a new name if you want a clean slate).
- **Bash:** `export VOX_DB_PATH="${TMPDIR:-/tmp}/vox-scratch-$$.db"; rm -f "$VOX_DB_PATH"` then run your command.

Unset remote replica env (`VOX_DB_URL` / `VOX_DB_TOKEN` and compatibility aliases) when you intend **local file** mode only.

**Fact check vs code:** [`DbConfig::resolve_canonical`](../../../crates/vox-db/src/config.rs) (used by `VoxDb::connect_default` / Codex default) **never** selects in-memory SQLite when the environment is empty — it falls back to a **concrete path** (`VOX_DB_PATH`, then platform default, then `app.db`). In-memory (`:memory:`) is for explicit test helpers such as [`VoxDb::open_memory`](../../../crates/vox-db/src/store/open.rs), not for “I cleared env vars.”

When you **do** need historical rows, keep using your real path and complete **Stage 1** if you hit `LegacySchemaChain` / `vox_db::legacy_schema`.

## Baseline bumps (repository releases)

When the monolithic Arca baseline advances (new `SCHEMA_FRAGMENTS` slice, new seed DDL, or digest change), three layers must stay aligned:

1. **Rust SSOT:** `pub const BASELINE_VERSION` in [`crates/vox-db/src/schema/manifest.rs`](../../../crates/vox-db/src/schema/manifest.rs) and the ordered fragment list used by `baseline_sql()`.
2. **Contract SSOT:** [`contracts/db/baseline-version-policy.yaml`](../../../contracts/db/baseline-version-policy.yaml) — `repository_baseline_integer` must equal `BASELINE_VERSION`, and `repository_baseline_digest_hex` must equal the Keccak-256 of `vox_db::schema::baseline_sql()` (run `cargo test -p vox-db baseline_digest_manual -- --ignored --nocapture`, then paste the printed `0x…` digest). CI enforces parity via `vox ci check-codex-ssot` (bundled in `vox ci ssot-drift`).
3. **Existing user databases:** On the next normal `VoxDb::connect` / migrate, a file whose `MAX(schema_version)` is **greater than zero and strictly less than** the new baseline is **advanced in place** by applying the idempotent baseline DDL batch (see `migrate` in [`crates/vox-db/src/store/open.rs`](../../../crates/vox-db/src/store/open.rs)). Narrow, version-gated SQL (for example the v51 reliability flatten) runs only when the pre-migrate version is below the gate called out in that module.

**When Stage 1 export/import still applies:** if `MAX(schema_version)` is **not** equal to the current baseline **and** the chain is not a simple “behind baseline” case the migrator can fold (mixed ad-hoc migration rows, unknown fork, or other non-baseline history), normal connect returns `StoreError::LegacySchemaChain` and logs `vox_db::legacy_schema`. Operators must follow **Stage 1** below (`export-legacy` → new file → baseline migrate → `import-legacy`). **`vox codex verify`** prints baseline / digest hints and points here for legacy primaries (see also [VoxDB connect policy](../archive/research-2026-q1/voxdb-connect-policy.md)).

## Stage 1 — Legacy `schema_version` chain (blocking)

**Symptom:** `StoreError::LegacySchemaChain` on normal `VoxDb::connect`.

1. `vox codex export-legacy backup.jsonl` (opens source without baseline migrate).
2. Point `VOX_DB_PATH` at a **new file** or delete the old DB.
3. Run any command that connects normally (e.g. `vox codex verify`) -> apply baseline.
4. `vox codex import-legacy backup.jsonl` (**replace** semantics — tables cleared then loaded).

## Stage 2 — Historical `vox_training_telemetry.db`

**When:** Older releases may have created `vox_training_telemetry.db` beside `vox.db`. Current Mens training uses [`VoxDb::connect_default`](../../../crates/vox-db/src/facade/connect.rs) against the **canonical** file only; a legacy primary returns `LegacySchemaChain` until Stage 1 completes (no automatic sidecar open or reset).

**Cleanup:** After primary migration, training rows live in canonical `vox.db`; delete or archive the sidecar file only after backup if it is no longer needed.

## Stage 3 — Telemetry consumers

- Align JSONL viewers with Populi envelope (`docs/src/reference/telemetry-metric-contract.md`).
- When changing `telemetry_schema`, update `vox mens watch-telemetry` and re-run `vox ci data-ssot-guards`.

## Stage 4 — Publication / news

- `published_news.content_sha3_256` gates syndication per content revision; see `docs/architecture/news_syndication_security.md`.
- **`publication_attempts`** is canonical for attempt history; `news_publish_attempts` is legacy.

## Rollback

- Keep `export-legacy` JSONL artifacts until Stage 1 verification passes on a clone.
- Do not delete primary DB until export verified.

