---
title: "Vox GUI UX Beautification & Build-out Plan"
description: "Comprehensive code review, bug catalog, and phased plan to take the Vox Tauri GUI from a functional 28-surface operator console to a fully built-out, accessible, design-system-driven product."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
last_updated: "2026-06-17"
training_rationale: "Canonical roadmap for Vox GUI UX work — captures current state, bugs, user-journey gaps, design-system gaps, and a phased rollout that any agent can resume."
---

# Vox GUI UX Beautification & Build-out Plan

> **Audience:** platform-gui, anyone touching `crates/vox-gui/ui/**`. Plan owner:
> GUI. Phases map to roughly 5–8 weeks of focused work; can be sliced per phase.

## 0. TL;DR

Vox GUI is a **Tauri 2 desktop shell** (React 19 + TS + Tailwind 3 + Radix +
lucide-react + dockview + xyflow + recharts + xterm). It already has a
distinctive "arcane/dark/gold" visual language: brass accent
(`#d4af37` → `212 175 55`), glass morphism, grid + radial-gradient backdrop,
mono numerics, and a full surface registry of **28 operator surfaces across 6
nav groups**. The bones are good. What it lacks is **design-system rigor, full
build-out, and bug-prevention discipline**.

This plan fixes the foundation first, then rebuilds surfaces one at a time
against a stable system. We deliver:

1. A **complete design-token system** (type, spacing, elevation, motion,
   status, focus).
2. A **promoted primitive set** (`Button`, `KPI`, `StatusPill`, `DataTable`,
   `EmptyState`, `Glass` with size variants).
3. A **bug-prevention pass** — 20+ issues that would have shipped to users,
   caught and fixed up front.
4. **User-journey hardening** for first-run, submit-task, approve, switch
   context, theme switch.
5. **Accessibility & resilience** (keyboard nav, focus traps, reduced-motion,
   high-contrast theme, touch targets, color-only-signal guard).
6. A **phased rollout** with clear go/no-go gates.

---

## 1. Current state (what we have)

### 1.1 Stack

| Layer | Choice | Notes |
|---|---|---|
| Shell | Tauri 2 | `crates/vox-gui/src/main.rs` + Tauri commands |
| UI | React 19 + TypeScript | `crates/vox-gui/ui/` |
| Styling | Tailwind 3 + Style Dictionary tokens | `tailwind.config.js`, `ui/tokens/*.json` |
| Primitives | Radix UI (Dialog, Slot, Tooltip) | Only 3 — under-used |
| Icons | lucide-react | Loaded as `Record<string, any>` map in `ui/Icons.tsx` |
| Layout | dockview 6.6.1 | `ui/src/styles/dockview-vox.css` |
| Diagrams | xyflow 12 (`@xyflow/react`) | Used by `Flow` surface (agent graph) |
| Charts | recharts 3.8 | Used by dashboard widgets |
| Terminal | xterm 5 | `Console` surface |
| State | React Query + EventBus via Tauri events | `ui/src/transport.ts`, `ui/src/hooks/useOrchestratorStatus.ts` |
| Tests | vitest + Testing Library + Playwright | Coverage skews toward unit; visual smoke is light |

### 1.2 Visual language

* **Themes (4)**: `arcane` (default gold `#d4af37`), `void` (violet
  `#8b5cf6`), `glacier` (cyan `#22d3ee`), `high-contrast` (declared in
  `lib/theme.ts` but **not implemented in `index.css`**).
* **Accent switching**: by setting `data-theme` on `<html>` → CSS variable
  `--brass` (RGB triple) → `tailwind.config.js` maps `brass` to
  `rgb(var(--brass) / <alpha-value>)`.
* **Backdrop** (`ui/src/components/ui/Backdrop.tsx`): layered grid (48px),
  3 radial gradients (gold/violet/cyan), scanline overlay. Distinctive and
  expensive-looking.
* **Glass** (`ui/src/components/ui/Glass.tsx`): rounded-2xl,
  `border-white/[0.06]`, `bg-white/[0.025]`, `backdrop-blur-2xl`, inset ring,
  inset-top highlight + bottom drop shadow.
* **Numerics**: mono, tabular-nums on all KPIs.
* **Animation**: `vox-ping` (live dot), `vox-shimmer` (chart loading),
  `vox-toast-in` (slide-in), `shimmer` (skeleton). Already respects
  `prefers-reduced-motion` globally.

### 1.3 Surface registry (28 live surfaces, 6 nav groups)

