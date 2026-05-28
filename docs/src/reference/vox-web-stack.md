---
title: "Vox full-stack web UI — single source of truth"
description: "Official documentation for Vox full-stack web UI — single source of truth for the Vox language. Detailed technical reference, architectur"
category: "Language Reference"
last_updated: "2026-04-07"
training_eligible: true

schema_type: "TechArticle"
---

# Vox full-stack web UI — single source of truth

> [!NOTE]
> **Path C (implemented):** reactive UI uses `component Name(...) { state ... view: ... }`. The `@island` decorator was retired 2026-05-03 — see [architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md). Suppress legacy-hook warnings in fixtures with **`VOX_SUPPRESS_LEGACY_HOOK_LINTS=1`** ([`env-vars.md`](env-vars.md)).

## Language boundary

- **`.vox` source** uses **only Vox syntax** (including Vox JSX-like UI). Do not embed TypeScript or JavaScript in `.vox` files.
- **TypeScript and React** appear only in **generated artifacts** (`dist/`, `app/src/generated/`) and **pnpm scaffolds** under `crates/vox-cli` templates. External React frontends import these generated components or call the generated `vox-client.ts` directly — see [architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).

## Shipped stack

| Layer | Role |
| ----- | ---- |
| `vox-compiler` / `codegen_ts` | `component`, `routes {`, tables, activities → `.tsx` / `.ts` |
| `vox-compiler` / `codegen_rust` | `http`, server fns, actors → Axum + `rust_embed` of `public/` |
| Vite + React 19 | Main app under `dist/app` (scaffolded by `vox run` / `vox bundle`) |
| `@tanstack/react-router` | Client routing for `routes {` (see [ADR 010](../adr/010-tanstack-web-spine.md)) |
| **v0.dev** | `V0_API_KEY`; TSX normalized to **named** `export function Name` for `routes {` imports |

## Canonical Frontend

> [!IMPORTANT]
> **`vox-dashboard` is the Single Source of Truth** for the Vox user-facing frontend experience (see [ADR 030](../adr/030-state-machine-ssot.md) and [ADR 031](../adr/031-deprecate-vox-vscode.md)).
> `apps/editor/vox-vscode/` is **deprecated** and retained only for its LSP client. Ship new MCP behavior, capability UX, and visualization in `crates/vox-dashboard/` — not in the VS Code extension.

The **orchestration dashboard** (`crates/vox-dashboard/`) is the primary Vox user surface. It is served by the Axum backend (`vox dashboard` command) and communicates with the orchestrator over a local MCP WebSocket proxy. All reactive UI state within the dashboard uses the Vox `state_machine` compiler primitive as the single source of truth (see below).

Frontend lifecycle ownership (canonical vs experimental vs fixture-only) is tracked in [`frontend-surface-ownership.md`](frontend-surface-ownership.md) and the machine-readable contract [`contracts/frontend/surface-ownership.v1.yaml`](../../../contracts/frontend/surface-ownership.v1.yaml).

- **Dashboard entry point:** `crates/vox-dashboard/app/src/app.vox` — lowered to `app/src/generated/` by `vox build`
- **Backend:** `crates/vox-dashboard/src/` — Axum routes, MCP proxy, settings API
- **Unified Grammar**: Vocabulary is synchronized via **`tree-sitter-vox/GRAMMAR_SSOT.md`**.
- **Retired**: Legacy `frontend/` (Next.js), `packages/vox-ui/`, and VS Code as primary surface have been removed/deprecated.

## `state_machine` as SSoT for reactive UI state

Within the dashboard (and any Vox-generated application), reactive state **must** be expressed using the Vox `state_machine` compiler primitive. This primitive is defined in `crates/vox-compiler/src/hir/nodes/state_machine.rs`, type-checked at `crates/vox-compiler/src/typeck/state_machine_check.rs`, and lowered to TypeScript+React by `crates/vox-codegen/src/codegen_ts/state_machine_emit.rs`.

Do **not** hand-write reactive `.tsx` files in `app/src/generated/` — they must be compiler outputs from `.vox` sources. The CI gate at `scripts/check_dashboard_ssot.vox` enforces this rule.

