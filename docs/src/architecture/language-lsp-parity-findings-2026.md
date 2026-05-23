---
title: "Language LSP parity — findings (2026)"
description: "Probe B: checklist-driven gaps between IDE features and compiler capabilities for Vox."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Surfaces stubbed or partial LSP paths called out in archetype coverage."
sort_order: 101
---

# Language LSP parity — findings (2026)

## Probe charter

Compare **what authors need** (completion, hover, goto-def, rename, code actions, diagnostics freshness) against **what `vox-lsp` negotiates and implements**.

## Evidence (2026-05-11)

- Validation path reuses lexer/parser/typechecker (+ optional HIR) — strong parity for diagnostics when the same flags apply.
- [Web App Archetype Coverage Map](web-app-archetype-coverage-2026.md) explicitly lists LSP completion/hover/goto-def gaps for several archetypes — treat as authoritative backlog input.

## Gaps (initial)

| Area | Risk |
| --- | --- |
| Cross-file symbols | Goto-def may miss generated or workspace-relative paths. |
| Completions vs SSOT builtins | Until Phase 1 codegen completes, completions may drift from typechecker manifest. |
| Code actions | Diagnostic embeds fixes; editor may not advertise full action kinds. |

## Next steps

1. Export capability list from `crates/vox-lsp/src/main.rs` initialize handler (manual table until scripted).
2. Add integration tests mirroring `crates/vox-integration-tests/tests/lsp_test.rs` patterns for each advertised capability.
3. Refresh [vox-lsp-capabilities-ssot-2026.md](vox-lsp-capabilities-ssot-2026.md) after measurements.

## Related

- [vox-lsp-capabilities-ssot-2026.md](vox-lsp-capabilities-ssot-2026.md)
- [vox-compiler-architecture-research-2026.md](vox-compiler-architecture-research-2026.md)
