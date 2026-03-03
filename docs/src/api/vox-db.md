# Crate API: vox-db

## Overview

High-level database facade for the Vox ecosystem. Wraps `vox-pm::CodeStore` with connection management, retry logic, and transaction support.

## Connection Modes

| Mode | Feature Flag | Use Case |
|------|-------------|----------|
| Remote (Turso) | (default) | Production — cloud-hosted |
| Local Turso | `local` | Development — file-based |
| In-Memory | `local` | Testing — ephemeral |
| Embedded Replica | `replication` | Hybrid — local + cloud sync |

## Key APIs

| Method | Description |
|--------|-------------|
| `VoxDb::connect(config)` | Connect with automatic retry (3× w/ backoff) |
| `VoxDb::store()` | Access the underlying `CodeStore` |
| `VoxDb::sync()` | Sync embedded replica with remote |
| `VoxDb::schema_version()` | Get current schema version |
| `VoxDb::transaction(f)` | Execute within BEGIN/COMMIT/ROLLBACK |

## Usage

```rust
use vox_db::{VoxDb, DbConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = VoxDb::connect(DbConfig::Remote {
        url: "turso://my-db.turso.io".to_string(),
        token: "my-token".to_string(),
    }).await?;

    let hash = db.store().store("fn", b"fn hello(): ret 42").await?;
    println!("Stored: {hash}");
    Ok(())
}
```

---

## Module: `vox-db\src\auto_migrate.rs`

Auto-migration engine for VoxDB.

Introspects the live SQLite schema from the database, compares it against the
desired schema derived from `@table` declarations, and applies non-destructive
migrations (ADD COLUMN, CREATE TABLE, CREATE INDEX).

Destructive operations (DROP TABLE, DROP COLUMN) are never performed automatically
— they are reported as pending manual migrations.


### `struct LiveColumn`

A column definition as read from `PRAGMA table_info(...)`.


### `struct LiveTable`

A table definition as introspected from the live database.


### `enum MigrationAction`

A single migration action to be applied.


### `struct MigrationPlan`

Result of a migration plan.


### `struct AutoMigrator`

The auto-migration engine.


## Module: `vox-db\src\collection.rs`

NoSQL-style document collection backed by SQLite JSON columns.

A `Collection` wraps a single SQLite table with the schema:

```sql
CREATE TABLE IF NOT EXISTS <name> (
_id INTEGER PRIMARY KEY AUTOINCREMENT,
_data TEXT NOT NULL,
_created_at TEXT DEFAULT (datetime('now')),
_updated_at TEXT DEFAULT (datetime('now'))
);
```

Documents are stored as JSON in the `_data` column.
Queries use `json_extract()` for filtering and indexing.


### `struct Collection`

A handle to a schemaless document collection.

All CRUD operations translate to SQL under the hood:
- `insert` → `INSERT INTO <name> (_data) VALUES (?1)`
- `get`    → `SELECT _data FROM <name> WHERE _id = ?1`
- `find`   → `SELECT _id, _data FROM <name> WHERE json_extract(_data, '$.<key>') = <value>`
- `patch`  → `UPDATE <name> SET _data = json_patch(_data, ?1) WHERE _id = ?2`
- `delete` → `DELETE FROM <name> WHERE _id = ?1`


### `enum CollectionError`

Error type for collection operations.


### `fn collection_ddl`

Generate DDL for a collection table.


### `fn collection_index_ddl`

Generate DDL for a `json_extract` expression index on a collection field.


### `enum DbConfig`

Configuration for connecting to a Vox database.


## Module: `vox-db\src\data_flow.rs`

Data Flow Tracer — static analysis for database operation mapping.

Tracks which mutations write which tables and which queries read
which tables, enabling LLMs to understand data flow without reading
source code. Exposed via MCP tools for AI context.


### `struct DataFlowMap`

Complete data flow map for a Vox module.


### `struct DataFlowEntry`

A single data flow entry mapping a function to its affected tables.


### `fn build_data_flow`

Build a data flow map from a schema digest.

Uses the `affected_tables` heuristic from the schema digest
to determine which functions interact with which tables.


### `fn format_data_flow`

Format the data flow map for LLM context.


### `fn data_flow_to_json`

Serialize the data flow map to JSON.


## Module: `vox-db\src\ddl.rs`

DDL Compiler — convert `@table` AST declarations into SQLite DDL.

This is the bridge between the Vox type system and SQLite's physical schema.
It generates `CREATE TABLE`, `CREATE INDEX`, and type-safe DDL from the AST.


### `fn tables_to_ddl`

Generate `CREATE TABLE` SQL statements from table declarations.


### `fn table_to_ddl`

Generate a single `CREATE TABLE` statement.


### `fn collections_to_ddl`

Generate `CREATE TABLE` SQL statements from collection declarations.


### `fn collection_to_ddl`

Generate a single `CREATE TABLE` and schema storage statement for a collection.


### `fn indexes_to_ddl`

Generate `CREATE INDEX` SQL statements from index declarations.


### `fn index_to_ddl`

Generate a single `CREATE INDEX` statement.


### `fn collection_index_to_ddl`

Generate a single `CREATE INDEX` statement for a collection document field.


### `fn vector_index_to_ddl`

Generate DDL for a vector index (stored as metadata table + index).


### `fn table_info_to_ddl`

Generate `CREATE TABLE` from `TableInfo`.


### `fn collection_info_to_ddl`

