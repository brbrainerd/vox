---
title: "ADR 030 — state_machine as SSoT for reactive UI state"
description: "Formally adopts the Vox state_machine compiler primitive as the single source of truth for all reactive browser UI state in vox-dashboard and Vox-generated applications."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-05-01"
training_eligible: true
schema_type: "TechArticle"
---
# ADR 030: `state_machine` as Single Source of Truth for reactive UI state

## Status
Accepted (2026-05-01)

## Context

The Vox compiler has supported `state_machine { … }` declarations since early 2026 (HIR node: `crates/vox-compiler/src/hir/nodes/state_machine.rs`; type-checker: `src/typeck/state_machine_check.rs`; TS lowering: `src/codegen_ts/state_machine_emit.ts`). However, `vox-dashboard` — the primary operational UI for the orchestrator — still uses hand-written React hooks and `useState` in `src/components/*.tsx`. This creates two reactive models in the same codebase, confuses the training corpus, and makes the compiler's own frontend an inconsistent example.

ADR 027 adopted Track A (Vox-native reactivity) as the canonical surface for greenfield Vox apps. This ADR extends that decision to the dashboard itself.

## Decision

1. **`state_machine` is the SSoT** for all reactive state in `crates/vox-dashboard/` and Vox-generated applications.
2. **Dashboard `.vox` sources** in `app/src/` are the authoritative definition of dashboard UI. The `app/src/generated/` directory contains only compiler output; files there must never be hand-edited.
3. **CI gate**: `scripts/check_dashboard_ssot.vox` runs in CI and fails if any `.tsx` file in `app/src/generated/` lacks a corresponding `.vox` source. This prevents hand-written reactive TSX from accumulating.
4. **Existing hand-written `src/components/*.tsx`** components are preserved until each is ported to a `.vox` source (tracked in the Phase 2 plan at `docs/superpowers/plans/2026-05-01-vox-frontend-convergence.md`).

## Consequences

- The dashboard becomes a living example of idiomatic Vox UI, improving the training corpus.
- The `state_machine` lowering pipeline (`state_machine_emit.ts`) must be kept green; any regression blocks the dashboard build.
- VS Code extension features ported to the dashboard must be expressed via `.vox` sources, not dropped in as raw React.

## Implementation touchpoints

- `crates/vox-compiler/src/hir/nodes/state_machine.rs` — HIR node
- `crates/vox-compiler/src/typeck/state_machine_check.rs` — type-checking
- `crates/vox-codegen/src/codegen_ts/state_machine_emit.ts` — lowering
- `crates/vox-dashboard/app/src/app.vox` — dashboard entry point (Tab switcher uses `state`)
- `scripts/check_dashboard_ssot.vox` — CI gate (to be created in Phase 3)

## Related

- [ADR 027 — Dual-track UI surfaces](027-dual-track-ui-surfaces.md)
- [ADR 010 — TanStack web spine](010-tanstack-web-spine.md) (governs *generated user apps*, not the dashboard)
- [ADR 031 — Deprecate vox-vscode](031-deprecate-vox-vscode.md)
- [vox-web-stack.md](../reference/vox-web-stack.md)
