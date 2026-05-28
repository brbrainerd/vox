---
title: "Vox VS Code Extension — Frontend Redesign Research (2026)"
description: "Research substrate for Industrial Cyber-Renaissance reskin using v0.dev workflow and React-based webview architecture."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-05"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox VS Code Extension — Frontend Redesign Research (2026)

## Purpose

This document consolidates the research phase for reskinning the Vox VS Code extension's webview
frontend using v0.dev as a design scaffold tool. It covers the current codebase structure, the
target aesthetic (Industrial Cyber-Renaissance), design principles, v0.dev workflow strategy,
VS Code adaptation patterns, and open architectural questions.

This is the **research substrate** from which the formal implementation plan will be built.

---

## 1. Current Extension Architecture

### 1.1 Tech Stack

| Layer | Technology |
|---|---|
| Extension Host | TypeScript, VS Code API |
| Webview Bundle | React 19 + TypeScript |
| Bundler | esbuild (custom `esbuild.js`, no PostCSS) |
| Animation | Framer Motion |
| Graphs | @xyflow/react (React Flow v12) |
| Icons | lucide-react |
| Charts | recharts |
| Syntax Highlighting | shiki |
| Markdown | react-markdown + remark-gfm |
| Styling | Hand-rolled Tailwind-like utilities in `index.css` (NOT actual Tailwind) |

### 1.2 Entry Point & Navigation

**File**: `webview-ui/src/index.tsx`

The app renders a `<aside>` icon rail (3 icons + settings gear) on the left and a `<main>` content
area on the right. Tab state:

```
Tab "chat"        → Chat panel (default)
Tab "dashboard"   → UnifiedDashboard
Tab "diagnostics" → EngineeringDiagnostics
```

An `execHint` status strip runs across the top of the content area providing orchestrator/MCP
connection state.

### 1.3 Component Inventory

| Component | File | Role |
|---|---|---|
| `App` | `index.tsx` | Root, state, message routing |
| `UnifiedDashboard` | `UnifiedDashboard.tsx` | Command Center: ops log, Ludus KPI, budget, mesh summary |
| `EngineeringDiagnostics` | `EngineeringDiagnostics.tsx` | Tasks, capabilities, AST, intentions, vox status |
| `AgentFlow` | `AgentFlow.tsx` | ReactFlow DAG of tasks, execution mode visualization |
| `MeshTopology` | `MeshTopology.tsx` | ReactFlow distributed node topology map |
| `IntentionMatrix` | `IntentionMatrix.tsx` | Socrates gate, agent confidence grid |
| `WorkflowScrubber` | `WorkflowScrubber.tsx` | Time-travel state inspector, actor mailboxes |
| `ContextExplorer` | `ContextExplorer.tsx` | Workspace context, repo query, browser lab, context store |
| `ComposerPanel` | `ComposerPanel.tsx` | File-targeted AI draft editor |
| `Panel` | `ui/Panel.tsx` | Shared glass-style card container |
| `StateChip` | `ui/StateChip.tsx` | Tone-coded status labels |
| `CodeBlock` | `CodeBlock.tsx` | Shiki-powered syntax highlighted code |
| `ErrorBoundary` | `ErrorBoundary.tsx` | Fault isolation shell |

### 1.4 Data Flows

**Extension Host → Webview** (via `parseHostToWebviewMessage`):
- `voxStatus` — budget/provider data
- `gamifyUpdate` — orchestrator snapshot (agents, mesh)
- `workflowStatus`, `meshStatus`, `intentionMatrix`, `oplog`
- `capabilitiesUpdate` — MCP tool count, connection state, fingerprint
- `ludusProgressSnapshot` — Ludus XP, level, achievements, notifications
- `chatHistory`, `chatMeta`
- `budgetHistory`, `modelList`
- `composerState`, `inspectorState`

