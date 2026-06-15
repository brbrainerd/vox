---
title: vox-gui Surface Map (graphify, 2026-06-14)
description: Complete edge map of the vox-gui front-end and its Tauri IPC boundary — surfaces, layout shell, shared primitives, lib logic, and the 38 Rust command modules — derived from a graphify AST+semantic extraction. The basis for gap-analysis against the GUI design principles.
category: "Architecture SSOTs"
---

# vox-gui Surface Map (graphify, 2026-06-14)

> **What this is.** A complete map of the vox-gui GUI surface (TypeScript front end + Tauri IPC boundary), built by graphify over 189 files (153 TS in `ui/src` + 36 Rust command modules in `src/`). **2,176 nodes · 4,154 edges · 229 communities** (99% EXTRACTED, 1% INFERRED). Outputs live in `crates/vox-gui/graphify-out/` (`graph.html` interactive, `graph.json`, `GRAPH_REPORT.md`). This is the substrate for the next step: scoring the surface against [`gui-frontend-design-principles-2026-06-14.md`](gui-frontend-design-principles-2026-06-14.md).
>
> **Note:** the root `graphify-out/` is the whole-repo (5977-node) graph; this GUI-scoped graph is separate, under `crates/vox-gui/graphify-out/`.

## The architecture in one picture

The graph resolves into a clean **four-layer** topology:

```
                         ┌─────────────────────────────────────┐
   FRONT END (ui/src)    │  App shell  →  Layout/Dock  →  ~30   │
   React + TS            │  Surfaces  +  UI primitives  +  lib  │
                         └───────────────────┬─────────────────┘
                                             │  invoke(...)  (single chokepoint)
                                  ┌──────────▼──────────┐
   IPC BOUNDARY                   │   VoxTransport (TS)  │   ← god node, deg 30
   (the trust boundary)          │   transport.ts       │
                                  └──────────┬──────────┘
                                             │  Tauri IPC
                         ┌───────────────────▼─────────────────┐
   RUST CORE (src/)      │  38 command modules (#[command] fns) │
                         │  browser, chat, models, scientia…    │
                         └──────────────────────────────────────┘
```

