---
title: "How-To: The Database Layer"
description: "How to perform CRUD operations, type-safe queries, and indexing using Vox's integrated database layer."
category: "How-To Guides"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "HowTo"
---

# How-To: Use the Database Layer

Vox utilizes a unified storage paradigm known as Codex, which compiles into type-safe SQLite database schemas and Rust structs. You never need to write raw migrations; they are deterministically derived from your file structures.

## Defining a Table

Any type struct adorned with the `@table` decorator becomes a persistent database entity.

```vox
{{#include ../../../examples/golden/getting_started.vox:data_model}}
```

### Indexing for Performance

To speed up lookups on large datasets, use the `@index` syntax. Vox determines the optimal storage engine (B-Tree or Hash) and generates the SQL automatically.

```vox
// vox:skip
@table type User {
    email: str
    team_id: Id[Team]
}

// Unique index: prevents duplicate emails
@index User.unique_email on (email) unique

// Composite index: speeds up filtered team lookups
@index User.by_team on (team_id, email)
```

> [!TIP]
> Always index foreign keys (like `Id[T]`) if you plan to filter or join on them frequently.

## Basic CRUD Accessors

The built-in `db` module uses code-generation to inject statically typed accessors for all your `@table` types.

- **Create**:
  ```vox
  // vox:skip
  let new_id: Id[Task] = db.Task.insert({ 
      title: "Clean desk", 
      done: false, 
      priority: 1, 
      owner: "alice" 
  })
  ```
- **Read**:
  ```vox
  // vox:skip
  match db.Task.find(new_id) {
      Some(t) -> println(t.title)
      None    -> println("Not found")
  }
  ```
- **Update**:
  ```vox
  // vox:skip
  db.Task.update(new_id, { done: true })
  ```
- **Delete**:
  ```vox
  // vox:skip
  db.Task.delete(new_id)
  ```

## Advanced Filtering

Instead of raw string interpolation, use Vox's exact literal querying to avoid injection attacks.

// Fetch simple exact match parameters
```vox
// vox:skip
let alice_tasks = db.Task.filter({ owner: "alice" })
```

// Advanced predicate-object queries
```vox
// vox:skip
let urgent_tasks = db.Task.where({ priority: { gt: 10 }, done: { eq: false } }).all()
```

### Query Chaining

You can apply limits, multi-field ordering, and select specific field projections by chaining.

```vox
// vox:skip
let feed = db.Task
            .where({ done: false })
            .order_by("priority", "desc")
            .limit(10)
            .all()
```

## Guarding Reads/Writes with `@endpoint(kind: query)` and `@endpoint(kind: mutation)`

For security, you should rarely expose `db.*` calls directly to UI islands or agents. Instead, wrap your database interactions in `@endpoint(kind: query)` (read-only) and `@endpoint(kind: mutation)` (write-enabled) functions.

The compiler verifies that an `@endpoint(kind: query)` function does not contain `.insert`, `.update`, or `.delete` operations.

### Transactional Integrity with `@endpoint(kind: mutation)`

Every function marked with `@endpoint(kind: mutation)` is automatically wrapped in a database transaction. If the function returns an `Error` or panics, the transaction is rolled back.

```vox
// vox:skip
@endpoint(kind: mutation)
fn transfer_funds(from: Id[Account], to: Id[Account], amount: int) to Result[Unit] {
    let mut sender = db.Account.find(from)?
    let mut receiver = db.Account.find(to)?
    
    sender.balance -= amount
    receiver.balance += amount
    
    db.Account.update(from, sender)
    db.Account.update(to, receiver)
    
    return Ok(())
}
```

Under the hood, this uses `Codex::transaction` to ensure ACID compliance across the local SQLite or distributed Turso mesh.


## The Escape Hatch: Raw SQL

Occasionally, complex analytic aggregations exceed the currently supported ORM builder patterns. You can drop down to raw SQL using `db.query`. 

> [!WARNING]
> Use this **only** as a last resort. Raw SQL queries bypass Vox's type checking checks on schema changes. 

```vox
// vox:skip
let count = db.query("SELECT COUNT(*) FROM Task WHERE owner = ?", ["alice"])
```

## A Note on Codex 

When running `vox-run`, the backing data source is the Local Codex Store (an embedded SQLite engine on disk). For enterprise orchestration and Populi GPU meshes, the database seamlessly promotes to Turso cloud sync clusters dynamically, without requiring any changes to your `.vox` schema definitions!

---

**Related Topics**:
- [Reference: Database Surface](../reference/ref-db-surface.md)
- [Tutorial: Your First App](../tutorials/tut-first-app.md)
