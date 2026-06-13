# vox-mental-tracker Modernization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Sandbox note:** dispatched subagents are read-only in this worktree. Execute inline in the main session.

**Goal:** Bring `apps/vox-mental-tracker` from a broken mid-migration state to a working, modern (Clinical-Clean) web app with green CI (vitest + Playwright) and per-route screenshot artifacts.

**Architecture:** The app is authored in Vox (`src/main.vox`). `vox build src/main.vox -o dist` emits `dist/*.tsx` + `runtime-install.ts`; `vite build` bundles `dist/` + `index.html` → `web-dist/`. Feature/route changes are Vox edits; the visual theme is hand-editable `.mh-*` CSS in `index.html`. Tests: `vitest` (runtime shim, needs a prior `vox build`) + Playwright e2e (auto-builds via its `webServer`).

**Tech Stack:** Vox language + Vox compiler (`vox-cli`), React 19 (emitted), Vite 6, Playwright, vitest.

**Build commands (run from `apps/vox-mental-tracker/`):**
- `pnpm build:vox` → `cargo run --release -q -p vox-cli --manifest-path ../../Cargo.toml -- build src/main.vox -o dist` (populates `dist/`). For faster local iteration you MAY substitute the prebuilt debug binary: `../../target/debug/vox build src/main.vox -o dist`.
- `pnpm build:web` → `build:vox` + `build:fixup` + `vite build` (→ `web-dist/`).
- `pnpm exec vitest run` (unit) · `pnpm exec playwright test` (e2e).

---

## File Structure

- Modify: `apps/vox-mental-tracker/src/main.vox` — TimelinePage + SettingsPage (P2).
- Modify: `apps/vox-mental-tracker/index.html` — Clinical-Clean `.mh-*` theme (P3).
- Modify: `.github/workflows/vox-mental-tracker.yml` — drop stale tsc step, add `build:vox` before vitest, upload screenshots (P0/P4).
- Delete: `apps/vox-mental-tracker/capacitor.config.ts`, `apps/vox-mental-tracker/ios/` (P1).
- Modify: `apps/vox-mental-tracker/Vox.toml` — drop `capacitor` keyword (P1).
- Modify: `apps/vox-mental-tracker/.gitignore` — add `tests/e2e/__screens__/` (P4).
- Create: `apps/vox-mental-tracker/tests/e2e/routes.spec.ts` — per-route render + screenshot (P4).

---

## Phase P0 — Build/CI repair

### Task 1: Confirm the theme location + baseline build

**Files:** (read-only investigation + one workflow edit)

- [ ] **Step 1: Confirm where the theme lives**

Run: `grep -n "mh-root\|mh-nav\|--mh-" apps/vox-mental-tracker/index.html`
Expected: the `.mh-*` rules + `--mh-bg/--mh-fg` fallbacks are in `index.html`'s `<style>` (hand-editable). Confirms P3 edits `index.html`, not Vox codegen.

- [ ] **Step 2: Baseline build the app**

Run (from `apps/vox-mental-tracker/`): `pnpm build:vox`
Expected: `dist/` is created containing `runtime-install.ts`, `entry.tsx`, per-page `.tsx`, `vox-tokens.css`. (First run compiles `vox-cli` in release — may take several minutes.)

- [ ] **Step 3: Verify the unit test now passes**

Run: `pnpm exec vitest run`
Expected: `tests/runtime_shim.test.ts` PASS (2 tests) — `dist/runtime-install` now exists.

- [ ] **Step 4: Remove the stale CI step**

In `.github/workflows/vox-mental-tracker.yml`, delete the step that builds the deleted plugin (the `vitest` job):

```yaml
      - name: Build Sherpa plugin package (declarations)
        run: pnpm exec tsc -p plugins/vox-sherpa-transcribe/tsconfig.json
        working-directory: apps/vox-mental-tracker
```

- [ ] **Step 5: Add a `vox build` step before vitest**

In the same `vitest` job, after "Install JS deps" and before the vitest run step, insert:

```yaml
      - name: Build Vox app (populate dist/)
        run: pnpm build:vox
        working-directory: apps/vox-mental-tracker
```

