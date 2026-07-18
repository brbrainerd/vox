# Axis Frontend Review Harness & Comprehensive Review — Design

**Date:** 2026-07-18 (adversarially hardened same day: 8-reviewer audit, 61 findings applied)
**Status:** Approved (user directed: assume approval, proceed to plan)
**Author:** brainstorming session following the 3-phase Axis GUI remediation program

## Problem

Evaluating Axis in a plain browser (Firefox, `localhost:1420` dev server) surfaces two classes of failure the existing test/monitoring estate does not catch:

1. **Raw IPC TypeErrors in browser mode.** `transport.ts` calls Tauri's `invoke`/`listen` directly; without a Tauri host every call throws `can't access property "invoke", window.__TAURI_INTERNALS__ is undefined`. Graceful degradation depends on each call site individually catching — and beyond transport.ts, **33 production files import `invoke` directly** (tracked by the `ipcBoundaries.test.ts` allowlist, including `App.tsx` and `Sidebar.tsx` which run on every load) and **7 files import `listen` directly** (tracked by no guard at all). Several caught paths surface the raw TypeError text to the user via `String(err)` toasts (e.g. App.tsx's "Chat sessions" warn toast).
2. **Visual defects invisible to current capture.** Occlusion/overlap, clipped/truncated content, missing icons, and narrow-width layout breakage are not caught because the existing sweep (`e2e/screenshots.spec.ts`) captures one static screenshot per surface at one desktop viewport with sparse mock data, Chromium only, and the AI visual review (`visus_review`) scores those static PNGs against a general-quality rubric rather than hunting defects.

The user's requirement: these error classes must be **caught by the system, repeatably** — by properly analyzing screenshots — not merely found once by a human/LLM session. Additionally: a complete tab-by-tab, button-by-button review of the frontend, and an audit of what is tested/monitored versus not.

## Decisions (from brainstorming + adversarial audit)

- **Target environment:** Both. Tauri desktop shell is the primary product; bare-browser mode must degrade honestly (no raw TypeErrors reaching the user, honest empty states, one visible banner). Captures run in browser engines (WebView2 ≈ Chromium); a manual Tauri-shell spot-check covers engine-specific rendering.
- **Capture scope:** Full matrix — every registry surface (**31** with non-null viewKey) × 3 viewports × curated interaction states, realistic overflow-length mock data, axe-core scan, per-state console-error capture, Chromium + Firefox (the user evaluates in Firefox). Plus a bounded **theme sub-dimension** and consolidated **empty/error mock states** (subsuming `screenshots-variants.spec.ts`).
- **Analysis:** Automated, repeatable AI screenshot analysis extending `visus_review` with a defect-focused rubric — a permanent pipeline stage, cached, runnable locally (frontier-resumable) and as a bounded post-merge advisory CI step.
- **Flow:** Review first, then fix plan. Phase A/B/C build the fix + harness + analysis; Phase D produces the comprehensive findings document (with a known-issue recall check); remediation is a separate follow-on plan.
- **Known ground truth:** User evaluated in Firefox; pasted `__TAURI_INTERNALS__` TypeErrors; reports multiple occlusion issues beyond those named. Phase D must demonstrate the pipeline *recalls* these known-real issues.

## Non-goals

- No PR-gating of any new job (fork F2 precedent: post-merge advisory only).
- No tauri-driver/WebDriver native automation (immature on Tauri 2/Windows; non-deterministic live data defeats repeatable review).
- No programmatic occlusion detector (bounding-box heuristics are noise-prone; occlusion judgment belongs to the vision model + reviewer eyes).
- No remediation of visual findings in this effort (Phase D output feeds a separate remediation plan).
- Loading/skeleton states: out of scope (static mocks resolve instantly); revisit with the visual-diff baselines add-on.
- Below-the-fold capture (scroll-position states): deferred; captures are viewport-clipped with content height recorded per entry.
- Rewriting the 33 component-level direct-invoke files onto the transport hub: out of scope (existing `ipcBoundaries` shrink-only allowlist owns that debt); Phase A instead guarantees no raw TypeError *reaches the user* via the extended rejection filter, and raw-text leakage through caught `String(err)` paths becomes a Phase D finding class.

## Phase A — Honest browser degradation (transport guard)

**One choke point, honestly scoped.** `src/lib/backendGuard.ts` (new):

- `backendAvailable(): boolean` — env-agnostic detection: `'__TAURI_INTERNALS__' in (typeof window !== 'undefined' ? window : globalThis)`, memoized per app load (with a test-only reset).
- `class BackendUnavailableError extends Error` with `command: string` and message `Axis is running without its desktop backend — '<cmd>' unavailable`.
- A rejection filter for `window.unhandledrejection` that swallows (a) `BackendUnavailableError` and (b) — because 33 core-importing + 7 event-importing files still call raw Tauri APIs — any `TypeError` whose message matches `__TAURI_INTERNALS__` **when the backend is unavailable**, logging once per command/message at debug level. This is what actually makes "zero raw TypeErrors surface uncaught" true without rewriting 40 files.

`transport.ts`: `safeInvoke<T>`/`safeListen<T>` defined between `// __VOX_RAW_IPC_BEGIN__` / `// __VOX_RAW_IPC_END__` markers; **all** 60 `invoke` call sites and **all 11** module-scope `listen` wrapper functions route through them; a source-scan guard (`transportIpcGuard.test.ts`) forbids raw IPC outside the marked region.

