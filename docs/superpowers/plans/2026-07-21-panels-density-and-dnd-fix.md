# Panels density, tab-sizing, and drag-and-drop fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix a real drag-and-drop-to-reorder regression in the Chat dock's tab strip, stop tabs from stretching to fill wasted horizontal space, remove any remaining dockview-native collapse/expand affordance the user is still seeing, and redesign the Sessions and Chat panel content (plus, by the same pattern, every other opt-in panel) into a compact, left-gutter, stacked-list layout that looks right once populated with realistic (non-empty) data — not just in today's near-empty dev state.

**Architecture:** All work is in `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` and the individual panel-content components it wraps (`ChatSessionRail.tsx`, transcript/composer, `Mercatus.tsx`, `RepositoryView.tsx`, `NeedsYouSurface.tsx`, `VoxGraphStatusPanel.tsx`, `DiscoverySurface.tsx`, `ApprovalsView`-backed panel), plus `crates/vox-gui/ui/src/styles/dockview-vox.css` for tab-strip sizing. No new dependencies — dockview 6.6.1 (already installed) and Tailwind (already used throughout).

**Tech Stack:** React + TypeScript, dockview-core/dockview-react 6.6.1, Tailwind, Vitest + Testing Library, Tauri (for the WebView2 CDP live-verification technique used throughout this session).

---

## Investigation findings (grounding for Task 1-3, done before writing this plan)

- **No leftover collapse/expand button exists in this codebase's own components.** Grepped `ChatSessionRail.tsx`, `ChatSurface.tsx`, and every opt-in panel-content component for `collapse|chevron|expand` — the only hits are unrelated (`aria-expanded` on menus, a per-message detail toggle in `ChatAgentEventRow.tsx`, `border-collapse` CSS). The collapse-arrow removal from earlier today (commit `678e4e1f02`) is real and complete in the source.
  - **Working theory**: what the user is seeing live is dockview-core's own **built-in tab-group collapse/overflow chip** (`.dv-tab-group-chip`, `.dv-tab--group-collapsed` in `dockview-core/dist/styles/dockview.css:2938-3011`) — a native dockview feature that collapses overflowing tabs into a chip when a tab strip runs out of room, which the app has never explicitly disabled. Task 3 below is to confirm this diagnosis live and either configure it off (if dockview exposes that) or reskin it to fit the app rather than removing a feature users need when many tabs are open.
