---
title: "TanStack Start Implementation Backlog"
description: "Complete checkbox-by-checkbox implementation backlog for Vox TanStack Start fullstack codegen. 200+ tasks organized by wave, with exact file names, line numbers, and code shapes for each change."
category: "Architecture SSOTs"
last_updated: "2026-04-08"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# TanStack Start Implementation Backlog

> [!NOTE]
> **Many file targets below name `tanstack_programmatic_routes.rs` — that module is retired.** Current implementation uses **`route_manifest.rs`**, **`vox_client.rs`**, **`scaffold.rs`**, and CLI templates. Treat unchecked items as **migration archaeology** unless explicitly refreshed against the tree.

> **SSOT spec:** [`tanstack-start-codegen-spec.md`](./tanstack-start-codegen-spec.md) (historical TanStack reference + charter links)  
> **Predecessor tasks (already done):** See [`tanstack-web-backlog.md`](./tanstack-web-backlog.md) Phases 0–6.

This backlog picks up where Phase 4 left off. Each task has a concrete file, change description, and `cargo check` gate where applicable.

## Wave status — truth table (manifest-first model)

Use this table **before** implementing any checkbox below. Rows summarize what shipped vs what was **cancelled** when the product moved to **`routes.manifest.ts` + user adapter** (no compiler-owned virtual route tree).

| Wave | Status | Ground truth in repo |
|------|--------|----------------------|
| **A** | **Mostly done** | [`RouteEntry`](../../../crates/vox-ast/src/decl/ui.rs): `loader_name`, `pending_component_name`, nested `children`; `redirect` / `is_wildcard` exist on AST but parser leaves defaults. [`RoutesDecl`](../../../crates/vox-ast/src/decl/ui.rs): `not_found_component`, `error_component`. Parser: [`tail.rs`](../../../crates/vox-compiler/src/parser/descent/decl/tail.rs) — `with loader:` / `pending:`, nested `{ }`, `not_found:`, `error:`. **Deferred:** `under LayoutName` / separate `layout_name` on `RouteEntry` (use nested route children); spec `layout_name` field in older docs does not match current AST. |
| **B–C** | **Partly obviated** | HIR ownership / legacy retirement evolved with Path C + `vox migrate web`. Verify current [`hir/nodes/decl.rs`](../../../crates/vox-compiler/src/hir/nodes/decl.rs) before acting on B/C checklists. |
| **D** | **Cancelled (shape)** | “New scaffold emitter” in compiler **exists** as opt-in [`codegen_ts/scaffold.rs`](../../../crates/vox-codegen-ts/src/scaffold.rs); **primary** one-time files come from **`vox-cli`** [`spa.rs`](../../../crates/vox-cli/src/templates/spa.rs) / [`tanstack.rs`](../../../crates/vox-cli/src/templates/tanstack.rs) / [`frontend.rs`](../../../crates/vox-cli/src/frontend.rs). Do not recreate D2–D4 Start-only `client.tsx` / `router.tsx` **from compiler alone** unless charter reopens that scope. |
| **E** | **Cancelled (product)** | Programmatic `__root.tsx` / `*.route.tsx` / `app/routes.ts` **virtual tree** from compiler is **gone**. Parity is [`route_manifest.rs`](../../../crates/vox-codegen-ts/src/route_manifest.rs) + TanStack **file** routes + optional `vox-manifest-route-adapter`. E6 “retired” already applies. |
| **F** | **Superseded** | `vox-client.ts` + Axum emit replaced `serverFns.ts` / `createServerFn`; see [`vox_client.rs`](../../../crates/vox-codegen-ts/src/vox_client.rs), [`http.rs`](../../../crates/vox-compiler/src/codegen_rust/emit/http.rs). |
| **G–K** | **Docs / tests polish** | Many G-items overlap [`react-interop-implementation-plan-2026.md`](./react-interop-implementation-plan-2026.md) Wave 7; tests exist under different names in `vox-compiler` / `vox-integration-tests`. |

**LLM guardrail:** If a task references `tanstack_programmatic_routes.rs` or “emit `app/routes.ts` from compiler,” treat it as **historical** unless you are explicitly restoring that architecture in a new ADR.

---

## WAVE A — AST Extensions

> **Status:** Superseded by the truth table above. Checkboxes A1–A15 remain for archaeology; **do not** treat all `[ ]` rows as open product work.

These tasks extend the parser/AST data model. Complete all before touching HIR or codegen.

### A1 — `RouteEntry`: Add `loader` field
- [ ] **File:** `crates/vox-ast/src/decl/ui.rs` line ~40
- [ ] Add `pub loader: Option<String>` to `RouteEntry` struct
- [ ] Doc comment: `/// Name of a @query or @server fn to use as TanStack Router route loader.`
- [ ] Add to `serde` derive and `PartialEq` impl (auto-derived — no manual work needed)

### A2 — `RouteEntry`: Add `pending_component` field
- [ ] **File:** `crates/vox-ast/src/decl/ui.rs`
- [ ] Add `pub pending_component: Option<String>` to `RouteEntry`
- [ ] Doc comment: `/// Per-route pending/suspense UI component (overrides module-level loading:).`

### A3 — `RouteEntry`: Add `layout_name` field
- [ ] **File:** `crates/vox-ast/src/decl/ui.rs`
- [ ] Add `pub layout_name: Option<String>` to `RouteEntry`
- [ ] Doc comment: `/// Name of a layout: fn this route should be nested under (pathless layout route).`

