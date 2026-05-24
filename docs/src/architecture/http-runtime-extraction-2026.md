# HTTP runtime extraction — ADR-041 §6(c) deferral & design

**Status:** Deferred (research / design constraints recorded).
**Date:** 2026-05-23.
**Owner:** codegen.
**Tracks:** ADR-041 §6(c) — "factor HTTP server boot into a reusable runtime
function so `emit_main_boot` can call it instead of emitting a TODO."

## TL;DR

Extracting a `vox-http-runtime::serve(db, hir_module) -> ServerHandle` symbol
that `emit_main_boot` could call is **a multi-week refactor of how codegen
emits routes**, not a one-day shim. The HTTP wiring in `emit_main` (see
`crates/vox-codegen/src/codegen_rust/emit/http.rs`) is deeply tied to the
per-binary code generation pass: each `@query` / `@mutation` / `@server`
function emits a *separate* named handler function, per-route static items
(rate-limit governors), and inlined middleware. There is no shape of
"hand the HIR to a runtime function and it serves itself" without either:

1. Writing a HIR interpreter that serves routes at runtime, or
2. Refactoring route emission to register `Box<dyn Fn>` closures with a
   runtime registry (similar to how `ActorRegistry` works post-§5.2).

Either path is a significant architectural change. §6(c) is **deferred**
pending one of them being scheduled. The current `emit_main` (axum-inlined)
remains the production path; `emit_main_boot` (durable-only) remains an
HTTP-less alternative still gated behind future wiring.

## What recon found

### 1. `emit_main_boot` is not yet wired into production codegen

Searching the workspace shows the **only callers of `emit_main_boot` are
tests** (`tests/main_boot_snapshot.rs`, `tests/main_boot_hir_roundtrip.rs`).
Production codegen — both `generate_axum_local_server` and
`generate_tauri_workspace` in `emit/mod.rs` — invokes `emit_main` (from
`http.rs`), which emits a single `main()` that combines DB init, HIR
register (after §6(b)), and HTTP serve all inline.

This means the `TODO(phase5-followup)` in `main_boot.rs:100` is **not a
runtime hole experienced by users today**. It is a hole in the
*durable-only* alternative `main()` that has not yet been promoted to be
the production entry point. The original ADR-041 §6(c) wording reads as
if `emit_main_boot` is in production and silently skips HTTP; in fact it
is a snapshot-tested alternative still awaiting integration.

### 2. The HTTP emit shape

`emit_main` in `http.rs` (462 LoC of emission logic) does five things that
all need a home if HTTP were factored to a runtime crate:

| Concern | How it's emitted today | Why it can't trivially move to a runtime |
|---|---|---|
| Per-handler functions (`handle_q_foo`, `handle_m_bar`, `handle_sf_baz`) | Emitted as free `async fn`s in `main.rs`, with HIR body lowered to Rust via `emit_stmt` | Handler bodies are *code-generated* from the HIR statement tree with `inferred_types` baked in. They cannot exist before codegen. |
| Per-route rate-limit guards | Each `@endpoint` with `rate_limit(by: ip)` emits a `static VOX_RL_<name>: OnceLock<...>` + a `vox_rl_guard_<name>` middleware fn | The static item key is per-route, not per-module. A runtime registry could replace this, but the emit currently hard-codes the static name into the `.layer(...)` call. |
| Per-route CORS layers | Each `@endpoint(cors: ...)` emits a literal `CorsLayer::new()...` chain inline at route registration | Could move to a CORS-policy struct + runtime helper, but the emit currently inlines the literal. |
| Dev proxy + SPA fallback (`serve_dispatch`, `serve_embedded`, `Assets` `rust_embed::Embed`) | All free items in `main.rs` | These *could* move to a runtime crate cleanly — they are not per-route. |
| Router assembly (`Router::new().route(path, post(handler)).layer(...)`) | Inlined into `main()` | Could move to a runtime helper if handlers were registered, not referenced by name. |

The last two items are roughly **200 lines of pure boilerplate** that
*could* move to a runtime crate today. The first three are **route-shape
state** that needs codegen ↔ runtime negotiation.

