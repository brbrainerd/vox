# vox-typeck

**Constraint-based type inference and checking for the Vox language.**

## Overview

`vox-typeck` implements a bidirectional type checking algorithm with Hindley-Milner style
type inference using union-find (UF) based constraint unification.

## Architecture

```
AST Module
    ↓
typecheck_module()
    ↓
╔═══════════════════════════════╗
║  TypeEnv                      ║  ← symbol table with scoped bindings
║  UnionFind                    ║  ← constraint solver (unification)
║  check_expr / check_stmt      ║  ← bidirectional type checking
╚═══════════════════════════════╝
    ↓
Vec<Diagnostic>  (errors + warnings)
```

### Key Components

| File | Purpose |
|------|---------|
| `check.rs` | Main type-checking logic: `check_expr`, `check_stmt`, `check_decl` |
| `env.rs` | `TypeEnv` — scoped symbol table with push/pop scope support |
| `ty.rs` | `Ty` enum — internal type representation used during checking |
| `unify.rs` | Union-Find constraint solver for type unification |
| `diagnostics.rs` | `Diagnostic` and `Severity` types for error reporting |

### Type Inference Algorithm

1. **Fresh type variables** — Unknown types are assigned fresh `Ty::TypeVar(id)` values
2. **Constraint generation** — Binary expressions, calls, and assignments generate equality constraints
3. **Unification** — The UF solver merges equivalent type variables, detecting conflicts
4. **Substitution** — After solving, type variables are replaced with their resolved types
5. **Error reporting** — Unresolvable conflicts produce `Diagnostic` errors

### Scoping

The `TypeEnv` maintains a scope stack:
- `push_scope()` — Enter a new lexical scope (function body, lambda, block)
- `pop_scope()` — Exit scope, discarding local bindings
- `define(name, ty)` — Bind a name to a type in the current scope
- `lookup(name)` — Resolve a name by searching outward through scopes

### Example Flow

```
let x = 42       →  TypeEnv.define("x", Ty::Int)
let y = x + 1    →  check x: Ty::Int, check 1: Ty::Int, unify(+): Int×Int→Int
fn f(a):          →  push_scope, define "a" as fresh TypeVar
    ret a + 1     →  unify TypeVar(a) with Int → a: Int
```

## Usage

```rust
use vox_typeck::{typecheck_module, diagnostics::Severity};

let diagnostics = typecheck_module(&ast_module, &source_content);
let errors: Vec<_> = diagnostics.iter()
    .filter(|d| d.severity == Severity::Error)
    .collect();
```
