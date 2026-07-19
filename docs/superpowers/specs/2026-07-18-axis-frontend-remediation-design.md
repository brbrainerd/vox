---
title: Axis Frontend Remediation Design
date: 2026-07-18
status: approved
owner: gui
---

# Axis Frontend Remediation Design

**Source:** [`docs/superpowers/reviews/2026-07-18-axis-frontend-comprehensive-review.md`](../reviews/2026-07-18-axis-frontend-comprehensive-review.md)
— the ranked findings register (F-01–F-07) and the review's own recommended
remediation order. This spec turns that register into an implementable design.
It does not re-derive the findings; it assumes the review's evidence is
correct and focuses on root-causing and fixing each one.

**Scope:** Items 1–6 of the review's recommended order. **Explicitly out of
scope** (tracked as follow-up debt, not part of this effort):
- Item 7 (new component/e2e test coverage for `mercatus`, `publications`,
  `vox-search`, and the 21 surfaces with no dedicated e2e spec).
- The Tauri-shell (`tauri-driver`) spot check the review flagged as never
  having been run.
- The `settings`/keybinds blank-content-pane candidate the review noted in
  passing (per-surface table) but did not triage.

## Root-cause findings (from source inspection, this session)

Before design, each finding was traced to an actual file/line so the plan
below can cite exact edit sites rather than "find and fix somewhere":

- **F-01/F-04 (Firefox overlay occlusion/layout collapse):** the shared
  `Glass` primitive ([`src/components/ui/Glass.tsx`](../../../crates/vox-gui/ui/src/components/ui/Glass.tsx))
  applies `bg-overlay-subtle` (`rgba(255,255,255,0.04)` —
  [`src/styles/tokens.generated.css:51`](../../../crates/vox-gui/ui/src/styles/tokens.generated.css))
  combined with `backdrop-blur-2xl` as its only background. Chromium and
  Firefox composite a ~4%-alpha `backdrop-filter: blur()` layer differently;
  Firefox lets content underneath bleed through at full opacity, Chromium
  effectively darkens/opacifies it via the blur. Every occlusion cell in the
  review traces to a `Glass`-based full-panel overlay: `AchievementsDrawer`
  ([`src/components/gamify/AchievementsDrawer.tsx:38`](../../../crates/vox-gui/ui/src/components/gamify/AchievementsDrawer.tsx)),
  the compact-viewport session/execution rail overlays in `ChatSurface`
  ([`src/components/surfaces/Chat/ChatSurface.tsx:264,317`](../../../crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx)),
  and Console's `DiscoveryRail`
  ([`src/components/surfaces/Console/DiscoveryRail.tsx:99`](../../../crates/vox-gui/ui/src/components/surfaces/Console/DiscoveryRail.tsx)).
  `Popover.tsx` is NOT affected — it already uses `bg-zinc-950/95` (95% opaque),
  confirming the low-alpha background is specifically the defect.
- **F-02 (shared null-deref TypeError toast):** two distinct null-deref call
  sites in `App.tsx`, both feeding the same `pushToast(... body: String(err)
  ...)` pattern:
  - [`src/App.tsx:404`](../../../crates/vox-gui/ui/src/App.tsx) —
    `.then((s) => setActiveSessionId(s.session_id))` on the
    `chat_create_session` fallback path; if the backend/mock resolves the
    session as `null` (the empty-mock case), this throws
    `TypeError: Cannot read properties of null (reading 'session_id')` —
    matches the review's minified `"session_id, s is null"` signature exactly.
  - [`src/App.tsx:787-792,825-828`](../../../crates/vox-gui/ui/src/App.tsx) —
    `res.is_error ? ... : ...` after an MCP tool call, where `res` itself
    (not `res.is_error`) is null in the `error`-mock variant — matches
    `"is_error, res is null"`.
- **F-03 (`__TAURI_INTERNALS__` toast leak):** the same `String(err)` pattern
  used at ~15 call sites in `App.tsx` (e.g. lines 405, 408, 667, 682, 757,
  795, 831, 899, 910, 917, 924, 948, 1052) renders whatever the caught
  exception's `.toString()` produces, unfiltered, as toast body text. When the
  underlying error is a raw Tauri `TypeError`, its message contains
  `__TAURI_INTERNALS__` verbatim. `src/lib/backendGuard.ts`'s
  `BackendUnavailableError` already carries an honest, sanitized `.message`
  for the no-backend case — the toast path just isn't using it.
- **F-06 (blank simulation viewport):** `Dashboard.tsx:448-452` mounts
  `<LudusSandbox />` inside a fixed `h-[250px]` container with no
  loading/error/empty state of its own.
  [`src/components/gamify/LudusSandbox.tsx`](../../../crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx)'s
  canvas-draw effect silently bails (`if (!layout) return;` at line 92, and
  similar guards at lines 118/155/165/176/184/221/244/271) whenever `layout`
  is unset — which is exactly what happens when there's no backend/profile
  data — leaving a blank `<canvas>` with zero user-facing affordance.
