# Vox Dashboard — Claude Design Port (Implementation Plan)

> **Phase 5 dependency.** Sections describing React-component capitalized imports (e.g. `CodeEditor(path=active_path)` calling into a React-authored TSX file) depend on Phase 5 of [external-frontend-interop-plan-2026](../../../src/architecture/external-frontend-interop-plan-2026.md), which is in-plan as of 2026-05-08. Until Phase 5 lands, those surfaces require a hand-authored compat layer (TBD).

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the existing 4-tab Latin-named Vox dashboard with the 7-surface design spec in [`docs/src/architecture/vox-dashboard-design-brief-2026.md`](../../../src/architecture/vox-dashboard-design-brief-2026.md), implemented entirely in Vox VUV source, fully wired to the orchestrator over the existing HTTP/WebSocket gateway.

**Architecture:** Vox source files in `crates/vox-dashboard/app/src/` compile to TSX via `vox build`. TSX is bundled by Vite, served as static assets baked into the orchestrator binary, mounted at `/dashboard/`. Each surface fetches initial state via REST (`/api/v2/...`) and subscribes to live updates via WebSocket (`/v1/ws`). Settings persist via `SettingsState` (file-based flat JSON — see Section 11 of the brief for the namespace conventions). VoxDB is used by the orchestrator for run history and build timelines; settings are deliberately kept out of VoxDB.

**Tech Stack:** Vox + VUV view-call syntax · Rust orchestrator (Axum) · React 19 + Vite (bundler only — Vox source is the authoring surface, no hand-written `.tsx` for surfaces except `CodeEditor.tsx` in Phase 5.3, which is a Phase 5 dependency) · file-based `SettingsState` · WebSocket `/v1/ws` for live data · SQLite via VoxDB for orchestrator-owned data (run history, build timelines, cmdk recents — NOT dashboard settings).

---

## What changed from the original (JSX-form) draft

| Original assumption | Reality on this branch | Impact |
|---|---|---|
| JSX angle-bracket syntax is the authoring surface | JSX retired in VUV-9; `<` in expression position is a parse error | All surface code in VUV view-call form throughout |
| SVG needs a React interop wrapper or allowlist | SVG passthrough works natively — fixed by `01702849b`, smoke-tested by `d7a88f975` and `7dcdcf6c5` | No SVG-interop task; all icons and topology authored directly in VUV |
| Settings backed by VoxDB | `SettingsState` is file-based flat JSON (`7923d2154`) | Phase 0 settings task is done; Phase 8 UI wires to existing SettingsState API |
| Phase 0 includes SVG aliasing task | Snake_case → camelCase aliasing was already in `compat.rs` for all SVG attrs | Removed from Phase 0 task list |
| Phase 0 includes fragment parser task | Fragments land as part of VUV progressive grammar work (separate track); dashboard surfaces do not require them | Not a Phase 0 blocker; removed |
| Phase 0 includes VoxDB settings store | SettingsState landed in `7923d2154` | Phase 0 settings done; `put_token_mask` and namespace conventions are ground-truth |
| Phase 0 includes `/api/v2` namespace | `/api/v2` + `ok`/`ok_page`/`err` envelope helpers landed in `86f7a5ccf` | Done |
| Phase 0 includes event-bus variants | `BuildStage`, `ThroughputTick`, `CostTick`, `FileDiagChanged`, `MeshTopologyChanged` landed in `b16f23fb3` | Done |

---

## Scope decomposition

| Phase | Subsystem | Shippable outcome | Status |
|---|---|---|---|
| 0 | Compiler + backend skeleton | SVG passthrough, `/api/v2`, SettingsState, event variants | **DONE (Assets Landed 2026-05-12)** |
| 1 | Chrome & composites | Shell, TopBar, LeftRail, StatusBar, icon set, composite primitives | To do |
| 2 | Mesh surface | Live topology + inspector + activity strip | To do |
| 3 | Runs surface | Run history table + drawer + live-tail | To do |
| 4 | Models surface | Registry + cost horizon + routing panel | To do |
| 5 | Code surface | File tree + editor + diagnostics | To do |
| 6 | Forge surface | Pipeline + time-travel wired to build events | To do |
| 7 | Speak surface | Thread list + chat + tool-call cards + streaming | To do |
| 8 | Settings surface | Identity, tokens, budget, telemetry wired to SettingsState | To do |
| 9 | Command palette | ⌘K overlay — actions / files / surfaces | To do |
| 10 | Polish | Watch script, legacy deletion, E2E, architecture-index regen | To do |

---

## File structure

### New Vox source (everything in VUV form)

```
crates/vox-dashboard/app/src/
├── app.vox                          REWRITE — shell + 7 routes
├── lib/
│   ├── tokens.vox                   NEW — color/font/spacing constants
│   ├── icons.vox                    NEW — ~40 Lucide icon components (SVG via VUV passthrough)
│   ├── primitives.vox               NEW — StateChip, NodeBadge, KeyHint, Label, SectionHeading,
│   │                                       IconBtn, Btn, Toggle, Input, Codeframe, Timeline
│   ├── chrome.vox                   NEW — TopBar, LeftRail, StatusBar, Shell
│   └── transport.vox                NEW — fetch/WS helpers callable from Vox
├── surfaces/
│   ├── mesh.vox                     NEW
│   ├── runs.vox                     NEW
│   ├── models.vox                   NEW
│   ├── code.vox                     NEW
│   ├── forge.vox                    NEW
│   ├── speak.vox                    NEW
│   ├── settings.vox                 NEW
│   └── cmdk.vox                     NEW
└── tabs/                            DELETE — legacy Latin tab specs (Phase 10)
```

