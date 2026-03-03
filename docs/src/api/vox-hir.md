# Crate API: vox-hir

## Overview

High-Level Intermediate Representation for the Vox compiler. Desugars syntax, resolves names, and detects dead code.

## Purpose

Transforms the typed AST from `vox-ast` into a simpler, canonical IR that the type checker and code generators consume. This stage resolves all identifier references to their definitions and detects unreachable code.

## Key Files

| File | Purpose |
|------|---------|
| `hir.rs` | `HirModule`, `HirDecl`, `HirExpr`, `HirStmt` — IR node types |
| `lower.rs` | `lower_module()` — AST → HIR transformation |
| `def_map.rs` | `DefMap` — name resolution mapping identifiers to definitions |
| `dead_code.rs` | Dead code detection pass |
| `validate.rs` | `validate_module()` — HIR validation rules |

## Usage

```rust
use vox_hir::lower_module;

let hir_module = lower_module(&ast_module);
// hir_module contains resolved references and desugared expressions
```

## Key Operations

1. **Name resolution** — All identifiers are resolved to their definitions via `DefMap`
2. **Desugaring** — Complex patterns and expressions are simplified
3. **Dead code detection** — Unreachable functions and variables are flagged
4. **Validation** — Structural invariants are checked before type checking

---

### `struct DefMap`

Tracks name → DefId mappings at each scope level.


### `struct DefId`

Unique identifier for definitions within a module.


### `struct HirModule`

A fully lowered Vox module ready for type checking and code generation.


### `struct HirImport`

A resolved import.


### `struct HirFn`

A function or component in HIR.


### `struct HirConst`

A constant declaration in HIR.


### `struct HirParam`

A function parameter in HIR.


### `enum HirType`

Type representation in HIR (resolved from TypeExpr).


### `enum HirExpr`

Expression in HIR (mirrors AST but with resolved names).


### `struct HirTypeDef`

ADT / struct type definition in HIR.

- **ADT** (sum type): `variants` populated, `fields` empty.
- **Struct** (product type): `fields` populated, `variants` empty.


### `struct HirRoute`

HTTP route in HIR.


### `struct HirActor`

Actor definition in HIR.


### `struct HirWorkflow`

Workflow definition in HIR.


### `struct HirActivity`

Activity definition in HIR.


### `struct HirServerFn`

A server function — callable from the frontend, auto-generates API route + fetch wrapper.


### `struct HirTable`

Table definition — a persistent record type.


### `struct HirMock`

A configuration block `@config { ... }` in HIR.


### `struct HirTableField`

A field within a table definition.


### `struct HirCollection`

Collection definition — a schemaless JSON document store.

Unlike `@table` (typed, columnar), a `@collection` stores documents
as JSON in a single TEXT column. Optionally, some fields may be typed
for indexing and validation while the rest remain flexible.


### `struct HirIndex`

Index definition for a table.


### `struct HirVectorIndex`

Vector index definition for a table.


### `struct HirSearchIndex`

Full-text search index definition for a table.


### `struct HirMcpTool`

MCP tool declaration — a function exposed via the Model Context Protocol.


### `struct HirAgent`

Native agent declaration.


### `struct HirMessage`

Native message declaration.


### `struct HirScheduled`

Scheduled function — runs at a fixed interval or cron schedule.


## Module: `vox-hir\src\lib.rs`

# vox-hir

High-Level Intermediate Representation for the Vox compiler.

Desugars syntax, performs name resolution via [`def_map::DefMap`],
and detects dead code. The [`lower_module`] function transforms
a `vox-ast` module into `HirModule`.


### `fn lower_module`

Lower an AST Module to a HirModule.

This process resolves names, assigns unique `DefId`s, drops redundant tokens,
and validates basic structure to produce the High-level Intermediate Representation (HIR)
which is used for type checking and code generation.


### `struct HirValidationError`

A validation diagnostic.


### `fn validate_module`

Validate structural invariants of a HirModule.
Returns a list of validation errors (empty = valid).


