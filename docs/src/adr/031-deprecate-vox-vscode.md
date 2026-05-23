---
title: "ADR 031 — Deprecate vox-vscode; dashboard is the primary surface"
description: "Formally deprecates the vox-vscode VS Code extension as the primary Vox user surface. The Axum-served vox-dashboard is the replacement. vox-vscode is retained for LSP only."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-05-01"
training_eligible: false
schema_type: "TechArticle"
---
# ADR 031: Deprecate `vox-vscode`; `vox-dashboard` is the primary user surface

## Status
Accepted (2026-05-01)

## Context

`apps/editor/vox-vscode/` was historically documented as the "Single Source of Truth" for the Vox user-facing frontend (`docs/src/reference/vox-web-stack.md`, line 34, pre-2026-05-01). In practice:

1. The VS Code extension hardcodes VS Code APIs (`vscode.window`, `TextEditor`, `DiagnosticsCollection`, `WebviewPanel`). These APIs are unavailable outside the extension host and cannot be reused in a browser-native or CLI context.
2. `crates/vox-dashboard/` (started in ADR 024) has grown into a capable Axum+React SPA that covers the same orchestration surface without editor lock-in.
3. The Vox-trained MENS model is now a first-class catalog provider (`ProviderType::VoxLocal`, ADR pre-record, 2026-05-01) — inference routing belongs in the orchestrator, not the extension.
4. Browser-native MCP (via WebSocket proxy) eliminates the need for the extension as an MCP transport.

## Decision

1. **`vox-dashboard` is the primary user surface** for Vox orchestration, visualization, chat, and model management from 2026-05-01 onwards.
2. **`vox-vscode` is deprecated.** Its feature set will not grow. New capability UX, MCP behavior, and visualization must land in `crates/vox-dashboard/`.
3. **`vox-vscode` retains its LSP client** (`src/core/LspClientManager.ts`) as a convenience for VS Code users who want syntax highlighting, diagnostics, and completions. The LSP client will continue to receive bug fixes.
4. **Feature parity gate**: the extension may not be archived until `vox-dashboard` achieves parity with every feature listed in the Phase 2 plan (`docs/superpowers/plans/2026-05-01-vox-frontend-convergence.md`).

## Migration path for users

- Install `vox dashboard` and open `http://localhost:3921` in a browser.
- The VS Code extension is still available for LSP (syntax, diagnostics, completions); install it if you want editor integration.
- The `vox generate` CLI command routes through the orchestrator's VoxLocal path directly — no extension required.

## Consequences

- No new features land in `apps/editor/vox-vscode/src/` beyond LSP fixes.
- `apps/editor/vox-vscode/package.json` should be updated to mark `deprecated: true` once Phase 2 parity is achieved.
- All documentation that says "ship new behavior in `apps/editor/vox-vscode/` first" must be updated to say "ship in `vox-dashboard/`".

## Related

- [ADR 024 — Dashboard Axum SPA](024-dashboard-axum-spa.md)
- [ADR 030 — state_machine SSoT](030-state-machine-ssot.md)
- [vox-web-stack.md](../reference/vox-web-stack.md)
- Phase 2 implementation plan: `docs/superpowers/plans/2026-05-01-vox-frontend-convergence.md`