(`runtime_shim.test.ts` imports `../dist/runtime-install`, which only exists after `vox build`.)

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/vox-mental-tracker.yml
git commit -m "fix(mental-tracker): vitest CI builds dist via vox build; drop deleted-plugin tsc step"
```

---

## Phase P1 — Cleanup (dead Capacitor)

### Task 2: Remove Capacitor remnants

**Files:**
- Delete: `apps/vox-mental-tracker/capacitor.config.ts`
- Delete: `apps/vox-mental-tracker/ios/`
- Modify: `apps/vox-mental-tracker/Vox.toml`

- [ ] **Step 1: Confirm nothing imports capacitor**

Run: `grep -rn "@capacitor\|capacitor.config\|CapacitorConfig" apps/vox-mental-tracker/src apps/vox-mental-tracker/tests apps/vox-mental-tracker/package.json`
Expected: no matches (only `capacitor.config.ts` itself + maybe `Vox.toml` keyword).

- [ ] **Step 2: Delete the dead config + native dir**

```bash
git rm -f apps/vox-mental-tracker/capacitor.config.ts
git rm -r --cached apps/vox-mental-tracker/ios 2>/dev/null || true
rm -rf apps/vox-mental-tracker/ios
```

(`/ios/` is already in `.gitignore`; `git rm --cached` drops any tracked remnant.)

- [ ] **Step 3: Drop the `capacitor` keyword from Vox.toml**

Run: `grep -n capacitor apps/vox-mental-tracker/Vox.toml`
Then remove the `"capacitor"` entry from the `keywords = [...]` array (leave the rest intact).

- [ ] **Step 4: Verify the app still builds**

Run (from `apps/vox-mental-tracker/`): `pnpm build:vox && pnpm exec vitest run`
Expected: build OK, vitest PASS.

- [ ] **Step 5: Commit**

```bash
git add -A apps/vox-mental-tracker
git commit -m "chore(mental-tracker): remove dead Capacitor config + ios shell"
```

---

## Phase P2 — Feature completion (Vox edits)

### Task 3: Real `/timeline` (effective-event list)

**Files:**
- Modify: `apps/vox-mental-tracker/src/main.vox`

- [ ] **Step 1: Add a human-readable timeline query**

In `main.vox`, after `timeline_events_json()` (ends ~line 336), add a query that returns a newline-joined list of effective events (one line each, kind + payload). Mirrors the proven `weekly_summary_json` → `text(mh-pre)` render pattern:

```vox
/// Human-readable list of effective (non-superseded) events, newest rows last,
/// one per line: "• <kind> — <payload_json>". Empty-state friendly.
@query fn timeline_lines() to str {
    match db.HealthEventLog.all() {
        Ok(rows) => {
            let mut out = ""
            let mut shown = 0
            for r in rows {
                if _is_effective(r, rows) {
                    shown = shown + 1
                    out = out + "• " + r.event_kind + " — " + r.payload_json + "\n"
                }
            }
            if shown is 0 { return "No events yet. Add one from Home or Voice." }
            out
        }
        Error(_) => "Could not read the event log."
    }
}
```

- [ ] **Step 2: Render the list in TimelinePage**

Replace the `TimelinePage` component (currently ~lines 501–512) with one that shows the real list plus the count:

```vox
component TimelinePage() {
    state total: int = 0
    state lines: str = ""
    on mount: {
        total = health_event_count()
        lines = timeline_lines()
    }
    view: column(raw_class="mh-root") {
        NavBar()
        heading(level=2) { "Timeline" }
        text(raw_class="mh-sub") { str(total) + " events recorded · superseded & deleted rows hidden" }
        text(raw_class="mh-list") { lines }
        link(href="/export") { "Export CSV / clinical HTML →" }
    }
}
```

- [ ] **Step 3: Rebuild + sanity-check the emitted output**

Run (from `apps/vox-mental-tracker/`): `pnpm build:vox`
Expected: builds with no Vox errors; `dist/TimelinePage.tsx` references `timeline_lines`.

- [ ] **Step 4: Vox-level test for the query**

Add a `@test` next to the other tests (after `test_csv_quote_inner_quotes`, ~line 212) asserting the empty-state string is stable:

```vox
@test
fn test_timeline_lines_formats_effective_only() to Unit {
    // Pure formatting check against the helper the query delegates to.
    let target = _mk_test_event("e1", "mood_recorded", "")
    let tomb = _mk_test_event("t1", "_deleted", "e1")
    let rows = [target, tomb]
    // The only effective row is... none (target superseded, tomb hidden).
    assert(not _is_effective(target, rows))
    assert(not _is_effective(tomb, rows))
}
```

Run: `../../target/debug/vox test src/main.vox` (or `cargo run -q -p vox-cli --manifest-path ../../Cargo.toml -- test src/main.vox`)
Expected: all `@test` fns PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/vox-mental-tracker/src/main.vox
git commit -m "feat(mental-tracker): real /timeline list of effective events"
```

### Task 4: Real, non-destructive `/settings`

**Files:**
- Modify: `apps/vox-mental-tracker/src/main.vox`

- [ ] **Step 1: Replace SettingsPage with a functional, non-destructive screen**

