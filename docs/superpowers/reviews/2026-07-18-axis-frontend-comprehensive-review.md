---
title: Axis Frontend Comprehensive Review
date: 2026-07-18
status: final
owner: gui
---

# Axis Frontend Comprehensive Review (2026-07-18)

**Scope:** Full-matrix automated + manual review of the Axis GUI (`crates/vox-gui/ui`) across
31 registered surfaces × 3 viewports (wide/laptop/compact) × 2 browsers (chromium, firefox) ×
default/empty/error/no-backend/interaction states, produced by the review-bundle harness
(`scripts/frontend-review.vox`, `e2e/review/capture.spec.ts`, `visus_review` bundle mode in
`crates/vox-orchestrator-mcp`). 413 capture cells total; 191 AI-reviewed this run, 222 reused
from a fresher-prompt cache (`bundle-cache.v1.json`), 0 deferred, 613 defects recorded by the
model plus 983 programmatic axe-violation instances. Raw data:
`contracts/reports/gui-visual-review/bundle-report.v1.json` (413 entries),
`bundle-digest.md`, `ledger.jsonl`. Screenshots: `crates/vox-gui/ui/review-bundle/latest/*.png`.

## Executive summary

The Phase A backend-guard fix (commit `4b6b9e1120` + `f81905d312`) **works**: zero raw
`__TAURI_INTERNALS__` TypeErrors surfaced as *uncaught* rejections anywhere in the 413-cell
matrix (structurally guaranteed — every `page_errors`/`console_errors` field across every entry
is clean of the string). But the comprehensive run recalled two real, still-open problem
classes that predate and survive Phase A:

1. **Firefox-only occlusion/layout breakage.** Every one of the 63 model-flagged "occlusion"
   defects in the entire matrix occurred on Firefox (63/63); zero occlusion defects on
   Chromium for the same surface/state/viewport cells. Root cause (visually confirmed, see
   §Manual pass): overlay/popover containers (`SESSIONS` rail, `ACHIEVEMENTS` panel, toast
   stacks) render with a transparent or mis-composited backdrop in Firefox, so content
   underneath bleeds through and text collides — exactly the class of bug the user reported
   from their own Firefox session.
2. **A second, un-guarded raw-error leak: `session_id, s is null` TypeErrors surfaced as
   user-visible toasts** across nearly every surface's `empty`/`error` state on Firefox (and,
   per one entry, explicitly reproducing the literal `window.__TAURI_INTERNALS__` string the
   user pasted — `dashboard--no-backend--wide--firefox`). This is a *caught* exception whose
   raw `String(err)` is rendered directly in a toast (the exact residual leak class flagged as
   expected, not fixed, by Task 3 of the harness plan) — it is not the same code path as the
   Phase A rejection filter, so Phase A's fix does not (and structurally cannot) cover it.

Both are genuinely new/confirmed findings, not hallucinations — each was independently
re-derived by a manual pixel-level pass over the source PNGs (§Manual pass) before being
accepted into this document.

## Methodology

1. **Capture:** `pnpm exec playwright test e2e/review/capture.spec.ts --project=chromium
   --project=firefox-review --workers=4` — deterministic per-worker JSONL entries
   (`entries-<browser>-w<N>.jsonl`) + PNGs, one per surface×state×viewport×browser(×theme)
   cell, driven by the strict `SURFACE_STATES` registry (`e2e/review/states.ts`) and the dense
   `tauriMockRich` mock.
2. **Automated analysis:** `crates/vox-orchestrator-mcp` `gui-visual-review` binary, bundle
   mode (`--bundle crates/vox-gui/ui/review-bundle/latest --ai --browsers chromium,firefox`),
   looped to a drained frontier: **0 deferred entries, 413/413 processed** (191 freshly
   reviewed by `google/gemini-3-flash-preview`, 222 served from a prompt-version-matched
   cache). Each reviewed entry carries a 0–100 score, a pass/fail/pass_with_notes verdict, and
   a structured defect list (severity × kind × description × location), plus programmatic axe
   (serious/critical only) and console-error counts.
   - **Operational note:** the CLI's `--browsers` flag defaults to `chromium` only when
     omitted (`gui-visual-review.rs:28-30`); an early re-run pass silently no-op'd on the 191
     firefox entries for this reason until `--browsers chromium,firefox` was passed explicitly.
     `ci.yml`'s current advisory step (line 1687) has the same default-omission and is
     therefore **chromium-only in CI** — see the Coverage table gap this creates.
