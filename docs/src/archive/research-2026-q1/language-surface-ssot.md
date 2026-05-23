---
title: "Language surface SSOT (keywords, decorators, manifests)"
description: "Authoritative plan for unifying Vox language-surface metadata across LSP, MCP, docs, eval, and speech tooling."
category: "Architecture SSOTs"
status: "current"
sort_order: 0
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Key architecture constraints and definitions required for agent context"

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Language surface SSOT

## Problem

The same **keyword, decorator, and surface-syntax** information is maintained in multiple places, which causes drift and duplicate review burden:

| Consumer | Location | Role |
|----------|----------|------|
| LSP completions | `crates/vox-lsp/src/completions.rs` | Snippets + docs for editor |
| MCP introspection | `crates/vox-orchestrator/src/mcp_tools/tools/introspection_tools.rs` | `vox_language_surface`, `vox_decorator_registry` |
| Website / search | `docs/src/api/decorators.json`, `docs/src/api/keywords.json` | Structured API search |
| Eval heuristics | `crates/vox-eval/src/lib.rs` | Regex-based construct detection |
| Speech / constrained decoding | `contracts/speech-to-code/vox_grammar_artifact.json` | Machine-readable lexer hints |
| Compiler (ground truth) | `crates/vox-compiler/src/lexer/token.rs`, parser docs in `parser/mod.rs` | What the language actually accepts |

## Implemented SSOT (code)

- [`crates/vox-compiler/src/language_surface.rs`](../../../crates/vox-compiler/src/language_surface.rs) — `LSP_KEYWORD_SNIPPETS`, `LSP_DECORATOR_DOCS`, `LEXER_KEYWORDS`, `LEXER_DECORATORS`, builtin/type name slices. **Lexer-first decorators** now include `@pure`, `@scheduled`, and `@deprecated` (see [`token.rs`](../../../crates/vox-compiler/src/lexer/token.rs)); MCP merges [`MCP_ROADMAP_DECORATORS`](../../../crates/vox-orchestrator/src/mcp_tools/tools/introspection_tools.rs) for spellings not yet promoted to dedicated tokens.
- [`crates/vox-lsp/src/completions.rs`](../../../crates/vox-lsp/src/completions.rs) — reads `vox_compiler::language_surface`.
- [`crates/vox-orchestrator/src/mcp_tools/tools/introspection_tools.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/introspection_tools.rs) — merges lexer lists with `MCP_ROADMAP_DECORATORS` for agent-facing extras.
- Test [`crates/vox-compiler/tests/language_surface_ssot.rs`](../../../crates/vox-compiler/tests/language_surface_ssot.rs) — every `LSP_DECORATOR_DOCS` entry must appear in `LEXER_DECORATORS`.

## Decision: authoritative source

**Ground truth remains the compiler lexer and parser** (`vox-compiler`). Any manifest that lists keywords or decorators must either:

1. Be **generated** from compiler metadata (preferred long-term), or
2. Be **validated in CI** against a single checked-in contract under `contracts/` that is itself generated or diff-tested against the compiler.

**Recommended contract location (phased):**

- Add `contracts/language/vox-language-surface.json` (or `.yaml` + JSON Schema) as the machine-readable SSOT for **minimal** surface lists (keywords, decorator names, punctuators) used by speech and MCP.
- Generate `decorators.json` **rich** fields (descriptions, `docUrl`, codegen hints) from a merge of: generated name list + hand-authored overlay file (e.g. `contracts/language/decorator-overlays.yaml`) so editorial content stays intentional.

## Consumer map (target state)

```text
vox-compiler (lexer/parser) ──► codegen / build.rs or `vox ci` step
        │
        ├──► contracts/language/* (committed)
        ├──► docs/src/api/*.json (generated)
        ├──► vox-lsp (include! or generated module)
        ├──► vox-mcp introspection (calls into vox-compiler or includes generated JSON)
        ├──► vox-eval (optional: generate regex table from same list, or call compiler)
        └──► contracts/speech-to-code/vox_grammar_artifact.json (generated)
```

- Replacing the recursive-descent parser or `logos` lexer with external parser frameworks solely to deduplicate lists.

## Syntax Modernization (Path C)

As part of the legacy codebase retirement (OP-0179, OP-0158), surface definitions are being realigned towards **Path C** syntax (`component Name() { ... }`).
The legacy `@component fn` surface is formally deprecated and will be removed from the canonical SSOT generator once all downstream UI surfaces conform to Path C.
- Deleting `decorators.json` editorial fields without an overlay story.

## Implementation order

1. Add a **single** generator entrypoint (crate binary or `vox ci` subcommand) that emits the minimal JSON contract from `Token` / parser tables.
2. Wire **one** consumer (speech artifact or MCP) -> the generated file; keep the old file until diff is zero.
3. Migrate LSP and eval last (highest churn in snippets vs plain names).

See also: [Outbound HTTP policy](outbound-http-policy.md), [OpenAPI contract SSOT](openapi-contract-ssot.md).