### Modified backend

```
crates/vox-dashboard/src/
├── api/
│   ├── mod.rs                       MODIFY — mount new route modules
│   ├── settings.rs                  DONE (7923d2154) — SettingsState + token-mask helpers
│   ├── mesh.rs                      NEW (Phase 2)
│   ├── runs.rs                      NEW (Phase 3)
│   ├── models.rs                    NEW (Phase 4)
│   ├── files.rs                     NEW (Phase 5)
│   ├── forge.rs                     NEW (Phase 6)
│   ├── speak.rs                     NEW (Phase 7)
│   └── cmdk.rs                      NEW (Phase 9)
└── events.rs                        DONE (b16f23fb3) — BuildStage, ThroughputTick, etc.
```

### Build pipeline

```
crates/vox-dashboard/
├── vite.config.ts                   MODIFY (Phase 10) — vox-watch plugin for HMR
└── scripts/
    └── vox-watch.ts                 NEW (Phase 10)
```

---

## Phase dependencies

```
Phase 0 (DONE)
   │
   └──► Phase 1 (Chrome & composites)
           │
           └──► Phase 2 (Mesh — primary)
                   │
                   ├──► Phase 3 (Runs)
                   ├──► Phase 4 (Models)
                   ├──► Phase 5 (Code)
                   ├──► Phase 6 (Forge)
                   ├──► Phase 7 (Speak)
                   ├──► Phase 8 (Settings UI)
                   └──► Phase 9 (Command Palette)
                               │
                               └──► Phase 10 (Polish + legacy deletion)
```

---

## Visual & data conventions

### Token map (`crates/vox-dashboard/app/src/lib/tokens.vox`)

```vox
let bg        = "zinc.950"
let surface   = "zinc.900"
let surface2  = "zinc.800"
let surface3  = "zinc.700"
let border    = "white/6"
let border2   = "white/10"
let text_pri  = "white/86"
let text2     = "zinc.400"
let text3     = "zinc.500"
let text4     = "zinc.600"
let blue      = "blue.600"
let blue_soft = "blue.600/14"
let emerald   = "emerald.400"
let amber     = "amber.400"
let rose      = "rose.500"
```

Status → color: `running`/`ok`/`ready` → emerald; `warn`/`blocked`/`pending` → amber; `error`/`failed`/`errored` → rose; `idle`/`paused`/`done` → text4.

### WebSocket event envelope (ground-truth — 5 variants from `b16f23fb3`)

```json
{
  "v": 1,
  "id": "evt-…",
  "ts_ms": 1746316800000,
  "kind": "agent.status_changed | run.started | run.event | build.stage | throughput.tick | cost.tick | file.diag_changed | mesh.topology_changed | speak.token | settings.changed",
  "payload": {}
}
```

### REST envelope (ground-truth — `ok`/`ok_page`/`err` from `86f7a5ccf`)

```json
{ "v": 1, "data": { … } }                                 // ok()
{ "v": 1, "data": [ … ], "cursor": "base64…" }            // ok_page()
{ "v": 1, "error": { "code": "not_found", "message": "…" } }  // err()
```

### Fixture workspace (load-example fallback)

- Workspace: `aurelia-mesh`
- Orchestrators: `orchestrator-7c2a`, `orchestrator-3f1b`, `orchestrator-9d4e`
- Agents: `lex-2`, `parse-1`, `hir-3`, `typecheck-1`, `codegen-2`, `runner-1`, `inspect-1`
- Models: `sonnet-4.6`, `opus-4.7`, `haiku-4.5`, `llama-70b-local`
- Files: `src/{lex,parse,typecheck,hir,codegen,main}.vox`, `runtime/mesh.rs`

---

## Phase 0: DONE

All 10 commits landed on this branch. Retrospective:

| Commit | What landed | Notes |
|---|---|---|
| `092ee018e` | Subscript expression (Index variant) | Enables `arr[i]` in Vox expressions |
| `86f7a5ccf` | `/api/v2` namespace + `ok`/`ok_page`/`err` envelope helpers + `build_app` factory | REST ground-truth |
| `2beb85e68` | Migrate `inventory_rosetta_platform.vox` from JSX to VUV | Signals JSX fully retired |
| `7bd29d35f` | `populi::serve_with_listener` for tests | Isolation improvement |
| `d7a88f975` | SVG via VUV passthrough smoke test | First SVG green |
| `01702849b` | Fix compiler: emit children of unknown-tag view-calls as JSX children | Critical SVG fix |
| `7dcdcf6c5` | Nested SVG (mesh topology pattern) compiler test | Confirms topology authoring works |
| `6f45d9a0e` | Isolate `economy_test` from local model cache | Pre-existing test fix |
| `b16f23fb3` | Event-bus variants: `BuildStage`, `ThroughputTick`, `CostTick`, `FileDiagChanged`, `MeshTopologyChanged` | WS live-data ground-truth |
| `7923d2154` | `SettingsState` token-mask helper + namespace conventions | Settings ground-truth |

What changed from the original Phase 0 spec: the SVG aliasing task was already done (compat.rs had the attrs); the fragment task was dropped as a non-blocker; the VoxDB settings task was replaced by the lighter `SettingsState` file-based store, which is simpler and sufficient for the dashboard's needs.

