---
title: "Vox Web: Minimal React Interop Implementation Plan"
description: "Complete implementation plan for Vox's minimal-surface, framework-agnostic React interop system. Supersedes the TanStack Start-specific codegen plan. Covers route manifest pattern, vox-client typed fetch SDK, v0/shadcn compatibility, decorator retirement, and full migration with 250+ tasks."
category: "Architecture SSOTs"
last_updated: "2026-04-08"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox Web: Minimal React Interop — Implementation Plan

> **⚠ Superseded — archived 2026-05-03.**
> The current plan for this area is [external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).
> Retained for historical context only. Do not implement against this document.

> **Research foundation:** [`react-interop-research-findings-2026.md`](./react-interop-research-findings-2026.md)  
> **Supersedes:** [`tanstack-start-codegen-spec.md`](./tanstack-start-codegen-spec.md) (archived, not deleted)  
> **Backlog (250+ tasks):** [`react-interop-backlog-2026.md`](./react-interop-backlog-2026.md)

---

## Strategic Principle

**Vox is a component engine and API contract generator, not a framework bundler.**

Vox emits:
1. Pure named-export React functional components (stable forever)
2. A route manifest array (consumed by any router)
3. A typed `fetch` API client (consumed by any data layer)
4. Axum HTTP endpoint handlers (Rust, framework-free)
5. Typed TypeScript interfaces from Vox ADT declarations

Vox does NOT emit:
- Framework-specific file routing conventions (`__root.tsx`, `page.tsx`)
- Framework-specific RSC directives (`"use server"`, `"use client"`)
- Framework-specific server function calls (`createServerFn`)
- Routing configuration files (TanStack `routes.ts`, Next.js `app/` structure)

These belong in **user-owned scaffold files** that Vox generates once and never overwrites.

archived_date: 2026-04-18
---

## Architecture Overview

```
Vox Source (.vox)
       │
       ▼ vox build
┌──────────────────────────────────────────────────────────────┐
│ dist/ (regenerated every build)                              │
│                                                              │
│   *.tsx              ← Named-export React components         │
│   routes.manifest.ts ← VoxRoute[] array (path, component,   │
│                         loader?, pendingComponent?)          │
│   vox-client.ts      ← Typed fetch SDK for @query/@mutation  │
│   types.ts           ← TypeScript interfaces from @table     │
│   vox-islands-meta.ts ← Island registry for hydration       │
└──────────────────────────────────────────────────────────────┘

app/ (scaffold — written once, never overwritten)
│   main.tsx            ← ReactDOM.createRoot entry point
│   App.tsx             ← Router adapter (user customizes this)
│   globals.css         ← Tailwind v4 import
│   components.json     ← shadcn/ui registry configuration
│   vite.config.ts      ← Vite config with /api proxy
│   package.json        ← React + react-router + lucide-react
│   tsconfig.json       ← jsx, paths, moduleResolution
└── islands/            ← @island TypeScript implementations
```

**Key design decision:** `App.tsx` is the **adapter**. It imports `voxRoutes` from `dist/routes.manifest.ts` and wires them into whatever router the user prefers. Vox ships a default using `react-router` library mode, which works everywhere.

---

## What Changes vs. The Old Plan

| Area | Old Plan (TanStack-specific) | New Plan (Framework-agnostic) |
|------|------------------------------|-------------------------------|
| Routes output | `__root.tsx` + `*.route.tsx` + `app/routes.ts` | Single `routes.manifest.ts` array |
| Server functions | `createServerFn({ method: "GET" })` | `fetch(`/api/query/${fn}`)` typed SDK |
| Scaffold router | TanStack-specific `app/router.tsx` + `app/client.tsx` + `app/ssr.tsx` | Standard `app/App.tsx` + `main.tsx` |
| Routing dep | `@tanstack/react-router` | `react-router` (library mode) |
| Maintenance risk | High (TanStack API changes frequently) | Very Low (fetch + plain React are stable) |
| v0 compatibility | Requires TanStack cognizance | Perfect: v0 emits named-export React |
| SSR | Requires TanStack Start + Nitro | Optional: user chooses (Next.js, RR7 framework, none) |

archived_date: 2026-04-18
---

## Decorator Fate Table (Final)

