---
title: "Vox Dashboard — Design Brief for Anthropic Labs Design (2026)"
description: "Screen-by-screen spec for the Vox dashboard redesign. VUV-form code examples, primitive inventory, token map, anti-patterns."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-03"
training_eligible: false
---

# Vox Dashboard — Design Brief for Anthropic Labs Design (2026)

> **How to use this file.** Open the design tool, attach this repository (link `crates/vox-dashboard/` as the subdirectory, not the monorepo root), then paste the **Companion Prompt** at the bottom of this file as your first message. Claude Design will extract the existing design system from the codebase and apply it to the screens specified here. Iterate surface-by-surface via inline comments.
>
> All code examples in this file are in **VUV** (Vox view-call syntax). No JSX angle brackets — that syntax was retired in VUV-9 and is a parse error in the current compiler.

---

## 1. Product thesis

The Vox Dashboard is the **operator's harness** for the Vox language and its agent runtime. It is not a marketing site, not a chat wrapper, and not an IDE. It is the single GUI surface from which a developer can run, observe, and debug a mesh of orchestrators and agents; pick models (local + hosted) and route work to them; edit code and repositories the mesh is operating on; build, type-check, and inspect Vox programs through the compiler pipeline; and talk to the system in natural language. The product it most resembles in spirit: Temporal UI crossed with LangSmith crossed with a code editor. Closest visual benchmarks: Linear (information density, keyboard-first), Vercel (status semantics, run timelines), Cursor (code workspace patterns), Grafana (mesh observability). It is **not** Notion, not Figma, not a dashboard-as-art-object.

---

## 2. Audience and jobs-to-be-done

Three concrete users; design for them, not for screenshots.

**A. The orchestrator operator** (primary). Has a running mesh. Wants to: see what is executing right now, intervene on a stuck task, kill a runaway, replay a workflow, swap a model mid-run.

**B. The Vox language developer.** Editing `.vox` files. Wants to: build, see compiler errors with codeframe, jump from a HIR/typecheck error back to source, see the generated TS/Rust, run the program against the mesh.

**C. The new arrival.** First time launching the dashboard. Wants to: understand what each surface does, see one working example, and not feel like they joined a cult.

If a screen serves none of these three, cut it.

---

## 3. Critique of the current dashboard

The existing shell at `crates/vox-dashboard/app/src/app.vox` has real bones — dark zinc, Inter-style type, monospace code blocks, and a four-tab split. However it has concrete problems the redesign must fix.

**Problem 1: Latin LARP (partially addressed, not finished).** The current `AppShell` still ships tab buttons labeled `LOQUELA / IMPERIUM / RETE / FABRICA`:

```vox
// current app.vox — these must go
component LatinTabs() {
    state tab: str = "speak"
    view: row(gap=2) {
        button(raw_class="tab-btn", on_click=fn() { tab = "speak" })   { "LOQUELA" }
        button(raw_class="tab-btn", on_click=fn() { tab = "command" }) { "IMPERIUM" }
        button(raw_class="tab-btn", on_click=fn() { tab = "network" }) { "RETE" }
        button(raw_class="tab-btn", on_click=fn() { tab = "forge" })   { "FABRICA" }
    }
}
```

The route names (`speak`, `command`, `network`, `forge`) are already English internally. The surface labels shown to users are the only thing to change. **Fix:** `Speak / Mesh / Forge / Code / Models / Runs / Settings`. Latin is permitted only as a small-caps subtitle or internal route key — never as a navigation label.

**Problem 2: Empty-state desert.** Every tab opens to a "NO X DATA · Phase 2" center-floated message. The user's first impression is a series of stubs:

```vox
// current NetworkTab empty state
component EmptyMesh() {
    view: column(flex=1, items="center", justify="center", color="zinc.500", gap=4) {
        text(size="sm", weight="bold", case="upper", tracking="widest") { "NO MESH DATA" }
        text(size="xs") { "Agent graph renders here via React interop NetworkGraph (Phase 2)." }
    }
}
```