---

## Phase 1: Chrome & Composites

**Goal:** New shell renders with 7 English nav items, WS-wired status bar, icon set, and all composite primitives needed by later surfaces. No Latin labels. App compiles and launches.

**Estimated time:** 3–4 days.

---

### Task 1.1: Tokens module

**File:** `crates/vox-dashboard/app/src/lib/tokens.vox` (NEW)

- [ ] Create `tokens.vox` with the token map from the Visual & data conventions section above.
- [ ] Verify `vox build app/src/lib/tokens.vox` compiles without errors.
- [ ] Commit: `feat(dashboard): tokens module`.

---

### Task 1.2: Icon components (SVG via VUV passthrough)

**File:** `crates/vox-dashboard/app/src/lib/icons.vox` (NEW)

Each icon is a component wrapping a `svg()` view-call. The passthrough pattern (verified by `d7a88f975` and `01702849b`) means no React wrapper is needed.

```vox
component PlayIcon(size: int, stroke: float) {
    view: svg(view_box="0 0 24 24", fill="none",
              stroke="currentColor", stroke_width=stroke,
              stroke_linecap="round", stroke_linejoin="round") {
        polygon(points="5 3 19 12 5 21 5 3")
    }
}

component MeshIcon(size: int, stroke: float) {
    view: svg(view_box="0 0 24 24", fill="none",
              stroke="currentColor", stroke_width=stroke,
              stroke_linecap="round", stroke_linejoin="round") {
        circle(cx=6, cy=6, r=2)
        circle(cx=18, cy=6, r=2)
        circle(cx=6, cy=18, r=2)
        circle(cx=18, cy=18, r=2)
        circle(cx=12, cy=12, r=2)
        path(d="M7.5 7.5L10.6 10.6M16.5 7.5L13.4 10.6M7.5 16.5L10.6 13.4M16.5 16.5L13.4 13.4")
    }
}
```

Required icons: `SpeakIcon`, `MeshIcon`, `ForgeIcon`, `CodeIcon`, `ModelsIcon`, `RunsIcon`, `SettingsIcon`, `SearchIcon`, `PlayIcon`, `PauseIcon`, `StopIcon`, `PlusIcon`, `XIcon`, `FilterIcon`, `RefreshIcon`, `CheckIcon`, `AlertIcon`, `FileIcon`, `FolderIcon`, `FolderOpenIcon`, `TerminalIcon`, `ChevDownIcon`, `ChevRightIcon`, `ArrowRightIcon`, `SendIcon`, `CopyIcon`, `TrashIcon`, `ExternalIcon`, `CommandIcon`, `UserIcon`, `ZapIcon`, `DatabaseIcon`, `CpuIcon`, `PinIcon`, `DownloadIcon`, `UploadIcon`, `MoreIcon`, `WrenchIcon`, `DiffIcon`.

- [ ] Add each icon to `icons.vox`.
- [ ] Smoke-test a representative sample: `PlayIcon`, `MeshIcon`, `SettingsIcon`.
- [ ] Commit: `feat(dashboard): Lucide icon set as VUV SVG components`.

---

### Task 1.3: Primitive composites — `StateChip`, `NodeBadge`, `KeyHint`, `Label`, `SectionHeading`, `IconBtn`, `Btn`, `Toggle`, `Input`, `Codeframe`

**File:** `crates/vox-dashboard/app/src/lib/primitives.vox` (NEW)

`StateChip`:
```vox
component StateChip(status: str) {
    view: row(
        items="center", gap=1,
        pad_x=2, pad_y=0,
        radius="full",
        bg=if status is "running" { "emerald.400/15" }
           else if status is "warn" or status is "blocked" { "amber.400/15" }
           else if status is "error" or status is "failed" { "rose.500/15" }
           else { "white/5" },
        raw_class="h-5 inline-flex"
    ) {
        panel(
            w=1, h=1, radius="full",
            bg=if status is "running" { "emerald.400" }
               else if status is "warn" or status is "blocked" { "amber.400" }
               else if status is "error" or status is "failed" { "rose.500" }
               else { "zinc.600" }
        )
        text(
            size="xs", font_family="mono", tracking="widest", case="upper",
            color=if status is "running" { "emerald.400" }
                  else if status is "warn" or status is "blocked" { "amber.400" }
                  else if status is "error" or status is "failed" { "rose.400" }
                  else { "zinc.500" }
        ) { status }
    }
}
```

`KeyHint`:
```vox
component KeyHint(key: str) {
    view: panel(
        border=true, border_color="white/15",
        radius="sm", pad_x=1,
        bg="white/5",
        raw_class="h-5 inline-flex items-center"
    ) {
        text(size="xs", font_family="mono", color="zinc.400") { key }
    }
}
```

`NodeBadge` (already in `app.vox` — migrate to primitives.vox and remove from app.vox).

`Codeframe`:
```vox
component Codeframe(file: str, line: int, col: int, excerpt: str, caret: str) {
    view: panel(
        bg="zinc.900", border=true, border_color="white/10",
        radius="lg", pad=3, gap=1
    ) {
        text(size="xs", font_family="mono", color="zinc.500") { file }
        text(size="xs", font_family="mono", color="white/70", leading="relaxed") { excerpt }
        text(size="xs", font_family="mono", color="rose.400") { caret }
    }
}
```

