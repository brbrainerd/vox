---
title: "ADR 024 — Dashboard as local Axum-served SPA"
description: "Decision record for migrating the Vox dashboard from a VS Code webview to a standalone Axum-served SPA."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-04-23"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 024 — Dashboard as local Axum-served SPA

**Status**: Accepted  
**Date**: 2026-04-23

---

## Context

The orchestration dashboard was originally bound to the `vox-vscode` extension as a webview. However, as the orchestration surface grew (now featuring 247 MCP tools in the control surface and previously containing 237 `vscode.*` API references across 19 files), the coupling to VS Code became restrictive. We needed a dashboard that was editor-agnostic, easily iterative, and aligned with our existing Vite/TanStack commitment from [ADR 010](010-tanstack-web-spine.md). We considered keeping it in the VS Code webview, or moving to Tauri, Electron, a native Rust GUI, or a local Axum-served SPA.

---

## Decision

1. **Standalone Crate**: `crates/vox-dashboard` is the canonical home for the orchestration UI.
2. **Axum Integration**: It is mounted into `http_gateway` under `#[cfg(feature = "dashboard")]`, served at `/dashboard` on the same origin as `/v1/*`.
3. **Asset Embedding**: Assets are compile-time embedded via `include_dir!`.
4. **CLI Entry Point**: `vox dashboard` is the CLI entry point; an optional `--app` flag wraps it in Chromium `--app=` mode.

---

## Rejected alternatives

- **Tauri**: Adds >2 min build times which is hostile to dev iteration and introduces WebKit/WebView2 library dependencies.
- **Electron**: Reintroduces a heavy Node runtime dependency which contradicts our Rust-first direction.
- **Bundled VS Code (Browser)**: Carries significant licensing and payload size overhead.
- **Native Rust GUI (egui/Iced)**: Offers no React component reuse, which would mean discarding the rich ecosystem currently leveraged (Radix UI, Framer Motion, xyflow).

---

## Consequences

- The `vox-vscode` extension shrinks to primarily the LSP, inline tools, and an "open dashboard" command.
- LSP-capable editors (Neovim, Helix, Zed, IntelliJ) get full language and orchestration support for free.
- Any browser can access the dashboard.
- There is no runtime-level JavaScript dependency required to run the dashboard host.

---

## References

- [ADR 010 — TanStack as the Vox web spine](010-tanstack-web-spine.md)
- [ADR 012 — Internal web IR strategy](012-internal-web-ir-strategy.md)
- [Dashboard Migration Research (2026)](../architecture/dashboard-migration-research-2026.md)
