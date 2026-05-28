---
title: "TypeScript boundary policy"
description: "Official documentation for TypeScript boundary policy for the Vox language. Detailed technical reference, architecture guides, and implem"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# TypeScript boundary policy

| Class | Decision | Rationale |
|-------|----------|-----------|
| **`editors/apps/editor/vox-vscode/**`** | **Keep TS** | VS Code extension host APIs are TS-first; no Rust replacement without a separate LSP bridge. |
| **Generated Vite apps (`dist/app`)** | **Keep TS/React** | Frontend output of `vox build` / `vox run`; migrate only via Vox→TS codegen. |
| **`.opencode/scripts/**`** | **Keep** per file unless a `vox ci` guard subsumes it; then **wrap** with a one-line delegate to **`vox ci …`** (or `cargo run -p vox-cli -- ci …` when `vox` is not on `PATH`). | Low ROI to rewrite ad-hoc JS; prefer SSOT in Rust for CI. |
| **Repo policy / guard scripts** | **Migrate to `vox ci`** | Done for doc inventory + SSOT + Mens matrix; wrappers must stay **thin** (see [command surface duals](../ci/command-surface-duals.md)). |

## Smoke expectations

When retaining TS utilities, add or keep a **pnpm**-based check (install + typecheck or `node --check`) in CI only if the script is product-critical; otherwise document manual verification in the script header.

## `.opencode/scripts/*` (owners: dev-tooling)

| File | Disposition |
|------|-------------|
| `check-versions.ts` | **Keep** — local toolchain probe; no CI gate. |
| `spawn-agents.ts` | **Keep** — orchestration helper. |
| `review.ts` | **Keep** — review helper. |
| `status.ts` | **Keep** — status helper. |