### A4 — `RoutesDecl`: Add `not_found_component` field
- [ ] **File:** `crates/vox-ast/src/decl/ui.rs` line ~16
- [ ] Add `pub not_found_component: Option<String>` to `RoutesDecl`
- [ ] Doc comment: `/// Component name for TanStack Router notFoundComponent (global 404 page).`

### A5 — `RoutesDecl`: Add `error_component` field
- [ ] **File:** `crates/vox-ast/src/decl/ui.rs`
- [ ] Add `pub error_component: Option<String>` to `RoutesDecl`
- [ ] Doc comment: `/// Component name for TanStack Router errorComponent (global error boundary).`

### A6 — Update `RoutesDecl::parse_summary` for new fields
- [ ] **File:** `crates/vox-ast/src/decl/ui.rs`
- [ ] Update `RoutesParseSummary` struct: add `not_found_component: Option<String>`, `error_component: Option<String>`
- [ ] Update `parse_summary()` impl to populate new fields

### A7 — Parser: extend route entry parsing with `with (loader:, pending:)`
- [ ] **File:** `crates/vox-compiler/src/parser/descent/decl/tail.rs` (or wherever routes `{ }` body is parsed — search for `RouteEntry`)
- [ ] After parsing `to ComponentName`, optionally parse `with` keyword
- [ ] `with loader: fnName` → `RouteEntry.loader = Some("fnName")`
- [ ] `with (loader: fnName)` → same as above
- [ ] `with (loader: fnName, pending: SpinnerName)` → both fields
- [ ] `with (pending: SpinnerName)` → only `pending_component`
- [ ] Emit parse error with helpful hint if `with` is followed by unexpected token

### A8 — Parser: extend route entry parsing with `under LayoutName`
- [ ] **File:** same as A7
- [ ] After optional `with (...)` clause, optionally parse `under LayoutName`
- [ ] `under LayoutName` → `RouteEntry.layout_name = Some("LayoutName")`
- [ ] Works with or without `with`

### A9 — Parser: `not_found: ComponentName` in routes body
- [ ] **File:** same as A7
- [ ] Inside `routes { }` body, parse `not_found: ComponentName` as a special entry
- [ ] Store in `RoutesDecl.not_found_component`
- [ ] `not_found:` is a keyword-colon form — check if token is `Token::NotFound` or `Token::Ident("not_found")`
- [ ] If `Token::NotFound` doesn't exist in lexer, handle as `Token::Ident("not_found")`

### A10 — Parser: `error: ComponentName` in routes body
- [ ] **File:** same as A7
- [ ] Parse `error: ComponentName` in routes body → `RoutesDecl.error_component`
- [ ] Similar to A9

### A11 — Parser: deprecation warning on `context: Name { }`
- [ ] **File:** wherever `Decl::Context` is parsed (search `parse_context`)
- [ ] After successfully parsing, push a `ParseError` warning (not error):
  - Message: `"context: declarations are retired. Use TanStack Router's router.context or pass state via @island TypeScript instead."`
  - Severity: Warning (ParseErrorClass::DeprecatedSyntax or similar)

### A12 — Parser: hard error on `@hook fn`
- [ ] **File:** `crates/vox-compiler/src/parser/descent/decl/head.rs` — find where `Token::AtHook` or `@hook` is dispatched
- [ ] Emit `ParseError` with message: `"@hook fn is retired. Hooks belong in @island TypeScript files (islands/src/<Name>/<Name>.tsx). See docs/src/reference/ref-decorators.md"`
- [ ] Return Err(()) — do not produce an AST node

### A13 — Parser: hard error on `@provider fn`
- [ ] **File:** same as A12
- [ ] Emit: `"@provider fn is retired. Wrap app-level providers in __root.tsx (generated scaffold). See docs/src/reference/ref-decorators.md"`

### A14 — Parser: hard error on `page: "path" { }`
- [ ] **File:** wherever `Decl::Page` is parsed
- [ ] Emit: `"page: declarations are retired. Use routes { } with TanStack Router file routes instead."`

### A15 — `cargo check` gate after A1–A14
- [ ] Run `cargo check -p vox-compiler`
- [ ] Fix any compilation errors from new required fields (add default values to constructors in tests or use `..Default::default()`)

archived_date: 2026-04-18
---

## WAVE B — HIR Changes

Extend and de-deprecate HIR to carry the new route metadata.

### B1 — `HirModule::client_routes` — Remove deprecation
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` line ~92
- [ ] Remove `#[deprecated(since = "0.3.0", note = "...")]` from `client_routes` field
- [ ] Update field doc: `/// Client-side TanStack route declarations (canonical AppContract field).`

### B2 — `HirModule::islands` — Remove deprecation
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` line ~94
- [ ] Remove deprecation attribute
- [ ] Update field doc: `/// @island declarations — canonical for TanStack Start island mounting.`

### B3 — `HirModule::loadings` — Remove deprecation
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` line ~112
- [ ] Remove deprecation attribute
- [ ] Update field doc: `/// loading: components — maps to TanStack Router pendingComponent.`

### B4 — `HirModule::layouts` — Remove deprecation
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` line ~96
- [ ] Remove deprecation attribute
- [ ] Update field doc: `/// layout: fn declarations — maps to TanStack Router pathless layout routes.`

### B5 — `HirModule::not_founds` — Remove deprecation
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` line ~115
- [ ] Remove deprecation attribute
- [ ] Update field doc: `/// not_found: components — maps to TanStack Router notFoundComponent.`