**Webview → Extension Host** (via `vscode.postMessage`):
- `submitTask`, `composerGenerate/Apply/Discard`
- `agentPause/Resume/Drain/Retire`
- `rebalance`, `resumeWorkflow`
- `setSocratesGate`, `rejectExecution`
- `pickModel`, `setModel`, `updateApiKey`, `updateBudgetCap`
- `ludusAckNotification`, `ludusAckAllNotifications`
- `browserOpen/Navigate/Extract/Screenshot`
- `planGoalPreview`, `repoQueryText`, `contextSetValue`, `projectInit`

### 1.5 Gamification (Ludus) — Current State

Currently surfaced in:
1. `UnifiedDashboard` — KPI strip (events, XP, crystals, streak) and notification list
2. `SidebarProvider.ts` — `maybePushLudusSnapshot()` throttled at 3s minimum interval
3. Controlled by `ConfigManager.gamifyShowHud` (config: `vox.gamify.showHud`)

The HUD was previously a separate flyout. It's partially integrated into the Dashboard but lacks:
- Persistent level/XP status embedded in the nav rail or header
- Achievement toast integration
- Quest stream integration
- Prestige visual effect hooks

### 1.6 Existing Execution Mode Visual Language

| Mode | Color | Animation |
|---|---|---|
| Efficient | `#4ADE80` (green) | 800ms linear draw |
| Fast | `#EF4444` (red) | 250ms burst + ember spark |
| Verbose | `#60A5FA` (blue) | Breathing cloud, 2s draw |
| Precision | `#A78BFA` (violet) | Convergent focus, heartbeat pulse |

Node states: Completed (emerald), Failed (rose + shake), Cancelled (grey dashed), Blocked (amber pulse).

archived_date: 2026-04-18
---

## 2. Target Aesthetic: Industrial Cyber-Renaissance

### 2.1 Inspiration Source

The Vox hero banner image establishes the design language: a central glowing steampunk orb
("VOX") flanked by tarnished copper machinery on the left (circuit boards, gears, pipes,
cyan terminal text) and a holographic glass display on the right (clean UI charts, sans material).

**Aesthetic Classification**: "Industrial Cyber-Renaissance" / Retro-Futuristic

**Comparable universes**: Deus Ex (gold-tinted cyberpunk), Thief (gritty clockpunk grime),
mixed with holographic UI (Ghost in the Shell, Cyberpunk 2077 terminal interfaces).

**Subliminal message**: Bare-metal engineering foundation + sleek cutting-edge developer experience.

### 2.2 Design System Tokens

#### Color Palette

```css
:root {
  /* The Void — Backgrounds */
  --vox-bg-void:     #0D1117; /* Deepest background, editor area */
  --vox-bg-machine:  #1A1A1D; /* Gunmetal Gray, sidebars/panels */
  --vox-bg-surface:  #22252A; /* Card surfaces */
  --vox-bg-elevated: #2A2D33; /* Dropdowns, tooltips */

  /* The Machinery — Structural */
  --vox-brass:       #B5A642; /* Tarnished Brass — card borders, dividers */
  --vox-copper:      #B87333; /* Oxidized Copper — nav rail, active borders */
  --vox-steel:       #6B7280; /* Brushed Steel — muted text, icons */

  /* The Logic — Functional/Code */
  --vox-cyan:        #00FFFF; /* Electric Cyan — code, links, active states */
  --vox-cyan-dim:    #00BFBF; /* Dimmed Cyan — hover, secondary accents */
  --vox-cyan-glow:   rgba(0, 255, 255, 0.15); /* Cyan glow background */

  /* The Core — Brand */
  --vox-amber:       #FFBF00; /* Incandescent Amber — CTAs, logo, XP */
  --vox-amber-dim:   #CC9900; /* Dimmed Amber — hover states */
  --vox-amber-glow:  rgba(255, 191, 0, 0.15); /* Amber glow background */

  /* Status Colors (adjusted for the palette) */
  --vox-success:     #4ADE80; /* Execution: Efficient */
  --vox-danger:      #EF4444; /* Execution: Fast / errors */
  --vox-info:        #60A5FA; /* Execution: Verbose */
  --vox-precision:   #A78BFA; /* Execution: Precision */
  --vox-warning:     #F59E0B; /* Blocked states */
}
```