- [ ] Implement all composites listed above.
- [ ] Acceptance: each component renders in isolation without compiler errors.
- [ ] Commit: `feat(dashboard): primitive composites (StateChip, NodeBadge, KeyHint, Codeframe, …)`.

---

### Task 1.4: Transport helpers

**File:** `crates/vox-dashboard/app/src/lib/transport.vox` (NEW)

```vox
module dashboard.transport

extern fn fetch_json(url: str) -> Promise<any>
extern fn ws_connect(url: str) -> WebSocket

fn rest_get(path: str) -> Promise<any> {
    return fetch_json("/api/v2" + path)
}
```

The `extern` declarations produce typed bindings in the generated TSX that resolve to the runtime's `fetch`/WebSocket. Full WS subscribe helper wired in Task 1.9.

- [ ] Implement `transport.vox`.
- [ ] Commit: `feat(dashboard): transport helpers`.

---

### Task 1.5: `TopBar` component

**File:** `crates/vox-dashboard/app/src/lib/chrome.vox` (NEW)

```vox
component TopBar(workspace: str, on_cmdk: fn() -> ()) {
    view: row(
        h=12, border_b=true, border_color="zinc.800",
        pad_x=4, items="center", justify="between", shrink=0,
        bg="zinc.950"
    ) {
        row(items="center", gap=2) {
            text(size="xs", weight="bold", color="zinc.400",
                 tracking="widest", case="upper") { "Vox" }
            text(size="xs", color="zinc.700") { "/" }
            text(size="xs", color="zinc.300") { workspace }
        }
        button(
            raw_class="flex items-center gap-2",
            bg="zinc.900", border=true, border_color="white/10",
            radius="lg", pad_x=3, pad_y=1,
            color="zinc.500", size="sm",
            on_click=on_cmdk
        ) {
            SearchIcon(size=14, stroke=1.5)
            text(size="xs", color="zinc.600") { "⌘K" }
        }
        row(items="center", gap=3) {
            text(size="xs", color="zinc.500") { "sonnet-4.6" }
            panel(w=2, h=2, radius="full", bg="emerald.400")
        }
    }
}
```

- [ ] Implement `TopBar`.
- [ ] Commit included with Task 1.10 (Shell).

---

### Task 1.6: `LeftRail` component

**File:** `crates/vox-dashboard/app/src/lib/chrome.vox` (append)

```vox
component NavItem(label: str, surface: str, active_surface: str,
                  on_nav: fn(str) -> ()) {
    view: button(
        raw_class="flex items-center gap-3 w-full text-left",
        pad_x=3, pad_y=2, radius="lg",
        bg=if active_surface is surface { "white/8" } else { "transparent" },
        color=if active_surface is surface { "white" } else { "zinc.500" },
        on_click={on_nav(surface)}
    ) {
        // icon slot resolved by caller
        text(size="sm") { label }
    }
}

component LeftRail(active: str, on_nav: fn(str) -> ()) {
    view: column(
        w=56, border_r=true, border_color="zinc.800",
        pad=2, gap=1, shrink=0, bg="zinc.950"
    ) {
        NavItem(label="Speak",    surface="speak",    active_surface=active, on_nav=on_nav)
        NavItem(label="Mesh",     surface="mesh",     active_surface=active, on_nav=on_nav)
        NavItem(label="Forge",    surface="forge",    active_surface=active, on_nav=on_nav)
        NavItem(label="Code",     surface="code",     active_surface=active, on_nav=on_nav)
        NavItem(label="Models",   surface="models",   active_surface=active, on_nav=on_nav)
        NavItem(label="Runs",     surface="runs",     active_surface=active, on_nav=on_nav)
        panel(flex=1)
        NavItem(label="Settings", surface="settings", active_surface=active, on_nav=on_nav)
    }
}
```

- [ ] Implement `LeftRail` with icon slots for each of the 7 nav items (import from `icons.vox`).
- [ ] Acceptance: switching `active` prop changes the highlight, no Latin labels anywhere.

---

### Task 1.7: `StatusBar` wired to `/api/v2` and WebSocket

**File:** `crates/vox-dashboard/app/src/lib/chrome.vox` (append)

Subscribes to `throughput.tick`, `cost.tick`, `build.stage`, and `mesh.topology_changed` from the WS event bus. Feeds from `rest_get("/mesh/summary")` and `rest_get("/models/usage_24h")` on mount.

```vox
component StatusBar() {
    state mesh_count: int = 0
    state queue: int = 0
    state errors: int = 0
    state model: str = "—"
    state build: str = "idle"

    view: row(
        h=8, border_t=true, border_color="zinc.800",
        pad_x=4, items="center", gap=4, shrink=0,
        bg="zinc.950"
    ) {
        row(items="center", gap=2) {
            panel(w=1, h=1, radius="full",
                  bg=if errors > 0 { "rose.500" } else { "emerald.400" })
            text(size="xs", font_family="mono", color="zinc.500") { mesh_count }
            text(size="xs", color="zinc.700") { "nodes" }
        }
        text(size="xs", color="zinc.700") { "·" }
        row(items="center", gap=1) {
            text(size="xs", font_family="mono", color="zinc.500") { errors }
            text(size="xs", color="zinc.700") { "errors" }
        }
        text(size="xs", color="zinc.700") { "·" }
        text(size="xs", font_family="mono", color="zinc.500") { model }
        text(size="xs", color="zinc.700") { "·" }
        text(size="xs", font_family="mono",
             color=if build is "building" { "blue.400" } else { "zinc.600" }) { build }
    }
}
```

