---
category: "Architecture SSOTs"
title: "GUI Honesty Audit + Durable Prevention — Design"
date: 2026-06-25
status: design
---

# GUI Honesty Audit + Durable Prevention — Design

**Goal:** Find and fix every non-functional GUI element (dead buttons, no-op toasts,
"not yet wired" placeholder text) in `crates/vox-gui/ui`, then make "shipping a broken
element" a compile/CI failure so it cannot recur — now and for future changes.

**Intended executor:** Sonnet 4.6, via `subagent-driven-development`.

## Problem

The Vox GUI surfaces ~28 navigable views (`SURFACE_REGISTRY`,
`crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`). An initial sweep finds
**114 placeholder / "not (yet) implemented / coming soon / stub / no-op" strings across 42
files**, a global `Toasts` system plus per-surface toasts (`SecretaryToast`,
`AchievementToast`, `useAchievementToasts`), and interactive elements whose handlers lead
nowhere. The result: the UI advertises behavior it does not have, and shows toasts that
confirm nothing. There is no automated guard preventing this, so it keeps growing.

## Goals

1. **Audit** every surface for (a) visual-design errors and (b) non-functional behavior.
2. **Fix** each finding under the policy: *wire it if a backend command already exists and
   the wiring is cheap; otherwise hide it behind a flag so the shipped UI only shows things
   that work.* Remove no-op and redundant toasts.
3. **Prevent recurrence** with two dependency-free enforcement layers wired into CI.

## Non-Goals

- **No Figma.** Figma cannot detect broken behavior and is a lossy manual redraw. The
  evaluation substrate is the app's own runtime: `vox ci gui-visual-review` (screenshot →
  AI design critique) + Playwright e2e. (A Figma redesign target is a possible *later*
  follow-up for surfaces that fail the visual critique; out of scope here.)
- **No new lint framework.** The repo has no ESLint. Adding one violates the
  use-what-is-installed rule. Enforcement uses `tsc` (already in `typecheck`/`build`) and
  `vitest` (already the test runner), mirroring existing source-scan guard tests.
- **No unbounded backend building.** Elements with no cheap backing are hidden, not built
  out. Building missing Rust/Vox commands is deferred to per-element follow-up tickets.
- **No worktrees.** Surfaces live in separate directories under
  `crates/vox-gui/ui/src/components/surfaces/<Name>/`, giving a clean file partition; shared
  files are edited in a single serial task. Parallel agents therefore never conflict.

## Definitions

A GUI element is **non-functional** if any of:
- **dead** — an `onClick`/`onSubmit`/`onChange` whose body is empty, `() => {}`, or only
  logs / only calls `pushToast` without any state or backend effect.
- **noop-toast** — a `pushToast(...)` that is the *entire* effect of an action (the action
  claims success but changes nothing).
- **placeholder** — visible text matching `/\b(not (yet )?(implemented|wired|available|
  working|hooked|connected)|coming soon|todo|stub|placeholder)\b/i` rendered in a shipped
  (non-hidden) code path.

An element is **cheap to wire** if a Tauri command (`invoke('<cmd>')`) or an existing
local handler already implements the behavior and wiring is ≤ ~15 lines with no new backend.

## Toast taxonomy (drives the Toast fix + enforcement)

Current type (`crates/vox-gui/ui/src/types/tauri.ts`):

```ts
export type Toast = {
  tone: 'ok' | 'warn' | 'info';
  title: string;
  body?: string;
  cmd?: string;
};
```

A toast is legitimate only if it reports the result of a *real* effect. The enforcement
layer adds a required, typed `cause` so a toast that does not name a real effect cannot be
written:

```ts
export type ToastCause =
  | 'backend-ok'      // a Tauri command / mutation succeeded
  | 'backend-error'   // a Tauri command / mutation failed
  | 'validation'      // user input rejected before any effect
  | 'navigation'      // confirmed a view/route change
  | 'clipboard'       // copied to clipboard (a real OS effect)
  | 'external';       // opened an external app/url

export type Toast = {
  tone: 'ok' | 'warn' | 'info';
  title: string;
  body?: string;
  cmd?: string;
  cause: ToastCause; // required — a causeless toast is a compile error
};
```

There is no `cause` value for "a thing that did nothing", so a no-op toast becomes
unwritable. This is the "compiler-level" catch.

## Workflow (6 phases)

Concurrency cap **8** (memory: server rate-limit + cargo-lock contention above this).

