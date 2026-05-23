---
title: "Reference: documentation comments"
description: "How Vox sources relate to Markdown docs, doctests, and generated reference material."
category: "Language Reference"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Aligns contributors with AGENTS doctest policy and mdBook pipeline."
schema_type: "TechArticle"
---

# Reference: documentation comments

Vox does **not** currently define a Rust-style `///` / `///!` doc-comment syntax in the lexer. Author-facing documentation lives primarily in:

- This reference section (`docs/src/reference/`).
- Architecture and SSOT pages (`docs/src/architecture/`).
- Executable golden examples under `examples/golden/` with `@test` blocks ([test-first policy](../../../AGENTS.md)).

## Markdown and fenced code

Per root [`AGENTS.md`](../../../AGENTS.md):

- Fenced blocks tagged **`vox`** in docs may use `{{#include}}` from `examples/golden/` or mark illustrative snippets with `// vox:skip`.
- Other fenced languages are not run through the Vox parser by default.

## Building the docs book

- CLI entry: see [`reference/cli.md`](./cli.md) (`build-docs` / mdBook wiring).

## See also

- [`docs/src/contributors/documentation-governance.md`](../contributors/documentation-governance.md)
- [Style guide](./style-guide.md)