### B6 — `HirModule::error_boundaries` — Remove deprecation
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` line ~108
- [ ] Remove deprecation attribute
- [ ] Update field doc: `/// error_boundary: components — maps to TanStack Router errorComponent.`

### B7 — Update `field_ownership_map` — reclassify fields as AppContract
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` line ~187–195
- [ ] Change `"layouts"` from `MigrationOnly` to `AppContract`
- [ ] Change `"loadings"` from `MigrationOnly` to `AppContract`
- [ ] Change `"not_founds"` from `MigrationOnly` to `AppContract`
- [ ] Change `"error_boundaries"` from `MigrationOnly` to `AppContract`
- [ ] (client_routes and islands were already AppContract — verify)

### B8 — `HirRoutes` wrapper — route entries now carry loader/pending/layout metadata
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` line ~243
- [ ] `HirRoutes(pub crate::ast::decl::RoutesDecl)` wraps the AST RoutesDecl verbatim — since RouteEntry now has loader/pending/layout fields, HIR gets them automatically
- [ ] Verify that `HirRoutes.0.entries[n].loader` etc. are accessible in the route emitter
- [ ] No struct change needed (wrapper pattern)

### B9 — `HirLoweringMigrationFlags` — Remove classic component tracking notes
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` lines ~22–30
- [ ] Keep `used_classic_component_path` flag for now (needed for warning emission in typeck)
- [ ] Update doc to say: "Classic @component fn usage causes lint.legacy_component_fn; tracked here for warning-only gating."

### B10 — `HirModule::lower()` — Remove `#[allow(deprecated)]` after de-deprecation
- [ ] **File:** `crates/vox-compiler/src/hir/lower/mod.rs` line ~56
- [ ] After B1–B6, the `#[allow(deprecated)]` on `fn lower()` can be removed for the fields we de-deprecated
- [ ] Keep `#[allow(deprecated)]` only for `components`, `v0_components`, `pages`, `contexts`, `hooks` (still MigrationOnly)

### B11 — `to_semantic_hir()` — Keep deprecated fields excluded
- [ ] **File:** `crates/vox-compiler/src/hir/nodes/decl.rs` lines ~205–229
- [ ] Verify `SemanticHirModule` does NOT include: `components`, `v0_components`, `layouts`, `loadings`, `not_founds`, `error_boundaries`, `pages`, `contexts`, `hooks`
- [ ] Wait — after B4–B6, layouts/loadings/not_founds/error_boundaries become AppContract; they should probably be in SemanticHirModule
- [ ] Add `layouts`, `loadings`, `not_founds`, `error_boundaries` to `SemanticHirModule`
- [ ] Do NOT add `components`, `v0_components`, `pages`, `contexts`, `hooks` (still MigrationOnly — truly deprecated)

### B12 — `cargo check` gate after B1–B11
- [ ] Run `cargo check -p vox-compiler`
- [ ] Fix any clippy::deprecated warnings that remain

---

## WAVE C — Retire True Legacy (MigrationOnly fields)

These changes retired code paths that truly have no TanStack mapping. Do after Wave B so deprecated fields still exist while you clean up all their callers first.

### C1 — Typeck: Upgrade `@component fn` lint to ERROR
- [ ] **File:** `crates/vox-compiler/src/typeck/ast_decl_lints.rs` lines ~226–243
- [ ] Change `TypeckSeverity::Warning` to `TypeckSeverity::Error` for `lint.legacy_component_fn`
- [ ] Update message: `"Classic @component fn syntax is no longer supported. Migrate to Path C: component Name() { ... }"`
- [ ] Add suggestion: `"Run: vox migrate component <filename>.vox to auto-migrate"`

### C2 — Typeck: Upgrade `context:` lint to ERROR
- [ ] **File:** `crates/vox-compiler/src/typeck/ast_decl_lints.rs`
- [ ] Add a new lint check for `Decl::Context` — emit Error, not Warning
- [ ] Message: `"context: declarations are retired. Use TanStack Router router.context or islands for local state."`

### C3 — Typeck: Add `@hook` lint (already Error from parser)
- [ ] **File:** `crates/vox-compiler/src/typeck/ast_decl_lints.rs`
- [ ] If `Decl::Hook` somehow makes it past the parser (legacy AST files), emit Error in typeck too
- [ ] Verify the HIR lowercase arm still pushes to `hooks` and emits migration flag

### C4 — Typeck: Add `page:` lint (Error)
- [ ] **File:** `crates/vox-compiler/src/typeck/ast_decl_lints.rs`
- [ ] For `Decl::Page`: emit TypeckSeverity::Error
- [ ] Message: `"page: declarations are retired. Use routes { } with TanStack Router."`

### C5 — Emitter: Remove classic `components` loop
- [ ] **File:** `crates/vox-codegen-ts/src/emitter.rs` lines ~96–107
- [ ] Remove the loop `for hir_comp in &hir.components { ... }`
- [ ] Remove the matching CSS loop `for hir_comp in &hir.components { if !comp.styles.is_empty() { ... } }` (lines ~233–257)
- [ ] These loops emit the old `@component fn` TypeScript — now superseded by Path C

### C6 — Emitter: Remove `v0_components` placeholder loop
- [ ] **File:** `crates/vox-codegen-ts/src/emitter.rs` lines ~125–137
- [ ] Remove the loop `for hir_v0 in &hir.v0_components { ... }`
- [ ] `@v0` directives should be handled via `@island` with a v0 download note — no separate loop needed
- [ ] Verify: is `@v0` still parsed and lowered to `HirV0Component`? If so, update lowering to convert to `HirIsland` with a special `is_v0` flag, or emit a deprecation error at parse time