### Phase 0 — Inventory & baseline *(serial, main loop)*
Build one canonical manifest JSON of every interactive element + toast call site:
`{surface, file, line, kind, snippet, has_handler, handler_target, backend_command_exists}`.
Capture before-screenshots via the existing `playwright.screens.config.ts`.
**Gate G0:** manifest exists; baseline screenshots captured; `pnpm test` + `pnpm typecheck`
green. *(automated)*

### Phase 1 — Audit *(parallel, one sub-agent per surface ≈28, cap 8)*
Each surface sub-agent: (a) visual critique via `vox ci gui-visual-review` output for that
surface; (b) behavioral trace — follow every handler to a real `invoke`/event or prove it is
dead/noop-toast/placeholder. Returns findings JSON per the Definitions above. Surfaces are
independent → clean fan-out, no shared state.
**Gate G1:** every surface has a findings file; a second adversarial agent re-checks a
sample of `dead` verdicts so we never "fix" something merely hard to trace.
*(automated + sampling)*

### Phase 2 — Triage & synthesis *(serial, main loop — barrier; needs all findings)*
Merge/dedup into one decision table, one row per element:
`WIRE | HIDE | KEEP | TOAST-FIX`.
**Gate G2 (human):** user reviews the triage table before any code change — it decides which
intended features get hidden.

### Phase 3 — Fix *(parallel, one sub-agent per surface; TDD)*
Per element: write the e2e/vitest assertion first (element does X, or is absent), then
apply its triage decision. Shared files (`App.tsx`, `surfaceComponents.tsx`, `Toasts.tsx`,
`types/tauri.ts`, registries) are pulled OUT into one serial task — never edited by two
agents at once.
**Gate G3:** per surface, `pnpm test` green + `vox ci gui-visual-review` re-critique not
worse; then `superpowers:code-reviewer` pass. *(automated + review)*

### Phase 4 — Durable prevention *(serial, single agent; TDD)*
- **Layer A (type, tsc):** add required `ToastCause` (above). Update all real toast sites.
  A causeless toast now fails `tsc --noEmit`.
- **Layer B (vitest guard):** `surfaceHonesty.guard.test.ts` scans
  `src/components/surfaces/**` source for banned placeholder literals and dead handlers,
  asserting none outside an explicit `HIDDEN_ALLOWLIST` (flag-gated, non-shipped paths).
- **Layer C (CI gate):** add `vox ci gui-honesty` (Rust subcommand mirroring
  `gui-surface-registry`) that runs the typecheck + the guard test and exits non-zero on
  violation; register it in the CI gate list.
**Gate G4:** the gate fails-red on a planted violation and passes-green on the clean tree.
*(automated self-test)*

### Phase 5 — Verify & close *(serial)*
Full e2e + visual review + new gate green; before/after screenshot diff as proof;
`finishing-a-development-branch`. *(verification-before-completion + human merge gate)*

## Agent / sub-agent model

- **Sub-agents (parallel)** ONLY for per-surface independent work with bounded context:
  Phase 1 audit and Phase 3 fix. Dispatched via `dispatching-parallel-agents`, cap 8.
- **Main loop (serial)** for anything needing the whole-repo view or touching shared files:
  Phase 0 inventory, Phase 2 triage barrier, Phase 3 shared-file task, Phase 4 cross-cutting
  enforcement, Phase 5 verify.

## Gates summary

| Gate | Phase | Type |
|------|-------|------|
| G0 | 0 | automated (manifest + baseline + green) |
| G1 | 1 | automated + adversarial sampling |
| G2 | 2 | **human** (triage approval) |
| G3 | 3 | automated tests + visual re-critique + code review |
| G4 | 4 | automated self-test (red-then-green) |
| G5 | 5 | verification-before-completion + human merge |

## Skills invoked (in order)

`writing-plans` → `dispatching-parallel-agents` + `subagent-driven-development` (Ph 1, 3) →
`test-driven-development` (Ph 3, 4) → `systematic-debugging` (when a "dead" element is
actually mis-wired) → `requesting-code-review` / `code-reviewer` (G3, G4) →
`verification-before-completion` (Ph 5) → `finishing-a-development-branch` (close).

## Success criteria

1. Zero shipped placeholder strings (guard green; allowlist documents every exception).
2. Every interactive element either does something real or is absent from the shipped UI.
3. No no-op toasts; every `pushToast` carries a truthful `cause`.
4. `vox ci gui-honesty` exists, gates CI, and is proven red-then-green.
5. Full e2e + visual review green; before/after screenshot diff attached.