Source of truth: `contracts/gui/surface-registry.v1.yaml` (28 active +
~50 CLI-only entries). The sidebar uses
`ui/src/generated/surfaceRegistry.generated.ts` + a `TOP_NAV_META` map in
`Sidebar.tsx`.

* **Operate** (7): Agents, Flow, Dashboard, Matrix, Tasks, Runs, Approvals,
  Policies, Gamify
* **Develop** (8): Workspace, Commands, Catalog, Skills, Harness, Browser,
  Console, Repository
* **Knowledge** (5): Knowledge, Search, Memory, Research, Claims, Discovery
  Inbox, Discovery Review, Publications, Review, Archive Panel, Scientia
* **Compute** (4): Compute, Models, Mesh, Mens, Oratio, Populi
* **System** (2): Settings, Coverage
* **Hub** (1): Chat

### 1.4 Chrome (layout shell)

`ui/src/components/layout/AppShell.tsx`:

```
[Sidebar 64/212/280px]   [TopHud KPI strip]
                         [BreadcrumbBar]
                         [StatusBar  ← duplicates TopHud KPIs!]
                         [DockShell → SurfaceErrorBoundary → main surface]
                         [chat-dock  ← wired but chatDocked=false]
```

Sidebars modes: `rail | default | wide`, cycled with `Cmd+B`. HUD mode:
`full | slim | hidden`, cycled with `Cmd+Shift+H`. Command palette:
`Cmd+K`.

---

## 2. Code review — bugs & smells

### 2.1 Bugs (will trip users)

> Path references are `crates/vox-gui/ui/src/...` unless noted.

