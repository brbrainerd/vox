---
title: "Vox GUI Design Review (annotated mockups + component specs)"
description: "Visual design review for the Vox Tauri GUI. No code, no commits — annotated ASCII mockups for the top 5 operator surfaces, design-system foundations, component API proposals, and a checklist of open design decisions for the operator/owner."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
last_updated: 2026-06-17
training_rationale: "Captures the visual design intent for Vox GUI surfaces — high-value for any LLM doing follow-up implementation work, and the canonical place to start when bringing on a designer."
---

# Vox GUI Design Review

> **Scope:** No code, no commits. This is a **design artifact** for review.
> Source-of-truth plan: [`vox-gui-ux-beautification-plan-2026.md`](./vox-gui-ux-beautification-plan-2026.md).
> This document adds the *visual* layer on top of that plan: annotated
> mockups, component APIs, and a list of design questions only a human can
> answer.

## 0. TL;DR

Vox's visual language is already distinctive (arcane gold + glass + radial
gradient backdrop + mono numerics). What it's missing is **typographic
discipline, status-tone unification, and a primitive layer** that surfaces
can compose against. The mockups below show what each of the 5 highest-leverage
surfaces should look like when those foundations are in place.

Key shifts:

* **Replace ad-hoc `text-[Npx]` with a 10-step type scale** (`3xs`–`5xl`).
* **Unify 4 status-tone tables** into one `STATUS_TONE` map.
* **Promote `Button`, `KPI`, `StatusPill`, `EmptyState`, `DataTable`, `Glass`**
  to first-class primitives with full APIs.
* **Replace the TopHud + StatusBar duplication** with a single HUD strip and
  a context bar.
* **Give every surface real loading + empty + error states** with consistent
  copy and recovery paths.
* **Adopt a Z-tier ladder** (10/20/30/40/50/60/70) for toasts, modals, popovers.

## 1. Top 5 surfaces, ranked

Selection criteria: **operator traffic + design leverage** (how many other
surfaces will learn from the pattern set here).

| # | Surface | Why it's in the top 5 |
|---|---|---|
| 1 | **Dashboard** | Mission control. Highest traffic. Sets the visual standard for the rest. |
| 2 | **Chat** | Where the work happens. The Loquela composer is the heart of the app. |
| 3 | **Tasks** | The operator's triage surface. Pure data-density — exposes every table pattern problem. |
| 4 | **Runs** | Oversight + scoreboard. Mix of timeline + table + decision preview. |
| 5 | **Policies** | Governance. The trust surface — must be calm, legible, and audit-friendly. |

(Bonus: **Settings** is referenced because theme + customization live there.
Will spec as a sidebar item if you want to expand the review.)

## 2. Design system foundations (recap + tightening)

The current system lives in `crates/vox-gui/ui/tokens/`,
`crates/vox-gui/ui/src/styles/tokens.ts`, and
`crates/vox-gui/ui/src/index.css`. This section is what it *should* look
like after Phase 0 of the plan.

### 2.1 Type scale (NEW — replaces 30+ `text-[Npx]` instances)

| Token | Size | Use |
|---|---|---|
| `3xs` | 9px | StatusPill label, monospace metadata |
| `2xs` | 10px | Section overline, segmented control, breadcrumb |
| `xs` | 11px | Secondary body, chip, button-sm |
| `sm` | 12px | Primary body, form input, table cell |
| `base` | 13px | Default body — `text-base` becomes the new default |
| `md` | 14px | KPI value (compact), nav label |
| `lg` | 16px | KPI value (full), section heading |
| `xl` | 18px | Page title (compact) |
| `2xl` | 20px | KPI value (hero) |
| `3xl` | 24px | Page title |
| `4xl` | 32px | Empty state title |
| `5xl` | 40px | Marketing-style hero (used once, in welcome state) |

Weights: `regular` (400), `medium` (500), `semibold` (600), `bold` (700).
Tracking: `tighter` (-0.02em), `tight` (-0.01em), `normal` (0), `wide`
(0.04em), `wider` (0.12em), `widest` (0.22em). The current `tracking-[0.22em]`
and friends collapse into `tracking-widest`.

### 2.2 Spacing scale (4px base)

`0, 0.5, 1, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24` — covered by
Tailwind already. New: **named semantic tokens** for common patterns.

| Token | Tailwind | Use |
|---|---|---|
| `gap.control` | `gap-2` | Between control siblings |
| `gap.section` | `gap-4` | Between sections in a panel |
| `gap.panel` | `gap-5` | Between Glass panels in a grid |
| `pad.panel` | `p-5` | Inside a Glass panel |
| `pad.dense` | `p-3` | Inside a dense list (Tasks, Runs) |
| `pad.chrome` | `px-4 py-2` | AppShell inner padding |

### 2.3 Elevation ladder (3 steps + flush)

| Tier | Shadow | Border | Use |
|---|---|---|---|
| 0 — flush | none | `border-transparent` | Inline tags, embedded chips |
| 1 — surface | `0 1px 0 rgba(255,255,255,0.04) inset, 0 12px 24px -16px rgba(0,0,0,0.5)` | `border-white/[0.06]` | `Glass` (default), KPI tile |
| 2 — raised | `0 1px 0 rgba(255,255,255,0.06) inset, 0 24px 48px -20px rgba(0,0,0,0.6)` | `border-white/[0.10]` | Dropdown, popover, hover state |
| 3 — overlay | `0 1px 0 rgba(255,255,255,0.08) inset, 0 32px 64px -20px rgba(0,0,0,0.7)` | `border-white/[0.14]` | Modal, command palette, dialog |

### 2.4 Z-tier ladder (NEW — replaces `z-40`/`z-50`/`z-[60]` chaos)