- **F-07 (tablist ARIA violation):**
  [`src/components/layout/WorkbenchTabBar.tsx:24-39`](../../../crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx) —
  the `role="tablist"` container's direct children are plain `<div>` wrappers
  (line 32), each containing a `<button role="tab">` plus a close `<button>`.
  ARIA 1.2 requires `tablist`'s direct children to carry `role="tab"` (or
  `role="presentation"`/`"none"` if intervening) — the wrapping `<div>` has
  neither, which is exactly what axe's `aria-required-children` flags. This
  also explains the highest-frequency axe violation across the whole review
  (51 instances on dashboard alone): the same tab bar renders on every
  surface via `AppShell`.

## Phase 1 — CI wiring (unblocks everything else)

**Problem:** `.github/workflows/ci.yml`'s advisory `gui-playwright-smoke`
capture step runs `--project=chromium` only; its analysis step passes no
`--browsers` flag and silently defaults to chromium-only
(`gui-visual-review.rs:28-30`). Every Firefox-only defect (F-01, F-04, F-05,
and the Firefox instance of F-03) is therefore invisible to CI today.

**Fix:** add `--project=firefox-review` to the capture step's Playwright
invocation, and `--browsers chromium,firefox` to the analysis step's
`gui-visual-review --bundle ... --ai` invocation. Mechanical, ~2 lines.

**Why first:** every fix in Phases 2–6 needs a Firefox-aware CI gate to keep
it fixed. Landing this after the other phases means the whole effort could
regress unnoticed the day someone touches an overlay component.

## Phase 2 — F-01/F-04: Firefox overlay compositing

**Approach (per user decision): opaque fallback background, not a
root-cause investigation of Firefox's blur compositing.**

Add a Firefox-safe solid background layer to `Glass` so its content is never
see-through regardless of `backdrop-filter` support/compositing. Concretely:
add a new CSS custom property (e.g. `--color-overlay-solid`, a fully-opaque
near-match to the current dark theme background — sample the existing
`bg-bg-base` token, not a new arbitrary color) and apply it as `Glass`'s
background instead of `bg-overlay-subtle`, keeping `backdrop-blur-2xl` as a
purely decorative enhancement layered on top (blur only affects what's
*behind* an already-opaque layer, so this can't regress Chromium's current
look — it removes translucency, which is the entire point).