### C7 — Emitter: Remove web_projection_cache check for `hir.components`
- [ ] **File:** `crates/vox-codegen-ts/src/emitter.rs` lines ~86–93
- [ ] The `web_projection_cache` condition checks `hir.components.is_empty()` — after removing the components loop, this check is still valid but update to reflect new semantics
- [ ] New condition: `if hir.reactive_components.is_empty() && hir.loadings.is_empty()`

### C8 — `#[allow(deprecated)]` audit in `generate_with_options`
- [ ] **File:** `crates/vox-codegen-ts/src/emitter.rs` line ~63
- [ ] After C5–C7, audit which deprecated fields `generate_with_options` still touches
- [ ] For fields still needed (e.g. `client_routes`, `islands`, `loadings` — now de-deprecated), remove from allow list
- [ ] For fields truly removed (components, v0_components), remove the allow
- [ ] Keep allow only for `pages`, `contexts`, `hooks` if those are read for lint emission only

### C9 — HIR lower: Remove `contexts` and `hooks` lowering arms (or mark as error-only)
- [ ] **File:** `crates/vox-compiler/src/hir/lower/mod.rs` lines ~275–282
- [ ] `Decl::Context` arm: currently pushes to `hir.contexts` — change to push a hard diagnostic instead (or no-op since parser now hard-errors)
- [ ] `Decl::Hook` arm: same — parser hard-errors, but if AST node exists from old serialized code, emit diagnostic

### C10 — Remove callable.rs legacy arms (or update comments)
- [ ] **File:** `crates/vox-ast/src/decl/callable.rs`
- [ ] Search for arms that handle `ComponentDecl`, `LayoutDecl`, `ProviderDecl`, `HookDecl`
- [ ] These handle security decoration on declarations — if deprecated, add `// [RETIRED]` comment and emit a warning that the security model for these decls is unsupported

### C11 — Printer cleanup: Update fmt/printer.rs
- [ ] **File:** `crates/vox-compiler/src/fmt/printer.rs`
- [ ] Find arms for `Decl::Context`, `Decl::Hook`, `Decl::Provider`, `Decl::Page`
- [ ] Add `// [RETIRED]` comment and print with `// [retired syntax]` prefix
- [ ] Or: emit a `[Retired: use ... instead]` line for each

### C12 — `cargo check` gate after C1–C11
- [ ] Run `cargo check -p vox-compiler`
- [ ] Fix all new errors from removed fields
- [ ] Run `cargo test -p vox-compiler` — expect some snapshot failures from removed emission

archived_date: 2026-04-18
---

## WAVE D — New Scaffold Emitter

> **Cancelled as specified:** Scaffold is owned by **`vox-cli` templates** + optional **`codegen_ts::scaffold.rs`** (not the D2–D4 Start-only file set below as the only path). Implement D only if charter explicitly revives compiler-only Start app entrypoints.

Create the scaffold emission system from scratch.

### D1 — Create `crates/vox-codegen-ts/src/scaffold.rs` [NEW FILE]
- [ ] Create file with module doc: `//! Scaffold file emitter for TanStack Start projects. See tanstack-start-codegen-spec.md §8.3`
- [ ] Add `pub fn generate_scaffold_files(hir: &HirModule, project_name: &str) -> Vec<(String, String)>`
- [ ] Implement all sub-functions as listed below

### D2 — `scaffold.rs`: `fn client_tsx() -> String`
- [ ] Return exact `app/client.tsx` content from spec §4.8
- [ ] Includes: `StartClient`, `getRouter`, `ReactDOM.hydrateRoot`

### D3 — `scaffold.rs`: `fn router_tsx() -> String`
- [ ] Return exact `app/router.tsx` content from spec §4.8
- [ ] Includes: `getRouter()` factory, `createRouter`, `Register` declaration augmentation

### D4 — `scaffold.rs`: `fn ssr_tsx() -> String`
- [ ] Return `app/ssr.tsx` content: `createStartHandler({ createRouter: getRouter })(defaultStreamHandler)`

### D5 — `scaffold.rs`: `fn vite_config_ts() -> String`
- [ ] Return `vite.config.ts` content: `tanstackStart()`, `react()`, port 3000
- [ ] Note in comment: `// react plugin MUST come after tanstackStart`

### D6 — `scaffold.rs`: `fn package_json(project_name: &str) -> String`
- [ ] Return `package.json` content
- [ ] Scripts: `"dev": "vite dev"`, `"build": "vite build"`, `"start": "node .output/server/index.mjs"`
- [ ] Deps: `@tanstack/react-router`, `@tanstack/react-start`, `@tanstack/react-query`, `@tanstack/virtual-file-routes`, `react`, `react-dom`
- [ ] DevDeps: `@vitejs/plugin-react`, `typescript`, `vite`

### D7 — `scaffold.rs`: `fn tsconfig_json() -> String`
- [ ] Return `tsconfig.json` with: `jsx: "react-jsx"`, `moduleResolution: "Bundler"`, `module: "ESNext"`, `target: "ES2022"`, `skipLibCheck: true`, `strictNullChecks: true`
- [ ] Paths: `"~/*": ["./app/*"]`
- [ ] Include: `["app", "dist", "src"]`

### D8 — `scaffold.rs`: `fn generate_scaffold_files()` — assemble all
- [ ] Call each sub-function
- [ ] Return `Vec<(path, content)>` pairs with paths: `"app/client.tsx"`, `"app/router.tsx"`, `"app/ssr.tsx"`, `"vite.config.ts"`, `"package.json"`, `"tsconfig.json"`
- [ ] Do NOT include `"app/routes.ts"` here — that is generated by the route emitter since it changes on every build

