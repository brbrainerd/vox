---
name: vox-pipeline
description: Complete Vox compiler pipeline knowledge — lexer, parser, AST, HIR, typeck, codegen stages with file paths and the workflow for adding language features
---

## Pipeline Stages

1. **Lexer** (`crates/vox-lexer`) — Uses `logos` for high-performance tokenization
2. **Parser** (`crates/vox-parser`) — Recursive descent, Rowan-based lossless syntax tree
3. **AST** (`crates/vox-ast`) — Strongly typed wrappers around the untyped CST
4. **HIR** (`crates/vox-hir`) — High-level Intermediate Representation with name resolution
5. **TypeCheck** (`crates/vox-typeck`) — Bidirectional type checking with unification-based inference
6. **CodeGen-Rust** (`crates/vox-codegen-rust`) — Emits Rust code (Axum servers)
7. **CodeGen-TS** (`crates/vox-codegen-ts`) — Emits TypeScript (React + JSX)

## Adding a Language Feature

1. Update grammar in `crates/vox-parser/src/grammar.rs`
2. Add AST node wrappers in `crates/vox-ast/src/`
3. Map AST→HIR in `crates/vox-hir/src/lower.rs`
4. Add inference rules in `crates/vox-typeck/src/check.rs`
5. Add emission in both `crates/vox-codegen-rust/src/emit_expr.rs` and `crates/vox-codegen-ts/src/jsx.rs`
6. Add integration test in `crates/vox-integration-tests/tests/`

## Key Rules

- Every new AST node must be handled in ALL stages (parser → HIR → typeck → both codegens)
- No `.unwrap()` in production code — use `?` or `.expect("descriptive message")`
- No `null` — use `Option[T]` or `Result`
- Scope discipline: `env.push_scope()` before and `env.pop_scope()` after binding expressions
