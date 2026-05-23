---
title: "Compiler diagnostics and Rust codegen ergonomics"
description: "Policy for miette vs custom diagnostics, error layers, and how contributors should read and troubleshoot vox check output."
category: "Architecture SSOTs"
status: "current"
sort_order: 0
last_updated: "2026-04-17"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Compiler diagnostics and Rust codegen ergonomics

This document outlines the architecture of Vox compiler errors and provides guidance for contributors on how to read and troubleshoot diagnostic output across the three error layers: LSP, CLI, and CI.

## The three error layers

As a contributor, you will encounter Vox diagnostics in three ways:

1. **LSP (Real-time):** Surfaced in your editor via `vox-lsp`. These are generated continuously as you type. They focus on parse and typecheck errors.
2. **CLI (`vox check`):** The local terminal tool. Runs the full compiler pipeline and surfaces all diagnostic categories.
3. **CI (`cargo test` + `vox ci`):** The ultimate gate. Includes TOESTUB rules and test failures alongside compiler diagnostics.

**Why error cascade happens:**
Vox is a multi-pass compiler. If the parser fails to recover from a syntax error, it produces an incomplete AST. When the incomplete AST is lowered to HIR and typechecked, you will often see a cascade of "undefined symbol" or "type mismatch" errors that are merely symptoms of the original parse failure.
**Troubleshooting rule:** Always fix the first parse error (`category: "parse"`) before chasing typecheck errors.

## Diagnostic categories

Structured diagnostics (`vox_compiler::typeck::Diagnostic`) carry a `DiagnosticCategory`. See [Diagnostic taxonomy](../reference/diagnostic-taxonomy.md) for the full SSOT.

For contributors, the primary categories you must fix are:

- **`parse`**: Syntax violations. Often block the rest of the pipeline. Fix these first.
- **`typecheck`**: Inference, unification, undefined names, and arity errors.
- **`hir_invariant`**: Structural violations (e.g., empty route paths). These are caught after lowering.
- **`lint`**: Policy violations and style warnings. Can be escalated to errors in strict environments.

## Reading `vox check --json` output

When automating tasks or debugging CI, use `vox check --json`. The output contains a JSON array of diagnostic objects.

Key fields to look for:
- `category`: Matches the taxonomy above.
- `severity`: `"error"`, `"warning"`, or `"info"`.
- `span`: The source location `[start_byte, end_byte]`.
- `message`: The human-readable error description.

Example:
```json
{
  "category": "typecheck",
  "severity": "error",
  "message": "Type mismatch: expected int, found string",
  "span": [120, 135]
}
```

## `miette` vs custom errors (Architecture)

**Current state:**

- `miette` is a dependency of `vox-compiler` and is used for **Rust codegen** failures (`codegen_rust/pipeline.rs`, `emit/mod.rs`, projection validation).
- **Parse / typecheck / HIR** use bespoke error types (`ParseError`, `Diagnostic`, `HirValidationError`) mapped to LSP in `vox-lsp`.

**Decision (near term):**

- **No forced unification** until there is bandwidth to thread `Span` ↔ `miette::SourceSpan` (including UTF-16 LSP offsets) through the full pipeline.
- **Directional preference:** when adding **new** rich user-facing errors in codegen paths, use `miette`. For LSP-facing parse/type errors, keep the existing structured diagnostics until a deliberate migration plan exists.
- **Parse errors discipline:** New parse errors must use `ParseError`, not `miette`. This ensures they are properly surfaced by the LSP.

**Rationale:** Unifying on `miette` everywhere is high-touch (CLI, MCP, tests, serde-stable diagnostics); partial adoption already delivers value on codegen.

## Rust emission: `quote` / `prettyplease`

**Current state:** Most Rust output is string emission under `crates/vox-compiler/src/codegen_rust/emit/`.

**Decision:**

- **Pilot first:** pick one hot file (e.g. a small `emit/*` module with heavy escaping) and try `quote!` for syntactic fragments; optionally run `prettyplease` on output in tests only to validate shape.
- **Not a goal:** rewriting the entire emitter to proc-macro style in one pass.

**Rationale:** `quote` reduces nested-quote bugs; full migration is a large formatting and snapshot-test churn.

## Related

- `crates/vox-compiler/src/codegen_rust/pipeline.rs`
- `crates/vox-compiler/src/parser/error.rs`
- `crates/vox-compiler/src/typeck/diagnostics.rs`
- `crates/vox-lsp/src/lib.rs` (diagnostic mapping)