| Decorator | Status | New Behavior |
|-----------|--------|--------------|
| `component Name() { view: ... }` | **KEEP — canonical** | Emits named-export `.tsx` |
| `@component fn` (classic) | **RETIRE → hard Error** | Migration: `component Name() { }` |
| `@island Name { prop: T }` | **KEEP — core** | Emits island registry entry |
| `@v0 Name` | **KEEP** | Emits island stub with v0 install comment |
| `routes { }` | **KEEP + SIMPLIFY** | Emits `routes.manifest.ts` VoxRoute[] |
| `loading: fn Name()` | **REPURPOSE** | Route manifest: `pendingComponent` field |
| `layout: fn Name()` | **REPURPOSE** | Route manifest: `children` grouping |
| `not_found: fn Name()` | **REPURPOSE** | Route manifest: registered in `App.tsx` scaffold |
| `error_boundary: fn Name()` | **REPURPOSE** | Route manifest: registered in `App.tsx` scaffold |
| `@query fn` | **KEEP + FIX** | `vox-client.ts`: typed `fetch` GET |
| `@mutation fn` | **KEEP + FIX** | `vox-client.ts`: typed `fetch` POST |
| `@server fn` | **KEEP + FIX** | `vox-client.ts`: typed `fetch` POST |
| `context: Name { }` | **RETIRE → hard Error** | No output. Migration: use React Context manually in App.tsx |
| `@hook fn` | **RETIRE → hard Error** | No output. Migration: use hooks in `@island` TypeScript files |
| `@provider fn` | **RETIRE → hard Error** | No output. Migration: add providers in scaffold `App.tsx` |
| `page: "path" { }` | **RETIRE → hard Error** | No output. Migration: use `routes { }` |

---

## New Codegen Output Specification

### 1. Component: `component Name() { }` → `Name.tsx`

**No change.** Path C emission is canonical. Named export, pure React TSX.

```tsx
// vox:skip
export function PostList(): React.ReactElement {
  return <div className="posts">...</div>
}
```

archived_date: 2026-04-18
---

### 2. Routes: `routes { }` → `routes.manifest.ts`

**Before (broken TanStack virtual files):**
```tsx
// vox:skip
// __root.tsx  ← framework-specific, brittle
export const Route = createRootRoute({ ... })

// posts.route.tsx ← framework-specific
export const Route = createFileRoute("/posts")({ ... })
```

**After (stable manifest):**
```ts
// generated/routes.manifest.ts
import type { ComponentType } from "react"
import { Home } from "./Home"
import { PostList } from "./PostList"
import { PostDetail } from "./PostDetail"
import { Spinner } from "./Spinner"
import { NotFoundPage } from "./NotFoundPage"

export type VoxRoute = {
  path: string
  component: ComponentType<any>
  loader?: (ctx: { params: Record<string, string> }) => Promise<unknown>
  pendingComponent?: ComponentType
  errorComponent?: ComponentType<{ error: Error }>
  children?: VoxRoute[]
  index?: boolean
}

export const notFoundComponent = NotFoundPage
export const globalPendingComponent = Spinner

export const voxRoutes: VoxRoute[] = [
  {
    path: "/",
    component: Home,
    index: true,
  },
  {
    path: "/posts",
    component: PostList,
    loader: () => voxFetch("GET", "/api/query/getPosts"),
    pendingComponent: Spinner,
  },
  {
    path: "/posts/:id",
    component: PostDetail,
    loader: ({ params }) => voxFetch("GET", `/api/query/getPost?id=${params.id}`),
  },
]

// Internal fetch primitive — do not use directly; use vox-client.ts
function voxFetch(method: string, path: string, body?: unknown) {
  const base = import.meta.env.VITE_API_URL ?? "http://localhost:4000"
  return fetch(`${base}${path}`, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  }).then(r => { if (!r.ok) throw new Error(`${path} ${r.status}`); return r.json() })
}
```

---

### 3. Data: `@query` / `@mutation` → `vox-client.ts`

**Before (broken TanStack createServerFn):**
```ts
export const getPosts = createServerFn({ method: "POST" })
  .handler(async (data) => fetch("/api/...").then(r => r.json()))
```

