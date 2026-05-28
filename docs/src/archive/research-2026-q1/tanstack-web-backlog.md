---
title: "TanStack web backlog"
description: "Official documentation for TanStack web backlog for the Vox language. Detailed technical reference, architecture guides, and implementati"
category: "Language Reference"
last_updated: "2026-04-08"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# TanStack web backlog

Decompose epics into actionable tasks. Check off as you complete; prefer **issues/PRs** for assignment, this file as **SSOT mirror**.

## Phase 0 — Hygiene

- [x] Narrative: non-product UI paths described in SSOT/ADR without legacy stack names
- [x] Remove or rewrite **`vox-codegen-html`** references (Cargo exclude comment, forward-migration charter, Ludus quests, CodeRabbit planner allowlist)
- [x] Link ADR 010 + this roadmap from **AGENTS.md** (optional one-liner)

## Phase 1 — Examples

- [x] Create `examples/archive/` and move non-golden `.vox` files
- [x] Update `crates/vox-parser/tests/parity_test.rs` `MUST_PARSE` (recursive walk)
- [x] Document golden list in `examples/README.md`
- [x] `examples/STYLE.md` + `FEATURE_INDEX.md` + `PARSE_STATUS.md`; optional **`VOX_EXAMPLES_STRICT_PARSE=1`** in `parity_test`

## Phase 2 — TanStack Router

- [x] Emit `createRootRoute` / `createRoute` / `createRouter` / `RouterProvider` from `routes {` ([`vox-codegen-ts/src/emitter.rs`](../../../crates/vox-codegen/src/codegen_ts/emitter.rs))
- [x] Add `@tanstack/react-router` to [`templates.rs`](../../../crates/vox-cli/src/templates/tanstack.rs) `package_json`; drop unused router dep from **`islands`** `package.json` template
- [x] Prefer **`App`** entry in [`fs_utils::find_component_name`](../../../crates/vox-cli/src/fs_utils.rs) when `App.tsx` exists
- [x] Integration tests: `routes {` codegen assertions ([`pipeline.rs`](../../../crates/vox-integration-tests/tests/pipeline.rs))

## Phase 3 — pnpm workspace

- [x] Emit root **`pnpm-workspace.yaml`** when `islands/` + main app paths are known ([`frontend.rs`](../../../crates/vox-cli/src/frontend.rs))
- [x] Document **root** `pnpm install` / `pnpm -r build` in [ref-cli.md](../reference/cli.md)
- [x] Align **islands** workspace paths: resolve **`islands/`** or **`packages/islands/`** (`island_package_root`, `pnpm-workspace.yaml`, `build_islands_if_present`)

## Phase 4 — TanStack Start + SSR