**Backend stubs (needed now):**

```rust
// crates/vox-dashboard/src/api/mesh.rs — stub for StatusBar initial fetch
pub async fn summary() -> Json<Value> {
    ok(json!({"nodes": 0, "queue": 0, "errors": 0,
              "default_model": "—", "build_state": "idle"}))
}
// crates/vox-dashboard/src/api/models.rs — stub
pub async fn usage_24h() -> Json<Value> {
    ok(json!({"total_usd": 0.0, "buckets_5min": []}))
}
```

Mount at `/api/v2/mesh/summary` and `/api/v2/models/usage_24h`. Real wiring comes in Phases 2 and 4.

- [ ] Implement `StatusBar`.
- [ ] Add stub routes.
- [ ] Acceptance: status bar renders with "0 nodes · 0 errors" on initial load; WS events update counts.
- [ ] Commit: `feat(dashboard): StatusBar wired to /api/v2 + WS`.

---

### Task 1.8: `Shell` wrapper

**File:** `crates/vox-dashboard/app/src/lib/chrome.vox` (append)

```vox
component Shell(active: str, on_nav: fn(str) -> (), on_cmdk: fn() -> ()) {
    view: column(min_h="screen", bg="zinc.950", color="zinc.100") {
        TopBar(workspace="aurelia-mesh", on_cmdk=on_cmdk)
        row(flex=1, overflow="hidden") {
            LeftRail(active=active, on_nav=on_nav)
            panel(flex=1, overflow="hidden") {
                route_outlet()
            }
        }
        StatusBar()
    }
}
```

- [ ] Implement `Shell`.
- [ ] Commit: `feat(dashboard): Shell, TopBar, LeftRail, StatusBar chrome`.

---

### Task 1.9: Replace `app.vox`

**File:** `crates/vox-dashboard/app/src/app.vox` (REWRITE)

```vox
import dashboard.chrome
import dashboard.surfaces.mesh
import dashboard.surfaces.runs
import dashboard.surfaces.models
import dashboard.surfaces.code
import dashboard.surfaces.forge
import dashboard.surfaces.speak
import dashboard.surfaces.settings
import dashboard.cmdk

component App() {
    state surface: str = "mesh"
    state palette_open: bool = false

    view: column(min_h="screen") {
        Shell(
            active=surface,
            on_nav={fn(s) { surface = s }},
            on_cmdk={palette_open = true}
        ) {
            if surface is "speak"    { SpeakScreen() }
            else if surface is "mesh"     { MeshScreen() }
            else if surface is "forge"    { ForgeScreen() }
            else if surface is "code"     { CodeScreen() }
            else if surface is "models"   { ModelsScreen() }
            else if surface is "runs"     { RunsScreen() }
            else if surface is "settings" { SettingsScreen() }
            else { MeshScreen() }
        }
        if palette_open {
            CommandPaletteOverlay(
                on_close={palette_open = false},
                on_navigate={fn(s) { surface = s, palette_open = false }}
            )
        }
    }
}

routes {
    "/" to App
}
```

Each `*Screen()` stub in `surfaces/*.vox` returns:

```vox
component MeshScreen() {
    view: column(flex=1, items="center", justify="center", color="zinc.600") {
        text(size="sm") { "Mesh — wiring lands in Phase 2" }
    }
}
```

- [ ] Rewrite `app.vox`.
- [ ] Create stub `surfaces/*.vox` for all 7 surfaces.
- [ ] Compile and launch: `vox build app/src/app.vox`, `pnpm dev`.
- [ ] Acceptance: 7 English nav items, top bar, status bar, tab switch works, **no Latin labels anywhere** in the rendered HTML.
- [ ] Commit: `feat(dashboard): replace 4-tab Latin shell with 7-surface English shell`.

---

## Phase 2: Mesh Surface

**Goal:** Live topology canvas, inspector with Kill/Pause/Replay, throughput activity strip — all wired to the orchestrator.

**Estimated time:** 5–6 days.

### Tasks

- **2.1 Backend — mesh API routes.** `GET /api/v2/mesh/nodes` (list all agents with status, orch parent, model), `GET /api/v2/mesh/edges`, `GET /api/v2/mesh/summary` (real implementation replacing the stub from Task 1.7), `POST /api/v2/mesh/nodes/{id}/kill`, `POST /api/v2/mesh/nodes/{id}/pause`, `POST /api/v2/mesh/nodes/{id}/replay`. File: `crates/vox-dashboard/src/api/mesh.rs`. Acceptance: integration test posts a fake topology, asserts JSON matches envelope shape.

- **2.2 Vox — `MeshSummary` KPI bar.** 6 chips: nodes / active / blocked / errors / tok/s / cost/h. Subscribes to `throughput.tick` and `cost.tick` events. File: `crates/vox-dashboard/app/src/surfaces/mesh.vox`.

- **2.3 Vox — `MeshTopology` SVG canvas.** Three layout presets (force / hierarchy / cluster) as hand-laid coordinate maps for the fixture workspace. Hexagon orchestrator nodes via a `fn hex(cx, cy, r)` helper. Circle agent nodes. Halos for running agents. Edges dashed for blocked. Click handler sets `selected` state. All SVG authored in VUV passthrough (`svg()`, `polygon()`, `circle()`, `line()`, `path()`, `defs()`, `pattern()`, `radial_gradient()`).