| # | Where | Issue | Impact | Fix |
|---|---|---|---|---|
| B1 | `components/ui/Sparkline.tsx:29` | `gid = "g" + (data.join("").length + data[0] * 7 \| 0)` is non-unique. Two sparklines with the same data array share gradient def → second renders in the first's color. | Visual collision on Dashboard with multiple KPIs. | Use `React.useId()`. |
| B2 | `components/layout/TopHud.tsx:191` | "Active Model" tile uses `kpis.activeAgents.spark` (wrong source — should be a model-confidence/burn or empty). | Misleading trend on the top bar. | Remove the sparkline or source from a model metric. |
| B3 | `components/layout/StatusBar.tsx:131-136` | `label="Model" value="auto-route"` is hardcoded. `activeModel` prop is not consumed by StatusBar. | Dead UI. | Either consume `activeModel` or remove the segment. |
| B4 | `App.tsx:968-1029` | `chatDocked = false` is hardcoded; the dock code path is shipped but never used. The `mainPaddingBottom` branch (`pb-[180px]` vs `pb-5`) is therefore dead-code-conditional. | Confusion; future bug when re-enabled (huge pad). | Remove the dock code OR enable for Chat surface. |
| B5 | `App.tsx:898-935` | Command-palette `handleCommandAction` uses string-prefix discrimination on `cmd.id` (`startsWith('agent:')`, `startsWith('skill:')`) — stringly-typed. | Refactor risk; one typo and an action is silently swallowed. | Discriminated union with a `type` field. |
| B6 | `components/ui/Toasts.tsx:23-24` | `bottom-[200px] right-6` is a magic number; `z-40` for toasts vs `z-[60]` for achievements stack → if both fire, toasts sit under the achievement pop-in. | Stack collision. | Use a named z-stack (see §3.1) and pin via `data-attribute` slot. |
| B7 | `App.tsx:171` | `useLocalStorage('vox_active_view', 'dashboard')` writes on first render **before** the deep-link parser runs in the bootstrap effect. | Flash of wrong surface on first paint. | Use `useState` initializer reading `parseViewFromLocation` once. |
| B8 | `App.tsx:185-194` | `setPolicyBadge(null)` on any error → user sees no badge and assumes "all good", but it could be 401. | False-negative UX. | Distinguish "fetch failed" (warning tone in badge) from "all clear". |
| B9 | `components/surfaces/Tasks/TasksView.tsx:169-179` | `onBlur` calls `saveEdit` and `onKeyDown:Enter` also calls `saveEdit`. On Enter+blur the row saves twice → second save is a no-op but emits an event. | Spurious event. | Cancel blur on Enter, or guard with a ref. |
| B10 | `components/surfaces/Tasks/TasksView.tsx:11-20` | `loadSessionTitles()` reads `localStorage.getItem('vox_chat_sessions')` synchronously on every render — and that key is written by a different subsystem. If the writer is renamed, the chip labels silently break. | Fragile cross-subsystem contract. | Read via a context or a Tauri command, not a hand-rolled localStorage key. |
| B11 | `components/surfaces/Chat/ChatExecutionRail.tsx` *(suspect — needs read)* | Many surfaces receive `pushToast` but never use it. Prop leaks. | Noisy API surface. | Drop the prop where unused. |
| B12 | `components/ui/EmptyState.tsx:21-28` | Action button has no `aria-label`, no focus ring, raw Tailwind classes — diverges from any future `Button` primitive. | A11y + visual inconsistency. | Migrate to `<Button variant="primary" size="sm">`. |
| B13 | `components/ui/StateChip.tsx` | Uses `text-[9px]`, `tracking-widest`, `font-extrabold`, `border-white/10` for `neutral` — different visual language from `Pill.tsx` (which is `text-[10px]`, `font-medium`, `ring-1`, transparent bg). | Two "chip" languages. | Unify (see §3.2). |
| B14 | `components/ui/Glass.tsx:18-19` | The "inset" ring is always rendered even when caller wants a flat surface (some places want no ring at all). | Visual noise in some surfaces. | Make `inset` opt-out (already a prop, but the default is `true` and there's no `false` usage in code). |
| B15 | `components/ui/Pill.tsx:42-46` | `vox-ping` animation is applied to all non-Paused pills → in a list of 10 agents, 10 ping rings fire simultaneously. Visual cacophony. | Visual noise. | Cap at first 3 or only on hovered row. |
| B16 | `components/layout/AppShell.tsx:133-138` | `SurfaceErrorBoundary key={surfaceKey}` re-keys on every nav → remounts the whole surface on every tab change. Loses in-flight editor state (TasksView draft, RunsView filter). | Annoying state loss. | Memo on `surfaceKey`; only re-key on parent change. |
| B17 | `components/layout/Sidebar.tsx:138-144` | When `filterQuery` matches nothing, the user sees a blank nav. | Confusing. | Add an "empty filter result" hint. |
| B18 | `components/surfaces/Dashboard/Dashboard.tsx:62` | `filters = ["all","validated","in-progress","doubted","speculative"]` are hardcoded; the `filterKind` chip also shortens "in-prog" but the underlying value is "in-progress". | Copy-paste future bug. | Single source of truth. |
| B19 | `App.tsx:615-689` | `dispatchSessionChat` is called with `runId` from the callback inside `executeIpcWithRun`. If the invoke rejects before minting the runId, the assistant bubble is never created → user sees nothing. | Silent failure. | Mint a local runId before the invoke, with an `erroring` state. |
| B20 | `App.tsx:665-670` | `window.confirm(...)` for "duplicate? submit anyway" — blocks the renderer, breaks Tauri webview focus. | Inconsistent UX. | Replace with an `InlineConfirm` or modal. |
| B21 | `index.css:72-76` | Focus ring uses `var(--color-accent-default)` directly, but `accent.default` is a hex literal, not a CSS variable — change is brittle. | Theming leak. | Use a `--focus-ring` variable. |
| B22 | `lib/theme.ts:13` | `'high-contrast'` is in the union but missing from `index.css` `:root[data-theme='high-contrast']` block. | Empty state on selection. | Implement or remove. |
| B23 | `components/ui/Sparkline.tsx:21` | `range = max - min \|\| 1` flattens constant data to mid-line; for a status dot, the user can't tell "stable" from "off". | Visual ambiguity. | Show last value as a dot only when range is 0. |
| B24 | `components/layout/TopHud.tsx:227-249` | `slim` mode shows raw numeric `queue {kpis.queueDepth.value}` but `kpis.mesh.peers` is used directly while `kpis.mesh.value` exists; in `full` mode the mesh tile uses `kpis.mesh.value` (different number). | Two different "mesh" values in the same UI. | Pick one. |
| B25 | `components/surfaces/Tasks/TasksView.tsx:317-371` | Virtual list `style={{ height: Math.min(Math.max(inProgress.length, 1) * (ITEM_HEIGHT + GAP), 320) }}` — the hardcoded `320` cap is invisible to the user; tasks past 320 have no scroll. | Hidden truncation. | Make height responsive (flex-1 with min/max). |

### 2.2 Architectural smells

* **Magic-number typography**: 30+ `text-[Npx]` and `text-[Ntiny]` instances
  across surfaces. No type-scale token. → §3.1.
* **Status-tone fragmentation**: 4 separate tone tables —
  `STATUS_BADGE_CLASS`, `STATUS_RAIL_BADGE_CLASS`, `PHASE_TONE` (Pill),
  `toneClass` (StateChip), plus the inline `liveClasses` in TopHud/StatusBar.
  → §3.1.
* **No real loading skeletons**: only Dashboard has them. Tasks, Runs,
  Approvals, Policies, etc. show a blank/empty state while loading. → §3.3.
* **No per-widget error boundary** in `DashboardGrid`; one widget error
  blanks the whole grid. → §3.5.
* **No Storybook / visual regression harness**. Coverage is from `*.test.tsx`
  + Playwright smoke. Hard to refactor safely. → §3.6.
* **Stringly-typed palette** in `lib/installedSkills.ts`, `paletteSources.ts`,
  `commandPaletteActions.ts` — no discriminated union for action types. → §3.2.
* **`chatComposer` is passed into `surfaceProps` even when not needed** by
  non-chat surfaces. Prop leaks across boundaries. → §3.5.
* **`useOrchestratorStatus` freshness** doesn't downgrade the freshness dot
  fast enough — `LIVE_EVENT_FRESH_MS` default is 10s but the freshness hook
  only triggers a re-render when `lastOrchEventAt` changes (App.tsx:316-321).
  → §3.5.
* **Backdrop is a fixed cost on every render** (4 layered divs with
  `-z-10`). It's static — should be a `position: fixed` element outside the
  React tree, mounted once. Tiny perf, but right thing. → §3.5.
* **No first-run / empty-state guidance** anywhere. → §3.4.
* **No undo for destructive actions** (`cancel_orchestrator_task`,
  `pause_orchestrator_agent`, `doubt_orchestrator_task`). Toast says
  "Rollback complete" but the action is fire-and-forget. → §3.4.

---

## 3. Beautification plan (phased)

### 3.1 Phase 0 — Foundation (design tokens, bug sweep)

> Gate: token system compiles, all B1–B25 fixed, no visual regressions.

**Add the token system.** Style Dictionary can output both CSS and TS. Add
to `ui/tokens/`:

```jsonc
// ui/tokens/type.json
{
  "type": {
    "size":  { "3xs": "9px", "2xs": "10px", "xs": "11px", "sm": "12px",
               "base": "13px", "md": "14px", "lg": "16px", "xl": "18px",
               "2xl": "20px", "3xl": "24px", "4xl": "32px", "5xl": "40px" },
    "weight": { "regular": "400", "medium": "500", "semibold": "600",
                "bold": "700" },
    "leading": { "tight": "1.25", "snug": "1.375", "normal": "1.5",
                 "relaxed": "1.625" },
    "tracking": { "tighter": "-0.02em", "tight": "-0.01em",
                  "normal": "0", "wide": "0.04em",
                  "wider": "0.12em", "widest": "0.22em" }
  }
}
```

```jsonc
// ui/tokens/elevation.json
{
  "elevation": {
    "0": { "shadow": "none", "border": "rgba(255,255,255,0.00)" },
    "1": { "shadow": "0 1px 0 rgba(255,255,255,0.04) inset, 0 12px 24px -16px rgba(0,0,0,0.5)", "border": "rgba(255,255,255,0.06)" },
    "2": { "shadow": "0 1px 0 rgba(255,255,255,0.06) inset, 0 24px 48px -20px rgba(0,0,0,0.6)", "border": "rgba(255,255,255,0.10)" },
    "3": { "shadow": "0 1px 0 rgba(255,255,255,0.08) inset, 0 32px 64px -20px rgba(0,0,0,0.7)", "border": "rgba(255,255,255,0.14)" }
  }
}
```

```jsonc
// ui/tokens/z.json
{ "z": { "base": "0", "dropdown": "10", "sticky": "20", "overlay": "30",
         "modal": "40", "popover": "50", "toast": "60", "system": "70" } }
```

* **Wire `tailwind.config.js`** to a `text-{size}` and `text-tracker-{key}`
  utility extension. Replace magic numbers in a sweep PR.
* **Unify status tones** in `ui/src/styles/tokens.ts`:

  ```ts
  export const STATUS_TONE = {
    pass:   { dot: 'bg-emerald-400',  ring: 'ring-emerald-400/30',  text: 'text-emerald-300',  soft: 'bg-emerald-400/10',  solid: 'bg-emerald-400',  onSolid: 'text-zinc-950' },
    fail:   { dot: 'bg-red-500',      ring: 'ring-red-500/30',      text: 'text-red-300',      soft: 'bg-red-500/10',      solid: 'bg-red-500',      onSolid: 'text-zinc-950' },
    warn:   { dot: 'bg-amber-400',    ring: 'ring-amber-400/30',    text: 'text-amber-300',    soft: 'bg-amber-400/10',    solid: 'bg-amber-400',    onSolid: 'text-zinc-950' },
    info:   { dot: 'bg-sky-400',      ring: 'ring-sky-400/30',      text: 'text-sky-300',      soft: 'bg-sky-400/10',      solid: 'bg-sky-400',      onSolid: 'text-zinc-950' },
    neutral:{ dot: 'bg-zinc-500',     ring: 'ring-zinc-500/30',     text: 'text-zinc-300',     soft: 'bg-white/[0.04]',    solid: 'bg-zinc-500',     onSolid: 'text-zinc-100' },
    accent: { dot: 'bg-brass',        ring: 'ring-brass/30',        text: 'text-brass',        soft: 'bg-brass/10',        solid: 'bg-brass',        onSolid: 'text-zinc-950' },
    // phase-specific (Pill)
    Executing:   { dot: 'bg-brass',     ring: 'ring-brass/30',       text: 'text-brass',       soft: 'bg-brass/10',       solid: 'bg-brass',       onSolid: 'text-zinc-950' },
    Verifying:   { dot: 'bg-violet-400',ring: 'ring-violet-400/30', text: 'text-violet-300',  soft: 'bg-violet-400/10',  solid: 'bg-violet-400',  onSolid: 'text-zinc-950' },
    Planning:    { dot: 'bg-cyan-400',  ring: 'ring-cyan-400/30',    text: 'text-cyan-300',    soft: 'bg-cyan-400/10',    solid: 'bg-cyan-400',    onSolid: 'text-zinc-950' },
    Paused:      { dot: 'bg-zinc-500',  ring: 'ring-zinc-500/30',    text: 'text-zinc-300',    soft: 'bg-white/[0.04]',   solid: 'bg-zinc-500',    onSolid: 'text-zinc-100' },
    Validated:   { dot: 'bg-emerald-400',ring:'ring-emerald-400/30', text: 'text-emerald-300', soft: 'bg-emerald-400/10', solid: 'bg-emerald-400', onSolid: 'text-zinc-950' },
    Doubted:     { dot: 'bg-amber-400', ring: 'ring-amber-400/30',   text: 'text-amber-300',   soft: 'bg-amber-400/10',   solid: 'bg-amber-400',   onSolid: 'text-zinc-950' },
    Speculative: { dot: 'bg-violet-400',ring: 'ring-violet-400/30',  text: 'text-violet-300',  soft: 'bg-violet-400/10',  solid: 'bg-violet-400',  onSolid: 'text-zinc-950' },
    Active:      { dot: 'bg-cyan-400',  ring: 'ring-cyan-400/30',    text: 'text-cyan-300',    soft: 'bg-cyan-400/10',    solid: 'bg-cyan-400',    onSolid: 'text-zinc-950' },
    Root:        { dot: 'bg-white',     ring: 'ring-white/30',       text: 'text-white',       soft: 'bg-white/[0.06]',   solid: 'bg-white',       onSolid: 'text-zinc-950' },
  } as const;
  ```
  Then refactor `Pill`, `StateChip`, `STATUS_BADGE_CLASS`, freshness pills to
  read from this single map.

* **Add `--focus-ring` CSS var** in `index.css`; update the global
  `*:focus-visible` rule.
* **Implement `high-contrast` theme** (or remove from union).
* **Bug sweep**: fix B1–B25 above in a single PR (or 2 if conflicts).

### 3.2 Phase 1 — Component polish

> Gate: primitives have full APIs, 5 surfaces migrated, snapshots + visual
> review pass.

**Promote primitives to first-class components with full API:**

```tsx
// Button — replaces 30+ ad-hoc button classNames
<Button variant="primary|secondary|ghost|outline|danger"
        size="xs|sm|md|lg|icon"
        loading?: boolean
        icon?: React.ReactNode
        trailingIcon?: React.ReactNode
        asChild?: boolean>…</Button>

// KPI — used by TopHud, StatusBar, dashboard widgets, Chat
<Kpi label="Active Agents" value={7} unit="" delta={2} trend="up|down|flat"
      accent="cyan|amber|emerald|violet|brass|zinc"
      sparkData={[…]}>
  <Kpi.Sub>awaiting daemon cap</Kpi.Sub>
  <Kpi.Spark />
</Kpi>

// StatusPill — replaces Pill + StateChip + ad-hoc status indicators
<StatusPill tone="pass|fail|warn|info|neutral|accent|Executing|…"
            pulse?: boolean
            size="xs|sm"
            icon?: React.ReactNode />

// DataTable — for Tasks, Runs, Approvals, Claims, Policies, Mesh peers
<DataTable rows={…} columns={…} groupBy?: (r) => string
          onRowAction?: (id, action) => void
          emptyState={<EmptyState variant="no-data" … />}
          loading?: boolean
          virtualized?: boolean
          getRowId?: (r) => string />

// EmptyState — first-class with variants
<EmptyState variant="no-data|no-permission|no-connection|error|welcome"
            icon={…} title={…} description={…}
            primaryAction={…} secondaryAction={…} />

// Glass — size variants
<Glass size="sm|md|lg" inset?: boolean>…</Glass>
```

**Initial migrations** (5 surfaces, ordered by density of repeated ad-hoc
patterns):

1. `components/surfaces/Tasks/TasksView.tsx` → `DataTable` + `EmptyState`.
2. `components/surfaces/Runs/RunsView.tsx` → `DataTable` + `StatusPill`.
3. `components/surfaces/Approvals/ApprovalsView.tsx` → `DataTable` + `StatusPill` + `EmptyState`.
4. `components/surfaces/Dashboard/Dashboard.tsx` → `Kpi` for in-grid widgets, `StatusPill` for stream tags.
5. `components/surfaces/Chat/ChatSurface.tsx` → `Kpi` for execution rail, `StatusPill` for phase indicators.

### 3.3 Phase 2 — Layout & navigation

> Gate: sidebar parent/child visible without filter, TopHud and StatusBar
> not duplicating, no magic numbers in chrome.

* **Consolidate TopHud + StatusBar.** Pick one — keep TopHud as the canonical
  KPI strip; replace StatusBar with a **context bar** (current view name,
  short git branch, last orch event, achievement trigger). The KPI tiles move
  to the surface header.
* **Sidebar parent/child visibility.** Drop the "only show children when
  filter is active" gating (`Sidebar.tsx:147-159`). Always render the parent
  group header + children, collapsed by default but expandable inline.
* **Quick-search context awareness.** The `Search or jump…` button in
  TopHud opens the command palette; add a "Recent" section (last 5 surfaces
  the user visited, persisted to localStorage).
* **Breadcrumb on Chat.** `BreadcrumbBar.tsx:13` currently returns `null` for
  `viewKey === 'chat'`. Render a single-segment "Chat" with a sibling
  "Sessions" pill instead.
* **Active-view context line.** Add a thin row under the breadcrumb:
  `Viewing: Tasks (root) · 23 tasks · 2 sessions · last update 4s ago`. Uses
  the surface registry `notes` + runtime metrics.
* **Backdrop → static.** Move the 4 layered divs from
  `Backdrop.tsx` into a single `position: fixed; inset: 0` element with
  all backgrounds composed. Mount once outside the React tree.
* **Tauri window chrome.** Document / wire the titlebar to use the
  `arcane/void/glacier` accent for the active state. Drop the default OS
  frame if Tauri decorations allow.

### 3.4 Phase 3 — Onboarding, empty states, undo

> Gate: every surface has explicit empty + loading + error states; every
> destructive action has an undo toast.

**Empty-state catalog** (all surfaces need one):

| Surface | Variant | Default copy |
|---|---|---|
| Dashboard | `no-data` + `welcome` | "Vox is online. Submit a task in Chat to get started." with primary "Open Chat" |
| Tasks | `no-data` | "Queue is empty — the agent is all yours." (already partly there) |
| Runs | `no-data` | "No runs in the last 24h." |
| Approvals | `no-pending` | "Nothing waiting on you." |
| Policies | `no-data` | "Policy registry not yet populated. Run `vox policy refresh`." |
| Mesh | `no-peers` | "No peers online. Start a Populi node to grow the mesh." |
| Models | `no-models` | "No models configured. Add one in Settings → Models." |
| Search | `no-results` | "Nothing matched. Try a broader query." |
| Knowledge | `no-data` | "No research in this workspace yet." |
| Memory | `no-context` | "Project memory is empty. Add a VOX.md to seed it." |
| Console | `welcome` | "Pick an agent above to attach a terminal, or hit Enter for the root." |
| Browser | `welcome` | "Open a URL or pick a preview from the workspace." |

**Loading skeleton policy.** Every surface that has a `loading` boolean must
also have a `<Skeleton variant="…">` rendered when true. Add a `<Surface
loading skeleton={…}>` wrapper or per-surface skeletons.

**Undo pattern.** Add a `<Toast action={{label:"Undo", onClick:…}}>` to all
destructive actions:

* `cancel_orchestrator_task` → "Task cancelled · Undo (5s)"
* `pause_orchestrator_agent` → "Agent paused · Resume"
* `doubt_orchestrator_task` → "Doubt injected · Withdraw (10s)"
* `overrule_orchestrator_task` → "Doubt overruled · Re-doubt (10s)"

**First-run tour.** On first launch (`localStorage['vox_first_run_done']`),
launch a 4-step walkthrough: Dashboard → Chat → Submit a task → Approve
flow. Dismissable; replayable from Settings → Onboarding.

### 3.5 Phase 4 — User journeys

> Gate: each journey can be completed in ≤ 3 clicks, with full keyboard
> support, and has explicit empty/error/loading coverage.

**Journey 1 — Submit a task (operator).**

```
Cmd+K → "Submit task" → composer opens in Chat surface
  → context chips auto-populate from active surface
  → submit → progress visible in Stream
  → completion toast with "Open transcript" link
```

**Journey 2 — Approve a risky action (governance).**

```
Chat assistant asks for approval
  → InlineApprovals card appears with diff preview
  → Approve / Deny buttons
  → toast "Approved · can revoke from Approvals tab"
  → Approvals surface shows audit trail
```

**Journey 3 — Switch context (multi-task).**

```
Dashboard → see Task A in Stream
  → click agent card → Agent Flow surface
  → click task → Tasks surface with row selected
  → breadcrumb shows "Agents › Flow › A-03"
  → breadcrumb click returns to Dashboard, A-03 still highlighted
```

**Journey 4 — Theme switch (personalization).**

```
Settings → Theme picker shows 4 swatches (arcane, void, glacier, high-contrast)
  → click swatch → preview applied instantly to current surface
  → Save → persisted to localStorage
  → re-launch → theme survives
```

**Journey 5 — Investigate a failure (debug).**

```
Dashboard stream shows "doubted" badge
  → click stream item → opens detail
  → "Why was this doubted?" → chat assistant in context
  → "Override" → overrule flow
  → audit row in Approvals
```

Each journey needs a per-step spec and a Playwright happy-path test. Add
the specs under `ui/e2e/journeys/`.

### 3.6 Phase 5 — Accessibility, resilience, quality

> Gate: WCAG 2.1 AA across all surfaces, Storybook deployed, visual
> regression in CI.

* **Keyboard nav.**
  * Sidebar items navigable with `↑`/`↓`/`Home`/`End`.
  * Modals focus-trap (Radix Dialog already gives us most of it; verify).
  * `Esc` closes the command palette, achievement drawer, settings
    sheets, and inline approvals.
  * `?` opens a one-page keyboard shortcut help dialog.
* **Touch targets.** Audit and bump every clickable element to ≥ 44×44px.
  * Sidebar nav: `py-2.5` → `py-3` (40 → 48px).
  * TopHud tiles: ensure `min-h-11` or larger.
  * Pill close buttons: ensure 44×44.
* **Color-only signaling.** Wherever a `Pill`/`StatusPill` carries the
  message, add an icon when state is non-default (`✓` for pass, `!` for
  fail, `?` for warn). The `Pill` is the only indicator in many lists
  (Tasks, Runs, Approvals).
* **Reduced motion.** Currently the global `prefers-reduced-motion` rule
  works, but `vox-ping` on every active pill still runs. Add a per-component
  override that suppresses the ping ring on `prefers-reduced-motion`.
* **High-contrast theme.** Implement `:root[data-theme='high-contrast']`
  with pure white/yellow on black and stricter borders; bump font weight
  to `medium` for body text.
* **Storybook.** Install Storybook 8, add a story per primitive
  (`Button`, `Kpi`, `StatusPill`, `DataTable`, `EmptyState`, `Glass`).
  No stories for surfaces yet — that comes in Phase 6.
* **Visual regression.** Wire Playwright's screenshot mode to capture
  every surface under each theme; add a CI job that fails on diff > 0.5%.
* **Bundle budget enforcement.** `lib/dashboardBundleBudget.ts` already
  exists; add a `vox ui bundle-check` command and run in CI.
* **Console-leak guard.** Add a CI grep that fails the build if
  `console.log`/`console.debug` appears in `ui/src/**` (warnings are
  fine; info-level are gated to dev-only).

### 3.7 Phase 6 — Surface-by-surface rebuild

> Gate: every surface has a one-page spec, a story, a Playwright happy
> path, and ships against the new design system.

Each surface gets the same micro-process: spec → tokens → component audit →
fix → test → ship. Order (by user-traffic, then by surface-dependency):

1. **Dashboard** (already most polished)
2. **Chat** (highest user traffic; the composer is the heart of the app)
3. **Tasks**, **Runs**, **Approvals** (orchestrator triumvirate)
4. **Policies** (governance)
5. **Mesh**, **Models**, **Compute** (compute triumvirate)
6. **Knowledge** family: Knowledge, Search, Memory, Research, Claims, Discovery
7. **Workspace**, **Commands**, **Catalog**, **Skills**, **Harness**, **Browser**, **Console**, **Repository**
8. **Settings**, **Coverage**
9. **Gamify** (drawer, achievements, trophy, banner)

### 3.8 Phase 7 — Polish & micro-interactions

* **Sound design** (opt-in, default off): subtle ticks on task transitions
  in the stream, soft chime on approval, mute toggle in StatusBar.
* **Haptic feedback** on touch surfaces (Tauri supports it on supported
  platforms).
* **Window-level keyboard shortcuts dialog** (`?`): one screen, themed,
  searchable.
* **Custom titlebar** themed per accent; macOS traffic lights, Windows
  min/max/close with the brass accent on hover.
* **Resizable panels** (using `react-resizable-panels`) on the chat +
  execution rail layout, persisted to localStorage.
* **Drag-to-rearrange sidebar** order (persisted).

---

## 4. Pre-ship checklist (every PR that touches the GUI)

* [ ] No `text-[Npx]` outside `ui/tokens/`. Magic numbers in components get
      rejected.
* [ ] No `bg-red-500/20` etc. outside the `STATUS_TONE` map.
* [ ] No `console.log`. (`console.warn` is OK with a TODO; `console.error`
      is OK in error paths.)
* [ ] All clickable elements ≥ 44×44px touch target.
* [ ] `*:focus-visible` is visible against the surface (≥ 3:1 contrast).
* [ ] `<StatusPill>` used for any status display, not a hand-rolled chip.
* [ ] `<EmptyState>` used for any "no data" case; no raw "no data" text.
* [ ] Loading state renders a `<Skeleton variant>` (not a blank).
* [ ] Error state renders a recovery path, not a stack trace.
* [ ] Destructive actions ship with an `Undo` toast action.
* [ ] New components have a Storybook story.
* [ ] Affected surfaces have a Playwright screenshot test under each
      theme.

---

## 5. Metrics

* **Lighthouse** for the UI (run via Playwright + axe-core): target ≥ 95
  per category across all surfaces.
* **Time-to-first-task** (median, p95) on a fresh install: target ≤ 60s.
* **Command-palette usage** vs. sidebar clicks: should grow (palette is
  faster).
* **Undo-toast usage**: how often users undo destructive actions; tune
  timeout.
* **Error boundary trip rate**: target 0 per 1k sessions.
* **Bundle size**: main bundle ≤ 600 KB gz, each lazy surface chunk
  ≤ 150 KB gz.

---

## 6. Out of scope (intentionally)

* **Tauri 2 → 3 migration** — separate plan; this plan assumes Tauri 2.
* **Vox mobile** (RN/Expo) — different surface; see
  `vox-runtime-rn`. The design tokens here should be portable.
* **Vox vscode webview** — deprecated; do not invest.
* **Vox dashboard (retired Axum SPA)** — already archived.

---

## 7. References

* `crates/vox-gui/ui/src/index.css` — theme variables, focus ring, motion.
* `crates/vox-gui/ui/tokens/*.json` — current Style Dictionary source.
* `crates/vox-gui/ui/src/components/ui/Glass.tsx` — current glass primitive.
* `crates/vox-gui/ui/src/components/ui/Backdrop.tsx` — backdrop composition.
* `crates/vox-gui/ui/src/components/layout/AppShell.tsx` — current shell.
* `contracts/gui/surface-registry.v1.yaml` — surface ownership SSOT.
* `contracts/frontend/surface-ownership.v1.yaml` — surface ownership.
* `docs/src/architecture/where-things-live.md` — concept-to-crate lookup.
* `docs/src/architecture/vox-gui-capability-audit-2026.md` — capability audit.
* `crates/vox-gui/ui/src/lib/theme.ts` — theme switcher.
* `crates/vox-gui/ui/src/styles/tokens.ts` — current status tones.
