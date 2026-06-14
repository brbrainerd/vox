# vox-gui Design-Principles Application — Design Spec

**Date:** 2026-06-14
**Status:** Design approved (brainstorming) → handing to writing-plans
**Author:** AI Assistant (brainstorming session)
**Executor target:** Sonnet 4.6 (TDD execution, per established project pattern)

## Source documents (required reading for the executor)

1. **The principles** — [`docs/src/architecture/gui-frontend-design-principles-2026-06-14.md`](../../src/architecture/gui-frontend-design-principles-2026-06-14.md) — 360 numbered principles across 9 sections. `[V]` = primary-source-verified.
2. **The surface map** — [`docs/src/architecture/vox-gui-surface-map-2026-06-14.md`](../../src/architecture/vox-gui-surface-map-2026-06-14.md) — complete edge map: 2,176 nodes / 4,154 edges / 229 communities (75 named). Interactive graph at `crates/vox-gui/graphify-out/graph.html`.
3. **This spec** — the bridge: how to apply (1) to the surfaces in (2).

## Goal

Apply the full body of GUI/front-end design principles to **every** surface of vox-gui, grounded in the verified codebase state, via a foundations layer plus an exhaustive per-surface pass. Output: a sequence of TDD implementation plans Sonnet 4.6 can execute in resumable waves.

## Scope decision

