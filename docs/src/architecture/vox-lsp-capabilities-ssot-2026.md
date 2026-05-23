---
title: "Vox LSP capabilities matrix (research)"
description: "Capability inventory for crates/vox-lsp: validation path, diagnostics mapping, and parity gaps vs vox check and IDE expectations."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Prevents duplicate or divergent editor behavior; anchors LSP work to compiler facts."
sort_order: 6
---

# Vox LSP capabilities matrix (research)

**Naming:** File suffix `ssot` reflects the original plan slug; this page is **not** `B-canon` until registered in [`contracts/documentation/canonical-map.v1.yaml`](../../../contracts/documentation/canonical-map.v1.yaml). Use as the working matrix.

**See also:** [Vox compiler architecture (research)](vox-compiler-architecture-research-2026.md), [language-lsp-parity-findings-2026.md](language-lsp-parity-findings-2026.md), [web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md).

## Crate role

`crates/vox-lsp` implements a stdio JSON-RPC language server using the compiler’s **`lex` → `parse` → `typecheck_ast_module`** path (and optional HIR validation). Shared helpers convert typechecker diagnostics to LSP `Diagnostic` values (severity, code string, structured `data` for suggestions/fixes).

## Capability matrix

| Capability | Status | Implementation notes |
| --- | --- | --- |
| Publish diagnostics | **Implemented** | Driven off compiler pipeline; severity mapped from `TypeckSeverity`. |
| Diagnostic `code` | **Implemented** | Pass-through of typechecker codes where present. |
| Related fixes in `data` | **Implemented** | JSON payload with suggestions and editable replacement ranges. |
| Completions | **Partial** | `completions` module; parity with CLI/`vox check` not guaranteed without periodic audits. |
| Hover | **Partial / surface-dependent** | See Speech reference and archetype coverage gaps for stubbed areas. |
| Go to definition | **Variable** | `symbols` module; cross-file and codegen symbols need explicit tests. |
| Code actions | **Limited** | Fix payloads exist in diagnostics; full code-action lifecycle may be incomplete vs spec. |
| Formatting | **Not advertised** | `initialize` omits `documentFormattingProvider`; use CLI `vox fmt`. Future LSP formatting must add capability + handler together — see [language-formatter-semantics-findings-2026.md](language-formatter-semantics-findings-2026.md). |
| Workspace symbols | **Unknown / TBD** | Confirm in `main.rs` / capability negotiation. |

## Design constraints

- **Single source of diagnostic truth:** Prefer emitting diagnostics once in `vox-compiler` and rendering in LSP rather than reimplementing checks in the server.
- **Parity checks:** When adding a `vox check` diagnostic, add or extend an integration test that exercises the LSP path where feasible.

## Integration coverage (2026-05-11)

Automated exercises live in [`crates/vox-integration-tests/tests/lsp_capabilities_test.rs`](../../../crates/vox-integration-tests/tests/lsp_capabilities_test.rs) — asserts advertised capabilities vs absent document formatting, plus semantic tokens, completions, hover, symbols, code lenses, and quick-fix extraction.

## Next research steps

1. Generate a machine-readable capability table from `initialize` response + handler registry (or document manually until automated).
2. Align with Phase 1 SSOT collapse for completion sources ([vox-language-rules-phase1-ssot-collapse-2026.md](vox-language-rules-phase1-ssot-collapse-2026.md)).
3. Close archetype-blocked items calling out LSP stubs in [web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md).
