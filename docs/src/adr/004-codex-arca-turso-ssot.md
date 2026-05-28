---
title: "ADR 004: Codex over Arca over Turso"
description: "Official documentation for ADR 004: Codex over Arca over Turso for the Vox language. Detailed technical reference, architecture guides, a"
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 004: Codex over Arca over Turso

> [!NOTE]
> Historical note: the `TURSO_*` env var names in this ADR are superseded by `VOX_DB_URL` / `VOX_DB_TOKEN`. ADR text is preserved for context.

## Status

Accepted — greenfield release baseline.

## Context

Vox persisted data through `vox-db` (`VoxDb` / **Codex**), with related crates (`vox-pm`, etc.) and scattered env names (`VOX_DB_*`, legacy `TURSO_*`). Documentation referred to **Arca**, **Codex**, and **VoxDb** interchangeably. The public product name for the database layer must be **Codex** (not “codecs” or other typos). **Schema DDL and store operations** live in **`crates/vox-db`** (`schema/` domains + `store/ops_*.rs`); the only supported SQL engine is **Turso** / libSQL.

## Decision

1. **Codex** — The public, application-facing data API. In Rust, `vox_db::Codex` is a type alias for `VoxDb`; new docs and APIs should say **Codex**.
2. **Arca** — Internal name for schema fragments, baseline migration, CAS tables, and SQL operations **owned by `vox-db`** (`schema/manifest.rs`, `store/`). No second physical store.
3. **Turso** — Sole database engine. No parallel PostgreSQL/SQLite product paths for the same data plane.
4. **Greenfield baseline** — Fresh releases use a forward migration chain from the current schema version; legacy shape is preserved via explicit **importers**, not an unbounded pile of historical migrations in docs.
5. **Convex-like behavior** — Implemented as Codex capabilities (change log, subscriptions, invalidation, SSE/WebSocket), not a second database.
6. **Secrets** — `VOX_DB_TOKEN` (and auth material) are **environment-only**; never committed in TOML. `VOX_DB_URL` may appear in config for convenience; token must not.

## Consequences

- **Repository tenancy** — MCP and orchestration shard filesystem paths; coordination tables use `repository_id` where applicable (e.g. `a2a_messages`). The `agent_events` table does not currently include `repository_id` on the baseline DDL. Session rows carry tenant context in `agent_sessions.task_snapshot` JSON when MCP sets `SessionConfig::repository_id` in `vox-orchestrator`.

> Nomenclature note (2026-05-08): `vox-pm` was renamed to `vox-package`; references in this ADR are historical.
- `VoxDb` remains the stable Rust identifier for ABI/compatibility; prefer **Codex** in user-facing text and new modules.
- Compatibility aliases for legacy Turso-prefixed env names map to the same remote resolution as `VOX_DB_URL` / `VOX_DB_TOKEN` in `vox_db::DbConfig::resolve_standalone` (after canonical env, before older Turso spellings listed in [env-vars SSOT](../reference/env-vars.md)).
- Older first-party Turso env spellings are **deprecated**; they remain last-resort shims in `resolve_standalone` alongside the `VOX_TURSO_*` compatibility tier (ordering in env-vars SSOT).
- Direct `turso::` usage outside `vox-db` (and documented exceptions) is discouraged; domain code should call **`VoxDb` / `Codex` APIs** (`store/ops_*.rs`). See [direct Turso allowlist](../archive/research-2026-q1/codex-turso-allowlist.md) for the current enforcement story.

## References

- [Environment variables (SSOT)](../reference/env-vars.md) — canonical `VOX_DB_*` / Turso alias precedence
- [Codex / Arca compatibility boundaries](../archive/research-2026-q1/codex-arca-compatibility-boundaries.md) — API, env, and migration contract
- [Codex vNext schema domains](../archive/research-2026-q1/codex-vnext-schema.md)
- [Codex BaaS scaffolding](../archive/research-2026-q1/codex-baas.md)
- [Orphan surface inventory](../archive/research-2026-q1/orphan-surface-inventory.md)
- Crate: `crates/vox-db`, `crates/vox-pm`

> **Nomenclature note (2026-05-08):** `vox-pm` was renamed to `vox-package`; references in this ADR are historical.

## Status update — 2026-05

### Sanctioned satellites (libSQL files outside `vox.db`)

The "Turso-only" rule does not mean "single DB file." It means "every relational
store uses libSQL/Turso, with the schema either in `vox-db`'s `SCHEMA_FRAGMENTS`
manifest or in an explicitly-listed sanctioned satellite."

Current sanctioned satellites:

| Crate | DB file | Reason | Owner |
|---|---|---|---|
| `vox-secrets` | `.vox/clavis_vault.db` | Blast-radius isolation: secrets must never share a process-level connection with user-data Codex. | Security |
| `vox-package` | `.vox_modules/local_store.db` | Transitional; folded away by M-67. | Package |

The list above is mirrored mechanically in
[`contracts/db/data-storage-policy.v1.yaml`](../../../contracts/db/data-storage-policy.v1.yaml)
(`tiers.a_relational.{owners, allow_direct_access, temporary_exceptions}`).
Three CI checks enforce no further drift:

* `vox ci db-schema-coverage` — every `CREATE TABLE` lives in an owner crate.
* `vox ci policy-allowlist-parity` — txt allowlist agrees with policy YAML.
* `vox ci turso-import-guard` — built-in prefixes auto-derived from policy YAML.

### What is NOT a satellite

* Operational JSON state (`.vox/process-supervision/*.state.json`, orchestrator
  context snapshots) — Tier D cache. See file headers for rationale.
* In-process `Arc<Mutex<HashMap>>` registries (rate limit buckets, broadcast
  subscriptions, per-request receipts) — ephemeral by design.