This single change in `Glass.tsx` covers `AchievementsDrawer`,
`ChatSessionRail`/`ChatExecutionRail` (via `ChatSurface`'s overlay wrapper),
and `DiscoveryRail`, because all three compose `Glass` rather than
reimplementing their own background. No per-surface changes needed unless a
surface overrides the background via `className` (grep for
`bg-overlay-subtle` passed as an override `className` to `<Glass>` call
sites as a verification step — the plan's task will do this explicitly).

**Verification:** re-run the harness (`vox run scripts/frontend-review.vox`)
scoped to `chat`, `dashboard`, `console` with `firefox-review`, and confirm
the specific regression cells the review named
(`chat--rails-overlay-open--compact--firefox`,
`dashboard--achievements-open--{compact,laptop,wide}--firefox`,
`console--default--compact--firefox`,
`chat--session-menu-open--compact--firefox`) score clean with 0 occlusion
defects.

## Phase 3 — F-03: `__TAURI_INTERNALS__` / raw-error toast leak

**Fix:** add a `sanitizeErrorForToast(err: unknown): string` helper (natural
home: `src/lib/backendGuard.ts`, alongside `BackendUnavailableError`, since
it needs to special-case that type) that:
1. Returns `err.message` directly if `err instanceof BackendUnavailableError`
   (already honest).
2. Otherwise returns `String(err)` **unless** the resulting string contains
   `__TAURI_INTERNALS__`, `invoke`, or matches other raw-IPC-internal
   patterns — in which case return a generic
   `"An unexpected error occurred."` (or similarly honest but
   non-leaking message).
3. Replace all `body: String(err)` occurrences in `App.tsx`'s `pushToast`
   calls with `body: sanitizeErrorForToast(err)`.

This directly fixes the `dashboard--no-backend--wide--firefox` cell the
review cited, and closes the whole class rather than one occurrence, since
every `pushToast(..., body: String(err) ...)` call site shares the exposure.

## Phase 4 — F-02: shared null-deref TypeError

**Fix (per user decision: root-cause the null-check, not just sanitize):**
1. `App.tsx:404` — guard the fallback session-creation path:
   ```ts
   invoke<Session>('chat_create_session', { title: 'Chat' })
     .then((s) => { if (s?.session_id) setActiveSessionId(s.session_id); })
     .catch((err) => pushToast({ tone: 'warn', title: 'Chat session', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
   ```
2. `App.tsx:787-792` and `825-828` — guard both `res.is_error` reads:
   ```ts
   const failed = !res || res.is_error;
   pushToast({
     tone: failed ? 'warn' : 'ok',
     title: failed ? 'Rollback failed' : 'Rollback complete',
     body: failed ? sanitizeErrorForToast(res?.error ?? 'Unknown error') : ...,
     cause: failed ? 'backend-error' : 'backend-ok',
   });
   ```
   (Exact shape depends on each call site's existing variable names — the
   plan's tasks give the precise diff per site.)

Because Phase 3 lands first, these guarded paths get `sanitizeErrorForToast`
for free on the remaining thrown-error branches.

**Verification:** re-run the harness on the `empty`/`error` variant states
for the ~9 affected surfaces named in the review (`chat`, `dashboard`,
`gamify`, `runs`, `policies`, `vox-search`, `memory`, `models`, `approvals`)
and confirm zero `session_id`/`is_error` null-deref defects remain.

## Phase 5 — F-07 + top axe classes

1. **F-07:** in `WorkbenchTabBar.tsx`, give the per-tab wrapping `<div>`
   (line 32) `role="presentation"` — this satisfies ARIA 1.2's allowance for
   non-tab intervening elements inside a `tablist` without restructuring the
   DOM or losing the close-button's independent focusability.
2. **`page-has-heading-one`** (39 instances on `chat`/`dashboard`, 19-25 on
   most others): audit each surface's root component for a semantic `<h1>`
   — most already have a styled title `<span>`/`<div>` that should become an
   `<h1>` (visually restyled via CSS, not a new visible heading) rather than
   adding a redundant one.
3. **`landmark-unique`**: audit for duplicate unlabeled `<nav>`/`<main>`
   landmarks (likely the sidebar + workbench both rendering as unlabeled
   `<nav>` — give each a distinct `aria-label`).

Each of these three axe rules is single-shared-component or single-pattern,
consistent with the review's assessment that they're not surface-specific.

**Verification:** re-run `axe-core` via the harness's existing in-page audit
(`e2e/review/audits.ts`) across all 31 surfaces and confirm the
`aria-required-children`/`page-has-heading-one`/`landmark-unique` counts in
`bundle-digest.md` drop to (near-)zero.

## Phase 6 — F-05/F-06 re-verify + fix

1. **F-05 (chat rail clipping):** re-run the harness on
   `chat--rails-overlay-open--compact--firefox` and
   `chat--session-menu-open--compact--firefox` *after* Phase 2 lands, before
   writing any new code — the review explicitly flagged this as likely a
   symptom of F-01's broken layout. If it's gone, mark resolved; if not,
   triage as a genuinely separate clipping bug at that point (do not
   pre-guess a fix in this plan).
2. **F-06 (blank simulation viewport):** in `LudusSandbox.tsx`, when the
   canvas-draw effect's guard (`if (!layout) return;`) fires because there is
   no `layout` to render, render a sibling fallback (a simple centered
   "Simulation unavailable" message with the same honesty standard as
   `BackendBanner`) instead of leaving a blank canvas. `Dashboard.tsx`'s
   `LudusSandbox` mount doesn't need to change — the fix belongs entirely
   inside the component that has the data.

## Testing approach

- **Unit-testable fixes** (Phases 3, 4, 5's ARIA/heading changes, Phase 6's
  `LudusSandbox` empty state) get component tests written TDD-first: a
  failing test asserting the honest/guarded behavior, then the minimal fix.
- **Visual/compositing fixes** (Phase 2, and Phase 6's F-05 re-verify) are
  proven by the existing harness, not new Playwright specs — the harness's
  durable regression states (`rails-overlay-open`, `achievements-open`,
  `session-menu-open`, `no-backend`) already exist in
  `e2e/review/states.ts` for exactly this purpose. Re-running
  `frontend-review.vox` scoped to the affected surfaces after each fix *is*
  the test.
- **CI wiring (Phase 1)** is verified by confirming the changed `ci.yml`
  lines produce Firefox entries in a manual CI-equivalent local run (already
  proven possible — the Task 13 pipeline run this session used
  `--browsers chromium,firefox` locally).

## Non-goals

- No new component/e2e test authorship for un-specced surfaces (tracked
  separately as coverage debt per the review, item 7).
- No Tauri-shell/`tauri-driver` harness — F-01 through F-07 remain
  "confirmed-in-Firefox, presumed-absent-in-the-Tauri-shell" after this
  effort, same caveat the review already recorded.
- No `settings`/keybinds blank-pane investigation — noted as a candidate
  finding, not triaged, in the source review; out of scope here.
- No general contrast/color-token remediation for the 397 unenumerated minor
  findings — those are batched separately per the review's own
  recommendation (a design-token audit), not part of this plan.
