---
title: "Codex / Arca compatibility boundaries"
description: "Official documentation for Codex / Arca compatibility boundaries for the Vox language. Detailed technical reference, architecture guides,"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Codex / Arca compatibility boundaries

This page is the **contract** between application code, `vox-db`, and `vox-pm` for persisted data. It implements the boundaries implied by [ADR 004: Codex over Arca over Turso](../adr/004-codex-arca-turso-ssot.md).

## Naming

| Layer | Name | Rust / code |
|-------|------|-------------|
| Public product API | **Codex** | `vox_db::Codex` (type alias for `VoxDb`) |
| Stable ABI / legacy call sites | **VoxDb** | `vox_db::VoxDb` |
| Schema + SQL DDL ownership | **Arca** | [`crates/vox-db/src/schema/`](../../../crates/vox-db/src/schema/) (`SCHEMA_FRAGMENTS`, `BASELINE_VERSION`) |
| Engine | **Turso / libSQL** | Only supported SQL backend for the same data plane |

Do not introduce a second physical store for the same logical data without a new ADR.

## What application code may call

- **Prefer** `VoxDb::connect` / `Codex::connect` with [`DbConfig`](../../../crates/vox-db/src/config.rs) from `vox-db`.
- **Prefer** `VoxDb::store` / domain helpers in `vox-db` for CAS and schema-backed operations.
- **Avoid** new direct `turso::` usage outside the [direct Turso allowlist](codex-turso-allowlist.md). If you must extend the allowlist, update that document in the same change.

## Configuration (canonical env)

| Variable | Role |
|----------|------|
| `VOX_DB_URL` | Remote libSQL / Turso URL |
| `VOX_DB_TOKEN` | Remote auth token (**never** commit; env-only per ADR 004) |
| `VOX_DB_PATH` | Local file path when using file-backed Codex |

Resolution for CLIs and long-running apps:

- `DbConfig::from_env` — minimal parsing; with `local` feature, empty env may yield in-memory for tests.
- `DbConfig::resolve_canonical` (alias of `resolve_standalone`) — **canonical user-global** Codex: `VOX_DB_*` first, then **legacy** `TURSO_URL` + `TURSO_AUTH_TOKEN`, then a concrete file path (never silent `:memory:` when `local` is enabled). See [how-to-voxdb-canonical-store](../how-to/how-to-voxdb-canonical-store.md).
- `open_project_db` — **non-canonical** repo-local `.vox/store.db` for snippets/share/cache only.

## Migrations and SQL rules (Arca)

- Schema DDL is owned by **`vox-db`** under [`schema/domains/`](../../../crates/vox-db/src/schema/domains/), ordered in [`manifest.rs`](../../../crates/vox-db/src/schema/manifest.rs) as **`SCHEMA_FRAGMENTS`** and applied once at **`BASELINE_VERSION`** (single maintained baseline row in `schema_version`). Older databases with `MAX(schema_version) != BASELINE_VERSION` must be **exported** (`vox codex export-legacy`), moved to a **new file**, then **imported** after baseline — no in-place bridge. Capability checks in `vox-db` use **required table sets**, not numeric version thresholds (see [codex-vnext-schema](codex-vnext-schema.md)).
- Higher-level writes for chat/search domains should go through **`VoxDb`** helpers in [`codex_chat.rs`](../../../crates/vox-db/src/codex_chat.rs) where possible instead of ad-hoc SQL.
- Bodies use patterns consistent with Turso batch execution: **`execute_batch`** for non-row-returning DDL/DML; pragmas via **`pragma_update`** where applicable. Fragment `v7` remains intentionally empty in the manifest (historical no-op).

## Convex-like features

Subscriptions, change logs, invalidation, and HTTP streaming are **Codex capabilities** layered on one database — not a separate DB product (ADR 004 § Decision item 5).

## Verification

- **`vox ci check-codex-ssot`** (shim: `scripts/check_codex_ssot.sh`) — required SSOT files exist (includes this page).
- **`vox ci check-docs-ssot`** (shim: `scripts/check_docs_ssot.sh`) — doc inventory and path references.
- Crate tests: `cargo test -p vox-db --lib` (with `local` feature as in CI) exercises in-memory Codex and the `Codex` alias.

## Related

- [Codex BaaS scaffolding](codex-baas.md)
- [Direct Turso usage allowlist](codex-turso-allowlist.md)
- [Forward migration charter](forward-migration-charter.md)
- [Doc-to-code acceptance checklist](doc-to-code-acceptance-checklist.md)