Every empty state must answer three questions: *what would be here*, *what action populates it*, *what does a populated example look like*. A silhouette, a sentence, and a primary action — not a centered label.

**Problem 3: Missing surfaces.** Four whole categories of functionality have no home: model picker / registry, code and repo workspace, identity / budget / settings, persistent run timeline. The current IMPERIUM tab is one "RUN BUILD" button — that is orchestrator control via a stub, not a real surface.

**Problem 4: No command palette.** Operator tools live and die by `⌘K`. There is none.

**Problem 5: No status bar.** No persistent surface for "mesh = 12 nodes / 2 errors / build idle / model = sonnet-4.6". The user is always one click from each fact, never zero clicks.

**Problem 6: Decoration where information should be.** The `WorkflowScrubber` has `<<` `PLAY` `>>` controls floating over a centered "NO ACTIVE WORKFLOW" message. Time-travel scrubbers are useless without a timeline; the timeline is the primary visual.

---

## 4. Information architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ Top bar:  Vox  /  workspace name         ⌘K      status  user  │
├──────┬──────────────────────────────────────────────────────────┤
│ Rail │  Active surface (Speak / Mesh / Forge / Code /           │
│      │  Models / Runs / Settings)                               │
│      │                                                          │
│      │                                                          │
├──────┴──────────────────────────────────────────────────────────┤
│ Status bar: mesh nodes · queue · errors · model · build state  │
└─────────────────────────────────────────────────────────────────┘
```

- **Left rail:** seven destinations, icon + label, collapsible to icons-only. Order: Speak, Mesh, Forge, Code, Models, Runs, Settings.
- **Top bar:** workspace switcher (left), command palette trigger (center, faux-search showing `⌘K`), run-status pill (right), user/identity (far right).
- **Status bar (always present):** mesh count · queue depth · error count · active model · build state. Click any segment to jump to that surface.
- **Command palette (`⌘K`):** primary navigation. Actions ("kill run 4f2", "switch model to opus-4.7"), files ("open `src/lex.vox`"), and surfaces ("go to mesh"). Arrow-key + Enter, no mouse required.

---

## 5. The seven surfaces

### 5.1 Speak

Two-pane: thread list (collapsible left, 240px) + conversation (right). Composer pinned at bottom with model-picker chip and tool-toggle chips. Streaming tokens render with a faint pulse; tool calls render as collapsible cards (name, args, result, duration) — not raw JSON.

VUV sketch:

```vox
component ChatMessage(role: str, content: str) {
    view: row(
        pad_x=4, pad_y=2,
        justify=if role is "user" { "end" } else { "start" }
    ) {
        panel(
            max_w=if role is "user" { "xl" } else { "2xl" },
            bg=if role is "user" { "blue.600/20" } else { "white/5" },
            border=true,
            border_color=if role is "user" { "blue.500/30" } else { "white/10" },
            radius="2xl",
            radius_br=if role is "user" { "sm" } else { "2xl" },
            pad_x=4, pad_y=3
        ) {
            text(size="xs", weight="bold", color="zinc.400",
                 case="upper", tracking="widest", mb=2) { role }
            text(size="sm", color="white/80", leading="relaxed") { content }
        }
    }
}
```

### 5.2 Mesh (was Network / RETE)

The operator's home screen. Answers: *what is running, where, on which model, costing what.*

Three coordinated regions:

- **Topology canvas** (60% of viewport). SVG force-directed graph. Node = agent; color = status (emerald running, amber blocked, rose errored, zinc-600 idle); halo = currently-streaming. Hexagon nodes for orchestrators, circle nodes for agents. Pan/zoom, click-to-focus. Legend bottom-left (the existing `MeshLegend` component has the right shape already).
- **Inspector** (right rail, 25%). When a node is selected: identity, current task, model in use, last 5 events, Kill / Pause / Replay actions.
- **Activity strip** (bottom, 80px). A horizon chart of mesh-wide token throughput over the last hour. Hovering the strip scrubs the topology backward in time.

VUV sketch of the topology container:

```vox
component MeshTopology(layout: str, selected: str) {
    view: panel(flex=1, position="relative", overflow="hidden", bg="zinc.950") {
        svg(view_box="0 0 1000 560", preserve_aspect_ratio="xMidYMid meet",
            w="full", h="full") {
            // grid background, edges, orchestrator hexagons, agent circles
            // all authored as nested view-calls — no React interop needed
        }
        MeshLegend()
    }
}
```

Empty state: a labeled silhouette of a populated mesh (agents as faint circles, edges as faint lines, legend in position) + "No orchestrators running" + "Start an orchestrator" primary action.

### 5.3 Forge (was FABRICA)

Two sub-surfaces, segmented control top-left:

- **Pipeline.** Horizontal flow: `Lex → Parse → HIR → Typecheck → Codegen`. Each stage is a card showing duration, status pill, and (when expanded) the IR or output. When a diagnostic is present, show a `Codeframe` inline (file:line:col + 3-line excerpt + caret). The existing `PipelineStage` component has the right shape; the fix is wiring it to real build events.
- **Time travel.** A real horizontal **timeline of workflow events** as the primary surface. Each event is a tick on the timeline; selecting one shows workflow state at that instant in a right rail (variables, pending activities, history). The play / pause / step controls operate on a *visible* timeline — not over empty space.

VUV sketch of the segmented control:

```vox
component ForgeScreen() {
    state panel: str = "pipeline"

    view: column(flex=1, overflow="hidden") {
        row(h=10, border_b=true, border_color="zinc.800",
            pad_x=4, items="center", shrink=0, gap=2) {
            button(
                bg=if panel is "pipeline" { "white/10" } else { "transparent" },
                color=if panel is "pipeline" { "white" } else { "zinc.500" },
                pad_x=3, pad_y=1, radius="lg", size="xs",
                on_click={panel = "pipeline"}
            ) { "Pipeline" }
            button(
                bg=if panel is "time_travel" { "white/10" } else { "transparent" },
                color=if panel is "time_travel" { "white" } else { "zinc.500" },
                pad_x=3, pad_y=1, radius="lg", size="xs",
                on_click={panel = "time_travel"}
            ) { "Time Travel" }
        }
        if panel is "pipeline" {
            PipelineView()
        } else {
            TimeTravelView()
        }
    }
}
```

### 5.4 Code (missing entirely from current dashboard)

A scoped editor surface: file tree (left, 240px), tabbed editor (center, monospace, syntax highlight for `.vox` + Rust + TS), context strip (right, showing recent agents that touched this file, current diagnostics, "open in compiler" jump).

Not a full IDE. This is the surface where an operator reads what a mesh is editing or types a one-off fix. Use Monaco or CodeMirror, dark theme, line numbers, no minimap by default. Multi-cursor and find-in-file are required; debugging is not.

```vox
component FileTreeNode(name: str, kind: str, depth: int, open: bool) {
    view: row(
        pad_l=if depth > 0 { depth * 4 } else { 2 },
        pad_y=1, pad_r=2, items="center", gap=2,
        raw_class="hover:bg-white/5 cursor-pointer"
    ) {
        if kind is "dir" {
            if open {
                FolderOpenIcon()
            } else {
                FolderIcon()
            }
        } else {
            FileIcon()
        }
        text(size="xs", color="zinc.300", font_family="mono") { name }
    }
}
```

### 5.5 Models (missing entirely)

A registry, not a dropdown. Card-grid view grouped by `Hosted` and `Local`. Each card: provider name, model name, context window, cost ($/MTok in/out), p50 latency, current load. Card actions: "Set as default", "Test prompt", "View runs".

Top of page: a small horizon chart of cost across the last 24h; a budget bar with a soft-cap line (reads from `budget.soft_cap_usd` in SettingsState).

```vox
component ModelCard(name: str, provider: str, ctx_k: int, cost_in: float, cost_out: float, latency_p50_ms: int, load_pct: int) {
    view: panel(
        bg="zinc.900", border=true, border_color="white/10",
        radius="xl", pad=5, gap=4
    ) {
        row(justify="between", items="start", mb=1) {
            column(gap=1) {
                text(size="sm", weight="bold", color="white/90") { name }
                text(size="xs", color="zinc.500") { provider }
            }
            StateChip(status="ready")
        }
        row(gap=4) {
            column(gap=0) {
                text(size="xs", color="zinc.500") { "ctx" }
                text(size="xs", font_family="mono", color="zinc.300") { ctx_k }
            }
            column(gap=0) {
                text(size="xs", color="zinc.500") { "in $/MTok" }
                text(size="xs", font_family="mono", color="zinc.300") { cost_in }
            }
            column(gap=0) {
                text(size="xs", color="zinc.500") { "p50 ms" }
                text(size="xs", font_family="mono", color="zinc.300") { latency_p50_ms }
            }
        }
        LoadBar(pct=load_pct)
        row(gap=2, mt=1) {
            button(variant="outline", size="sm") { "Set default" }
            button(variant="ghost", size="sm") { "Test" }
            button(variant="ghost", size="sm") { "Runs" }
        }
    }
}
```

### 5.6 Runs (missing entirely)

A persistent log of every orchestrator run. Table view with columns: started, duration, orchestrator, model, status, cost, tokens. Row click → drawer (full event tree). Filters across the top: status pill set, model multi-select, time range. Live-tail toggle in the corner.

```vox
component RunRow(run_id: str, started: str, duration: str, orchestrator: str, model: str, status: str, cost_usd: float, tokens: int, on_click: fn() = fn() {}) {
    view: row(
        pad_x=4, pad_y=3, border_b=true, border_color="white/5",
        items="center", gap=4, raw_class="hover:bg-white/5 cursor-pointer",
        on_click=on_click
    ) {
        StateChip(status=status)
        text(size="xs", font_family="mono", color="zinc.400") { started }
        text(size="xs", font_family="mono", color="zinc.400") { duration }
        text(size="xs", font_family="mono", color="zinc.300") { orchestrator }
        text(size="xs", font_family="mono", color="zinc.500") { model }
        text(size="xs", font_family="mono", color="zinc.400") { cost_usd }
        text(size="xs", font_family="mono", color="zinc.500") { tokens }
    }
}
```

### 5.7 Settings

Sections: Identity & API tokens (provider keys masked after entry — last 4 only), Workspace (paths, env), Budget (monthly cap, per-model caps, alerts), Telemetry (opt-in toggles), Appearance (theme, density). One section per page, left-rail sub-nav within the surface. No surprise modal flows.

---

## 6. Visual language

The codebase already has this mostly right. Preserve and tighten.

| Token | Value | Usage |
|---|---|---|
| `bg` | `#09090b` (zinc-950) | Page background |
| `surface` | `#18181b` (zinc-900) | Cards, panels, side rails |
| `surface2` | `#27272a` (zinc-800) | Elevated panels, selected rows |
| `surface3` | `#3f3f46` (zinc-700) | Hover states, input fills |
| `border` | `rgba(255,255,255,0.06)` | Default borders |
| `border2` | `rgba(255,255,255,0.10)` | Active / focused borders |
| `text` | `rgba(255,255,255,0.86)` | Body copy |
| `text2` | `#a1a1aa` (zinc-400) | Secondary labels |
| `text3` | `#71717a` (zinc-500) | Tertiary / metadata |
| `text4` | `#52525b` (zinc-600) | Empty-state captions |
| `blue` | `#2563eb` (blue-600) | Primary action only |
| `blue_soft` | `rgba(37,99,235,0.14)` | Active background tint |
| `emerald` | `#34d399` (emerald-400) | Running / ok / ready |
| `amber` | `#fbbf24` (amber-400) | Warn / blocked / pending |
| `rose` | `#f43f5e` (rose-500) | Error / failed |