```
0  base
10  dropdown       (menus, autocomplete, combobox)
20  sticky         (sidebar, top HUD, status bar)
30  overlay        (backdrop of a modal)
40  modal          (centered modal)
50  popover        (tooltips, hover cards)
60  toast          (toast notifications, achievement pop-ins)
70  system         (Tauri-native OS prompts)
```

### 2.5 Motion ladder (3 steps)

| Token | Duration | Use |
|---|---|---|
| `motion.fast` | 120ms | Hover state, focus ring, button press |
| `motion.base` | 200ms | Panel enter, modal scale, sidebar collapse |
| `motion.slow` | 400ms | Page transition, hero animation |

Already defined in `tokens.json` (`fast: 120ms, base: 200ms, slow: 400ms`).
**No changes needed** — the problem is the code that ignores them.

### 2.6 Status tone (NEW — single source of truth)

Replaces 4 separate tables: `STATUS_BADGE_CLASS`,
`STATUS_RAIL_BADGE_CLASS`, `Pill.PHASE_TONE`, `StateChip.toneClass`, plus
the inline freshness pills in `TopHud.tsx`/`StatusBar.tsx`.

```
pass      emerald-400   emerald-400/10   ✓  Green Check
fail      red-500       red-500/10       !  Red Bang
warn      amber-400     amber-400/10     ?  Amber Question
info      sky-400       sky-400/10       i  Sky Info
neutral   zinc-500      white/[0.04]     ·  Muted Dot
accent    brass         brass/10         ◆  Brass Diamond

// phase-specific (Pill)
Executing    brass         (animated pulse)
Verifying    violet-400
Planning     cyan-400
Paused       zinc-500      (no pulse)
Validated    emerald-400
Doubted      amber-400
Speculative  violet-400
Active       cyan-400
Root         white         (largest glow)
```

Each tone = `{dot, ring, text, soft, solid, onSolid, icon}`. The icon is
new: **every non-default tone now carries a glyph** to break the
color-only-signal accessibility issue (see plan §3.6).

### 2.7 Color-only signal guard

Anywhere a `StatusPill` carries the message, it must also carry a glyph or
text label that doesn't depend on color. Today many rows use `<Pill phase="…">`
with no other indicator; screen-reader users and colorblind users both lose
context.

### 2.8 Backdrop simplification

The current `Backdrop.tsx` renders 4 layered divs in React. Hoist to a
single `position: fixed; inset: 0; pointer-events: none; z-index: -1` element
mounted once at the React root. Saves 3 divs per render, removes a class of
re-render noise.

---

## 3. Surface 1 — Dashboard

**Files:** `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`,
`StreamCard.tsx`, `AgentRow.tsx`, `LudusBanner.tsx`.

### 3.1 Current state — what's working, what's not

**Working:**

* The Stream filter pills (`all / validated / in-progress / doubted / speculative`)
  are good — short labels, real value.
* The "Submit tasks in Chat" CTA banner is a good first-run affordance.
* Sparkline usage is good (KPIs carry visual trend).

**Not working:**

* The "Customize dashboard" / "Add widget" / "Reset to default" trio floats
  absolutely-positioned in the top-right with no visual grouping. Looks
  tacked-on.
* The dashboard CTA banner uses **indigo** (`border-indigo-500/20`,
  `bg-indigo-500/[0.06]`, `text-indigo-100`) — a one-off color that doesn't
  appear anywhere else in the system. Breaks the brass/cyan/violet/emerald
  palette.
* "in-prog" is shortened in the filter chip but the underlying value is
  `"in-progress"` — copy-paste future bug.
* "Customize dashboard" button has no icon; the icon-to-text ratio is
  unbalanced with the other toolbar buttons.
* The "live agent telemetry streams here once tasks run" copy is a
  second-person conditional — for an empty state, switch to second-person
  present ("No events yet. Submit a task to see the stream come alive.").

### 3.2 Proposed layout