### D9 — `scaffold.rs`: Add to `codegen_ts/mod.rs`
- [ ] **File:** `crates/vox-codegen-ts/src/mod.rs`
- [ ] Add: `pub mod scaffold;`
- [ ] Add: `pub use scaffold::generate_scaffold_files;`

### D10 — Wire `generate_scaffold_files` into `vox build --scaffold` CLI
- [ ] **File:** `crates/vox-cli/src/commands/build.rs` (or wherever build command is)
- [ ] Add `--scaffold` flag to the build command using clap
- [ ] When `--scaffold` is passed: call `generate_scaffold_files(hir, project_name)`
- [ ] For each file: if it already exists at dest path → skip (print "Skipping existing: {path}")
- [ ] If it does not exist → write (print "Created: {path}")

### D11 — Wire scaffold into `vox init --web` 
- [ ] **File:** `crates/vox-cli/src/commands/init.rs` (wherever init is handled)
- [ ] `vox init --web` should run scaffold emission after generating the `.vox` template
- [ ] After writing scaffold files: print instructions for `npm install` / `pnpm install`

### D12 — `cargo check` gate after D1–D11
- [ ] `cargo check -p vox-compiler -p vox-cli`

---

## WAVE E — Route Tree Emitter Refactor

> **Superseded in-tree:** the programmatic emitter module is **gone**. Equivalent product behavior is **`routes.manifest.ts`** + TanStack **file** routes + adapter/scaffold; use **Wave E** tasks only as a checklist when auditing manifest fields and adapter coverage.

This wave historically targeted `tanstack_programmatic_routes.rs` virtual file routes.

### E1 — Add `fn emit_root_tsx()` ~~to `tanstack_programmatic_routes.rs`~~
- [ ] **File:** ~~`crates/vox-codegen-ts/src/tanstack_programmatic_routes.rs`~~ — use `route_manifest.rs` / user `__root.tsx`
- [ ] New function signature: `fn emit_root_tsx(not_found: Option<&str>, error_comp: Option<&str>, global_loading: Option<&str>) -> String`
- [ ] Emits `__root.tsx` with `createRootRoute`, `HeadContent`, `Scripts`, `Outlet`
- [ ] Conditionally includes `notFoundComponent` and `errorComponent` lines if present
- [ ] Imports `HeadContent`, `Scripts` from `@tanstack/react-router`
- [ ] Root body: full html/head/body structure as per spec §4.2

### E2 — Add `fn emit_route_file()` to `tanstack_programmatic_routes.rs`
- [ ] New function: `fn emit_route_file(path: &str, component: &str, loader: Option<&str>, pending: Option<&str>) -> (String, String)` → (filename, content)
- [ ] Emits per-route file with `createFileRoute(path)({ loader, pendingComponent, component })`
- [ ] Loader arg handling: if loader present, emit `loader: ({ params }) => loaderFn({ data: { ...params } })()`
- [ ] Wait — params extraction requires knowing whether the loader needs params. For now: `loader: () => loaderFn()` for 0-param loaders, `loader: ({ params }) => loaderFn({ data: params })` for parameterized routes (path contains `$`)
- [ ] Filename generation: `/` → `index.route.tsx`, `/posts` → `posts.route.tsx`, `/posts/$id` → `posts-$id.route.tsx`

### E3 — Add `fn emit_layout_file()` to `tanstack_programmatic_routes.rs`
- [ ] New function: `fn emit_layout_file(layout_name: &str) -> (String, String)` → (filename, content)
- [ ] Emits a pathless layout component file that wraps `<Outlet />`
- [ ] The actual component logic comes from the `layout: fn Name()` Vox source — for now emit a stub that imports the component and wraps it
- [ ] NOTE: The `layout: fn` body is already emitted as a Path C component by `generate_reactive_component` (since `LayoutDecl` wraps a `FnDecl`). The layout file just re-exports it as a route layout.

### E4 — Add `fn emit_virtual_routes_ts()` to `tanstack_programmatic_routes.rs`
- [ ] New function: `fn emit_virtual_routes_ts(routes: &RoutesDecl, global_loading: Option<&str>) -> String`
- [ ] Imports: `rootRoute, route, index, layout` from `@tanstack/virtual-file-routes`
- [ ] Groups routes by layout_name (entries with same layout_name are under a `layout()`)
- [ ] Generates `routes = rootRoute("../dist/__root.tsx", [...])` tree
- [ ] Index route (`"/"` or `""`) uses `index(...)` not `route(...)` 
- [ ] Wildcard routes (`is_wildcard: true`) use `route("$",...)`

### E5 — Refactor `push_route_tree_files()` to use new functions
- [ ] **File:** ~~`crates/vox-codegen-ts/src/tanstack_programmatic_routes.rs`~~ — see `emitter.rs` + `route_manifest.rs`
- [ ] Replace the current body of `push_route_tree_files` with calls to E1–E4
- [ ] For each `HirRoutes` entry in `hir.client_routes`:
  - Call E1 → push `("__root.tsx", content)`
  - For each `entry` in routes.entries: call E2 → push `(filename, content)`
  - For each distinct `layout_name` in entries: call E3 → push `("LayoutName.route.tsx", content)` (but only if not already emitted as a reactive component)
  - Call E4 → push `("app/routes.ts", content)`
