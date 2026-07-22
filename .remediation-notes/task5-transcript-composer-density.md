# Task 5 — Chat transcript/composer panel density audit + fix

## Audit method

Real-browser (Chromium via Playwright) render against the live Vite dev
server, not jsdom (jsdom performs no real layout — screenshots/bounding
boxes are meaningless there). A throwaway script (not committed) mounted the
real app shell with a mocked `chat_get_messages` response: 16 realistic
user/assistant exchanges (32 messages, mixed short replies and one long
multi-paragraph assistant response) for the active session, plus a real
`attention_budget` snapshot decoded via `@msgpack/msgpack` into
`get_orchestrator_status_bin` (matching `useOrchestratorStatus`'s real
decode path). Viewport 1400x900, so the transcript panel sat at its real
~460px-plus dockview-constrained width per this session's earlier fix.

## Finding

With a realistic 32-message conversation, the transcript panel showed only
the first ~1.5 exchanges before scrolling was needed — most of the panel's
vertical space was consumed by fixed-height chrome below the transcript:
the ATTENTION BUDGET card (~131px: header row + 8px progress bar + two
caption paragraph lines + 1rem padding) and the composer's own controls,
unconditionally rendered at that footprint regardless of whether the
conversation above it was empty or long and active.

Screenshot evidence (scratchpad, not committed):
`task5-before-full.png` / `task5-before-transcript.png`.

## Fix

Made `AttentionBudgetMeter` (`crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.tsx`)
collapsible:
- New `defaultCollapsed` prop, tracked reactively via `useEffect` (not just a
  `useState` initializer) — necessary because `attention_budget` typically
  arrives from the orchestrator status stream before the session's messages
  finish hydrating, so a mount-only initializer permanently missed the
  "now there's a real conversation, collapse" transition (caught via live
  verification below, not by the jsdom unit tests alone).
- Collapsed renders a single ~28-36px summary row (focus label + pct/minutes
  + toggle) instead of the ~110-131px full card; `role="meter"` and its
  `aria-value*` attributes are unchanged in both states.
- Once the user manually toggles, their choice wins over further
  `defaultCollapsed` prop changes for that mount (`userToggledRef`).

`ChatSurface.tsx` computes `defaultCollapsed` as: conversation has more than
a few messages (`messages.length > 4`) AND nothing urgent needs the full
card up front (no waiting questions, no blocked tasks, spend ratio < 80%).

CSS: `crates/vox-gui/ui/src/index.css` — `.attention-budget-meter[data-collapsed="true"]`
variant + `__summary`/`__toggle`/`__bar--compact` rules.

## Verification

- RED: `npx vitest run src/components/surfaces/AttentionBudgetMeter.test.tsx`
  — 2 new tests failed before the component supported `defaultCollapsed`.
- GREEN after implementation: same file, 8/8 passed (6 original + 2 new).
  A third regression test (prop-transition-after-mount) and a fourth
  (user-toggle-wins-over-later-prop-changes) were added after the live
  verification below caught the mount-only-initializer bug; all 8 pass.
- Full suite: `npx vitest run` — 232 files / 1152 tests passed.
- `npx tsc --noEmit` — clean.
- Live verification (real Chromium, not jsdom): before the reactive-effect
  fix, the card stayed expanded in the live app despite `messages.length`
  being 32 (bounding-box height 131px) — confirming the mount-only-`useState`
  bug in practice, not just in theory. After the fix, same scenario:
  bounding-box height dropped to 68px (screenshot: one-line "63% · 150/240M
  Focused ▾" row), with visibly more transcript content above the fold.

## No commit yet

Source changes are staged for a project commit
(`feat(gui): compact the Chat transcript/composer panel for realistic
conversation density`) alongside this note.