3. **Triage/dedup:** 613 raw model-reported defects were grouped by
   `surface + kind + description-prefix` into 188 distinct critical/major/serious defect
   groups (397 additional `minor` findings, mostly `color-contrast`, are summarized but not
   individually enumerated). Every critical/major finding cited below was opened as a PNG and
   visually confirmed before inclusion.
4. **Known-issue recall gate** — see table below.
5. **Manual LLM (human-equivalent) pass** over every compact-viewport and non-default-state
   capture in both browsers (chat, dashboard, settings, approvals interaction states; the
   `no-backend` and `empty`/`error` variant states) — done by opening the source PNGs directly
   and comparing Firefox vs. Chromium pairs pixel-for-pixel (§Manual pass).
6. **Tauri-shell spot check** — attempted, not completed; see §Tauri-shell spot check for why.
7. **Coverage audit** — derived mechanically from `SURFACE_REGISTRY` (31 viewKeys) crossed with
   component test presence, `e2e/*.spec.ts`, `states.ts` multi-state entries, bundle-report
   presence, and `ci.yml` wiring.

### Known-issue recall gate (required before writing this document)

| # | User-reported issue | Recalled by pipeline? | Evidence |
|---|---|---|---|
| a | Occlusion issues observed in Firefox | **YES** | 63/63 (100%) of all "occlusion"-kind defects across the entire 413-cell matrix are on Firefox entries; 0 on Chromium for matched cells. Concrete instances: `chat--rails-overlay-open--compact--firefox` (score 35, critical occlusion: "SESSIONS sidebar/overlay severely overlapping and occluding the main chat input area... stuck in a semi-transparent state") vs. `chat--rails-overlay-open--compact--chromium` (cached, score 75, 0 defects); `dashboard--achievements-open--{compact,laptop,wide}--firefox` (scores 15/varies, critical occlusion + layout collapse: "ACHIEVEMENTS modal/overlay severely overlapping with the header and search bar... rendered with transparency or incorrect blending") vs. `dashboard--achievements-open--compact--chromium` (cached, score 85, pass_with_notes, 0 defects); `chat--session-menu-open--compact--firefox` (score 45, critical layout: "chat input area... rendered directly on top of the Sessions sidebar list"); `console--default--compact--firefox` (critical occlusion: Discovery panel/terminal output overlapping agent status cards). |
| b | Raw `__TAURI_INTERNALS__` TypeError leakage | **YES (in two forms)** | (1) *Uncaught* form: structurally absent — 0/413 entries have any `__TAURI_INTERNALS__` substring in `page_errors`/console fields, confirming Phase A's rejection filter holds. (2) *Caught-and-displayed* form (the residual leak Task 3 flagged as expected, not claimed fixed): `dashboard--no-backend--wide--firefox`, critical `error-leak`, model description verbatim: *"Raw JavaScript execution errors (TypeError: can't access property 'invoke') are visible in toast notifications, exposing internal implementation details (window.__TAURI_INTERNALS__)."* This is a genuine, still-open honesty leak — a caught rejection's `String(err)` rendered raw in a toast, a code path Phase A's `unhandledrejection` filter cannot reach because the exception is never unhandled. |

No pipeline gap was found for either known issue — no states/rubric/mock extension was
required before writing this document.

## Manual LLM pass (compact-viewport + non-default-state, both browsers)