- [ ] The `_tanstack_start: bool` parameter: now always behaves as `tanstack_start = true`. Keep param for API compat, but ignore value.

### E6 — Remove old `App.tsx` and `VoxTanStackRouter.tsx` emission paths
- [x] **Retired** with programmatic emitter removal (`emitter.rs` / manifest path)
- [ ] Search for any code that emits `App.tsx` (SPA RouterProvider) — either in this file or in `emitter.rs`
- [ ] Remove the SPA path entirely — TanStack Start is the only output
- [ ] If `app/router.tsx` is now the canonical router entry, `App.tsx` is no longer needed

### E7 — Update `emitter.rs` to call `push_route_tree_files` with correct args
- [ ] **File:** `crates/vox-codegen-ts/src/emitter.rs` line ~259
- [ ] Current: `push_route_tree_files(&mut files, hir, options.tanstack_start);`
- [ ] After E5, the function signature may change — update call site
- [ ] Also: `app/routes.ts` is now in `files` — this is an `app/` prefixed path. Ensure the CLI's file writer handles `app/` subdirectory creation.

### E8 — `cargo check` gate after E1–E7
- [ ] `cargo check -p vox-compiler`
- [ ] Run existing snapshot tests — expect many failures (update snapshots)

### E9 — Update snapshot tests for new route file output
- [ ] **File:** `crates/vox-compiler/tests/` or `crates/vox-integration-tests/tests/`
- [ ] Update any test that asserts `VoxTanStackRouter.tsx` exists → assert `__root.tsx` and `index.route.tsx` and `app/routes.ts` exist instead
- [ ] Update content assertions for route files

### E10 — Update `pipeline.rs` integration tests
- [ ] **File:** `crates/vox-integration-tests/tests/pipeline.rs`
- [ ] Find TanStack route assertions (search `tanstack` or `Router`)
- [ ] Update expected output file names and content to match virtual file routes format

archived_date: 2026-04-18
---

## WAVE F — Server Function Fix

Fix the broken `serverFns.ts` emission.

### F1 — Add `fn emit_params_ts()` helper to `emitter.rs`
- [ ] **File:** `crates/vox-codegen-ts/src/emitter.rs`
- [ ] New private function: `fn emit_params_ts(params: &[HirParam]) -> String`
- [ ] Returns TypeScript parameter list: `"title: string, body: string"`
- [ ] Uses `crate::codegen_ts::hir_emit::map_hir_type_to_ts` for type mapping

### F2 — Add `fn emit_return_type_ts()` helper to `emitter.rs`
- [ ] **File:** `crates/vox-codegen-ts/src/emitter.rs`
- [ ] New private function: `fn emit_return_type_ts(ret: &Option<HirTypeRef>) -> String`
- [ ] Returns `"any"` if None, mapped type otherwise

### F3 — Add `fn has_path_params()` helper
- [ ] New private function: `fn has_path_params(path: &str) -> bool`
- [ ] Returns true if `path.contains('$')` (TanStack path param syntax)

### F4 — Replace server fn emission block in `emitter.rs` — @query fns
- [ ] **File:** `crates/vox-codegen-ts/src/emitter.rs` lines ~176–230
- [ ] Remove the existing block (save the structure for reference)
- [ ] Write new block for `@query` fns:
  - `method: "GET"`
  - No `inputValidator` for 0-arg queries
  - With params: `.inputValidator((data: { ... }) => data).handler(async ({ data }) => { ... })`
  - URL: uses query string for GET params via `URLSearchParams`
  - Uses `VOX_API` env var constant

### F5 — Write new emission block for `@mutation` fns
- [ ] Same location as F4
- [ ] `method: "POST"`
- [ ] `.inputValidator(...)` when params exist
- [ ] Body: JSON.stringify
- [ ] Correct `({ data })` destructure pattern in handler

### F6 — Write new emission block for `@server` fns
- [ ] Same location as F4
- [ ] Same as mutation (POST)

### F7 — Emit `const VOX_API = ...` at top of serverFns.ts
- [ ] Before all function declarations, emit:
  ```ts
  const VOX_API = process.env.VOX_API_URL ?? "http://localhost:4000";
  ```

### F8 — `cargo check` and test gate after F1–F7
- [ ] `cargo check -p vox-compiler`
- [ ] Write a new test: `query_fns_emit_get_method` — asserts emitted `serverFns.ts` contains `method: "GET"` for `@query` fns and `method: "POST"` for `@mutation` fns

---

## WAVE G — Documentation Updates

### G1 — Update `docs/src/architecture/tanstack-web-roadmap.md`
- [ ] Phase 4 status: "In progress → Done (virtual file routes + scaffold emitter)"
- [ ] Phase 5 status: "Now In progress — route loaders wired, @query method fix done"
- [ ] Add Phase 7 row: "TanStack Start complete codegen (scaffold, virtual routes, loaders, server fns)"
- [ ] Link to `tanstack-start-codegen-spec.md`

### G2 — Update `docs/src/architecture/tanstack-web-backlog.md`
- [ ] Mark existing Phase 4 items as done that are now done
- [ ] Add Phase 7 section with tasks from this backlog

### G3 — Update `docs/src/reference/ref-web-model.md`
- [ ] Section: routes syntax — Add `with (loader: fnName)` example
- [ ] Section: routes syntax — Add `under LayoutName` example
- [ ] Section: routes syntax — Add `not_found:` and `error:` examples
- [ ] Section: loading: — Clarify this maps to TanStack `pendingComponent`
- [ ] Section: layout: — Clarify this maps to TanStack pathless layout route

