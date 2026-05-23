---
title: "Language diagnostic drift — findings (2026)"
description: "Probe A: evidence and hypotheses for duplicate or conflicting diagnostics across vox-compiler, vox-lsp, vox-code-audit, and CI."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Documents unknown-unknown risk that agents receive conflicting fix guidance from different tools."
sort_order: 100
---

# Language diagnostic drift — findings (2026)

## Probe charter

Determine whether the **same logical mistake** can surface under different **IDs**, **severities**, or **messages** across:

- `vox-compiler` / `vox check`
- `crates/vox-lsp` document validation
- `vox-code-audit` detectors
- Optional Web IR / emit validators

## Evidence (2026-05-11)

- LSP maps typechecker diagnostics with `source: Some("vox-lsp")` while preserving string `code` from the compiler ([`vox-lsp/src/lib.rs`](../../../crates/vox-lsp/src/lib.rs)).
- Phase plans mandate stable IDs and `--for-llm` JSON ([Phase 2 plan](vox-language-rules-phase2-lint-extension-2026.md)); implementation completeness must be tracked separately.

## Hypotheses

1. **Source field drift:** Consumers keyed on `source` may dedupe incorrectly between `"vox"` and `"vox-lsp"`.
2. **Detector overlap:** TOESTUB / audit rules may duplicate compiler warnings with different naming until SSOT collapse completes.
3. **HIR-only diagnostics:** Extra HIR validation in LSP when `include_hir` may diverge from default CLI flags.

## Recommended next steps

1. Add a small **golden fixture** set: one `.vox` file per canonical diagnostic; snapshot CLI JSON vs LSP payload.
2. Document **precedence rules** in [vox-diagnostic-ux-ssot-2026.md](vox-diagnostic-ux-ssot-2026.md) once measurements exist.
3. Optionally unify `source` string or add explicit `tool` field in `--for-llm` output (future RFC).

## Related

- [vox-diagnostic-ux-ssot-2026.md](vox-diagnostic-ux-ssot-2026.md)