**Key structural finding:** the front end reaches Rust through **one TS hub — `VoxTransport`** (`ui/src/transport.ts`, god node at degree 30). This is the single most important fact for the design-principles audit: the IPC trust boundary (Principles #317–319) is architecturally real and centralized, not scattered `invoke()` calls. Whether every surface actually routes through it (vs. calling `invoke` directly) is the first thing the gap-analysis must check.

## God nodes (core abstractions / hubs)

| Rank | Node | Edges | Role |
|------|------|-------|------|
| 1 | `React` | 100 | UI framework — touches ~37 communities (expected) |
| 2 | `String` | 45 | Rust primitive (command args/returns) |
| 3 | `BrowserState` | 37 | Browser-automation shared state (Rust) |
| 4 | **`VoxTransport`** | **30** | **The IPC client hub — the trust boundary** |
| 5 | `String` (2nd) | 27 | Rust primitive |
| 6 | `Result` | 24 | Rust command return type |
| 7 | `Icon` | 23 | Shared UI primitive (icon set) |
| 8 | `Arc` | 22 | Rust shared-ownership (command state) |
| 9 | `Glass()` | 21 | Shared UI primitive (glassmorphism container) |
| 10 | `mcp_tool_call()` | 19 | MCP bridge helper |

`Icon` and `Glass()` ranking as top hubs confirms a **shared UI-primitive layer** exists (good — Principle #256). `React` bridging 37 communities is the expected framework signal (it's the "why" behind the top suggested question).

## Layer 1 — App shell, layout & navigation

| Community | What it is | Files |
|-----------|-----------|-------|
| App Shell & State Map (C11) | `App`, `mapAgent/Event/Alert/Stream` state mappers, ErrorBoundary | `App.tsx`, `main.tsx` |
| Command Palette & Spark Hooks (C9) | `CommandPalette()`, `buildPaletteItems()`, spark-window hooks | `layout/CommandPalette.tsx`, `hooks/` |
| Palette Sources (C64) | `PaletteSources`, surfaces/docs/settings indexers | `layout/paletteSources.ts` |
| Parent Surface & Subtabs (C57) | `ParentSurface()`, `SubTabs()`, `useLocalStorage` | `layout/ParentSurface.tsx`, `SubTabs.tsx` |
| Slash Commands & Router (C7) | `buildSlashEntries()`, `slashRouter`, builtin slashes | `lib/slashCommands.ts`, `lib/slashRouter.ts` |

Also present (not separately clustered): `DockShell`, `Sidebar`, `TopHud`, `surfaceComponents`, `generated/surfaceRegistry.generated.ts`.

## Layer 2 — Surfaces (~30 feature views)

Each surface is its own community, typically a `*View.tsx` + helper `.ts` + tests. Grouped by domain:

- **Agents/Orchestration:** Dashboard (C18), Agent Flow Graph (C10), Flow & Matrix (C29), Console Core (C61) + Agent Strip/Tab (C38) + Input Editor (C63) + Terminal Tab (C52) + Discovery Rail (C66) + OSC633 (C23) + A2A (C49 IPC).
- **Chat:** Chat & Approvals Surface (C27), Chat Correlation (C26), Chat Reducer & Session Store (C67), Approvals (C39).
- **Scientia/Research:** Scientia Dashboard (C71), Claims & Command Cards (C40), Discovery Review (C16/C53) + Argv (C45) + Inbox (C50), Archive (C30), Cost Rollup (C24), Novelty Evidence (C74), Pipeline Timeline (C34), Research Actions (C65), Publications.
- **Config/Ops:** Settings (C31), Priority Chain Editor (C58), Policies (C14), Budget Config (C42), Coverage (C73), Memory, Mesh (C70), Models, Repository Isolation (C17).
- **Misc surfaces:** Browser (C21), Gamify (C35), Loquela voice (C4), Skills & Plugins (C37), Search Helpers (C48), Tasks (C8), Catalog/Command Catalog Form (C54), Runs.

## Layer 3 — Shared primitives & lib logic

- **UI primitives** (`components/ui/`): `Glass` (god node), `Icon` (god node), `Panel`, `Pill`, `StateChip`, `EmptyState`, `Backdrop`, `Sparkline`, `Toasts`, `ErrorBoundary`. → `cn`/`clsx`/`tailwind-merge` utility cluster (C91).
- **Theme & tokens:** `lib/theme.ts` (`applyTheme`, `normalizeTheme`, `ThemeId`) + Console Bridge (C69); `styles/tokens.ts`, `index.css`, `styles/dockview-vox.css`.
- **lib logic:** chat correlation, pipeline, navigation, search controller, session chat store, ludus, mcpToolResult, ids, consoleBridge — each unit-tested (`.test.ts` present throughout).
- **MCP plumbing:** `mcpToolResult` / `parseMcpToolText` / `unwrapMcpEnvelope` cluster (C92).

## Layer 4 — Tauri IPC boundary (Rust, `src/commands/`)

The transport hub `VoxTransport` (C36) fans out to **38 command modules**. Each is its own community (high cohesion — well-isolated):

| Module (community) | Cohesion | Module (community) | Cohesion |
|---|---|---|---|
| VCS Isolation (C56) | 0.37 | Browser (C0) | 0.08 |
| MCP (C72) | 0.32 | Models & Routing (C1) | 0.08 |
| Research (C68) | 0.28 | Scientia Review (C2) | 0.09 |
| Signing (C51) | 0.26 | Gamify (C3) | 0.08 |
| Memory (C47) | 0.23 | Search (C6) | 0.09 |
| LLM Settings (C60) | 0.26 | Policy/Git (C5) | 0.10 |
| Execute (C62) | 0.24 | Mic Capture (C12) | 0.10 |
| Dynamic Mapping (C55) | 0.23 | PTY/Terminal (C13) | 0.09 |
| Runs (C41) | 0.22 | Control Plane (C15) | 0.17 |
| Docs Index (C46) | 0.21 | Orchestrator (C19) | 0.13 |
| Chat IPC & DB (C28) | 0.19 | User Config (C20) | 0.14 |
| Harness (C43) | 0.19 | Action Manifest (C22) | 0.17 |
| Scientia (C33) | 0.19 | Mesh (C44) | 0.17 |
| Discovery/Catalog (C32) | 0.19 | Daemon (C59) | 0.20 |
| Console A2A (C49) | 0.17 | + build_info, preferences, identity, devlog, oratio |

**Cohesion read:** small focused command modules (signing, MCP, execute, runs) cluster tightly (0.2–0.37); the big aggregator modules (browser ~88 nodes, models, scientia-review, gamify) are *weakly* cohesive (0.08) — graphify flags these as candidates to split (see Suggested Questions). That's a maintainability signal, not a correctness one.

## Surprising connections (graphify INFERRED)

- `BrowserView` shares data with `PreviewStatus`, `PlaywrightValidateResult`, `BrowserTab`, `ControlMode` — the browser surface couples to four playwright/preview DTOs.
- `captureOutput` (TerminalTab.tsx) ↔ `Block` (osc633.ts) — terminal rendering reaches across into the OSC-633 block model.

## Import cycles (16, all self-referential)

All 16 reported cycles are 1-file self-loops in `src/commands/*.rs` (browser, daemon, execute, gamify, harness, mcp, mic, pty, models, oratio, orchestrator, policy, research, search, vcs_isolation, app_state). These are the AST extractor seeing intra-file recursion/mutual helpers — **not cross-module cycles**. No genuine architectural import cycle was found in the front end.

## Knowledge gaps (for the audit to probe)

- **589 isolated nodes** (≤1 connection): `View`, `LEGACY_VIEWS`, `KNOWN_VIEWS`, `AGENT_EVENT_LABELS`, many `Props` types. Mostly type aliases and per-component prop interfaces — expected, but `LEGACY_VIEWS`/`KNOWN_VIEWS` is worth checking for dead/legacy navigation state.
- **126 thin communities** (<3 nodes) omitted from the report — small helper clusters.

## What the audit (next step) should test against the principles

This map sets up the gap-analysis. The high-value checks the topology already suggests:

1. **IPC boundary discipline (Principles #317–329):** does *every* surface go through `VoxTransport`, or do some call `invoke()` directly, bypassing the single chokepoint? (C84 shows `voxTransport` used by PriorityChainEditor — good; confirm the rest.)
2. **Shared-primitive reuse (#256–268):** `Glass`/`Icon`/`Panel`/`StateChip` are hubs — but do all 30 surfaces use them, or do some hand-roll their own containers/buttons?
3. **Design tokens (#247–255):** `styles/tokens.ts` + `lib/theme.ts` exist — are they the single source, and do components reference semantic tokens or raw values?
4. **Loading/empty/error states (#1.1, #3.5):** `EmptyState`, `Toasts`, `StateChip`, `ErrorBoundary` exist — is every async surface wired to them, or do some leave silent waits?
5. **Accessibility primitives (#195–211):** no `aria-*` cluster surfaced — likely a gap to verify per surface.
6. **CSP set? (#330–335):** check `tauri.conf.json` / `capabilities/` — opt-in, so absence = no XSS mitigation.
7. **Module cohesion:** the 0.08-cohesion aggregators (browser, models, scientia-review, gamify) are split candidates.

> Method: graphify 0.8.x, AST (1,462 nodes) + OpenRouter semantic (1,044 nodes) merged → 2,176 nodes / 4,154 edges; Leiden communities; ~314k in / 237k out tokens. Built 2026-06-14.
