---
title: "Frontend Convergence Findings (2026)"
description: "Audit of Vox's TypeScript/JSX/React emit, GUI primitive surface, parser/HIR lowering, and two-way interop seeds. Identifies dead surfaces to retire, the canonical SSOT (HIR → Web IR → emitters), and the missing Contract IR layer that unifies wire-format outputs. Companion to the External Frontend Interop Plan (2026)."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Canonical convergence reference; defines the SSOT seam and the redundancies to delete on the path to Phase 2/5 of the interop plan."
---

# Frontend Convergence Findings (2026)

**Audit date:** 2026-05-08
**Companion to:** [External Frontend Interop Plan (2026)](external-frontend-interop-plan-2026.md), [GUI-Native Roadmap Status (2026)](gui-native-roadmap-status-2026.md)

## Convergence decisions (2026-05-11 pilot)

- **Contract IR remains the SSOT seam** for Zod/OpenAPI and future TS client emit; do not add parallel HIR→TS shortcuts.
- **Scaffold/template churn** stays behind explicit codegen flags (`VOX_EMIT_*` / compiler gates) until R3/R5 land — prefer Contract IR snapshots over raw JSX golden churn.
- **R3 (`HirRoute`) + R5 (Express)** remain deferred per §Implementation status below; next milestone is route lowering without breaking live Axum emit.

## Implementation status (2026-05-08)

This findings doc is partially landed:

- **R1 + R2 (dead Path-A surface)** — landed in commit `dd3e0432a`. `Decl::Component`, `ComponentDecl`, `HirFn.is_component`, and the unused emit functions in `codegen_ts/component.rs` are gone. 19 files, −546 lines.
- **Contract IR module** — landed in commit `1ed467d48`. `crates/vox-compiler/src/contract_ir/` with `ContractIr`, `WireType`, projection rules, 8 unit tests.
- **Zod emit through Contract IR** — landed in commit `1ed467d48`. `codegen_ts/zod_emit.rs` now reads `ContractIr` instead of HIR directly.
- **OpenAPI 3.1 emit** — landed in commit `1ed467d48`. `codegen_ts/openapi_emit.rs` with 7 unit tests; `openapi.json` is emitted alongside `schemas.ts` and `vox-client.ts`. Phase 2 of the interop plan unblocked.
- **R3 (`HirRoute`)** — deferred. Touches the active `codegen_rust::emit::http::emit_main` (live Axum `main.rs` emitter) and an active integration test surface; warrants a focused PR.
- **R4 (deprecated `@server` / `@query` / `@mutation`)** — **landed**. The lexer tokens (`AtServer`/`AtQuery`/`AtMutation`), parser entries (`parse_server_fn`/`parse_query_fn`/`parse_mutation_fn`), AST variants (`Decl::ServerFn`/`Query`/`Mutation`), AST structs (`ServerFnDecl`/`QueryDecl`/`MutationDecl`), HIR lowering branches, fmt printer arms, and consumers in `vox-ml-cli`, `vox-corpus`, `vox-db`, `vox-lsp` are all gone. Five `.vox` fixtures migrated to `@endpoint(kind: …)`. `contracts/speech-to-code/vox_grammar_artifact.json` no longer lists the retired decorators.
- **R5 (Express server emit)** — deferred. The emitter (`codegen_ts/routes.rs::generate_routes`) is still gated off behind `VOX_EMIT_EXPRESS_SERVER`; consumer audit pending.
- **R6 (legacy `voxdb.rs`)** — **landed**. `crates/vox-codegen/src/codegen_ts/voxdb.rs` was orphaned (defined `generate_voxdb_handlers` with no callers); deleted alongside R4.
- **`@py.import` + Python lane** — **landed**. `Decl::PyImport` AST variant + `PyImportDecl` struct removed; `vox-cli/commands/container.rs` lost the `Init` action; `vox-deploy-codegen` lost `pyproject.rs`, `python_dockerfile.rs`, `setup.rs`, `env.rs` (~540 LoC); the `venv_detection_test.rs` integration test was deleted; `vox-corpus`, `vox-ml-cli`, `vox-orchestrator` references purged. Aligns with [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md): "Vox is the glue language. Python and shell are retired glue surfaces."
- **VUV-4 lowering correctness fixes** — landed alongside R4. Fixed four real bugs surfaced by the `golden_dashboard_chrome_test` snapshots: (a) `border_x/y/t/b/l/r=true|false` produced literal class strings like `"border-b-true"`; (b) `border=false` produced `"border-false"`; (c) `role={region}` (unbound identifier — invalid TS) emitted instead of `role={"region"}`; (d) `style` attribute pushed CSS string into a JSX expression slot. Affected files: [`web_ir/primitives/mod.rs`](../../../crates/vox-codegen/src/web_ir/primitives/mod.rs), [`web_ir/lower.rs`](../../../crates/vox-codegen/src/web_ir/lower.rs), [`codegen_ts/jsx.rs`](../../../crates/vox-codegen/src/codegen_ts/jsx.rs), [`codegen_ts/hir_emit/mod.rs`](../../../crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs). Snapshot suite re-baselined against the corrected emit.
- **`vox-react-runtime.ts` / `vox-api.ts` barrels** — deferred. Cosmetic K-reduction with snapshot churn cost; revisit when component snapshots regenerate for VUV-5/6.

