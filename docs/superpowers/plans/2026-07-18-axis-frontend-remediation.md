# Axis Frontend Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the six confirmed finding classes (F-01 through F-07, minus F-05 which
re-verifies-or-triages) from the 2026-07-18 Axis frontend comprehensive review, plus
the CI wiring gap that made the Firefox-only defects invisible.

**Architecture:** Twelve tasks in six phases. Phase 1 (CI) first because everything
else needs Firefox-aware CI to stay fixed; Task 11's F-05 re-verify explicitly waits
on Task 2. Visual/compositing fixes are proven by re-running the existing
`frontend-review.vox` harness; caught-exception and ARIA fixes get discriminating TDD
tests (toast-content and a11y-tree assertions — never "renders without throwing",
because every F-02 throw is caught and surfaces as a toast, not a crash).

**Tech Stack:** React + TypeScript (`crates/vox-gui/ui`), Vitest + Testing Library,
Playwright (existing `e2e/review/` harness), Style Dictionary (`tokens/*.json` →
three generated CSS files), GitHub Actions.

**Design doc:** [`docs/superpowers/specs/2026-07-18-axis-frontend-remediation-design.md`](../specs/2026-07-18-axis-frontend-remediation-design.md)

**Global prerequisites for every harness-verification task (3, 9, 11, 12):**
- `OPENROUTER_API_KEY` must be set locally (Clavis holds it) — the `--ai` analysis
  cannot run without it.
- The analysis digest is written to `contracts/reports/gui-visual-review/bundle-digest.md`
  (NOT under `review-bundle/`), and is **overwritten by every analysis run**. A stale
  committed digest exists at that path — always re-run analysis before grepping it.
- `e2e/review/globalSetup.ts` **deletes the entire `review-bundle/latest/` directory**
  on every capture run with `VOX_REVIEW_CAPTURE=1` — a `--grep`-scoped capture wipes
  all other surfaces' entries. Scoped runs are fine for spot verification; any
  full-matrix count comparison must come from a fresh unscoped run.

---

### Task 1: CI wiring — make Firefox visible to CI (install + capture + analyze)

**Files:**
- Modify: `.github/workflows/ci.yml` (the `gui-playwright-smoke` job: the
  `playwright install` line near 1663 and the review-bundle capture/analysis steps
  near 1676-1688)

- [ ] **Step 1: Confirm current line positions**

Run: `grep -n "playwright install\|Review-bundle capture\|Review-bundle AI defect analysis" .github/workflows/ci.yml`
Expected: an install line (`pnpm exec playwright install chromium`) plus the capture
and analysis step names. (There are two `playwright install` occurrences in the file —
edit the one inside the `gui-playwright-smoke` job, near line 1663.)

- [ ] **Step 2: Install Firefox in CI**

Change the `gui-playwright-smoke` job's install line from:
```yaml
        run: pnpm exec playwright install chromium
```
to:
```yaml
        run: pnpm exec playwright install chromium firefox
```
Without this, adding the Firefox project fails **silently** (the capture step is
`continue-on-error: true`) — CI goes green with zero Firefox entries, recreating the
exact invisibility this task exists to fix.

- [ ] **Step 3: Add the Firefox project to capture and `--browsers` to analysis**

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
(`--max-reviews` 40→60: cell count roughly doubles; still bounded, still advisory.
`--browsers` takes a comma list and `--max-reviews` exists — verified against
`src/bin/gui-visual-review.rs:27-30`.)

- [ ] **Step 4: Re-diff the block to confirm only these lines changed, then commit**

```bash
git diff .github/workflows/ci.yml
git add .github/workflows/ci.yml
git commit -m "ci(gui): install firefox + capture and review both browsers in the advisory review-bundle steps"
```

---

### Task 2: `Glass` opaque background — fix F-01/F-04 at the root

