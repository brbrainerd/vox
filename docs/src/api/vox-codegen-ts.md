# Crate API: vox-codegen-ts

## Overview

TypeScript/TSX code generator for the Vox compiler. Emits React components, fetch wrappers, ADT types, and route definitions.

## Purpose

Transforms the typed HIR into TypeScript source files. The emitter is modularized by concern — each module handles a specific category of output.

## Key Files

| File | Purpose |
|------|---------|
| `emitter.rs` | `generate()` — main entry point, orchestrates all modules |
| `jsx.rs` | React JSX component rendering |
| `component.rs` | `@component` declarations and hook wiring |
| `activity.rs` | Activity and workflow client wrappers |
| `routes.rs` | React Router route definitions |
| `adt.rs` | TypeScript discriminated union types from Vox ADTs |

## Output Mapping

| Vox | Generated TypeScript |
|-----|---------------------|
| `@component fn` | React functional component |
| `@server fn` | Typed `fetch()` wrapper |
| `type A = \| B \| C` | Discriminated union type |
| `routes:` | React Router `<Route>` elements |
| `@deprecated` | `/** @deprecated */` JSDoc |
| `style:` | CSS-in-JS object |

## Usage

```rust
use vox_codegen_ts::generate;

let ts_output = generate(&ast_module);
// ts_output: String — complete TypeScript/TSX file
```

---

### `fn generate_activity`

Generate a TypeScript async function from a Vox activity declaration.
Returns the TypeScript source code for the activity.


### `fn generate_activity_runner`

Generate a TypeScript wrapper function that applies `with` options
at the call site. This emits a helper that wraps the activity call
with retry/timeout logic.


### `fn generate_types`

Generate TypeScript type definitions from Vox ADTs and struct types.


### `fn generate_component`

Generate a React component from a Vox @component function declaration.
Returns (filename, content) tuple.


### `fn map_vox_type_to_ts`

Map a Vox type expression to a TypeScript type string.


### `struct CodegenOutput`

Output from the TypeScript code generator.


### `fn map_jsx_attr_name`

Map Vox JSX attribute names to React attribute names.


### `fn emit_jsx_element`

Emit a JSX element with children to TypeScript.


### `fn emit_jsx_self_closing`

Emit a self-closing JSX element.


### `fn emit_expr`

Emit a Vox expression as TypeScript.


### `fn emit_stmt`

Emit a Vox statement as TypeScript.


### `fn emit_pattern_public`

Emit a pattern as TypeScript destructuring.


## Module: `vox-codegen-ts\src\lib.rs`

# vox-codegen-ts

TypeScript/TSX code generator for the Vox compiler. Emits React
components, fetch wrappers, discriminated union types, and route
definitions from the Vox AST.


### `fn generate_routes`

Generate Express.js route handlers from Vox http route and @server fn declarations.


### `fn generate_api_client`

Generate a typed API client for HTTP routes and server functions.


### `fn generate_voxdb_schema`

Generate a VoxDB `schema.ts` from all @table, @index, and @vector_index declarations.

Emits `defineSchema({ tableName: defineTable({ ... }) })` with proper VoxDB validators.


### `fn type_to_voxdb_validator`

Map a Vox TypeExpr to a Convex validator expression (e.g. `v.string()`).


