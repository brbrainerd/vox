# Handoff Prompt — GUI IPC Resilience & Command-Parity Linting

**Target executor:** Claude Sonnet 4.6 (fresh session).
**Created by:** Opus 4.8, 2026-06-20, during the Vox Axis "Limes" restyle session.
**Plan:** [`docs/superpowers/plans/2026-06-20-gui-ipc-resilience-and-parity-linting.md`](plans/2026-06-20-gui-ipc-resilience-and-parity-linting.md) — read it in full first; it is self-contained and TDD-structured.

---

## Copy-paste kickoff for the new session

> You are implementing the plan at
> `docs/superpowers/plans/2026-06-20-gui-ipc-resilience-and-parity-linting.md`.
> Use the **superpowers:subagent-driven-development** (or **executing-plans**)
> skill and work task-by-task. The plan turns three runtime error classes in
> the Vox Axis GUI into compile/CI-time guardrails. Start with Workstream 1
> Tasks 1–3 (they unblock the rest), then run WS2 Task 5 and WS3 Task 9 — each
> *discovers* the real backlog of unregistered commands / broken surfaces, so
> expect their first test run to fail and treat every failure as a tracked fix,
> not a reason to weaken the gate. Do not merge; this lands on a branch for
> review.

## Session context the new agent needs

**Why this exists.** A "Limes" Roman restyle of `crates/vox-gui/` was just
completed and the user clicked through every surface in a **bare browser**
(`pnpm dev` → `http://localhost:1420`, no Tauri webview). Every surface threw
`TypeError: can't access property "invoke", window.__TAURI_INTERNALS__ is
undefined` and showed fallbacks like "Memory status unavailable". These are
**not styling bugs** — they are pre-existing fragilities the restyle merely
surfaced: surfaces call `invoke()` with no environment guard, no fallback, and
no error boundary, and nothing checks that the commands they call are actually
registered on the Rust host. The user asked for these to be "linted and fixed …
ensured for by parity by design."

**Where the code is.**
- Worktree: `C:\Users\Owner\vox\.claude\worktrees\cool-kapitsa-15e2dd`
- Branch: `claude/cool-kapitsa-15e2dd` (restyle commits live here; build on it or branch from it).
- Frontend: `crates/vox-gui/ui` (React 19 + TS + Vite + vitest jsdom). Run commands from here.
- Rust host: `crates/vox-gui/src`; command registry at `crates/vox-gui/src/main.rs:108` (`tauri::generate_handler![ … ]`, ~169 commands).

**Existing infra to extend, NOT rebuild** (the plan's File Structure section details each):
- `ui/src/transport.ts` — the `VoxTransport` IPC hub; imports `invoke` raw (the chokepoint WS1 wraps).
- `ui/src/guards/ipcBoundaries.test.ts` — existing static allowlist gate (`ALLOW_DIRECT_INVOKE`) tracking the ~30 surfaces that still import `invoke` directly; WS1 shrinks it toward `{ lib/ipc.ts }`.
- `ui/src/generated/surfaceRegistry.generated.ts` — `SURFACE_REGISTRY`, SSOT of all surfaces; WS3 smoke-tests every entry.

**The four guardrails (one per error class + glue):**
1. **WS1** — `lib/ipc.ts`: `isTauri()` + `safeInvoke()` + `IpcUnavailableError` + `lib/devMocks.ts`. Surfaces render in a browser via mocks and degrade to one catchable error in prod. Then route `transport.ts` (and, in waves, each surface) through it.
2. **WS2** — frontend⊆backend command-parity gate: a vitest guard (`guards/commandParity.test.ts`) **and** a `.vox` CI gate (`scripts/gui-command-parity.vox`) asserting every `invoke('cmd')` literal is in `generate_handler!`.
3. **WS3** — `SurfaceErrorBoundary` wrapping the active surface in `App.tsx` + `guards/surfaceSmoke.test.tsx` mounting every `SURFACE_REGISTRY` surface under a mocked transport.
4. **WS4** — SSOT doc `docs/src/architecture/gui-ipc-resilience.md` + CI wiring + final verification.

## Project-specific gotchas (from repo memory / `AGENTS.md`)

- **No new `.ps1`/`.sh`/`.py`** automation — the CI gate must be VoxScript (`.vox`). Copy an existing gate's structure (`grep -rl "vox ci" scripts/*.vox`). Run `.vox` with `--mode interp`; single-line fn sigs; no multi-line `+` exprs; no `list.set`.
- Component tests need `// @vitest-environment jsdom` as the **first line** (no global vitest config).
- **Windows:** never pipe `cargo`/`pnpm` through `head`/`grep` — it orphans thousands of processes. Redirect to a file or rely on the Bash tool's own truncation.
- `docs/src/**` Markdown needs frontmatter incl. `category: "Architecture SSOTs"`.
- `pnpm dlx @tauri-apps/cli` currently fails with `ERR_PNPM_NO_IMPORTER_MANIFEST_FOUND` in this environment (not needed for this plan, just noted).

## Definition of done

- `cd crates/vox-gui/ui && pnpm typecheck && pnpm vitest run` → all green.
- `vox run scripts/gui-command-parity.vox --mode interp` → exit 0.
- `ALLOW_DIRECT_INVOKE` reduced to `{ 'lib/ipc.ts' }`.
- `pnpm dev` opened in a plain browser shows surfaces rendering (dev mocks / localized fallback cards), no `__TAURI_INTERNALS__` crashes.
- Committed on a branch, **not merged** (human merge gate).