#### Typography

```css
@import url('https://fonts.googleapis.com/css2?family=Rajdhani:wght@400;600;700&family=JetBrains+Mono:wght@400;700&family=Inter:wght@400;500;600&display=swap');

:root {
  --font-display: 'Rajdhani', 'Inter', system-ui;    /* Section headers, nav labels */
  --font-body:    'Inter', system-ui;                 /* Body text, UI labels */
  --font-mono:    'JetBrains Mono', 'Fira Code', ui-monospace; /* Code, telemetry, logs */
}
```

**Notes on Rajdhani**: Industrial-geometric feel, works well at small sizes in VS Code sidebar.
Fallback to Inter Bold for contexts where Rajdhani is unavailable.

**Avoid Orbitron** in the sidebar — too wide, poor readability at 10–12px. Reserve for
full-width canvas sections (MeshTopology header, IntentionMatrix title).

#### Glow Effects

```css
/* Cyan neon glow (code, links, active state borders) */
.glow-cyan {
  box-shadow: 0 0 6px rgba(0,255,255,0.4), 0 0 20px rgba(0,255,255,0.15);
}
.text-glow-cyan {
  text-shadow: 0 0 8px rgba(0,255,255,0.6);
}

/* Amber glow (brand, XP, CTAs) */
.glow-amber {
  box-shadow: 0 0 6px rgba(255,191,0,0.4), 0 0 20px rgba(255,191,0,0.15);
}

/* Brass structural borders */
.border-brass {
  border-color: var(--vox-brass);
  box-shadow: inset 0 1px 0 rgba(181,166,66,0.2);
}
```

#### Glassmorphism (Holographic Panel)

```css
.vox-glass {
  background: rgba(26, 26, 29, 0.75);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border: 1px solid rgba(0, 255, 255, 0.12);
  box-shadow: 0 0 20px rgba(0, 255, 255, 0.04),
              inset 0 1px 0 rgba(255, 255, 255, 0.03);
}
```

#### Mechanical Corner Treatment

Instead of soft `border-radius: 0.75rem` everywhere, use a mix:
- **Cards/panels**: 4px radius with chamfered visual hint (pseudo-element or clip-path)
- **Buttons**: 2px radius (sharp, mechanical) with brass border on action items
- **Input fields**: 0px radius (terminal feel) with cyan bottom border on focus
- **Nav rail items**: 4px radius, copper-tinted active state

---

## 3. Proposed Layout Architecture

### 3.1 Current Weaknesses

1. **3-tab model is too coarse** — Chat, Dashboard, Diagnostics collapses too many surfaces into 3
2. **Gamification is second-class** — Ludus lives in a small KPI strip in Dashboard, no persistent
   presence showing the user's journey
3. **Model selection is hidden** — gear icon → VS Code quick pick; no visual context of current model
4. **MeshTopology is buried** — it's a full-height ReactFlow canvas but unreachable unless on Dashboard tab and the topology data exists
5. **No persistent orchestrator status** — the `execHint` strip is monospace text, hard to parse
6. **Chat has no visual identity** — no indication of which model, what budget remains, Socrates gate state in context

### 3.2 Proposed New Navigation Model

```
┌─────────────────────────────────────────────────┐
│ ┌──┐  VOX                  [Model Pill] [XP Bar] │  ← Header strip (if space allows)
│ └──┘                                             │
├────┬────────────────────────────────────────────┤
│ 💬 │                                            │
│ 🔮 │   Main Content Area                       │
│ 📡 │                                            │
│ 🧪 │                                            │
│    │                                            │
│ ─── │                                           │
│ ⚙️ │                                            │
│ [V] │  ← Level badge / XP glow ring             │
└────┴────────────────────────────────────────────┘
```