**Exhaustive — all surfaces.** Foundations + a full pass over all ~30 surfaces. Packaged as **Approach A**: one spec (this doc) → one *foundations* plan → per-*wave* surface plans (grouped by the graph's communities), each sized for a single execution session. The repeatable per-surface checklist guarantees nothing is skipped.

## Verified baseline (audit run against the actual code, 2026-06-14)

| # | Check | Finding | Verdict |
|---|-------|---------|---------|
| 1 | CSP | `tauri.conf.json` → `"csp": null` | 🔴 No XSS mitigation (#330/#358) |
| 2 | IPC discipline | `VoxTransport` (`ui/src/transport.ts`) is the intended hub, but ~12 surfaces call `invoke()` directly (App, Browser, Gamify, Loquela, Matrix, Memory, Models, Search, Settings, Tasks, CommandPalette, DockShell) + hooks/consoleBridge | 🟡 Split — hub bypassed |
| 3 | Design tokens | `styles/tokens.ts` = 15 lines (badge-class maps only); 33 raw hex + 53 inline `style={{}}` in components; Tailwind used ad-hoc | 🔴 No token SSOT (#247–255) |
| 4 | Empty/loading/error | `EmptyState` imported in 4/47 surface files; 27 have some loading state | 🟡 Partial wiring (#1.1, #3.5) |
| 5 | Accessibility | 25 `aria-`, 5 `role=`, 8 keyboard handlers, **0** reduced-motion — but 177 `<button>` and **0** onClick-on-div | 🟡 Thin, native-control foundation sound (#195 ✅) |
| 6 | Async/state layer | No zustand/redux/react-query; only `App.tsx` uses `useReducer` — hand-rolled | 🟡 No query/cache layer (#277–285) |
| 7 | Module cohesion | Browser/Models/Scientia-Review/Gamify command modules at ~0.08 | 🟡 Split candidates |

**Stack:** React 19, TypeScript 5, Vite 6, Tailwind 3.4, dockview 6.6, @xyflow/react, @xterm, lucide-react, clsx + tailwind-merge. **Test baseline: 37 vitest files + Playwright e2e** (browser, dashboard, dock-layout, screenshots) → plan is genuinely TDD-capable.

## Architecture decisions (best-in-class libraries)

| Area | Decision | Principles |
|------|----------|-----------|
| **Tokens** | Introduce a real token pipeline: **Style Dictionary** (W3C design-tokens format) → emits CSS variables + TS constants + Tailwind theme extension. Three layers: primitive → semantic → component. Light/dark/high-contrast themes by swapping semantic values. Palette contrast-validated. | #247–255, #99–110, #178–183 |
| **IPC/state** | Adopt **TanStack Query** for ALL server/IPC state. Every Rust call goes through `VoxTransport`, wrapped in query/mutation hooks. Eliminate all direct `invoke()` in surfaces. Generate shared TS types from Rust at the command boundary (ts-rs/specta) where feasible. | #277–285, #317–329, #269 |
| **a11y primitives** | Adopt a **headless-a11y primitive lib (Radix or Ark)** under the existing `Glass`/`Panel` shells. Build accessible base controls (Button, Dialog, Menu, Tabs, Select, Tooltip) once so all surfaces inherit focus mgmt + ARIA + keyboard. Add global `focus-visible` + `prefers-reduced-motion`. | #195–211, #256–268 |
| **CSP** | Set a strict CSP in `tauri.conf.json` (currently `null`). Tauri auto-injects nonces/hashes for bundled assets; configure only app-specific sources. No `unsafe-inline`/`unsafe-eval`. | #330–335 |
| **Shared state components** | Standardize `EmptyState`, loading-skeleton, and error-boundary primitives + a Query-bound `<Async>` wrapper that renders idle/loading/empty/error/success uniformly. | #163–168, #136, #47–52 |
| **Type/spacing scale** | Codify type scale + 4px spacing scale as tokens; extend Tailwind theme; retire ad-hoc sizes/hex. | #74–98 |
| **Perf** | Provide a list-virtualization utility; document the RAIL budgets (100/50/10/50 ms); convention: heavy work → Rust commands, animate only `transform`/`opacity`. | #212–227 |
| **Cohesion (optional/deferred)** | Splitting the 0.08-cohesion Rust aggregator modules (browser/models/scientia-review/gamify) is flagged but **deferred** — maintainability, not principle conformance. | map §cohesion |

## The canonical per-surface checklist

Applied to **every** surface in every wave. Each item cites its principle(s). The executor audits the surface against this list (first step of each surface), then fixes gaps tests-first.

1. **IPC** — all data access via `VoxTransport` (no direct `invoke`); typed results; args validated in the Rust command. (#317–322)
2. **Server state** — via TanStack Query hook (cache/dedup/stale-while-revalidate); client UI state local. (#277–285)
3. **Async states** — explicit idle/loading/empty/error/success through the shared `<Async>` wrapper; input acknowledged ≤100 ms. (#1.1, #212, #228)
4. **Loading** — skeleton for content-shaped loads, spinner only for short indeterminate waits; no layout shift. (#136, #229–234)
5. **Empty** — deliberate `EmptyState`; distinguishes new/filtered/error; offers a primary action. (#163–168)
6. **Error** — actionable (retry/reconnect); preserves input; plain language; color + icon + text. (#47–52, #167)
7. **Visual hierarchy** — exactly one primary action per view; scale/weight/contrast rank elements. (#65–73)
8. **Tokens** — all color/space/type from semantic tokens; zero raw hex, zero ad-hoc inline styles. (#247–255, #88, #99)
9. **Typography & spacing** — type scale + spacing scale snapped; measure/line-height sane. (#74–98)
10. **Primitives** — uses `Glass`/`Panel`/`Pill`/`StateChip`/`Icon`/`EmptyState`/`Toasts` + Radix base controls; no hand-rolled containers/buttons. (#256–268)
11. **a11y controls** — native/Radix controls; every input labeled; icon-only buttons named. (#195–203)
12. **a11y keyboard** — full keyboard operability; visible focus; logical tab order; Esc closes; standard widget keys. (#187–194)
13. **a11y ARIA** — `aria-live` for async announcements; state synced (`aria-expanded`/`selected`); landmarks; heading hierarchy. (#200–205)
14. **a11y contrast** *(manual)* — text 4.5:1, large 3:1, UI components & focus ring 3:1, in every theme. (#178–183)
15. **a11y motion** — respects `prefers-reduced-motion`. (#171, #207, #224)
16. **a11y zoom/target** — usable at 200% zoom/reflow; targets ≥24 px. (#208, #211)
17. **Forms** *(if present)* — labels above; no placeholder-as-label; inline validation; preserves input; correct input controls. (#140–153)
18. **Navigation** — current location obvious; reversible; predictable back. (#154–162)
19. **Feedback/microinteractions** — immediate press feedback; optimistic UI where apt; debounce high-frequency input. (#132–138, #169–172)
20. **Performance** — no main-thread block (heavy → Rust/worker); virtualize long lists; animate `transform`/`opacity` only. (#212–227)
21. **Consistency** — terminology/iconography/color meaning consistent; platform shortcuts honored. (#20–24, #302–308)
22. **Minimalism** — progressive disclosure; remove noise. (#42–46)
23. **Error prevention** — confirm destructive actions; disable invalid options; sensible defaults. (#25–31)
24. **Tests (TDD)** — vitest unit for extracted logic, Playwright e2e for the surface, written first. (existing tooling)

## Verification (standard TDD, existing tooling)

- **Tests-first** for every change: vitest unit (logic/helpers), Playwright e2e (surface behavior), `tsc` strict.
- Existing `vox ci` / repo gates must pass.
- **a11y and contrast verified manually** against the checklist (no new axe/visual-regression/contrast automation added). Radix/Ark provides much of the a11y for free.
- No step is "done" until its tests pass and `tsc` is clean.

## Packaging & wave ordering

One **foundations plan** first, then **surface waves** as separate plan docs (each one Sonnet session). Surfaces grouped by the graph's communities so related code lands together and dependencies flow foundations → pilots → clusters.

- **Phase 0 — Foundations plan:** Style Dictionary token pipeline + theme; CSP; route all `invoke` through `VoxTransport` + typed boundary; TanStack Query integration + `<Async>` wrapper; Radix/Ark base controls under Glass/Panel; shared EmptyState/skeleton/error primitives; type/spacing scale; list-virtualization util; global focus-visible + reduced-motion. *(Cohesion split deferred.)*
- **Wave 1 — Pilots:** App Shell, Dashboard, Settings — exercise every checklist item end-to-end and validate the foundations before scaling.
- **Wave 2 — Console cluster:** Console, AgentStrip, AgentTab, InputEditor, TerminalTab, DiscoveryRail, OSC633, A2A.
- **Wave 3 — Chat cluster:** ChatSurface, Approvals, chat correlation/reducer/session store.
- **Wave 4 — Scientia cluster:** ScientiaDashboard, Claims, DiscoveryReview/Inbox, Archive, CostRollup, Novelty, PipelineTimeline, Research, Publications.
- **Wave 5 — Config/Ops cluster:** Policies, PriorityChainEditor, Budget, Coverage, Memory, Mesh, Models, RepositoryIsolation.
- **Wave 6 — Remaining surfaces:** Browser, Gamify, Loquela, SkillsPlugins, Search, Tasks, Catalog, Matrix, Flow, Runs.

writing-plans produces Phase 0 now; later waves are generated from the same checklist as capacity allows.

## Risks & open items

- **Size:** even packaged in waves, this is a large body of work; each wave plan must stay within one session. The foundations plan is the riskiest (touches every surface's call sites for the `invoke`→Query migration) — it may itself need sub-phasing.
- **Type generation** (ts-rs/specta) may not be wired yet; if absent, Phase 0 either adds it or falls back to hand-maintained boundary types (decide during writing-plans).
- **Radix vs Ark** final choice deferred to Phase 0 (both headless+accessible; pick during foundations).
- **dockview/xterm/@xyflow** are third-party surfaces with their own DOM — a11y/token conformance there is best-effort, not full control.
- **Branch:** this spec is committed on `claude/semantic-coverage-wave1f` (unrelated work); consider moving GUI work to its own branch before execution.