```
┌────────────────────────────────────────────────────────────────────────┐
│ [V] Operator        ⌘K Search   │ ⊙ Live   [⚙] [🎯] [▼]                 │ ← TopHud (consolidated)
├────────────────────────────────────────────────────────────────────────┤
│ Workspace › Dashboard              (no breadcrumb for home, OK)        │
├────────────────────────────────────────────────────────────────────────┤
│ [Active Agents 7 ▲2 ▁▃▅] [Queue 23 ▼1 ▁▁▂▃] [Budget $0.42/$5 ▃▄▅]    │ ← KPI strip (now here,
│ [Mesh 4 peers ▂▃▃▄] [Model: auto-route ▁▁▁] [+ Add tile]               │   not duplicated in StatusBar)
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  ┌──────────────────── The Stream ──────────────────────┐              │
│  │ 23 events        [All] [Validated] [Active] [Doubted]│              │
│  │                                                       │              │
│  │ ┌─────────────────────────────────────────────────┐  │              │
│  │ │ ✓ A-03  Compiled 4 files in 1.2s                │  │              │
│  │ │   path: src/eval/value.rs   $0.003   ↗ Flow    │  │              │
│  │ └─────────────────────────────────────────────────┘  │              │
│  │ ┌─────────────────────────────────────────────────┐  │              │
│  │ │ ◆ A-07  Submitting "Audit effort route"         │  │              │
│  │ │   ↳ Model: claude-3-5-sonnet  ETA 8s            │  │              │
│  │ └─────────────────────────────────────────────────┘  │              │
│  │   …                                                   │              │
│  └──────────────────────────────────────────────────────┘              │
│                                                                        │
│  ┌── Active Agents ──┐  ┌── Budget Burn (24h) ─┐  ┌── Policy Status ──┐│
│  │ A-ROOT  ◇ Idle    │  │   ╱╲    ╱╲            │  │ ✓ 12 pass         ││
│  │ A-03    ◆ Exec   │  │  ╱  ╲__╱  ╲___         │  │ ! 1  fail         ││
│  │ A-07    ◆ Exec   │  │                      │  │ ? 2  warn         ││
│  │   +3 dormant      │  │  $0.42 / $5.00  8%   │  │ ↗ Open Policies   ││
│  └───────────────────┘  └──────────────────────┘  └────────────────────┘│
│                                                                        │
│  ┌── Ludus Alerts ─────────────────────────────────────────────────┐  │
│  │ ⚠ Approval pending — agent A-04 wants to edit Cargo.toml        │  │
│  │   [Review]  [Dismiss]                                            │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│                                  [Customize] [Add widget] [Reset]      │ ← grouped, right-aligned
│                                                                        │
├────────────────────────────────────────────────────────────────────────┤
│ ● Live  ·  4s ago  ·  v0.6.0  ·  [trophy]                              │ ← Context bar (was StatusBar)
└────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Specific changes

* **Move KPI tiles from `TopHud` into the dashboard surface header.**
  Rationale: KPIs are surface-scoped (Dashboard cares about all 6; Tasks
  cares about queue depth only). Today TopHud carries the same 6 numbers
  that StatusBar carries at the bottom — pure duplication. Move the KPI
  strip per surface, leave TopHud as workspace brand + search + freshness,
  and reduce StatusBar to a context bar (freshness, build, achievement
  trigger, surface name).
* **Replace the indigo CTA with `brass` accent** — matches the rest of the
  palette.
* **Group the customize trio into a single pill control** with a chevron
  to expand (`[⚙ Customize ▾]`).
* **Use `<StatusPill>` with glyphs** for stream cards (today they use raw
  `tag` strings in colored boxes — no glyph, no `aria-label`).
* **Add a "Policy Status" widget** to the default dashboard grid — it
  shows 12/1/2 (pass/fail/warn) on the current branch. This is currently
  only visible in the sidebar badge.
* **Replace ad-hoc filter button row** with the new `<SegmentedControl>`
  primitive (4 options, 1 selected, no border-button styling).
* **Empty state copy** (per §2.1 of the plan): "No events yet. Submit a
  task in Chat to see the stream come alive."

### 3.4 Annotations

* `KPI` tile in the strip uses the new `Kpi` primitive (§5.2). The sparkline
  on each tile is clickable → drilldown.
* The Stream filter row is a `SegmentedControl` with 5 segments; the "All"
  segment is a real filter, not a clear button.
* The 3 widget cards in the middle row are `Glass` size=`md` (default).
* The Alerts panel uses `<EmptyState variant="no-data">` when there are
  none — currently it shows "All clear — no open alerts." (works, but
  the dashed border looks unintentional).
* Status bar at the bottom is 1 line tall, 12px text, no padding beyond
  `px-3 py-2`. The "Model" segment is **deleted** (B3) — the model lives
  in the per-surface header now.
* The Customize group is right-aligned, single pill, expands to a
  `Menu`/`Dropdown` with "Add widget" + "Reset to default" inside.

---

## 4. Surface 2 — Chat

**Files:** `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`,
`ChatSessionRail.tsx`, `ChatTranscript.tsx`, `ChatExecutionRail.tsx`,
`ContextWindowMeter.tsx`, `SecretaryToast.tsx`, and the global
`Loquela` composer (`components/surfaces/Loquela/Loquela.tsx`).

### 4.1 Current state — what's working, what's not

**Working:**

* Three-pane layout: `ChatSessionRail` (sessions) ‖ `ChatTranscript`
  (messages) ‖ `ChatExecutionRail` (KPIs + tasks + intents). Solid
  information density.
* Loquela composer is its own surface and is *also* passed into Chat via
  the `chatComposer` prop. This is correct (composer lives on Chat only)
  but the prop is leak-prone.
* `SecretaryToast` listens to the `vox://secretary-proposed` Tauri event
  and pops a banner with a proposed task. Good pattern.

**Not working:**

* No global "what context is pinned" indicator. The chips array lives in
  `App.tsx` (`chips` state) and is passed only to the Loquela composer.
  If the user is on Tasks and pins a file, nothing shows it visually.
* Three-pane split is hard-coded (`flex` widths in `ChatSurface.tsx`).
  No resizing, no collapse-to-rail for the session list. A user with 20
  sessions wastes horizontal space.
* The Loquela composer's "active skill" picker is a chip in the input
  area. Good, but the connection between an active skill and the
  underlying catalog entry isn't shown. Where's the skill from? What
  does it do?
* `ContextWindowMeter` shows usage as a percentage but the source data
  isn't exposed. Users see "73% of context used" with no way to drill in.
* The execution rail shows "Tasks" + "Intents" as two lists with no
  visual relationship. A task that's been issued for an intent should
  show the linkage.
* No "thinking…" / "streaming…" indicator that distinguishes:
  token streaming vs. tool call vs. awaiting approval vs. paused.

