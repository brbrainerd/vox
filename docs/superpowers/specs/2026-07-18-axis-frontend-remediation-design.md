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

**Revision note (2026-07-18, adversarial audit):** this spec was audited by
five parallel reviewers against the codebase after its first draft. The audit
found and this revision fixes: a nonexistent `res.error` field, an ARIA fix
(`role="presentation"`) that would not actually silence axe, a phantom
"second nav" in the landmark analysis (real culprits: unlabeled `<aside>`s +
repeated `role="region"`s), a stale F-06 premise (LudusSandbox already
handles `scanFailed`; the uncovered state is `layout`-null-while-pending), an
overclaimed F-03 scope (~95 `String(err)` toast sites live *outside*
`App.tsx`), a Firefox-not-installed-in-CI gap, the `hover:bg-overlay-subtle`
regression path in the Glass fix, and the travertine theme token gap.

**Scope:** Items 1–6 of the review's recommended order. **Explicitly out of
scope** (tracked as follow-up debt, not part of this effort):
- Item 7 (new component/e2e test coverage for `mercatus`, `publications`,
  `vox-search`, and the 21 surfaces with no dedicated e2e spec).
- The Tauri-shell (`tauri-driver`) spot check the review flagged as never
  having been run.
- The `settings`/keybinds blank-content-pane candidate the review noted in
  passing (per-surface table) but did not triage.
- `page-has-heading-one` on surfaces other than `chat`/`dashboard` (the two
  highest-instance surfaces are fixed here; the rest are enumerated as debt).

## Root-cause findings (from source inspection + adversarial audit)

- **F-01/F-04 (Firefox overlay occlusion/layout collapse):** the shared
  `Glass` primitive ([`src/components/ui/Glass.tsx`](../../../crates/vox-gui/ui/src/components/ui/Glass.tsx))
  applies `bg-overlay-subtle` (`rgba(255,255,255,0.04)` —
  `tokens/semantic.json` → `tokens.generated.css`) combined with
  `backdrop-blur-2xl` as its only background. Chromium and Firefox composite
  a ~4%-alpha `backdrop-filter: blur()` layer differently; Firefox lets
  content underneath bleed through at full opacity. Every occlusion cell in
  the review traces to a `Glass`-based overlay: `AchievementsDrawer`
  (`src/components/gamify/AchievementsDrawer.tsx:38`), the compact-viewport
  session/execution rail overlays in `ChatSurface`
  (`src/components/surfaces/Chat/ChatSurface.tsx:264,317`), and Console's
  `DiscoveryRail` (`src/components/surfaces/Console/DiscoveryRail.tsx:99`).
  `Popover.tsx` is NOT affected — it already uses `bg-zinc-950/95`.
  **Audit additions:** (a) `Glass.tsx:37`'s `interactive &&
  "hover:bg-overlay-subtle"` would reintroduce the translucent background on
  hover — the fix must cover it too; (b) the travertine theme's Style
  Dictionary build sources `semantic.travertine.json`, not `semantic.json`,
  so the new token must be added to both (travertine gets a light-appropriate
  opaque value) or that theme falls back to a dark `#11151a` on a light
  background; (c) `<Glass as="button">` composes `bg-transparent` after the
  base class, so button-rendered Glass stays transparent — intentional
  (buttons are not overlay panels), documented, not fixed; (d) this change
  makes **all 72 Glass call sites** opaque, not just the overlay ones — an
  accepted global visual change (cards on `bg-surface` will render slightly
  darker than their parent instead of lighter), to be eyeballed in the
  harness output as part of verification.
- **F-02 (shared null-deref TypeError toast):** the null-derefs are **caught**
  exceptions — `.then` throws land in the adjacent `.catch`/`try-catch` and
  surface as *leaky warn toasts*, not crashes. Any test for them must assert
  on toast content, not on "does not throw" (App renders fine today with a
  null-resolving mock; the existing `App.test.tsx` smoke test silently
  exercises this exact path). Sites:
  - `src/App.tsx:403-405` — `.then((s) => setActiveSessionId(s.session_id))`
    on the `chat_create_session` fallback; null `s` → the review's
    `"session_id, s is null"` toast.
  - `src/App.tsx:785-793` (rollback) — `res.is_error` reads with no null
    guard. The failure body is `typeof res.result === 'string' ? res.result
    : JSON.stringify(res.result)`; **there is no `res.error` field**
    (`McpInvokeResult` is `{ tool, is_error, result }` per
    `src/lib/mcpToolResult.ts:6-9`) — the fix must preserve `res.result` as
    the failure text when `res` is non-null, not discard it.
  - `src/App.tsx:820-829` (audit) — the deref happens at **line 821-823**
    (`typeof res.result === 'string' ...`), *before* the `pushToast`; a guard
    only around `is_error` still throws. The throw is caught by the inner
    `catch (err)` at 830 → 'Audit unavailable' toast with the TypeError
    leaked. The same handler's first attempt derefs `out.exit_code`/`out.stdout`
    (lines ~807-815) with no null guard — same class, fixed together.