- **2.4 Vox — `ActivityStrip`.** 80px bottom bar. 60-bucket ring buffer maintained in component state. Subscribe to `throughput.tick`. SVG bar chart (`rect()`) in VUV.

- **2.5 Vox — `MeshInspector`.** 320px right rail. `NodeBadge` + `StateChip` + grid (orch / model / uptime / tokens / cost) + current-task text + last-5-events list + footer row (Pause / Replay / Kill buttons calling the mesh API routes from 2.1).

- **2.6 Vox — `MeshScreen`.** Wire summary + topology + inspector + activity strip. Layout selector (force/hierarchy/cluster) in a surface-local toolbar row.

- **2.7 E2E test.** `crates/vox-dashboard/tests/e2e_mesh.rs` — 12 fixture nodes render in topology, clicking `hir-3` shows "blocked" in inspector, Kill calls API.

**Acceptance:** Mesh tab shows real agents from the orchestrator; Kill removes them; throughput strip updates within ~1s.

---

## Phase 3: Runs Surface

**Estimated time:** 3–4 days.

### Tasks

- **3.1 Backend — runs API.** Define `Run` struct (`id`, `started_ms`, `duration_ms`, `orchestrator`, `top_model`, `status`, `cost_usd`, `tokens`, `root_event_id`). Routes: `GET /api/v2/runs` (cursor-paginated via `ok_page()`), `GET /api/v2/runs/{id}`, `GET /api/v2/runs/{id}/events` (recursive event tree), `DELETE /api/v2/runs/{id}` (kill). File: `crates/vox-dashboard/src/api/runs.rs`.

- **3.2 Vox — `RunsToolbar`.** Search input + status pill filters + model multi-select + time-range picker + live-tail toggle + Export button. File: `surfaces/runs.vox`.

- **3.3 Vox — `RunRow` + table header.** 7-column grid matching the `RunRow` composite from `lib/primitives.vox`. Selection state lifts to `RunsScreen`.

- **3.4 Vox — `RunDrawer`.** 420px right rail. KPI row (duration / model / cost / tokens) + recursive `EventTreeRow` list. Fetches `/api/v2/runs/{id}/events` on open.

- **3.5 Vox — `RunsScreen`.** Initial fetch `/api/v2/runs?limit=100`. Live-tail: subscribe to `run.started` WS events; prepend new rows.

- **3.6 E2E test.** Spawn 3 fixture runs, assert table populates, drawer shows event tree, live-tail prepends without full reload.

**Acceptance:** Run table reflects orchestrator history; drawer event tree expands; live tail updates.

---

## Phase 4: Models Surface

**Estimated time:** 3–4 days.

### Tasks

- **4.1 Backend — models API.** `GET /api/v2/models` (list, real implementation), `GET /api/v2/models/usage_24h` (96 × 5-min cost buckets — replaces stub from Phase 1), `POST /api/v2/models/set_default`, `POST /api/v2/models/test` (prompt + model → response). File: `crates/vox-dashboard/src/api/models.rs`.

- **4.2 Vox — `ModelCard`.** Card composite from the brief (Section 5.5 sketch). Provider name, ctx, cost in/out, p50 latency, `LoadBar`, Set default / Test / Runs actions.

- **4.3 Vox — `CostHorizon`.** SVG bar chart of `usage_24h` 96 buckets. Soft-cap line dashed amber at `budget.soft_cap_usd` (read from SettingsState via `/api/v2/settings/budget.soft_cap_usd`).

- **4.4 Vox — `RoutingPanel`.** Auto-route toggle + rule list. Toggle `PUT /api/v2/settings` `routing.auto_enabled`.

- **4.5 Vox — `ModelsScreen`.** Header (refresh + "Add provider") + `CostHorizon`/`RoutingPanel` grid + hosted card grid + local card grid.

**Acceptance:** Real models show with current load; setting default updates status bar within 1s; cost horizon reflects actual 24h spend.

---

## Phase 5: Code Surface

**Estimated time:** 5–6 days.

### Tasks

- **5.1 Backend — files API.** `GET /api/v2/files/tree?root=.` (recursive up to 1000 entries), `GET /api/v2/files/read?path=…`, `PUT /api/v2/files/write` (body: `{path, content}`), `GET /api/v2/files/diagnostics?path=…`. File: `crates/vox-dashboard/src/api/files.rs`.

- **5.2 Vox — `FileTreeNode` (recursive).** Indent based on depth. Folder expand/collapse toggle. File click emits `on_open` event.

- **5.3 Editor integration.** The editor surface uses Monaco (already a dependency if the TSX bundle includes it) or CodeMirror. This is authored as a React island — the only surface that requires one — because a full code editor is not representable as VUV primitives. File: `crates/vox-dashboard/src/components/CodeEditor.tsx`. The VUV surface file imports it via the capitalized component call pattern (`CodeEditor(path=active_path)`).

  <!-- Phase 5 dependency: requires the React-component import bridge. Until external-frontend-interop-plan-2026 §Phase 5 lands, mount manually via hand-authored TSX. -->

- **5.4 Vox — `ContextStrip`.** Right rail: recent agents that touched this file (from `mesh.topology_changed` events filtered by path), current file diagnostics as `Codeframe` rows, "Open in Forge" jump button.

- **5.5 Vox — `CodeScreen`.** File tree (240px) + tabbed editor (center) + `ContextStrip` (right). Tabs persist open files in URL query params. Ctrl/⌘+S calls `files.write`, shows a 1.2s toast.

