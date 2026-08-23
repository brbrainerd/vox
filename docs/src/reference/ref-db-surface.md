---
title: "Database Query Reference"
description: "Complete syntactic reference for Vox db.* accessors and complex filtering criteria."
category: "Language Reference"
status: "current"
training_eligible: true

schema_type: "TechArticle"
---

# Reference: Database Query Surface

Vox provides a built-in typed surface targeting the unified storage layer (Codex/Arca) via the standard `db.*` API domain. 

## Standard Table Fetch & Mutations

When you declare a `table Model`, the compiler auto-instantiates a `db.Model` handler namespace holding explicit data actions. 

- `db.Model.all() -> list[Model]`  
  *Retrieve every matched record in a table.*
- `db.Model.find(id: Id[Model]) -> Option[Model]`  
  *Extract a specific row given a compiler-tracked typed Identifier key.*
- `db.Model.insert(fields) -> Id[Model]`  
  *Insert mapping with schema constraints automatically typed and parameterized. ID is returned upon storage completion.*
- `db.Model.update(id: Id[Model], diff) -> Unit`  
  *Replaces explicit parameters targeted inside `diff` directly over the previously generated ID scope.*
- `db.Model.delete(id: Id[Model]) -> Unit`  
  *Removes row associated with that specific Identifier entirely.*

## Filters and Predicates

Query structures map to literal internal predicates mapped across your database indexes mapping securely. Note: Filtering and pagination requires appending `.all()` to trigger SQL fulfillment. 

- `db.Model.filter({ field: val })`  
  *Creates simple equality matches across the field table parameters.* 
  ```vox
  // vox:skip -- db.Table.filter() is not yet implemented (see examples/golden/db_advanced_queries.vox)
  db.User.filter({ age: 30 }).all()
  ```

- `db.Model.where({ field: { predicate } })`  
  *Accepts complex structured parameter ranges such as `gt`, `lt`, `eq`, `ne`, `in`.* 
  ```vox
  // vox:skip -- db.Table.where() is not yet implemented (see examples/golden/db_advanced_queries.vox)
  db.User.where({ age: { gt: 18, lt: 65 }, status: { ne: "blocked" } }).all()
  ```

## Query Context Chaining 

The Vox DB handler uses deterministic chained methods. 

- `.order_by("field", "asc" | "desc")`  
  *Orders results chronologically or structurally based on the explicit field value sequence.*
- `.limit(n: int)`  
  *Determines max response array element limits.*
- `.select("field1", "field2")`  
  *Performs column restrictions at query transit.* 

**Chain Aggregation Example**:
```vox
// vox:skip -- .where()/.order_by()/.limit() chaining is not yet implemented (see examples/golden/db_advanced_queries.vox)
return db.User
   .where({ role: { eq: "admin" } })
   .order_by("created_at", "desc")
   .limit(5)
   .all()
```

## Advanced Storage Modifiers 

These chainable context selectors modify *how* the operation interacts with the underlying Arca distribution: 

- `.using("hybrid")` / `.using("fts")` / `.using("vector")`  
  *Instructs VoxDb to use advanced indexing patterns (full-text or vector space).*
- `.live("channel")`  
  *Marks result sets as real-time subscriptions linked to a websocket client.*
- `.scope("name")`  
  *Isolates queries within multitenant architectures seamlessly.*
- `.sync()`  
  *Forces local edge SQLite consistency mapping back to global Turso control planes immediately.*

## Database Escape Hatch 

- `db.query(sql: str, params: list[T]) -> list[Result]`
  *Allows writing explicit raw parameter-bound queries that entirely bypass the compiler's safety assertions. Designed exclusively for highly customized analytics scripts mapping across disparate tables.*

## Transactional boundaries: `query` vs `mutation` vs `server`

Verified against `crates/vox-codegen/src/codegen_rust/emit/http.rs` and
`crates/vox-compiler/src/app_contract.rs`.

- **`query`** handlers (`emit_query_fn_handler`) never run inside a
  transaction. There is no transaction parameter in that code path at all —
  queries are read-only by construction, so there's nothing to make atomic.
- **`mutation`** handlers run inside `db.transaction(async move { ... })`
  whenever the enclosing module declares any `table` — `wraps_db_transaction`
  in `app_contract.rs` is `!module.tables.is_empty()`, a **module-level**
  flag applied uniformly to every mutation in that module, not a per-function
  decision based on whether a given mutation actually touches the database.
  In a module with no tables, mutations run unwrapped.
- **`server`** handlers share the exact same emitter as `mutation`
  (`emit_server_fn_handler`) but are called with the transaction flag
  hard-coded to `false`. A `server` function that writes to the database gets
  no atomicity guarantee — that's a deliberate consequence of the surface,
  not an oversight: `server` is the boundary for side effects the compiler
  can't reason about (external APIs, non-DB I/O), so wrapping it in a DB
  transaction would be a false promise for the common case where there's no
  DB write in the body at all.

Practical implication: if you need transactional atomicity, the write has to
be a `mutation` in a module that has at least one `table` declared — a
`server` function with a DB write inside it is not atomic, even though the
syntax looks identical to a `mutation`.