- **F-03 (`__TAURI_INTERNALS__` toast leak):** the `body: String(err)`
  pattern exists at **14 sites in `App.tsx`** (405, 408, 667, 682, 757, 795,
  831, 861, 899, 910, 917, 924, 948, 1052) **and ~95 more sites across ~27
  other files** (`BrowserView.tsx` ×13, `SettingsView.tsx` ×17,
  `SkillsPluginsView.tsx` ×10, `InlineApprovals.tsx` — which receives App's
  `pushToast` as a prop and renders into the same toast stack — and others).
  Fixing only `App.tsx` covers ~13% of the class. The fix is therefore
  repo-wide mechanical replacement **plus a source-scan guard test** (same
  pattern as `transportIpcGuard.test.ts`) forbidding `body: String(` under
  `src/`, so the class cannot silently regrow. `BackendUnavailableError`
  (`src/lib/backendGuard.ts`) already carries an honest message — the
  sanitizer special-cases it. The leak pattern `\binvoke\b` does not match
  `invoke_mcp_tool` (underscore is a word char); prose like "failed to
  invoke X" degrades to the generic message — acceptable.
- **F-06 (blank simulation viewport) — corrected premise:**
  `LudusSandbox.tsx:298-303` **already renders** "Workspace scan unavailable
  — the town cannot render." when `scanFailed`, and
  `LudusSandbox.test.tsx` **already exists** and asserts it. The genuinely
  uncovered state is `!layout && !scanFailed` — scan pending, or resolved
  without producing a layout — which leaves the mounted canvas blank with no
  affordance. The fix targets exactly that state.
