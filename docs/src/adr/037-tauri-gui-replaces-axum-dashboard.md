---
title: "037 - Tauri GUI Replaces Axum Dashboard"
date: "2026-05-11"
status: "current"
category: "Architecture Decisions (ADRs)"
---

# 037 - Tauri GUI Replaces Axum Dashboard

## Context

The legacy `vox-dashboard` utilized an Axum-based SPA served over HTTP/WebSockets to display the orchestration dashboard and provide visual command surfaces. As Vox shifts toward a Single Source of Truth (SSOT) architecture, keeping the CLI commands and the dashboard UI in sync became a manual, error-prone task. Furthermore, the web-served nature of the dashboard required dealing with browser security policies, CORS, and network port management which was brittle.

## Decision

We will decommission the legacy standalone dashboard (`vox-dashboard`) and unify orchestration and UI logic into a single-source-of-truth architecture using a native Tauri 2 application (`vox-gui`).

The GUI will be automatically generated from the CLI definition. The `vox-cli` `clap` manifest generates a `CommandCatalog` JSON, which is directly consumed by the Tauri webview to render the sidebar navigation, command forms, and feature gates.

We are enforcing:
1. **No manual navigation entries in TypeScript**. All GUI menus are derived from `get_command_catalog()`.
2. **No `fetch()` or `WebSocket` in the webview**. All IPC goes through Tauri `invoke()` and events.
3. **CLI as SSOT**. The CLI is the definitive source of truth. Any new command added to `vox-cli` appears automatically in the GUI.

## Consequences

- We can drop all HTTP/WebSocket multiplexing code in the orchestrator related to the old dashboard.
- The user experience will feel significantly faster and more native.
- We must enforce that `vox-gui` does not contain business logic; it merely wraps the `vox` binary via Tauri Sidecars (`shell-path`) and fast-path IPC (`invoke()`).