**Typography.** Inter (or a near-equivalent geometric sans) for UI. JetBrains Mono / Berkeley Mono for code, identifiers, durations, counts, file paths. Headings use `weight="black" tracking="tighter"` (already present in `app.vox` and correct). All-caps + `tracking="widest"` is right for labels and section headers — cap that pattern at 12px and never use it for navigation labels longer than two words.

**Density.** Linear-tier. 32px row height baseline, 8px grid, 12–14px body, 11px metadata.

**Iconography.** Lucide outlines, 16px in chrome, 14px inline. No emoji, no decorative glyphs.

**Motion.** Sub-200ms eased transitions. Streaming = a faint pulse on the receiving surface, never a spinner over the whole pane.

---

## 7. Component inventory

### VUV primitives (already in the compiler registry)

These map directly to HTML elements + Tailwind classes via the lowering layer. Use them as the building blocks for all surfaces. They accept universal style kwargs (`pad`, `bg`, `color`, `border`, `radius`, `gap`, `flex`, etc.).

| Primitive | HTML tag | Notes |
|---|---|---|
| `stack`, `column` | `<div>` | `flex flex-col` |
| `row` | `<div>` | `flex flex-row` |
| `wrap` | `<div>` | `flex flex-wrap` |
| `text` | `<p>` | `size`, `weight` kwargs |
| `heading` | `<h1>`–`<h6>` | `level` kwarg |
| `link` | `<a>` | underline-on-hover |
| `image` | `<img>` | `src`, `alt` |
| `button` | `<button>` | `variant` (`default`/`outline`/`ghost`/`destructive`), `size` (`sm`/`lg`/`icon`) |
| `panel`, `card` | `<div>` | `surface` kwarg for token pair |
| `list`, `list_item` | `<ul>` / `<li>` | |
| `route_outlet` | `<div>` | |
| `overlay`, `toast`, `drawer`, `modal` | `<div>` | `position`, `z` kwargs |

