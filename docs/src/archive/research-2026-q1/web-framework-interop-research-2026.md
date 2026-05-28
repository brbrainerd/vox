---
title: "Web Framework Interop Research 2026"
description: "Codebase-grounded research on React, TanStack, Svelte, SolidJS, Next.js, Qwik, Astro, and Vite — their TypeScript requirements, convergence trends, and how Vox can support all of them as an AI-first compilation target without creating a maintenance nightmare."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-14"
training_eligible: false
training_rationale: "Documents the framework landscape and Vox's multi-backend strategy for TypeScript output, grounded in codebase audit of codegen_ts, web_ir, and cli templates."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Web Framework Interop Research 2026

> What does the modern web framework ecosystem require from Vox's TypeScript output, and how do we support it all without creating a maintenance nightmare?

## Related Vox Documents

| Kind | Document | Description |
|---|---|---|
| **SSOT** | [vox-web-stack.md](../reference/vox-web-stack.md) | Current shipped stack reference (Path C, React + TanStack Router/Start) |
| **ADR** | [ADR 010 — TanStack web spine](../adr/010-tanstack-web-spine.md) | Decision: TanStack Router as client routing backbone |
| **ADR** | [ADR 012 — Internal web IR strategy](../adr/012-internal-web-ir-strategy.md) | Decision: compiler-owned WebIR for frontend IR |
| **Analysis** | [web-architecture-analysis-2026.md](web-architecture-analysis-2026.md) | Path A/B/C evaluation; Path C chosen |
| **Roadmap** | [tanstack-web-roadmap.md](tanstack-web-roadmap.md) | TanStack phases: Router → Start, SSR |
| **Backlog** | [tanstack-web-backlog.md](tanstack-web-backlog.md) | Task-level decomposition of TanStack integration |
| **IR Schema** | [internal-web-ir-implementation-blueprint.md](internal-web-ir-implementation-blueprint.md) | Current-vs-target WebIR representation mapping |
| **Blueprint** | [internal-web-ir-implementation-blueprint.md](internal-web-ir-implementation-blueprint.md) | WebIR phased execution plan |
| **Research** | [research-llm-native-lang-design-2026.md](research-llm-native-lang-design-2026.md) | LLM-native language design implications |
| **Research** | [mobile-desktop-convergence-research-2026.md](mobile-desktop-convergence-research-2026.md) | Mobile/desktop unified browser view |

## Implementation Touchpoints

| File | Role | Framework Coupling |
|---|---|---|
| [`codegen_ts/emitter.rs`](../../../crates/vox-codegen/src/codegen_ts/emitter.rs) | Top-level TS file bundle orchestrator | Medium (React imports via components) |
| [`codegen_ts/reactive.rs`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs) | Path C reactive → React hooks (765 lines) | **High** — `useState`, `useMemo`, `useEffect` |
| [`codegen_ts/component.rs`](../../../crates/vox-codegen/src/codegen_ts/component.rs) | Classic `@component fn` → React TSX | **High** — React JSX output |
| [`codegen_ts/adt.rs`](../../../crates/vox-codegen/src/codegen_ts/adt.rs) | ADT → TS discriminated unions | **None** — pure TypeScript |
| [`codegen_ts/vox_client.rs`](../../../crates/vox-codegen/src/codegen_ts/vox_client.rs) | Typed fetch client for `@query`/`@mutation`/`@server` | **None** — pure `fetch()` |
| [`codegen_ts/route_manifest.rs`](../../../crates/vox-codegen/src/codegen_ts/route_manifest.rs) | `routes { }` → `routes.manifest.ts` | **Low** — imports `ComponentType` from `"react"` |
| [`codegen_ts/schema/`](../../../crates/vox-codegen/src/codegen_ts/schema/) | `@table` → TypeScript VoxDB schema | **None** — pure TypeScript |
| [`web_ir/mod.rs`](../../../crates/vox-codegen/src/web_ir/mod.rs) | WebIR schema (DOM, Behavior, Style, Route, Interop) | **Low** — `InteropNode::ReactComponentRef` |
| [`web_ir/emit_tsx.rs`](../../../crates/vox-codegen/src/web_ir/emit_tsx.rs) | WebIR → JSX string (parity/preview) | **High** — React JSX |
| [`react_bridge.rs`](../../../crates/vox-compiler/src/react_bridge.rs) | Vox `use_*` → React hooks mapping | **High** — React-specific |
| [`app_contract.rs`](../../../crates/vox-compiler/src/app_contract.rs) | `vox-app-contract.json` — machine-readable API surface | **None** — pure JSON |
| [`cli/templates/tanstack.rs`](../../../crates/vox-cli/src/templates/tanstack.rs) | TanStack Start scaffold (root, router, routeTree) | **High** — `@tanstack/react-router` |
| [`cli/templates/spa.rs`](../../../crates/vox-cli/src/templates/spa.rs) | SPA scaffold + manifest router adapter | **High** — React + TanStack |
| [`cli/templates/islands.rs`](../../../crates/vox-cli/src/templates/islands.rs) | Islands Vite bundle scaffold | **High** — React hydration |
| [`codegen_rust/`](../../../crates/vox-compiler/src/codegen_rust/) | Vox → Axum Rust server (HTTP, tables, actors) | **None** — framework-agnostic server |

