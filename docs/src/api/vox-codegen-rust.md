# Crate API: vox-codegen-rust

## Overview

Rust code generator for the Vox compiler. Emits Axum server code, actor implementations, table schemas, and test harnesses.

## Purpose

Transforms the typed HIR into runnable Rust source using the [`quote!`](https://docs.rs/quote) macro. Each Vox decorator maps to specific Rust constructs.

## Key Files

| File | Purpose |
|------|---------|
| `emit.rs` | `generate()` — main entry point, `CodegenOutput` type |
| `emit_main.rs` | Generates `main()` with Axum router setup |
| `emit_table.rs` | Generates Turso schema definitions from `@table` |
| `emit_expr.rs` | Expression-level code emission |
| `emit_agent.rs` | Agent definition code generation |
| `emit_lib.rs` | Library-level module emission |
| `emit_trait.rs` | Trait and interface code generation |

## Decorator Mapping

| Vox | Generated Rust |
|-----|---------------|
| `@server fn` | Axum handler + route registration |
| `@table type` | Struct + Turso `CREATE TABLE` |
| `@test fn` | `#[test]` function |
| `@deprecated` | `#[deprecated]` attribute |
| `@pure` | `/* @pure */` annotation |
| `actor` | Tokio task + mpsc mailbox struct |
| `workflow` | Durable state machine |

## Usage

```rust
use vox_codegen_rust::generate;

let output = generate(&hir_module);
// output.main_rs — main entry point with Axum routes
// output.lib_rs  — library module with types and functions
```

---

### `fn generate`

It also emits an optional TypeScript API client for any `@server` functions.


### `fn emit_cargo_toml`

Generate a `Cargo.toml` manifest with required dependencies for the
synthesized Rust project.


### `fn emit_fn`

Emit a single Rust function from a HIR function declaration.

Handles `#[deprecated]`, `/* @pure */` annotations, `@require` assertions,
parameter types, return types, and the function body.


### `fn emit_type`

Convert a HIR type to its Rust equivalent.

Maps Vox primitives (`int` → `i64`, `str` → `String`),
generic containers (`List[T]` → `Vec<T>`, `Option[T]` → `Option<T>`),
and composite types (Result, Map, tuples, functions).


### `fn capitalize`

Capitalize the first letter of a string (used for struct names).


### `fn emit_api_client`

Generate a TypeScript API client for all `@server` functions.

Produces `fetch()`-based wrappers that call the server's HTTP endpoints.


### `fn hir_type_to_ts`

Convert a HIR type to its TypeScript equivalent.

Maps Vox types to TS types: `int`/`float` → `number`, `str` → `string`,
`List[T]` → `T[]`, `Map[K,V]` → `Record<K,V>`.


### `fn emit_stmt_warp`

Like emit_stmt but for warp route handlers (emits Ok(warp::reply::json(...))).


### `fn emit_server_fn_handler`

Generate a warp handler for a function exposed as an API endpoint.


### `fn emit_sse_handler`

Generate a warp SSE handler for a `@server fn` returning `Stream[T]`.

Each `emit value` statement in the body maps to a SSE data event.


### `fn emit_db_setup`

Uses Turso: local file (default) or remote when VOX_TURSO_URL and VOX_TURSO_TOKEN are set.


### `fn emit_mcp_server`

Generate the MCP (Model Context Protocol) JSON-RPC server binary.
Communicates over stdio: reads JSON-RPC requests from stdin, writes responses to stdout.


### `fn hir_type_to_sql`

Map a Vox HIR type to a Turso-compatible column type.


### `fn emit_table_struct`

Generate a Rust struct for a @table type with async Turso-backed methods.


### `fn emit_table_ddl`

Generate `CREATE TABLE IF NOT EXISTS` DDL for a @table.


### `fn emit_index_ddl`

Generate `CREATE INDEX IF NOT EXISTS` DDL for a @index.


### `fn emit_collection_struct`

Generate a Rust struct for a @collection type wrapping serde_json::Value.


### `fn emit_collection_ddl`

Generate `CREATE TABLE IF NOT EXISTS` DDL for a @collection.


## Module: `vox-codegen-rust\src\lib.rs`

# vox-codegen-rust

Rust code generator for the Vox compiler. Emits warp server code
actor implementations, table schemas, and test harnesses using
the [`quote!`](https://docs.rs/quote) macro.