For raw HTML elements not in the primitive set (`input`, `select`, `textarea`, `svg`, `path`, …), the lowercase + named-args rule applies and the tag passes through verbatim. This is how SVG is authored — see Section 10.

### Dashboard composites (VUV components authored on top of primitives)

These are not primitives — they are regular Vox components living in `crates/vox-dashboard/app/src/lib/`. Do not extend the primitive registry for these.

| Component | File | Notes |
|---|---|---|
| `Shell` | `chrome.vox` | Full-page wrapper: TopBar + LeftRail + surface outlet + StatusBar |
| `TopBar` | `chrome.vox` | Workspace name, ⌘K trigger, run status, user |
| `LeftRail` | `chrome.vox` | 7 nav items, icon + label, collapsible |
| `StatusBar` | `chrome.vox` | Persistent footer strip, segmented, click-through |
| `StateChip` | `primitives.vox` | Status pill — idle / running / warn / error |
| `NodeBadge` | `primitives.vox` | Agent identity + status dot (already in `app.vox`) |
| `KeyHint` | `primitives.vox` | `⌘K`-style key cap chip |
| `Label` | `primitives.vox` | All-caps 10px section header |
| `SectionHeading` | `primitives.vox` | 12px mixed-case label with optional action slot |
| `IconBtn` | `primitives.vox` | `button(size="icon")` with a Lucide icon child |
| `CommandPalette` | `cmdk.vox` | Modal overlay, fuzzy search, keyboard-first |
| `RunRow` | `surfaces/runs.vox` | Table row — status, timing, model, cost, tokens |
| `ModelCard` | `surfaces/models.vox` | Provider card — ctx, cost, latency, load |
| `Codeframe` | `primitives.vox` | File:line excerpt with caret, used in diagnostics |
| `Timeline` | `primitives.vox` | Horizontal scrubbable event strip (Forge + Mesh activity) |