Generate `CREATE TABLE` from `CollectionInfo`.


### `fn index_info_to_ddl`

Generate `CREATE INDEX` from `IndexInfo`.


### `fn vox_type_to_sqlite_type`

Map a Vox type string (e.g. "str", "Option[int]") to a SQLite type.


### `fn type_to_sqlite_type`

Map a Vox `TypeExpr` to a SQLite column type.


### `fn to_snake_case`

Convert PascalCase to snake_case for SQL table names.


### `struct SchemaDiff`

Represents differences between two schema versions.


### `fn diff_schemas`

Diff two schema versions to produce migration SQL.


### `fn diff_to_sql`

Generate SQL migration statements from a schema diff.


### `fn describe_diff`

Generate a human-readable description of schema changes.


## Module: `vox-db\src\error_enrichment.rs`

Error Enrichment for VoxDB — LLM-first error messages.

When a database operation fails, this module enriches the error with
schema context so AI models can self-correct without needing to re-read
the schema. This is a key differentiator over traditional databases
where error messages are opaque to LLMs.


### `struct EnrichedDbError`

An enriched database error with schema context.


### `fn enrich_error`

Enrich a database error with schema context.

Takes a raw error message and the current schema digest, then produces
an enriched error that includes:
- Which table was involved
- What fields are available
- Fuzzy-matched suggestions for typos
- Example correct usage


### `fn format_enriched_error`

Format an enriched error for display (suitable for LLM consumption).


## Module: `vox-db\src\learning.rs`

Behavioral Learning Engine for VoxDB.

The `BehavioralLearner` observes user actions, detects usage patterns,
infers preferences, and generates suggestions for improving the user
experience. It sits on top of the `CodeStore` CRUD layer and provides
higher-level analytics.


### `struct BehavioralLearner`

High-level behavioral learning engine.

Records user actions, automatically detects patterns, and generates
suggestions for improving the development workflow.


### `struct Suggestion`

A suggestion generated by the learning engine.


### `struct FrequencyItem`

A frequency analysis result.


### `struct TimeUsageBucket`

A time-of-day usage bucket.


### `struct ErrorPattern`

Error pattern with details.


### `struct ActionSequence`

A detected action sequence (workflow).


## Module: `vox-db\src\lib.rs`

# vox-db — High-level database facade for Vox

Provides a unified `VoxDb` interface that wraps `vox_pm::CodeStore` and
supports multiple connection modes:

- **Remote** (Turso cloud) — always available
- **Local** (file-based Turso) — requires `local` feature
- **Embedded replica** (local + cloud sync) — requires `replication` feature

```no_run
use vox_db::{VoxDb, DbConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
let db = VoxDb::connect(DbConfig::Remote {
url: "turso://my-db.turso.io".to_string(),
token: "my-token".to_string(),
}).await?;

let hash = db.store().store("fn", b"fn hello(): ret 42").await?;
println!("Stored: {hash}");
Ok(())
}
```


### `struct VoxDb`

High-level database facade for the Vox ecosystem.

Wraps `CodeStore` and provides convenience methods for common operations.


### `struct Migration`

Declarative schema migration entry.


### `fn validate_migrations`

Validate migration ordering and uniqueness.


### `fn builtin_migrations`

Returns the canonical set of built-in schema migrations defined in vox-pm.

These correspond 1:1 with the `MIGRATIONS` constant in `vox_pm::schema`.


## Module: `vox-db\src\paths.rs`

Cross-platform data directory resolution for Vox.

Delegates to `vox_config` for a single source of truth. Re-exports for backward compatibility.


### `enum RetrievalMode`

Retrieval mode for hybrid search plans.


### `struct RetrievalQuery`

Query specification for retrieval pipelines.


### `struct RetrievalResult`

Minimal retrieval result metadata suitable for provenance capture.


### `fn fuse_hybrid_results`

Merge vector/full-text candidates with simple weighted rank fusion.


## Module: `vox-db\src\schema_digest.rs`

Schema Digest Generator — the LLM Context Engine for VoxDB.

Walks `Module` AST declarations and produces a structured `SchemaDigest`
that makes the database fully self-describing for AI models.

This is the **core differentiator** that makes VoxDB "LLM-first":
AI coding assistants using VoxDB always know the exact database shape,
field types, relationships, indexes, and can generate accurate queries
without guessing.


### `struct SchemaDigest`

Complete schema digest — the single source of truth for LLM context.


### `struct TableInfo`

Information about a single database table.


### `struct CollectionInfo`

Information about a single document collection.


### `struct FieldInfo`

Information about a single table field.


### `struct Relationship`

A detected relationship between tables.


### `enum RelationshipKind`

Relationship kind.


### `struct IndexInfo`

Information about an index.


### `enum IndexKind`

The kind of index.


### `struct FunctionInfo`

Information about a query, mutation, or action function.


### `struct ParamInfo`

A function parameter.


### `fn generate_schema_digest`

Generate a schema digest from a parsed Vox module.

This is the primary entry point. Given an AST `Module`, it extracts
all database-related declarations and produces a structured digest
that can be:
1. Serialized to JSON and served via MCP tools
2. Embedded as comments in codegen output
3. Used for error enrichment


### `fn format_llm_context`

Generate a formatted context block suitable for LLM prompts.

This is the text that gets injected into AI assistant context so they
understand the database without reading source code.


### `fn digest_to_json`

Generate a JSON representation of the schema digest.


