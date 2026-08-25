---
title: "Codex HTTP API"
description: "Official documentation for Codex HTTP API for the Vox language. Detailed technical reference, architecture guides, and implementation pat"
category: "Language Reference"
training_eligible: true

schema_type: "TechArticle"
---

# Codex HTTP API

Rust implementation surfaces live in **`vox-db`** (Codex schema, readiness, store ops). There is **no** separate `vox-codex-api` workspace crate; operators integrate HTTP routers built on **`vox_db`** types (see OpenAPI below).

## SSOT

- **OpenAPI 3** — [`contracts/codex-api.openapi.yaml`](../../../contracts/codex-api.openapi.yaml) (validated by **`vox ci check-codex-ssot`**).

## Tests

- `cargo test -p vox-db` — integration tests under [`crates/vox-db/tests/`](../../../crates/vox-db/tests/) (e.g. `ops_codex_tests.rs`) exercise Codex HTTP / store behavior where applicable.

## Defaults

| Item | Value |
|------|--------|
| Bind | `VOX_DASH_HOST` (default `127.0.0.1`) + `VOX_DASH_PORT` (default `3847`) when a dashboard-compatible server is run |
| Readiness | `GET /ready` uses [`vox_db::evaluate_codex_api_readiness`](../../../crates/vox-db/src/codex_schema.rs) (baseline `schema_version` **1** + required tables + manifest digest) |

## Speech ingress (`/api/audio/*`)

OpenAPI paths **`GET /api/audio/status`**, **`POST /api/audio/transcribe`**, **`POST /api/audio/transcribe/upload`** are implemented by the speech ingress stack ([`crates/vox-speech`](../../../crates/vox-speech) library surfaces + thin HTTP adapters): Oratio STT on **paths under `VOX_ORATIO_WORKSPACE`** (or process CWD) or **multipart upload**. Same bind vars as the table above. This is separate from Codex CRUD routes but lives in the shared [`contracts/codex-api.openapi.yaml`](../../../contracts/codex-api.openapi.yaml) catalog for client codegen.

## Rust API surface (`Codex` = `vox_db::VoxDb`)

`Codex` is a type alias, not a distinct struct: `pub type Codex = VoxDb` in
[`crates/vox-db/src/lib.rs`](../../../crates/vox-db/src/lib.rs). It's the
type injected as `Extension<Arc<Codex>>` into every generated Axum handler
(see [Transactional boundaries](ref-db-surface.md#transactional-boundaries-query-vs-mutation-vs-server)),
and the type every generated per-table CRUD method takes as its first
argument (`db: &Codex`).

**Connecting:** `Codex::connect(config: DbConfig)`, `connect_default()`, and
`connect_canonical()` (the last resolving `VOX_DB_URL`/`VOX_DB_TOKEN`, legacy
`TURSO_*`, or a local file path, per [env-vars.md](env-vars.md)).

**Transactions:** `db.transaction(f)` runs an async closure between `BEGIN`
and `COMMIT`, rolling back on error
([`facade/migrations.rs`](../../../crates/vox-db/src/facade/migrations.rs)).
This is the primitive generated `mutation` handlers wrap around whenever
their module has a `table` (see [Transactional boundaries](ref-db-surface.md#transactional-boundaries-query-vs-mutation-vs-server)).
**Caveat, stated in the method's own doc comment:** the closure is awaited
without holding a guard across it, so concurrent use of the same `Codex`
inside the closure is unsafe — keep transaction bodies short and sequential.

**Raw queries:** `db.connection()` returns a `GuardedConnection` wrapping
`turso::Connection` behind a `tokio::sync::Mutex` — every `query`/`execute`/
`execute_batch`/`pragma_update` call acquires the lock for that call only.
This exists to prevent the underlying driver's
`Misuse("concurrent use forbidden")` error class. One documented residual
edge case: `last_insert_rowid()` is deliberately *not* guarded (it's a
synchronous, non-blocking read), so a call-site pattern of
`execute(...).await; connection.last_insert_rowid()` can race with another
task's guarded `execute()` in between and silently observe the wrong task's
row id — noted in the source as a known gap, not fixed by the guard.

**Generated per-table methods** (`insert`, `get`, `all`, `all_order_limit`,
`count`, `count_where`, `filter_where`, `filter_where_order_limit`,
`unsafe_query_raw_clause`, `delete`) are emitted per `table` declaration by
[`crates/vox-codegen/src/codegen_rust/emit/tables/codegen.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/tables/codegen.rs) —
these are the Rust functions the `.vox`-level `db.Table.*` surface in
[ref-db-surface.md](ref-db-surface.md) desugars to.

**Facade extension modules** under [`crates/vox-db/src/facade/`](../../../crates/vox-db/src/facade/)
add hand-written `impl VoxDb` methods for platform subsystems, not generated
per-project schema:

| Module | Adds |
|---|---|
| `connect.rs` | `connect`/`connect_default`/`connect_canonical` |
| `migrations.rs` | `transaction`, schema migration ops |
| `memory.rs` | `store_memory`, `search_memories`, ingested-chunk search |
| `workflow.rs` | durable-workflow activity result persistence, project registration |
| `hitl_approvals.rs` | durable audit trail for the `PendingApprovals` HITL gate (see [`tool` confirmation](ref-decorators.md)) |
| `agent_runs.rs` | the canonical `agent_runs` ledger (one row per agent/CLI invocation) |
| `actor_state.rs` | actor KV state persistence |
| `scheduled.rs` | `scheduled_runs` state for the workflow scheduler |
| `scientia.rs` | discovery pending/approve/reject ops |
| `vox_mesh.rs` | kudos and peer-reputation ops |
| `model_prompt.rs` | `model_prompt_profiles` table ops |
| `writer_raw.rs` | raw event/cost/exec-history inserts, bypassing the generated-schema path |

## Related

- [Environment variables (SSOT)](env-vars.md) — `VOX_DASH_*`, Codex DB envs
- [Codex BaaS scaffolding](../archive/research-2026-q1/codex-baas.md)
- [Codex vNext schema](../archive/research-2026-q1/codex-vnext-schema.md)
- [Nomenclature migration map](../archive/research-2026-q1/nomenclature-migration-map.md) — retired `vox-codex-api` name


