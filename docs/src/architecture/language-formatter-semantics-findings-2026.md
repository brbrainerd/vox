---
title: "Language formatter semantics — findings (2026)"
description: "Probe C: risks where vox fmt / printer behavior interacts with diagnostics, spans, and semantic preservation."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Formatter can accidentally become an undeclared semantics gate."
sort_order: 102
---

# Language formatter semantics — findings (2026)

## Probe charter

Identify whether **formatting** can:

- Shift line/column spans expected by diagnostics or golden tests
- Reject programs that still parse pre-format
- Hide issues by pretty-printing invalid constructs into valid-looking shapes (unlikely but worth guarding)

## Evidence (2026-05-11)

- [`reference/cli.md`](../reference/cli.md) documents `vox fmt` behavior and failure modes — treat CLI prose as user-facing canon pending deeper audit.
- LSP formatting is often delegated or partial relative to CLI — parity risk documented in [vox-lsp-capabilities-ssot-2026.md](vox-lsp-capabilities-ssot-2026.md).

## Hypotheses

1. **Span instability:** Multi-edit workflows (IDE format-on-save + compiler) may reorder diagnostics if spans are not byte-stable.
2. **Second parser gate:** Fail-closed fmt encourages treating the printer as part of developer workflow — needs explicit policy vs `vox check`.

## Next steps

1. Add tests: format → re-parse → identical AST snapshot for golden files subset.
2. Document explicit policy in [vox-diagnostic-ux-ssot-2026.md](vox-diagnostic-ux-ssot-2026.md): whether fmt is required before CI merge.

## Related

- [vox-language-rules-phase2-lint-extension-2026.md](vox-language-rules-phase2-lint-extension-2026.md)