### G4 — Create or update `docs/src/reference/ref-decorators.md`
- [ ] Document: `loading: fn Name() { view: ... }`
- [ ] TanStack mapping: `pendingComponent` on routes
- [ ] Show full example with routes block binding

### G5 — Create or update `docs/src/reference/ref-decorators.md`
- [ ] Document: `layout: fn Name() { view: <div>...<Outlet/>...</div> }`
- [ ] TanStack mapping: pathless layout route file
- [ ] Show `under LayoutName` in routes block

### G6 — Update `docs/src/reference/ref-decorators.md`
- [ ] Document: `not_found: ComponentName` inside `routes { }` block
- [ ] TanStack mapping: `notFoundComponent` on `createRootRoute`

### G7 — Create `docs/src/reference/ref-decorators.md`
- [ ] Document: `error_boundary: ComponentName` inside `routes { }` block (or standalone)
- [ ] TanStack mapping: `errorComponent` on `createRootRoute`

### G8 — Update `docs/src/reference/ref-decorators.md` — RETIRED
- [ ] Mark as retired
- [ ] Add migration guide: "Use `router.context` from `createRouter({ context: {...} })` or `@island` TypeScript for local state"
- [ ] Remove code examples that use `context:` syntax

### G9 — Update `docs/src/reference/ref-decorators.md` — RETIRED
- [ ] Mark as retired
- [ ] Migration guide: "React hooks belong in `@island` TypeScript files: `islands/src/<Name>/<Name>.tsx`"

### G10 — Update `docs/src/reference/ref-decorators.md` — RETIRED
- [ ] Mark as retired
- [ ] Migration guide: "Add providers to `app/client.tsx` or `__root.tsx` wrapping `<Outlet />`"

archived_date: 2026-04-18
---

## WAVE H — Golden Examples

### H1 — Create `examples/golden/blog_fullstack.vox`
- [ ] Full golden example using: `@table`, `@query` with loader, `loading:`, `routes { with loader: }`, `component`, `@island`
- [ ] Must use `// vox:skip` or `// [REGION:display]` wrappers per doc pipeline rules
- [ ] Must parse cleanly without errors after Wave A parser changes
- [ ] Must produce complete virtual file routes output when compiled

### H2 — Create `examples/golden/layout_routes.vox`
- [ ] Demonstrates `layout: fn`, `under LayoutName` in routes
- [ ] Must parse and emit correctly

### H3 — Create `examples/golden/not_found_error.vox`
- [ ] Demonstrates `not_found:` and `error:` in routes block
- [ ] Must emit correct `__root.tsx` with `notFoundComponent` and `errorComponent`

### H4 — Update `examples/golden/rest_api.vox` if it exists
- [ ] Ensure it uses `@query`/`@mutation` not deprecated patterns
- [ ] Ensure `@server fn` examples are correct

### H5 — Run doc pipeline lint
- [ ] `vox doc-pipeline --lint-only` on updated docs
- [ ] Fix any `{{#include}}` directive failures from new golden files

---

## WAVE I — Tests

### I1 — Add snapshot test: `routes_emit_root_tsx`
- [ ] **File:** `crates/vox-compiler/tests/codegen_ts_routes.rs` (create if needed)
- [ ] Input: `.vox` with `routes { "/" to Home }`
- [ ] Assert `files` contains `("__root.tsx", content_with_createRootRoute)`
- [ ] Snapshot the content

### I2 — Add snapshot test: `routes_emit_index_route_tsx`
- [ ] Input: same as I1
- [ ] Assert files contains `("index.route.tsx", content_with_createFileRoute)`
- [ ] Snapshot content

### I3 — Add snapshot test: `routes_emit_virtual_routes_ts`
- [ ] Input: `routes { "/" to Home, "/posts" to PostList }`
- [ ] Assert files contains `("app/routes.ts", content_with_rootRoute_and_index_and_route)`

### I4 — Add test: `routes_with_loader_emits_loader_line`
- [ ] Input: `routes { "/posts" to PostList with loader: fetchPosts }`
- [ ] Assert route file contains `loader: () => fetchPosts()`

### I5 — Add test: `routes_with_pending_emits_pending_component`
- [ ] Input: `routes { "/posts" to PostList with pending: Spinner }`
- [ ] Assert route file contains `pendingComponent: Spinner`

### I6 — Add test: `routes_not_found_in_root_tsx`
- [ ] Input: `routes { "/" to Home \n not_found: NotFoundPage }`
- [ ] Assert `__root.tsx` contains `notFoundComponent: NotFoundPage`

### I7 — Add test: `routes_error_in_root_tsx`
- [ ] Input: `routes { "/" to Home \n error: ErrorFallback }`
- [ ] Assert `__root.tsx` contains `errorComponent: ErrorFallback`

### I8 — Add test: `query_fns_emit_get_in_server_fns_ts`
- [ ] Input: `@query fn getPosts() -> list[str] { ... }`
- [ ] Assert `serverFns.ts` contains `method: "GET"`
- [ ] Assert does NOT contain `method: "POST"`

### I9 — Add test: `mutation_fns_emit_post_in_server_fns_ts`
- [ ] Input: `@mutation fn createPost(title: str) -> str { ... }`
- [ ] Assert `serverFns.ts` contains `method: "POST"`
- [ ] Assert contains `.inputValidator((data: { title: string }) => data)`
- [ ] Assert handler uses `({ data })` destructuring

