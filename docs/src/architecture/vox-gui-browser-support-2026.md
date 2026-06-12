---
title: "Vox GUI Browser Support (2026)"
description: "Architecture for embedded app preview, agent CDP live view, and Playwright validation in the vox-gui operator surface."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Documents how browser preview and automation integrate with vox-gui, orchestrator MCP, and existing chromiumoxide CDP stack."
---

# Vox GUI Browser Support (2026)

The operator **vox-gui** surface now exposes browser workflows in two tabs:

1. **Preview** — embed a localhost URL in an `<iframe>` (direct URL or Vite dev server via `OrchestratedViteGuard`).
2. **Agent live view** — mirror and control CDP browser sessions (screencast-frame first, screenshot fallback) with URL bar, back/forward/reload/stop, tab list/attach, click/scroll/key input, and a server-enforced user/agent control-mode lock.

Playwright is a **validation harness** for preview URLs, not the agent automation engine. Production browser driving remains **chromiumoxide CDP** via [`vox-plugin-browser`](../../../crates/vox-plugin-browser/).

## Layer map

| Layer | Path | Role |
| --- | --- | --- |
| GUI surface | `crates/vox-gui/ui/src/components/surfaces/Browser/` | Preview iframe, agent frame viewer, tab strip, URL/nav controls, interactive input mapping |
| Tauri commands | `crates/vox-gui/src/commands/browser.rs` | `preview_start/stop`, `browser_open/attach/list/page_info/navigate/goto/scroll/click_xy/type/key`, `browser_screenshot_frame`, `browser_validate_playwright` |
| Event stream | `vox://browser-frame`, `vox://preview-available` | Browser frame snapshots and auto-preview notifications |
| MCP tools | `crates/vox-orchestrator-mcp/src/browser_tools.rs` | `vox_browser_*` dispatched through orchestrator daemon |
| CDP engine | `crates/vox-plugin-browser/` | `BrowserAutomation` trait (chromiumoxide) |
| Dev server | `crates/vox-cli/src/frontend.rs` | `OrchestratedViteGuard` spawns `pnpm run dev:ssr-upstream` or `pnpm run dev` |
| Playwright E2E | `crates/vox-gui/ui/e2e/browser-*.spec.ts` | Surface smoke + preview URL harness |

## Preview flow

```mermaid
flowchart LR
  BrowserView[BrowserView Preview tab]
  preview_start[preview_start IPC]
  ViteGuard[OrchestratedViteGuard]
  iframe[iframe localhost URL]

  BrowserView --> preview_start --> ViteGuard
  ViteGuard --> iframe
```

- **Direct URL:** pass `url` to `preview_start` — no child process.
- **Vite app:** pass `app_dir` (relative to repo root); requires `package.json` with `dev:ssr-upstream` or `dev`.
- **Auto-preview bootstrap:** if `VOX_SSR_DEV_URL` is present when `vox-gui` starts, backend emits `vox://preview-available` and the Browser surface preloads the preview URL.

## Agent live view + control flow

```mermaid
flowchart LR
  Loquela[Loquela chat]
  User[Operator input]
  Daemon[vox-orchestrator-d]
  Mcp[vox_browser_* tools]
  Plugin[vox-plugin-browser CDP]
  FrameStream[vox://browser-frame]
  AgentTab[Agent live view + controls]

  Loquela --> Daemon --> Mcp --> Plugin
  User --> AgentTab --> Daemon
  Plugin --> FrameStream --> AgentTab
```

- GUI can also open sessions via `browser_open_session` (wraps `vox_browser_open`).
- GUI can attach to any existing agent page via `browser_list_pages` + `browser_attach_session`.
- Live frames try `vox_browser_screencast_frame` first and fall back to `vox_browser_screenshot_viewport`.
- GUI sends `actor: "human"` for interactive controls and toggles `vox_browser_set_control_lock` on mode changes (`human`, `agent`, or clear on close).

```mermaid
flowchart LR
  Click[Frame click clientX/clientY]
  Map[mapClickToViewport]
  Cmd[vox_browser_click_xy]
  Page[CDP page viewport 1280x800]

  Click --> Map --> Cmd --> Page
```

## Playwright role

- **Validate (Playwright)** runs `e2e/browser-preview.spec.ts` against `VOX_PREVIEW_URL`.
- Full GUI E2E remains `scripts/ci/gui-e2e-check.vox` + `pnpm run test:e2e`.
- Defer `playwright-rust` engine binding per [vox-native-scraping-scoping-2026-06-03.md](./vox-native-scraping-scoping-2026-06-03.md).

## Governance

- `Browser.*` / `Scrape.*` builtins require `Net` capability (`stdlib_module_capability` in `effect_check.rs`).
- Browser MCP tools are registered in `contracts/operations/catalog.v1.yaml` and surface through the GUI action manifest automatically.

## Deferred follow-up

- Continuous high-FPS screencast loop (current implementation captures one screencast frame per poll tick and acknowledges it, then falls back when unavailable).
- AX-ref / numbered-bbox hybrid tooling (`vox_browser_snapshot`, `vox_browser_click_ref`) for lower-token AI control loops.
- High-stakes HITL confirmation flow (server-side lock exists; confirmation policy layer is still pending).

## Related docs

- [vox-native-scraping-scoping-2026-06-03.md](./vox-native-scraping-scoping-2026-06-03.md) — CDP vs Playwright engine decision
- [where-things-live.md](./where-things-live.md) — crate lookup table
- [contracts/frontend/surface-ownership.v1.yaml](../../../contracts/frontend/surface-ownership.v1.yaml) — canonical GUI surfaces