**After (stable typed fetch client):**
```ts
// generated/vox-client.ts
// Generated by Vox. Regenerated on every vox build. Do not edit.
const BASE = import.meta.env.VITE_API_URL ?? "http://localhost:4000"

async function $get<T>(path: string): Promise<T> {
  const r = await fetch(`${BASE}${path}`)
  if (!r.ok) throw new Error(`GET ${path} failed: ${r.status}`)
  return r.json()
}

async function $post<T>(path: string, body: unknown): Promise<T> {
  const r = await fetch(`${BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  })
  if (!r.ok) throw new Error(`POST ${path} failed: ${r.status}`)
  return r.json()
}

// @query fn getPosts() -> list[Post]
export async function getPosts(): Promise<Post[]> {
  return $get<Post[]>("/api/query/getPosts")
}

// @mutation fn createPost(title: str, body: str) -> Post
export async function createPost(data: { title: string; body: string }): Promise<Post> {
  return $post<Post>("/api/mutation/createPost", data)
}
```

archived_date: 2026-04-18
---

### 4. Scaffold: New Files (written once, never overwritten)

#### `app/main.tsx`
```tsx
// vox:skip
import React from "react"
import ReactDOM from "react-dom/client"
import { App } from "./App"
import "./globals.css"

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode><App /></React.StrictMode>
)
```

#### `app/App.tsx` — The Adapter
```tsx
// vox:skip
// This file is yours to modify. Vox generated it once and will never overwrite it.
// To use a different router (TanStack Router, Next.js, etc.), replace the body of this file.
import { BrowserRouter, Routes, Route, Navigate } from "react-router"
import { Suspense } from "react"
import {
  voxRoutes,
  notFoundComponent: NotFound,
  globalPendingComponent: GlobalSpinner,
  type VoxRoute,
} from "../dist/routes.manifest"

function renderRoutes(routes: VoxRoute[]) {
  return routes.map(r => (
    <Route
      key={r.path}
      path={r.path}
      index={r.index}
      element={
        <Suspense fallback={r.pendingComponent ? <r.pendingComponent /> : <GlobalSpinner />}>
          <r.component />
        </Suspense>
      }
    >
      {r.children && renderRoutes(r.children)}
    </Route>
  ))
}

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        {renderRoutes(voxRoutes)}
        <Route path="*" element={<NotFound />} />
      </Routes>
    </BrowserRouter>
  )
}
```

#### `app/globals.css`
```css
/* Tailwind v4 */
@import "tailwindcss";
```

#### `app/components.json`
```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "default",
  "rsc": false,
  "tailwind": {
    "config": "",
    "css": "app/globals.css",
    "baseColor": "slate",
    "cssVariables": true
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui"
  }
}
```

Note: `rsc: false` ensures v0.dev generates client-compatible components (no `"use server"`/`"use client"` directives). This is the critical v0 compatibility flag.

#### `vite.config.ts`
```ts
import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"
import path from "path"

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "./app") },
  },
  server: {
    port: 3000,
    proxy: {
      "/api": {
        target: process.env.VITE_API_URL ?? "http://localhost:4000",
        changeOrigin: true,
      },
    },
  },
})
```

#### `package.json`
```json
{
  "name": "vox-app",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "react-router": "^7.0.0",
    "lucide-react": "^0.400.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "tailwindcss": "^4.0.0",
    "@tailwindcss/vite": "^4.0.0",
    "typescript": "^5.6.0",
    "vite": "^6.0.0"
  }
}
```

#### `tsconfig.json`
```json
{
  "compilerOptions": {
    "jsx": "react-jsx",
    "moduleResolution": "Bundler",
    "module": "ESNext",
    "target": "ES2022",
    "skipLibCheck": true,
    "strictNullChecks": true,
    "paths": { "@/*": ["./app/*"] }
  },
  "include": ["app", "dist"]
}
```

---

## Vox Source Syntax: New Route Entry Forms

### Current (must still parse):
```vox
// vox:skip
routes {
  "/" to Home
  "/posts" to PostList
}
```

### Extended (implemented in compiler; layout `as` syntax is future work)

> **Parser status:** `with loader` / `with pending` / **nested** `{ ... }` child routes / `not_found:` / `error:` **parse and emit** into `routes.manifest.ts`. **`"/path" as layout Name { ... }`**, **HTTP redirects**, and **wildcard route lines** are **not** implemented yet (see `RouteEntry.redirect` / `is_wildcard` placeholders in the AST).

```vox
// vox:skip
@loading fn GlobalSpinner() to Element {
  ret <div class="spinner">"Loading…"</div>
}