- **Tabs stretch to fill the tab strip width evenly, wasting space next to short labels.** `dockview-core/dist/styles/dockview.css:2755-2757` sets `.dv-tab { flex-shrink: 0; }` with no width rule at the base layer — the actual per-tab width comes from an inline style dockview-react computes at runtime (default "fill" sizing divides the strip evenly among tabs unless a fixed/content-sized mode is configured). This matches the user's "wastes more screen real estate ... to the right of whatever the tab is titled" complaint exactly: a short label like "FLOW" or "TO-DOS" sits inside a tab box sized the same as a long one. Task 2 investigates dockview's real sizing-mode option for this version and applies a content-width mode.
- **Drag-and-drop uses the browser's native HTML5 DnD path for mouse users** (`dockview-core`'s `dndCapabilities.js`: `html5: true` on fine-pointer devices), confirmed enabled with no `disableDnd` override anywhere in this codebase (verified by an earlier review agent this session). A CDP-driven synthetic mouse-event sequence could not engage it (expected — synthetic events don't satisfy the browser's "real user gesture" requirement for starting HTML5 DnD), so a prior verification pass could not confirm drag-and-drop live and fell back to code inspection. **The user has now directly reported it does not work in the actual running app for them, which is a stronger signal than the earlier inconclusive CDP result.** Task 1 re-investigates with fresh eyes, including checking for a WebView2-specific HTML5-DnD limitation (this is a known category of issue for Tauri/WebView2 apps — HTML5 drag events sometimes don't fire correctly inside WebView2 depending on OS/webview version), which the earlier pass didn't consider.

---

### Task 1: Diagnose and fix real drag-and-drop-to-reorder failure

**Files:**
- Investigate: `crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx`
- Investigate: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx:444-466` (the existing transcript-tab drag-prevention listener — a plausible interference point even though it's scoped only to the transcript tab)
- Test: `crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.test.tsx` or `ChatSurface.test.tsx` (whichever already has drag-related coverage — extend it)

- [ ] **Step 1: Reproduce and characterize the failure precisely**

This is a live-app bug report, not something jsdom can reproduce (jsdom has no real HTML5 DnD implementation at all — the existing tests for the transcript-tab drag-prevention listener work around this by testing the `dragstart` handler function directly, not real browser DnD). Before touching code:

1. Launch the real app (`cargo build -p vox-gui` if stale, then run `target/debug/vox-gui.exe` directly — no CDP needed for this step, a human or an agent with GUI-interaction tools should try an actual mouse drag).
2. If no direct GUI-interaction capability is available in your environment, use the CDP technique with `Input.dispatchMouseEvent` (mousePressed → several mouseMoved steps → mouseReleased) — this could not engage a real drag in the prior verification pass this session, but re-attempt it and specifically check: does `Input.dispatchDragEvent` (a distinct CDP command, not `dispatchMouseEvent`) work instead? CDP has dedicated drag-event dispatch (`Input.dispatchDragEvent` with `type: 'dragEnter'|'dragOver'|'drop'|'dragCancel'`) that doesn't require a prior "real gesture" the way HTML5 DnD-from-mouse-events does — try driving the drag via this dedicated API before concluding it's untestable.
3. Check WebView2's known behavior: search whether this project or its dependencies have any existing WebView2-specific DnD workaround/flag (grep the whole `crates/vox-gui` Rust tree and `src-tauri`-equivalent config for "drag", "dnd", "WEBVIEW2"). Tauri on Windows uses WebView2, which has had historical bugs/quirks with HTML5 `draggable` attribute drags specifically (as opposed to OS-level file drags) depending on the WebView2 runtime version — check the installed WebView2 runtime version on this machine and whether it's a known-affected one, if you can determine that.

- [ ] **Step 2: Identify root cause from Step 1's evidence**

Likely candidates to check, in order of likelihood given what's already been ruled out this session (no `disableDnd`, no DnD-restricting config in `DockWorkspaceShell.tsx`/`ChatSurface.tsx` found in this session's earlier review):
- A WebView2-level HTML5 DnD limitation (browser/webview issue, not app code) — if confirmed, the fix is dockview's `dndStrategy` option (check `dockview-core/dist/esm/dockview/options.d.ts` and `dndCapabilities.d.ts` for a way to force the `pointer`-based DnD backend instead of `html5`, since dockview supports both per the investigation already done this session — pointer-based DnD doesn't rely on the browser's native drag events and would sidestep a WebView2 HTML5-DnD bug entirely).
- The transcript-tab drag-prevention listener (`ChatSurface.tsx:444-466`) accidentally over-matching and suppressing drags on panels OTHER than the transcript — re-read this listener's target-matching logic carefully; confirm with a live test that dragging a non-transcript tab (e.g. Execution or Flow) is unaffected by it.
- Something about how tabs are sized (Task 2's fix) interfering with drag hit-testing (e.g. if tabs currently overlap due to stretched widths, drag start points could land on the wrong tab).

- [ ] **Step 3: Implement the fix**

Depends entirely on Step 2's finding — no placeholder here since the actual code change can't be known until the real cause is confirmed live. If it's a `dndStrategy` config change, it's a one-line prop addition to `DockviewReact` in `DockWorkspaceShell.tsx` (verify the exact prop name/shape against the installed `dockview-react` version's type defs before writing it — don't assume a prop name from a different major version's docs). If it's the transcript listener, narrow its matching logic and add a regression test asserting the listener's `dragstart` handler only calls `preventDefault()` when the event target really is the transcript tab, not any other tab (this class of test — calling the handler function directly with a constructed event/target — is exactly what the existing transcript-drag test already does; follow that pattern).

- [ ] **Step 4: Verify live**

Re-run whatever reproduction method Step 1 established (direct interaction if available, or CDP `Input.dispatchDragEvent`) and confirm a tab can now be dragged to reorder within its group, split right/left of another panel, and stack below another panel. Screenshot each successful outcome. If genuinely no reliable live-verification method exists in this environment even after trying `Input.dispatchDragEvent`, say so explicitly and rely on the root-cause fix being clearly correct by code inspection — do not claim "fixed" without at least one of these two forms of evidence.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "fix(gui): restore working drag-and-drop tab reordering/docking in the Chat dock"
```

---

### Task 2: Tabs should size to their content, not stretch to fill the strip

**Files:**
- Modify: `crates/vox-gui/ui/src/styles/dockview-vox.css`
- Modify (if a React-level prop is the real mechanism, not pure CSS): `crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx`

- [ ] **Step 1: Confirm the real sizing mechanism for dockview-react 6.6.1**

Read `node_modules/.pnpm/dockview-react@*/node_modules/dockview-react/dist/**/*.d.ts` (or wherever the installed version's types live — confirm the exact package/version first with `cat crates/vox-gui/ui/node_modules/dockview-react/package.json | grep version` or equivalent) for any tab-sizing-mode prop (e.g. something like `tabs: { sizing: 'fit' | 'fill' }` — do not assume this exact name exists, verify against the real type defs). Also check whether `dockview-core`'s `.dv-tab` width is actually set via an inline style (inspect a live DOM node via CDP `Runtime.evaluate` — `document.querySelector('.dv-tab').style.width` and `getComputedStyle`) to confirm whether this is JS-computed (needs a prop-level fix) or purely CSS-driven (a pure CSS fix in `dockview-vox.css` suffices, e.g. `.dv-tab { width: auto !important; min-width: ...; }` combined with sizing the `.dv-default-tab-content` to hug the text).

- [ ] **Step 2: Apply the real fix (CSS-only or CSS+prop, per Step 1's finding)**

Whichever mechanism Step 1 identifies, apply it so each tab is only as wide as its label + close button + reasonable padding require, with tabs left-aligned in the strip (not stretched to fill remaining width). Preserve the existing 22px tab-strip height and font-size/letter-spacing already set in `dockview-vox.css:29-40`.

- [ ] **Step 3: Verify live**

CDP screenshot of the tab strip with several panels open (e.g. Sessions/Chat/Execution/Flow/To-dos/Mercatus), confirming tabs are now sized to their labels with no dead space stretched to the right of short titles, and that there's no regression in click-target size (tabs shouldn't become so narrow they're hard to click — sanity-check a reasonable minimum, e.g. ~60-80px, doesn't need to be exact, use judgment).

- [ ] **Step 4: Run the suite and commit**

```bash
cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit
git add crates/vox-gui/ui/src/styles/dockview-vox.css
git commit -m "fix(gui): dock tabs size to their content instead of stretching to fill the tab strip"
```

---

### Task 3: Diagnose the "still-remaining collapse/expand button" report

**Files:**
- Investigate: live app, dockview-core's tab-group-chip feature (see Investigation findings above)

- [ ] **Step 1: Confirm live whether this is dockview's native overflow-chip feature**

Open the Chat tab with enough panels checked that the tab strip genuinely overflows (7+ opt-in panels), and screenshot the tab strip. Compare against `dockview-core`'s `.dv-tab-group-chip`/`.dv-tab--group-collapsed` CSS classes (`dockview-core/dist/styles/dockview.css:2938-3011`) — does a chip/collapsed-group indicator appear that could be mistaken for a leftover "expand/collapse" button? If yes, this confirms the working theory from this plan's Investigation section.

- [ ] **Step 2: Decide and implement based on Step 1's finding**

If it IS dockview's native overflow-chip: this is a legitimate, needed feature once many tabs are open (users need SOME way to reach overflowed tabs) — the fix is to **reskin it** to fit the app's visual language (brass/gold accents, matching the rest of `dockview-vox.css`) rather than remove it, since removing it would just reintroduce a different "tabs disappear with no way to reach them" bug. Add CSS rules for `.dv-tab-group-chip` and related classes to `dockview-vox.css` matching the existing token usage.

If Step 1 finds something else entirely (a genuinely different, not-yet-identified collapse/expand control): stop and re-investigate — do not guess a fix for a UI element that hasn't actually been located and confirmed.

- [ ] **Step 3: Verify live and commit**

Screenshot before/after. Run the full suite + tsc (this is CSS-only, so no test changes expected, but confirm no regression).

```bash
git add crates/vox-gui/ui/src/styles/dockview-vox.css
git commit -m "style(gui): reskin dockview's native tab-overflow chip to match the app's design tokens"
```

---

### Task 4: Redesign the Sessions panel — compact, left-gutter, stacked list

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.test.tsx`

- [ ] **Step 1: Read the current component fully and identify what "realistic filled" looks like**

Read `ChatSessionRail.tsx` in full. Find (or ask, if genuinely no test fixture exists) what a realistically-populated session list looks like — multiple sessions, varying title lengths, message counts, possibly timestamps/status indicators. The current near-empty dev-data view ("New chat, 7164 msgs" — one item) does not reveal how this will look with 10-20 real sessions. If the component already renders a `.map()` over a real sessions array, construct a test with 8-12 synthetic session objects (varied realistic titles like "Fix auth middleware", "Debug CI runner", not "test"/"test2") to visualize real density before making layout changes — this is the concrete content-audit step the design spec called for, applied to this panel specifically.

- [ ] **Step 2: Write the failing layout test**

Assert the redesigned structure: a left gutter/indent (e.g. a `pl-2` or a thin left border-accent per row, consistent with how other list-style UI in this codebase indicates hierarchy — grep for an existing left-gutter pattern to match, e.g. check `TasksView.tsx` or similar list components for precedent rather than inventing a new one), and a genuinely compact per-row height (current rows look like large 60-80px+ cards per the live screenshot; target something closer to a standard list-row height, ~32-40px, unless the content genuinely needs more).

```tsx
// Example shape — adapt exact assertions to the real DOM structure found in Step 1
it('renders sessions as a compact stacked list with a left gutter, not tall cards', () => {
  render(<ChatSessionRail sessions={manySessions} ... />);
  const rows = screen.getAllByTestId(/session-row/); // adjust selector to what exists
  const firstRowHeight = rows[0].getBoundingClientRect().height;
  expect(firstRowHeight).toBeLessThan(48); // adjust threshold to the agreed target once Step 1's audit is done
});
```

- [ ] **Step 3: Confirm RED, implement the compact/gutter redesign, confirm GREEN**

- [ ] **Step 4: Verify live**

CDP screenshot with several synthetic sessions (temporarily inject test data via the dev build if there's an existing seam for that, or use the live app's real session list if enough sessions already exist) showing the new compact stacked layout with left gutter.

- [ ] **Step 5: Run the suite and commit**

```bash
cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.test.tsx
git commit -m "feat(gui): redesign Sessions panel as a compact stacked list with a left gutter"
```

---

### Task 5: Redesign the Chat transcript/composer panel for compact density

**Files:**
- Modify: whichever component renders the transcript+composer inside `ChatSurface.tsx`'s `transcript` panel (identify exact file — likely `ChatSurface.tsx` itself inline, or a dedicated component it imports; read to confirm)
- Test: corresponding test file

- [ ] **Step 1: Audit with realistic filled content**

Same content-audit approach as Task 4: construct/find a scenario with a long, realistic conversation (10-20 exchanges, mixed user/assistant/tool messages, at least one long assistant response) rendered in the transcript panel at its real (now-dominant, ~460px+ minimum per this session's earlier fix) width. Screenshot it. Identify concretely what looks wasteful or oversized (e.g. the "ATTENTION BUDGET" card and composer controls in the live screenshots today take significant fixed vertical space even when not actively relevant — evaluate whether these should compress/collapse when there's an active long conversation, versus their current always-expanded footprint).

- [ ] **Step 2-5: Same TDD/verify/commit pattern as Task 4**, scoped to whatever concrete redesign Step 1's audit identifies (do not pre-specify the exact visual change here — Step 1 must ground it in a real rendered-with-realistic-data screenshot first, per this plan's own investigation discipline).

```bash
git commit -m "feat(gui): compact the Chat transcript/composer panel for realistic conversation density"
```

---

### Task 6: Apply the same compact-content-audit pass to the remaining opt-in panels

**Files:**
- Modify (as needed per-panel, one commit per panel touched): `Mercatus.tsx`, `RepositoryView.tsx`, `NeedsYouSurface.tsx`, `VoxGraphStatusPanel.tsx`, `DiscoverySurface.tsx`, the Approvals-backed panel component
- Test: corresponding test files

- [ ] **Step 1: For each panel, audit with realistic filled data**

Same method as Tasks 4-5: render each panel's FULL (non-condensed) view with realistic synthetic data — e.g. Repository with several real conflict entries, Needs You with several pending items, Mercatus with several parts/sources, Search Index with several corpora, Discovery with a populated timeline — and screenshot. Note this is explicitly what the earlier condensed/full toggle work (this session's Task 5 of the prior plan) did NOT evaluate, since it only checked the near-empty dev state.

- [ ] **Step 2: For each panel where the audit finds real problems (not just "could look nicer" — concrete: text overflow, awkward wrapping, wasted space, misaligned rows), fix them individually.**

One task/commit per panel that actually needs a change — skip any panel where the realistic-data audit shows it already looks fine (say so explicitly, don't force a change).

- [ ] **Step 3: Full-effort live verification**

Screenshot the whole Chat tab with all opt-in panels open simultaneously, populated with realistic (not empty) data across the board, confirming the overall "flow together" impression the user asked about — consistent density, no panel that looks jarringly different in polish from its neighbors.

```bash
cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit
```

---

## Self-Review

**Spec coverage** — every item from the user's request maps to a task: drag-and-drop still broken → Task 1; wasted space right of tab titles → Task 2; remaining expand/collapse buttons → Task 3; Sessions redesign (gutter, compact, stacked) → Task 4; Chat redesign (same treatment) → Task 5; "same deal" for all other menus/panels, content-realistic audit → Task 6.

**Placeholder scan** — Tasks 1, 4, 5, and 6 deliberately do NOT pre-specify the exact code change for their core fix, because the plan's own investigation found real uncertainty that can only be resolved by live reproduction (Task 1's DnD root cause) or a realistic-data screenshot audit (Tasks 4-6's redesigns) — pre-guessing either would violate this session's established discipline of grounding fixes in live evidence, not assumptions, which is why several agents this session were specifically instructed to verify claims independently rather than trust reports. This is different from a lazy "add appropriate handling" placeholder: each of these steps names exactly what evidence must be gathered and what decision procedure to apply once it's gathered.

**Type consistency** — no new shared types introduced by this plan; each task operates on an existing component's existing props/state.
