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

```vox
table User {
    age: int
    status: str
    role: str
    created_at: str
}
```

Every fence on this page shares this one `User` table declaration.

## Standard Table Fetch & Mutations

When you declare a `table Model`, the compiler auto-instantiates a `db.Model` handler namespace holding explicit data actions. Signatures below are verified against the method table in
[`crates/vox-compiler/src/typeck/builtins.rs`](../../../crates/vox-compiler/src/typeck/builtins.rs) (`TypeckBuiltins::lookup_method` on `Ty::Table`) — every method returns `Result[T, str]`, not a bare `T`.

- `db.Model.all() -> Result[list[Model], str]`
  *Retrieve every record in a table.*
- `db.Model.find(id: Id[Model]) -> Result[Option[Model], str]`
  *Extract a specific row by primary key (alias of `.get()`).*
- `db.Model.insert(fields) -> Result[int, str]`
  *Insert a full record. Returns the inserted row's `rowid`, not an `Id[Model]`.*
- `db.Model.update(id: Id[Model], fields) -> Result[Unit, str]`
  *Full-record replace by primary key — `fields` must supply every column, same shape as `insert`. Added 2026-08-23; generates a parameterized `UPDATE ... SET ... WHERE` (see `crates/vox-codegen/src/codegen_rust/emit/tables/codegen.rs`).*
- `db.Model.delete(id: Id[Model]) -> Result[Unit, str]`
  *Removes the row at that key.*
- `db.Model.count() -> Result[int, str]`
  *Row count for the table.*

```vox
mutation deactivate(id: Id[User], age: int, role: str, created_at: str) to Result[Unit, str] {
    return db.User.update(id, { age: age, status: "inactive", role: role, created_at: created_at })
}
```

## Filters and Predicates

Verified 2026-08-23 by compiling each example below with `vox check` — all pass. An earlier version of this page marked these `// vox:skip` as "not yet implemented"; that was wrong, and so was the claim that `.all()` must be appended to a filter/where call — appending it is in fact a type error, since `.filter()`/`.where()` already return `Result[list[Model], str]` directly.

- `db.Model.filter({ field: val })`
  *Simple equality matches across fields.*
  ```vox
  query filtered_users() to Result[list[User], str] {
      return db.User.filter({ age: 30 })
  }
  ```

- `db.Model.where({ field: { predicate } })`
  *Structured predicates: `gt`, `lt`, `eq`, `ne` (see `crates/vox-compiler/src/hir/lower/expr_db.rs` for the full operator set).*
  ```vox
  query filtered_users_range() to Result[list[User], str] {
      return db.User.where({ age: { gt: 18, lt: 65 }, status: { ne: "blocked" } })
  }
  ```

## Query Context Chaining 

The Vox DB handler uses deterministic chained methods. Verified against the same `expr_db.rs` match arms as above.

- `.order_by("field", "asc" | "desc")`  
  *Orders results by the given field.*
- `.limit(n: int)`  
  *Caps the result count.*
- `.select("field1", "field2")`  
  *Restricts which columns are returned.*

**Chain Aggregation Example** (compiles as-is; note no trailing `.all()`):
```vox
query admins_recent() to Result[list[User], str] {
    return db.User.where({ role: { eq: "admin" } }).order_by("created_at", "desc").limit(5)
}
```

Note the chain is written on one line: Vox does not support a leading-dot method call continuing onto the next line the way some other languages do.

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

- `db.Model.query(clause: str) -> Result[list[Model], str]`
  *Not `db.query(sql, params)` — the escape hatch lives on the table handler, takes one raw SQL-fragment string (no separate params list), and is appended after `SELECT * FROM t`.*

This is deliberately hostile to use. `vox check` rejects it with a hard error by default —
`.query(clause) builds dynamic SQL; prefer .all() or .get(id)` — and it must be called from a
`mutation`, not a `query`, even for a read-only clause (the compiler cannot statically verify
a raw clause doesn't mutate). Reach for `.filter()`/`.where()` first; this exists for the small
minority of queries their predicate algebra can't express.

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