**Tab proposal** (4 nav items instead of 3):
1. **Commune** (💬) — Chat & Composer (current "chat" tab, redesigned)
2. **Sanctum** (🔮 or 🌐) — Unified orchestrator dashboard: live ops stream, agent cards, mesh preview, inline Ludus KPI
3. **Nexus** (📡) — Mesh visualization (full ReactFlow canvas — promoted from buried sub-section)
4. **Crucible** (🧪) — Engineering Diagnostics: tasks DAG, intention matrix, AST, context explorer

**Bottom of nav rail**:
- Settings gear → opens model picker / preferences sub-panel inline
- **"V" Orb** — the level badge (circular XP progress ring in amber/brass glow, glows on level-up)

### 3.3 Gamification Integration Strategy

Instead of a separate flyout, Ludus becomes ambient:

1. **"V" Orb (nav rail bottom)** — circular amber progress ring around the Vox logo pill.
   Shows level, XP to next level as ring fill. Click → expands inline quest/achievement panel.

2. **Sanctum tab** — top strip shows: `[⚡ XP: 12,450] [🏆 Level 42 — Architect] [🔥 3 day streak]`

3. **Achievement toasts** → micro-animation overlay (blossom burst from nav rail V orb, 800ms)
   using Framer Motion, non-intrusive

4. **Quest stream** → shown in Sanctum as a collapsible "Active Quests" accordion section

### 3.4 Model Selector Surface

Replace gear icon + VS Code quick pick with:
- **Persistent model pill** in the header or chat area: `[⚡ gemini-2.0-flash] [fast|reason|creative]`
- Clicking opens an **inline dropdown panel** (not VS Code quickpick) with:
  - Task-based categories (Speed, Reasoning, Creative)
  - BYOK key management
  - Budget cap slider

archived_date: 2026-04-18
---

## 4. v0.dev Workflow Strategy

### 4.1 What v0.dev Produces

v0.dev generates **React + TypeScript + Tailwind CSS + shadcn/ui** components. These assume:
- Next.js App Router (RSC + client components)
- Tailwind CSS (via PostCSS)
- shadcn/ui component library (`@radix-ui/*`, `class-variance-authority`, `clsx`)
- Standard Node.js browser environment

### 4.2 Adaptation Requirements for VS Code Webview

| v0.dev Default | VS Code Webview Requirement | Adaptation |
|---|---|---|
| Next.js runtime | Static iframe (CSR only) | Remove all `next/*` imports, server components, RSC |
| `"use client"` directives | Not needed (all client) | Strip safely |
| `next/image` | Not available | Replace with `<img>` |
| `next/link` | Not available | Replace with `<button onClick>` or `<a>` |
| Server actions / API routes | vscode.postMessage bridge | Wire all data to `vscode.postMessage` events |
| Tailwind via PostCSS | esbuild (no PostCSS) | Run `tailwindcss` CLI separately (see §4.3) |
| shadcn/ui | Must be manually included/inlined | Copy component files directly into `webview-ui/src/components/ui/` |
| Standard CSS vars | Must map to `--vscode-*` or use fixed dark theme | See §4.4 |

### 4.3 Adding Tailwind CSS to the Build

The current `esbuild.js` does not support PostCSS. Recommended approach:

```jsonc
// package.json scripts addition
"build:css": "tailwindcss -i webview-ui/src/input.css -o out/webview.css --minify",
"build:js": "node esbuild.js",
"compile": "npm run build:css && npm run build:js",
"watch:css": "tailwindcss -i webview-ui/src/input.css -o out/webview.css --watch",
```

Tailwind config `content` must include `webview-ui/src/**/*.{tsx,ts}`.

The `_getHtml()` in `SidebarProvider.ts` already loads `out/webview.css` via:
```ts
const styleUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'out', 'webview.css'));
```
This works immediately once the Tailwind build outputs there.

### 4.4 Theming Strategy: Fixed Dark Theme vs. VS Code Token Mapping

Two viable options:

**Option A — VS Code Token Mapping (current approach, extended)**
- Map new design tokens to `--vscode-*` CSS variables
- Pros: works in light themes, adapts to user themes
- Cons: VS Code themes don't have brass/copper/cyan tokens; must approximate

