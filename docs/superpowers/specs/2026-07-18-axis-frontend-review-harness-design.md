# Axis Frontend Review Harness & Comprehensive Review — Design

**Date:** 2026-07-18
**Status:** Approved (user directed: assume approval, proceed to plan)
**Author:** brainstorming session following the 3-phase Axis GUI remediation program

## Problem

Evaluating Axis in a plain browser (Firefox, `localhost:1420` dev server) surfaces two classes of failure the existing test/monitoring estate does not catch:

1. **Raw IPC TypeErrors in browser mode.** `transport.ts` calls Tauri's `invoke`/`listen` directly; without a Tauri host every call throws `can't access property "invoke", window.__TAURI_INTERNALS__ is undefined`. Graceful degradation depends on each of ~60 call sites individually catching — several don't, so raw exception text reaches the user.
2. **Visual defects invisible to current capture.** Occlusion/overlap, clipped/truncated content, missing icons, and narrow-width layout breakage are not caught because the existing sweep (`e2e/screenshots.spec.ts`) captures one static screenshot per surface at one desktop viewport with sparse mock data, Chromium only, and the AI visual review (`visus_review`) scores those static PNGs against a general-quality rubric rather than hunting defects.

The user's requirement: these error classes must be **caught by the system, repeatably** — by properly analyzing screenshots — not merely found once by a human/LLM session. Additionally: a complete tab-by-tab, button-by-button review of the frontend, and an audit of what is tested/monitored versus not.

## Decisions (from brainstorming)

- **Target environment:** Both. Tauri desktop shell is the primary product; bare-browser mode must degrade honestly (no raw TypeErrors, honest empty states, one visible banner). Captures run in browser engines (WebView2 ≈ Chromium); a manual Tauri-shell spot-check covers engine-specific rendering.
- **Capture scope:** Full matrix — every registry surface × 3 viewports × curated interaction states, realistic overflow-length mock data, axe-core accessibility scan, per-state console-error capture, Chromium + Firefox (the user evaluates in Firefox).
- **Analysis:** Automated, repeatable AI screenshot analysis extending `visus_review` with a defect-focused rubric — a permanent pipeline stage, cached, runnable locally and as a post-merge advisory CI step.
- **Flow:** Review first, then fix plan. Phase A/B/C build the fix + harness + analysis; Phase D produces the comprehensive findings document; remediation is a separate follow-on plan re-prioritized from the full picture.
- **Known ground truth:** User evaluated in Firefox; pasted `__TAURI_INTERNALS__` TypeErrors; reports multiple occlusion issues beyond those named.

## Non-goals

- No PR-gating of any new job (fork F2 precedent stands: post-merge advisory only).
- No tauri-driver/WebDriver native automation (immature on Tauri 2/Windows; non-deterministic live data defeats repeatable review).
- No programmatic occlusion detector (bounding-box overlap heuristics are noise-prone; occlusion judgment belongs to the vision model + reviewer eyes on screenshots).
- No remediation of visual findings in this effort (Phase D output feeds a separate remediation plan).

## Phase A — Honest browser degradation (transport guard)

**One choke point.** `transport.ts` gains:

- `backendAvailable(): boolean` — `typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window`, evaluated once (module-scope lazy).
- `class BackendUnavailableError extends Error` with `command: string` and message `Axis is running without its desktop backend — '<cmd>' unavailable`.
- A private `safeInvoke<T>(cmd, args?)` used by **every** `VoxTransport` method (~60 `invoke<T>(...)` call sites mechanically rewritten): if the backend is unavailable, reject with `BackendUnavailableError(cmd)` without touching Tauri APIs; otherwise delegate to `invoke`.
- The same guard on event registration: `listenOrchStatus`, `listenAgentEvents`, and any other `listen` wrapper reject with `BackendUnavailableError('listen:<event>')` when no host is present.

**App shell honesty.** `App.tsx`: when `backendAvailable()` is false, render a persistent, dismissible banner ("Browser preview — no desktop backend connected; surfaces show empty states"). A `window.addEventListener('unhandledrejection', …)` handler swallows `BackendUnavailableError` rejections (log once per command at debug level) so no uncaught path can spam the console or an error overlay.

**Contract:** existing per-caller `.catch` degradation keeps working unchanged — the error *type* and *message* become honest, and uncaught paths become impossible to surface raw.

**Tests:** vitest — `safeInvoke` rejects with `BackendUnavailableError` when `__TAURI_INTERNALS__` is absent and delegates when present; banner renders iff backend unavailable; unhandled-rejection handler filters only this error type. A guard test asserts `transport.ts` contains no direct `invoke<`/`invoke(` call outside `safeInvoke` (source-scan idiom, like `ipcBoundaries.test.ts`).

## Phase B — Review-bundle capture harness

**Location:** `crates/vox-gui/ui/e2e/review/`.

**State registry** (`e2e/review/states.ts`): per-surface curated interaction states. Every `SURFACE_REGISTRY` surface MUST have at least `['default']`; surfaces with interactive chrome add named states, each a `(page) => Promise<void>` setup:

- global: `sidebar-collapsed`, `omnibar-open` (captured once per viewport, not per surface)
- `chat`: `model-picker-open`, `session-menu-open`, `composer-filled`
- `tasks`: `composer-filled`, `priority-select-open`
- `approvals`: `row-focused`
- `settings`: `section-keybinds`, `search-filtered`
- (initial list; extended per-surface as the registry grows)

**Registry guard** (`src/guards/reviewStates.guard.test.ts`): fails when a registry surface lacks a state entry — same pattern as `surfaceRegistryEscape.test.ts`, so the matrix cannot silently rot as surfaces are added.

**Rich mock** (`e2e/lib/tauriMockRich.ts`): layered over `tauriMockShared`/`tauriMock` — overflow-length titles (120+ chars), 40+ hopper tasks, 12+ chat sessions, 30+ model cards, long agent/provider names, unicode/RTL samples, nonzero costs. Sparse mocks are why occlusion has been invisible; realistic density is the point.

**Matrix:** every surface × viewports `[1440×900 ("wide"), 1100×720 ("laptop"), 900×600 ("compact")]` × its states × browsers `[chromium, firefox]` (Firefox project added to `playwright.config.ts`, used only by the review capture spec via `grep`/project filtering; the asserting CI sweep stays chromium-only).

**Per capture, recorded into the bundle:**

- full-page PNG: `review-bundle/latest/<surface>--<state>--<viewport>--<browser>.png`
- sha256 of the PNG (cache key for analysis)
- axe-core scan (`@axe-core/playwright`, new devDependency): violations with impact ≥ `moderate`
- console messages (error+warning) and pageerrors collected during the state
- programmatic icon audit: SVG elements with zero rendered size or no drawable children; `img` with `naturalWidth === 0`
- overflow flags: `document.body` horizontal scroll (`scrollWidth > clientWidth`), plus any `[data-testid="surface-scroll-host"]` horizontal overflow
- capture duration

**Manifest:** each capture appends one JSON object per line to `review-bundle/latest/entries-<browser>.jsonl` — `{ id: "<surface>--<state>--<viewport>--<browser>", surface, state, viewport, browser, file, sha256, state_ok, axe_violations: [...], console_errors: [...], page_errors: [...], icon_issues: [...], overflow: {...}, captured_at }`. JSONL append is parallel-worker-safe (no shared-manifest write races across Playwright workers); the Rust analyzer reads `entries-*.jsonl` directly.

**Invocation:** `pnpm review:capture` (package.json script setting `VOX_REVIEW_CAPTURE=1` and running `playwright test e2e/review/capture.spec.ts` across both browser projects). The spec self-skips without the env var so the default sweep/CI is unaffected. Estimated volume: ~25 surfaces × ~1.5 avg states × 3 viewports × 2 browsers ≈ 200–250 captures.

**Harness tests:** unit tests (vitest) for the icon-audit and overflow-detector page-function builders; the registry guard above; capture spec asserts only "no crash + all entries written" (capture is advisory evidence, not a gate).

## Phase C — Automated defect analysis (visus_review v2)

Extend `crates/vox-orchestrator-mcp/src/visus_review/` (feature `gui-visual-review`):

- **Bundle mode:** `gui-visual-review --bundle crates/vox-gui/ui/review-bundle/latest` reads the bundle manifest (new `BundleManifest`/`BundleEntry` types alongside the legacy single-viewport `Manifest`, which keeps working until the CI step is switched).
- **Defect rubric:** new `DEFECT_RUBRIC` + `defect_user_prompt(entry)` and a `PROMPT_VERSION` bump. The model is instructed to hunt defects, not grade aesthetics: occlusion/overlap of elements, clipped or truncated text/controls, missing or blank icon slots, raw error/exception text visible in UI copy, blank panels where content is expected, broken layout at the compact viewport, insufficient contrast, overlapping z-layers (menus/toasts/HUD). The prompt includes the entry's programmatic findings (axe violations, console errors, icon issues, overflow flags) so the model correlates rather than rediscovers.
- **Output contract:** per-entry JSON `{ defects: [ { severity: "critical"|"major"|"minor", kind: "occlusion"|"clipping"|"icon"|"error-leak"|"blank"|"layout"|"contrast"|"other", description, location } ], score: 0-100, verdict }`. Report written to `contracts/reports/gui-visual-review/bundle-<date>.json` + a markdown digest grouped by surface and severity.
- **Cache:** the Phase-3 cache (keyed on screenshot sha256 + model + prompt version, schema v1, dead-key pruning) is reused with entry key `id` — unchanged screenshots cost nothing on re-runs; the rubric bump forces exactly one full re-review.
- **CI:** the existing post-merge advisory AI-review step in `gui-playwright-smoke` switches from the legacy single-viewport manifest to the bundle (chromium-only in CI to bound cost; Firefox captures are local/on-demand). `continue-on-error: true` stays — advisory per F2. The report + bundle manifest upload as artifacts.
- **One-command local flow:** `scripts/frontend-review.vox` (VoxScript-only glue per repo policy) runs `pnpm review:capture` then the bundle analysis and prints the digest path.

**Tests:** Rust unit tests — bundle manifest deserialization (including legacy-tolerant serde defaults), defect-output parsing, cache decisions on the new entry keys, prompt-version regression (rubric mentions occlusion/icons/error-leak).

## Phase D — The comprehensive review (deliverable)

With harness + analysis in place, produce `docs/superpowers/reviews/2026-07-18-axis-frontend-comprehensive-review.md`:

1. **Run the pipeline** (both browsers locally) and triage every automated finding (AI defects + axe + icon + overflow + console) — dedupe, verify against screenshots, rank by severity.
2. **Manual LLM pass over the bundle**: tab-by-tab, button-by-button review of every surface's screenshots across states/viewports — catching what the model missed, with explicit per-surface sections.
3. **Tauri-shell spot check:** launch the real desktop shell and screenshot ~6 representative surfaces to catch WebView-specific rendering gaps versus the browser captures.
4. **Coverage audit table:** per surface — unit tests? e2e spec? screenshot capture? states covered? AI-analyzed? monitored in CI? — cross-referenced from the test-inventory contract, `e2e/` specs, and `.github/workflows/ci.yml`. Names what is tested/monitored and what isn't.
5. **Ranked findings register:** every defect with severity, surface, state/viewport, evidence path, and a remediation sketch — the direct input to the follow-on remediation plan (separate plan, re-prioritized by the user from the full picture).

## Testing strategy summary

| Layer | Vehicle |
|---|---|
| transport guard | vitest unit + source-scan guard (no raw `invoke` outside `safeInvoke`) |
| banner/unhandled-rejection | vitest component tests |
| state registry completeness | vitest guard (registry-escape idiom) |
| icon/overflow helpers | vitest unit |
| capture spec | self-verifying run (all manifest entries written), env-gated |
| bundle manifest / rubric / cache | Rust unit tests (`cargo test -p vox-orchestrator-mcp --features gui-visual-review`) |
| pipeline end-to-end | Phase D run itself + post-merge advisory CI step |

## Deferred / out of scope

- Visual-diff baselines (`toHaveScreenshot`): natural add-on after Phase-D fixes land — the bundle becomes the baseline set. Not this effort.
- Programmatic occlusion detection; tauri-driver automation; PR gating.
- Remediation of visual findings (separate plan from Phase D's register).