- **F-07 (tablist ARIA violation) — corrected fix:**
  `src/components/layout/WorkbenchTabBar.tsx:32-68` — each `tablist` child is
  a plain `<div>` wrapping a `role="tab"` button plus (non-pinned tabs) a
  plain close `<button>`. **`role="presentation"` on the wrapper does NOT fix
  this**: axe computes owned children by looking *through* presentational
  and unroled wrappers, so the close button remains an owned non-`tab` child
  of the tablist and the rule still fires (the wrapper is already looked
  through today — that's why there are 51 violations). The correct fix is a
  restructure: the wrapper `<div>` becomes the `role="tab"` element (taking
  `aria-selected`, the `data-testid`, and the select handler), the inner
  label button demotes to a `<span>`, and the close affordance becomes
  presentational to AT (`aria-hidden`, `tabIndex={-1}` — still clickable
  with a pointer) with keyboard close provided via `Delete` on the focused
  tab (the ARIA-canonical pattern, since `role="tab"` has
  children-presentational semantics that flatten nested interactive
  elements anyway). The 4 unit-test and 3 e2e assertions that target
  `getByRole('button', { name: 'Close X' })` / testid-on-the-inner-button
  must be updated in the same change.
- **`landmark-unique` — corrected analysis:** the layout has only **one**
  unlabeled `<nav>` (`Sidebar.tsx:162`; `BreadcrumbBar.tsx:52` is already
  labeled; `AppShell.tsx` renders no nav). The review's chat count of 29
  comes from **unlabeled `<aside>` (complementary) landmarks** —
  `Sidebar.tsx:134`, `ChatSessionRail.tsx:52/71`,
  `ChatExecutionRail.tsx:96/115` — plus **repeated `role="region"`**
  instances (`ModelBadge.tsx:48` per-message, `ChatExecutionRail.tsx:133,160`)
  that duplicate accessible names once per message. Fix: distinct
  `aria-label`s on the nav and the three asides; de-landmark (or uniquely
  name) the repeated per-message regions — plain `role`-less containers are
  correct for per-message chrome.

## Phase 1 — CI wiring (unblocks everything else)

**Problem:** `.github/workflows/ci.yml`'s advisory review-bundle capture step
runs `--project=chromium` only; the analysis step passes no `--browsers` flag
and defaults to chromium-only. **Audit addition:** the job's only browser
install is `pnpm exec playwright install chromium` (line ~1663) — adding the
Firefox project without installing Firefox fails *silently* (the step is
`continue-on-error: true`), recreating the exact invisibility being fixed.

**Fix (3 edits):** change the install line to
`pnpm exec playwright install chromium firefox`; add
`--project=firefox-review` to the capture invocation; add
`--browsers chromium,firefox` to the analysis invocation (and raise
`--max-reviews` 40→60 since the cell count roughly doubles). All steps stay
advisory (`continue-on-error: true`).

**Why first:** every fix in Phases 2–6 needs a Firefox-aware CI gate to keep
it fixed.

## Phase 2 — F-01/F-04: Firefox overlay compositing

**Approach (per user decision): opaque fallback background, not a
root-cause investigation of Firefox's blur compositing.**

1. Add `overlay.solid` to `tokens/semantic.json` (`{color.basalt.850}` =
   `#11151a`, opaque, one step lighter than `bg.base`) **and** to
   `tokens/semantic.travertine.json` (a light-appropriate opaque value, e.g.
   `{color.travertine.100}`-family) — the travertine build does not source
   `semantic.json`. The high-contrast build sources `semantic.json` and
   inherits automatically.
2. Wire `'overlay-solid'` into `tailwind.config.js` alongside the existing
   `overlay-subtle`/`overlay-hover` entries; `pnpm tokens:build` regenerates
   all three generated CSS files.
3. In `Glass.tsx`: base class `bg-overlay-subtle` → `bg-overlay-solid`, AND
   the interactive branch's `hover:bg-overlay-subtle` → an opaque hover
   (e.g. `hover:bg-bg-elevated`) — otherwise hovering any interactive Glass
   reintroduces the translucent background Firefox mis-composites.
   `backdrop-blur-2xl` stays as decoration; the `ring-overlay-subtle` inset
   ring (1px decorative) stays; the `as="button"` transparent branch stays
   (documented as intentional).
4. Call-site check: no `<Glass>` call site overrides with
   `bg-overlay-subtle`, but several override with other translucent
   backgrounds (`DueNudge.tsx` `bg-zinc-950/65`, `FunGauge.tsx`/
   `HudPanels.tsx` `bg-zinc-950/80`, `RunsView.tsx` `bg-black/30`) — these
   win via twMerge and stay translucent. They are small HUD chips, not
   full-panel overlays; leave them unless the harness re-run flags them.

**Verification:** re-run the harness scoped to `chat`, `dashboard`,
`console` with `firefox-review` and confirm the named regression cells
(`chat--rails-overlay-open--compact--firefox`,
`dashboard--achievements-open--*--firefox`,
`console--default--compact--firefox`,
`chat--session-menu-open--compact--firefox`) show 0 occlusion/layout
defects; also eyeball the dashboard-grid chromium captures for the accepted
global elevation-cue change.

## Phase 3 — F-03: raw-error toast leak, repo-wide

1. `sanitizeErrorForToast(err: unknown): string` in
   `src/lib/backendGuard.ts`: returns `err.message` for
   `BackendUnavailableError`; otherwise `String(err)` unless it matches
   `/__TAURI_INTERNALS__|\binvoke\b/`, in which case a generic honest
   message.
2. Mechanical replacement of **every** `body: String(err)` / `body:
   String(e)` under `crates/vox-gui/ui/src` (~109 sites, 28 files) with
   `sanitizeErrorForToast(...)` + the import.
3. A source-scan guard test (`src/guards/toastBodyGuard.test.ts`, same
   pattern as `transportIpcGuard.test.ts`) failing on any
   `body: String(` occurrence under `src/`, so the class cannot regrow.

The only existing test asserting a raw error toast body
(`CodeRabbitView.test.tsx:72`, asserts `'boom'`) passes unchanged —
`'boom'` doesn't trip the pattern.

## Phase 4 — F-02: shared null-deref TypeError

All three fixes must preserve real failure text and be tested via **toast
content assertions** (the throws are caught — "does not throw" tests pass
before the fix and prove nothing):

1. `App.tsx:403-405`: `.then((s) => { if (s?.session_id)
   setActiveSessionId(s.session_id); })`. Test: with `chat_create_session`
   mocked to resolve `null`, assert the 'Chat session' warn toast does NOT
   appear (before the fix, the caught TypeError produces it).
2. `App.tsx:785-793` (rollback): guard once —
   `const failed = !res || res.is_error;` — and the failure body becomes
   `!res ? 'Unknown error' : sanitizeErrorForToast(typeof res.result ===
   'string' ? res.result : JSON.stringify(res.result))`, preserving the real
   error text; success branch text unchanged.
3. `App.tsx:807-829` (audit): guard **both** derefs — `out` (lines ~807-815)
   and `res`/`res.result` (lines 820-829, the deref at 821 precedes the
   pushToast) — with the same `failed` pattern.

## Phase 5 — F-07 + top axe classes

1. **F-07:** restructure `WorkbenchTabBar` per the corrected analysis above
   (wrapper becomes the tab; label demotes to span; close affordance
   `aria-hidden`/`tabIndex={-1}` + `Delete`-key close; testid and
   `aria-selected` move to the wrapper; update the 4 unit and 3 e2e
   assertions that target the old structure). Component test asserts the
   axe-relevant invariant directly: every direct child of the tablist has
   `role="tab"`, and `within(tablist).queryAllByRole('button')` is empty
   (testing-library's a11y-tree semantics approximate axe's owned-children
   computation).
2. **`page-has-heading-one`** (chat + dashboard only, per scope): neither
   surface has any `<h1>`, and neither has a visible root title to promote
   (`Dashboard.tsx`'s `<h2>The Stream</h2>` is a section heading). Both get
   an `sr-only` `<h1>` as the first child of the surface root.
   `SurfaceMiniRender`'s `aria-hidden="true"` frame keeps embedded mini
   surfaces' h1s out of the a11y tree, so the dashboard stays at exactly one
   accessible h1. (Latent caveat, documented in a code comment: if
   `chatDocked` — currently hardcoded false at `App.tsx:1080` — is ever
   enabled, a docked ChatSurface would add a second h1 to the page.)
3. **`landmark-unique`:** distinct `aria-label`s on `Sidebar.tsx:162`'s nav
   and the three asides (`Sidebar.tsx:134`, `ChatSessionRail`,
   `ChatExecutionRail`); remove `role="region"` from per-message chrome
   (`ModelBadge.tsx:48`) or give instances unique accessible names; same
   for `ChatExecutionRail.tsx:133,160`. Tests live in the components' own
   test files, NOT `AppShell.test.tsx` (which mocks Sidebar with an
   already-labeled stub — a test there can never fail).

## Phase 6 — F-05/F-06 re-verify + fix

1. **F-05 (chat rail clipping):** re-run the harness on the two chat cells
   *after* Phase 2 lands, before writing any code. If clipping is gone, mark
   resolved; if not, triage as a separate bug — do not pre-guess a fix.
2. **F-06 (blank simulation viewport) — corrected:** the `scanFailed` state
   is already handled and tested. Fix the uncovered `!layout && !scanFailed`
   state: render a centered "Simulation loading…"/"no workspace data yet"
   fallback adjacent to the canvas (canvas hidden via conditional class
   while `!layout`, kept mounted so refs stay valid). Extend the **existing**
   `LudusSandbox.test.tsx` (which already mocks `../../transport`) — do not
   create a new file.

## Testing approach

- Unit-testable fixes (Phases 3, 4, 5, 6's F-06) are TDD, with the audit's
  correction applied throughout: **tests must discriminate** (assert toast
  content / a11y-tree queries), never "renders without throwing" — the bugs
  here are caught exceptions and axe-tree violations, both invisible to
  crash assertions.
- Visual/compositing fixes (Phase 2, Phase 6's F-05) are proven by the
  existing harness against its durable regression states. Local analysis
  runs require `OPENROUTER_API_KEY` (Clavis) and write the digest to
  `contracts/reports/gui-visual-review/bundle-digest.md` (NOT under
  `review-bundle/`). Scoped capture runs **delete the whole prior bundle**
  (`globalSetup` clears `review-bundle/latest` unconditionally when
  `VOX_REVIEW_CAPTURE=1`) — final counts must come from a fresh full-matrix
  run.
- CI wiring (Phase 1) is verified by the presence of Firefox entries in the
  next CI run's uploaded bundle artifact (the install fix is what makes this
  possible at all).

## Non-goals

- No new component/e2e test authorship for un-specced surfaces (item 7 debt).
- No Tauri-shell/`tauri-driver` harness.
- No `settings`/keybinds blank-pane investigation.
- No general contrast/color-token remediation for the 397 minor findings.
- No `page-has-heading-one` fixes beyond chat/dashboard (remaining surfaces
  enumerated as debt during execution).
