---
title: "Reference: async and concurrency"
description: "async/await keywords, spawn, and workflow-shaped syntax vs runtime scheduling."
category: "Language Reference"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Clarifies parse-level vs runtime-level async behavior for agents."
schema_type: "TechArticle"
---

# Reference: async and concurrency

## Keywords

- `async` and `await` are recognized by the lexer ([`token.rs`](../../../crates/vox-compiler/src/lexer/token.rs)).
- **`spawn`** is a keyword used for fire-and-forget style tasks in examples (see lexer tests and GUI roadmap).

## Bare-keyword workflow shapes

- `workflow`, `activity`, and `actor` blocks are part of [Grammar Unification](../../../AGENTS.md#grammar-unification-vox-source-syntax).
- **Runtime note:** durable scheduling and journal-backed replay are **not** universally implemented; see [`durability-runtime-audit-2026.md`](../architecture/durability-runtime-audit-2026.md) and the parse-vs-runtime note in [`AGENTS.md`](../../../AGENTS.md).

## CLI vs LSP validation paths

- `vox check` runs the full compiler pipeline including pipeline-level guards.
- `vox-lsp` validation follows lex → parse → typecheck (see [`vox-lsp/src/lib.rs`](../../../crates/vox-lsp/src/lib.rs)); behavior may differ when pipeline-only diagnostics exist.

## See also

- [Syntax](./ref-syntax.md)
- [FFI and interop](./ref-ffi.md)
