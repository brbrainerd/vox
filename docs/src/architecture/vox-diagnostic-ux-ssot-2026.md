---
title: "Vox diagnostic UX taxonomy (research)"
description: "Taxonomy for compiler and tooling diagnostics: stable IDs, severity, human vs LLM consumption, LSP mapping, and drift risks vs vox-code-audit."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Unifies diagnostic policy across compiler, LSP, CI, and MENS-facing outputs."
sort_order: 7
---

# Vox diagnostic UX taxonomy (research)

**Naming:** Planned slug `vox-diagnostic-ux-ssot-2026`; **not** `B-canon` until registered in [`contracts/documentation/canonical-map.v1.yaml`](../../../contracts/documentation/canonical-map.v1.yaml).

Parent context: [Vox Language Rules & Enforcement — Top-Level Plan (2026)](vox-language-rules-and-enforcement-plan-2026.md), Phase 2 (`--for-llm`, `LintFix`), Phase 4 (idiom telemetry hooks).

## Goals

| Consumer | Need |
| --- | --- |
| Human in IDE | Stable severity, actionable spans, optional related information. |
| LLM agent | Stable machine-readable codes, minimal repro, symmetric fix hints ([Phase 2](vox-language-rules-phase2-lint-extension-2026.md)). |
| CI | Same codes as local `vox check`; no duplicate conflicting rules without documented precedence. |

## Namespace conventions

- Prefer hierarchical IDs: `vox/<category>/<kebab-case>` for new mesh/language work ([mesh phase 1 plan](mesh-phase1-language-spine-plan-2026.md)).
- Typechecker-owned codes live in `vox-compiler`; extended detectors live in `vox-code-audit` — document collisions in [language-diagnostic-drift-findings-2026.md](language-diagnostic-drift-findings-2026.md).

## Severity ladder

| Level | Typical use |
| --- | --- |
| Error | Program rejected or unsound boundary crossed. |
| Warning | Correctness or policy risk; may become error after deprecation window. |
| Note / info | Optional style, migration nudges, education (map carefully to LSP `Hint` vs `Information`). |

## LSP mapping

The language server maps `TypeckSeverity` to LSP `DiagnosticSeverity` and attaches structured fix data ([`crates/vox-lsp/src/lib.rs`](../../../crates/vox-lsp/src/lib.rs)). Gaps to track:

- Whether every compiler diagnostic intended for authors appears in LSP validation.
- Whether `code_description` URLs (`--explain`) exist per Phase 2 plan.

## CLI envelope (`vox check --for-llm`)

**Shipped shape:** `vox check <file>.vox --for-llm` prints a single JSON object (`CheckForLlmEnvelope` in [`crates/vox-cli/src/pipeline.rs`](../../../crates/vox-cli/src/pipeline.rs)):

| Field | Meaning |
| --- | --- |
| `envelope_version` | Integer schema revision (currently `1`). |
| `file_path` | Label passed to `check_file` (usually the `.vox` path). |
| `ok` | `true` only when **zero** error-severity diagnostics are present. |
| `error_count` / `warning_count` | Aggregates over machine payloads (parse + typecheck + HIR validation). |
| `diagnostics` | Array of [`VoxCompilerDiagnosticPayload`](../../../crates/vox-compiler/src/typeck/diagnostics.rs) structs (same JSON shape as legacy `--json` diagnostic arrays, wrapped). |

Golden lock: [`crates/vox-cli/tests/check_for_llm_envelope.rs`](../../../crates/vox-cli/tests/check_for_llm_envelope.rs).

## Open questions

1. **Precedence:** When `vox-code-audit` and `vox-compiler` disagree, which wins in CI vs IDE?
2. **Formatting:** Can `vox fmt` change spans such that diagnostics shift — see formatter findings doc.
3. **Privacy:** Which diagnostic payloads are safe for default telemetry vs opt-in — [telemetry-trust-ssot.md](telemetry-trust-ssot.md).

## Related

- [vox-lsp-capabilities-ssot-2026.md](vox-lsp-capabilities-ssot-2026.md) (working matrix)
- [language-diagnostic-drift-findings-2026.md](language-diagnostic-drift-findings-2026.md)
