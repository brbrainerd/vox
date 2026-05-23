---
title: "Reference: diagnostic ID policy"
description: "Namespaces and backward-compatibility rules for compiler vs audit diagnostic identifiers."
category: "Language Reference"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Prevents duplicate IDs across vox-compiler and vox-code-audit."
schema_type: "TechArticle"
---

# Reference: diagnostic ID policy

Machine-readable registry: [`contracts/diagnostics/registry.v1.yaml`](../../../contracts/diagnostics/registry.v1.yaml).

## Principles

1. **Append-only codes** — Once published in a stable release, a diagnostic `code` string is not repurposed; deprecate via alias messages instead.
2. **Namespaced owners**
   - **`vox-compiler`** — `E####` style, `lint.*`, `typecheck.*`, `vox/<category>/<slug>` for newer namespaces ([mesh language spine plan](../architecture/mesh-phase1-language-spine-plan-2026.md)).
   - **`vox-code-audit`** — hierarchical rule IDs such as `skeleton/untested-pub-api`, `stub/todo`, `security/hardcoded-secret/*`, `ai-laziness/*`.
3. **Disjoint prefixes** — Audit rule IDs **must not** exactly equal any compiler `code` listed in `vox_compiler::typeck::diagnostics::codes::ALL_COMPILER_DIAGNOSTIC_CODES` (enforced by [`crates/vox-compiler/tests/audit_rule_collision.rs`](../../../crates/vox-compiler/tests/audit_rule_collision.rs)). The YAML registry summarizes reserved prefixes for humans.

## Consumers

- CLI JSON diagnostics (`VoxCompilerDiagnosticPayload`) — [`crates/vox-compiler/src/typeck/diagnostics.rs`](../../../crates/vox-compiler/src/typeck/diagnostics.rs).
- `vox check --for-llm` envelope — [`crates/vox-cli/src/pipeline.rs`](../../../crates/vox-cli/src/pipeline.rs).
- LSP `Diagnostic.code` — [`crates/vox-lsp/src/lib.rs`](../../../crates/vox-lsp/src/lib.rs).

## See also

- [Diagnostic UX research](../architecture/vox-diagnostic-ux-ssot-2026.md)
- [Language rules Phase 2](../architecture/vox-language-rules-phase2-lint-extension-2026.md)
