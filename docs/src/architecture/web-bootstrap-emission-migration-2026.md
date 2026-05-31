---
title: "Web Bootstrap Emission — Migration Plan (2026)"
description: "Bring the Vox web target to RN parity by emitting the app entry, a generic router over routes.manifest, and the runtime-global install — retiring the hand-written web shell so the .vox is the single source of truth."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: false
---

# Web Bootstrap Emission — Migration Plan (2026)

> Produced by a 5-agent scouting workflow (`web-bootstrap-migration-scout`) +
> Stage-0 verification. Goal: **fully migrate the vox-mental-tracker web bootstrap
> to Vox-emitted output**, eliminating the hand-written app-level split-brain.

## Problem (verified)

The web target emits only **artifacts** — `{Component}.tsx`, `routes.manifest.ts`,
`routes.ts`, `vox-client.ts`, `types.ts` — and pushes all **bootstrap** to
hand-written files (`emitter.rs:171` `generate_with_options`). RN, by contrast,
emits a complete runnable bootstrap (`rn/mod.rs:57` `generate_rn` → `App.tsx` /
Expo Router tree + full Expo scaffold). The mental-tracker hand-writes
`src/main.tsx` (entry + a client router over the emitted `routes.manifest`),
`src/runtime.ts` (runtime-global install), `src/sync.ts` (SW), and
`src/ErrorBoundary.tsx`; `src/pages/SettingsPage.tsx` is an **orphan** duplicate
of the emitted `SettingsPage`.

## Stage-0 finding (verified)

- `str()`→`String()` and `len()` rewrites **work** — no bare `str(`/`len(` in
  emitted web. Those globals are **not** load-bearing.
- Emitted components **do** reference `mobile.*` (and `Speech.*`/`std.*`) as bare
  globals. The emitted `runtime-install.ts` must reproduce those browser-fallback
  globals **and** the `__VOX_TEST_TRANSCRIPT__` e2e hook (`runtime.ts`).

## What WEB codegen must newly emit (new `web_entry.rs`)

Three artifacts into `dist/` (overwritten every build — NOT the never-overwrite
`scaffold.rs` path, which is retired as the bootstrap mechanism):

- **`entry.tsx`** — `ReactDOM.createRoot(#root).render(<VoxApp/>)` + side-effect
  import of `runtime-install`. Mirror of RN `emit_app_tsx` (`rn/scaffold.rs:200`).
- **`vox-app.tsx`** — generic `BrowserRouter` + `Routes` walking `voxRoutes`
  (`VoxRoute[]`, `route_manifest.rs:168-192`), generalizing the hand-written
  `renderRoutes()`. **Must branch to a flat single-component mount when no routes
  are declared** (`voxRoutes` is absent — `route_manifest.rs:118`), using
  `screen_root_component_names(hir)`.
- **`runtime-install.ts`** — installs the browser-fallback `mobile`/`Speech`/`std`
  globals + `__VOX_TEST_TRANSCRIPT__`. (Follow-up C2: publish `@vox/runtime-web-shims`.)

Wire-in: push the three pairs into `files` from `emitter.rs` after the component
loop, gated on `mode != Library` and `!hir.components.is_empty()`.

## Delete vs keep (app `src/`)

| File | Verdict | Why |
|---|---|---|
| `src/main.tsx` | DELETE | Superseded by emitted `entry.tsx` + `vox-app.tsx`. |
| `src/runtime.ts` | DELETE (after Stage 2) | Becomes emitted `runtime-install.ts`. |
| `src/pages/SettingsPage.tsx` | DELETE | Confirmed orphan — emitted `SettingsPage` is canonical. |
| `src/ErrorBoundary.tsx` | KEEP | Real app glue (React error boundary + crash log); no Vox equivalent. |
| `src/sync.ts` | KEEP | Real app glue (PWA service-worker registration). |
| `index.html` | KEEP (edit) | Provides `#root`; repoint script `src/main.tsx` → `dist/entry.tsx`. |

**Glue hook:** emitted `entry.tsx` looks for an optional `src/app-hooks.ts`
exporting `{ wrapApp?, onBoot? }` — `ErrorBoundary` wraps via `wrapApp`,
`registerServiceWorker` runs via `onBoot`. Keeps the genuine glue app-owned
without a hand-written bootstrap.

## Tests to keep green

- `tests/runtime_shim.test.ts` imports `../src/runtime` → repoint to emitted shim.
- `tests/e2e/voice_flow.spec.ts` needs `__VOX_TEST_TRANSCRIPT__` → emitted shim must keep it.
- `package.json` build chain (`build:vox` → `vite build`) is unchanged; emitted files land in `dist/`.

## Default & opt-out

- **Default:** emit full bootstrap every web build (parity with RN, single source).
- **Opt-out:** `--no-emit-entry` / `entry = false` via `CodegenOptions` for external-React
  consumers (interop Phase 5). Inverts today's opt-in `--scaffold` / `VOX_WEB_EMIT_SCAFFOLD`,
  which is retired (`build.rs:294-300`); keep `--scaffold` as a deprecated alias one release.

## Staged build order (each its own PR; gates: `vox check`, web build, app e2e, arch-check)

0. ✅ **Resolve the `str`-global unknown** — not load-bearing; `mobile`/`Speech`/`std` are.
1. ✅ **Emit `vox-app.tsx` router** — `web_entry.rs`; dependency-free history router; tsc-clean vs `routes.manifest`.
2. ✅ **Emit `runtime-install.ts`** — generic browser shims + `__VOX_TEST_TRANSCRIPT__`.
3. ✅ **Emit `entry.tsx` + default `app-hooks.tsx`; repoint `index.html`** — app glue via app-owned `src/app-hooks.tsx` (overrides the emitted default via `postbuild-fixup.mjs`).
4. ✅ **Delete orphans** — `main.tsx`, `runtime.ts`, `pages/SettingsPage.tsx`; repoint `runtime_shim.test.ts`.
5. **In progress** — `--no-emit-entry` opt-out (`VOX_WEB_NO_EMIT_ENTRY`) wired; retire `scaffold.rs` bootstrap + this doc. Also folding the remaining hand-written glue (`ErrorBoundary`/`sync`) into emitted defaults so a fresh Vox web app needs zero hand-written TS.

**Verification:** `web_entry` unit tests + cli-tests fixtures (20/0) + the app's Vite bundle of the emitted bootstrap + Playwright e2e (`build:web` must out-run Playwright's 180s `webServer` timeout — pre-build the release `vox` binary so `build:vox` is fast).

## Risks

- `#root` assumption implicit in `entry.tsx` — `index.html` must keep `id="root"`.
- Router lock-in: emitted `vox-app.tsx` hardcodes `react-router` v7; external/TanStack consumers use `--no-emit-entry`.
- No-routes path must branch to flat mount or route-less apps break.
- `__VOX_TEST_TRANSCRIPT__` must survive the move into emitted code.

**Key files:** new `crates/vox-codegen/src/codegen_ts/web_entry.rs`;
`emitter.rs:171-225` (wire-in + `CodegenOptions` flag); `build.rs:294-300`
(retire scaffold trigger); `scaffold.rs` (retire); app: `index.html` (repoint),
new `src/app-hooks.ts`, delete `main.tsx`/`runtime.ts`/`pages/SettingsPage.tsx`.
