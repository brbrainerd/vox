# vox-hir

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