Replace `SettingsPage` (currently ~lines 739–746). It surfaces the timezone, the event count, an export shortcut, and the data-safety statement — **no clear/delete control** (data preservation):

```vox
component SettingsPage() {
    state total: int = 0
    on mount: {
        total = health_event_count()
    }
    view: column(raw_class="mh-root") {
        NavBar()
        heading(level=2) { "Settings" }

        text(raw_class="mh-section-label") { "Your data" }
        text(raw_class="mh-sub") { str(total) + " events stored locally on this device." }
        text(raw_class="mh-sub") { "Timezone: UTC (events are stamped with the device tz at record time)." }

        text(raw_class="mh-section-label") { "Export" }
        link(href="/export") { "Download CSV / clinical HTML →" }

        text(raw_class="mh-section-label") { "Data safety" }
        text(raw_class="mh-sub") { "This log is append-only. Nothing is ever overwritten or deleted — edits are recorded as corrections and removals as tombstones, so your clinical history stays intact. To start fresh, export first, then archive the device store outside the app." }
    }
}
```

- [ ] **Step 2: Rebuild**

Run (from `apps/vox-mental-tracker/`): `pnpm build:vox`
Expected: builds clean; `dist/SettingsPage.tsx` references `health_event_count`.

- [ ] **Step 3: Commit**

```bash
git add apps/vox-mental-tracker/src/main.vox
git commit -m "feat(mental-tracker): real non-destructive /settings (data summary + export + safety)"
```

---

## Phase P3 — Visual restyle (Clinical Clean)

### Task 5: Apply the Clinical-Clean theme

**Files:**
- Modify: `apps/vox-mental-tracker/index.html`

- [ ] **Step 1: Replace the `<style>` block with the Clinical-Clean theme**

Replace the entire `<style>…</style>` in `index.html` (and set the light `theme-color`) with:

```html
    <meta name="theme-color" content="#2563eb" />
    <link rel="manifest" href="/manifest.webmanifest" />
    <link rel="stylesheet" href="/dist/vox-tokens.css" />
    <style>
      :root {
        font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
        line-height: 1.5;
        --mh-bg: #f7f9fc;
        --mh-fg: #0f2747;
        --mh-primary: #2563eb;
        --mh-secondary: #5b6b80;
        --mh-border: #e2e8f0;
        --mh-surface: #ffffff;
      }
      body {
        margin: 0 auto; padding: 20px 16px; max-width: 720px;
        background: var(--mh-bg); color: var(--mh-fg);
      }
      .mh-root { display: flex; flex-direction: column; gap: 16px; }
      h1, h2 { color: var(--mh-fg); font-weight: 600; letter-spacing: -0.01em; margin: 4px 0; }
      h1 { font-size: 22px; } h2 { font-size: 18px; }
      .mh-actions { display: flex; flex-wrap: wrap; gap: 8px; }
      .mh-nav {
        display: flex; gap: 4px; flex-wrap: wrap; align-items: center;
        padding: 10px 12px; background: var(--mh-surface);
        border: 0.5px solid var(--mh-border); border-radius: 8px; font-size: 13px;
      }
      .mh-nav a { color: var(--mh-secondary); text-decoration: none; padding: 2px 6px; border-radius: 6px; }
      .mh-nav a:hover { color: var(--mh-primary); background: #eef2ff; }
      .mh-nav-dot { display: none; }
      .mh-sub { color: var(--mh-secondary); font-size: 13px; }
      .mh-section-label { color: var(--mh-fg); font-weight: 600; font-size: 13px; margin-top: 8px; }
      .mh-count { color: var(--mh-secondary); font-size: 13px; }
      .mh-pre, .mh-list {
        font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
        font-size: 12.5px; white-space: pre-wrap; word-break: break-word;
        background: var(--mh-surface); border: 0.5px solid var(--mh-border);
        border-radius: 8px; padding: 12px 14px; color: var(--mh-fg);
      }
      .mh-list { line-height: 1.8; }
      button {
        padding: 9px 14px; border: 0.5px solid var(--mh-border); border-radius: 6px;
        background: var(--mh-surface); color: var(--mh-fg); cursor: pointer; font-size: 13px;
      }
      button:hover { border-color: var(--mh-primary); color: var(--mh-primary); background: #f5f8ff; }
      .mh-actions button:first-child, form button[type="submit"] {
        background: var(--mh-primary); color: #fff; border-color: var(--mh-primary);
      }
      a { color: var(--mh-primary); }
      input, textarea {
        width: 100%; box-sizing: border-box; padding: 8px 10px;
        border: 0.5px solid var(--mh-border); border-radius: 6px; font: inherit; color: var(--mh-fg);
      }
      input:focus, textarea:focus { outline: none; border-color: var(--mh-primary); box-shadow: 0 0 0 3px #2563eb22; }
    </style>
```