component Home() { state n: int = 0 view: <span>"home"</span> }
component PostList() { state n: int = 0 view: <span>"posts"</span> }
component NotFoundPage() { state n: int = 0 view: <span>"404"</span> }
component ErrorFallback() { state n: int = 0 view: <span>"err"</span> }
@query fn getPosts() -> int { ret 0 }

routes {
  "/" to Home {
    "/posts" to PostList with loader: getPosts
  }
  not_found: NotFoundPage
  error: ErrorFallback
}
```

Future (not in the grammar today): `"/app" as layout AppShell { "/dashboard" to Dashboard }` — tracked as a parser/WebIR extension, not a normative example.

archived_date: 2026-04-18
---

## Execution Waves

### Wave 0 — AST/Parser Extensions
**Goal:** Support the new `routes { }` sub-syntax.

Tasks:
- `RouteEntry.loader: Option<String>` — name of a @query fn
- `RouteEntry.pending_component: Option<String>` — name of a loading: fn
- `RouteEntry.layout_name: Option<String>` — name of a layout group
- `RoutesDecl.not_found_component: Option<String>`
- `RoutesDecl.error_component: Option<String>`
- Parser: `with loader: fnName` clause after `to ComponentName`
- Parser: `with (loader: fnName, pending: SpinnerName)` variant
- Parser (deferred): `"/path" as layout Name { ... }` sub-block — **not implemented**; use nested string paths under a parent route instead
- Parser: `not_found: ComponentName` terminal in routes body
- Parser: `error: ComponentName` terminal in routes body
- Parser: hard error on `@hook fn` — message + docs link
- Parser: hard error on `@provider fn` — message + docs link
- Parser: hard error on `page: "path" { }` — message + docs link
- Parser: deprecation warning on `context: Name { }` — message + docs link
- `cargo check` gate

### Wave 1 — HIR De-deprecation
**Goal:** Remove `#[deprecated]` from HIR fields that are canonical AppContract items.

Tasks:
- Remove `#[deprecated]` from `HirModule::client_routes`
- Remove `#[deprecated]` from `HirModule::islands`
- Remove `#[deprecated]` from `HirModule::loadings`
- Remove `#[deprecated]` from `HirModule::layouts`
- Remove `#[deprecated]` from `HirModule::not_founds`
- Remove `#[deprecated]` from `HirModule::error_boundaries`
- Change all 6 fields from `MigrationOnly` → `AppContract` in `field_ownership_map()`
- Add `layouts`, `loadings`, `not_founds`, `error_boundaries` to `SemanticHirModule`
- Remove `#[allow(deprecated)]` from `generate_with_options` for these 6 fields
- `cargo check` gate

### Wave 2 — Retire True Legacy Codegen
**Goal:** Remove the code paths that generate stale, broken output.

Tasks:
- Upgrade `@component fn` lint from Warning → Error in `typeck/ast_decl_lints.rs`
- Add hard Error lint for `Decl::Context`
- Add Error lint for `Decl::Hook` (belt+suspenders behind parser error)
- Add Error lint for `Decl::Page`
- Remove `hir.components` loop from `codegen_ts/emitter.rs`
- Remove `hir.v0_components` standalone loop (keep @v0 as island)
- Remove `hir.components` CSS loop from `emitter.rs`
- Removed `VoxTanStackRouter.tsx` programmatic emitter (module retired; manifest + adapter is current)
- Remove `App.tsx` (SPA RouterProvider) emission path
- Keep `routeTree.gen.ts` re-export emission as a no-op / delete
- Remove `#[allow(deprecated)]` for `components`, `v0_components`, `pages` in `generate_with_options`
- Update `web_projection_cache` condition: use `reactive_components.is_empty() && loadings.is_empty()`
- `cargo check` gate + `cargo test` (many snapshot failures expected — update snapshots)

### Wave 3 — Route Manifest Emitter (New)
**Goal:** Replace the broken virtual file route emitter with the stable manifest emitter.