### I10 — Add test: `server_fns_ts_uses_vox_api_constant`
- [ ] Assert `serverFns.ts` starts with `const VOX_API = process.env.VOX_API_URL`

### I11 — Add test: `scaffold_files_are_generated`
- [ ] Call `generate_scaffold_files(hir, "test-app")`
- [ ] Assert all 6 scaffold file paths are present
- [ ] Assert `app/client.tsx` contains `StartClient`
- [ ] Assert `app/router.tsx` contains `getRouter` and `Register`
- [ ] Assert `app/ssr.tsx` contains `createStartHandler`
- [ ] Assert `vite.config.ts` contains `tanstackStart()`

### I12 — Add test: `component_fn_emits_error_not_warning`
- [ ] Input: `@component fn MyComp() { ret <div/> }`
- [ ] Assert typeck produces diagnostic with `code: "lint.legacy_component_fn"` and `severity: Error`

### I13 — Update `pipeline.rs` TanStack integration tests
- [ ] **File:** `crates/vox-integration-tests/tests/pipeline.rs`
- [ ] Remove assertions for `VoxTanStackRouter.tsx` output
- [ ] Add assertions for `__root.tsx`, `index.route.tsx`, `app/routes.ts`

### I14 — Run full test suite gate
- [ ] `cargo test -p vox-compiler -p vox-cli -p vox-integration-tests`
- [ ] Fix all failures

archived_date: 2026-04-18
---

## WAVE J — CLI Templates Update

### J1 — Update `crates/vox-cli/src/templates/tanstack.rs`
- [ ] Find `vite_config(...)` function — update to match spec §4.8 (tanstackStart plugin, no Vinxi reference)
- [ ] Find `package_json(...)` — update version pins for @tanstack/react-start, @tanstack/react-router
- [ ] Remove any reference to `vinxi` as a separate package (now bundled in react-start >= 1.x)
- [ ] Update `tsconfig_json(...)` if it exists here

### J2 — Update `vox init --web` template `.vox` file
- [ ] The `.vox` template generated by `vox init --web` should contain the new syntax:
  ```vox
// vox:skip
  component Home() {
    view: <h1>Hello from Vox!</h1>
  }
  
  routes {
    "/" to Home
  }
  ```
- [ ] No `@component fn`, no legacy syntax

### J3 — Update `crates/vox-cli/src/frontend.rs`
- [ ] Wherever `App.tsx` is referenced as the main entry point, update to `app/client.tsx` for TanStack Start mode
- [ ] Update `find_component_name` or equivalent — in Start mode the entry is `app/client.tsx`, not `App.tsx`

### J4 — Update `build_islands_if_present` logic
- [ ] **File:** `crates/vox-cli/src/frontend.rs` (or wherever islands build is triggered)
- [ ] Islands build is still triggered after main app build — no change to islands logic
- [ ] Just verify the islands package.json does not reference `@tanstack/react-router` separately (it should not — islands are plain React)

---

## WAVE K — Final ADR & Architecture Doc Updates

### K1 — Update `docs/src/adr/010-tanstack-web-spine.md`
- [ ] Add amendment section: "Amendment 2026-04-07: Virtual file routes adopted as canonical output" 
- [ ] Note: programmatic route tree (VoxTanStackRouter.tsx) is retired

### K2 — Update `docs/src/reference/vox-web-stack.md`
- [ ] Update the "code generation" section to reflect virtual file routes
- [ ] Add the server function architecture (TanStack Start + Axum topology)
- [ ] Update scaffold file list

### K3 — Update `docs/src/architecture/legacy-retirement-roadmap.md`
- [ ] Mark `@component fn`, `context:`, `@hook`, `@provider`, `page:` as RETIRED (not just deprecated)
- [ ] Mark `layout:`, `loading:`, `not_found:`, `error_boundary:` as REPURPOSED (mapped to TanStack)

### K4 — Update `docs/src/architecture/architecture-index.md`
- [ ] Add link to `tanstack-start-codegen-spec.md` under Web / Frontend Architecture

### K5 — Update `AGENTS.md` if needed
- [ ] No changes needed — AGENTS.md intentionally stays minimal

archived_date: 2026-04-18
---

## Execution Order

```
Wave A (AST) → cargo check
    ↓
Wave B (HIR de-deprecate) → cargo check
    ↓
Wave C (Retire legacy) → cargo check + test
    ↓ parallel with C:
Wave D (Scaffold emitter) → cargo check
    ↓
Wave E (Route emitter refactor) → cargo check + snapshot update
    ↓ parallel with E:
Wave F (Server fn fix) → cargo check + test
    ↓
Wave G (Docs) — parallel with E/F
Wave H (Golden examples) — after G
Wave I (Tests) — after E, F
Wave J (CLI templates) — after E, D
    ↓
Wave K (ADR updates) — last
```

---

## Done Criteria

- [ ] `cargo check -p vox-compiler -p vox-cli -p vox-integration-tests` passes with 0 errors
- [ ] `cargo test -p vox-compiler` passes (all snapshot tests updated)
- [ ] `cargo test -p vox-integration-tests` passes
- [ ] `vox build --scaffold` on `examples/golden/blog_fullstack.vox` produces all 13+ files
- [ ] `__root.tsx` is present with `createRootRoute`
- [ ] `index.route.tsx` is present with `createFileRoute("/")`
- [ ] `app/routes.ts` is present with `rootRoute`, `index`, and `route` calls
- [ ] `serverFns.ts` uses `GET` for `@query`, `POST` for `@mutation`
- [ ] Running `vite dev` on generated output starts a TanStack Start dev server without errors


