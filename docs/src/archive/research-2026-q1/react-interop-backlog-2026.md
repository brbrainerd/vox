---
title: "React interop migration backlog (2026)"
description: "Granular checklist derived from the mega-plan workstreams (WS01–WS26). Use with react-interop-migration-charter-2026.md."
category: "Architecture SSOTs"
last_updated: "2026-04-08"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# React interop backlog (2026)

This file tracks **expandable** workstream tasks (T001–T260). The authoritative wave order is in [react-interop-migration-charter-2026.md](./react-interop-migration-charter-2026.md) and the Cursor plan `react-interop-full-repo-migration-2026`.

## How to use

- Agents: pick the lowest incomplete **WSxx** row; complete **all** T tasks in that row before moving on.
- Humans: use this as a merge checklist; link PRs next to completed rows.

## WS01–WS10 (routing + client + scaffold)

| WS | Range | Theme |
|----|-------|--------|
| WS01 | T001–T010 | Governance / charter / risk register |
| WS02 | T011–T020 | Parser: routes `with`, nesting, `not_found` / `error` |
| WS03 | T021–T030 | Typecheck: loader/pending resolution, duplicate paths |
| WS04 | T031–T040 | HIR: de-deprecation, ownership map |
| WS05 | T041–T050 | `route_manifest.rs` core |
| WS06 | T051–T060 | Manifest interop helpers / adapters |
| WS07 | T061–T070 | `vox-client.ts` emitter |
| WS08 | T071–T080 | Remove TanStack tree + `serverFns` |
| WS09 | T081–T090 | Scaffold emitter (one-time files) |
| WS10 | T091–T100 | SPA + SSR adapter templates |

(Full T001–T260 table lives in the accepted Cursor plan artifact; this doc is the **repo-local index** so links from the implementation plan resolve.)

## WS11–WS26

| WS | Range | Theme |
|----|-------|--------|
| WS11 | T101–T110 | Islands / hydration contracts |
| WS12 | T111–T120 | v0 / shadcn doctor + compatibility |
| WS13 | T121–T130 | Tailwind v4 scaffold |
| WS14 | T131–T140 | CLI build/run/bundle |
| WS15 | T141–T150 | Axum static + SPA fallback |
| WS16 | T151–T160 | WebIR parity / single emitter |
| WS17 | T161–T170 | Contracts / registries |
| WS18 | T171–T180 | Golden tests |
| WS19 | T181–T190 | CI jobs |
| WS20 | T191–T200 | Docs / education |
| WS21 | T201–T210 | `vox-vscode` |
| WS22 | T211–T220 | `tools/visualizer` |
| WS23 | T221–T230 | `tree-sitter-vox` |
| WS24 | T231–T240 | `vox migrate` tooling |
| WS25 | T241–T250 | Perf / telemetry |
| WS26 | T251–T260 | Cutover / delete legacy |

## Done in repo (update as you land work)

- [x] Charter + backlog stubs linked from architecture index
- [x] `routes.manifest.ts` default emission (`routes { }` → manifest emitter)
- [x] `vox-client.ts` default emission (POST JSON parity with Axum handlers)
- [x] Removal of `App.tsx` / `VoxTanStackRouter.tsx` / `serverFns.ts` from compiler codegen; TanStack Start scaffold uses file routes + `routes.manifest.ts` only
- [x] Optional scaffold via `VOX_WEB_EMIT_SCAFFOLD` + `codegen_ts::scaffold`
- [x] Lexer: `#` line comments (fixture / shell style)
- [x] Parser: `@v0 from "asset.png"` image hint form + `V0ComponentDecl.image_path`
- [x] Typecheck: retired `context` / `@hook` / `@provider` / `Page` → **Error**; `@component fn` → **parse error** by default; escape hatch `VOX_ALLOW_LEGACY_COMPONENT_FN=1` for transitional sources
- [x] Docs: `VOX_WEB_*` env registry rows; `docs/src/adr/README.md` for CI gate paths; `vox-codegen-ts.md` cross-links
- [x] `vox migrate web` — scan `.vox` sources and report migration lint codes (`lint.legacy_*`, `lint.retired_*`) + JSON output
- [x] `vox doctor` — pnpm/node + optional `components.json` `rsc:false` check (v0/shadcn client interop)
- [x] WebIR `WebIrLowerSummary` — route manifest parity counters (loaders, pending, `not_found` / `error` blocks)
- [x] Removed dead `tanstack_programmatic_routes.rs` emitter module
- [ ] **WebIR consolidation (platform)**
  - **Single-emitter default:** retire or gate parallel JSX / `hir_emit` paths per [internal-web-ir-implementation-blueprint.md](./internal-web-ir-implementation-blueprint.md) acceptance gates — reduces drift between “legacy emit” and WebIR-validated manifests.
  - **Autofix migrations + CI hybrid matrix:** follow blueprint §CI / autofix notes when flipping the default emitter (keeps golden + integration matrix green).
  - **tree-sitter-vox `routes` grammar:** extend [`tree-sitter-vox/`](../../../tree-sitter-vox/) ([`grammar.js`](../../../tree-sitter-vox/grammar.js)) so editor + corpus parsers match [`tail.rs`](../../../crates/vox-compiler/src/parser/descent/decl/tail.rs) surface (`with loader:`, nested `routes`, `not_found:` / `error:`).