**Acceptance:** Tree expands; file opens in editor; editing + saving persists; diagnostics show with `Codeframe` inline.

---

## Phase 6: Forge Surface

**Estimated time:** 5–6 days.

### Tasks

- **6.1 Backend — build pipeline events.** Modify the build runner to emit `BuildStage` events per `(run_id, stage, status_change)`. Persist timeline blobs in VoxDB under namespace `dashboard.forge.timeline`. File: `crates/vox-orchestrator/src/events.rs` (already has `BuildStage` variant from `b16f23fb3` — wire the publisher).

- **6.2 Backend — forge API.** `GET /api/v2/forge/timeline/{run_id}`, `GET /api/v2/forge/state_at/{run_id}?t=0.47`. File: `crates/vox-dashboard/src/api/forge.rs`.

- **6.3 Vox — `PipelineCard`.** Stage card: name, duration, `StateChip`, expandable `Codeframe` list. Subscribes to `build.stage` events filtered by stage name.

- **6.4 Vox — `PipelineView`.** 5-card horizontal flow: `Lex → Parse → HIR → Typecheck → Codegen`. The existing `PipelineStage`/`PipelineView` in `app.vox` is the right shape — rewrite as proper `PipelineCard` with real event wiring.

- **6.5 Vox — `Timeline`.** Horizontal scrubbable event strip. Three lanes (agent / model / message-diag). Event ticks colored by status. Playhead with shadow marker. Click to scrub — `on_scrub` callback updates `playhead` state. SVG authored in VUV.

- **6.6 Vox — `TimeTravelView`.** Toolbar (skip-back / play / step / diff / open-in-mesh) + `Timeline` as primary surface + right-rail state inspector (variables + pending activities). Playhead state fetches `forge.state_at`.

- **6.7 Vox — `ForgeScreen`.** Segmented toggle between Pipeline and Time Travel.

**Acceptance:** Triggering a build advances pipeline cards through `idle → running → ok/warn`; `Codeframe` shows real diagnostics; Time Travel scrubber moves through events.

---

## Phase 7: Speak Surface

**Estimated time:** 4 days.

### Tasks

- **7.1 Backend — speak API.** `GET /api/v2/speak/threads`, `GET /api/v2/speak/threads/{id}`, `POST /api/v2/speak/threads/{id}/send` (streaming response via `speak.token` WS events). Persist threads in VoxDB under namespace `dashboard.speak.threads`. File: `crates/vox-dashboard/src/api/speak.rs`.

- **7.2 Vox — `ToolCallCard`.** Collapsible panel showing tool name, args, result, duration, status chip. Status chip uses `StateChip`.

- **7.3 Vox — `ChatMessage`.** User vs assistant rendering (already in `app.vox` — migrate and extend). Streaming pulse on the last assistant message while `speak.token` events are arriving.

- **7.4 Vox — `ThreadRow`.** Compact thread list item: title (first user message truncated), timestamp, model badge.

- **7.5 Vox — `Composer`.** Model picker chip (reads active model from status bar state), tool-toggle chip, text area, send button. Send → optimistic message append → subscribe to `speak.token` → finalize on `speak.done`.

- **7.6 Vox — `SpeakScreen`.** Two-pane: thread sidebar (240px) + conversation column. Thread sidebar collapses to icon-only at narrow widths.

**Acceptance:** Sending messages streams tokens; tool calls render as collapsed cards inline; switching threads loads history without page reload.

---

## Phase 8: Settings Surface

**Estimated time:** 2–3 days.

### Tasks

- **8.1 Vox — `SettingsNav`.** Left sub-nav within the Settings surface: Identity, Workspace, Budget, Telemetry, Appearance.

- **8.2 Vox — `FieldRow`.** Label + input row used across all settings sections. Reads from and writes to `PUT /api/v2/settings/{key}`.

- **8.3 Vox — `TokenRow`.** Provider name + last-4 masked display + status chip + "Update" / "Remove" buttons. "Update" opens a modal (not an inline field) prompting for the full token, submits to `PUT /api/v2/settings/tokens/{provider}` (calls `put_token_mask` server-side), receives only `{provider, last4, status}` back. The full token never appears in the UI after submission.

- **8.4 Vox — `SettingsScreen`.** Wire all five sections. Sections: Identity (user_name, user_email), Workspace (config path display, env overrides), Budget (monthly_cap_usd, soft_cap_usd, per-model caps), Telemetry (timings, crashes, topology_snapshots toggles), Appearance (density selector).

- **8.5 Budget enforcement.** Add a dispatcher guard in `crates/vox-orchestrator/src/run.rs` that reads `budget.monthly_cap_usd` and the current-month spend before accepting a new run. Refuse with a structured error if cap is exceeded.

**Acceptance:** Setting a token masks after save and survives orchestrator restart; budget cap blocks new runs when exceeded; toggling telemetry persists immediately.

---

## Phase 9: Command Palette

**Estimated time:** 3 days.

### Tasks

- **9.1 Backend — cmdk search.** `GET /api/v2/cmdk/search?q=…` returns ranked matches across: actions (registered list with id/label/keywords), surfaces (7 nav destinations), files (prefix match via files API), recents (last 10 from VoxDB `dashboard.cmdk.recents`). File: `crates/vox-dashboard/src/api/cmdk.rs`.

- **9.2 Vox — `PaletteRow`.** Single result row: icon + label + kind badge + keyboard hint. Highlight on arrow-key selection.