Tasks:
- Create `crates/vox-codegen-ts/src/route_manifest.rs` [NEW FILE]
- Add `pub fn emit_route_manifest(hir: &HirModule) -> String` 
- Emit `VoxRoute` TypeScript type definition at top of manifest
- Emit `notFoundComponent` export if `RoutesDecl.not_found_component` is set
- Emit `globalPendingComponent` export from module-level `loading:` fn if set
- Emit `voxRoutes: VoxRoute[]` array
- For each `RouteEntry`:
  - Emit `{ path, component }` minimum
  - If `loader`: emit `loader: (ctx) => voxFetch(...)` or `loader: () => voxFetch(...)` depending on whether path has `:params`
  - If `pending_component`: emit `pendingComponent: SpinnerName`
  - If `layout_name`: group children under parent `{ path: layoutPath, component: LayoutComp, children: [...] }`
- Emit `voxFetch` internal helper at bottom
- Import all referenced component names at top of manifest
- Emit `index: true` for root `/` route when path is `""` or `"/"`
- Register module in `codegen_ts/mod.rs`
- Wire into `emitter.rs::generate_with_options`: replace `push_route_tree_files` call with `push_route_manifest_file`
- `cargo check` gate

### Wave 4 — vox-client.ts Emitter (Fix)
**Goal:** Replace broken `createServerFn` emission with stable typed fetch emission.

Tasks:
- Add `fn emit_server_fn_client(hir: &HirModule) -> String` to `emitter.rs` or new file
- Emit `$get<T>` and `$post<T>` private helpers using `import.meta.env.VITE_API_URL`
- For each `@query` fn: emit `async function fnName(params): Promise<ReturnType>` that calls `$get`
- For each `@mutation` fn: emit `async function fnName(params): Promise<ReturnType>` that calls `$post`
- For each `@server` fn: emit same as mutation
- For `@query` fns with 0 params: URL is `/api/query/fnName` with no query string
- For `@query` fns with params: URL is `/api/query/fnName` + serialize params as query string
- For `@mutation` / `@server` with params: URL is `/api/mutation/fnName` or `/api/server/fnName`, body is JSON
- Remove old `serverFns.ts` emission (was using `createServerFn`)
- Output file is now `vox-client.ts` (rename from `serverFns.ts`)
- Update all tests that reference `serverFns.ts` → `vox-client.ts`
- Update `vox-tanstack-query.tsx` import from `serverFns` → `vox-client`
- `cargo check` + tests

### Wave 5 — Scaffold Emitter (New)
**Goal:** Generate one-time scaffold files that the user owns permanently.

Tasks:
- Create `crates/vox-codegen-ts/src/scaffold.rs` [NEW FILE]
- `fn emit_main_tsx() -> &'static str` — returns `app/main.tsx` content
- `fn emit_app_tsx(not_found: Option<&str>, error: Option<&str>, pending: Option<&str>) -> String` — returns `app/App.tsx` adapting `voxRoutes`
- `fn emit_globals_css() -> &'static str` — returns `app/globals.css` with Tailwind v4 `@import`
- `fn emit_components_json(project_name: &str) -> String` — returns `app/components.json` with `rsc: false`
- `fn emit_vite_config() -> &'static str` — returns `vite.config.ts` with proxy + `@` alias
- `fn emit_package_json(project_name: &str) -> String` — returns `package.json` (React 19, RR7, Tailwind v4)
- `fn emit_tsconfig() -> &'static str` — returns `tsconfig.json`
- `fn generate_scaffold_files(hir: &HirModule, project_name: &str) -> Vec<(String, String)>` — assembles all
- Register in `codegen_ts/mod.rs`
- Wire into `vox build --scaffold` CLI flag: loop over files, if file exists → skip, else write
- Wire into `vox init --web`: call scaffold + print instructions
- `cargo check` gate

### Wave 6 — CLI + Templates Update
**Goal:** Align templates and CLI entry points with new outputs.

Tasks:
- Remove `tanstack.rs` template references to `@tanstack/react-start`, `vinxi`, `createServerFn`
- Update `templates/package_json()` to emit React 19 + react-router + lucide-react deps
- Update `templates/vite_config()` to emit proxy-based config (not tanstackStart plugin)
- Update `templates/tsconfig()` to Tailwind v4 compatible
- Update `frontend.rs::find_component_name` or equivalent — entry point is now `app/main.tsx`, not `App.tsx`
- Update `npm_install_and_build` to not run `tsr generate` (no TanStack Router CLI needed)
- Update `build_islands_if_present` — island package.json does not need `react-router` dep
- Update `vox init --web` template vox file to use canonical Path C syntax
- Update `vox run` orchestration: in dev, start Vite on port 3000 + Axum on port 4000 (simplified from 4-process TanStack Start)
- `cargo check -p vox-cli` gate