---

## 8. Empty, error, and loading states

A first-class concern. Every empty surface must show:

1. **A faint silhouette** of the populated layout — not a blank pane, not a centered icon.
2. **One sentence** explaining what would be here.
3. **One primary action** that populates it, plus a secondary "load example" that loads fixture data.

Loading: skeleton rows matching the populated row's geometry — never a spinner centered in a pane. Errors: inline above the affected region with a retry button, not floating toasts.

Fixture workspace for examples (used as fallback when no real workspace is connected):

- Workspace: `aurelia-mesh`
- Orchestrators: `orchestrator-7c2a`, `orchestrator-3f1b`, `orchestrator-9d4e`
- Agents: `lex-2`, `parse-1`, `hir-3`, `typecheck-1`, `codegen-2`, `runner-1`, `inspect-1`
- Models: `sonnet-4.6`, `opus-4.7`, `haiku-4.5`, `llama-70b-local`
- Files: `src/{lex,parse,typecheck,hir,codegen,main}.vox`, `runtime/mesh.rs`

---

## 9. Anti-patterns

Do not generate any of these:

- Latin labels in user-facing chrome (`LOQUELA`, `IMPERIUM`, `RETE`, `FABRICA`). They are out of the product.
- Lorem ipsum or pseudo-Latin filler. Use realistic domain placeholders: `orchestrator-7c2a`, `sonnet-4.6`, `lex.vox:42:7`.
- Centered "NO X DATA" messages floating in empty panes without a silhouette, sentence, and action.
- Toast notifications for non-transient state.
- A sidebar nesting more than two levels deep.
- Gradients on chrome. Glassmorphism. Drop shadows on flat surfaces.
- Decorative emoji or glyphs standing in for icons or content.
- Confirmation modals for reversible actions.
- Spinners that block a surface for longer than 200ms.
- Marketing-page hero patterns (oversized centered headline + CTA) on any operator surface.
- JSX angle-bracket syntax in any code example — it is a parse error in the current compiler.

