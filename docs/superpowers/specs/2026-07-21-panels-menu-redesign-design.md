---
title: Panels menu redesign — checkboxes, activation-order, color blend, content audit
status: approved
---

# Panels menu redesign — design

## Context

Live testing surfaced four related requests about the Panels ▾ dropdown in `ChatSurface.tsx`:

1. Replace the current "click a button, dropdown closes, click Panels again to add the next one" flow with checkboxes so several panels can be turned on in one open-dropdown session.
2. New panels should append in activation order (most-recently-activated at one end), not sit at a fixed position from `panelDefs`'s hardcoded `referenceChain`.
3. The dockview workspace should fill the full available width/height of the Chat tab's content area, adaptively.
4. The panel background reads as a visibly different dark-blue from the rest of the app; it should blend in. Separately, audit each already-wired panel's actual content against docked-panel width and rearrange/trim where it doesn't fit well.

User confirmed via question: checkboxes apply **live** (check → panel opens immediately, dropdown stays open for more selections), and re-checking a previously-closed panel always moves it to the **most-recently-activated** end of the order (not back to its old spot).

## Approach

### 1. Checkboxes, live-apply

Replace each "Add" button (`<button onClick={() => addDefaultPanel(...)}>`) with a `<input type="checkbox">` + label, checked state driven by `!!dockApiRef.current?.getPanel(id)`. `onChange` calls `addDefaultPanel(api, id)` when checked→newly-checked, or the existing close path (`api.getPanel(id)?.close()` — need to confirm this exact method name against `IDockviewPanel`, likely just `panel.close()`) when unchecked. The dropdown does **not** close on toggle — only Escape/outside-click/Reset-layout close it, matching "keep it open for more selections."

Core panels (Sessions/Chat/Execution/Flow/To-dos) also get checkboxes in the same list, for a single unified control surface, rather than the current split "reopen core panels" / "Add opt-in panels" two-section layout.

### 2. Activation-order positioning

Replace `panelDefs[id].referenceChain` (a fixed, hand-authored fallback chain per panel) with a **live activation-order list**: a `useRef<string[]>([])` tracking panel ids in the order they were turned on, appended to on every checkbox-driven open, with existing entries removed and re-appended (moved to the end) on re-activation. `addDefaultPanel` positions each new panel with `referencePanel` = the *previous* entry in that activation-order list (or the anchor panels — sessions/transcript — if the list is empty), so panels literally queue up left-to-right (or whichever direction the group's default split direction is) in the order the user turned them on. `closedPanelIds`-equivalent tracking already exists for core panels; the activation-order list becomes the new single source of truth for opt-in panel positioning, replacing `referenceChain`.

### 3. Full adaptive width/height

The dockview shell already fills its immediate container via the `h-full`/pixel-height fix landed this session. "Full length and width... adaptively" most likely means the *outer* `chat-surface-layout` container's own `min-h-[60vh]` floor (and whatever width constraint exists above it) should become a true `h-full`/flex-1 fill of the Chat tab's entire content region, not just a 60vh-tall island within a scrollable page. Verify the current outer container chain and remove any remaining height/width ceiling that isn't already resolved by the pixel-height fix.

### 4. Color blend — use the real token, not a hand-tuned guess

`dockview-vox.css`'s `--dv-background-color: rgba(9, 9, 11, 0.92)` is a separately hand-typed value close to, but not identical to, the app's real background token `--color-bg-base: #0c0e10` (confirmed in `tokens.generated.css`). The small numeric mismatch plus the 0.92 alpha compositing differently over different layers creates a visible seam. Fix: reference `var(--color-bg-base)` directly (opaque, no separate alpha) for `--dv-background-color` and any other dockview background var that should match the app chrome exactly, eliminating the seam by construction rather than by re-tuning another guessed rgba value.

### 5. Content audit — apply the proven condensed/full pattern more broadly

The 6 "strong" Phase 2 panels (Needs You, Search Index, Discovery, Repository, Mercatus, Harness) currently wrap their *entire* top-level component 1:1 with no width-awareness — this was correct per the original audit (they were judged narrow-tolerant), but live screenshots with several panels open simultaneously show real cramping (e.g. Repository's isolation-panel button grid, Mercatus's full price matrix table) that the original single-panel-width audit didn't anticipate for the *many-panels-open-at-once* case. Approach: reuse the width-driven toggle mechanism already built and proven for Approvals (`props.api.width`/`onDidDimensionsChange`) — each of the 6 panels gets its own real threshold (re-derive per-panel from the original audit's own minimum-width numbers, already documented: Mercatus ~320-360px, Repository ~260-300px, etc.) below which it shows a condensed summary instead of the full component. This is not new design work — it's applying an already-approved, already-tested mechanism to more surfaces, using numbers this session already measured.

## What this does not include

- The 8 "condensed-capable" Phase 3 surfaces not yet wired (mesh, tasks, coderabbit, skills, gamify, models, memory) — unaffected by this spec, still tracked by the existing `2026-07-21-universal-dock-workspace.md` plan's Phase 3.
- True drag-and-drop reordering beyond what dockview's native drag already provides — activation-order only controls *initial* position when a panel is turned on via the checkbox list; a user can still drag any panel elsewhere afterward, same as today.

## Testing

Same TDD discipline as every task this session: failing test first, confirm the failure reason, implement, confirm pass. Live verification via the CDP screenshot/DOM-inspection technique established this session (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`), since layout/color/width-threshold behavior is exactly the class of bug jsdom cannot detect — this session already found two real bugs (blank Chat surface, wrong panel titles) that 100%-passing jsdom suites missed entirely.