Hand-written React source for the dashboard's escape-hatch components is explicitly allowed under `src/components/`. Only the `app/src/generated/` subtree is compiler-owned.

## Not part of Vox

Vox does **not** ship HTML-fragment UIs or classless CSS microframeworks as first-class product paths. Use **React + Vite + Tailwind/ShadCN + TanStack Router** (→ TanStack Start per [ADR 010](../adr/010-tanstack-web-spine.md)) for all interactive web UI.

## Typed web API client and HTTP verbs

- **`vox-client.ts`** is emitted when the module has any `@endpoint` declarations.
- **`@endpoint(kind: query)`** uses **`GET`** against `/api/query/<name>` with **deterministic JSON-in-query** encoding (sorted keys; each argument value is JSON-serialized then URL-encoded). This matches the generated Axum handlers.
- **`@endpoint(kind: mutation)`** and **`@endpoint(kind: server)`** use **`POST`** with a JSON body — same shapes as Axum.

Normative detail: [vox-codegen-ts.md](../reference/cli.md) (transport section) and [vox-fullstack-artifacts.md](vox-fullstack-artifacts.md).

## TanStack Start vs manifest-driven SPA

- **Vite SPA scaffold (default):** when `routes.manifest.ts` is present, the scaffold writes **`vox-manifest-router.tsx`** + **`vox-manifest-route-adapter.tsx`** and drives the router from **`voxRoutes`** ([`spa.rs`](../../../crates/vox-cli/src/templates/spa.rs), [`frontend.rs`](../../../crates/vox-cli/src/frontend.rs)).
- **TanStack Start (opt-in):** the scaffold still seeds **file-based** `src/routes/*` and **`routeTree.gen.ts`**. If the compiler emitted **`routes.manifest.ts`**, the scaffold also adds **`vox-manifest-route-adapter.tsx`** as a **shared helper** you can merge into a programmatic router — it does **not** replace the default file-route `router.tsx` automatically.

## Mobile browser baseline

For mobile support, this web stack is the primary delivery surface for Vox applications.

- Generated app shells must emit a viewport meta tag and mobile-safe root layout defaults.
- Templates should keep touch ergonomics sane by default (tap-target sizing and responsive spacing in base CSS).
- Mobile support here means browser compatibility for generated Vox apps, not running the full Vox CLI/runtime on-device.
- Keep framework/runtime internals behind WebIR/AppContract/RuntimeProjection boundaries when extending mobile behavior.

## External references (ecosystem)