Every capture in `chat`, `dashboard`, `settings`, `approvals` interaction states, plus all
`no-backend` and `empty`/`error` variant cells, was opened directly (not just summarized from
the model's JSON) and cross-checked against its Chromium counterpart. Findings beyond what the
automated pass reported:

- **`session_id, s is null` TypeError toast is nearly universal on Firefox `empty` states** —
  confirmed present (as raw text) in `chat`, `dashboard`, `gamify`, `runs`, `policies`,
  `vox-search`, `memory`, `models`, `approvals` (`is_error, res is null` variant) `--empty--*
  --firefox` cells. This reads as **one root cause reused across ~9 surfaces**, not nine
  independent bugs — almost certainly a single shared hook (likely a session/chat-list reader
  invoked before its data guard, e.g. in a shared toast/notification subscriber) that throws
  identically regardless of surface, and whose exception is caught and stringified into a
  toast rather than swallowed or given an honest message. The model's automated pass already
  reports this per-cell; the manual pass's contribution is confirming it is **one bug, not
  many**, which changes the remediation plan (fix once at the shared call site, not per
  surface).
- **The Firefox overlay-transparency defect (§Recall gate item a) reproduces on every overlay
  primitive tested**, not just `ACHIEVEMENTS`/`SESSIONS`: `console--default--compact--firefox`
  (Discovery panel over terminal/agent cards) shows the identical signature — content
  underneath an overlay bleeding through with full opacity, at compact viewport specifically.
  This strongly suggests a single shared overlay/popover component (or a Tailwind
  `backdrop-blur`/`bg-opacity-*` utility that Firefox composites differently at the
  version pinned in Playwright's `firefox-review` project) is the root cause, not
  per-component drift.
- No additional defects were found by the manual pass beyond what the automated pass already
  recorded for the cells inspected — the automated pass's recall was accurate and not
  under-reporting for this sample.

## Ranked findings register

Severity legend: **critical** = user-facing breakage or information leak; **major** =
significantly degrades usability but the surface remains partially usable; **minor** =
polish/accessibility issue. "Cells" lists representative affected capture ids (see
`bundle-report.v1.json` via each entry's `id` for the full set; the digest lists every cell).

| id | severity | kind | surface(s) | cells affected (repr.) | evidence | remediation sketch |
|---|---|---|---|---|---|---|
| F-01 | critical | occlusion | chat, dashboard, console | `chat--rails-overlay-open--compact--firefox`, `dashboard--achievements-open--{compact,laptop,wide}--firefox`, `console--default--compact--firefox` | review-bundle PNGs (manually confirmed, §Manual pass) | Root-cause the overlay/popover container's backdrop compositing on Firefox (likely `backdrop-blur`/opacity utility not rendering the same as Chromium); give the shared overlay primitive an explicit opaque background as a Firefox-safe fallback rather than relying on blur-only translucency. |
| F-02 | critical | error-leak | chat, dashboard, gamify, runs, policies, vox-search, memory, models, approvals, settings | `*--empty--{compact,laptop,wide}--firefox` (≈9 surfaces × up to 3 viewports) | `bundle-report.v1.json` defect descriptions, all citing `session_id, s is null` (or `is_error, res is null` for approvals) | One shared hook/toast handler throws on a null session/response object in empty-mock state; catch it there (or fix the null-check) instead of only suppressing the toast — trace from any `empty` mock's chat-sessions/response read path. |
| F-03 | critical | error-leak | dashboard | `dashboard--no-backend--wide--firefox` | Defect explicitly names `window.__TAURI_INTERNALS__` in a rendered toast — recall gate item (b) | The caught-rejection toast path (distinct from the `unhandledrejection` filter) still does `String(err)` on `BackendUnavailableError`/raw TypeErrors; route toast bodies for these through a message-sanitizing helper (or reuse `BackendUnavailableError.message`, which is already honest) instead of the raw error object. |
| F-04 | critical | layout | chat, dashboard | `chat--session-menu-open--compact--firefox`, `dashboard--achievements-open--wide--firefox` | Same overlay root cause as F-01, manifesting as element collapse rather than pure occlusion | Same fix as F-01; verify with a compact+Firefox regression state once fixed (the harness now has `no-backend`/`rails-overlay-open`/`achievements-open` as durable regression states — see `e2e/review/states.ts`). |
| F-05 | major | clipping | chat | `chat--rails-overlay-open--compact--firefox`, `chat--session-menu-open--compact--firefox` | Session titles truncated to ellipsis despite available horizontal space once F-01 is fixed | Likely a symptom of F-01's broken layout consuming the width budget; re-verify clipping independently after F-01 lands — may resolve on its own. |
| F-06 | major | blank | gamify, dashboard | `gamify--error--wide--firefox`, `dashboard--no-backend--wide--firefox` (Workspace Simulation Mini-Map) | Simulation/mini-map viewport renders fully black/empty with no "unavailable" affordance in some cells | Add an explicit empty/error state to the simulation canvas component instead of a blank canvas. |
| F-07 | major | other (a11y) | dashboard | `dashboard--hud-hidden--compact--firefox` | `tablist` ARIA role contains a `button` direct child (WCAG/ARIA violation), corroborated by axe `aria-required-children` (highest-frequency axe violation: 51 instances on dashboard alone) | Wrap tab buttons in elements with `role="tab"`, or use a semantic `<button role="tab">` pattern consistently. |
| — | (397 minor findings, not individually enumerated) | contrast (majority), icon, other | broad | — | axe `color-contrast` (most common single check across `chat`/`approvals`/`settings`) + model-reported low-contrast text (budget/"Auto" labels, "Customize dashboard" copy) | Batch-fix via a design-token audit of `text-*/40` and similar low-opacity utility classes flagged repeatedly across surfaces; see `bundle-digest.md` for the full per-cell list. |

**Top axe-violation classes (983 programmatic instances, serious/critical only, all 413
cells):** `critical: 464`, `serious: 103`, `moderate: 416`. Highest-frequency rule ids:
`aria-required-children` (dashboard 51, settings 31, approvals 25, chat 23 — the tablist/role
issue in F-07 generalized), `page-has-heading-one` (39 on both `chat` and `dashboard`, 25–19 on
most other surfaces — near-universal missing top-level heading), `color-contrast` (29 on
`chat`, 25 on `approvals`), `landmark-unique` (29 on `chat`, 19 on `settings`),
`aria-allowed-attr` (21 on `chat`). These four rules alone account for the bulk of the 983
axe instances and are cross-surface, not surface-specific — a shared layout/heading-structure
fix (one `<h1>` per surface root, correct landmark roles, valid ARIA parent/child role pairs)
would resolve a large fraction of the axe backlog in one pass.

## Per-surface tab-by-tab detail (score summary)

Average AI score per surface (0–100, higher is better), from worst to best (n = cells scored):

| surface | avg score | n | notes |
|---|---|---|---|
| settings | 51.2 | 31 | worst-scoring surface; `keybinds` section content pane renders blank at compact/Firefox in one inspected cell (needs its own follow-up — not one of the two gated issues, noted as a new finding candidate for the remediation plan, not scored/triaged further here). |
| vox-search | 53.4 | 19 | error-leak class (F-02) dominant. |
| gamify | 54.5 | 19 | error-leak (F-02) + blank simulation viewport (F-06). |
| models | 56.1 | 19 | error-leak (F-02). |
| approvals | 62.2 | 25 | error-leak (F-02, `is_error` variant) + occlusion in `error` state. |
| memory | 62.4 | 19 | error-leak (F-02). |
| runs | 62.6 | 19 | error-leak (F-02). |
| scientia | 63.6 | 7 | no default-state-only issues beyond axe. |
| policies | 64.7 | 19 | error-leak (F-02). |
| chat | 64.9 | 39 | F-01, F-02, F-04, F-05 concentrated here; most heavily-stated surface (9 explicit states) and most heavily-affected. |
| dashboard | 65.2 | 51 | F-01, F-02, F-03, F-04, F-06, F-07 — the highest-severity concentration of any surface (`ACHIEVEMENTS` + `no-backend` + `hud-hidden` states). |
| console | 69.9 | 7 | F-01 (Discovery/terminal overlap). |
| coderabbit | 73.6 | 7 | clean beyond axe. |
| mercatus | 76.4 | 7 | clean beyond axe. |
| activity, browser, mesh, needs-you, sub-agents | 83.6–85.0 | 7 each | clean; `[DEFAULT]`-only states (Task 5 registry) — lower state density than `chat`/`dashboard`/`settings`/`approvals`, so this also reflects less-probed surface area, not necessarily more robust code (see Coverage table). |
| mens, tasks | 87.0–87.2 | 7–13 | clean. |
| coverage, catalog, flow, publications, repository, research, skills, populi, oratio, harness | 89.0–92.0 | 7 each | clean; all `[DEFAULT]`-only states. |

The correlation between "worst-scoring" and "most explicit non-default states" (`chat`,
`dashboard`, `settings`, `approvals` are the only four surfaces with hand-authored
interaction states beyond `[DEFAULT]`/variants) is itself a finding: **surfaces with only
`[DEFAULT]` states are scoring well largely because they are barely being probed**, not
because they are more correct. See Coverage table.

## Tauri-shell spot check

**Not completed.** Per AGENTS.md, a Tauri-shell (`tauri-driver`) run requires the release
sidecar binary and a running WebDriver-compatible session; this is explicitly named a
prerequisite gap ("Deferred / out of scope" in the harness plan: *"tauri-driver
automation"*). No sidecar/driver harness exists in this repo yet (confirmed: no
`tauri-driver` invocation anywhere in `crates/vox-gui` or CI). Standing up one is out of
scope for this review (Task 13 is capture+analysis+writeup, not new harness infrastructure)
and is recorded here as a explicit exclusion, not a silent gap: the Chromium capture is the
best available proxy for the real desktop shell (both are Chromium-family engines), so F-01
through F-07 above should be treated as **confirmed-in-Firefox, presumed-absent-in-the-Tauri-
shell** pending an actual spot check — a natural first task for the follow-on remediation plan.

## Coverage audit table (derived)

Rows = 31 `SURFACE_REGISTRY` viewKeys. Columns derived mechanically: component `*.test.tsx`
presence under `src/components/surfaces/<Dir>/`, a top-level `e2e/*.spec.ts` naming the
surface, an explicit multi-state entry in `e2e/review/states.ts` (vs. `[DEFAULT]`-only),
presence in the 413-cell bundle-report (all 31, by construction), and `ci.yml` wiring.

| surface | component tests | e2e spec | states.ts multi-state | bundle-report cells | CI wiring |
|---|---|---|---|---|---|
| chat | 9 (Chat/) | 4 specs (chat-composer-dock, chat-interactions, chat-session-rail, session-rail-actions) | yes (5 extra states + variants) | 39 | chromium only (capture.spec.ts, no `firefox-review`) |
| dashboard | 4 | 2 specs (dashboard, dashboard-pilot) | yes (5 extra states + variants) | 51 | chromium only |
| settings | 3 | 1 spec (settings.spec.ts) | yes (2 extra states + variants) | 31 | chromium only |
| approvals | 1 | 1 spec (approvals-interactions.spec.ts) | yes (1 extra state + variants) | 25 | chromium only |
| tasks | 4 | 1 spec (tasks-interactions.spec.ts) | 1 extra state (no variants — omitted, see states.ts) | 13 | chromium only |
| console | 7 | 1 spec (console-workbench.spec.ts) | `[DEFAULT]` only | 7 | chromium only |
| browser | 1 | 2 specs (browser-preview, browser-surface) | `[DEFAULT]` only | 7 | chromium only |
| coderabbit | 1 | 1 spec (coderabbit.spec.ts) | `[DEFAULT]` only | 7 | chromium only |
| policies | 1 | 1 spec (policies.spec.ts) | `[DEFAULT]` only + variants | 19 | chromium only |
| models | 2 | 1 spec (model-picker-interactions.spec.ts) | `[DEFAULT]` only + variants | 19 | chromium only |
| activity | 3 | 0 | `[DEFAULT]` only | 7 | chromium only |
| catalog | 1 | 0 | `[DEFAULT]` only | 7 | chromium only |
| coverage | 1 | 0 | `[DEFAULT]` only | 7 | chromium only |
| flow | 1 | 0 | `[DEFAULT]` only | 7 | chromium only |
| gamify | 2 | 0 | `[DEFAULT]` only + variants | 19 | chromium only |
| harness | 1 | 0 | `[DEFAULT]` only | 7 | chromium only |
| memory | 1 | 0 | `[DEFAULT]` only + variants | 19 | chromium only |
| mens | 0 (dir absent from surfaces/; served by Loquela? verify at remediation time) | 0 | `[DEFAULT]` only | 7 | chromium only |
| mercatus | 0 | 0 | `[DEFAULT]` only | 7 | chromium only |
| mesh | 1 | 0 | `[DEFAULT]` only | 7 | chromium only |
| needs-you | 2 (NeedsYou/) | 0 | `[DEFAULT]` only | 7 | chromium only |
| oratio | 0 (dir absent; likely Loquela) | 0 | `[DEFAULT]` only | 7 | chromium only |
| populi | 0 (dir absent) | 0 | `[DEFAULT]` only | 7 | chromium only |
| publications | 0 | 0 | `[DEFAULT]` only | 7 | chromium only |
| repository | 2 | 0 | `[DEFAULT]` only | 7 | chromium only |
| research | 1 | 0 | `[DEFAULT]` only | 7 | chromium only |
| runs | 1 | 0 | `[DEFAULT]` only + variants | 19 | chromium only |
| scientia | 8 | 0 | `[DEFAULT]` only | 7 | chromium only |
| skills | 2 (SkillsPlugins/) | 0 | `[DEFAULT]` only | 7 | chromium only |
| sub-agents | 4 | 0 | `[DEFAULT]` only | 7 | chromium only |
| vox-search | 0 (Search/ dir has 0 `.test.tsx`) | 0 | `[DEFAULT]` only + variants | 19 | chromium only |

**Zero component-test coverage:** `mercatus`, `publications`, `vox-search` (`Search/` dir has
no `*.test.tsx`), plus `mens`/`oratio`/`populi` whose component directories could not be
matched 1:1 to a `src/components/surfaces/<Dir>` name (likely served by `Loquela/` or another
shared directory — worth a follow-up rename/registry-audit pass, not resolved here to avoid
guessing).

**Zero top-level e2e interaction spec:** 22 of 31 surfaces have no dedicated
`e2e/*.spec.ts` beyond the generic `screenshots*.spec.ts`/the review-bundle capture itself —
only `chat`, `dashboard`, `settings`, `approvals`, `tasks`, `console`, `browser`, `coderabbit`,
`policies`, `models` have one.

**CI wiring gap (mechanical, confirmed):** `ci.yml`'s `gui-playwright-smoke` advisory step
(line 1682) runs `--project=chromium` only — Firefox is never captured in CI, and the
analysis step (line 1687) has no `--browsers` flag, so it silently defaults to
`chromium`-only even if Firefox entries existed. **This means the entire class of
Firefox-only defects found in this review (F-01, F-04, F-05, and the Firefox-specific
instance of F-03) is invisible to CI today** — the only way they were caught here is this
Task 13 local run explicitly requesting `firefox-review`. This is the single most
actionable coverage gap from this review and should be the first line item in any
remediation/CI-hardening follow-up.

**Explicit exclusions (not gaps):** doc-reader tabs (`DocReader/`), toast/notification
components themselves (reviewed only as artifacts *within* other surfaces' captures, per the
harness's out-of-scope list), and the Tauri-shell spot check (see above).

## Recommended remediation order

1. **CI wiring**: add `firefox-review` to the CI capture step and `--browsers
   chromium,firefox` to the CI analysis step (mechanical, ~2-line change) — otherwise every
   fix below regresses silently the day this review is forgotten.
2. **F-01/F-04 (Firefox overlay compositing)** — highest severity, highest cell count, single
   root cause across 3+ surfaces; fix the shared overlay/popover primitive's Firefox
   backdrop rendering.
3. **F-03 (`__TAURI_INTERNALS__` toast leak)** — small, isolated, directly answers the user's
   second reported complaint; route caught-rejection toast bodies through the same honesty
   standard as the `unhandledrejection` filter.
4. **F-02 (`session_id`/`is_error` null TypeError)** — one shared root cause reused across 9
   surfaces; highest fix-leverage item on the list after F-01.
5. **F-07 + the `aria-required-children`/`page-has-heading-one`/`landmark-unique` axe classes**
   — batch-fixable via one heading/landmark/ARIA-role pass across surface roots.
6. **F-05/F-06** — re-verify after F-01 lands; may partially self-resolve.
7. **Coverage**: add component tests for `mercatus`, `publications`, `vox-search`; add
   dedicated e2e specs for the 21 remaining un-specced surfaces (lowest priority — tracked
   as debt, not a defect).

All of the above is scoped for a **separate remediation plan** per this harness plan's
"Out of scope" section — this document is the findings register that plan should consume,
not a fix itself.

## Remediation status (2026-07-19)

Executed via [`docs/superpowers/plans/2026-07-18-axis-frontend-remediation.md`](../plans/2026-07-18-axis-frontend-remediation.md)
(12 tasks, subagent-driven with two-stage review — spec compliance then code
quality — after every implementation). All results below are re-derived from a
fresh full-matrix harness run (413 cells, both browsers) after the last code
change landed, not carried over from the original review.

| Finding | Status | Evidence |
|---|---|---|
| CI wiring gap (Firefox invisible to CI) | **Fixed** | Task 1: `playwright install chromium firefox`, `--project=firefox-review` added to capture, `--browsers chromium,firefox` added to analysis, in `.github/workflows/ci.yml`'s `gui-playwright-smoke` job. |
| F-01/F-04 (Firefox overlay occlusion/layout collapse) | **Fixed** | Task 2: `Glass.tsx`'s background token changed from a ~4%-alpha translucent tint (`bg-overlay-subtle`, base AND hover) to an opaque token (`bg-overlay-solid`/`bg-bg-elevated`), across both the main and travertine theme token files. Confirmed by two independent harness re-runs: the AchievementsDrawer's own defect description changed from *"rendered with transparency... stuck in a semi-transparent state"* (score 15–35) to *"is an opaque overlay... without a dimming backdrop"* (score 85, pass_with_notes); zero defects anywhere in the final 413-cell run mention transparency/blending/backdrop-compositing. |
| F-02 (`session_id`/`is_error` null-deref TypeError leak) | **Fixed** | Task 5: guarded 4 sites in `App.tsx` (chat-session creation, MCP rollback, MCP audit ×2) with `!res ||`-style null checks, tested via toast-content assertions (not crash-freedom, since every deref was already caught). **Plus an unplanned gap found during Task 12's final verification**: `ApprovalsView.tsx` had 3 more unguarded sites sharing the identical pattern (never in Task 5's App.tsx-only scope) — `approvals--empty--*` cells leaked the exact `is_error`/`res is null` TypeError text across both browsers. Fixed (commit `14cf90d621`), re-verified: those cells now score 85 pass_with_notes with only unrelated minor contrast/clipping notes. Final full-matrix run: zero `session_id`/`is_error`-null leak defects anywhere. |
| F-03 (`__TAURI_INTERNALS__`/raw IPC toast leak) | **Fixed** | Task 4: `sanitizeErrorForToast()` added to `src/lib/backendGuard.ts`; mechanically applied to all ~109 `body: String(err)` sites across 28 files (not just the 14 in `App.tsx`, which was only ~13% of the class) plus a follow-up pass covering 9 non-toast display-state sinks (`setX(String(err))` patterns) found by broadening the guard. A source-scan guard test (`toastBodyGuard.test.ts`) prevents regrowth. Final full-matrix run: zero `__TAURI_INTERNALS__` occurrences anywhere in 413 cells. |
| F-05 (session-list clipping) | **Fixed for realistic titles; residual is a mock-data artifact** | Task 11 triage confirmed the original hypothesis (F-05 as a downstream symptom of F-01) was **refuted** — clipping was present on both browsers independent of the transparency fix. Follow-up fix (commit `ca18981267`): widened the session rail 176px→256px, changed titles from single-line `truncate` to `line-clamp-2`, and added a native `title` tooltip for full-text discoverability. Re-verification showed a real improvement (`chat--session-menu-open--wide` moved from `fail` to `pass_with_notes`, clipping downgraded major→minor) but compact-viewport cells still show ellipsis — traced to `e2e/lib/tauriMockRich.ts`'s mock title generator padding titles to 90–142 characters as an overflow stress test, which no reasonable rail width avoids eliding. Accepted as expected behavior for pathological input; see `.remediation-notes/task11-f05-verdict.md` and `.remediation-notes/task-f05-fix-verdict.md`. |
| F-06 (blank simulation viewport) | **Fixed** | Task 10, corrected premise: the `scanFailed` state was *already* handled and tested before this plan started — the actual gap was `!layout && !scanFailed` (scan pending, or resolved without a layout), which left the mounted `<canvas>` silently blank. Fixed with a loading-affordance overlay (canvas stays mounted for ref stability, hidden via `invisible`), tested by making the scan hang forever. |
| F-07 (tablist `aria-required-children` violation) | **Fixed** | Task 6: `WorkbenchTabBar` restructured so the tab wrapper itself carries `role="tab"` (NOT `role="presentation"`, which would not have silenced the rule — axe looks through presentational wrappers). Close affordance became `aria-hidden` + pointer-only, closable via `Delete`; a code-quality review caught that the initial restructure's roving `tabIndex` removed keyboard access to background tabs entirely (a regression vs. the pre-fix state) — fixed with full Arrow/Home/End keyboard navigation plus `aria-keyshortcuts="Delete"` before landing. Full-matrix re-run: `aria-required-children` dropped from a peak of 51 (dashboard) to 0 everywhere except chat's 23, which trace to an **unrelated pre-existing bug** (the chat model-picker's `role="option"` elements are wrapped in `<li>` instead of being direct children of `role="listbox"`) — confirmed never in this task's scope, logged as follow-up debt. |
| `page-has-heading-one` (axe) | **Fixed on chat + dashboard (scoped); debt elsewhere** | Task 7: sr-only `<h1>` added to both surfaces' roots, including a self-caught gap in Dashboard's loading-skeleton branch (which returns before the main-branch h1, so needed its own copy). Final run: 0 on chat/dashboard (was 39/39); 284 instances remain across the other 27 surfaces — explicitly out of scope for this plan, tracked below. |
| `landmark-unique` (axe) | **Fixed, exceeded scope** | Task 8: labeled Sidebar's nav+aside, both chat rails' asides; removed an unlabeled per-message `role="region"` from `ModelBadge.tsx` (rendered once per chat message — the actual duplication source, not a phantom "second nav" as an early hypothesis assumed). Final run: 0 everywhere, including `settings` (baseline 19), which was outside the plan's literal scope but cleared as a side effect. |

### Recorded debt (not fixed in this plan — explicit exclusions)

- **`page-has-heading-one`** on 19 enumerated surfaces beyond chat/dashboard (full list: `ApprovalsView`, `CodeRabbitView`, `CoverageView`, `GamifyView`, `MemoryView`, `MeshView`, `ModelsView`, `PoliciesView`, `PublicationsView`, `RepositoryView`, `ResearchView`, `RunsView`, `ClaimsView`, `SkillsPluginsView`, `SubAgentsView`, `ActivitySurface`, `DiscoverySurface`, `NeedsYouSurface`, `ScientiaSurface`) — 284 total axe instances across all non-chat/dashboard surfaces.
- **Chat model-picker `aria-required-children` violation** (`role="option"` wrapped in `<li>` instead of direct `role="listbox"` children) — 23 instances, pre-existing, never in Task 6's scope.
- ~~F-05 session-title clipping~~ — fixed (see table above); residual compact-viewport ellipsis on the harness's 90–142-char stress-test mock titles is expected, not a bug.
- **SESSIONS panel bottom-truncation** (`chat--rails-overlay-open--compact`, major clipping, both browsers) and **console DISCOVERY-panel structural overlap** (`console--default--compact`, major occlusion via layout/sizing — not transparency, both browsers score ~45) — surfaced incidentally by Task 3's verification run; pre-existing, cross-browser, never part of F-01/F-04's Firefox-transparency signature.
- **Achievements panel missing a dimming scrim** behind the (now-correctly-opaque) overlay — cosmetic, minor.
- Item 7 from the original recommended order (component/e2e test coverage for `mercatus`, `publications`, `vox-search`, and 21 un-specced surfaces) — untouched, as originally scoped out.
- The Tauri-shell (`tauri-driver`) spot check — still not run; no such harness exists in this repo.
- `color-contrast` (75 instances) and `heading-order` (58 instances) axe classes — visible in the same harness data, never in this plan's scope.

### Process note

Task 12's final verification sweep (re-running the full harness after all 11 other
tasks landed) caught a real, unplanned gap — the `ApprovalsView.tsx` F-02 leak —
that the original task scoping (App.tsx only) missed entirely. This is direct
evidence the "verify with the same harness that found the bugs" methodology works
as intended, not just as a formality.