---

## 10. SVG handling — verified working

VUV passthrough handles SVG natively. When the compiler encounters a lowercase tag it does not recognise as a primitive, it emits it verbatim as an HTML element with all children preserved. This was confirmed and fixed by commit `01702849b` on this branch (`fix(compiler): emit children of unknown-tag (passthrough) view-calls as JSX children`) and smoke-tested by `d7a88f975` and `7dcdcf6c5`.

Vox source uses **snake_case** for SVG attributes, which the compiler lowers to camelCase in the React output (same mechanism as `on_click` → `onClick`). The attribute-aliasing table is in `crates/vox-codegen-ts/src/hir_emit/compat.rs::map_jsx_attr_name`.

Working SVG component pattern:

```vox
component PlayIcon() {
    view: svg(view_box="0 0 24 24", fill="none", stroke="currentColor",
              stroke_width=1.5, stroke_linecap="round", stroke_linejoin="round") {
        polygon(points="5 3 19 12 5 21 5 3")
    }
}
```

The dashboard's ~40 Lucide icons, the Mesh topology SVG, the Models cost horizon chart, and the Forge Timeline all author directly in VUV using this pattern. No React interop component is needed for SVG.

Reserved snake_case aliases already wired in `compat.rs`: `view_box` → `viewBox`, `stroke_width` → `strokeWidth`, `stroke_linecap` → `strokeLinecap`, `stroke_linejoin` → `strokeLinejoin`, `preserve_aspect_ratio` → `preserveAspectRatio`, `text_anchor` → `textAnchor`, `font_family` → `fontFamily`, `stop_color` → `stopColor`, `stop_opacity` → `stopOpacity`.

---

## 11. Settings architecture

Settings are file-based via `SettingsState` in `crates/vox-dashboard/src/api/settings.rs`. No VoxDB, no SQLite — a flat JSON file at `$VOX_CONFIG_DIR/dashboard-settings.json` (falls back to `$HOME/.vox/dashboard-settings.json`).

Namespace conventions added at commit `7923d2154`:

| Namespace prefix | Example keys | Type |
|---|---|---|
| `identity.*` | `identity.user_name`, `identity.user_email` | string |
| `tokens.<provider>.*` | `tokens.anthropic.last4`, `tokens.anthropic.status` | string |
| `budget.*` | `budget.monthly_cap_usd`, `budget.soft_cap_usd` | number |
| `telemetry.*` | `telemetry.timings`, `telemetry.crashes` | bool |
| `routing.*` | `routing.auto_enabled`, `routing.rules` | bool / JSON |
| `cmdk.*` | `cmdk.recents` | JSON array |