### 4.2 Proposed layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ [V] Operator        ⌘K Search    │ ⊙ Live      [⚙] [🎯] [▼]              │
├─────────────────────────────────────────────────────────────────────────┤
│ Chat › chat_4f2a · "Audit effort route"                                  │
│ ┊ pinned: src/eval/value.rs (×)   [⊞ Show 2 more]                       │ ← context chips now global
├────────────┬───────────────────────────────────────────┬─────────────────┤
│ Sessions   │ Transcript                                │ Execution       │
│            │                                            │                 │
│ + New chat │  ┌──────────────────────────────────────┐ │ Active 4  Queue 7│
│            │  │ ◆ A-03   2 min ago                   │ │ Mesh 3 peers    │
│ ⊙ Audit…   │  │  Compiling vox-compiler              │ │ Model: auto     │
│   3 msgs   │  │   ⎿ 14 files  ✓ in 1.2s  $0.003     │ │                 │
│            │  └──────────────────────────────────────┘ │ ◯ In progress   │
│ ○ Refactor │  ┌──────────────────────────────────────┐ │ A-03  compiling │
│   12 msgs  │  │ You  2 min ago                        │ │ A-07  awaiting  │
│            │  │ @audit-effort —  please also flag     │ │       approval  │
│ ○ Release  │  │ any code that mixes sync + async      │ │ A-12  streaming │
│   7 msgs   │  └──────────────────────────────────────┘ │                 │
│            │  ┌──────────────────────────────────────┐ │ ◯ Queued        │
│ ○ Research │  │ A-04  just now                        │ │ 1  Audit deep   │
│   18 msgs  │  │ Spawning 3 sub-agents…   ▎            │ │ 2  …            │
│            │  │  ⎿ sub-agent A-04a "typeck"  ⏳ 12s    │ │                 │
│ (collapsed)│  │  ⎿ sub-agent A-04b "eval"    ✓       │ │ ◯ Intents       │
│            │  │  ⎿ sub-agent A-04c "tests"   ⏳ 6s    │ │ 6 pending       │
│            │  └──────────────────────────────────────┘ │                 │
│            │                                            │                 │
│            │  ┌─────────────────────────────────────┐  │                 │
│            │  │ ✦ What should I do next?            │  │                 │
│            │  │ [Ask ▾] [Use pinned context 2]      │  │                 │
│            │  │ [⏎ submit · ⇧⏎ newline · /diff]    │  │                 │
│            │  └─────────────────────────────────────┘  │                 │
│            │                                            │                 │
│            │  Context 12.3k / 16k  ████████░░  77%     │                 │
│            │   ↗ view breakdown                          │                 │
└────────────┴───────────────────────────────────────────┴─────────────────┘
```

### 4.3 Specific changes

* **Make the context chip strip global**, not composer-only. Render it
  under the breadcrumb in `AppShell` whenever `chips.length > 0`. The
  composer still shows them inline, but they're also visible from
  Tasks/Runs/etc.
* **Add resizable pane dividers** between Sessions ‖ Transcript ‖ Execution
  Rail. Use `react-resizable-panels` (not in `package.json` today — add
  it). Persist the split widths to `vox_chat_pane_widths` in localStorage.
* **Collapse the Session rail to a rail (32px) by default** when more than
  6 sessions exist; expand on hover or click.
* **Show the streaming state explicitly** in the assistant bubble:
  * `▎` (cursor) — tokens streaming
  * `⎿ tool` — tool call in progress (with name)
  * `⏳ Xs` — awaiting (count-up timer)
  * `✓` — done
  * `!` — failed (with error inline)
* **Link tasks to intents in the execution rail.** A queued task under
  an intent gets a faint connector line + the intent label.
* **Loquela composer** changes:
  * Add a slash-command picker (`/diff`, `/memory`, `/spawn`, `/rollback`,
    `/doubt`, `/audit`) as a popover that appears on `/`.
  * The "Active skill" chip becomes a button that opens a `Menu` with the
    full skill description + capability id + version. Today it's an opaque
    chip.
  * Replace `window.confirm` (B20) with an `InlineConfirm` widget inside
    the chat — appears in the transcript as a discrete message.
  * The "Duplicate skipped" toast (App.tsx:669) becomes an inline message
    in the chat with a "Submit anyway" button — not a transient toast.
* **The Secretary toast** becomes a 2-state popover instead of a 4-second
  auto-dismiss toast: a permanent card pinned to the bottom-right with
  "Accept" / "Dismiss" / "Edit" actions. Acceptance writes a new task;
  dismissal is a Tauri event; edit opens the composer pre-filled.

### 4.4 Annotations

* Session rail items show: bullet (active marker), title, message count.
  No timestamp. Hover reveals a timestamp + rename/delete in a popover.
* Transcript uses the new `<ChatBubble role="user|assistant|system|tool">`
  primitive with consistent padding (`pad.dense`), a 32×32 avatar slot,
  and the role label as `2xs` overline (today it's a colored dot only).
* The execution rail's "Tasks" list uses the new `<DataTable>` with
  columns: agent, status (StatusPill), ETA, cost. Sortable.
* The context window meter is `<Kpi variant="meter">` — same primitive as
  the other KPIs, with a click-to-drilldown that opens a breakdown modal.
* Streaming indicator: `▎` is a thin vertical bar animated at 1.2 Hz,
  respects `prefers-reduced-motion` (stops animating, stays visible).

---

## 5. Surface 3 — Tasks

**Files:** `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx`,
`tasksHelpers.ts`.

### 5.1 Current state — what's working, what's not

**Working:**

* Two virtualized lists: "In progress" and "Queued". Good.
* Priority chip is click-to-cycle (urgent → background → normal).
* "Add a task…" input at the top.
* Per-row action buttons (edit, delete) appear on hover.

**Not working:**

* Priority chip uses `text-[9px]`, raw colors that don't match the new
  `STATUS_TONE` map.
* Session filter chips show session *titles* read from
  `localStorage.getItem('vox_chat_sessions')` — fragile cross-subsystem
  contract (B10).
* `editingId` + `onBlur` + `Enter` interaction double-saves (B9).
* Hard-coded 320px max height for the virtual list (B25) — silent
  truncation.
* Overlap warning (`⚠ overlaps #1, #2`) and remote mesh tag are inline
  chips with no tooltip delay or copy-to-clipboard.
* The two list sections are not visually grouped — just two `<section>`s
  with the same h2. No "between" affordance.
* The "Add a task" input and the "Refresh" button are far apart (top
  left and top right) — no visual association with each other.
