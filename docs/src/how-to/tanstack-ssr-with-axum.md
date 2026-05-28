---
title: "TanStack SSR with Axum (development topology)"
description: "Official documentation for TanStack SSR with Axum (development topology) for the Vox language. Detailed technical reference, architecture"
category: "How-To Guides"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# TanStack SSR with Axum (development topology)

This how-to describes the **recommended** split from [ADR 010: TanStack web spine](../adr/010-tanstack-web-spine.md): **Axum** serves APIs and static assets; **TanStack Start** (or Vite SSR) serves **HTML** during SSR adoption.

## Why two processes (for now)

The shipped **`vox run`** path builds a **client** Vite bundle into `target/generated/public/` and runs the **generated Rust** binary with `rust_embed`. **Full-document SSR** requires a **JavaScript** runtime (Node) executing the TanStack Start server bundle. Until `vox run` orchestrates both, run them **side by side**.

## Suggested dev flow

1. **Terminal A** — generated Axum app (existing): `vox run` / `cargo run` in `target/generated` (port from `VOX_PORT`, default **3000**).
2. **Terminal B** — TanStack Start / Vite SSR dev server (after Start scaffold lands): `pnpm dev` in the web workspace package that owns Start (port **e.g. 3001**).
3. **Proxy** — point the browser at **3000** and configure Axum to **reverse-proxy** `GET /*` (except `/api`, static prefixes) -> **3001**, or browse **3001** directly during UI-only work.

## Environment variables (convention)

| Variable | Purpose |
| -------- | ------- |
| `VOX_PORT` | Axum listen port (existing) |
| `VOX_SSR_DEV_URL` | When set, generated Axum **GET** handlers fall back to proxying non-`/api` document requests to this origin (e.g. `http://127.0.0.1:3001`) before `rust_embed` |
| `VOX_ORCHESTRATE_VITE` | If `1`, `vox run` spawns **`pnpm run dev:ssr-upstream`** in `dist/app` (Vite on **3001**) and passes `VOX_SSR_DEV_URL` to the generated **`cargo run`** child unless you already exported it |

TanStack **Start**-specific `vite.config` and route files are still tracked in [tanstack-web-backlog.md](../archive/research-2026-q1/tanstack-web-backlog.md).

## Scaffold matrix (Vite app under `dist/.../app`)

| Mode | How to enable | What you get |
| ---- | ------------- | ------------ |
| **SPA (default)** | _(nothing)_ | `index.html` + `src/main.tsx` + Vite + TanStack Router imports from `src/generated/*`. |
| **TanStack Start** | `Vox.toml` **`[web] tanstack_start = true`** or **`VOX_WEB_TANSTACK_START=1`** (must match **`vox build`** so TS output aligns) | `vite dev` / `vite build`, `@tanstack/react-start` Vite plugin, `src/routes/__root.tsx`, `router.tsx`, `routeTree.gen.ts`. **`vox build`** emits **`routes.manifest.ts`** + components (no **`VoxTanStackRouter.tsx`**); the user-owned adapter wires TanStack file routes + manifest. **Without `routes {`:** `src/routes/index.tsx` plus a seed **`routeTree.gen.ts`**; **`pnpm run routes:gen`** refreshes it from **`@tanstack/router-cli`**. |

SSR in production still follows **ADR 010** (Axum + optional Node SSR upstream); this table is only the **local scaffold** written by `vox run` / bundle.

## Production Docker sketch

This is a **pattern**, not a single canonical image: your generated binary name and paths depend on the `.vox` project.

1. **Stage `web-build` (Node)** — `WORKDIR /app`, copy the scaffolded app (**`package.json`**, lockfile, **`src/`**), `pnpm install`, `pnpm run build` → Vite/Start **`dist/`** (or the output directory your template uses).
2. **Stage `rust-build`** — `WORKDIR /src`, copy the workspace (or at least the crate that builds the generated Axum binary), `cargo build --release -p <crate>` (often the generated package under **`target/generated`** in your pipeline).
3. **Runtime image** — slim Debian/Alpine (or `distroless`), install **`ca-certificates`** if you call HTTPS APIs, copy the **`target/release/<binary>`** from stage 2 and the **static** tree from stage 1 (or embed with **`rust_embed`** as in local `vox run`). Set **`VOX_PORT`** (or your listen binding) and, if you terminate TLS at Axum, document it separately.

For **full-document SSR** in production, ADR 010’s **Node SSR upstream** may run as a second container; Axum proxies **`GET /**`** to that service (same idea as **`VOX_SSR_DEV_URL`**, but with a stable internal URL).

## See also

- [TanStack web roadmap](../archive/research-2026-q1/tanstack-web-roadmap.md)
- [vox-web-stack.md](../reference/vox-web-stack.md)


