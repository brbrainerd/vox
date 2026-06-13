---
title: "vox-mental-tracker modernization"
description: "Bring the half-migrated (Capacitor→Tauri) mental-health tracker to a working, modern web app: green CI (vitest+Playwright), real /timeline & /settings, Clinical-Clean restyle, screenshot artifacts."
category: "Architecture SSOTs"
status: "current"
---

# vox-mental-tracker modernization (2026-06-13)

## Summary

The `apps/vox-mental-tracker` app is ~90% through a Capacitor→Tauri migration
but left broken: a stale CI step, tests that need a `vox build`, dead Capacitor
remnants, and two placeholder screens. This brings it to a **working, modern,
web-target** state with **green CI** (vitest + Playwright e2e) and visual
confirmation via per-route screenshot artifacts.

The app is authored in **Vox** (`src/main.vox`, 757 lines): `vox build` emits
`dist/*.tsx` + `runtime-install.ts` + `vox-tokens.css`, then `vite build` →
`web-dist/`. All feature and visual changes are **edits to `main.vox`**, not
hand-written TypeScript.

**Scope decisions (locked in brainstorming):**
- **Target:** web build only. No native `src-tauri` shell is built; the Tauri
  STT path keeps its browser/test shim. STT stays mocked in tests.
- **Aesthetic:** "Clinical Clean" — crisp white, `#2563eb` blue accent, tight
  numeric scale, 0.5px borders, data-forward.
- **Data safety:** append-only event log + tombstone soft-delete is preserved;
  settings is **non-destructive** (no clear-data action that could lose patient
  / clinical data).
- **Visual confirmation:** Playwright captures one screenshot per route as a CI
  **artifact**; screenshots are **gitignored, never committed** (no
  pixel-diff baselines).

## Phases (strict order — nothing is visually confirmable until P0 works)

```
P0 Build/CI repair → P1 Cleanup → P2 Feature completion → P3 Visual restyle → P4 Playwright confirmation
```

### P0 — Build/CI repair (the unblock)
- Remove the stale `pnpm exec tsc -p plugins/vox-sherpa-transcribe/tsconfig.json`
  step from `.github/workflows/vox-mental-tracker.yml` (the plugin dir was
  deleted in `abe849746b`).
- Make the `vitest` job runnable: `tests/runtime_shim.test.ts` imports
  `../dist/runtime-install`, which only exists after `vox build`. Add a
  `pnpm build:vox` step before the vitest step (the runner already builds
  `vox-cli` for the `vox-check` job).
- Lockfile: already regenerated (`apps/vox-mental-tracker/pnpm-lock.yaml`;
  `pnpm install --frozen-lockfile` passes) — committed in P0.
- Local exit criteria: `pnpm build:web` produces `web-dist/`; `vite preview`
  serves a working app; `pnpm exec vitest run` is green.

### P1 — Cleanup (dead Capacitor)
- Delete `apps/vox-mental-tracker/capacitor.config.ts` and the
  `apps/vox-mental-tracker/ios/` directory.
- Remove the `capacitor` keyword from `apps/vox-mental-tracker/Vox.toml`.
- Verify zero remaining `@capacitor/*` references (already confirmed by the
  explore pass).

### P2 — Feature completion (Vox edits to `src/main.vox`)
- **`/timeline`**: replace the count-only placeholder with a real
  reverse-chronological list of *effective* events — honoring the existing
  tombstone / `correction_of` soft-delete model (`_is_effective`) — showing
  each event's kind, local time, and a one-line payload summary.
- **`/settings`**: a real, **non-destructive** settings screen: timezone
  display, and export shortcuts that reuse the existing CSV + clinical-HTML
  exporters. No clear/reset action (data preservation). Document that
  "starting fresh" is an export-then-archive workflow outside the app, since
  the log is append-only by design.

### P3 — Visual restyle (Clinical Clean)
- Update the Vox-emitted design tokens / `.mh-*` styles to the Clinical-Clean
  palette consistently across all 7 routes (`/`, `/mood`, `/timeline`,
  `/weekly`, `/export`, `/voice`, `/settings`): white surfaces, `#2563eb`
  primary, `#0f2747` headings, `#5b6b80` secondary text, 0.5px `#e2e8f0`
  borders, 6px radii, tight spacing, numeric mood scale.
- Source of truth: whatever `main.vox` / its token emission controls. Where the
  theme lives in `index.html` inline `.mh-*` CSS vs Vox-emitted
  `vox-tokens.css`, edit the authoritative one (determined in the plan) so a
  rebuild reproduces it deterministically.

### P4 — Playwright confirmation
- Extend `tests/e2e/` to one spec per route asserting key elements render, and
  capture a screenshot per route via `page.screenshot()` into a gitignored
  `tests/e2e/__screens__/` (added to `.gitignore`).
- CI `playwright` job uploads that directory with `actions/upload-artifact`.
- STT stays mocked via the existing `__VOX_TEST_TRANSCRIPT__` stub; the
  `voice_flow` spec keeps its `VOX_BACKEND_URL`-gated parse assertion.
- Exit criteria: the full `vox-mental-tracker.yml` workflow is green
  (`vox-check`, `vitest`, `playwright`, `contracts`, `app-summary`).

## Components / where things live

| Concern | Location |
|---|---|
| App source (features, routes, components, theme tokens) | `apps/vox-mental-tracker/src/main.vox` |
| Build pipeline | `package.json` scripts (`build:vox`→`build:fixup`→`vite build`), `vite.config.ts`, `index.html` |
| CI | `.github/workflows/vox-mental-tracker.yml` |
| Unit test | `apps/vox-mental-tracker/tests/runtime_shim.test.ts` |
| e2e specs + screenshots | `apps/vox-mental-tracker/tests/e2e/*.spec.ts`, `tests/e2e/__screens__/` (gitignored) |
| Tauri STT guest (unchanged) | `crates/vox-tauri-stt/guest-js/` |

## Error handling / degradation
- No STT/Tauri at runtime in the browser → the emitted `runtime-install` shim
  and `__VOX_TEST_TRANSCRIPT__` keep the voice route testable; real STT is a
  native-shell concern out of scope here.
- `vox build` unavailable in a job → that job fails loudly (no silent skip);
  the binary is produced on the same self-hosted runner image used by
  `vox-check`.
- Data: every mutation remains append-only; deletions are tombstones; settings
  exposes no destructive action.

## Testing
- **P0:** `vitest` green (runtime shim) after `vox build`; manual `vite preview`
  smoke.
- **P2:** new Playwright assertions for `/timeline` (list renders effective
  events) and `/settings` (tz + export shortcuts present, no destructive
  control).
- **P4:** all 7 route specs pass + screenshots captured; whole workflow green.

## Out of scope
- Native `src-tauri` desktop shell and live on-device STT verification.
- Visual-regression pixel baselines.
- Android/iOS native packaging.
- Any change to the append-only data model or the Vox compiler/codegen itself
  (we consume what it emits; compiler fixes, if needed, are a separate effort).