### 3. Why a thin shim doesn't help much

A "Path B" shim — e.g., a `vox_http_runtime::serve_with(router, port) -> Handle`
function that just wraps the listener bind + axum::serve + graceful
shutdown — would deduplicate maybe 30 lines from `emit_main` but would
**not** let `emit_main_boot` call `serve(db)` because `emit_main_boot`
still doesn't know how to construct the `Router` from the HIR. The
`Router` construction is the substantial part, not the bind/serve.

A Path-B shim is worth doing if and only if we keep both `emit_main` and
`emit_main_boot` alive and want them to share infrastructure. Since the
plan of record is to converge them (see §"Two-phase plan" below), the
Path-B shim is best done *as part of* the convergence rather than as a
prelude that gets rewritten.

## Two-phase plan when this is scheduled

### Phase 1 — Route registration via Box<dyn Fn>

Restructure the emit so each `@endpoint` function lowers to:

1. A trait-shaped handler signature (`async fn(db, request) -> Response`)
   in the generated `lib.rs` (already most of the way there — handlers
   currently take `Extension(db)` + `Json<request>`).
2. A **registration closure** generated alongside, e.g.
   `register_endpoints(router: &mut Router) { router.route("/api/...", post(...)); ... }`.
3. The new `vox_http_runtime::serve(db, register_fn, config)` accepts
   the registration closure, builds the `Router`, applies global
   middleware (request-id, tracing, CORS-default), and serves.

Per-route CORS + rate-limit *layers* stay in the emitted code (they are
part of the registration closure), so the runtime crate doesn't need to
understand per-route policy.

**Estimated LoC:** ~300 in `vox-http-runtime` (new L3 crate, depends on
axum/tower-http/governor); ~150 LoC delta in `http.rs` to emit the
registration closure rather than inlining the `Router::new()` chain.

### Phase 2 — Converge `emit_main` and `emit_main_boot`

Once Phase 1 lands, `emit_main_boot`'s HTTP step #4 becomes:

```rust
let http_handle = vox_http_runtime::serve(db.clone(), register_endpoints, http_config).await?;
```

and `emit_main` is retired in favor of `emit_main_boot` as the single
production `main()` emitter for the axum local-server shell. Tauri keeps
its own emit (it does not use axum at runtime).

**Estimated LoC:** -400 LoC in `http.rs` (removing the inlined `Router`
build); +100 LoC in `main_boot.rs` (HTTP-step wiring).

## What lives at §6(c) today

- `emit_main_boot` (`main_boot.rs:34`) emits a `TODO(phase5-followup)`
  comment in step 4 instead of HTTP wiring. **This is harmless** because
  `emit_main_boot` is not on the production path.
- `emit_main` (`http.rs:129`) continues to emit a fully inlined HTTP
  server in `main()`. **This is what runs today.**
- ADR-041 §6(c) now points to this design doc and is marked `DEFERRED`.

## Acceptance criteria for closing §6(c)

Either:

1. **Phase 1 of the two-phase plan ships** — a `vox-http-runtime` crate
   exists with a callable `serve(db, register_fn, config)` symbol used
   by both `emit_main_boot` and (eventually) `emit_main`. **OR**

2. **`emit_main_boot` is retired** in favor of staying with the inlined
   `emit_main` permanently, in which case §6(c) is closed as "WONTFIX —
   inlined emit is the chosen shape" and the TODO comment in
   `main_boot.rs` is removed alongside `emit_main_boot` itself.

## References

- ADR-041 §6(c) — `docs/src/adr/041-durable-functions-completion-2026.md`
- Current HTTP emit — `crates/vox-codegen/src/codegen_rust/emit/http.rs`
- Current durable boot emit — `crates/vox-codegen/src/codegen_rust/emit/main_boot.rs`
- Snapshot tests locking the current shape —
  `crates/vox-codegen/tests/main_boot_snapshot.rs`,
  `crates/vox-codegen/tests/main_boot_hir_roundtrip.rs`
- ActorRegistry pattern (the model for runtime-registered handlers) —
  `crates/vox-actor-runtime/src/registry.rs` (P5.2 landing)