- [TanStack Router + Vite](https://tanstack.com/router/latest/docs/installation/with-vite)
- [TanStack Start (React)](https://tanstack.com/start/latest/docs/framework/react/overview)

## Implementation touchpoints

- Templates: `crates/vox-cli/src/templates/` (`spa.rs`, `tanstack.rs`; `package.json`, Vite config).
- Frontend build: `crates/vox-cli/src/frontend.rs`.
- v0: `crates/vox-cli/src/v0.rs`, `crates/vox-cli/src/v0_tsx_normalize.rs`.
- React hook mapping / `component` emission: `crates/vox-codegen/src/codegen_ts/component.rs` (imports [`react_bridge`](../../../crates/vox-compiler/src/react_bridge.rs): Vox `use_*` → React hooks, shared AST walks). Path C reactive: `crates/vox-codegen/src/codegen_ts/reactive.rs`, `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs`. Server-fn API path prefix: [`web_prefixes::SERVER_FN_API_PREFIX`](../../../crates/vox-compiler/src/web_prefixes.rs) (HIR + TS fetch URLs stay aligned). Route manifest + typed client: [`codegen_ts/route_manifest.rs`](../../../crates/vox-codegen/src/codegen_ts/route_manifest.rs), [`codegen_ts/vox_client.rs`](../../../crates/vox-codegen/src/codegen_ts/vox_client.rs); Start file layout glue lives in [`codegen_ts/scaffold.rs`](../../../crates/vox-codegen/src/codegen_ts/scaffold.rs) and CLI templates (`tanstack.rs`). Opt-out for legacy-hook warnings: env **`VOX_SUPPRESS_LEGACY_HOOK_LINTS`** ([`env-vars.md`](env-vars.md)).
- **`vox run` auto mode**: `crates/vox-cli/src/commands/run.rs` + `commands/runtime/run/run.rs` — default is an `@page` scan in the first 8 KiB; override with **`[web] run_mode`** in `Vox.toml` (`auto` \| `app` \| `script`) or env **`VOX_WEB_RUN_MODE`** (same values; parsed in `vox-config`).
- **TanStack Start scaffold (opt-in)**: `Vox.toml` **`[web] tanstack_start = true`** or **`VOX_WEB_TANSTACK_START=1`** — `crates/vox-cli/src/templates.rs` + `frontend.rs` emit Start file layout + `@tanstack/react-start` (see [vox-fullstack-artifacts.md](vox-fullstack-artifacts.md)).
- **External frontend interop (2026)**: `component` declarations lower to plain TSX under the generated `app/` directory; an external React/TanStack/mobile app imports them or calls the endpoints declared in `.vox` via the generated `vox-client.ts`. SSG HTML shells still come from **`vox-ssg`** + `routes {`. Full plan: [architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).

**Web IR gate matrix (OP-S068, OP-S129, OP-S152, OP-S209):** parity and validate thresholds are enumerated under [acceptance gates G1–G6](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) with tests in `web_ir_lower_emit.rs`, `reactive_smoke.rs`, `pipeline.rs`, and `full_stack_minimal_build.rs`.

## Data grids (TanStack Table)

For **dense, interactive tables** (sorting, filtering, column visibility, virtualization), **[@tanstack/react-table](https://tanstack.com/table/latest)** is the usual fit: headless hooks compose with your design system (e.g. ShadCN data-table patterns). **Hand-rolled** `<table>` markup or simple mapped lists stay appropriate when you do not need those features—avoid pulling Table only for static layouts.

## Roadmap

- [TanStack web roadmap](../archive/research-2026-q1/tanstack-web-roadmap.md) — phases Router → Start, SSR, workspace merge.
- [TanStack web backlog](../archive/research-2026-q1/tanstack-web-backlog.md) — checkbox task decomposition.
- [ADR 010 — TanStack web spine](../adr/010-tanstack-web-spine.md) — decisions (topology, examples, v0, `vox-codegen-html` retirement).
- [ADR 012 — Internal web IR strategy](../adr/012-internal-web-ir-strategy.md) — ranked trade-offs and migration plan for compiler-owned frontend IR while keeping React ecosystem interop.
- [Internal Web IR implementation blueprint](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) — weighted execution plan and staged task quotas for compiler migration.
- [WebIR operations catalog (OP-0001..OP-0320)](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) — ordered, file-by-file operation map with complexity/test/token budgets.
- [Internal Web IR side-by-side schema](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) — parser-grounded current-vs-target full-stack representation mapping.
- [WebIR K-complexity quantification](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) — token+grammar+escape-hatch delta for the canonical worked app.
- [WebIR K-metric appendix](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) — reproducible class registries, worked counts, and equation trace.

## Examples (canonical `.vox` shape)

- [`examples/STYLE.md`](../../../examples/STYLE.md) — target formatting for golden examples (LLM + human).
- [`examples/PARSE_STATUS.md`](../../../examples/PARSE_STATUS.md) — golden vs optional strict parse (`VOX_EXAMPLES_STRICT_PARSE`).

## Related docs

- [vox-codegen-ts.md](../reference/cli.md) — `routes.manifest.ts`, `vox-client.ts` transport (**GET** `@endpoint(kind: query)` / **POST** mutations).
- [vox-fullstack-artifacts.md](vox-fullstack-artifacts.md) — build outputs, Express `server.ts` opt-in, containers.
- [`cli.md`](cli.md) — CLI including `vox populi` (feature `populi`).
- [TanStack SSR with Axum](../how-to/tanstack-ssr-with-axum.md) — dev topology during SSR adoption.
- [Mens SSOT](populi.md) — worker/runtime mens registry and HTTP control plane; not emitted by `vox-codegen-*` (operator env only).
- [`AGENTS.md`](../../../AGENTS.md) — architecture index.

