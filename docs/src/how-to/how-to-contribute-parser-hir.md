---
title: "Contributing — parser and HIR"
description: "Onboarding for vox-compiler frontend, HIR lowering, and diagnostic discipline."
category: "How-To Guides"
last_updated: "2026-04-17"

schema_type: "HowTo"
---

# Contributing — parser and HIR

## Read first

- [Architecture — pipeline](../explanation/expl-architecture.md)
- [Parser ambiguity inventory](../reference/parser-ambiguity-inventory.md)
- [HIR legacy inventory](../reference/hir-legacy-inventory.md)
- [Diagnostic taxonomy](../reference/diagnostic-taxonomy.md)
- [Compiler diagnostics and Rust codegen ergonomics](../archive/research-2026-q1/compiler-diagnostics-ergonomics.md)

## Key crates

| Path | Role |
|------|------|
| `crates/vox-compiler/src/lexer` | Tokenization |
| `crates/vox-compiler/src/parser` | Recursive descent → `ast::decl::Module` |
| `crates/vox-compiler/src/hir/lower` | AST → `HirModule` |
| `crates/vox-compiler/src/hir/validate.rs` | Structural invariants |
| `crates/vox-compiler/src/typeck` | HIR typechecking |

## Diagnostic Category Discipline

When adding new errors to the parser or HIR:
- **Parse errors:** Use `ParseError` (not `miette`). This ensures errors are mapped correctly to the LSP offsets for real-time editor feedback.
- **Type/HIR errors:** Use the `Diagnostic` struct with the appropriate `DiagnosticCategory` (`typecheck`, `hir_invariant`, etc.).
- **Codegen errors:** Use `miette`.

## HIR Legacy Graduation

Not all AST nodes have been fully lowered to strict HIR representations. The `legacy_ast_nodes` field is a temporary escape hatch.
- When contributing a new language construct, **try to create a dedicated `Hir*` representation** (e.g., `HirExpr`, `HirDecl`).
- If you must use `legacy_ast_nodes`, you **must** update the `HIR legacy inventory` SSOT and include a documented graduation plan to strict HIR.

## TOESTUB Considerations

The parser and HIR modules are historically dense. Watch out for `arch/god_object` limits (500 lines).
Several files in `crates/vox-ast/src/` and `crates/vox-compiler/src/parser/` are on the near-threshold watchlist. If your PR pushes a file over 500 lines, you must refactor it into smaller submodules using `mod.rs` and `pub use`.

## Commands

```bash
cargo test -p vox-compiler
cargo test -p vox-compiler --test parser_recovery
# Validate against golden examples
cargo test -p vox-compiler --test golden_vox_examples
```

## Definition of done

- Parser / HIR changes include tests (unit or `tests/*.rs`).
- `cargo test -p vox-compiler` runs completely green.
- `vox corpus eval --mode ast examples/golden/` passes without new failures.
- New declaration kinds either get a dedicated `Hir*` vector or land in `legacy_ast_nodes` **only** with an inventory update and a graduation plan.
- The changed files pass `vox stub-check` with no god-object violations.