**Option B — Fixed Industrial Dark (new approach)**
- Use hardcoded design tokens (the palette above)
- Override `--vscode-*` variables to point to our tokens
- Lock theme to "always dark" regardless of VS Code theme
- Pros: guarantees the Industrial aesthetic
- Cons: some VS Code users use light themes; extension will always appear dark

**Recommendation**: Option B with graceful override — define our tokens as CSS custom properties on
`:root`, then map the `--vscode-*` variables that our components use to those tokens. Users who
want a light VS Code theme will have a dark sidebar, which is actually common (developers often
prefer secondary panels dark even in light IDE setups).

### 4.5 v0.dev Prompting Strategy

The key to usable output is decomposed, well-specified prompts. Recommended prompt structure:

```
Component: [Name]
Stack: React 19, TypeScript, Tailwind CSS, shadcn/ui, framer-motion, lucide-react
Environment: VS Code Webview sidebar (320–400px width, full height, no URL routing)
Theme: Industrial Cyber-Renaissance. Dark backgrounds (#0D1117, #1A1A1D). 
       Tarnished brass borders (#B5A642). Electric cyan accents (#00FFFF) with glow.
       Incandescent amber (#FFBF00) for brand/XP. Glassmorphism panels.
       Mechanical corners (2–4px radius, not rounded-xl). JetBrains Mono for code.
       NO: next/*, server components, API routes, routing, browser fetch

Data source: All data flows from window.addEventListener('message', ...) events.
  Outbound: vscode.postMessage({type: '...', ...})

[Component-specific spec]
```

**Recommended component decomposition for v0.dev prompts**:
1. App shell + nav rail (4 tabs + XP orb at bottom)
2. Chat panel with streaming message bubbles, model pill, composer toggle
3. Sanctum dashboard (op stream cards, agent status cards, Ludus KPI strip)
4. Gamification widget (XP ring, level badge, quest accordion, achievement toast)
5. Model selector inline panel
6. Mesh topology node card design (custom React Flow nodes)
7. Intention matrix grid (Socrates gate)
8. Budget/telemetry history sparkline card

### 4.6 What NOT to Use v0.dev For

- ReactFlow custom nodes (do manually — need VS Code postMessage wiring)
- WorkflowScrubber (complex state, keep hand-rolled)
- Extension host TypeScript (`SidebarProvider.ts`, protocol, commands)
- ContextExplorer (too many VS Code-specific interactions)

---

## 5. Design Principles (Research-Derived)

### 5.1 From AI Orchestrator Dashboard Research

1. **The Cockpit Model**: Surface only mission-critical info in primary view; diagnostic detail is
   one drill-down away (never zero, never infinite).

2. **5-Second Rule**: Agent count, orchestrator state, last error, budget — visible without
   scrolling in Sanctum.

3. **Information Hierarchy** (top to bottom):
   - Tier 0 (always visible): Model pill, Socrates gate, MCP status, XP orb
   - Tier 1 (Sanctum tab): Ops stream, agent cards, pipeline health, Ludus KPI
   - Tier 2 (Nexus tab): Full mesh topology
   - Tier 3 (Crucible tab): Task DAG, intention matrix, AST, context keys

4. **Trust-Centric**: Confidence scores, Socrates risk level, model used — always shown.

5. **Human-in-the-Loop**: Agent pause/resume/drain/retire must be 1-click from the agent card,
   not buried behind AgentFlow canvas panel.

### 5.2 From Gamification UX Research

1. **Ambient, Not Intrusive**: Level progress is always visible (XP orb); achievements are
   non-blocking toasts (800ms bloom burst), not modals.

2. **Contextual Integration**: Quest items that map to current code health (TOESTUB, debt counters)
   feel more meaningful than abstract XP farms.

3. **Respect Flow State**: Option to minimize gamification elements; `vox.gamify.showHud` config
   must still work.

4. **Collective not Individual**: Emphasis on session streaks, workspace milestones — not
   competitive leaderboards.

### 5.3 From Agent-to-Agent Visualization Research