### Wave 7 — Documentation Updates
**Goal:** Bring all docs into sync with the **manifest + `vox-client.ts`** model.

**Done (verify / maintain):**
- [`tanstack-web-backlog.md`](./tanstack-web-backlog.md) Phase 7 **wave verdicts** + Phase 5 Query note (**`useVoxServerQuery`** emitted; optional component auto-wrap).
- [`vox-web-stack.md`](../reference/vox-web-stack.md) — SPA vs Start, GET `@query`, links to [`vox-codegen-ts.md`](../reference/cli.md) + [`vox-fullstack-artifacts.md`](../reference/vox-fullstack-artifacts.md).
- [`ref-web-model.md`](../reference/ref-web-model.md) — route / loader / `not_found` / `error` (nested paths; **no** `as layout` / redirect / wildcard until implemented).
- [`tanstack-ssr-with-axum.md`](../how-to/tanstack-ssr-with-axum.md) — Start as user adapter; Axum proxy env.
- **API docs:** [`query.md`](../reference/ref-decorators.md), [`mutation.md`](../reference/ref-decorators.md), [`server.md`](../reference/ref-decorators.md), [`v0.md`](../reference/ref-decorators.md), [`component.md`](../reference/ref-decorators.md), [`deprecated.md`](../reference/ref-decorators.md). Route-level `loading` / `not_found` / `error` / nested `routes` syntax: [`ref-web-model.md`](../reference/ref-web-model.md) (per-decorator `loading.md` / `layout.md` files are **optional** future splits).
- [`architecture-index.md`](./architecture-index.md) links to interop research when touching navigation.

**Deferred / optional:**
- Dedicated **`v0-shadcn-vox.md`** cookbook (covered today by [`v0.md`](../reference/ref-decorators.md), doctor, scaffold `components.json`; add how-to when we want one narrative page).
- [`tanstack-web-roadmap.md`](./tanstack-web-roadmap.md) Phase 8 archive line — editorial when roadmap is next revised.

**Ongoing:** `mdbook build` in CI / local when editing `docs/src/`.

### Wave 8 — Golden Examples
**Goal:** Update examples to use canonical, new syntax.

**Status:**
- [x] `examples/golden/web_routing_fullstack.vox` — nested `routes`, `@query` loader, `@loading`, `not_found` / `error` (guarded by `cargo test -p vox-compiler all_golden_vox_examples_parse_and_lower`).
- [x] `examples/golden/blog_fullstack.vox` — `@table` + `@query` + `@mutation` + nested routes; pipeline: `cargo test -p vox-integration-tests --test pipeline golden_blog_fullstack_codegen_emits_manifest_get_and_post`.
- [x] `examples/golden/v0_shadcn_island.vox` — `@v0` chat-id stub + `routes`; pipeline: `golden_v0_shadcn_island_codegen_includes_routes_manifest`.
- [ ] `examples/golden/layout_groups.vox` — **blocked** until `"/path" as layout Name { }` is implemented; use nested string paths today.

### Wave 9 — Tests
**Goal:** Codegen and scaffold coverage.

Coverage today (names may differ from original sketch): `codegen_routes_produces_route_manifest_ts`, `codegen_routes_with_loading_emits_pending_component`, `codegen_tanstack_start_flag_does_not_emit_separate_router_file`, `golden_web_routing_fullstack_codegen_emits_manifest_and_client` in `crates/vox-integration-tests/tests/pipeline/includes/include_01.rs`; `codegen_nested_route_manifest_…`, `codegen_output_never_includes_vox_tanstack_router_or_server_fns`, `emitter_source_orders_validate_gate_before_route_manifest` in `crates/vox-compiler/tests/web_ir_lower_emit.rs`; `axum_emit_contract.rs` for GET query routes + mutation transaction error JSON.

**Deferred:** layout-group snapshot until `as layout` parsing exists.

---

## v0.dev / shadcn Compatibility Checklist

Scaffold vs compiler vs doctor — **\[scaffold]** items are written by `scaffold_react_app`; **\[compiler]** from `vox build` output; **\[doctor]** optional `vox doctor` checks when files exist.