## Premise

Vox emits TypeScript/React/JSX as its frontend output today. The strategic plan in
[external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md) commits to keeping
that emission as a first-class language feature **and** adding bidirectional component
interop with the React ecosystem. This document audits what actually exists in the tree
on 2026-05-08 against that plan, identifies redundant surfaces, names the single source
of truth, and specifies the missing convergence layer (the **Contract IR**) that
unifies wire-format outputs without touching components or routes.

The goal: reduce the K-complexity of Vox's emitted output and the surface area of the
authored Vox source, while preserving the substrate that makes both first-class —
without parallel-developing the React ecosystem.

## Method

Three parallel audits — emit pipeline, GUI primitives + native runtime, parser/HIR —
across `crates/vox-compiler/`, `crates/vox-cli/`, `crates/vox-actor-runtime/`, `examples/golden/`,
and the architecture docs. Findings cite file:line consistently. Recommendations are
in [§Convergence design](#convergence-design); deletion targets are in
[§Redundancies to retire](#redundancies-to-retire).

## State of the system today

### Frontend emit pipeline

`crates/vox-codegen/src/codegen_ts/` contains 24 modules orchestrated by
[emitter.rs](../../../crates/vox-codegen/src/codegen_ts/emitter.rs). The emit fans out
into seven distinct outputs from one HIR module:

1. **Components** — `{Name}.tsx` per `component`, optional scoped `{Name}.css`
2. **Types** — `types.ts` (ADT discriminated unions + struct aliases)
3. **Validators** — `schemas.ts` (Zod)
4. **Client SDK** — `vox-client.ts` (typed fetch wrapper) + `vox-app-contract.json`
5. **Routes** — `routes.manifest.ts` (App mode) or `.json` (Library mode)
6. **State / URLs** — `state_machines.ts`, `urls.ts`
7. **Utilities** — `vox-tanstack-query.tsx`, `schema.ts`, `fragments.tsx`,
   `{Module}Provider.tsx`, `vox-tokens.{css,ts}`

Plus one-time **scaffold** files (user-owned): `app/main.tsx`, `app/App.tsx`,
`vite.config.ts`, `package.json`, `tsconfig.json`, `app/globals.css`,
`app/components.json`.

### Two parallel component paths in the tree

| Path | Status | HIR | Codegen | Notes |
|---|---|---|---|---|
| **Path A** — legacy decorator-on-fn component syntax (retired) | **Tombstoned at parser** | `HirFn { is_component: true }` | `codegen_ts/component.rs` | Parser rejects this form; the HIR field and codegen file are dead state |
| **Path C** — `component Name() { state…; view: … }` | **Canonical** | `HirReactiveComponent` (separate `HirModule.components` vector) | `codegen_ts/reactive.rs` | Active surface; reactive members lower to React hooks |

Path C is correct. Path A's HIR flag and codegen file remain only because nothing
deleted them when the parser path was removed.

### Three routing surfaces stacked

| Surface | Status | Disposition |
|---|---|---|
| `HirRoute` (HTTP-method routes from old syntax) | **Tombstoned** | Delete the node + references |
| `HirEndpointFn` (`@endpoint(kind: query\|mutation\|server)`) | **Canonical for HTTP** | Keep |
| `routes { … }` block → Web IR `RouteNode` → `routes.manifest.ts` | **Canonical for client routing** | Keep |

The `routes` block AST currently lives in `legacy_ast_nodes` retention on `RoutesDecl`
because lowering goes AST → Web IR rather than HIR. That's an arrangement, not a
problem — but the `HirRoute` node is genuinely dead and ships in every build.

### Web IR is already the right convergence point

[web_ir/](../../../crates/vox-codegen/src/web_ir/) sits between HIR and TSX emit. It validates
routes, component primitives, and universal style kwargs (VUV-4) and is consumed by
`route_manifest.rs`, `reactive.rs`, and `jsx.rs`. This is the existing single source of
truth for the *frontend* projection of HIR. No new layer is needed for components or routes.

### Wire-format outputs are not unified

Today, three TS-side wire-format emitters each walk HIR independently from
[emitter.rs](../../../crates/vox-codegen/src/codegen_ts/emitter.rs):

- `zod_emit.rs` — Zod schemas from `HirTypeDef` + `HirTable`
- `schema/from_hir.rs` — JSON Schema from HIR types
- `vox_client.rs` — typed fetch wrapper from `HirEndpointFn`

OpenAPI emit, called for in [Phase 2](external-frontend-interop-plan-2026.md#phase-2--wire-format-ssot-and-standards-based-schema-emit)
of the interop plan and codified in [wire-format-v1-ssot.md](wire-format-v1-ssot.md), does not
exist. Adding it as a fourth direct HIR walker would compound the divergence; adding it
through a shared projection unifies them.

### Two-way interop has one working seed

`extern fn` (commit `180b3ae07`, 2026-05-08) is the foundation:

```vox
extern fn isValidEmail(s: str) to bool = "./ts_source_ffi_helper"
```

Lexer adds `Token::Extern`; parser writes `FnDecl.ts_extern_module`; HIR carries
`HirFn.ts_extern_module: Option<String>`; codegen emits an `import` line. Typeck
returns early. Golden: [examples/golden/ts_source_ffi.vox](../../../examples/golden/ts_source_ffi.vox).

There is **no** parser hook for an `import react …` form yet. That is Phase 5 work.

### "GUI native" does not mean a non-React renderer

`wgpu` is in the workspace but only for GPU compute (`vox-populi`, MENS training).
None of `egui`, `iced`, `dioxus`, `tauri`, `winit`, `tao`, `gpui`, or `slint` are
present. The
[gui-native-roadmap-status-2026.md](gui-native-roadmap-status-2026.md) title refers to
**GUI as native language features** (typed primitives, typed style kwargs, validation),
not native rendering.

### VUV is a phase plan that constrains the input surface

VUV (Vox UI as Values) is the K-complexity reduction program for the *authored* Vox
surface. VUV-4 (typed style kwargs, universal across primitives) is implemented;
VUV-2/3 (trailing-block call form), VUV-5 (typed event handlers), VUV-6 (delete the
JSX path) are in-flight.

VUV does not change emitted output K — it changes *input* K, which is correct: the
`.vox` source is what humans and LLMs author; the `.tsx` is generated and re-emit-stable.

### Reactive primitives are already minimal

`state`, `derived`, `effect`, `mount`, `cleanup`, `view:` are bare keywords that lower
to React hooks via [reactive.rs](../../../crates/vox-codegen/src/codegen_ts/reactive.rs).
No signals, no custom reactivity layer. This is the right size — adding more reactive
primitives would not help; lowering future targets (Dioxus, native) into this set would.

## Redundancies to retire

Concrete, mechanical deletions. None changes language semantics.

| # | Surface | File / Symbol | Reason |
|---|---|---|---|
| R1 | `HirFn.is_component` flag | [hir/nodes/decl.rs](../../../crates/vox-compiler/src/hir/nodes/decl.rs) (line ~262) | Tombstoned at parser; flag is dead state |
| R2 | Path-A component codegen | [codegen_ts/component.rs](../../../crates/vox-codegen/src/codegen_ts/component.rs) | Targets the tombstoned AST path |
| R3 | `HirRoute` node + lowering | [hir/nodes/decl.rs](../../../crates/vox-compiler/src/hir/nodes/decl.rs) line 176 | Replaced by `HirEndpointFn` + `routes { }` |
| R4 | Deprecated decorators | ~~`@server`, `@query`, `@mutation`~~ | **Removed** — `@endpoint(kind: …)` is canonical |
| R5 | Express server emit | [codegen_ts/routes.rs](../../../crates/vox-codegen/src/codegen_ts/routes.rs) | Gated off; `route_manifest.rs` is the active path |
| R6 | Legacy schema.ts emit | ~~`codegen_ts/voxdb.rs`~~ | **Removed** — orphan; `generate_voxdb_handlers` had no callers |

R1, R2, R4, R6 are landed. R3 is deferred (touches active Axum emit). R5
requires consumer verification before deletion.

## Necessities to keep

| # | Surface | Why it stays |
|---|---|---|
| K1 | `HirReactiveComponent` + reactive members | Canonical Path C; shape is portable to non-web targets |
| K2 | Web IR | Existing convergence point for components, routes, style kwargs |
| K3 | `extern fn` (TS-source FFI) | Foundation primitive; reusable for Phase 5 component import |
| K4 | Zod / types / client TS emitters | Necessary at the TS-runtime boundary |
| K5 | `routes { }` block + manifest | First-class language feature; user-router agnostic |
| K6 | Reactive keyword set (state/derived/effect/mount/cleanup/view) | Minimum complete primitive surface |
| K7 | VUV-4 universal style kwargs | Removes Tailwind class strings as a string-typed sub-language |
| K8 | Design token registry | Tokens are typed values, not magic strings; required for VUV-4 |

## Convergence design

### Single source of truth

```
                          ┌─ {Name}.tsx              (reactive.rs)
HIR ──► Web IR ───────────┼─ routes.manifest.ts      (route_manifest.rs)
        (frontend SSOT)   └─ vox-tokens.{css,ts}     (tokens_emit.rs)

HIR ──► Contract IR ──────┬─ schemas.ts (Zod)        (zod_emit.rs)        [exists]
        (wire-format SSOT)├─ types.ts                (existing)            [exists]
        [NEW LAYER]       ├─ vox-client.ts           (vox_client.rs)      [exists]
                          ├─ openapi.yaml            (openapi_emit.rs)    [Phase 2]
                          └─ JSON Schema             (schema/)            [exists]
```

The HIR is the SSOT. Two projections fan out from it:

- **Web IR** — the frontend projection. Already exists. Components, routes,
  primitives, style kwargs.
- **Contract IR** — the wire-format projection. Does not yet exist as a named layer;
  today each wire-format emitter walks HIR directly. This document proposes naming
  it and routing all wire-format emit through it.

### What goes in Contract IR

A typed projection over:

- `HirTypeDef` — flattened to a wire-format type with discriminants resolved
- `HirTable` — wire-shape for tabular records
- `HirEndpointFn` — method, path, params, request type, response type, error envelope,
  auth requirement, rate-limit hint

The projection enforces the wire-format-v1 rules from
[wire-format-v1-ssot.md](wire-format-v1-ssot.md): `Decimal`/`BigInt` as strings, RFC
3339 dates, absent-key for `Option<T>`, `_tag`-discriminated sums.

Each consumer (Zod, JSON Schema, OpenAPI, the TS client) reads Contract IR — never
HIR directly. This is the seam where Phase 2's OpenAPI emit lands without re-walking
HIR or duplicating wire-format rules.

### Two-way component interop without a new keyword

Phase 5's "Vox imports React" should not introduce an `import react …` keyword. The
existing `extern fn` primitive generalizes to:

```vox
// vox:skip
extern component MyButton(label: str, on_click: fn()) = "../ui/MyButton.tsx"
```

`extern component` reuses `HirFn.ts_extern_module` semantics (or, more precisely, the
analogous `HirReactiveComponent.ts_extern_module`) and the existing TSX import
emission. No new bare keyword. AGENTS.md's grammar-unification rule is preserved.

The TSX type bridge for prop types reads the source `.tsx` directly when possible
(option (a) in the plan); the fallback is a sidecar `.vox.types.json` produced by a
new `vox import-types` subcommand.

### K-complexity reduction in emitted TSX

Two mechanical wins, neither of which touches the HIR:

1. **Single React-runtime barrel.** Emit `vox-react-runtime.ts` that re-exports
   `useState`, `useEffect`, `useMemo`, `useCallback`. Each component file imports
   from one path instead of importing each hook directly. Smaller diffs across
   re-emits, smaller K for downstream tools.
2. **Single API barrel.** Fold `vox-client.ts`, `schemas.ts`, `types.ts` behind one
   `vox-api.ts` re-export. Keep the internal split for codegen ergonomics; present a
   single import surface to consumers. Phase 5's "React imports Vox" depends on the
   emitted output looking like a normal npm package.

## Sequencing

The pre-work cleanup is independent of the Contract IR work and can land first.

```
Pre-work (R1, R2, R3) ──► Contract IR module ──► OpenAPI emit (Phase 2) ──► extern component (Phase 5)
                                │
                                ├─► vox-react-runtime.ts barrel
                                └─► vox-api.ts barrel
```

Phases 3 (HTTP ergonomics decorators) and 4 (auth/ops stdlib) from the interop plan
are orthogonal and unaffected by this convergence work.

## What this document does not decide

- The exact module layout for Contract IR (one crate vs. a submodule of `vox-compiler`)
  — to be specified when the implementation lands
- The precise type-bridge mechanism for `extern component` prop types (Phase 5
  sub-spec)
- Whether to keep R5 (Express server emit) and R6 (legacy `voxdb.rs`) — both require
  consumer audit before deletion

## Related references

- [External Frontend Interop Plan (2026)](external-frontend-interop-plan-2026.md) — the
  five-phase plan this audit converges with
- [Wire Format v1 SSOT](wire-format-v1-ssot.md) — rules Contract IR must enforce
- [GUI-Native Roadmap Status (2026)](gui-native-roadmap-status-2026.md) — VUV phase
  status and the type-safe primitive surface
- [Svelte-Mineable Features Implementation Plan (2026)](svelte-mineable-features-implementation-plan-2026.md)
  — the related M1–M7 mining work that touches the same surfaces
- [AGENTS.md §Grammar Unification](../../../AGENTS.md) — the bare-keyword vs decorator rule
  that constrains how Phase 5 must be expressed
