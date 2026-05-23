---
title: "Diagnostic taxonomy (compiler)"
description: "Categories for Vox compiler diagnostics (parse, lowering, typecheck, HIR, runtime, lint)"
category: "Language Reference"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "TechArticle"
---

# Diagnostic taxonomy

Structured diagnostics (`vox_compiler::typeck::Diagnostic`) carry a **`category`** (`DiagnosticCategory`) for filtering, metrics, and documentation. Definitions live in [`crates/vox-compiler/src/typeck/diagnostics.rs`](../../../crates/vox-compiler/src/typeck/diagnostics.rs).

| Category | When used |
|----------|-----------|
| **`parse`** | Reserved for parse-stage diagnostics when surfaced through the same struct (primary parse errors today use `ParseError` until unified). [`ParseErrorClass`](../../../crates/vox-compiler/src/parser/error.rs) includes `ReactiveComponentMember` for unknown tokens inside a Path C reactive body (stable for metrics and doc extraction). |
| **`lowering`** | AST → HIR lowering shape issues (future unified messages). |
| **`typecheck`** | Default: inference, unification, undefined names, arity, match exhaustiveness, etc. |
| **`hir_invariant`** | Structural checks from [`validate_module`](../../../crates/vox-compiler/src/hir/validate.rs) after lowering (empty names, empty route paths, …). |
| **`runtime_contract`** | Host / deploy / embedding guards (when reported via the same pipeline). |
| **`lint`** | AST-level declaration lints (`@index` / `@search_index`), hook style warnings, and policy diagnostics. Severity can be `warning` or `error` (for example, `db.Table.query(clause)` now reports a lint-category error). |

CLI JSON diagnostics (`vox check --json`, shared `pipeline`) include a `category` field per row when using the structured diagnostic path.

## Related

- [Language reference](ref-syntax.md)
- [Architecture — compiler pipeline](../explanation/expl-architecture.md)