**Files:**
- Modify: `crates/vox-gui/ui/tokens/semantic.json`
- Modify: `crates/vox-gui/ui/tokens/semantic.travertine.json`
- Modify: `crates/vox-gui/ui/tailwind.config.js`
- Modify: `crates/vox-gui/ui/src/components/ui/Glass.tsx`
- Test: `crates/vox-gui/ui/src/components/ui/Glass.test.tsx` (existing file, has the
  jsdom pragma)

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-gui/ui/src/components/ui/Glass.test.tsx`:
```tsx
  it('uses an opaque background, not a low-alpha overlay tint (Firefox backdrop-blur compositing bug)', () => {
    render(<Glass data-testid="g">Content</Glass>);
    const el = screen.getByTestId('g');
    expect(el).toHaveClass('bg-overlay-solid');
    expect(el).not.toHaveClass('bg-overlay-subtle');
  });

  it('interactive hover state stays opaque too (no translucent hover regression)', () => {
    render(<Glass interactive data-testid="g">Clickable</Glass>);
    const el = screen.getByTestId('g');
    expect(el).not.toHaveClass('hover:bg-overlay-subtle');
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/ui/Glass.test.tsx`
Expected: FAIL — both new tests (base class is still `bg-overlay-subtle`, hover is
still `hover:bg-overlay-subtle`).

- [ ] **Step 3: Add the token to BOTH semantic token files**

In `crates/vox-gui/ui/tokens/semantic.json`, change:
```json
    "overlay": { "subtle": { "value": "rgba(255,255,255,0.04)" }, "hover": { "value": "rgba(255,255,255,0.07)" } }
```
to:
```json
    "overlay": { "subtle": { "value": "rgba(255,255,255,0.04)" }, "hover": { "value": "rgba(255,255,255,0.07)" }, "solid": { "value": "{color.basalt.850}" } }
```
(`basalt.850` = `#11151a`, opaque, one step lighter than `bg.base`/`basalt.900`.)

Then in `crates/vox-gui/ui/tokens/semantic.travertine.json`, find its `overlay` block
(it has its own — `subtle: rgba(0,0,0,0.04)`, `hover: rgba(0,0,0,0.07)`, black-based
for the light theme) and add a light-appropriate opaque `"solid"` alongside, using an
existing light primitive from `tokens/primitive.json` (a travertine/cream family value
near the theme's surface color — read the primitive file and pick the closest opaque
step above the travertine theme's `bg.surface`; do NOT reuse `basalt.850`, which is
near-black on a light theme). The travertine Style Dictionary build sources
`semantic.travertine.json` INSTEAD of `semantic.json`, so omitting this leaves that
theme without the variable. (The high-contrast build sources `semantic.json` plus an
override file, so it inherits automatically — no edit needed there.)

- [ ] **Step 4: Wire the Tailwind class**

In `crates/vox-gui/ui/tailwind.config.js` (lines ~26-27), extend:
```js
        'overlay-subtle': 'var(--color-overlay-subtle)',
        'overlay-hover': 'var(--color-overlay-hover)',
```
with:
```js
        'overlay-solid': 'var(--color-overlay-solid)',
```

- [ ] **Step 5: Regenerate the CSS and update `Glass.tsx` (base AND hover)**

Run: `cd crates/vox-gui/ui && pnpm tokens:build`
Expected: `--color-overlay-solid` appears in ALL THREE generated files
(`src/styles/tokens.generated.css`, `tokens.contrast.generated.css`,
`tokens.travertine.generated.css`) — verify with
`grep -l "color-overlay-solid" src/styles/*.generated.css` (expect 3 files).

Then in `crates/vox-gui/ui/src/components/ui/Glass.tsx` make TWO changes:

Line 34, change:
```tsx
        "relative border border-border-subtle bg-overlay-subtle backdrop-blur-2xl shadow-[0_1px_0_rgba(255,255,255,0.04)_inset,0_24px_60px_-30px_rgba(0,0,0,0.9)]",
```
to:
```tsx
        "relative border border-border-subtle bg-overlay-solid backdrop-blur-2xl shadow-[0_1px_0_rgba(255,255,255,0.04)_inset,0_24px_60px_-30px_rgba(0,0,0,0.9)]",
```

Line 37 (the `interactive &&` branch), change `hover:bg-overlay-subtle` to
`hover:bg-bg-elevated` (opaque `basalt.700` — hovering must not reintroduce the
translucent background Firefox mis-composites; this was the audit's hover-regression
finding). Leave everything else in that branch unchanged.

Deliberately UNCHANGED (do not "clean these up"):
- Line 42's `ring-overlay-subtle` inset ring — 1px decorative, not a background.
- Line 35's `isButton && "bg-transparent ..."` — `<Glass as="button">` stays
  transparent by design; buttons are not overlay panels (twMerge resolves
  `bg-overlay-solid` + later `bg-transparent` to transparent).

- [ ] **Step 6: Check call-site background overrides**

Run: `grep -rn "<Glass" crates/vox-gui/ui/src --include="*.tsx" -A2 | grep "bg-"`
Known translucent overrides that WIN via twMerge and stay translucent:
`DueNudge.tsx` (`bg-zinc-950/65`), `FunGauge.tsx`/`HudPanels.tsx` (`bg-zinc-950/80`),
`RunsView.tsx:231` (`bg-black/30`). These are small HUD chips, not full-panel
overlays — leave them. Note any NEW full-panel override the grep surfaces beyond
these four; only a full-panel translucent override would need the same treatment.

- [ ] **Step 7: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/ui/Glass.test.tsx`
Expected: PASS (both new tests plus the two pre-existing ones).

- [ ] **Step 8: Run the full component test suite (regression check)**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run`
Expected: all files pass. (Verified: no existing test asserts `bg-overlay-subtle`.)

- [ ] **Step 9: Commit**

```bash
git add crates/vox-gui/ui/tokens/semantic.json crates/vox-gui/ui/tokens/semantic.travertine.json \
  crates/vox-gui/ui/tailwind.config.js crates/vox-gui/ui/src/styles/tokens.generated.css \
  crates/vox-gui/ui/src/styles/tokens.contrast.generated.css crates/vox-gui/ui/src/styles/tokens.travertine.generated.css \
  crates/vox-gui/ui/src/components/ui/Glass.tsx crates/vox-gui/ui/src/components/ui/Glass.test.tsx
git commit -m "fix(gui): Glass uses an opaque background (base + hover) instead of a low-alpha overlay tint

Firefox composites backdrop-blur over a ~4%-alpha background differently
than Chromium, letting underlying content bleed through at full opacity
(F-01/F-04: AchievementsDrawer, chat rail overlays, console DiscoveryRail).
bg-overlay-solid (basalt.850, opaque) replaces bg-overlay-subtle on the
base AND the interactive hover state; travertine theme gets its own
light-appropriate solid token. Accepted global change: all Glass panels
are now opaque, trading translucent elevation cues for correctness."
```

---

### Task 3: verify F-01/F-04 fixed via the harness

**Files:** none modified — verification only.
**Prereq:** `OPENROUTER_API_KEY` set (see Global prerequisites).

- [ ] **Step 1: Run the capture scoped to the affected surfaces, both browsers**

```bash
cd crates/vox-gui/ui
VOX_REVIEW_CAPTURE=1 npx playwright test e2e/review/capture.spec.ts \
  --project=chromium --project=firefox-review --workers=2 \
  --grep "chat|dashboard|console"
```
Expected: exit 0. NOTE: this DELETES all prior `review-bundle/latest/` entries
(globalSetup clears the directory) — only chat/dashboard/console cells exist after.
That is fine for this spot check; Task 12 re-runs the full matrix.
(Title grep works because test titles are `` `${surface} -- ${state} -- ${viewport}` ``
and the only registry viewKeys containing these substrings are exactly `chat`,
`dashboard`, `console`.)

- [ ] **Step 2: Run the analysis**

```bash
cargo run -p vox-orchestrator-mcp --features gui-visual-review --bin gui-visual-review -- \
  --bundle crates/vox-gui/ui/review-bundle/latest --ai --browsers chromium,firefox
```
Expected: frontier drains (0 deferred; loop with repeated invocations if a
`--max-reviews` bound is hit, same as the harness effort did).

- [ ] **Step 3: Confirm the named regression cells are clean**

Run: `grep -A3 "rails-overlay-open--compact--firefox\|achievements-open\|console--default--compact--firefox\|session-menu-open--compact--firefox" contracts/reports/gui-visual-review/bundle-digest.md`
(The digest path is `contracts/reports/gui-visual-review/bundle-digest.md` — the
analysis run just overwrote it; never grep it before running Step 2.)
Expected: no `occlusion` or `layout` defects for these cells; Firefox scores
comparable to Chromium counterparts (75+, vs the review's recorded 15-45).

Also eyeball 2-3 chromium `dashboard--default` PNGs under
`crates/vox-gui/ui/review-bundle/latest/` for the accepted global change (cards now
opaque): confirm nothing looks broken, just less translucent.

If any named cell still shows occlusion/layout defects, stop and re-open Task 2.

- [ ] **Step 4: No commit** (bundle dir is gitignored; the digest/cache under
  `contracts/` will be committed once, at Task 12, from the final full-matrix run —
  not from this scoped run).

---

### Task 4: sanitize toast error bodies repo-wide + guard — fix F-03

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/backendGuard.ts`
- Test: `crates/vox-gui/ui/src/lib/backendGuard.test.ts` (existing file — extend it)
- Create: `crates/vox-gui/ui/src/guards/toastBodyGuard.test.ts`
- Modify: ALL files under `crates/vox-gui/ui/src` containing `body: String(` —
  14 sites in `App.tsx` (lines 405, 408, 667, 682, 757, 795, 831, 861, 899, 910,
  917, 924, 948, 1052) plus ~95 sites across ~27 other files (`BrowserView.tsx` ×13,
  `SettingsView.tsx` ×17, `SkillsPluginsView.tsx` ×10, `MemoryView.tsx`,
  `ChatSurface.tsx`, `ApprovalsView.tsx`, `DiscoveryReview.tsx`, `Loquela.tsx`,
  `InlineApprovals.tsx`, and the rest surfaced by the Step 5 grep)

- [ ] **Step 1: Write the failing unit test**

`crates/vox-gui/ui/src/lib/backendGuard.test.ts` ALREADY imports
`BackendUnavailableError` (line ~3-8). Add `sanitizeErrorForToast` to the EXISTING
import list — do NOT add a second import statement (duplicate-identifier compile
error). Then append:
```ts
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

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/lib/backendGuard.test.ts`
Expected: FAIL — `sanitizeErrorForToast` is not exported (the file loads fine because
the import merge in Step 1 was into the existing statement).

- [ ] **Step 3: Implement `sanitizeErrorForToast`**

Add to `crates/vox-gui/ui/src/lib/backendGuard.ts` (after `BackendUnavailableError`):
```ts
/**
 * Toast bodies must never leak raw IPC internals (F-03: a caught rejection's
 * String(err) rendering __TAURI_INTERNALS__ verbatim in a user-visible toast).
 * Distinct from the unhandledrejection filter — this runs on *caught*
 * exceptions the app chooses to display. \binvoke\b does not match
 * invoke_mcp_tool (underscore is a word char); prose like "failed to invoke X"
 * degrades to the generic message, which is acceptable.
 */
const LEAK_PATTERN = /__TAURI_INTERNALS__|\binvoke\b/;

export function sanitizeErrorForToast(err: unknown): string {
  if (err instanceof BackendUnavailableError) return err.message;
  const text = String(err);
  return LEAK_PATTERN.test(text) ? 'An unexpected error occurred.' : text;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/lib/backendGuard.test.ts`
Expected: PASS.

- [ ] **Step 5: Write the failing guard test, then replace repo-wide**

Create `crates/vox-gui/ui/src/guards/toastBodyGuard.test.ts`:
```ts
import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

/** F-03 guard: toast bodies must go through sanitizeErrorForToast, never raw
 * String(err) — a raw Tauri TypeError's text contains __TAURI_INTERNALS__. */
const SRC_ROOT = join(import.meta.dirname, '..');

function* walk(dir: string): Generator<string> {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else if (/\.(ts|tsx)$/.test(name) && !/\.test\.(ts|tsx)$/.test(name)) yield p;
  }
}

describe('toast body sanitization containment', () => {
  it('no raw `body: String(` anywhere under src/', () => {
    const offenders: string[] = [];
    for (const file of walk(SRC_ROOT)) {
      const src = readFileSync(file, 'utf8');
      if (/body:\s*String\(/.test(src)) offenders.push(file.replace(SRC_ROOT, 'src'));
    }
    expect(offenders, `use sanitizeErrorForToast instead: ${offenders.join(', ')}`).toEqual([]);
  });
});
```

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/guards/toastBodyGuard.test.ts`
Expected: FAIL, listing ~28 files.

Then do the mechanical replacement across ALL listed files: every
`body: String(err)` → `body: sanitizeErrorForToast(err)` (matching each site's
actual catch-variable name — `err`, `e`, etc.), adding
`import { sanitizeErrorForToast } from '<relative>/lib/backendGuard';` to each file
(correct relative depth per file). Use per-file edits, not a blind sed — TSX import
blocks vary.

- [ ] **Step 6: Run guard + full suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/guards/toastBodyGuard.test.ts && pnpm exec vitest run && npx tsc --noEmit`
Expected: guard passes; full suite passes (the one existing raw-body assertion,
`CodeRabbitView.test.tsx:72` asserting `'boom'`, still passes — `'boom'` doesn't trip
LEAK_PATTERN); typecheck clean (catches any wrong relative import path).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src
git commit -m "fix(gui): sanitize ALL toast error bodies + guard test (F-03)

sanitizeErrorForToast() special-cases BackendUnavailableError and blocks
__TAURI_INTERNALS__/invoke text from user-visible toasts. Replaces ~109
raw body: String(err) sites across 28 files (App.tsx was only ~13% of
the class), and adds a source-scan guard (toastBodyGuard.test.ts) so the
pattern cannot regrow."
```

---

### Task 5: null-deref guards — fix F-02 (toast-content TDD, not crash TDD)

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (three sites: the `chat_create_session`
  fallback at ~403-405; the rollback handler at ~785-793; the audit handler's BOTH
  derefs at ~807-815 (`out`) and ~820-829 (`res`, deref at 821 precedes the pushToast))
- Test: `crates/vox-gui/ui/src/App.test.tsx` (existing file)

**Test-harness facts (verified):** `App.test.tsx` mocks `@tauri-apps/api/core` with
an inline `vi.fn().mockResolvedValue(null)` (no exported `mockInvoke` handle — use
`vi.mocked((await import('@tauri-apps/api/core')).invoke)` or restructure the mock to
expose a handle); all tests render via a `renderApp()` helper wrapping
`LanguageProvider` + `QueryClientProvider` — bare `render(<App />)` fails. There is
no `app-shell` testid anywhere. Every F-02 throw is CAUGHT (adjacent `.catch` /
`try-catch`) and surfaces as a warn toast — tests MUST assert toast content; a
"renders without throwing" test passes before the fix and proves nothing (the
existing smoke test already exercises the null path silently).

- [ ] **Step 1: Restructure the core mock to an accessible handle (test-infra only, no behavior change)**

In `App.test.tsx`, convert the inline mock to the hoisted-handle pattern the repo
uses elsewhere (see `ChatSurface.test.tsx`'s module-level `invokeMock` for the
house style):
```ts
const invokeMock = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
```
with a `beforeEach(() => { invokeMock.mockReset(); invokeMock.mockResolvedValue(null); })`
preserving the existing default. Run the file's existing tests to confirm no
regression: `pnpm exec vitest run src/App.test.tsx` → all pre-existing tests pass.

- [ ] **Step 2: Write the failing session-null test (asserts on TOAST, not crash)**

```tsx
  it('null chat_create_session result produces no leaky "Chat session" toast (F-02)', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      return Promise.resolve(null); // chat_create_session -> null
    });
    renderApp();
    // Let the mount effect's promise chain settle.
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('chat_create_session', expect.anything()));
    await waitFor(() => {
      expect(screen.queryByText('Chat session')).toBeNull();
    });
  });
```
(Before the fix: null `s` → `s.session_id` throws inside `.then` → caught → a warn
toast titled 'Chat session' with the TypeError text renders → `queryByText` finds it
→ FAIL. After the fix: the guard skips silently → PASS.)

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/App.test.tsx`
Expected: FAIL — the 'Chat session' toast is present (rendered from the caught
TypeError).

- [ ] **Step 4: Fix the session-creation call site**

`App.tsx:403-405`, change:
```ts
            invoke<Session>('chat_create_session', { title: 'Chat' })
              .then((s) => setActiveSessionId(s.session_id))
```
to:
```ts
            invoke<Session>('chat_create_session', { title: 'Chat' })
              .then((s) => { if (s?.session_id) setActiveSessionId(s.session_id); })
```
(The `.catch` on the next line already uses `sanitizeErrorForToast` after Task 4.)

- [ ] **Step 5: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/App.test.tsx`
Expected: PASS.

- [ ] **Step 6: Read the rollback + audit handlers and identify their UI triggers**

Run: `sed -n '770,835p' crates/vox-gui/ui/src/App.tsx`
Confirm: rollback failure body is currently
`typeof res.result === 'string' ? res.result : JSON.stringify(res.result)` (there is
NO `res.error` field — `McpInvokeResult` is `{ tool, is_error, result }` per
`src/lib/mcpToolResult.ts:6-9`); the audit handler derefs `out.exit_code`/`out.stdout`
around 807-815 and `res.result` at 821-823 BEFORE its pushToast. Identify the
handler names and how `App.test.tsx` can drive them (they are invoked via the
command/palette dispatch — find the dispatching action or exported callback and
drive it the same way existing tests in the file drive interactions; if no existing
test drives these handlers, drive them via the UI element that calls them, located
by reading where the handlers are passed as props).

- [ ] **Step 7: Write the failing rollback/audit null-res test (asserts toast BODY content)**

Following the trigger mechanism identified in Step 6:
```tsx
  it('null MCP rollback/audit result produces an honest failure toast, not a TypeError leak (F-02)', async () => {
    invokeMock.mockResolvedValue(null); // every backend call resolves null
    renderApp();
    // drive the rollback trigger identified in Step 6
    // then:
    await waitFor(() => {
      const leak = screen.queryByText(/is_error|session_id|TypeError|null/i);
      expect(leak).toBeNull();
    });
  });
```
Expected on run: FAIL — the caught TypeError's text (mentioning `is_error`/null)
renders in the failure toast body.

- [ ] **Step 8: Fix all three remaining deref sites**

Rollback (`App.tsx:785-793`) — guard once, PRESERVE the real failure text:
```ts
          const failed = !res || res.is_error;
          pushToast({
            tone: failed ? 'warn' : 'ok',
            title: failed ? 'Rollback failed' : 'Rollback complete',
            body: !res
              ? 'No response from the backend.'
              : failed
                ? sanitizeErrorForToast(typeof res.result === 'string' ? res.result : JSON.stringify(res.result))
                : /* keep the existing success-branch text verbatim */,
            cause: failed ? 'backend-error' : 'backend-ok',
          });
```
Audit (`App.tsx:807-829`) — guard BOTH derefs with the same pattern: `out` (`!out ||
out.exit_code !== 0` style, preserving current semantics with a leading null check)
and `res` (`const failed = !res || res.is_error;` BEFORE the line-821
`typeof res.result` read, which becomes `!res ? 'No response from the backend.' :
<existing expression>`). Keep every success-branch string verbatim; only add null
guards and route failure text through `sanitizeErrorForToast`.

- [ ] **Step 9: Run tests to verify they pass, then the full suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/App.test.tsx && pnpm exec vitest run`
Expected: all pass.

- [ ] **Step 10: Verify against the harness (empty/error states of the ~9 affected surfaces)**

Capture + analyze (same commands as Task 3 Steps 1-2) with
`--grep "empty|error"` and no surface filter, then:
`grep -i "session_id\|is_error" contracts/reports/gui-visual-review/bundle-digest.md`
Expected: no null-deref defect descriptions remain. (Reminder: this scoped capture
wiped the Task 3 bundle — fine; Task 12 re-runs the full matrix.)

- [ ] **Step 11: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/App.test.tsx
git commit -m "fix(gui): guard the four null-deref sites feeding leaky warn toasts (F-02)

chat_create_session's .then, the rollback res.is_error read, and the
audit handler's out.* and res.result derefs all assumed non-null
resolutions; empty/error mock states resolve null, and the CAUGHT
TypeErrors surfaced as raw toast bodies across ~9 surfaces. Tests assert
toast content (queryByText), not 'does not throw' — the throws were
always caught, so crash tests cannot discriminate."
```

---

### Task 6: WorkbenchTabBar ARIA restructure — F-07

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.test.tsx`
  (EXISTS — extend it, do not create an `.a11y.test.tsx`)
- Modify: `crates/vox-gui/ui/e2e/workbench-tabs.spec.ts` (3 assertions target the
  old structure)

**Why a restructure, not `role="presentation"`:** axe's `aria-required-children`
computes owned children by looking THROUGH presentational/unroled wrappers — the
current unroled div is already looked through (that's why the rule fires today), and
`role="presentation"` on it changes nothing: the close `<button>` remains an owned
non-`tab` child. The correct pattern: the wrapper IS the tab; nested interactive
elements are flattened by `tab`'s children-presentational semantics, so the close
affordance must be presentational to AT with a keyboard alternative on the tab
itself.

- [ ] **Step 1: Write the failing test (a11y-tree assertions that model axe's computation)**

Add to the EXISTING `WorkbenchTabBar.test.tsx`:
```tsx
  it('tablist owns only tabs: every direct child is role=tab and no buttons exist in the a11y tree (F-07)', () => {
    render(
      <WorkbenchTabBar
        tabs={[{ id: 'chat', label: 'Chat', pinned: true }, { id: 'console', label: 'Console' }]}
        activeTab="chat"
        onSelect={() => {}}
        onClose={() => {}}
      />,
    );
    const tablist = screen.getByRole('tablist');
    for (const child of Array.from(tablist.children)) {
      expect(child.getAttribute('role')).toBe('tab');
    }
    // Buttons inside a tablist are what aria-required-children actually flags;
    // testing-library's role queries respect aria-hidden, approximating axe.
    expect(within(tablist).queryAllByRole('button')).toEqual([]);
  });

  it('Delete key on a focused tab closes it (keyboard replacement for the AT-hidden close affordance)', async () => {
    const onClose = vi.fn();
    render(
      <WorkbenchTabBar
        tabs={[{ id: 'chat', label: 'Chat', pinned: true }, { id: 'console', label: 'Console' }]}
        activeTab="console"
        onSelect={() => {}}
        onClose={onClose}
      />,
    );
    const tab = screen.getByRole('tab', { name: /console/i });
    tab.focus();
    await userEvent.keyboard('{Delete}');
    expect(onClose).toHaveBeenCalledWith('console');
  });
```
(Props verified against `WorkbenchTabBarProps` — `{ tabs: {id,label,badge?,pinned?}[],
activeTab, onSelect, onClose }`. Import `within` from `@testing-library/react` and
`userEvent` per the file's existing imports.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/layout/WorkbenchTabBar.test.tsx`
Expected: FAIL — direct children are unroled divs; close buttons are in the tree;
Delete does nothing.

- [ ] **Step 3: Restructure the component**

In `WorkbenchTabBar.tsx`, replace the per-tab block (lines ~32-68) with: the wrapper
div becomes the tab —
```tsx
          <div
            key={tab.id}
            role="tab"
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            data-testid={`workbench-tab-${tab.id}`}
            onClick={() => onSelect(tab.id)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') onSelect(tab.id);
              if (e.key === 'Delete' && !tab.pinned) onClose(tab.id);
            }}
            className={`group flex cursor-pointer items-center gap-0.5 rounded-md pl-2 pr-1 py-1 transition ${
              selected
                ? 'bg-overlay-subtle text-text-primary ring-1 ring-white/10'
                : 'text-text-muted hover:bg-overlay-subtle hover:text-text-secondary'
            }`}
          >
            <span className="font-display text-[10px] uppercase tracking-[0.18em]">
              {tab.label}
              {tab.badge != null && tab.badge > 0 ? (
                <span className="ml-1.5 rounded-full bg-brass/20 px-1.5 text-[9px] text-brass">
                  {tab.badge}
                </span>
              ) : null}
            </span>
            {!tab.pinned ? (
              <span
                aria-hidden="true"
                onClick={(e) => {
                  e.stopPropagation();
                  onClose(tab.id);
                }}
                className="flex size-5 cursor-pointer items-center justify-center rounded opacity-60 transition hover:bg-white/10 hover:opacity-100"
              >
                <Icon.x className="size-3" />
              </span>
            ) : null}
          </div>
```
Key moves: `role="tab"`/`aria-selected`/`data-testid`/select-handler to the wrapper;
inner label `<button>` → `<span>`; close `<button>` → `aria-hidden` `<span>` (still
pointer-clickable; AT users close via `Delete`); roving `tabIndex` so keyboard focus
lands on the selected tab.

- [ ] **Step 4: Update the existing unit tests that target the old structure**

Run: `pnpm exec vitest run src/components/layout/WorkbenchTabBar.test.tsx` and fix
the pre-existing assertions the restructure breaks: `getByRole('tab', { name })`
still works (name now computed from the wrapper's content — note the accessible name
may now include badge text, e.g. "Console 3"; use `{ name: /console/i }` regex
matchers); `getByRole('button', { name: 'Close X' })` no longer matches — those
assertions become pointer-click tests on the close element located via
`container.querySelector` or a new `data-testid="workbench-tab-close-<id>"` added to
the close span (add the testid — cleaner than querySelector).

- [ ] **Step 5: Update the 3 e2e assertions in `e2e/workbench-tabs.spec.ts`**

Run: `grep -n "Close \|aria-selected\|workbench-tab-" crates/vox-gui/ui/e2e/workbench-tabs.spec.ts`
The `aria-selected`-on-testid assertion now targets the same element (testid moved
WITH aria-selected to the wrapper — no change needed, verify). The
`getByRole('button', { name: 'Close Console' })` locator(s) change to the new close
testid: `page.getByTestId('workbench-tab-close-console')`.

- [ ] **Step 6: Run unit + e2e to verify**

```bash
cd crates/vox-gui/ui && pnpm exec vitest run src/components/layout/WorkbenchTabBar.test.tsx
npx playwright test e2e/workbench-tabs.spec.ts --project=chromium
```
Expected: all pass.

- [ ] **Step 7: Run the full component suite, then commit**

```bash
cd crates/vox-gui/ui && pnpm exec vitest run
git add crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx \
  crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.test.tsx \
  crates/vox-gui/ui/e2e/workbench-tabs.spec.ts
git commit -m "fix(gui): WorkbenchTabBar — wrapper becomes the tab; close affordance AT-hidden with Delete-key close (F-07)

role=presentation on the wrapper would NOT fix aria-required-children:
axe looks through presentational wrappers and still finds the close
button as an owned non-tab child (exactly why the unroled div already
violates today). Restructure per the canonical pattern: tab owns the
label (children-presentational), close is aria-hidden + pointer-only,
keyboard close via Delete on the focused tab. Highest-frequency axe
violation in the review (51 instances on dashboard alone)."
```

---

### Task 7: sr-only `<h1>` for chat + dashboard — axe `page-has-heading-one`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx`
  (exists; mocks `@tauri-apps/api/core` with a module-level `invokeMock` and renders
  inline `<LanguageProvider><ChatSurface pushToast={...} .../></LanguageProvider>` —
  there is NO `minimalRequiredProps` helper; `pushToast` is the only required prop)
- Test: the Dashboard surface's existing test file (locate via
  `ls crates/vox-gui/ui/src/components/surfaces/Dashboard/*.test.tsx` and extend the
  one that renders the full Dashboard; if none renders the full Dashboard, add the
  h1 test to a new `Dashboard.h1.test.tsx` following ChatSurface.test.tsx's mock
  pattern)

**Verified facts:** neither surface has ANY `<h1>`; neither has a visible root title
to promote (`Dashboard.tsx`'s `<h2>The Stream</h2>` at line 220 is a section heading
— promoting it would mislabel a section). The sr-only path is the correct one for
both. `SurfaceMiniRender`'s `aria-hidden="true"` frame keeps embedded mini-surfaces'
h1s out of the a11y tree, so dashboard stays at exactly one accessible h1.

- [ ] **Step 1: Write the failing ChatSurface test**

Add to `ChatSurface.test.tsx`, following its existing render pattern:
```tsx
  it('has exactly one accessible h1 for the surface root (axe page-has-heading-one)', async () => {
    render(
      <LanguageProvider>
        <ChatSurface pushToast={() => {}} activeSessionId="s1" />
      </LanguageProvider>,
    );
    expect(await screen.findAllByRole('heading', { level: 1 })).toHaveLength(1);
  });
```
(`findAllByRole` — async-tolerant; the component fires `chat_list_sessions` on mount
via the file's existing mock.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — zero h1 elements.

- [ ] **Step 3: Add the sr-only h1**

In `ChatSurface.tsx`, as the first child of the component's outermost container:
```tsx
      {/* Axe page-has-heading-one: surfaces render inside a heading-less shell.
          NOTE: if chatDocked (App.tsx, currently hardcoded false) is ever
          enabled, a docked ChatSurface adds a second h1 to the page. */}
      <h1 className="sr-only">Chat</h1>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS.

- [ ] **Step 5: Repeat Steps 1-4 for `Dashboard.tsx`** (`<h1 className="sr-only">Dashboard</h1>`,
  same first-child placement, same test shape in the Dashboard test file identified
  above).

- [ ] **Step 6: Full suite + commit**

```bash
cd crates/vox-gui/ui && pnpm exec vitest run
git add crates/vox-gui/ui/src/components/surfaces/Chat crates/vox-gui/ui/src/components/surfaces/Dashboard
git commit -m "fix(gui): sr-only h1 for chat + dashboard surface roots (axe page-has-heading-one)

Neither surface had any h1 nor a visible root title to promote
(Dashboard's 'The Stream' h2 is a section heading). SurfaceMiniRender's
aria-hidden frame keeps embedded mini-surface h1s out of the a11y tree,
so dashboard stays at exactly one accessible h1."
```

- [ ] **Step 7: Enumerate remaining surfaces as recorded debt (no fixes)**

Run: `grep -rLn "<h1" crates/vox-gui/ui/src/components/surfaces/*/[A-Z]*View.tsx crates/vox-gui/ui/src/components/surfaces/*/[A-Z]*Surface.tsx 2>/dev/null`
Paste the resulting list into the Task 12 Step 8 remediation-status note as
"page-has-heading-one debt (out of scope per spec)". Do NOT fix them here.

---

### Task 8: `landmark-unique` — label the real landmarks (nav + 3 asides + repeated regions)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` (unlabeled `<nav>` at
  ~162 AND unlabeled `<aside>` at ~134)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx`
  (`<aside>` at ~52 and ~71)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx`
  (`<aside>` at ~96/115; repeated `role="region"` at ~133/160)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatAgentEventRow.tsx` /
  `ModelBadge.tsx` (per-message `role="region"` at ModelBadge.tsx:48)
- Test: `crates/vox-gui/ui/src/components/layout/Sidebar.branding.test.tsx` or a new
  `Sidebar.landmarks.test.tsx`; `ChatSessionRail`/`ChatExecutionRail` component test
  files (extend existing ones if present — check with `ls`)

**Verified facts:** there is NO second nav to label (`AppShell.tsx` renders none;
`BreadcrumbBar.tsx:52` is already labeled `"Breadcrumb"`). The review's chat count
(29) comes from unlabeled asides + per-message `role="region"` duplicates. Do NOT
put these tests in `AppShell.test.tsx` — it mocks Sidebar with an already-labeled
stub, so a test there passes without any fix.

- [ ] **Step 1: Write the failing Sidebar test**

In a new `crates/vox-gui/ui/src/components/layout/Sidebar.landmarks.test.tsx`
(reuse the render/mock pattern from the existing `Sidebar.branding.test.tsx` — read
it first and copy its setup):
```tsx
  it('sidebar nav and aside landmarks carry aria-labels (axe landmark-unique)', () => {
    renderSidebar(); // per the branding test's setup helper/pattern
    expect(screen.getByRole('navigation')).toHaveAttribute('aria-label');
    expect(screen.getByRole('complementary')).toHaveAttribute('aria-label');
  });
```

- [ ] **Step 2: Run to verify it fails, then label Sidebar's landmarks**

Run: `pnpm exec vitest run src/components/layout/Sidebar.landmarks.test.tsx` → FAIL.
Add `aria-label="Primary navigation"` to `Sidebar.tsx`'s `<nav>` (~162) and
`aria-label="Sidebar"` to its `<aside>` (~134). Re-run → PASS.

- [ ] **Step 3: Label the chat rails' asides**

`ChatSessionRail.tsx` — both return branches' `<aside>` (collapsed ~52, expanded
~71): `aria-label="Chat sessions"`. `ChatExecutionRail.tsx` — both branches (~96,
~115): `aria-label="Execution rail"`. Extend each component's existing test file
(check `ls crates/vox-gui/ui/src/components/surfaces/Chat/*.test.tsx`) with the same
one-line `toHaveAttribute('aria-label')` assertion pattern as Step 1; where no test
file exists for a rail, add the assertion to `ChatSurface.test.tsx` (which mounts
both rails) instead of creating new files.

- [ ] **Step 4: De-landmark the per-message regions**

`ModelBadge.tsx:48` and `ChatExecutionRail.tsx:133,160`: read each site. If the
`role="region"` conveys nothing a heading/label doesn't already (per-message chrome),
remove the `role` attribute entirely; if it is genuinely a labeled region users
navigate to, make its `aria-label` unique per instance (include the model/message
identity). Default to removal — landmark-per-message is noise for AT users, which is
exactly what `landmark-unique` flags.

- [ ] **Step 5: Full suite + commit**

```bash
cd crates/vox-gui/ui && pnpm exec vitest run
git add crates/vox-gui/ui/src
git commit -m "fix(gui): label the real duplicate landmarks — nav + 3 asides, de-landmark per-message regions (axe landmark-unique)

There was no unlabeled second nav (BreadcrumbBar is labeled; AppShell
renders none) — the review's chat count came from unlabeled asides
(Sidebar, ChatSessionRail, ChatExecutionRail) and role=region repeated
once per message (ModelBadge, ChatExecutionRail)."
```

---

### Task 9: verify the axe-class fixes via a full-matrix harness run

**Files:** none modified — verification only.
**Prereq:** `OPENROUTER_API_KEY`.

- [ ] **Step 1: FULL-matrix capture + analysis** (no `--grep` — the axe counts are
  cross-surface, and prior scoped runs deleted earlier entries):

```bash
cd crates/vox-gui/ui
VOX_REVIEW_CAPTURE=1 npx playwright test e2e/review/capture.spec.ts \
  --project=chromium --project=firefox-review --workers=4
cd ../../..
cargo run -p vox-orchestrator-mcp --features gui-visual-review --bin gui-visual-review -- \
  --bundle crates/vox-gui/ui/review-bundle/latest --ai --browsers chromium,firefox
```
(Loop the analysis invocation until 0 deferred, as the harness effort did.)

- [ ] **Step 2: Confirm the axe counts dropped in the fresh digest**

Run: `grep -c "aria-required-children\|page-has-heading-one\|landmark-unique" contracts/reports/gui-visual-review/bundle-digest.md`
Expected vs the review baseline:
- `aria-required-children`: near-zero everywhere (the shared tab bar renders on every
  surface, so Task 6's restructure applies globally).
- `page-has-heading-one`: 0 on `chat` and `dashboard` specifically. Other surfaces
  still violate — EXPECTED, recorded as debt by Task 7 Step 7, not a regression.
- `landmark-unique`: 0 on `chat` (the asides + regions were the whole count) and
  materially reduced elsewhere; if `settings` (baseline 19) still shows instances,
  read which landmarks they are and record them in the Task 12 status note — only
  chat-path landmarks were in scope.

- [ ] **Step 3: No commit** (artifacts committed once at Task 12).

---

### Task 10: `LudusSandbox` loading/no-data state — F-06 (corrected premise)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx`
- Test: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` — **EXISTS**
  (60-line suite mocking `../../transport`; already asserts the `scanFailed` state's
  "scan unavailable" text). EXTEND it; do not create or clobber.

**Verified facts:** `scanFailed` already renders "Workspace scan unavailable — the
town cannot render." (`LudusSandbox.tsx:298-303`) and is already tested. The
UNCOVERED state is `!layout && !scanFailed` — scan pending or resolved-empty — which
leaves the mounted canvas blank (every draw effect early-returns on `!layout`).

- [ ] **Step 1: Read the existing test file's transport-mock setup**

Run: `cat crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx`
Note how it mocks `workspaceTownScan` (resolve/reject shapes) — the new test reuses
this mock, making the scan PEND (never resolve) or resolve with a shape that yields
no layout.

- [ ] **Step 2: Write the failing test**

Add to the existing file, using its mock idiom:
```tsx
  it('shows a loading/no-data affordance instead of a blank canvas while layout is unset (F-06)', async () => {
    // Make the scan hang (pending forever): the component has neither layout
    // nor scanFailed — the previously-blank state.
    workspaceTownScanMock.mockImplementation(() => new Promise(() => {}));
    render(<LudusSandbox />);
    expect(await screen.findByText(/simulation loading|no workspace data/i)).toBeInTheDocument();
  });
```
(Adapt the mock name to the file's actual identifier from Step 1.)

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/gamify/LudusSandbox.test.tsx`
Expected: the NEW test fails (no such text; canvas mounted blank); the existing
tests still pass.

- [ ] **Step 4: Add the fallback for `!layout && !scanFailed`**

In `LudusSandbox.tsx`'s main return (after the `scanFailed` early-return at ~298),
inside the wrapper div, add a sibling to the canvas:
```tsx
        {!layout && (
          <div className="absolute inset-0 flex items-center justify-center text-xs text-text-muted">
            Simulation loading — no workspace data yet.
          </div>
        )}
```
and hide the blank canvas while unset: add `${layout ? '' : 'invisible'}` to the
canvas's className (keep it MOUNTED — the draw effects hold refs to it).

- [ ] **Step 5: Run tests to verify all pass, then the full suite + commit**

```bash
cd crates/vox-gui/ui && pnpm exec vitest run src/components/gamify/LudusSandbox.test.tsx && pnpm exec vitest run
git add crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx \
  crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx
git commit -m "fix(gui): LudusSandbox shows a loading affordance while layout is unset (F-06)

scanFailed was already handled and tested; the uncovered state was
scan-pending/resolved-empty, where every draw effect early-returns on
!layout and the mounted canvas stays silently blank — the review's
dashboard no-backend / gamify error cells."
```

---

### Task 11: F-05 re-verify (clipping) — triage only, no pre-guessed fix

**Files:** none modified until the verification result is known.
**Prereq:** `OPENROUTER_API_KEY`. Requires Task 2 landed.

- [ ] **Step 1: Scoped capture of the two chat interaction states, both browsers**

```bash
cd crates/vox-gui/ui
VOX_REVIEW_CAPTURE=1 npx playwright test e2e/review/capture.spec.ts \
  --project=chromium --project=firefox-review --workers=2 \
  --grep "rails-overlay-open|session-menu-open"
```
(Grep matches TEST TITLES — `` `${surface} -- ${state} -- ${viewport}` `` with
spaced separators and NO browser suffix. Cell ids like
`chat--rails-overlay-open--compact--firefox` are bundle ids, not titles — grepping
for them matches zero tests. This scoped run deletes the Task 9 bundle; Task 12
re-runs the full matrix.)

- [ ] **Step 2: Analyze, then read the two cells' defect lists**

Run the analysis (Task 3 Step 2 command), then:
`grep -B2 -A5 "rails-overlay-open--compact--firefox\|session-menu-open--compact--firefox" contracts/reports/gui-visual-review/bundle-digest.md`

- [ ] **Step 3a: If no clipping defect remains** — F-05 resolved (symptom of F-01, as
  the review predicted). Record in the Task 12 status note. No commit.

- [ ] **Step 3b: If clipping persists** — do NOT improvise a fix. Record the observed
  behavior (from the actual PNG) in the Task 12 status note as a separate open
  finding with its screenshot path, for its own follow-up triage.

---

### Task 12: whole-effort verification sweep + artifacts + push

**Files:** verification + artifact commits.
**Prereq:** `OPENROUTER_API_KEY`.

- [ ] **Step 1: Full test suites**

```bash
cd crates/vox-gui/ui && pnpm exec vitest run && npx tsc --noEmit
```
Expected: all pass, typecheck clean.

- [ ] **Step 2: Full Playwright sweep (chromium)**

Run: `cd crates/vox-gui/ui && npx playwright test --project=chromium`
Expected: same shape as the pre-remediation baseline (113 passed / 223 skipped — the
skips are the env-gated capture matrix; Tasks 2-10 added only vitest tests, no
`e2e/*.spec.ts`, so counts hold except `workbench-tabs.spec.ts`'s updated assertions
must pass).

- [ ] **Step 3: FULL-matrix harness run** (capture unscoped + analysis, both browsers
  — Task 9 Step 1's commands verbatim; prior scoped runs deleted earlier bundles).

- [ ] **Step 4: Confirm every finding's end state in the fresh digest**

- F-01/F-04: 0 occlusion/layout defects on the named cells.
- F-02: `grep -i "session_id\|is_error" <digest>` → no null-deref defects.
- F-03: `grep -i "TAURI_INTERNALS" <digest>` → nothing.
- F-05: per Task 11's outcome.
- F-06: no blank-viewport defects on `gamify`/`dashboard` cells.
- F-07/axe: per Task 9 Step 2's criteria (including the expected residuals:
  `page-has-heading-one` outside chat/dashboard; possibly `landmark-unique`
  outside the chat path).
(digest = `contracts/reports/gui-visual-review/bundle-digest.md`)

- [ ] **Step 5: Commit the analysis artifacts** (matching the harness effort's
  precedent — these are tracked files under `contracts/` and a re-run leaves them
  dirty otherwise):

```bash
git add contracts/reports/gui-visual-review/bundle-cache.v1.json \
  contracts/reports/gui-visual-review/bundle-digest.md \
  contracts/reports/gui-visual-review/bundle-report.v1.json
git commit -m "chore(visual-review): post-remediation full-matrix run artifacts"
```

- [ ] **Step 6: Regenerate contracts with a freshly-built release `vox`**

`cargo build --release -p vox-cli --locked` (if stale — NEVER touch
`~/.cargo/bin/vox.exe`), then:
```bash
./target/release/vox.exe ci gui-surface-coverage --write
./target/release/vox.exe ci test-inventory --output contracts/reports/test-inventory.v1.json
```
Commit any diff (new test files change the inventory).

- [ ] **Step 7: Append a "Remediation status" section to the source review**

In `docs/superpowers/reviews/2026-07-18-axis-frontend-comprehensive-review.md`:
per-finding fixed/open status, the F-05 triage outcome, the Task 7 Step 7
heading-debt list, and any landmark residuals from Task 9 Step 2. Commit with
Step 6's contract regen.

- [ ] **Step 8: Push to main**

Direct push (admin bypass, no PR — house convention for verified work). Long
timeout; pre-push hooks may run the audit gate. Confirm:
`git log origin/main -1 --oneline`.
