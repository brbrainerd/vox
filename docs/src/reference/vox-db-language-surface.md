---
title: "Vox database language surface (canonical)"
description: "Canonical @table, @query, @mutation, @server, and db.* operations for Turso/Codex — low-K syntax for LLM-authored code."
category: "Language Reference"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "TechArticle"
---

# Vox database language surface (canonical)

This page is the **single** SSOT for how persistence appears in `.vox` source. Older docs that show `@get`, `db.User.find` without `get`, or `db.query(Task)` as the primary API are **deprecated**; align new examples here.

## Declarations

- **`@table type Name { field: Type ... }`** — Turso table + generated Rust row type. A surrogate **`_id`** column (integer primary key) is always added; do **not** add a separate column named `id` (the compiler warns; use another name for application ids).
- **`@index Table.idx on (col1, col2)`** — B-tree index DDL.
- **`@query fn name(...) to T { ... }`** — Read-oriented function; HTTP route **`GET /api/query/<name>`** with JSON-encoded query parameters (sorted keys). Compiler rejects `insert`/`delete`/raw `.query(...)` inside `@query`.
- **`@mutation fn name(...) to T { ... }`** — Write-oriented function; **`POST /api/mutation/<name>`**.
- **`@server fn name(...) to T { ... }`** — General RPC; **`POST /api/<name>`**.
- **HTTP routes** — Use `http get|post|put|delete "/path" to T { ... }` (optional named handler forms are not in the canonical grammar; see parser tests).

## `db` operations (HIR: `DbTableOp` + `FilterRecord` / `Count`)

Inside functions, `db` is an implicit binding. Table handles are **`db.TableName`** (PascalCase matches `@table` type name).

| Method | Meaning | Safety |
|--------|---------|--------|
| **`db.Table.insert(record)`** | Insert row (`serde` struct / JSON object). | Parameterized `INSERT`. |
| **`db.Table.get(id)`** | Load by `_id`. | Parameterized `SELECT`. |
| **`db.Table.find(id)`** | Alias of **`get`** (LLM-friendly spelling). | Same as `get`. |
| **`db.Table.delete(id)`** | Delete by `_id`. | Parameterized `DELETE`. |
| **`db.Table.all()`** | Full scan **`SELECT *`**. | Safe; no user SQL fragment. |
| **`db.Table.filter({ col: value, ... })`** | Equality predicates combined with **`AND`**; keys must be real columns. | Parameterized `WHERE`; HIR **`FilterRecord`**. |
| **`db.Table.where({ ...predicate... })`** | Predicate-object form (`eq`, `neq`, `lt`, `lte`, `gt`, `gte`, `in`, `contains`, `is_null`, `and`, `or`, `not`). | Parameterized SQL from typed predicate IR; no raw clause strings. |
| **`db.Table.all().order_by("col", "asc\|desc").limit(n)`** | Ordered / capped list for table scans. | Compiler validates column names; emits typed `ORDER BY` / `LIMIT` helpers. |
| **`db.Table.filter({...}).order_by("col", "asc\|desc").limit(n)`** | Ordered / capped filtered reads. | Parameterized filter + validated order/limit modifiers. |
| **`db.Table.count()`** | **`SELECT COUNT(*)`** for the table. | Safe aggregate; HIR **`Count`**. |
| **`db.Table.filter({...}).count()`** | Count with equality predicates. | Parameterized `COUNT(*) WHERE ...`; HIR lowers chain to `Count` + filter args. |
| **`... .sync()`** | Plan capability hint: pull replica/sync-backed stores before query execution. | Lowers to plan capability `requires_sync`; Rust backends may sync before execution. |
| **`... .using("fts" \| "vector" \| "hybrid")`** | Retrieval strategy hint for search/retrieval paths. | Lowers to plan capability `retrieval_mode` for backend/tooling selection. |
| **`... .live("topic")`** | Mark query for live invalidation/subscription topic linkage. | Lowers to plan capability `live_topic` + `emits_change_log`. |
| **`... .scope("populi" \| "orchestrator" \| "...")`** | Attach orchestration routing scope metadata. | Lowers to plan capability `orchestration_scope`. |
| **`db.Table.query(clause)`** | Dynamic fragment after `SELECT * FROM t`. | **Lint-category Error:** prefer **`filter`**, **`all()`**, or **`get`**/`find`; Rust emits **`unsafe_query_raw_clause`**. |

## Nullable columns

Use **`Option[T]`** in the `@table` field type for **NULL** SQL columns; other fields get **`NOT NULL`** in generated DDL.  
`select(...)` projections may return partial rows; omitted fields are not auto-required.

## Deprecated / do not teach to models

- `@get("/path")` — use `http get "/path" to T { ... }` (same form as other verbs).
- `db.User.find` **without** `get` — use **`find` == `get`** as above.
- `db.query(Task)` / Convex-only TS styles — not the Rust/Turso path; see TS codegen separately.

## Data-lane crate policy

The first-class data lane is `turso+vox-db` behind Vox language/database surfaces.

- Treat `sqlx`, `diesel`, and `sea-orm` as deferred or escape-hatch crate families unless a concrete lane requirement is proven.
- Prefer bounded wrappers and query capability metadata over exposing broad ORM APIs directly in Vox.
- Re-score deferred ecosystems against capability value vs debt cost before any tier promotion.

## Related

- [Environment variables](./env-vars.md) — `VOX_DB_*`, `VOX_EMBEDDING_SEARCH_CANDIDATE_MULT`.
- [ADR 004: Codex / Arca / Turso](../adr/004-codex-arca-turso-ssot.md)