The `put_token_mask(provider, last4)` helper writes only the last-4 characters plus `added_ms` and `status`. **The full token is never persisted by SettingsState.** A real secrets vault is deferred; Phase 8 of the implementation plan covers wiring a safe token-entry modal that discards the full value in-process after the masked last-4 is stored.

On read, missing keys are returned as JSON `null`. Surfaces supply their own defaults.

---

## 12. Out of scope (this iteration)

Multi-tenant org switching, billing UI, public sharing, mobile breakpoints below 1024px, dedicated a11y audit pass (keyboard nav, focus rings, and WCAG contrast are expected baseline — not "out of scope" — but a separate audit sprint is deferred). Do not design these screens; do not leave placeholders for them in the IA.

---

## Companion prompt

Paste this into the design tool as your first message:

> Generate the Vox Dashboard — an operator harness for a multi-agent orchestration runtime.
>
> The repo attached contains the existing shell (`crates/vox-dashboard/`). Extract the design system from its Tailwind token usage and existing component shapes. Apply it consistently across every screen.
>
> Build seven surfaces, navigated from a left rail with a persistent top bar (workspace + ⌘K trigger + run status) and persistent status bar (mesh count · queue depth · errors · model · build state):
>
> 1. **Speak** — chat with the mesh. Two-pane: thread list (collapsible, 240px) + conversation. Tool calls render as collapsible cards inline, not raw JSON. Streaming tokens pulse faintly.
> 2. **Mesh** — SVG topology canvas (60% viewport, authored directly in VUV view-calls, no React wrapper) + inspector right-rail (25%, selected node details + Kill/Pause/Replay) + token-throughput horizon strip (80px bottom). Empty state shows a labeled silhouette of a populated mesh, not a centered icon.
> 3. **Forge** — compiler workspace. Segmented Pipeline (Lex → Parse → HIR → Typecheck → Codegen as horizontal cards; codeframes on diagnostics) and Time Travel (real horizontal timeline of events as the primary surface, right-rail state inspector at the selected tick).
> 4. **Code** — file tree (240px) + tabbed editor (dark, monospace, .vox / Rust / TS syntax) + right context strip (recent agents, file diagnostics, "open in compiler"). Not a full IDE.
> 5. **Models** — card grid of hosted + local models (provider, ctx window, $/MTok in/out, p50 latency, load bar). Top: 24h cost horizon SVG chart + budget bar at `budget.soft_cap_usd`. Card actions: Set default, Test, View runs.
> 6. **Runs** — table of every orchestrator run (started, duration, orchestrator, model, status, cost, tokens). Filters: status pills + model multi-select + time range. Live-tail toggle. Row click opens a drawer with the full event tree.
> 7. **Settings** — sectioned: Identity & API tokens (mask after entry, show last-4 only), Workspace, Budget (monthly + per-model caps), Telemetry (opt-in toggles), Appearance.
>
> **Visual language:** zinc-950 page background, zinc-900 surfaces, white/5–white/10 borders, blue-600 accent reserved for primary actions only, emerald/amber/rose status colors. Inter for UI, JetBrains Mono for code/identifiers/durations. Linear-tier density, 8px grid, 32px row baseline. Lucide outline icons 16px chrome / 14px inline. Sub-200ms transitions.
>
> **Hard constraints:** no Latin labels in user-facing chrome — use Speak / Mesh / Forge / Code / Models / Runs / Settings; no lorem ipsum — use realistic domain placeholders (`orchestrator-7c2a`, `sonnet-4.6`, `lex.vox:42:7`); no centered "NO DATA" messages — every empty state shows a silhouette + one sentence + one primary action + one "load example"; no gradients, no glass, no drop-shadows on flat surfaces; no decorative emoji; no angle-bracket JSX syntax in any code example (it is retired).
>
> Generate Mesh first (it is the operator's home screen), then Runs, then Models, then Code, then Forge, then Speak, then Settings. Provide a CommandPalette (`⌘K`) overlay that lists actions, files, and surfaces with fuzzy search and prefix filters (`>` for actions, `@` for files).