1. **Graph + Stream Dual View**: Node-link graph (Nexus) for spatial understanding + event stream
   (Sanctum ops log) for temporal understanding. Both needed.

2. **Trace Everything**: A2A tasks should show source agent → target agent arrows in Nexus.

3. **Semantic Edges**: Different edge colors/animations per execution mode (already implemented,
   must survive redesign).

4. **NodeToolbar**: Pause/Resume/Drain/Retire controls on node hover (ReactFlow NodeToolbar)
   instead of the current side panel.

### 5.4 From Model Selector UX Research

1. **Use-case labels over model names**: "Fast", "Reasoning", "Creative" → show model name as
   secondary metadata. Current `chatProfile` state already supports this.

2. **Transparent cost/speed**: Each profile shows latency tier indicator + cost indicator ($ $$).

3. **Streaming state clarity**: Visually distinguish "thinking" (reasoning model chain-of-thought)
   from "streaming" (token output).

### 5.5 From Inline Gamification Research

1. **Circular progress ring around V orb**: Most space-efficient XP representation for the narrow
   rail (compact, works at 32px).

2. **Slim linear XP bar**: As an alternative/addition in the chat header (1px height, amber fill).

3. **Milestone "pip" indicators**: Row of 5 hexagonal pips in Sanctum header → fills as daily tasks complete.

archived_date: 2026-04-18
---

## 6. v0.dev Code Conversion Checklist

When code arrives from v0.dev, apply these transformations:

### Remove
- [ ] `"use client"` directives (entire file is client-side)
- [ ] `import { ... } from 'next/*'`
- [ ] Server actions (`async function serverAction() {}` pattern)
- [ ] `<Link href="...">` → replace with `<button onClick={() => setActiveTab(...)}>` 
- [ ] `<Image ...>` from `next/image` → replace with `<img>`
- [ ] `useRouter()`, `usePathname()` → replace with local tab state
- [ ] Any `fetch()` calls → replace with `vscode.postMessage` + message listener

### Keep
- [ ] All Tailwind utility classes (after building CSS via CLI)
- [ ] shadcn/ui component files (copy to `webview-ui/src/components/ui/`)
- [ ] framer-motion animations
- [ ] lucide-react icons
- [ ] TypeScript types

### Add
- [ ] `const vscode = getVsCodeApi();` at component top
- [ ] Appropriate `vscode.postMessage({type: '...'})` calls
- [ ] Message receiver hook where component subscribes to state updates
- [ ] VS Code theme mapping overrides for any hardcoded light-mode colors

### Verify
- [ ] No `document.location`, `window.history`, or `window.fetch` usage
- [ ] No external CDN script loads (violates CSP)
- [ ] Any `@radix-ui/*` imports are bundled by esbuild (add to `package.json` if missing)
- [ ] `clsx`, `class-variance-authority`, `tailwind-merge` present in `package.json`

---

## 7. Component-by-Component Redesign Notes

### Chat / "Commune" Panel

**Current pain points**:
- Session ID input feels like a debug field, not user-facing
- Profile selector (fast/reasoning/creative) is an HTML `<select>`, not visually branded
- No stop-generation button
- No visible streaming indicator
- Composer toggle is a small text button, easy to miss

**Redesign targets**:
- Header bar: `[Model Pill ▾] [Profile: ⚡ Fast | 🧠 Reason | ✨ Create] [💰 $0.03]`
- Message bubbles: User = right-aligned amber-border glass card; Agent = left-aligned cyan-border glass card
- Streaming indicator: Animated cyan dots + "Vox is reasoning..." text
- Stop button: Red X overlaid on streaming message
- Composer: Sticky bottom section that slides up, not a toggle button

### Sanctum / Dashboard Panel

**Current pain points**:
- 12-column grid works, but op-stream items lack visual hierarchy
- Pipeline Health is just an icon; no history or progress
- Ludus KPI strip is too compact and lacks meaning for newcomers
- No agent cards showing live state

