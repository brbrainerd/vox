---
title: "Vox language feature maturity matrix (2026)"
description: "Cross-cutting maturity table for language features: parse, HIR, typecheck, codegen, runtime, LSP, formatter, tests, and corpus exposure."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Makes partial implementations visible to prioritization and prevents overselling shipped semantics."
sort_order: 9
---

# Vox language feature maturity matrix (2026)

**Legend:** ✅ shipped / wired · 🟡 partial · ⬜ not started / stub · **?** needs verification in tree.

Matrix is **aspirationally complete**; tighten cells with crate owners over time. Sources: [durability-runtime-audit-2026.md](durability-runtime-audit-2026.md), [gui-native-roadmap-status-2026.md](gui-native-roadmap-status-2026.md), mesh language spine plans.

| Feature | Parse | HIR | Typecheck | Codegen | Runtime | LSP | Formatter | Golden / integration tests | Corpus / MENS lane |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `fn` / components / routes | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 | 🟡 | ✅ | 🟡 |
| `actor` / `workflow` / `activity` keywords | ✅ | ✅ | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 |
| `@durable` / `@scheduled` | ✅ | 🟡 | 🟡 | ⬜ | ⬜ | 🟡 | ? | 🟡 | ? |
| `@endpoint` HTTP stack | ✅ | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | ? |
| Effects (`@uses`, Phase 5) | 🟡 | 🟡 | 🟡 | 🟡 | ⬜ | ? | ? | ? | ? |
| Mesh: `@remote`, `DurablePromise` | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | ? | ? | ? | ? |
| MENS decorators (`@inference`, etc.) | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | ? | ? | 🟡 | 🟡 |
| CLI `--for-llm` diagnostic envelope | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ | ⬜ |
| Compiler ↔ LSP diagnostic snapshots | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ✅ | ⬜ |
| Formatter AST round-trip (goldens) | 🟡 | 🟡 | 🟡 | ⬜ | ⬜ | ⬜ | 🟡 | 🟡 | ⬜ |

## How to update

1. Pick a feature row; cite file:line or test name in a PR note.
2. If runtime is ⬜ but parse is ✅, add a warning to user-facing docs — See durability audit pattern.
3. Link new mesh/language tasks back to [mesh-phase1-language-spine-plan-2026.md](mesh-phase1-language-spine-plan-2026.md).
4. Follow [test-first policy](../../../AGENTS.md) when promoting a cell from ⬜ → ✅.

## Related

- [Vox compiler architecture (research)](vox-compiler-architecture-research-2026.md)
- [vox-lsp-capabilities-ssot-2026.md](vox-lsp-capabilities-ssot-2026.md)
