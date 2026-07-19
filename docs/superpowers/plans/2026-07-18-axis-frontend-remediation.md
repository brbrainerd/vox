# Axis Frontend Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the six confirmed finding classes (F-01 through F-07, minus F-05 which
self-resolves-or-triages) from the 2026-07-18 Axis frontend comprehensive review, plus
the CI wiring gap that made the Firefox-only defects invisible.

**Architecture:** Six independent phases, each gated on the prior only where the spec
says so (Phase 1 first because everything else needs Firefox-aware CI to stay fixed;
Phase 6's F-05 re-verify explicitly waits on Phase 2). Every visual/compositing fix is
proven by re-running the existing `frontend-review.vox` harness against its own durable
regression states — no new Playwright specs are written for those. Null-deref and ARIA
fixes get conventional TDD component tests.

**Tech Stack:** React + TypeScript (`crates/vox-gui/ui`), Vitest + Testing Library,
Playwright (existing `e2e/review/` harness), Style Dictionary (`tokens/*.json` →
`tokens.generated.css`), GitHub Actions.

**Design doc:** [`docs/superpowers/specs/2026-07-18-axis-frontend-remediation-design.md`](../specs/2026-07-18-axis-frontend-remediation-design.md)

---

### Task 1: CI wiring — make Firefox visible to CI

**Files:**
- Modify: `.github/workflows/ci.yml:1678-1688` (the `gui-playwright-smoke` job's
  review-bundle capture + analysis steps)

- [ ] **Step 1: Read the current steps to confirm line numbers haven't drifted**

Run: `grep -n "Review-bundle capture\|Review-bundle AI defect analysis" .github/workflows/ci.yml`
Expected: two matches, a capture step and an analysis step, a few lines apart.

- [ ] **Step 2: Add the Firefox project to the capture step and `--browsers` to the analysis step**

Change:
```yaml
      # Review-bundle capture (chromium-only in CI to bound cost; firefox is
      # local/on-demand via scripts/frontend-review.vox). Advisory per F2.
      - name: Review-bundle capture (chromium)
        working-directory: crates/vox-gui/ui
        env:
          VOX_REVIEW_CAPTURE: "1"
        run: pnpm exec playwright test e2e/review/capture.spec.ts --project=chromium --workers=2
        continue-on-error: true
      - name: Review-bundle AI defect analysis (advisory)
        env:
          OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
        run: cargo run -p vox-orchestrator-mcp --features gui-visual-review --bin gui-visual-review -- --bundle crates/vox-gui/ui/review-bundle/latest --ai --max-reviews 40
        continue-on-error: true
```
to:
```yaml
      # Review-bundle capture, both browsers — Firefox-only defects (occlusion/
      # layout) are otherwise invisible to CI (2026-07-18 review finding).
      # Advisory per F2.
      - name: Review-bundle capture (chromium + firefox)
        working-directory: crates/vox-gui/ui
        env:
          VOX_REVIEW_CAPTURE: "1"
        run: pnpm exec playwright test e2e/review/capture.spec.ts --project=chromium --project=firefox-review --workers=2
        continue-on-error: true
      - name: Review-bundle AI defect analysis (advisory)
        env:
          OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
        run: cargo run -p vox-orchestrator-mcp --features gui-visual-review --bin gui-visual-review -- --bundle crates/vox-gui/ui/review-bundle/latest --ai --max-reviews 60 --browsers chromium,firefox
        continue-on-error: true
```
(`--max-reviews` raised from 40 to 60 since the cell count roughly doubles with two
browsers; still bounded, still advisory/`continue-on-error: true`.)

- [ ] **Step 3: Validate the YAML**

Run: `python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" || pwsh -c "Get-Content .github/workflows/ci.yml | Out-Null"`
Expected: no parse error. (If neither `python` nor a YAML linter is available, visually
re-diff the block against the snippet above — no other lines should change.)

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(gui): capture + review Firefox in the advisory review-bundle steps"
```

---

### Task 2: `Glass` opaque background — fix F-01/F-04 at the root

**Files:**
- Modify: `crates/vox-gui/ui/tokens/semantic.json`
- Modify: `crates/vox-gui/ui/tailwind.config.js`
- Modify: `crates/vox-gui/ui/src/components/ui/Glass.tsx`
- Test: `crates/vox-gui/ui/src/components/ui/Glass.test.tsx`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-gui/ui/src/components/ui/Glass.test.tsx`:
```tsx
  it('uses an opaque background, not a low-alpha overlay tint (Firefox backdrop-blur compositing bug)', () => {
    render(<Glass data-testid="g">Content</Glass>);
    const el = screen.getByTestId('g');
    expect(el).toHaveClass('bg-overlay-solid');
    expect(el).not.toHaveClass('bg-overlay-subtle');
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/ui/Glass.test.tsx`
Expected: FAIL — `bg-overlay-solid` class not present (`Glass.tsx` still uses
`bg-overlay-subtle`).

- [ ] **Step 3: Add the new token**

In `crates/vox-gui/ui/tokens/semantic.json`, change:
```json
    "overlay": { "subtle": { "value": "rgba(255,255,255,0.04)" }, "hover": { "value": "rgba(255,255,255,0.07)" } }
```
to:
```json
    "overlay": { "subtle": { "value": "rgba(255,255,255,0.04)" }, "hover": { "value": "rgba(255,255,255,0.07)" }, "solid": { "value": "{color.basalt.850}" } }
```
(`basalt.850` = `#11151a`, already defined in `tokens/primitive.json:15` — a fully
opaque shade one step lighter than `bg.base`/`basalt.900`, so overlay panels read as
a distinct elevated surface rather than pure background.)

- [ ] **Step 4: Wire the Tailwind class**

In `crates/vox-gui/ui/tailwind.config.js`, find the `overlay-subtle`/`overlay-hover`
color mapping (near line 15's "Semantic tokens" comment) and add the `solid` variant
alongside it, e.g.:
```js
        'overlay-subtle': 'var(--color-overlay-subtle)',
        'overlay-hover': 'var(--color-overlay-hover)',
        'overlay-solid': 'var(--color-overlay-solid)',
```
(match whatever the existing two lines' exact key names/quoting are — add the third
following the same pattern.)

- [ ] **Step 5: Regenerate the CSS and update `Glass.tsx`**

Run: `cd crates/vox-gui/ui && pnpm tokens:build`
Expected: `src/styles/tokens.generated.css` gains a `--color-overlay-solid: #11151a;` line.

Then in `crates/vox-gui/ui/src/components/ui/Glass.tsx`, change:
```tsx
        "relative border border-border-subtle bg-overlay-subtle backdrop-blur-2xl shadow-[0_1px_0_rgba(255,255,255,0.04)_inset,0_24px_60px_-30px_rgba(0,0,0,0.9)]",
```
to:
```tsx
        "relative border border-border-subtle bg-overlay-solid backdrop-blur-2xl shadow-[0_1px_0_rgba(255,255,255,0.04)_inset,0_24px_60px_-30px_rgba(0,0,0,0.9)]",
```
(`backdrop-blur-2xl` stays — it's now a decorative blur over an already-opaque layer,
which is visually inert but harmless, instead of being load-bearing for opacity.)

- [ ] **Step 6: Check for background-overriding call sites**

Run: `grep -rn "bg-overlay-subtle" crates/vox-gui/ui/src --include="*.tsx" | grep -v "hover:bg-overlay-subtle"`
Expected: any remaining non-hover `bg-overlay-subtle` usages are either unrelated
(buttons, badges — fine to leave translucent, they're not full-panel overlays) or a
`<Glass className="...">` override that would now conflict. If any `<Glass>` call site
passes `bg-overlay-subtle` in its own `className`, remove it there — `Glass`'s own
default now supersedes it and Tailwind's `cn()` merge order would otherwise cause a
conflicting-class warning, not a bug, but worth cleaning up if found.

- [ ] **Step 7: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/ui/Glass.test.tsx`
Expected: PASS (both the new test and the two pre-existing ones).

- [ ] **Step 8: Run the full component test suite (regression check)**

Run: `cd crates/vox-gui/ui && npx vitest run`
Expected: all files pass (996+ tests) — no snapshot/class-assertion test elsewhere
hardcodes `bg-overlay-subtle` on a `Glass`-based component. If one does, update that
assertion to `bg-overlay-solid` (that test was asserting the old, buggy behavior).

- [ ] **Step 9: Commit**

```bash
git add crates/vox-gui/ui/tokens/semantic.json crates/vox-gui/ui/tailwind.config.js \
  crates/vox-gui/ui/src/styles/tokens.generated.css crates/vox-gui/ui/src/components/ui/Glass.tsx \
  crates/vox-gui/ui/src/components/ui/Glass.test.tsx
git commit -m "fix(gui): Glass uses an opaque background instead of a low-alpha overlay tint

Firefox composites backdrop-blur over a ~4%-alpha background differently
than Chromium, letting underlying content bleed through at full opacity
(F-01/F-04: AchievementsDrawer, chat rail overlays, console DiscoveryRail).
bg-overlay-solid is fully opaque; backdrop-blur-2xl is now purely
decorative rather than load-bearing for translucency."
```

---

### Task 3: verify F-01/F-04 fixed via the harness (no new Playwright specs)

**Files:** none modified — this task only runs the existing harness and reads output.

- [ ] **Step 1: Build a release `vox` if stale (needed by `frontend-review.vox`)**

Run: `ls -la target/release/vox.exe` — if missing or older than the latest commit,
run `cargo build --release -p vox-cli --locked`.

- [ ] **Step 2: Run the capture step scoped to the affected surfaces, both browsers**

Run (from repo root):
```bash
cd crates/vox-gui/ui
VOX_REVIEW_CAPTURE=1 npx playwright test e2e/review/capture.spec.ts \
  --project=chromium --project=firefox-review --workers=2 \
  --grep "chat|dashboard|console"
```
Expected: exit 0, new entries written under `review-bundle/latest/`.

- [ ] **Step 3: Run the analysis step**

Run (from repo root): `./target/release/vox.exe`-equivalent —
```bash
cargo run -p vox-orchestrator-mcp --features gui-visual-review --bin gui-visual-review -- \
  --bundle crates/vox-gui/ui/review-bundle/latest --ai --browsers chromium,firefox
```

- [ ] **Step 4: Confirm the named regression cells are clean**

Run: `grep -A3 "chat--rails-overlay-open--compact--firefox\|dashboard--achievements-open\|console--default--compact--firefox\|chat--session-menu-open--compact--firefox" crates/vox-gui/ui/review-bundle/latest/bundle-digest.md`
Expected: no `occlusion` or `layout` defects listed for these cells (scores should be
comparable to their Chromium counterparts, e.g. 75+ rather than the review's recorded
15-45).

If any cell still shows an occlusion/layout defect, stop and re-open Task 2 — do not
proceed to Task 4 with a known-unfixed regression state.

- [ ] **Step 5: No commit for this task** (verification only — the harness output
  under `review-bundle/` is gitignored per the existing harness design).

---

### Task 4: sanitize toast error bodies — fix F-03

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/backendGuard.ts`
- Test: `crates/vox-gui/ui/src/lib/backendGuard.test.ts`
- Modify: `crates/vox-gui/ui/src/App.tsx` (all `body: String(err)` / `body: String(e)`
  call sites — see Step 5 for the exact list)

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-gui/ui/src/lib/backendGuard.test.ts`:
```ts
import { sanitizeErrorForToast, BackendUnavailableError } from './backendGuard';

describe('sanitizeErrorForToast', () => {
  it('returns the honest message for BackendUnavailableError', () => {
    const err = new BackendUnavailableError('chat_list_sessions');
    expect(sanitizeErrorForToast(err)).toBe(err.message);
  });

  it('does not leak __TAURI_INTERNALS__ or raw invoke internals', () => {
    const err = new TypeError(`can't access property "invoke", window.__TAURI_INTERNALS__ is undefined`);
    expect(sanitizeErrorForToast(err)).not.toMatch(/__TAURI_INTERNALS__/);
    expect(sanitizeErrorForToast(err)).not.toMatch(/invoke/);
  });

  it('passes through ordinary error text unchanged', () => {
    expect(sanitizeErrorForToast(new Error('Network timeout'))).toBe('Error: Network timeout');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/backendGuard.test.ts`
Expected: FAIL — `sanitizeErrorForToast` is not exported.

- [ ] **Step 3: Implement `sanitizeErrorForToast`**

Add to `crates/vox-gui/ui/src/lib/backendGuard.ts` (after `BackendUnavailableError`):
```ts
/**
 * Toast bodies must never leak raw IPC internals (F-03: a caught rejection's
 * String(err) rendering __TAURI_INTERNALS__ verbatim in a user-visible toast).
 * This is distinct from the unhandledrejection filter above — it runs on
 * *caught* exceptions that the app itself chooses to display.
 */
const LEAK_PATTERN = /__TAURI_INTERNALS__|\binvoke\b/;

export function sanitizeErrorForToast(err: unknown): string {
  if (err instanceof BackendUnavailableError) return err.message;
  const text = String(err);
  return LEAK_PATTERN.test(text) ? 'An unexpected error occurred.' : text;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/backendGuard.test.ts`
Expected: PASS.

- [ ] **Step 5: Replace every `body: String(err)` / `body: String(e)` in `App.tsx`**

Run: `grep -n "body: String(err)\|body: String(e)" crates/vox-gui/ui/src/App.tsx`
to get the current exact line list (expect ~15 matches: lines 405, 408, 667, 682, 757,
795, 831, 899, 910, 917, 924, 948, 1052 per the design doc — re-confirm exact numbers
since they may have shifted). For each match, replace `String(err)` with
`sanitizeErrorForToast(err)` (or `String(e)` → `sanitizeErrorForToast(e)` matching
whichever catch-variable name that call site uses). Add the import at the top of
`App.tsx`:
```ts
import { sanitizeErrorForToast } from './lib/backendGuard';
```

- [ ] **Step 6: Run the full component test suite**

Run: `cd crates/vox-gui/ui && npx vitest run`
Expected: all tests pass — no existing test asserts a toast body equals a raw
`String(err)` value for a case that would now be sanitized (if one does, it was
asserting the leak; update it to expect the sanitized text instead).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/lib/backendGuard.ts crates/vox-gui/ui/src/lib/backendGuard.test.ts \
  crates/vox-gui/ui/src/App.tsx
git commit -m "fix(gui): sanitize toast error bodies — no more raw __TAURI_INTERNALS__ leak (F-03)

sanitizeErrorForToast() special-cases BackendUnavailableError (already
honest) and blocks any text matching __TAURI_INTERNALS__/invoke from
reaching a user-visible toast, replacing the ~15 raw String(err) call
sites in App.tsx's pushToast calls."
```

---

### Task 5: null-deref guards — fix F-02

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (the `chat_create_session` fallback around
  line 403-405, and the two `res.is_error` reads around lines 787-792 and 825-828)
- Test: `crates/vox-gui/ui/src/App.test.tsx`

- [ ] **Step 1: Write the failing test for the session-creation null-deref**

Add to `crates/vox-gui/ui/src/App.test.tsx` (adapt to whatever mock/render harness
the existing file already uses — it already mocks `invoke` per the file's current
setup):
```tsx
  it('does not throw when chat_create_session resolves null (F-02)', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'chat_create_session') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<App />);
    // Previously threw "Cannot read properties of null (reading 'session_id')"
    // synchronously inside the .then — assert no uncaught error surfaces and
    // the app still renders instead of crashing.
    expect(await screen.findByTestId('app-shell')).toBeInTheDocument();
  });
```
(Use whatever test-id/selector the existing `App.test.tsx` tests already rely on to
assert a successful render — check the file first and match its pattern rather than
inventing a new `data-testid`.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/App.test.tsx`
Expected: FAIL or an uncaught rejection logged — `s.session_id` throws when `s` is
`null`.

- [ ] **Step 3: Fix the session-creation call site**

In `crates/vox-gui/ui/src/App.tsx`, change:
```ts
            invoke<Session>('chat_create_session', { title: 'Chat' })
              .then((s) => setActiveSessionId(s.session_id))
              .catch((err) => pushToast({ tone: 'warn', title: 'Chat session', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
```
to:
```ts
            invoke<Session>('chat_create_session', { title: 'Chat' })
              .then((s) => { if (s?.session_id) setActiveSessionId(s.session_id); })
              .catch((err) => pushToast({ tone: 'warn', title: 'Chat session', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
```
(This task assumes Task 4 already landed, so the catch already uses
`sanitizeErrorForToast` — if Task 4 has not landed yet, keep `String(err)` here and
let Task 4 update it later; do not duplicate the change.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/App.test.tsx`
Expected: PASS.

- [ ] **Step 5: Write the failing test for the `res.is_error` null-deref**

Add to `crates/vox-gui/ui/src/App.test.tsx`:
```tsx
  it('does not throw when a rollback/audit MCP call resolves null res (F-02)', async () => {
    // Exercise whichever user action in App.tsx drives the rollback/audit
    // pushToast at lines ~787/825 — read the surrounding handler first to
    // find its trigger (a button click or keyboard shortcut) and drive it
    // via userEvent, mocking the underlying invoke/invokeMcpTool call to
    // resolve `null` instead of a `{ is_error, ... }` object.
  });
```

- [ ] **Step 6: Read the handlers around `App.tsx:787` and `App.tsx:825` to find their exact trigger and current variable names**

Run: `sed -n '770,835p' crates/vox-gui/ui/src/App.tsx` and read the surrounding
function to identify the handler name, its trigger (button/shortcut), and the exact
variable holding the MCP response (`res` per the design doc, but confirm). Fill in
Step 5's test body with the real trigger once identified — do not leave it a stub;
this is a required step, not optional, before continuing.

- [ ] **Step 7: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/App.test.tsx`
Expected: FAIL — `res.is_error` throws when `res` is `null`.

- [ ] **Step 8: Fix both `res.is_error` read sites**

For each of the two sites (around lines 787-792 and 825-828), change the pattern from:
```ts
          pushToast({
            tone: res.is_error ? 'warn' : 'ok',
            title: res.is_error ? 'Rollback failed' : 'Rollback complete',
            body: res.is_error ? sanitizeErrorForToast(...) : ...,
            cause: res.is_error ? 'backend-error' : 'backend-ok',
          });
```
to a pattern that null-guards once at the top:
```ts
          const failed = !res || res.is_error;
          pushToast({
            tone: failed ? 'warn' : 'ok',
            title: failed ? 'Rollback failed' : 'Rollback complete',
            body: failed ? sanitizeErrorForToast('Unknown error') : ...,
            cause: failed ? 'backend-error' : 'backend-ok',
          });
```
(Preserve each site's existing success-branch body/title text exactly — only the
`res.is_error` reads become `failed`, and a leading `const failed = !res || res.is_error;`
is added. Repeat for the "Audit" site with its own title text.)

- [ ] **Step 9: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/App.test.tsx`
Expected: PASS.

- [ ] **Step 10: Run the full component test suite**

Run: `cd crates/vox-gui/ui && npx vitest run`
Expected: all tests pass.

- [ ] **Step 11: Verify against the harness (the ~9 affected surfaces' empty/error states)**

Run the same capture+analysis sequence as Task 3 Step 2-3, scoped this time with
`--grep "empty|error"` across `chat|dashboard|gamify|runs|policies|vox-search|memory|models|approvals`.
Confirm `session_id`/`is_error` null-deref defects no longer appear in
`bundle-digest.md` for those cells.

- [ ] **Step 12: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/App.test.tsx
git commit -m "fix(gui): guard two null-deref sites feeding the shared toast error leak (F-02)

chat_create_session's fallback .then and both res.is_error reads assumed
a non-null resolution; the empty/error mock states resolve null, throwing
'session_id, s is null' / 'is_error, res is null' TypeErrors across ~9
surfaces via the shared pushToast(String(err)) path. One fix per site,
not per surface."
```

---

### Task 6: WorkbenchTabBar ARIA fix — F-07

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx`
- Test: create `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.a11y.test.tsx`
  (or extend an existing `WorkbenchTabBar.test.tsx` if one exists — check first)

- [ ] **Step 1: Check for an existing test file**

Run: `ls crates/vox-gui/ui/src/components/layout/WorkbenchTabBar*.test.tsx 2>/dev/null || echo none`

- [ ] **Step 2: Write the failing test**

If a test file exists, add this case to it; otherwise create
`crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.a11y.test.tsx`:
```tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { WorkbenchTabBar } from './WorkbenchTabBar';

describe('WorkbenchTabBar a11y', () => {
  it('every direct child of the tablist is role=tab or role=presentation (aria-required-children)', () => {
    render(
      <WorkbenchTabBar
        tabs={[{ id: 'chat', label: 'Chat', pinned: true }, { id: 'dashboard', label: 'Dashboard' }]}
        activeTab="chat"
        onSelect={() => {}}
        onClose={() => {}}
      />,
    );
    const tablist = screen.getByRole('tablist');
    for (const child of Array.from(tablist.children)) {
      const role = child.getAttribute('role');
      expect(['tab', 'presentation', 'none']).toContain(role);
    }
  });
});
```
(Match the prop shape to `WorkbenchTabBar`'s actual `WorkbenchTabBarProps` interface —
read the component's top of file first if the prop names above don't match.)

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/WorkbenchTabBar.a11y.test.tsx`
Expected: FAIL — the wrapping `<div>` (no `role` attribute) is a direct `tablist` child.

- [ ] **Step 4: Fix the wrapping div**

In `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx`, change:
```tsx
          <div
            key={tab.id}
            className={`group flex items-center gap-0.5 rounded-md pl-2 pr-1 py-1 transition ${
```
to:
```tsx
          <div
            key={tab.id}
            role="presentation"
            className={`group flex items-center gap-0.5 rounded-md pl-2 pr-1 py-1 transition ${
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/WorkbenchTabBar.a11y.test.tsx`
Expected: PASS.

- [ ] **Step 6: Run the full component test suite**

Run: `cd crates/vox-gui/ui && npx vitest run`
Expected: all tests pass — `role="presentation"` doesn't change any existing
`getByRole`/keyboard-nav test since those already target the inner `role="tab"`
button, not the wrapper div.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx \
  crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.a11y.test.tsx
git commit -m "fix(gui): WorkbenchTabBar tablist direct children need role=tab or presentation (F-07)

ARIA 1.2's aria-required-children fired because the per-tab wrapper div
(holding the tab button + close button) had neither role. role=presentation
on the wrapper satisfies the spec without restructuring the DOM; this is
the highest-frequency axe violation in the review (51 instances on
dashboard alone) since the same tab bar renders on every surface."
```

---

### Task 7: missing `<h1>` per surface root — axe `page-has-heading-one`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`
- (Extend to other flagged surfaces in Step 4 once the pattern from these two is
  confirmed — do not guess at unread files' structure.)

- [ ] **Step 1: Confirm the current heading element in `ChatSurface.tsx`**

Run: `grep -n "text-\[10px\]\|uppercase tracking\|<h[1-6]\|useLabel('chat" crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx | head -10`
to find the surface's current title element (likely a styled `<span>`/`<div>`, not a
heading tag at all — that's the axe violation: zero `<h1>`s anywhere in the surface).

- [ ] **Step 2: Write the failing test**

In `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (or the
nearest existing test file for this surface — check first), add:
```tsx
  it('has exactly one h1 for the surface root (axe page-has-heading-one)', () => {
    render(<ChatSurface {...minimalRequiredProps} />);
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
  });
```
(`minimalRequiredProps` — reuse whatever prop-building helper the file's other tests
already use; do not fabricate new required props from scratch.)

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — zero `h1` elements found.

- [ ] **Step 4: Change the title element to `<h1>`, restyled to look identical**

Find the surface's title element (from Step 1) and change its tag to `h1` while
keeping its existing className unchanged, e.g. if it was:
```tsx
<span className="font-display text-[10px] uppercase tracking-[0.18em] text-brass">{useLabel('chat-sessions')}</span>
```
this is the *sessions rail* label, not the surface root title — locate the actual
top-level surface title instead (likely near the surface's outermost return, probably
undocumented as an `<h1>` anywhere). If `ChatSurface.tsx` genuinely has no visible
title text at all (some surfaces render straight into content with the title living
in the tab bar instead), add a visually-hidden `<h1>` instead of fabricating new
visible chrome:
```tsx
<h1 className="sr-only">Chat</h1>
```
placed as the first child of the surface's outermost container. Prefer promoting an
existing visible title element over adding a new `sr-only` one — only add `sr-only`
if no visible title exists to promote.

- [ ] **Step 5: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS.

- [ ] **Step 6: Repeat Steps 1-5 for `Dashboard.tsx`**

Same pattern: find or add exactly one `<h1>` for the dashboard surface root.

- [ ] **Step 7: Run the full component test suite**

Run: `cd crates/vox-gui/ui && npx vitest run`
Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat crates/vox-gui/ui/src/components/surfaces/Dashboard
git commit -m "fix(gui): add a single h1 per surface root for chat + dashboard (axe page-has-heading-one)

Highest-frequency axe instance after the tablist fix (39 on both chat
and dashboard). Promotes the existing title element to h1 where one
exists visibly; adds an sr-only h1 only where no visible title exists
to promote."
```

- [ ] **Step 9: Flag remaining surfaces as follow-up debt, do not fix them all in this task**

Run: `grep -rLn "role=\"heading\"\|<h1" crates/vox-gui/ui/src/components/surfaces/*/[A-Z]*.tsx` to
list surface root files with no heading element at all (beyond chat/dashboard, already
fixed). Record the list in the commit message body of Step 8 or as a code comment in
this plan's tracking issue — **do not silently expand this task's scope to cover all
31 surfaces**; the review named `chat`/`dashboard` as the two highest-instance-count
surfaces, and the remaining ones are lower-priority coverage debt per the design doc's
non-goals.

---

### Task 8: `landmark-unique` — distinct labels for duplicate landmarks

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/AppShell.tsx`

- [ ] **Step 1: Find the unlabeled/duplicate landmark elements**

Run: `grep -n "<nav\|<main\|role=\"navigation\"\|role=\"main\"" crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/components/layout/AppShell.tsx`

- [ ] **Step 2: Write the failing test**

In whichever of `Sidebar.test.tsx` / `AppShell.test.tsx` already exists, add:
```tsx
  it('nav landmarks have distinct accessible names (axe landmark-unique)', () => {
    render(<AppShell>{/* minimal children per existing test setup */}</AppShell>);
    const navs = screen.getAllByRole('navigation');
    const names = navs.map((n) => n.getAttribute('aria-label'));
    expect(new Set(names).size).toBe(names.length);
    expect(names.every(Boolean)).toBe(true);
  });
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/AppShell.test.tsx`
Expected: FAIL — one or both `<nav>` elements lack an `aria-label`, or share the same
one.

- [ ] **Step 4: Add distinct `aria-label`s**

Add `aria-label="Primary navigation"` to the sidebar's `<nav>` (in `Sidebar.tsx`) and
`aria-label="Workbench tabs"` (or similarly distinct, matching what it actually is) to
whichever second `<nav>`/`role="navigation"` element `AppShell.tsx` renders — pick
names that describe each landmark's actual content rather than generic placeholders.

- [ ] **Step 5: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/AppShell.test.tsx`
Expected: PASS.

- [ ] **Step 6: Run the full component test suite**

Run: `cd crates/vox-gui/ui && npx vitest run`
Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/components/layout/AppShell.tsx
git commit -m "fix(gui): distinct aria-labels for nav landmarks (axe landmark-unique)"
```

---

### Task 9: verify Phases 5's axe fixes via the harness

**Files:** none modified — verification only.

- [ ] **Step 1: Run the capture + analysis sequence** (same as Task 3 Steps 1-3) across
  all 31 surfaces (no `--grep` filter this time, since the axe rules are cross-surface).

- [ ] **Step 2: Confirm the axe counts dropped**

Run: `grep -c "aria-required-children\|page-has-heading-one\|landmark-unique" crates/vox-gui/ui/review-bundle/latest/bundle-digest.md`
Expected: substantially lower than the review's recorded baseline (`aria-required-children`:
dashboard 51/settings 31/approvals 25/chat 23; `page-has-heading-one`: 39 on chat and
dashboard — those two specifically should now be 0; `landmark-unique`: 29 on chat, 19 on
settings). `page-has-heading-one` on surfaces *not* touched by Task 7 (only chat/dashboard
were fixed) will still show violations — that's expected, not a regression; note it in the
verification output, don't chase it in this task.

- [ ] **Step 3: No commit for this task.**

---

### Task 10: `LudusSandbox` empty state — F-06

**Files:**
- Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx`
- Test: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.mappers.test.ts` (existing
  file — add a new test near the existing ones, or create
  `LudusSandbox.test.tsx` if the existing file only covers pure mapper functions,
  not the component itself — check first)

- [ ] **Step 1: Confirm whether a component-level test file exists**

Run: `ls crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx 2>/dev/null || echo none`
(the existing `.mappers.test.ts` tests pure functions, not the rendered component, per
its name — a new file is likely needed).

- [ ] **Step 2: Write the failing test**

Create `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx`:
```tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { LudusSandbox } from './LudusSandbox';

describe('LudusSandbox', () => {
  it('shows an explicit unavailable message instead of a blank canvas when there is no layout (F-06)', () => {
    render(<LudusSandbox />);
    // With no backend/profile data, `layout` never resolves — assert the
    // fallback text renders rather than a silently-blank canvas.
    expect(screen.getByText(/simulation unavailable/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL — no such text renders; the canvas is present but blank.

- [ ] **Step 4: Read the component's render return and `layout` state declaration**

Run: `grep -n "return (\|const \[layout\|useState.*layout" crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx | head -10`
to find exactly where `layout` is declared and where the component's JSX return
starts, so the fallback can be added without duplicating the canvas markup.

- [ ] **Step 5: Add the fallback**

In the component's render return, add a conditional sibling to the canvas (not a
replacement — keep the canvas mounted so the draw effect's refs stay valid once data
arrives):
```tsx
      {!layout && (
        <div className="flex h-full items-center justify-center text-xs text-text-muted">
          Simulation unavailable — no workspace data yet.
        </div>
      )}
```
placed adjacent to (not wrapping) the existing `<canvas>` element, likely toggling the
canvas's own visibility with a conditional className (e.g. `className={layout ? '' : 'hidden'}`
on the canvas) so exactly one of the two is visible at a time.

- [ ] **Step 6: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/gamify/LudusSandbox.test.tsx`
Expected: PASS.

- [ ] **Step 7: Run the full component test suite**

Run: `cd crates/vox-gui/ui && npx vitest run`
Expected: all tests pass, including the existing `LudusSandbox.mappers.test.ts`.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx \
  crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx
git commit -m "fix(gui): LudusSandbox shows an explicit unavailable message instead of a blank canvas (F-06)

The draw effect silently bailed (if (!layout) return) with zero
user-facing affordance whenever there's no workspace data — exactly the
no-backend/error-mock case the review's dashboard--no-backend--wide
cell caught."
```

---

### Task 11: F-05 re-verify (clipping) — triage only, no pre-guessed fix

**Files:** none modified until the verification result is known.

- [ ] **Step 1: Run the capture + analysis sequence** (same pattern as Task 3) scoped to
  `chat--rails-overlay-open--compact--firefox` and
  `chat--session-menu-open--compact--firefox`, now that Task 2 (F-01 fix) has landed.

- [ ] **Step 2: Read the resulting defect list for these two cells**

Run: `grep -B2 -A5 "rails-overlay-open--compact--firefox\|session-menu-open--compact--firefox" crates/vox-gui/ui/review-bundle/latest/bundle-digest.md`

- [ ] **Step 3a: If no clipping defect remains** — mark F-05 resolved (self-resolved as
  a symptom of F-01, matching the design doc's prediction). No further action, no
  commit needed for this task.

- [ ] **Step 3b: If a clipping defect still appears** — do not guess a fix here. Open a
  new, narrowly-scoped follow-up task (outside this plan) describing the *actual*
  remaining clipping behavior observed in the screenshot, since the design doc
  explicitly deferred this decision pending re-verification. Stop this task at
  triage; do not improvise a fix inline.

---

### Task 12: whole-effort verification sweep

**Files:** none modified — verification only.

- [ ] **Step 1: Full test suites**

Run:
```bash
cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit
```
Expected: all vitest tests pass, typecheck clean.

- [ ] **Step 2: Clippy + Rust tests (only if any Rust file changed — Tasks 1-11 are
  frontend-only, so this step should be a no-op check, not a real gate)**

Run: `cargo clippy -p vox-orchestrator-mcp --features gui-visual-review -- -D warnings`
Expected: clean (no Rust source changed by this plan, so this should already pass
unmodified).

- [ ] **Step 3: Full Playwright sweep (chromium)**

Run: `cd crates/vox-gui/ui && npx playwright test --project=chromium`
Expected: same pass/skip counts as the pre-remediation baseline (113 passed / 223
skipped — the skips are the env-gated review-capture matrix, not a regression), plus
any specs touched by Tasks 6-10's component changes still passing.

- [ ] **Step 4: Full review-bundle harness run, both browsers, all 31 surfaces**

Run the capture + analysis sequence from Task 3 with no `--grep` filter, full matrix.

- [ ] **Step 5: Confirm the review's F-01 through F-07 findings are resolved (or
  correctly triaged) in the fresh `bundle-digest.md`**

- F-01/F-04: 0 occlusion/layout defects on the previously-named cells.
- F-02: 0 `session_id`/`is_error` null-deref defects on the ~9 previously-affected
  surfaces' empty/error states.
- F-03: 0 `__TAURI_INTERNALS__`-or-similar-leak defects anywhere.
- F-05: resolved-or-triaged per Task 11's outcome.
- F-06: 0 blank-simulation-viewport defects on `gamify`/`dashboard`.
- F-07 + axe classes: `aria-required-children` near-zero across all surfaces (the
  shared tab-bar fix applies everywhere); `page-has-heading-one` zero specifically on
  `chat`/`dashboard` (other surfaces remain open per Task 7 Step 9's explicit scope
  note); `landmark-unique` zero.

- [ ] **Step 6: Regenerate contracts with a freshly-built release `vox`**

Run: `cargo build --release -p vox-cli --locked` (if stale), then
`./target/release/vox.exe ci gui-surface-coverage --write` and
`./target/release/vox.exe ci test-inventory --output contracts/reports/test-inventory.v1.json`.
Commit any diff.

- [ ] **Step 7: Push to main**

Follow the same admin-bypass direct-push pattern used for the harness-build effort
(no PR) — confirm with `git log origin/main -1 --oneline` after.

- [ ] **Step 8: Write a short remediation-results note**

Append a "Remediation status" section to the source review document
(`docs/superpowers/reviews/2026-07-18-axis-frontend-comprehensive-review.md`)
recording which findings are now fixed-and-verified vs. still-open (F-05 if triaged
as a separate bug in Task 11, and any surfaces Task 7 didn't reach). Commit this
alongside Step 6's contract regeneration.