---

## 1. Executive Summary

The web framework landscape in April 2026 has converged on three paradigms: **Virtual DOM with compiler assist** (React 19 + React Compiler), **compiled reactivity** (Svelte 5 Runes, SolidJS 2.0), and **resumability** (Qwik). All major meta-frameworks now build on **Vite 8 / Rolldown** (Rust bundler, 10–30× faster production builds). **TanStack** has emerged as the **framework-agnostic middleware layer** — Router, Start, and Query now support both React and SolidJS 2.0 beta (April 10, 2026).

### Codebase-Grounded Assessment

After auditing the actual codebase, the situation is better than expected in some areas and worse in others:

| Area | Status | Detail |
|---|---|---|
| `vox-client.ts` | ✅ **Already framework-agnostic** | Pure `fetch()` + JSON. No React or TanStack imports. See [`vox_client.rs`](../../../crates/vox-codegen/src/codegen_ts/vox_client.rs). |
| `types.ts` (ADTs) | ✅ **Already framework-agnostic** | TS discriminated unions with constructor fns. See [`adt.rs`](../../../crates/vox-codegen/src/codegen_ts/adt.rs). |
| `vox-app-contract.json` | ✅ **Already framework-agnostic** | Machine-readable JSON contract of all routes, server fns, queries, mutations, islands, MCP tools. See [`app_contract.rs`](../../../crates/vox-compiler/src/app_contract.rs). |
| CSS emission | ✅ **Already framework-agnostic** | Plain `.css` files from `style { }` blocks. No CSS-in-JS. |
| `schema.ts` (tables) | ✅ **Already framework-agnostic** | Pure TS interfaces from `@table`. |
| `routes.manifest.ts` | ⚠️ **Nearly agnostic** | Uses pure `VoxRoute` type but imports `ComponentType` from `"react"`. One-line fix. |
| Reactive components | 🔴 **Deeply React-coupled** | `reactive.rs` (765 lines) emits `useState`, `useMemo`, `useEffect` via [`react_bridge.rs`](../../../crates/vox-compiler/src/react_bridge.rs). |
| WebIR `InteropNode` | ⚠️ **React-coupled at IR level** | [`InteropNode::ReactComponentRef`](../../../crates/vox-codegen/src/web_ir/mod.rs#L386) is React-specific. Should generalize to `FrameworkComponentRef`. |
| CLI Templates | 🔴 **React+TanStack hardcoded** | [`tanstack.rs`](../../../crates/vox-cli/src/templates/tanstack.rs), [`spa.rs`](../../../crates/vox-cli/src/templates/spa.rs) emit React-specific scaffolds. |

**Key finding**: Vox already produces significant framework-agnostic output (types, API client, schema, app contract, CSS). The "library mode" concept would primarily surface these existing artifacts as a first-class build target, not require rewriting them.

archived_date: 2026-04-18
---

## 2. Framework Landscape — State of the Art (April 2026)

### 2.1 The Big Six + Vite

| Framework | Paradigm | Bundle/Perf | SSR Model | Build Tool | Status | TS Output Requirements |
|---|---|---|---|---|---|---|
| **React 19** | VDOM + Compiler auto-memo | Larger runtime, compiler reduces re-renders | RSC + Server Actions (in Next.js); SSR + Server Fns (in TanStack Start) | Vite 8 or Turbopack | Industry standard | `.tsx`, hooks, JSX |
| **Next.js 16** | React meta-framework | Turbopack (stable default), PPR for hybrid static/dynamic | RSC default + PPR | Turbopack | Production | File-based routing, `use server`, App Router |
| **TanStack Start** | React/Solid meta-framework | Minimal overhead, Vite-native | Selective SSR, streaming, server fns | Vite 8 | **RC** (feature-complete, API stable) | `createServerFn`, `createRoute`, type-safe loaders |
| **Svelte 5** | Compiled reactivity (Runes) | ~65% smaller JS than equivalent Next.js | SvelteKit SSR + streaming | Vite 8 | Production stable (Oct 2024) | `.svelte` files — NOT TypeScript components |
| **SolidJS 2.0** | Fine-grained signals (no VDOM) | Top-tier benchmarks | SolidStart v2 (via TanStack Start) | Vite 8 | **Beta** (Apr 2026) | JSX-like but with signal semantics |
| **Qwik** | Resumability (zero hydration) | ~1.6 KB initial JS | Resumable SSR | Vite 8 | Production stable | `.tsx` with `$()` lazy boundaries |
| **Astro 6** | Server Islands, zero-JS default | Near-zero client JS for content | Server Islands, SSR + streaming | Vite 8 | Production stable | `.astro` templates consume any framework |

### 2.2 TanStack: The Framework-Agnostic Middleware Layer

TanStack has become the **application infrastructure** between your UI framework and your data/server. From the [TanStack Start overview](https://tanstack.com/start/latest/docs/framework/react/overview) and [Solid 2.0 blog post](https://tanstack.com/blog/tanstack-start-solid-v2) (April 10, 2026):

| Library | Status | React | Solid | Angular | Vue | Svelte |
|---|---|---|---|---|---|---|
| **Router** | Stable | ✅ | ✅ (2.0 beta) | – | – | – |
| **Start** | RC | ✅ | ✅ (2.0 beta) | – | – | – |
| **Query** | Stable | ✅ | ✅ (2.0 beta) | ✅ | ✅ | ✅ |
| **Table** | Stable | ✅ | ✅ | – | ✅ | ✅ |
| **Form** | New | ✅ | ✅ | – | ✅ | – |
| **Store** | Alpha | Framework-agnostic core | | | | |
| **DB** | Beta | Framework-agnostic reactive database | | | | |
| **AI** | Alpha | Multi-provider AI helpers | | | | |

**New since our last survey**: TanStack DB (local-first reactive), TanStack AI (multi-provider), TanStack Pacer (rate-limiting), TanStack Hotkeys, TanStack Intent (gestures), TanStack CLI, TanStack Devtools, TanStack Builder (visual app builder), TanStack Learn.

**TanStack Start key features**: Full-document SSR, streaming, server routes & API routes, server functions (type-safe RPC), middleware & context, full-stack bundling, universal deployment, end-to-end type safety, selective SSR, SPA mode, static prerendering, ISR, LLM Optimization (LLMO) guide.

> [!IMPORTANT]
> TanStack Start for Solid 2.0 (beta) validates our multi-framework thesis: the same meta-framework infrastructure supports multiple UI runtimes. Vox's TanStack integration naturally extends to Solid when stable.

### 2.3 Vite 8 + Rolldown

Vite 8 (released March 12, 2026) ships [Rolldown](https://rolldown.rs) as default:

- **Rolldown 1.0 RC** (January 2026, [VoidZero announcement](https://voidzero.dev)): API stable, Rollup plugin API compatible
- **Unified dev/prod**: Same Rust bundler in development and production — eliminates behavior drift
- **10–30× faster** production builds than Rollup
- **Rollup plugin compat**: Majority of existing Vite/Rollup plugins work unmodified
- **Universal adoption**: React, Svelte, Solid, Qwik, Astro — every major framework uses Vite 8

**Implication**: Vox is already on Vite ([shipped stack](../reference/vox-web-stack.md)). This is confirmed correct. No custom bundler needed.

### 2.4 Svelte 5 — The Compiler Kinship

Svelte 5 blog survey (from [svelte.dev/blog](https://svelte.dev/blog), Oct 2024 – Apr 2026):

| Date | Feature | Significance for Vox |
|---|---|---|
| Oct 2024 | **Svelte 5 launches** — Runes (`$state`, `$derived`, `$effect`) | Compiled reactivity, closest analog to Vox's `state`/`derived`/`effect` |
| May 2025 | `await` in components | Async rendering, similar to Vox's async views |
| Jun 2025 | Attachments (new actions) | Composable DOM lifecycle, cf. Vox `on_mount` |
| Aug 2025 | **Async Svelte** — SSR + streaming | Full async SSR |
| Sep 2025 | Deno support, OpenTelemetry | Cross-runtime, observability |
| Oct 2025 | Remote Functions | Server functions (like Vox's `@server fn`) |
| Nov 2025 | Svelte MCP | AI tooling integration |
| Apr 2026 | Error boundaries on server | Server-side error handling |

> [!NOTE]
> Svelte uses `.svelte` files with its own compiler — **not** TypeScript/JSX. A Vox → Svelte codegen backend would require a **completely separate output path**. However, Svelte projects readily import TypeScript **libraries** (types, logic, schemas, API clients).

### 2.5 SolidJS 2.0 — The Signal-First Future

From the [TanStack Solid 2.0 blog post](https://tanstack.com/blog/tanstack-start-solid-v2) (April 10, 2026):

- **Fine-grained non-nullable async** — async values are always valid, no `undefined` gaps
- **Mutable derivations** — derived state that can be explicitly mutated
- **Derived signals** — more composable reactive primitives
- **Pull-based run-once SSR** — more efficient server rendering
- **Pending UI as an expression** — loading states as first-class values

Solid 2.0's reactivity model maps more naturally to Vox's semantics than React hooks:

| Vox HIR Node | React Output (current) | Solid Output (hypothetical) |
|---|---|---|
| `HirReactiveMember::State` | `const [x, setX] = useState(val)` | `const [x, setX] = createSignal(val)` |
| `HirReactiveMember::Derived` | `const y = useMemo(() => expr, [deps])` | `const y = createMemo(() => expr)` |
| `HirReactiveMember::Effect` | `useEffect(() => { body }, [deps])` | `createEffect(() => { body })` |
| `HirReactiveMember::OnMount` | `useEffect(() => { body }, [])` | `onMount(() => { body })` |
| `HirReactiveMember::OnCleanup` | `useEffect(() => () => { body }, [])` | `onCleanup(() => { body })` |

> [!TIP]
> Solid doesn't require manual dependency arrays — the compiler auto-tracks signal dependencies. This means Vox's [`extract_state_deps`](../../../crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs) logic would be unnecessary for a Solid backend, simplifying the codegen.

### 2.6 The Reactivity Model Divergence

The industry has split into three camps:

**Camp 1 — Virtual DOM + Compiler (React 19)**: Component-level re-renders with compiler-driven memoization. Manual hooks (`useState`, `useEffect`). React Compiler 1.0 auto-eliminates unnecessary `useMemo`/`useCallback`. Still the enterprise standard.

**Camp 2 — Compiled Reactivity (Svelte 5)**: Compiler generates surgical DOM updates at build time. Runes (`$state`, `$derived`, `$effect`) are compiler directives, not runtime APIs. No framework runtime shipped to the browser. `.svelte` file format is **not** TypeScript.

**Camp 3 — Fine-Grained Signals (SolidJS 2.0)**: Signals bind directly to DOM nodes — no virtual DOM, no component re-renders. JSX syntax is similar to React but with fundamentally different semantics (`count()` accessor vs `count` value). Top-tier benchmark performance.

---

## 3. Codebase Audit: What Vox Actually Emits Today

### 3.1 Complete Artifact Inventory

From [`codegen_ts/emitter.rs`](../../../crates/vox-codegen/src/codegen_ts/emitter.rs) — the `generate_with_options` function produces:

| Artifact | Source Node | Framework Deps | Notes |
|---|---|---|---|
| **`types.ts`** | `hir.types` (ADTs) | **None** | Pure TS discriminated unions + constructors. [`adt.rs`](../../../crates/vox-codegen/src/codegen_ts/adt.rs) |
| **`vox-app-contract.json`** | Full `HirModule` | **None** | JSON contract: HTTP routes, server fns, queries, mutations, islands, MCP tools. [`app_contract.rs`](../../../crates/vox-compiler/src/app_contract.rs) |
| **`vox-tanstack-query.tsx`** | Static helper | **React + TanStack Query** | `QueryClientProvider` + `useVoxServerQuery` wrapper |
| **`<Name>.tsx`** | `hir.components` | **React** | Classic `@component fn` → React function component |
| **`<Name>.tsx`** | `hir.reactive_components` | **React** | Path C reactive → `useState`/`useMemo`/`useEffect` |
| **`<Name>.tsx`** | `hir.v0_components` | **React** | v0.dev component placeholders |
| **`server.ts`** | `hir.server_fns` (Express opt-in) | **Express** | Only when `VOX_EMIT_EXPRESS_SERVER=1` |
| **`activities.ts`** | `hir.activities` | **None** | Temporal-style activity runner |
| **`schema.ts`** | `hir.tables` | **None** | Pure TS table interfaces |
| **`vox-client.ts`** | `hir.server_fns/query_fns/mutation_fns` | **None** | Pure `fetch()` client — `$get` (GET + JSON query) / `$post` (POST + JSON body) |
| **`<Name>.css`** | `style { }` blocks | **None** | Plain CSS modules |
| **`routes.manifest.ts`** | `hir.client_routes` via WebIR | **React** (one import) | `import type { ComponentType } from "react"` — fixable |
| **`vox-islands-meta.ts`** | `hir.islands` | **None** | `VOX_ISLAND_NAMES` const array + type |
| **`mobile-bridge.ts`** | `hir.functions[is_mobile_native]` | **Capacitor** | Mobile native bridge |
| **`mobile-utils.ts`** | `hir.imports[std.mobile]` | **None** | Web API polyfills for mobile |
| **`Dockerfile.<env>`** | `hir.environments` | **None** | Container specs |

### 3.2 Already Framework-Agnostic Outputs

These files can be consumed by **any** TypeScript project today:

1. **`types.ts`** — discriminated unions from Vox ADTs (97 lines of codegen in [`adt.rs`](../../../crates/vox-codegen/src/codegen_ts/adt.rs))
2. **`vox-client.ts`** — typed `fetch()` client. Comment at top of [`vox_client.rs`](../../../crates/vox-codegen/src/codegen_ts/vox_client.rs): *"Framework-agnostic typed fetch client"*. Uses `import.meta.env?.VITE_API_URL` for base URL (Vite convention, works in any Vite project regardless of framework)
3. **`schema.ts`** — table interfaces from `@table` declarations
4. **`vox-app-contract.json`** — machine-readable contract with `schema_version: 2`, HTTP routes, server fns, queries, mutations, islands, MCP tools, server config
5. **`activities.ts`** — Temporal-style activities, pure TS
6. **`vox-islands-meta.ts`** — `const VOX_ISLAND_NAMES` + type
7. **`*.css`** — plain CSS from `style { }` blocks

### 3.3 Framework-Coupled Outputs (React-Specific)

These require React at runtime:

1. **All `.tsx` component files** — `import React from "react"`, React hooks
2. **`vox-tanstack-query.tsx`** — `QueryClientProvider`, `useQuery` wrappers
3. **`routes.manifest.ts`** — imports `ComponentType` from `"react"` (one line)

### 3.4 WebIR: Framework Coupling at the IR Level

The WebIR schema ([`web_ir/mod.rs`](../../../crates/vox-codegen/src/web_ir/mod.rs)) is **mostly framework-agnostic** except:

- **`InteropNode::ReactComponentRef`** (line 386) — hard-codes React. Should become `FrameworkComponentRef { framework: TargetFramework, ... }`.
- **`BehaviorNode`** variants (`StateDecl`, `DerivedDecl`, `EffectDecl`) are framework-agnostic — they describe **intent** not React hooks.
- **`DomNode`** variants are framework-agnostic — they describe **DOM structure** not JSX.
- **`RouteNode`/`RouteContract`** are framework-agnostic — they describe **URL patterns and metadata**.

The WebIR is well-positioned for multi-backend emission. The React coupling is in the **emitters**, not the IR.

archived_date: 2026-04-18
---

## 4. What Each Framework Requires from TypeScript

### 4.1 The Common Denominator

Every framework can import and use:

| Category | Example | Framework-Agnostic? |
|---|---|---|
| **Types/interfaces** | TS discriminated unions from Vox ADTs | ✅ Yes — `types.ts` |
| **API clients** | Typed `fetch` wrappers for `@query`/`@mutation` | ✅ Yes — `vox-client.ts` |
| **Validation logic** | Type guard functions, Zod schemas | ✅ Yes (Zod generation needed) |
| **Data schemas** | Table interfaces from `@table` | ✅ Yes — `schema.ts` |
| **Constants** | Enum values, shared vocabulary | ✅ Yes — `types.ts` |
| **Business logic** | Pure functions (sort, filter, transform) | ✅ Yes (not yet separated from components) |
| **Route definitions** | Path patterns, component names, metadata | ⚠️ Nearly — `routes.manifest.ts` has one React import |
| **App contracts** | Machine-readable API surface | ✅ Yes — `vox-app-contract.json` |

### 4.2 What's Framework-Specific

| Requirement | React | Svelte | SolidJS | Qwik | Notes |
|---|---|---|---|---|---|
| **Component format** | `.tsx` + JSX | `.svelte` | `.tsx` + signals JSX | `.tsx` + `$()` | Each unique |
| **State** | `useState` | `$state()` | `createSignal()` | `useSignal()` | Same intent, different APIs |
| **Derived** | `useMemo` | `$derived()` | `createMemo()` | `useComputed$()` | Same intent |
| **Effects** | `useEffect` | `$effect()` | `createEffect()` | `useTask$()` | Same intent |
| **Lifecycle** | `useEffect([])` | `onMount`/`onDestroy` | `onMount`/`onCleanup` | `useVisibleTask$()` | Same intent |

> [!IMPORTANT]
> Vox's WebIR `BehaviorNode` variants (`StateDecl`, `DerivedDecl`, `EffectDecl`) already represent the **intent** (state, derived, effect) without framework-specific API names. The framework coupling lives entirely in the **emitter** layer, not the IR. This is the right architecture for multi-backend support.

---

## 5. Gap Analysis and Critique of Current Architecture

### 5.1 Gaps Identified

| # | Gap | Severity | Current State | Proposed Fix |
|---|---|---|---|---|
| **G1** | No library-mode build target | 🔴 Critical | All output assumes "full app" context | Add `vox build --mode library` — emits only framework-agnostic artifacts |
| **G2** | No Zod schema generation | 🔴 Critical | ADTs emit TS types only, no runtime validation | Add Zod schema codegen from `HirTypeDef` alongside `types.ts` |
| **G3** | `routes.manifest.ts` imports React | 🟡 Medium | `import type { ComponentType } from "react"` at line 190 of [`route_manifest.rs`](../../../crates/vox-codegen/src/codegen_ts/route_manifest.rs) | In library mode, emit route manifest as JSON or use generic function type |
| **G4** | Business logic coupled to components | 🟡 Medium | Pure functions only exist inside component bodies or as server fns | Separate non-server pure functions into `lib.ts` |
| **G5** | `InteropNode::ReactComponentRef` | 🟢 Low | WebIR IR is React-specific for component refs | Generalize to `FrameworkComponentRef` with target framework field |
| **G6** | No npm package scaffolding | 🟡 Medium | No `package.json` for publishable library output | Add `package.json` generation for library mode |
| **G7** | No ESM-only guarantee | 🟢 Low | Output uses `import.meta.env` (ESM), which is correct | Document as ESM-only; no CJS needed in 2026 |
| **G8** | No OpenAPI spec generation | 🟢 Low | `vox-app-contract.json` has route info but not OpenAPI format | Derive OpenAPI from app contract |
| **G9** | Solid signals backend | 🟢 Future | No Solid codegen; TanStack Start for Solid is in beta | Add when TanStack Start for Solid reaches stable |

### 5.2 What's NOT a Gap (Correcting Original Research)

The original research document incorrectly identified these as gaps:

| Original Claim | Reality |
|---|---|
| "`vox-client.ts` has TanStack assumptions" | **Wrong.** `vox-client.ts` is already 100% framework-agnostic (`fetch` only). The docstring in [`vox_client.rs`](../../../crates/vox-codegen/src/codegen_ts/vox_client.rs) line 1 says: *"Framework-agnostic typed fetch client"*. |
| "Need framework-agnostic type generation" | **Wrong.** [`adt.rs`](../../../crates/vox-codegen/src/codegen_ts/adt.rs) already emits pure TypeScript discriminated unions with zero framework imports. |
| "CSS is framework-coupled" | **Wrong.** CSS emission in `emitter.rs` (lines 187–236) outputs plain `.css` files with no framework deps. |
| "Route definitions need to be data" | **Partially wrong.** `routes.manifest.ts` already defines `VoxRoute[]` as a data structure with `path`, `component`, `loader`, `children`, etc. The only coupling is one React type import. |

### 5.3 Architecture Critique

**What's right:**
- WebIR separates **semantic intent** from **framework-specific emission** — the IR design is sound for multi-backend
- `BehaviorNode` variants are named by intent (`StateDecl`, `DerivedDecl`, `EffectDecl`) not by framework API
- The `vox-app-contract.json` is a pure JSON contract that any tooling can consume
- `vox-client.ts` uses vanilla `fetch()` — any framework can use it

**What needs improvement:**
- The `reactive.rs` file (765 lines) mixes IR walking with React-specific emission. A multi-backend approach would separate the **analysis pass** (dependency extraction, state tracking) from the **printer** (React hooks vs Solid signals vs vanilla)
- The `react_bridge.rs` hook-name constants (`USE_STATE`, `USE_MEMO`, `USE_EFFECT`) are used in the analysis pass of `reactive.rs` — these should be decoupled so analysis doesn't import framework names
- Library mode would need the `emitter.rs` orchestrator to skip component/reactive emission and emit only the framework-agnostic subset

archived_date: 2026-04-18
---

## 6. The Multi-Backend Strategy: "App Mode" vs "Library Mode"

### 6.1 Architecture

```
                       .vox source
                            │
                        Parser → HIR
                            │
                    ┌───────┴───────┐
                    │               │
              "App Mode"      "Library Mode"
              (full-stack)     (framework-agnostic)
                    │               │
           ┌───────┼───────┐       │
           │       │       │       ├── types.ts (ADTs)
        React   Solid   Vanilla    ├── schemas.ts (Zod — NEW)
        +TSR    +TSR    Signals    ├── vox-client.ts (fetch)
        (now)   (future) (future)  ├── schema.ts (tables)
                                   ├── vox-app-contract.json
                                   ├── routes.manifest.json (NEW)
                                   ├── activities.ts
                                   └── *.css
```

### 6.2 Library Mode — What It Means in Practice

Library mode is NOT a rewrite — it's a **filter** on what `emitter.rs` already produces:

```rust
// Proposed change to codegen_ts/emitter.rs
pub enum BuildMode {
    /// Emit everything (current behavior).
    App,
    /// Emit only framework-agnostic artifacts.
    Library,
}

pub fn generate_with_options(
    hir: &HirModule,
    options: CodegenOptions,
) -> Result<CodegenOutput, String> {
    // ... existing code ...

    if options.mode == BuildMode::Library {
        // Skip: reactive components, classic components, v0 placeholders,
        //        vox-tanstack-query.tsx, Express server
        // Keep: types.ts, vox-client.ts, schema.ts, vox-app-contract.json,
        //        routes.manifest.ts (with React import removed),
        //        vox-islands-meta.ts, activities.ts, *.css
        // Add:  schemas.ts (Zod), package.json scaffold
    }
}
```

### 6.3 What Each Framework Gets from Library Mode

| Consumer Framework | What They Import | How They Use It |
|---|---|---|
| **React + TanStack** | Everything (they use App mode anyway) | Full app scaffold |
| **Svelte + SvelteKit** | `types.ts`, `schemas.ts`, `vox-client.ts`, `schema.ts` | Types + API client in `.svelte` components |
| **SolidJS + SolidStart** | Same as Svelte + eventually App mode | Library now, app mode when Solid backend ships |
| **Next.js** | `types.ts`, `schemas.ts`, `vox-client.ts` | Types + API client in RSC |
| **Qwik** | `types.ts`, `schemas.ts`, `vox-client.ts` | Types + API client in Qwik components |
| **Astro** | `types.ts`, `schemas.ts`, `vox-client.ts` + React islands | Mixed: library + React component islands |
| **Vanilla TS** | All library artifacts | Types + API client in any TS project |

---

## 7. Maintenance Nightmare Prevention

### 7.1 Backend Cost Model

| Backend | LOC to Build | External API Surface | Churn Risk | Priority |
|---|---|---|---|---|
| **Library mode** | ~100–200 (filter existing output) | Zero (pure TS) | Very Low | P0 — Ship first |
| **React+TanStack** (current) | ~1,130 (reactive.rs + component.rs + jsx.rs) | React hooks + TanStack Router | Medium | Already shipped |
| **Solid+TanStack** (planned) | ~400–600 (shared WebIR infra) | Solid signals + TanStack Router | Medium | P1 — When Solid stable |
| **Vanilla signals** (future) | ~300–400 | Custom ~2 KB signal runtime | Very Low | P2 — If demand exists |
| **Svelte** ❌ | ~1,500+ (separate compiler output) | Runes, SvelteKit, `.svelte` format | High | **Do NOT build** |
| **Next.js** ❌ | ~1,000+ (RSC, Turbopack, file routing) | Rapidly evolving Vercel conventions | Very High | **Do NOT build** |

### 7.2 The Decision Gate: When to Add a Backend

| Criterion | Threshold |
|---|---|
| **Market share** | Only if framework has >5% usage OR first-class TanStack support |
| **Output delta vs existing** | Only if >30% of output differs from existing React backend |
| **Ecosystem value** | Only if it unlocks a component/tool ecosystem not reachable via library mode |
| **Max active backends** | 3 full-stack + 1 library mode at any time |

### 7.3 WebIR as the Firewall

The WebIR insulates backends from parser/semantic changes:

```
Parser → AST → HIR → WebIR → [React Printer / Solid Printer / Library Printer]
                  ↑              ↑
        Semantic analysis    Framework-specific
        (shared, stable)     (isolated, swappable)
```

Each printer walks the same `WebIrModule` and emits framework-specific code. Adding a Solid printer means implementing ~10 functions (one per `BehaviorNode` variant + DOM walker + import header). The WebIR validation layer ensures structural correctness before any printer runs.

archived_date: 2026-04-18
---

## 8. Specific Framework Strategies

### 8.1 React + TanStack Start (Primary — Current)

**Status**: Fully implemented. Vox emits full React+TanStack Start apps with typed routing, server functions, islands, and v0 integration.

**Keep**: This is the default `vox build` / `vox run` path. No changes needed.

### 8.2 Svelte / SvelteKit — Library Mode Only

**Strategy**: Do NOT build a Svelte codegen backend.

Svelte developers get:
- `types.ts` — all Vox ADTs as TypeScript
- `schemas.ts` — Zod schemas for runtime validation (new)
- `vox-client.ts` — typed `fetch()` API client (already works)
- `schema.ts` — table interfaces
- `*.css` — styles (already framework-agnostic)
- `vox-app-contract.json` — API surface contract

They write UI in native `.svelte` runes. This is the correct and sustainable approach.

### 8.3 SolidJS 2.0 — Future App Mode Backend

**Strategy**: When TanStack Start for Solid reaches stable (currently beta as of April 10, 2026), add a Solid codegen backend. The WebIR already represents the right semantic intent — just need a Solid printer alongside the React printer.

### 8.4 Next.js — Library Mode Only

**Strategy**: Do NOT build a Next.js codegen backend. Vercel's convention churn is too high. Next.js projects use library mode imports (types, API client, schemas). If a developer wants the Vox server backend, they can connect via `vox-client.ts` → Axum API.

### 8.5 Astro — Library Mode + React Islands

**Strategy**: Astro's island architecture is perfectly complementary. Vox-generated React `.tsx` components work directly in Astro via `@astrojs/react`. Library mode provides types and API clients.

### 8.6 Qwik — Library Mode Only

**Strategy**: Qwik's `$()` lazy boundaries and resumability are too framework-specific. Library mode covers the need.

---

## 9. Implementation Priorities

### 9.1 Priority-Ordered Workload

| Priority | Task | Effort | Impact | Justification |
|---|---|---|---|---|
| **P0** | Library mode flag in `CodegenOptions` + filtered `emitter.rs` | 1–2 days | 🔴 Very High | Unlocks ALL frameworks instantly by surfacing existing agnostic artifacts |
| **P1** | Zod schema generation from `HirTypeDef` | 3–5 days | 🔴 Very High | Runtime validation at API boundaries; standard in every framework ecosystem |
| **P2** | Remove React import from `routes.manifest.ts` in library mode | 1 hour | 🟡 High | One-line change: use generic function type instead of `ComponentType` |
| **P3** | JSON route manifest export (alongside TS version) | 1 day | 🟡 High | Framework-agnostic route data for tools, mobile apps, non-React consumers |
| **P4** | npm `package.json` scaffold for library output | 1 day | 🟡 High | Makes library mode publishable to npm registries |
| **P5** | Separate pure functions into `lib.ts` in library mode | 2–3 days | 🟡 Medium | Business logic currently trapped in component bodies |
| **P6** | Generalize `InteropNode::ReactComponentRef` → `FrameworkComponentRef` | 1 day | 🟢 Low | IR cleanliness, prepares for multi-backend |
| **P7** | Solid signals printer (when TanStack Start for Solid stable) | 2–3 weeks | 🟢 Medium | Natural extension of WebIR architecture |
| **P8** | OpenAPI spec generation from `AppContractModule` | 1 week | 🟢 Low | API-first consumers, mobile, external services |

### 9.2 What NOT to Build

| Feature | Reason | Alternative |
|---|---|---|
| Svelte codegen backend | Completely different `.svelte` syntax, Runes compiler, high maintenance | Library mode |
| Next.js codegen backend | Vercel-proprietary, RSC complexity, rapid convention churn | Library mode |
| Qwik codegen backend | `$()` markers and resumability too framework-specific | Library mode |
| Angular codegen backend | Decorators, DI, modules — paradigm mismatch | Library mode |
| WASM UI backend | Premature — Component Model timeline is 2027+ | Wait |

archived_date: 2026-04-18
---

## 10. Industry Direction and Vox Positioning

### 10.1 Where the Industry Is Going (2026–2027)

1. **Compiled reactivity is winning** — Svelte 5 Runes, Solid 2.0 signals, and React Compiler all prove build-time optimization beats runtime overhead
2. **TanStack is becoming the middleware standard** — framework-agnostic routing, data, forms, tables, AI
3. **Vite + Rolldown is the universal build tool** — every major framework has standardized
4. **Server-first is table stakes** — SSR + streaming + server functions expected by default
5. **AI-native tooling is emerging** — Svelte MCP, TanStack LLMO guide, framework-specific AI integrations
6. **Signals are not replacing VDOM** — they're complementing it (React Compiler + signals-inspired patterns; hybrid architectures)

### 10.2 Vox's Strategic Position

Vox is uniquely positioned as a language that:

1. **Already emits significant framework-agnostic TypeScript** — types, API clients, schemas, contracts, CSS
2. **Has the right IR architecture** for multi-backend — WebIR separates intent from framework-specific emission
3. **Is AI-first by design** — deterministic grammar, constrained surface area, one syntax per concept
4. **Targets the framework-agnostic middleware layer** (TanStack) rather than individual UI runtimes

### 10.3 The One-Sentence Strategy

**Vox should formalize its existing framework-agnostic output as "library mode", add Zod schema generation, and defer new codegen backends until TanStack Start for Solid reaches stable — never building backends for Svelte, Next.js, or Angular.**

---

## 11. Research Sources

1. **React 19 / React Compiler**: Web search synthesis (Apr 2026). Compiler 1.0 stable (Oct 2025), auto-memoization. RSC as standard.
2. **TanStack Start**: [tanstack.com/start](https://tanstack.com/start/latest/docs/framework/react/overview) — RC status, Vite-native, feature-complete.
3. **TanStack + Solid 2.0**: [blog post](https://tanstack.com/blog/tanstack-start-solid-v2) (Apr 10, 2026). Beta support in Router, Start, Query.
4. **Svelte 5**: [svelte.dev/blog](https://svelte.dev/blog) — 18 months of monthly updates since Svelte 5 launch. Async SSR, Remote Functions, MCP, OpenTelemetry.
5. **SolidJS 2.0**: Via TanStack blog. Non-nullable async, mutable derivations, pull-based SSR.
6. **Next.js 16**: Web search. Turbopack stable, PPR stable.
7. **Vite 8 / Rolldown**: [vite.dev](https://vite.dev), [rolldown.rs](https://rolldown.rs), [voidzero.dev](https://voidzero.dev). Rolldown 1.0 RC (Jan 2026).
8. **Framework comparison**: Synthesized from strapi.io, quartzdevs.com, dev.to, medium.com, boundev.com, acropolium.com.
9. **Vox codebase**: Direct audit of `codegen_ts/`, `web_ir/`, `app_contract.rs`, `react_bridge.rs`, `codegen_rust/`, `cli/templates/`.