* No row-level status (Active / Paused / Failed). The whole surface is
  "Tasks" but doesn't tell you which tasks have been paused or have
  failed and need attention.
* No bulk actions (pause all, reprioritize selected, cancel selected).

### 5.2 Proposed layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Workspace › Tasks                                              [↻] [⚙] │
├─────────────────────────────────────────────────────────────────────────┤
│ 32 tasks across 4 sessions                                              │
│                                                                         │
│ [+ Add a task…                            ◯ Show: [All] [Mine] [Failed] │
│                                          [☐] 3 selected · [⏸ Pause] [⊗] │
│                                                                         │
│ ⚠ 2 tasks write the same files as another 1 task · [View]               │ ← group-level warnings
│                                                                         │
│ ┌─ ◯ In progress (8) ────────────────────────────────────────────────┐ │
│ │                                                                     │ │
│ │ [URGENT]  #142  Audit typeck performance         agent 04  · 2m     │ │
│ │           ↳ after #140   ⚠ overlaps #138      $0.14  mesh: node-3  │ │
│ │           [⏸] [✎] [⊗]   ← hover-revealed                              │ │
│ │                                                                     │ │
│ │ [NORMAL]  #141  Refactor value.rs               agent 03  · 4m     │ │
│ │           …                                                          │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
│                                                                         │
│ ┌─ ◯ Queued (24) ────────────────────────────────────────────────────┐ │
│ │                                                                     │ │
│ │ [URGENT]  #143  Compile for wasm32                · 3 deps upstream│ │
│ │           [⏸] [✎] [⊗]                                              │ │
│ │   …                                                                  │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
│                                                                         │
│ ⌘N  New task   ⌘⇧P  Pause selected   ⌘K  Search  ·  [Help]              │
└─────────────────────────────────────────────────────────────────────────┘
```

### 5.3 Specific changes

* **Replace the priority chip** with `<StatusPill tone="urgent|normal|background">`
  reading from `STATUS_TONE`. Cycle-on-click behavior stays.
* **Replace the two virtual lists** with a single `<DataTable>` grouped by
  status (In progress / Queued). Group headers expand/collapse.
* **Add a "Show" segmented control** with All / Mine / Failed filters.
* **Add multi-select + bulk actions** in the toolbar: select 3 → toolbar
  shows "3 selected · [⏸ Pause] [⊗ Cancel] [⇈ Top] [⇊ Bottom]". Persist
  selection in component state.
* **Replace the in-row action buttons** with a kebab (`⋮`) menu at row
  end. Hovering the row reveals it. The kebab is a 32×32 button with
  the standard 3-dot icon.
* **Add a "group-level warnings" strip** under the filters — surfaces
  warnings that apply to the whole set (overlaps, missing deps, etc.)
  as a one-line summary with a "View" link to the details modal.
* **Replace the localStorage session-title read** (B10) with a Tauri
  command `chat_list_sessions` (it already exists). Title comes back as
  a first-class field. No more cross-subsystem key contract.
* **Fix the 320px cap** (B25) — make the lists flex-1 with min/max
  heights per section.
* **Add keyboard shortcuts** shown in the footer: ⌘N (new), ⌘⇧P (pause
  selected), ⌘K (search), ⇧? (help).
* **Add a row-level status indicator** beyond priority: a small StatusPill
  (Paused / Failed / Awaiting approval / Executing) at the start of each
  row.

---

## 6. Surface 4 — Runs

**Files:** `crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.tsx`.

### 6.1 Current state — what's working, what's not

**Working:**

* Three vertical regions: `Scoreboard` (model leaderboard), `Recent runs`
  (table), `Routing decision` (preview).
* The scoreboard is genuinely useful — model × task category strength tags.
* Decision preview shows the latest routing decision with reasoning.

**Not working:**

* No resizing between regions.
* The scoreboard has no sorting (always top-down by `quality_score`).
* No date range / status filter for "Recent runs".
* No drill-down on a run — clicking a row does nothing.
* No export / share for the scoreboard.
* The "Decision preview" panel is always visible — when no decision is
  in flight, it shows an empty placeholder; when one is, it competes
  with the runs table for attention.
* The runs table rows have no "command" preview (today the `command`
  field is shown only as a small text under the workflow name, not the
  actual command that was run).

### 6.2 Proposed layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Workspace › Runs                                              [↻] [⚙] │
├─────────────────────────────────────────────────────────────────────────┤
│ Last 7 days · 142 runs · 12 workflows · 8 active                        │
│                                                                         │
│ ┌─ Model Scoreboard (7d) ──────────────────────────────────────────┐   │
│ │ model               category     calls  success  p50    cost     │   │
│ │ ────────────────────────────────────────────────────────────────  │   │
│ │ claude-3-5-sonnet   code-edit     142    94%     1.2s   $0.003   │   │
│ │ gpt-4o              chat          87     88%     0.9s   $0.005   │   │
│ │ gpt-4o-mini         classify       312   97%     0.3s   $0.0002  │   │
│ │ qwen-2.5-coder      code-edit      56    76%     2.1s   $0.000   │   │
│ │   ↗ Show all 17 models                                              │   │
│ └───────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│ ┌─ Recent Runs (last 50) ─────────── Show: [All ▾]  Status: [All ▾] ┐ │
│ │ run_id    workflow           status    steps     cost   duration   │ │
│ │ ─────────────────────────────────────────────────────────────────  │ │
│ │ r-4f2a    gui.loquela.submit  ✓ done    3/3       $0.014   4.2s   │ │
│ │ r-4f1c    agent.pause         ! failed  1/3       $0.003   0.4s   │ │
│ │ r-4f0e    gui.stream.doubt    ✓ done    2/2       $0.001   0.2s   │ │
│ │   ↗ Show all 142 runs · [⤓ Export]                                   │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
│                                                                         │
│ ┌─ Routing decision (live) ────────────────────────────────────────┐   │
│ │ r-4f2a · 4s ago   claude-3-5-sonnet · cost 2 · 1.2s              │   │
│ │ ↳ Because: "task is code-edit, model rank #1, no budget lock"   │   │
│ │ [View run]  [Re-route]                                            │   │
│ └───────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

### 6.3 Specific changes

* **Promote the scoreboard to a real table** (currently a list of rows
  with raw HTML). Column headers click-to-sort; a "Show all N models"
  expand.
* **Add filter dropdowns** above the recent runs table: Show (all /
  workflows / single workflow), Status (all / done / failed / running).
* **Add a drill-down side panel** — clicking a run row opens a right-side
  sheet (40% width) with: full command, all steps, all events, error
  detail, "View in Console" deep-link. Closes on Esc / outside click.
* **Make the routing decision panel collapsible** — collapsed by default
  in the absence of a live decision; expands when one is in flight, with
  a colored border to draw attention.
* **Add a "View in Console" deep-link** on every run row — opens the
  Console surface pre-attached to the agent that ran the workflow.
* **Replace the "Last refresh 4s ago" hand-rolled** with a `useFreshness`
  badge in the toolbar (already used elsewhere — propagate the pattern).
* **Add an export button** for the scoreboard (CSV download via a Tauri
  save-dialog). `get_model_scoreboard` already accepts a `windowDays`
  parameter; the export is the same data, different format.

---

## 7. Surface 5 — Policies

**Files:** `crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.tsx`,
`policyTree.ts`, `types.ts`.

### 7.1 Current state — what's working, what's not

**Working:**

* This is the trust surface. It must be calm, legible, audit-friendly.
* Per-rule status is the centerpiece. Tree view of rules.
* The Sidebar already shows a worst-status badge for the current branch.

**Not working:**

* No "current branch" indicator in the surface header — the user has
  to know which branch they're on from the title bar.
* No filtering by domain/group (the Tauri command `policy_list` accepts
  `domain` and `group` filters, but the UI doesn't expose them).
* No way to see *which commit* the policy status was computed against.
* No way to manually re-run a failed policy check.
* Failed rules don't show *why* in the surface — the user has to leave
  the GUI to investigate.
* The tree view (from `policyTree.ts`) is implemented but the rendering
  is bare HTML; no expand/collapse animation, no icon for rule kind.

### 7.2 Proposed layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Workspace › Policies                                          [↻ Re-run]│
├─────────────────────────────────────────────────────────────────────────┤
│ Current branch: feat/vault-decryption-recovery · commit 5e0dbd2 · 4s ago │
│                                                                         │
│ Domain: [All ▾]    Group: [All ▾]    Search: [____________]              │
│                                                                         │
│ ✓ 12 passing   ! 1 failing   ? 2 warn   · 15 total                       │
│                                                                         │
│ ┌─ Failing (1) ─────────────────────────────────────────────────────┐   │
│ │ ▼ ◆ code-audit › forbidden-corpus     !  Failed · 2m ago         │   │
│ │   ↳ 3 of 47 expected-error cases did not produce a `// expect-…`  │   │
│ │     ↳ [View cases]  [Re-run]  [Mark known-fail]                   │   │
│ │     [Expand stack]                                                 │   │
│ └─────────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│ ┌─ Warn (2) ────────────────────────────────────────────────────────┐    │
│ │ ▼ ◆ ci › policy-registry-parity       ?  Warn · 5m ago           │    │
│ │   ↳ 2 registries drifted from SSOT                                   │    │
│ │   …                                                                │    │
│ └─────────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│ ┌─ Pass (12) ──────────────────────────────────────────────────────┐    │
│ │ ▼ ◆ code-audit › drift-patterns        ✓  Passed · 5m ago         │    │
│ │ ▼ ◆ ci › runner-policy                 ✓  Passed · 5m ago         │    │
│ │   …                                                                │    │
│ └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

### 7.3 Specific changes

* **Surface header** shows the current branch + commit hash + last-run
  timestamp. This is the only surface where the branch matters *for
  display*, so make it prominent.
* **Group rules by status** (Failing / Warn / Pass / Not run) with
  expand/collapse. Today the tree is flat.
* **Show the failure reason inline** under each failing rule. Today the
  user has to leave the GUI to see why.
* **Add per-rule actions**: Re-run, Mark known-fail, View cases. These
  map to existing Tauri commands (or new ones if missing).
* **Add Domain and Group filters** as dropdowns. The data is already
  filterable via `policy_list`; expose it.
* **Add a search box** that filters rules by id, label, or failure
  message.
* **Replace the bare tree HTML** with the new `<DataTable>` primitive
  using `groupBy: rule => status`. Group headers are collapsible.
* **Add a "Re-run all" action** in the toolbar — confirms via modal
  (destructive-ish: re-runs all rules against HEAD).
* **Color the rule's icon with the new `STATUS_TONE`** so a `fail` is
  red, `warn` is amber, `pass` is emerald. Use a glyph, not just color.

---

## 8. Component API proposals (the primitives)

These are the new or promoted primitives that the mockups above depend on.
Full code lives in the plan; this is the API surface and the *why*.

### 8.1 `<Button>`

```tsx
<Button
  variant="primary | secondary | ghost | outline | danger"
  size="xs | sm | md | lg | icon"
  loading?: boolean
  icon?: ReactNode        // leading
  trailingIcon?: ReactNode
  asChild?: boolean       // Radix Slot — for wrapping Link
>
  {children}
</Button>
```

* Replaces 30+ ad-hoc button classNames.
* `loading` swaps the leading icon for a spinner, disables click.
* `size="icon"` produces a square button (32×32 by default; `sm` → 28×28,
  `md` → 40×40 for touch-target compliance).
* `variant="danger"` is the only one with a contrasting `bg-*` (red-500);
  everything else is `bg-brass` or `bg-white/[0.05]`.

### 8.2 `<Kpi>`

```tsx
<Kpi
  label="Active Agents"
  value={7}
  unit=""
  delta={2}
  trend="up | down | flat"      // auto-derived from delta if omitted
  accent="cyan | amber | emerald | violet | brass | zinc | sky"
  sparkData?: number[]
  icon?: ReactNode
  onClick?: () => void
>
  <Kpi.Sub>awaiting daemon cap</Kpi.Sub>      // optional subtitle
  <Kpi.Spark />                                // optional sparkline
</Kpi>
```

* Used by TopHud, StatusBar, dashboard widgets, Chat execution rail.
* Sparkline auto-degrades to a static dot if all values are equal
  (B23).
* Trend arrow uses the new `STATUS_TONE` (emerald for up, rose for down,
  zinc for flat).

### 8.3 `<StatusPill>`

```tsx
<StatusPill
  tone="pass | fail | warn | info | neutral | accent
        | Executing | Verifying | Planning | Paused
        | Validated | Doubted | Speculative | Active | Root"
  size="xs | sm"
  pulse?: boolean             // animated ring (respects prefers-reduced-motion)
  icon?: ReactNode            // explicit override; defaults to tone-specific glyph
>
  {label}
</StatusPill>
```

* Unifies `Pill` + `StateChip` + `STATUS_BADGE_CLASS` + `STATUS_RAIL_BADGE_CLASS`
  + the inline freshness pills in TopHud/StatusBar.
* `pulse` defaults to `true` for Executing/Active/Verifying, `false`
  for Paused/Pass/Neutral.
* `aria-live="polite"` wrapper when used in a list that updates from
  user action.

### 8.4 `<EmptyState>`

```tsx
<EmptyState
  variant="no-data | no-permission | no-connection | error | welcome"
  icon?: ReactNode            // default per variant
  title: string
  description?: string
  primaryAction?: { label: string; onClick: () => void }
  secondaryAction?: { label: string; onClick: () => void }
>
  {children}                  // optional custom body
</EmptyState>
```

* Variants pick default icon, accent, and copy skeleton.
* `aria-live="polite"` so screen readers announce the state.

### 8.5 `<DataTable>`

```tsx
<DataTable
  rows={rows}
  columns={[
    { key: 'id', header: 'ID', width: 80, sortable: true },
    { key: 'status', header: 'Status', render: r => <StatusPill ... /> },
    ...
  ]}
  groupBy?: row => 'in-progress' | 'queued' | ...
  selectable?: boolean
  onRowAction?: (id, action: 'pause' | 'cancel' | ...) => void
  emptyState={<EmptyState variant="no-data" ... />}
  loading?: boolean
  virtualized?: boolean
  getRowId?: row => string
  density="compact | default | comfortable"
/>
```

* Replaces `TasksView`, `RunsView` (table portion), `ApprovalsView`,
  `PoliciesView`, future claims/publications/discovery tables.
* Group headers are click-to-collapse with `motion.base` transition.
* `selectable` adds a leading checkbox column + a bulk-action toolbar
  that appears when ≥ 1 row is selected.
* `loading` shows a skeleton of the first 5 rows.

### 8.6 `<Glass>`

```tsx
<Glass
  size="sm | md | lg"          // default md
  inset?: boolean              // default true — inner ring
  interactive?: boolean        // adds hover state for clickable surfaces
  as?: keyof JSX.IntrinsicElements
>
  {children}
</Glass>
```

* `size` controls padding: sm → `p-3`, md → `p-5`, lg → `p-6`.
* `interactive` adds the elevation-2 hover state and a `cursor-pointer`.

### 8.7 `<SegmentedControl>`

```tsx
<SegmentedControl
  value={filter}
  onChange={setFilter}
  options={[
    { value: 'all', label: 'All' },
    { value: 'validated', label: 'Validated' },
    { value: 'in-progress', label: 'Active' },
    { value: 'doubted', label: 'Doubted' },
    { value: 'speculative', label: 'Speculative' },
  ]}
/>
```

* Used in Dashboard stream filter, Tasks show filter, Runs status filter.
* Replaces the current ad-hoc row of `<button>`s with the same Tailwind
  class string repeated 5 places.

### 8.8 `<Toast>`

```tsx
<Toast
  tone="ok | warn | info | error"
  title={...}
  body={...}
  action?: { label: string; onClick: () => void; autoCloseMs?: number }
  durationMs={5000}              // default 5000
  position="top-right | bottom-right | bottom-center"
/>
```

* `action` is the new "Undo" pattern (Phase 3 of the plan).
* `position` is one of 3 named slots; the Z-tier ladder
  (`z-60` for toasts) is applied automatically.

---

## 9. Cross-cutting changes

### 9.1 TopHud + StatusBar → TopHud + ContextBar

Today: TopHud (6 KPI tiles) + StatusBar (5 segments, mostly duplicating
TopHud). Waste of vertical space, visual noise, two places to update
the same number.

**Proposed split:**

* `TopHud` keeps: workspace brand, search/jump, freshness dot, theme
  accent on hover, build menu.
* Per-surface KPI strip lives in the surface header (each surface owns
  its own KPIs).
* `StatusBar` becomes a `ContextBar` — single line with: surface name,
  current branch, last event timestamp, build version, achievement
  trigger. No duplicated KPIs.

### 9.2 Backdrop → static, outside React tree

```html
<div data-vox-backdrop
  style="position:fixed;inset:0;z-index:-1;pointer-events:none">
  <!-- grid + 3 radial gradients + scanline overlay, composed once -->
</div>
```

Mount once in `index.html` or `main.tsx`; remove `Backdrop.tsx`'s React
component. Saves 4 divs per re-render. Backdrop never changes during the
session.

### 9.3 `--focus-ring` token + global focus

Replace the global `*:focus-visible` rule in `index.css` to use a
dedicated CSS variable:

```css
:root {
  --focus-ring: 0 0 0 2px var(--color-bg-base),
                 0 0 0 4px var(--color-accent-default);
}
*:focus-visible {
  outline: none;
  box-shadow: var(--focus-ring);
}
```

The double-ring pattern is more visible against any surface than a single
3px outline. High-contrast theme can override to use a thicker ring.

### 9.4 Tauri window chrome

Today: relies on the default OS titlebar. Proposal:

* On macOS: keep traffic lights, theme the title bar to match the
  `arcane/void/glacier` accent (subtle 1px gradient at the top).
* On Windows: use `decorations: false` and draw our own titlebar with
  min/max/close controls themed to the accent.

### 9.5 First-run tour

On first launch (`!localStorage['vox_first_run_done']`):

1. Welcome modal with the Vox logomark + "Let's go" CTA.
2. Highlights the Dashboard — "This is mission control."
3. Highlights the sidebar Chat — "All work starts here."
4. Submits a sample task in a sandboxed session.
5. Marks done; replayable from Settings → Onboarding.

### 9.6 Undo pattern for destructive actions

Every destructive action ships with an `Undo` toast (10s timeout for
some, 5s for cheap ones):

| Action | Toast | Undo behavior |
|---|---|---|
| `cancel_orchestrator_task` | "Task cancelled · Undo" | Re-submit with `priority=urgent` |
| `pause_orchestrator_agent` | "Agent paused · Resume" | Trivial re-call |
| `doubt_orchestrator_task` | "Doubt injected · Withdraw" | Tauri `withdraw_doubt` |
| `overrule_orchestrator_task` | "Overruled · Re-doubt" | Re-call doubt |

### 9.7 Loading skeletons policy

Every surface that has a `loading` boolean must render a `<Skeleton>`
when `true`. Skeletons match the surface's actual layout (not generic
gray bars). Add a `<Surface loading skeleton={<TasksSkeleton />}>` wrapper
or per-surface skeletons.