- **9.3 Vox — `CommandPaletteOverlay`.** Full-viewport overlay with centered modal (max-w-lg). Search input at top, scrollable results below. Keyboard: Up/Down navigate, Enter invokes, Escape closes. Prefix filters: `>` for actions only, `@` for files only.

- **9.4 App-level keybinding.** `App` component captures `on_keydown` at the root; `⌘K` / `Ctrl-K` sets `palette_open = true`. ESC closes.

**Acceptance:** `⌘K` from any surface opens the palette; "kill run" surfaces the right action; `@lex` filters to `lex.vox` family; recent invocations persist across reloads.

---

## Phase 10: Polish

**Estimated time:** 2 days.

### Tasks

- **10.1 Vite + Vox watcher.** `crates/vox-dashboard/scripts/vox-watch.ts` (NEW): chokidar on `app/src/**/*.vox` → spawn `vox build` on change → trigger Vite HMR. Wire as a Vite plugin in `vite.config.ts`. Target: edit-to-reload < 1.5s.

- **10.2 Delete legacy artifacts.** `git rm -r crates/vox-dashboard/app/src/tabs/`. Confirm via search that no remaining imports reference the deleted files before deleting.

- **10.3 E2E smoke test.** `crates/vox-dashboard/tests/e2e_smoke.rs`: walk all 7 surfaces + `⌘K`; assert no console errors; assert no Latin labels appear in any rendered HTML; assert status bar shows real node count.

- **10.4 Architecture index regen.** Per project memory rule: **never hand-edit** `docs/src/architecture/architecture-index.md`. Run the generator:

  ```bash
  vox run scripts/regen-architecture-index.vox
  ```

  Verify the new dashboard files appear in the index.

- **10.5 Cleanup `.tmp-design/`.** The original Claude Design bundle scratch directory is in the abandoned JSX worktree — it is not present on this branch. Confirm `.gitignore` excludes `.tmp-design/` (add entry if absent).

- **10.6 Final commit + PR.** `feat(dashboard): port Claude Design output to 7-surface VUV dashboard`.

**Acceptance:** Editing any `.vox` file rebuilds and reloads within 1.5s; no Latin labels; no console errors; all 7 surfaces show real data when an orchestrator is running.

---

## Self-review

### Spec coverage check

| Requirement | Covered by |
|---|---|
| 7 surfaces, English labels | Phase 1.9 replaces `app.vox`; Phase 10.2 deletes legacy Latin tabs |
| All code in VUV | JSX syntax never appears in any task; all surface code uses view-call form. Exception: `CodeEditor.tsx` in Phase 5.3 is a hand-authored TSX React island (Phase 5 dependency — VUV cannot yet import it natively until external-frontend-interop-plan-2026 §Phase 5 lands). |
| SVG in VUV | Phase 1.2 (icons), 2.3 (topology), 4.3 (cost horizon), 6.5 (timeline) — all via passthrough |
| SettingsState (not VoxDB) | Phase 8 wires to existing `SettingsState` API from `7923d2154` |
| Live data wiring | Phase 1.7 (StatusBar WS); per-surface WS subscriptions in Phases 2–7 |
| Command palette | Phase 9 |
| Shell chrome | Phase 1.5–1.8 |
| Empty states with silhouette + sentence + action | Surface-level task notes in Phases 2–7; anti-patterns section of the brief |
| Phase 0 is done | Retrospective table with all 10 SHAs |
| No hand-editing auto-generated docs | Phase 10.4 explicitly uses the generator |

### Placeholder scan

No "TBD", "TODO", "implement later", or vague "appropriate error handling" in Phase 0 or Phase 1. Phases 2–10 use task-level density (file paths, API shapes, key VUV sketches, acceptance criteria) by design — that is the right density for an agentic work queue, not placeholders.

### Type consistency

- `BuildStage` event variant (`b16f23fb3`) → consumed by Phase 6 forge pipeline.
- `ThroughputTick` and `CostTick` (`b16f23fb3`) → consumed by Phase 1.7 (StatusBar) and Phase 2.4 (ActivityStrip).
- `MeshTopologyChanged` (`b16f23fb3`) → consumed by Phase 2.3 (topology canvas) and Phase 5.4 (ContextStrip).
- `FileDiagChanged` (`b16f23fb3`) → consumed by Phase 5.4 (ContextStrip file diagnostics).
- `SettingsState` + `put_token_mask` (`7923d2154`) → consumed by Phase 8 token entry flow.
- `ok()` / `ok_page()` / `err()` helpers (`86f7a5ccf`) → all backend routes in Phases 2–9.
- Fixture workspace names (`orchestrator-7c2a`, `sonnet-4.6`, etc.) → consistent with empty-state "load example" fixtures across all surfaces.

---

## Plan complete — execution choice

Three options:

**1. Subagent-Driven (recommended).** Use the `superpowers:subagent-driven-development` skill. Dispatch a fresh subagent per task, review between tasks. Best for the high-density Phase 1 tasks and for keeping Phase 2–9 surfaces independent.

**2. Inline Execution.** Use `superpowers:executing-plans` to execute tasks in this session with checkpoints. Works well for Phase 1; Phases 2–9 are large enough that subagents are preferred.

**3. Phased sub-plans.** Complete Phase 1, then ask for Phase 2 to be expanded into a full TDD sub-plan, and so on. Avoids spec drift on the per-surface phases where the backend and Vox code must be co-authored.
