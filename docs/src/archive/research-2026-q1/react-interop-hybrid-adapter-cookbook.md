---
title: "Hybrid adapter cookbook (SPA + SSR)"
description: "How user-owned adapters consume routes.manifest.ts and vox-client.ts for SPA/islands vs SSR tracks."
category: "Architecture SSOTs"
last_updated: "2026-04-07"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Hybrid adapter cookbook (SPA + SSR)

**SSOT:** [`react-interop-migration-charter-2026.md`](./react-interop-migration-charter-2026.md), [`react-interop-implementation-plan-2026.md`](./react-interop-implementation-plan-2026.md).

## Shared inputs

- **`routes.manifest.ts`** — `export const voxRoutes`, optional `notFoundComponent` / `errorComponent` / `globalPendingComponent`.
- **`vox-client.ts`** — typed `fetch` helpers: **`GET`** (+ JSON query values) for `@query`, **`POST`** + JSON for `@mutation` / `@server` (matches Axum).
- **Component `*.tsx`** — named exports next to the manifest.

## SPA + islands (default)

1. Use **`VOX_WEB_EMIT_SCAFFOLD=1`** on **`vox build`** once to materialize `app/App.tsx`, `app/main.tsx`, and Vite/Tailwind stubs if missing (see [`env-vars.md`](../reference/env-vars.md)).
2. In **`App.tsx`**, import `voxRoutes` and wire **`react-router`** `createBrowserRouter` / **`RouterProvider`**, or TanStack/React Router in “library” mode — Vox does not emit framework-specific trees.
3. Islands: keep **`@island`** outputs and `data-vox-island` mounts per existing contracts; hydrate from the same Vite bundle.

## SSR track (parallel)

1. Consume the **same** manifest in a framework that supports server loaders (e.g. TanStack Start file routes, Remix, custom RSC shell).
2. Prefetch loader data on the server using the same **`vox-client`** call shapes as the browser (POST bodies must mirror codegen).
3. **Do not** rely on removed outputs (`VoxTanStackRouter.tsx`, generated `App.tsx`, `serverFns.ts` / `createServerFn`).

## TanStack Start scaffold today

**`vox-cli`** seeds **`src/routes/*`** + **`routeTree.gen.ts`** when **`VOX_WEB_TANSTACK_START=1`**. Compiler output remains **manifest + components**; bridge the manifest into your router in user code when you outgrow the default `/` file route stub.

## Troubleshooting

- **Missing relative imports:** `vox build` validates `./` imports from `routes.manifest.ts` (and optional `App.tsx` in `out_dir`).
- **Legacy `@component fn` (transitional):** unset the escape hatch so classic **`@component fn`** is a **parse error** by default; set **`VOX_ALLOW_LEGACY_COMPONENT_FN=1`** only while migrating last fixtures. Use **`vox migrate web --write`** for a deterministic keyword patch, then **`vox migrate web --check`** in CI to ensure no retired-pattern diagnostics remain.

## Release / onboarding checklist (short)

- [ ] `vox build` produces **`routes.manifest.ts`** + **`vox-client.ts`** (when RPC/routes exist).
- [ ] Scaffold or adapter imports manifest from **`dist/`** (or your configured out dir).
- [ ] `doctor` passes pnpm/node; **`components.json`** has **`rsc: false`** when using shadcn; globals.css uses **`@import "tailwindcss"`** (v4).