---

## 10. Open design questions (only a human can answer)

These are the decisions I cannot make alone. Each one changes the
implementation in a way that's hard to reverse.

1. **B3 — StatusBar "Model" segment:** consume the prop (display the
   real `activeModel`) or delete the segment entirely?
2. **B4 — `chatDocked` hardcoded to `false`:** remove the dead code
   branch, or enable the dock for the Chat surface (and resize the
   surface above it accordingly)?
3. **B8 — Policy badge error state:** what color/word for "fetch
   failed" vs "all clear"? My proposal: amber, "Unknown".
4. **B16 — SurfaceErrorBoundary re-key:** keep re-key on every nav
   (loses draft state but guarantees crash recovery), or memo per parent
   surface (preserves drafts, may keep a bad state across tabs)?
5. **B22 — High-contrast theme:** implement it properly, or remove from
   the `ThemeId` union (no ship without support)?
6. **TopHud/StatusBar consolidation:** fully agree, or keep both but
   remove the duplication (KPI tiles in TopHud; surface name + branch
   + last event in StatusBar)?
7. **Chat pane split default:** 25/45/30, or 20/55/25, or auto-fit?
   What if the user has no sessions — does the rail hide entirely?
8. **Tasks bulk actions:** expose in this surface, or push to a "Bulk
   operations" modal? (Bulk cancel is destructive enough to want
   confirmation.)