- [ ] **Step 2: Rebuild + eyeball each route**

Run (from `apps/vox-mental-tracker/`): `pnpm build:web && pnpm exec vite preview --port 5173 --host 127.0.0.1 --strictPort`
Then open `http://127.0.0.1:5173/`, `/mood`, `/timeline`, `/weekly`, `/export`, `/voice`, `/settings` — confirm the clean white/blue look renders on all, then stop the preview (Ctrl-C).

- [ ] **Step 3: Commit**

```bash
git add apps/vox-mental-tracker/index.html
git commit -m "feat(mental-tracker): Clinical-Clean theme across all routes"
```

---

## Phase P4 — Playwright per-route confirmation

### Task 6: Per-route render + screenshot specs

**Files:**
- Modify: `apps/vox-mental-tracker/.gitignore`
- Create: `apps/vox-mental-tracker/tests/e2e/routes.spec.ts`
- Modify: `.github/workflows/vox-mental-tracker.yml`

- [ ] **Step 1: Gitignore the screenshot dir**

Append to `apps/vox-mental-tracker/.gitignore`:

```
/tests/e2e/__screens__/
```

- [ ] **Step 2: Write the per-route spec (render assertion + screenshot)**

Create `apps/vox-mental-tracker/tests/e2e/routes.spec.ts`:

```ts
import { test, expect } from "@playwright/test";

const ROUTES: { path: string; heading: RegExp }[] = [
  { path: "/", heading: /Mental Health Tracker/i },
  { path: "/mood", heading: /Log your mood/i },
  { path: "/timeline", heading: /Timeline/i },
  { path: "/weekly", heading: /Weekly summary/i },
  { path: "/export", heading: /Exports/i },
  { path: "/voice", heading: /Voice/i },
  { path: "/settings", heading: /Settings/i },
];

for (const { path, heading } of ROUTES) {
  test(`route ${path} renders + screenshot`, async ({ page }) => {
    await page.goto(path);
    // The Vox NavBar is on every route — proves the app shell mounted.
    await expect(page.getByRole("link", { name: "Home" })).toBeVisible();
    // Route-specific heading.
    await expect(page.getByRole("heading", { name: heading })).toBeVisible();
    const slug = path === "/" ? "home" : path.replace(/\//g, "");
    await page.screenshot({ path: `tests/e2e/__screens__/${slug}.png`, fullPage: true });
  });
}

test("settings has no destructive clear/delete control", async ({ page }) => {
  await page.goto("/settings");
  await expect(page.getByRole("button", { name: /clear|delete|reset|wipe/i })).toHaveCount(0);
});
```

- [ ] **Step 3: Run the e2e suite locally**

Run (from `apps/vox-mental-tracker/`): `pnpm exec playwright test routes.spec.ts`
Expected: 8 tests PASS (7 routes + the no-destructive-control check); `tests/e2e/__screens__/*.png` produced. (Playwright's `webServer` auto-runs `build:web` + `vite preview`.)

- [ ] **Step 4: Run the full e2e suite (existing specs still pass)**

Run: `pnpm exec playwright test`
Expected: `smoke`, `mood_form`, `voice_flow`, and `routes` all PASS.

- [ ] **Step 5: Upload screenshots as a CI artifact**

In `.github/workflows/vox-mental-tracker.yml`, in the `playwright` job after the `playwright test` step, add:

```yaml
      - name: Upload route screenshots
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: vox-mental-tracker-screens
          path: apps/vox-mental-tracker/tests/e2e/__screens__/
          if-no-files-found: ignore
```

- [ ] **Step 6: Commit**

```bash
git add apps/vox-mental-tracker/.gitignore apps/vox-mental-tracker/tests/e2e/routes.spec.ts .github/workflows/vox-mental-tracker.yml
git commit -m "test(mental-tracker): per-route Playwright render+screenshot, upload as CI artifact"
```

---

## Verification summary (run before finishing the branch)

```bash
cd apps/vox-mental-tracker
pnpm build:web                 # vox build + vite build, no errors
pnpm exec vitest run           # runtime shim green
pnpm exec playwright test      # smoke + mood_form + voice_flow + routes green
```
All green + `tests/e2e/__screens__/` populated (gitignored) before invoking superpowers:finishing-a-development-branch. Then the `vox-mental-tracker.yml` workflow (vox-check, vitest, playwright, contracts, app-summary) should be green on CI.

## Out of scope
Native `src-tauri` shell, live on-device STT verification, visual-regression pixel baselines, Android/iOS packaging, Vox compiler/codegen changes.