**Test-suite survival (mandatory, first):** vitest runs in **node** env by default here (no global jsdom), and 40 existing test files mock `@tauri-apps/api/core` — `safeInvoke` checks availability *before* the mock is reached, so without a stub every one breaks. `src/test-setup.ts` (the configured `setupFiles`) gains `(globalThis as any).__TAURI_INTERNALS__ ??= {}` (paired with the env-agnostic detection above) **before** the transport rewrite lands. Suites that assert unavailable-mode behavior delete the stub + reset memoization in their own `beforeEach`.

**App shell honesty.** `BackendBanner` renders **in normal flow above the AppShell** (App.tsx wraps the shell in a flex column; AppShell root `h-screen` → `h-full`) — *not* a fixed overlay, which would occlude the sidebar header and TopHud (the exact defect class this project hunts). Dismissible; amber; `role="status"`. `main.tsx` installs the rejection filter and replaces its existing inline no-Tauri check with `backendAvailable()` (single source of truth).

**Contract:** existing per-caller `.catch` degradation keeps working unchanged; error *type* and *message* become honest; uncaught raw TypeErrors become impossible to surface; caught-path raw-text leakage (e.g. `String(err)` toasts) is enumerated in Phase D, not silently fixed here.

## Phase B — Review-bundle capture harness

**Location:** `crates/vox-gui/ui/e2e/review/`.

**State registry** (`e2e/review/states.ts`): `ReviewState { name, setup?, viewports?, mock? }` — `viewports?` restricts a state to the viewports where its UI exists (e.g. the chat rail overlay is compact-only); `mock?: 'rich' | 'empty' | 'error'` (default `rich`) selects the installer, **subsuming the empty/error variant sweep**: the 10 key surfaces from `screenshots-variants.spec.ts` get `empty` and `error` states here, and that spec + its advisory CI step are retired so all visual evidence flows through one bundle + one analyzer.

The **registry guard** requires an *explicit* entry per registry surface (`known.every(k => k in SURFACE_STATES)`) — adding a surface without deciding its states (even just `[DEFAULT]`) fails the guard, so the matrix cannot silently rot.

Curated states (verified selectors; initial set): global `focus-visible` (Tab ×4) and `no-backend` (no mock installed — asserts the banner, proves Phase A, and puts the banner itself under AI review); `chat`: model-picker-open, session-menu-open (viewport-tolerant via the rail toggle), composer-filled, rails-overlay-open (compact-only); `tasks`: composer-filled; `settings`: search-filtered, section-keybinds; `approvals`: row-focused (keyboard); `dashboard`: omnibar-open, sidebar-collapsed, achievements-open, hud-hidden. Native `<select>` popups (task priority) are uncapturable and deliberately omitted.

**Theme sub-dimension** (bounded): every surface's `default` state × wide viewport × chromium gains one capture per audited non-default theme (at minimum `high-contrast`), via `document.documentElement.dataset.theme` after navigation; `theme` recorded in the entry id.