**Redesign targets**:
- Agent cards: Compact cards per active agent (name, queue depth, execution mode indicator, pause button)
- Op stream: Rows with amber timestamp, cyan op-type label, agent moniker, status chip
- Left 60%: Op stream | Right 40%: Agent cards (stacked) + Pipeline health
- Bottom sticky: Ludus KPI ribbon (XP bar, streak flames, crystal count, level badge)
- Quest accordion: `[⚔️ Active Quests ▾]` expands to show 2–3 active technical debt quests

### Nexus / Mesh Tab (NEW — Promoted)

**Current pain points**:
- `MeshTopology.tsx` is only visible when `meshStatus` data exists AND user is on Dashboard
- Full ReactFlow canvas is wasted in the small 4-column right side of Dashboard

**Redesign targets**:
- Full-height dedicated tab
- Custom node styling: copper/brass tones for nodes, ceramic borders for primary nodes
- Animated edges: Electric cyan websocket links, brass-colored HTTP links
- NodeToolbar on hover: `[Inspect] [Drain] [Migrate]`
- Legend in top-left: Shows node type icons, connection protocol key
- Add `colorMode="dark"` prop to ReactFlow

### Crucible / Engineering Diagnostics Tab

**Current pain points**:
- EngineeringDiagnostics.tsx is a container delegating to sub-components, but the sub-tabs
  (AgentFlow, IntentionMatrix, WorkflowScrubber, ContextExplorer) are accessed via buttons,
  not a clean sub-navigation

**Redesign targets**:
- Sub-nav horizontal pill bar: `[Agent Flow] [Intentions] [Time Travel] [Context] [AST]`
- AgentFlow: Add NodeToolbar with lifecycle controls on node hover
- IntentionMatrix: Replace grid with compact confidence bar rows (more scannable)
- WorkflowScrubber: Visual timeline track (like a media player scrub track)

archived_date: 2026-04-18
---

## 8. Implementation Plan Prerequisites (Open Questions)

The following questions must be resolved before beginning the formal implementation plan.
See the clarifying questions section of the design research artifact for the full list.

1. Navigation paradigm (4 tabs vs. other schemes)
2. Tailwind CSS addition approval
3. Theme locking (fixed dark vs. VS Code token mapping)
4. Gamification persistence scope
5. Model selector surface location
6. Nexus tab scope (full ReactFlow vs. summary card)
7. v0.dev component priority list
8. shadcn/ui adoption scope

---

## 9. Web Research Summary

| Topic | Key Finding |
|---|---|
| v0.dev adaptation | Strip Next.js; keep React/Tailwind/shadcn; wire data via postMessage |
| VS Code webview patterns | CSP nonce required; `--vscode-*` CSS vars; esbuild static bundle |
| Industrial Cyber-Renaissance palette | Void blacks, brass/copper structure, cyan logic, amber brand |
| Earthy dark UI | 2025-26 trend toward "desert ochres" and warm terracotta — somewhat applicable |
| Gamification inline | Circular ring XP, slim progress bars, ambient toasts — NOT modals |
| AI orchestrator dashboard | Cockpit model: critical state in 5s, drill-down to detail |
| A2A visualization | Graph + telemetry stream dual view; NodeToolbar for per-agent actions |
| React Flow dark theme | Use `colorMode="dark"` + `NodeToolbar` + ELKjs for auto-layout |
| Model selector UX | Use-case labels (Fast/Reason/Creative) + transparent cost/speed |
| Tailwind + esbuild | Use Tailwind CLI separately; output CSS to `out/` before esbuild run |
| shadcn + pure CSR | Set `"rsc": false`; remove Next.js deps; all components work as plain React |
| Cyberpunk CSS | Multi-layer box-shadow glow; `repeating-linear-gradient` scanlines; `augmented-ui` for 45° clips |
| v0.dev prompting | Three-input: Product Surface + User Context + Technical Constraints; iterate by component |

archived_date: 2026-04-18
---

*Document created: 2026-04-04*
*Status: Research complete — awaiting clarifying questions answers before implementation plan*