- [x] **\[scaffold]** `components.json` includes `"rsc": false` (minimal shadcn-style manifest)
- [x] **\[scaffold]** `vite.config.ts` **`resolve.alias`**: `@` → `./src` (pairs with `tsconfig` paths; see [`spa.rs`](../../../crates/vox-cli/src/templates/spa.rs) `vite_config`)
- [x] **\[scaffold]** `tsconfig.json` includes `"baseUrl": "."` and `"paths": { "@/*": ["./src/*"] }`
- [x] **\[compiler]** JSX uses `className=` / named exports — see WebIR + `hir_emit`
- [x] **\[compiler]** No `"use server"` / `"use client"` in generated manifest
- [x] **\[compiler]** No `createServerFn` in `vox-client.ts` — `web_ir_lower_emit` / CI guards
- [x] **\[workflow]** `@island` implementations under `islands/src/`
- [x] **\[compiler]** `@v0` stub includes shadcn install hint comment in generated placeholder TSX
- [x] **\[scaffold]** Tailwind v4 — **policy:** default scaffold keeps **Vox theme baseline** CSS ([`index_css`](../../../crates/vox-cli/src/templates/spa.rs)); charter “interop target” means **CLI + docs align with shadcn/Tailwind v4** when authors add Tailwind (see [charter](./react-interop-migration-charter-2026.md#policy)). Optional: add `@import "tailwindcss"` in a follow-on template toggle.
- [x] **\[scaffold]** `lucide-react` in `package.json` dependencies

archived_date: 2026-04-18
---

## Migration Guide for Existing .vox Files

### `@component fn` → `component Name() { }`

```vox
// vox:skip
// BEFORE (error after migration)
@component fn MyButton(label: str) {
  view: <button>{{ label }}</button>
}

// AFTER (canonical Path C)
component MyButton(label: str) {
  view: <button>{{ label }}</button>
}
```

Run `vox migrate web` (with optional `--write` / `--check`) to auto-migrate `.vox` sources in the repo.

### `context: AuthContext { user: User }` → Delete

Not emitted. Replace with React Context in `@island` TypeScript or pass via props.

### `@hook fn useCounter()` → Move to island TypeScript

```ts
// islands/src/Counter/Counter.tsx
import { useState } from "react"

function useCounter(initial: number) {
  const [count, setCount] = useState(initial)
  return { count, increment: () => setCount(c => c + 1) }
}

export function Counter({ initial }: { initial: number }) {
  const { count, increment } = useCounter(initial)
  return <button onClick={increment}>{count}</button>
}
```

### `@provider fn ThemeProvider()` → Move to scaffold App.tsx

```tsx
// vox:skip
// app/App.tsx — add your providers here
import { ThemeProvider } from "./providers/theme"
...
export function App() {
  return (
    <ThemeProvider>
      <BrowserRouter>...</BrowserRouter>
    </ThemeProvider>
  )
}
```

---

## Done Criteria (machine gates + manual polish)

| Gate | Command / artifact | Notes |
|------|-------------------|--------|
| Compile | `cargo check -p vox-compiler -p vox-cli -p vox-integration-tests` | CI gate |
| Compiler tests | `cargo test -p vox-compiler` | Includes `web_ir_lower_emit`, `axum_emit_contract`, golden parse |
| Integration | `cargo test -p vox-integration-tests golden_web_routing_fullstack_codegen_emits_manifest_and_client` | Manifest + client smoke ([`include_01.rs`](../../../crates/vox-integration-tests/tests/pipeline/includes/include_01.rs)); add filters for new goldens as they land |
| Forbidden strings | `web_ir_lower_emit` / pipeline | No `VoxTanStackRouter`, `createServerFn` in generated TS (see compiler tests) |
| Optional E2E | `vox build` + `pnpm install && vite dev` on a scaffolded app | Manual / smoke job (`VOX_WEB_VITE_SMOKE`); not blocking on `blog_fullstack.vox` until golden exists |
| shadcn CLI | `npx shadcn@latest add …` | Validates `components.json` when authors run it; doctor warns on `rsc` |
| v0 drop-in | Islands + named exports | [`v0` decorator doc](../reference/ref-decorators.md), `v0_tsx_normalize` tests |

**Optional goldens:** `blog_fullstack.vox`, `v0_shadcn_island.vox` — tutorial narrative; **`web_routing_fullstack.vox`** already covers nested routes + loader + pending + `not_found` / `error`.


