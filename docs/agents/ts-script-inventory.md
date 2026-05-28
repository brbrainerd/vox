---
title: "Ts Script Inventory"
description: "Agent support documentation for ts script inventory"
category: "contributor"
status: "current"
training_eligible: true
---
# TypeScript script inventory (expanded)

| Area | Path pattern | Disposition |
|------|----------------|-------------|
| VS Code extension | `apps/editor/vox-vscode/**/*.ts` | **Keep** (host runtime). |
| Generated app | `dist/app/**` (after build) | **Keep** (codegen output). |
| mdBook / docs theme | `docs/theme/**` | **Keep** unless moved to pure mdBook defaults. |
| OpenCode helpers | `.opencode/scripts/**` | **Keep**; prefer `vox ci` for any check that must gate merges. |

See [`docs/src/architecture/typescript-migration-boundary.md`](../src/architecture/typescript-migration-boundary.md).