**Rich mock** (`e2e/lib/tauriMockRich.ts`): **layered over `installTauriMock`** (which answers ~60 commands with representative shapes) rather than the sparse bootstrap — a wrapper captures the base `invoke` and overrides only the dense commands: 44 hopper tasks (120+-char titles, unicode/RTL), 14 chat sessions (honoring `limit`), 32 model cards (ids covering ModelsView's hosted/local split), 6 provider statuses, and a **dense msgpack `OrchestratorStatus`** (9 agents, 24 events, alerts, peers — encoded at compose time in Node via `@msgpack/msgpack`, already a dependency) so dashboard/flow/console render full content instead of blanks.

**Per capture, recorded into the bundle:**

- viewport-clipped PNG (the AI input; content height recorded — fullPage explodes on long rich lists and would be downscaled to unreadability by the reviewer's `max_image_edge_px`)
- sha256 (cache key), capture duration
- axe-core scan (`@axe-core/playwright`): violations impact ≥ moderate (whole-page; the analyzer dedupes and forwards only serious+critical to the model)
- console **errors** (warnings recorded separately, excluded from AI prompts) with the benign-favicon filter; pageerrors
- programmatic icon audit (zero-size SVGs, drawless SVGs, broken images) and horizontal-overflow flags (body + `surface-scroll-host`)

**Determinism:** `emulateMedia({ reducedMotion: 'reduce' })`, `document.fonts.ready` + settle wait on every state, `screenshot({ animations: 'disabled' })`.

**Manifest:** each capture appends one JSON line to `review-bundle/latest/entries-<browser>-w<workerIndex>.jsonl` (per-worker files remove append-interleaving questions entirely; the analyzer globs `entries-*.jsonl`). A `globalSetup` gated on `VOX_REVIEW_CAPTURE=1` clears stale entries first.

**Invocation:** `pnpm review:capture` / the Task-11 wrapper; env-gated so the default sweep/CI is unaffected; Firefox runs via a `firefox-review` Playwright project grep-scoped to `@review-capture`.

**Volume (honest math):** 31 surfaces × default + ~15 extra interaction states + 20 empty/error states + globals ≈ ~68 surface-states × 3 viewports × 2 browsers, minus viewport-constrained states, plus ~31 theme captures ≈ **~370–400 PNGs** per full local run.

## Phase C — Automated defect analysis (visus_review v2)

Extend `crates/vox-orchestrator-mcp/src/visus_review/` (feature `gui-visual-review`):

- **Bundle mode:** `gui-visual-review --bundle <dir>` loads `entries-*.jsonl` (`bundle.rs`; serde-defaulted optional fields). Legacy single-viewport mode keeps working until explicitly retired (below).
- **Shared core, not duplication:** extract from `run()` the fence-tolerant JSON extraction (`extract_json_object`, generalizing `parse_verdict` — model output arrives markdown-fenced, a bare `serde_json::from_str` fails in production), the per-image call (`review_image(png_path, model, system, user)` wrapping fs::read + timing + `call_vision_model`), and model selection (`select_review_model(cfg)`); `run_bundle` reuses all three.
- **Defect rubric:** `DEFECT_RUBRIC` + `defect_system_prompt()` + `defect_user_prompt(entry)` (embedding the entry's deduped serious/critical axe violations, console *errors*, icon issues, overflow) and a `PROMPT_VERSION` bump to `2026-07-18.1` (deliberately invalidates the legacy cache once). Output contract: `{ score, verdict, defects: [{severity, kind: occlusion|clipping|icon|error-leak|blank|layout|contrast|other, description, location}] }`.
- **Budget economics (the audit's critical finding):** `run()` is sequential and its config budgets (`total_review_budget_ms: 90_000`, `per_surface_review_budget_ms`, `max_concurrent_reviews`) are dead/underpowered for 150+ entries. `run_bundle` therefore: takes an explicit budget + `--max-reviews` cap; **priority-orders** the frontier (New/Changed first, then compact viewport, non-default states, chromium before firefox); records un-reviewed entries as `deferred` in the report; and is **frontier-resumable** — the cache makes each rerun pick up where the last stopped. The local wrapper loops until `deferred == 0`.
- **Browser filter:** `--browsers` (default `chromium` for AI review; Firefox entries always get programmatic-only analysis — their AI value is targeted comparison when programmatic findings diverge, plus on-demand `--browsers all`).
- **Cache:** dedicated `bundle-cache.v1.json` keyed on entry `id` — separate from the legacy cache **because each mode prunes keys absent from its own input set** (sharing one file means each `--ai` run wipes the other mode's entries). Pruning is **browser-scoped**: a cached firefox entry survives a chromium-only run.
- **Reports:** `bundle-report.v1.json` (per-entry verdicts + defects + programmatic summary + `deferred` status) and `bundle-digest.md` (grouped by surface, severity-ordered) under `contracts/reports/gui-visual-review/`.
- **CI:** the `gui-playwright-smoke` advisory steps switch to: bundle capture (chromium, workers=2) + bundle AI analysis (`cargo run -p vox-orchestrator-mcp … --bundle … --ai`, `OPENROUTER_API_KEY` env, bounded by `--max-reviews`), both `continue-on-error: true`. The cache-commit step commits **only** `bundle-cache.v1.json` (+ digest), gated on reviews > 0; reports upload as artifacts. **Legacy retirement:** the legacy manifest AI step is removed; `e2e/visual-review.spec.ts` + `screenshotManifest.ts` and the legacy `Manifest`/`run()` path are deleted (or deprecated with a removal date) once bundle mode is proven; `screenshots.spec.ts` is retained as the *asserting* gate while its PNGs are superseded by the bundle.
- **One-command local flow:** `scripts/frontend-review.vox` (VoxScript idioms verified against `gui-build.vox`/`ci-runners-up.vox`: `process.run` returns an Option unwrapped to `{code, stdout, stderr}`, `pnpm.cmd` Windows fallback, `std.env.set` for the env-gate) chains capture → analysis, looping analysis until the frontier is empty when `VOX_REVIEW_AI=1`.

## Phase D — The comprehensive review (deliverable)

With harness + analysis in place, produce `docs/superpowers/reviews/2026-07-18-axis-frontend-comprehensive-review.md`:

1. **Run the pipeline** (both browsers; AI looped until `deferred == 0`) and triage every automated finding — dedupe defects repeating across viewports/browsers, verify every critical/major against its PNG.
2. **Known-issue recall check (gate):** enumerate the user's known-real complaints (Firefox occlusion sites, the `__TAURI_INTERNALS__` leak); confirm each is recalled by the pipeline (defect in a firefox entry / structurally impossible post-Phase-A with zero matching page_errors across entries). Any known issue NOT recalled is a pipeline gap — extend states/rubric/mocks and re-run **before** writing the review. Record the recall table in the methodology section.
3. **Manual LLM pass** over the bundle — tab-by-tab, button-by-button; at minimum every compact-viewport and non-default-state capture in both browsers.
4. **Tauri-shell spot check:** ~6 representative surfaces vs their chromium captures.
5. **Coverage audit table (derived, not hand-assembled):** rows from `SURFACE_REGISTRY`/`gui-surface-coverage.v1.json`; columns populated by globbing `src/components/surfaces/**/​*.test.tsx`, `e2e/*.spec.ts`, `states.ts` keys, bundle-report presence, and `ci.yml` job wiring. Explicitly lists surfaces with no coverage in any column, plus excluded UI (doc-reader tabs, toasts) as recorded exclusions.
6. **Ranked findings register:** severity, kind, surface, states/viewports affected, evidence path, remediation sketch — the input to the follow-on remediation plan.

## Testing strategy summary

| Layer | Vehicle |
|---|---|
| backendGuard (detection/error/filter incl. raw-TypeError branch) | vitest unit |
| transport containment | source-scan guard (marker region; 71 raw sites rewritten) |
| test-suite survival | mandatory `test-setup.ts` stub, landed before the rewrite |
| banner (normal-flow) | vitest component tests + the `no-backend` capture state (automated visual regression) |
| state registry completeness | vitest guard (explicit entry per surface) |
| icon/overflow helpers | vitest unit (jsdom-corrected semantics) |
| rich mock serialization | `new Function(richMockInitScript(...))()` against a bare fake window (the proven variants-test idiom) + dataset shape/density tests |
| capture spec | env-gated smoke (slice, then full); explicitly a harness task — verified by run, not RED/GREEN |
| bundle loader / prompts / cache / run_bundle | Rust unit tests incl. behavioral no-AI run over a temp bundle, browser-scoped prune, fence-tolerant parsing, deferred-frontier |
| pipeline end-to-end | Phase D run + known-issue recall gate + post-merge advisory CI |

## Deferred / out of scope

- Visual-diff baselines (`toHaveScreenshot`): post-remediation add-on — the bundle becomes the baseline set.
- Programmatic occlusion detection; tauri-driver automation; PR gating; scroll-position states; loading/skeleton states.
- Migrating the 33 direct-invoke files onto the transport hub (tracked debt; ipcBoundaries allowlist is shrink-only).
- Extending `vox ci test-inventory` to scan vitest/Playwright files so future coverage tables are fully mechanical (worth a follow-up task).
- Remediation of Phase D findings (separate plan).
