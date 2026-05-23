---
title: "Database Query Reference"
description: "Complete syntactic reference for Vox db.* accessors and complex filtering criteria."
category: "Language Reference"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---

# Reference: Database Query Surface

Vox provides a built-in typed surface targeting the unified storage layer (Codex/Arca) via the standard `db.*` API domain. 

## Standard Table Fetch & Mutations

When you declare an `@table type Model`, the compiler auto-instantiates a `db.Model` handler namespace holding explicit data actions. 

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
  // vox:skip
  db.User.filter({ age: 30 }).all()
  ```

- `db.Model.where({ field: { predicate } })`  
  *Accepts complex structured parameter ranges such as `gt`, `lt`, `eq`, `ne`, `in`.* 
  ```vox
  // vox:skip
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
// vox:skip
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