9. **Toast position default:** bottom-right, or bottom-center?
   Bottom-center is more visible but blocks the chat composer on
   Chat surface.
10. **Sidebar default mode:** `default` (212px), `rail` (64px), or
    `wide` (280px)? My proposal: `default`.
11. **Tauri titlebar:** draw our own, or accept the OS default?
    Drawing our own costs effort; accepting the OS default breaks the
    "fully built out" feel.
12. **First-run tour:** opt-in or opt-out? My proposal: opt-in (a
    subtle "Take the tour?" popover on the first launch, dismissable).

---

## 11. What this review does not cover (out of scope here)

* **Code changes** — the plan is the implementation target.
* **Tauri-specific build/packaging** — see the existing Tauri config in
  `crates/vox-gui/tauri.conf.json`.
* **Vox mobile** (RN/Expo) — different surface, separate review.
* **Vox vscode webview** — deprecated; do not invest.
* **Marketing / docs site** — separate surface; not in this review.

---

## 12. References

* `crates/vox-gui/ui/src/App.tsx` — top-level state + chrome.
* `crates/vox-gui/ui/src/components/layout/AppShell.tsx` — current shell.
* `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` — current sidebar.
* `crates/vox-gui/ui/src/components/layout/TopHud.tsx` — KPI strip.
* `crates/vox-gui/ui/src/components/layout/StatusBar.tsx` — bottom strip.
* `crates/vox-gui/ui/src/components/ui/{Glass,Button,EmptyState,Pill,StateChip,Sparkline,Toasts,Backdrop}.tsx` — primitives.
* `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` — Dashboard.
* `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` — Chat.
* `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` — Tasks.
* `crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.tsx` — Runs.
* `crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.tsx` — Policies.
* `crates/vox-gui/ui/src/styles/tokens.{ts,generated.ts,generated.css}` — current tokens.
* `crates/vox-gui/ui/src/index.css` — theme variables, focus ring, motion.
* `crates/vox-gui/ui/tokens/*.json` — Style Dictionary source.
* `contracts/gui/surface-registry.v1.yaml` — surface ownership.
* `docs/src/architecture/vox-gui-ux-beautification-plan-2026.md` — companion plan.
