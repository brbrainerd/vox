# vox-codegen-rust

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
