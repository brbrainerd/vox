---
title: "Vox full-stack build artifacts — single source of truth"
description: "Official documentation for Vox full-stack build artifacts — single source of truth for the Vox language."
category: "Language Reference"
last_updated: "2026-05-11"
training_eligible: true

schema_type: "TechArticle"
---

# Vox full-stack build artifacts — single source of truth

This document names **every major output** of `vox build` / `vox run` / `vox bundle` and the **canonical runtime** for the default product path. It complements [vox-web-stack.md](vox-web-stack.md) and [ADR 010 — TanStack web spine](../adr/010-tanstack-web-spine.md).

## Canonical path (default)

| Layer | Artifact | Role |
| ----- | -------- | ---- |
| HTTP API | `target/generated/src/main.rs` (+ `lib.rs`, …) | **Axum** listens on `VOX_PORT` (default 3000). |
| Typed HTTP client (`vox-client.ts`) | `out_dir/vox-client.ts` when `@endpoint` declarations exist | Covers **query**, **mutation**, and **server** endpoints: **`GET`** + JSON query args; **`POST`** + JSON body. Paths match [`web_prefixes.rs`](../../../crates/vox-compiler/src/web_prefixes.rs). Runtime base: **`configureVoxApiBase(...)`** then **`import.meta.env.VITE_API_URL`**. On non-OK responses, parses [Wire Format §6](../architecture/wire-format-v1-ssot.md#6-error-envelope) JSON into **`VoxWireError`** when the body matches (`VoxApiError.wireError`). |
| TS SDK-only emit | Same files as client target | **`vox emit client <file> [-o DIR]`** — same outputs as **`vox build --target=client`** (Library mode): `vox-client.ts`, `openapi.json`, types/schemas, optional **`package.json`** ([`library_package_emit.rs`](../../../crates/vox-codegen/src/codegen_ts/library_package_emit.rs)) — without emitting Rust. |
| Route manifest | `out_dir/routes.manifest.ts` | `voxRoutes` tree for SPA/Start adapters (`routes {` present). |
| UI | `out_dir/*.tsx`, `out_dir/*.ts` | React components + router shell; SPA scaffold uses manifest when present. |
| Static HTML shells | `target/generated/public/ssg-shells/**` | From [`vox-ssg`](../../../crates/vox-ssg/src/lib.rs): minimal shells for `routes {` / `@page` (hydration anchor, not a second UI runtime). |
| Embedded static (after frontend build) | `target/generated/public/**` | Vite `dist/` copied here for `rust_embed` in release flows. |

**`vox run`** (app mode): builds TS to `dist/`, runs **`cargo run` in `target/generated`** — the **Rust binary** is the primary server.

## Legacy / opt-in: Express `server.ts`

[`vox-codegen-ts`](../../../crates/vox-codegen/src/codegen_ts/routes.rs) can emit **`server.ts`**, an **Express** app that duplicates `@endpoint(kind: server)` and `http` route registration.

- **Default:** emission is **off** unless **`VOX_EMIT_EXPRESS_SERVER=1`** is set in the environment when running codegen (e.g. `vox build`). The supported browser/TS client against **Axum** is **`vox-client.ts`** from TypeScript codegen (Contract IR); Rust codegen **does not** emit **`api.ts`** ([`vox-codegen` `emit/mod.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/mod.rs) sets `api_client_ts` to empty).
- **Use case for `VOX_EMIT_EXPRESS_SERVER=1`:** Node-only demos, tests, or containers that intentionally run `npx tsx server.ts` instead of the Rust binary.

## Container images

[`vox_deploy_codegen::generate::generate_default_dockerfile`](../../../crates/vox-deploy-codegen/src/generate.rs) is **Rust-first**: **`FROM debian:bookworm-slim`**, **`COPY vox-app`**, **`CMD ["/app/vox-app"]`** (place the release binary from `vox bundle` / `cargo build --release` in `target/generated` into the build context as **`vox-app`**). **`@environment`** blocks and hand-authored Dockerfiles remain the place for a **Node + `npx tsx server.ts`** lane (requires **`VOX_EMIT_EXPRESS_SERVER=1`** at codegen). See [how-to-deploy.md](../how-to/how-to-deploy.md).

## Axum JSON error envelope (API handlers)

Normative shape: [Wire Format v1 SSOT §6](../architecture/wire-format-v1-ssot.md#6-error-envelope) — `{ ok: false, code, message, request_id?, details? }`, implemented in **`vox-http-envelope`** and emitted from generated Axum handlers for serialization failures, query decode failures, and mutation DB errors where applicable.

Legacy paths may still return ad hoc JSON on some branches; prefer the envelope for new handlers and uniform client parsing (`vox-client.ts` error bodies).

## Optional: v0 and external frontends

- **`@v0`** — TSX on disk under `out_dir`; named `export function` required for `routes {` imports ([`v0_tsx_normalize.rs`](../../../crates/vox-cli/src/v0_tsx_normalize.rs)).
- **External React frontend** — see [architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md). Vox emits plain React components from `component` declarations and a typed `vox-client.ts` from `@endpoint` declarations; an external React/TanStack/mobile app imports them directly. Islands were retired 2026-05-03.

## Related

- [TanStack SSR with Axum](../how-to/tanstack-ssr-with-axum.md) — `VOX_SSR_DEV_URL`, `VOX_ORCHESTRATE_VITE`.
- [ref-cli.md](cli.md) — CLI surface.