- [x] Scaffold Start-compatible **`vite.config`** / entry ([`templates.rs`](../../../crates/vox-cli/src/templates/tanstack.rs) `vite_config(..., tanstack_start: true)` + [`frontend.rs`](../../../crates/vox-cli/src/frontend.rs))
- [x] **`routes {` + Start**: manifest-first — codegen **`routes.manifest.ts`** + components + **`vox-client.ts`**; user-owned TanStack adapter + file routes + **`routeTree.gen.ts`** ([`emitter.rs`](../../../crates/vox-codegen/src/codegen_ts/emitter.rs), [`route_manifest.rs`](../../../crates/vox-codegen/src/codegen_ts/route_manifest.rs), CLI [`tanstack.rs`](../../../crates/vox-cli/src/templates/tanstack.rs) scaffold)
- [x] Regenerate **file-route** `routeTree.gen.ts` via **TanStack Router CLI** (`pnpm run routes:gen` / `tsr generate`) for the no-`routes {` path — **`pnpm install` / build** scripts run it when not using programmatic `voxRouteTree`
- [x] **`vox run`**: optional Vite upstream via **`VOX_ORCHESTRATE_VITE=1`** + **`VOX_SSR_DEV_URL`** (see how-to)
- [x] Generated Axum **`serve_dispatch`**: GET non-`/api` proxy to **`VOX_SSR_DEV_URL`** when set
- [x] Production **Docker** sketch — see [TanStack SSR with Axum](../how-to/tanstack-ssr-with-axum.md#production-docker-sketch) (multi-stage Node build + Rust binary; adjust paths to your crate/binary name)
- [x] **CI**: `pnpm install` + `vite build` on **`web-vite-build-smoke`** (`ubuntu-latest` exception) with **`examples/full_stack_minimal.vox`** (opt-in local: `VOX_WEB_VITE_SMOKE=1`)

## Phase 5 — Query / Table (optional)

- [x] **`@loading`**: lexer/parser → `Decl::Loading` → `Spinner.tsx` + TanStack Router **`pendingComponent`** via manifest / component wiring ([`route_manifest.rs`](../../../crates/vox-codegen/src/codegen_ts/route_manifest.rs), [`emitter.rs`](../../../crates/vox-codegen/src/codegen_ts/emitter.rs))
- [x] **TanStack Query helper emitted:** [`vox-tanstack-query.tsx`](../../../crates/vox-codegen/src/codegen_ts/tanstack_query_emit.rs) (via `emitter.rs`) defines **`useVoxServerQuery`** — import from generated output next to `vox-client.ts`.
- [ ] **Optional enhancement:** Auto-wrap **`useVoxServerQuery`** inside **Path C reactive components** that consume `@query` data (not inside `routes.manifest.ts` **loaders**, which must remain plain `async` functions — React hooks are invalid there). Until then, authors call `useVoxServerQuery(['key'], () => myQuery({...}))` in components. Legacy **`serverFns.ts` / Wave F** tasks in [`tanstack-start-implementation-backlog.md`](./tanstack-start-implementation-backlog.md) are superseded by **`vox-client.ts`**.
- [x] Table-heavy UIs: **TanStack Table** — prefer for sort/filter/column-heavy grids when staying in React; hand-rolled `<table>` or lightweight lists remain fine for simple cases (see [vox-web-stack.md](../reference/vox-web-stack.md#data-grids-tanstack-table))

## Phase 6 — v0

- [x] **`vox build`** validates each present `{Name}.tsx` for `@v0` against the **named export** contract; **`cargo test -p vox-cli v0_tsx_normalize`** covers matchers; optional **`vox doctor`** check when **`VOX_WEB_TS_OUT`** points at the TS output dir
- [x] Docs: [@v0](../reference/ref-decorators.md) links **v0.dev**, **named exports**, **islands** / `vox island`, and **doctor** env

## Phase 7 — Virtual File Routes + Complete TanStack Start

Full checklist (with **truth table**): [tanstack-start-implementation-backlog.md](./tanstack-start-implementation-backlog.md)  
Spec / historical fate table: [tanstack-start-codegen-spec.md](./tanstack-start-codegen-spec.md) — **treat virtual-file-route emit as historical**; shipped model is **manifest + adapter**.

- [x] **Wave A — obviated / done in tree:** Loader + pending + `not_found` / `error` + nested `routes` (field names: `loader_name`, `pending_component_name`). **Deferred:** `under` / `layout_name` on `RouteEntry`; `redirect` / wildcard parsing.
- **Partial — Wave B:** Open [`hir/nodes/decl.rs`](../../../crates/vox-compiler/src/hir/nodes/decl.rs) before executing backlog B-items; some deprecation noise intentionally remains for migration paths.
- **Partial — Wave C:** Classic `@component fn` and retired surfaces are **`Error`** (see typeck / parser); emitter loops may still exist for migration — verify tree, do not assume checklist is greenfield.
- [x] **Wave D — obviated (shape):** Scaffold files: **`vox-cli`** templates + optional [`codegen_ts/scaffold.rs`](../../../crates/vox-codegen/src/codegen_ts/scaffold.rs); not the spec’s exclusive Start-only `client.tsx` / `router.tsx` trio from compiler alone.
- [x] **Wave E — cancelled:** Compiler `__root.tsx` / `app/routes.ts` virtual program — replaced by **`routes.manifest.ts`** + file routes + optional manifest adapter.
- [x] **Wave F:** **`vox-client.ts`** + Axum (GET `@query`, POST mutation/server). Residual ergonomics: docs / env constants — non-blocking.
- [ ] **Wave G:** Docs drift vs **manifest-first** spec (roadmap, decorator pages, how-tos) — ongoing editorial.
- [x] **Wave H:** [`web_routing_fullstack.vox`](../../../examples/golden/web_routing_fullstack.vox), [`blog_fullstack.vox`](../../../examples/golden/blog_fullstack.vox), [`v0_shadcn_island.vox`](../../../examples/golden/v0_shadcn_island.vox) + pipeline tests. `layout_groups.vox` **blocked** until layout/redirect grammar unless expressed as nested paths only.
- **Partial — Wave I:** **No** virtual route snapshots; instead `web_ir_lower_emit`, `include_01` pipeline, `axum_emit_contract`. Add tests only if new grammar ships.
- **Partial — Wave J:** [`tanstack.rs`](../../../crates/vox-cli/src/templates/tanstack.rs), [`spa.rs`](../../../crates/vox-cli/src/templates/spa.rs), [`frontend.rs`](../../../crates/vox-cli/src/frontend.rs) are live; revisit when `vox init --web` changes.
- [ ] **Wave K:** ADR 010 / **architecture-index** links — spot-check when touching web ADRs.



