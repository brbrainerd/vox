# vox-ast

Strongly-typed Abstract Syntax Tree for the Vox language.

## Purpose

Provides typed wrappers around the parser's untyped Concrete Syntax Tree (CST) nodes. This is the API that downstream stages (HIR lowering, type checking) consume.

## Key Files

| File | Purpose |
|------|---------|
| `decl.rs` | `Decl` — function, type, actor, workflow, activity, table, decorator declarations |
| `expr.rs` | `Expr` — literals, binary ops, calls, match, spawn, JSX, lambda |
| `stmt.rs` | `Stmt` — let bindings, return, assignment, expression statements |
| `pattern.rs` | `Pattern` — destructuring and match patterns for ADTs |
| `types.rs` | `TypeExpr` — type annotations, generics, and type parameters |
| `span.rs` | `Span` — source location tracking (start/end offsets) |

## Design

Each AST node type is an enum with variants for every language construct. All nodes carry `Span` information for error reporting and LSP integration.

```
Module
├── Vec<Decl>
│   ├── FnDecl { name, params, return_type, body, decorators }
│   ├── TypeDecl { name, variants }
│   ├── ActorDecl { name, state, handlers }
│   ├── WorkflowDecl { name, params, body }
│   └── ...
├── Vec<Stmt>
└── Vec<Route>
```
