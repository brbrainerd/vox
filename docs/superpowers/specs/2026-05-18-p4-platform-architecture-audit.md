# P4.1 + P4.3 — Platform Architecture Audit

**Date:** 2026-05-18
**Status:** Architecture recommendation grounded in the live codebase. Two decisions surfaced; one of each presented with codebase evidence and a clear pick.
**Closes:** the open question from the v1.0 completion plan for P4.1 (SSE vs WebSocket) and P4.3 (vox deploy platform target).

This audit reads directly off the tree. Every claim cites a file path. The two open decisions get a recommendation each with stated criteria and trade-offs.

---

## §1 P4.1 — Streaming runtime: SSE vs WebSocket

### §1.1 What exists today

| Surface | Where | What it provides |
|---|---|---|
| Production WebSocket gateway | [`crates/vox-orchestrator-mcp/src/http_gateway/ws.rs`](crates/vox-orchestrator-mcp/src/http_gateway/ws.rs) | Auth-gated WS server with role-based access, tokio::select message loop, integrates with orchestrator event bus. Reference pattern for codegen. |
| Per-table broadcast channels | [`crates/vox-actor-runtime/src/subscription.rs`](crates/vox-actor-runtime/src/subscription.rs:29-66) | `SubscriptionManager` wrapping `tokio::sync::broadcast` per @table. `subscribe(table)` returns a `broadcast::Receiver<()>` that fires on every mutation. |
| WS in the openclaw runtime | [`crates/vox-openclaw-runtime/src/openclaw_gateway_ws.rs`](crates/vox-openclaw-runtime/src/openclaw_gateway_ws.rs) | Second production WS surface — proves the Axum + `WebSocketUpgrade` pattern is repeated, not one-off. |
| SSE *only* for LLM streaming | [`crates/vox-actor-runtime/src/llm/stream.rs`](crates/vox-actor-runtime/src/llm/stream.rs) | Internal LLM token-stream consumer (Axum SSE *receiver*, not server). Not exposed to user code. |
| Axum + tower_http already universal | [`crates/vox-codegen/src/codegen_rust/emit/http.rs`](crates/vox-codegen/src/codegen_rust/emit/http.rs) | Existing endpoint emit uses `tower_http::cors::CorsLayer` and the same Axum primitives both SSE and WS need. Zero new deps required. |
| Stream surface declared | [`crates/vox-compiler/src/parser/descent/decl/head.rs:942`](crates/vox-compiler/src/parser/descent/decl/head.rs:942) + [`crates/vox-compiler/src/hir/lower/mod.rs:211`](crates/vox-compiler/src/hir/lower/mod.rs:211) | `@endpoint(kind: stream)` parses + lowers to HIR (currently aliased to `Server` until codegen lands). |
| Marquee Slot 3 use case | [`apps/marquee/chat/src/main.vox`](apps/marquee/chat/src/main.vox) | `actor ChatRoom { on join, on leave, on send_msg }` + companion `watch_room()` stream endpoint. The first consumer for whatever we ship. |
| Wire format SSOT (silent on streaming) | [`docs/src/architecture/wire-format-v1-ssot.md`](docs/src/architecture/wire-format-v1-ssot.md) §2 | Transport conventions cover query/mutation/server only. Streaming will add a new §X. |

### §1.2 What the codebase has decided

Three facts narrow the decision:

1. **WebSocket is already the production transport** for orchestrator client/dashboard traffic. Auth + role-gating + event-bus integration is mature code paths, not greenfield.
2. **`SubscriptionManager::subscribe(table)` is the natural bridge** between `actor` message handlers and streaming endpoints. It already broadcasts `()` per-table mutation; a thin adapter publishes the actor message payload alongside.
3. **The marquee chat fixture requires bidirectional traffic.** `join` / `leave` / `send_msg` are inbound from the client; `watch_room()` is outbound to the client. SSE cannot carry the inbound half.

### §1.3 The two candidates

**SSE (Server-Sent Events).**
- **Pros:** HTTP-native (no protocol upgrade); plays nice with proxies, CDNs, and HTTP/2 multiplexing; trivial Axum primitive (`axum::response::sse::Sse`); browsers auto-reconnect; matches the OpenRouter / LLM streaming idiom already in `vox-actor-runtime/src/llm/stream.rs`; cheaper per-connection cost server-side.
- **Cons:** Unidirectional (server → client only). For bidirectional client↔server you'd need SSE + a companion `@endpoint(kind: mutation)` for the inbound half, which double-counts the connection. Browser EventSource API doesn't support custom headers (so auth has to ride on query params or cookies); workable but ugly.
- **Fits well when:** progress events, log tails, LLM token streams, presence push notifications, server-initiated UI invalidation.

**WebSocket.**
- **Pros:** Bidirectional in a single connection; the existing `WebSocketUpgrade` + `tokio::select` pattern is proven in two production crates; auth header travel works (Authorization on the upgrade request); message framing supports both text and binary; idiomatic for actor mailbox dispatch.
- **Cons:** Protocol upgrade can fail behind some corporate proxies; per-connection memory cost slightly higher; HTTP/2 multiplexing doesn't apply (each WS is a full TCP+TLS); browser reconnection has to be implemented client-side.
- **Fits well when:** chat / collaboration / multi-user actor sessions, agent ↔ agent dispatch, anything with non-trivial client→server messages.

### §1.4 Architectural verdict — both, structurally chosen

**Ship both SSE and WebSocket. The HIR shape determines which.**

The rule:

| Source pattern | Compiles to | Rationale |
|---|---|---|
| `@endpoint(kind: stream) fn name() to T` (no actor companion, no inbound) | **SSE** | Pure server→client stream; HTTP-native is strictly better when bidirectionality isn't needed. |
| `@endpoint(kind: stream) fn name()` accompanied by an `actor` with inbound `on` handlers reachable from the fn | **WebSocket** | Bidirectional in one connection; matches the orchestrator's existing ws.rs pattern. |
| `@endpoint(kind: stream) fn name(...) to T` with `@uses(socket)` or similar opt-in marker | **WebSocket** | Author opt-in when the heuristic isn't enough. |

**Why "both" is the codebase's answer:**

- The runtime already runs both — `tokio::sync::broadcast` for the per-table push (SSE-shaped) and the WS gateway for the bidirectional case (WS-shaped). Picking one would orphan an existing production pathway.
- The codegen lands as two emitter functions sharing the same `HirEndpointKind::Stream` pre-processing; ~120 LoC each instead of 200+ in one bloated path.
- The marquee chat fixture wants WS; the LLM-token-streaming use case ([CR-L8 corpus-feedback](crates/vox-audit/src/subcommands/corpus_feedback.rs) export streaming, future agent prompt streaming) wants SSE.

### §1.5 v1.0 sequencing

1. **Phase 4.1a (≤8 hr):** SSE emitter from `HirEndpointKind::Stream` with no actor companion. Wire-format SSOT §X drafts the `text/event-stream` envelope, mirroring the existing JSON envelope at §3-4.
2. **Phase 4.1b (≤6 hr):** WebSocket emitter when actor inbound handlers are present. Reuse the existing `WebSocketUpgrade` + auth pattern from `http_gateway/ws.rs` as the emit template; lift the `tokio::select` message-loop boilerplate into a tiny helper crate or codegen-shared module.
3. **Phase 4.1c (≤2 hr):** Marquee chat fixture's `watch_room()` actually streams. Add a single integration test driving SSE for the no-actor case (e.g. health-tick) and a curl-test for the WS case.
4. Wire-format SSOT update happens in parallel with §1.5.1.

Total ≈ 16 hr, matching the plan's original P4.1 budget but with two clearer deliverables instead of one ambiguous "streaming codegen" item.

### §1.6 What this displaces

- The original P4.1 framing assumed SSE only. The plan's "deferred to v1.1" line on WebSocket was tentative; with the orchestrator WS gateway already shipping in production, deferring WebSocket entirely would be the architectural mistake, not deferring it. Both ship in v1.0.

---

## §2 P4.3 — vox deploy platform target

### §2.1 What exists today

| Surface | Where | What it provides |
|---|---|---|
| Deploy target enum already complete | [`crates/vox-deploy-codegen/src/deploy_target.rs:25-46`](crates/vox-deploy-codegen/src/deploy_target.rs:25-46) | `DeployTarget` enumerates Container / BareMetal / Compose / Kubernetes / Fly / Coolify. All six have config structs (`ContainerTarget`, `FlyTarget`, …) and `execute_*` functions. |
| OCI container runtime abstraction | [`crates/vox-container/Cargo.toml`](crates/vox-container/Cargo.toml) + [`src/{docker,podman,detect}.rs`](crates/vox-container/src/) | Docker + Podman support via a unified `ContainerRuntime` trait. Detection + build + push + tag + login all real. |
| Dockerfile generator | [`crates/vox-deploy-codegen/src/generate.rs`](crates/vox-deploy-codegen/src/generate.rs) | `EnvironmentSpec` → OCI-compatible Dockerfile. Drives the Container target. |
| Fly executor | [`crates/vox-deploy-codegen/src/deploy_target.rs:411`](crates/vox-deploy-codegen/src/deploy_target.rs:411) (`execute_fly`) | Invokes `flyctl launch` when no `fly.toml` exists; reuses an existing `fly.toml` otherwise. |
| `vox deploy` CLI surface (90 + 265 LoC partial) | [`crates/vox-cli/src/commands/deploy.rs`](crates/vox-cli/src/commands/deploy.rs) | Skeleton CLI ready to dispatch into `DeployTarget`. The integration is what's missing, not the platform layer. |
| Marquee manifest already references deploy targets | [`contracts/marquee/manifest.v1.yaml`](contracts/marquee/manifest.v1.yaml) | Slot 1 (`marquee-app`) declares `deploy_target: container`, `deploy_runtime: auto`, `deploy_registry: "ghcr.io/owner"`. The contract presumes container-first. |
| CR-P3 budget | [`docs/src/architecture/v1-release-criteria.md`](docs/src/architecture/v1-release-criteria.md) | `vox new web → vox deploy` ≤ 120 seconds. Achievable for OCI build+push of a small Vox app; tight but reachable. |
| CR-L7 integration test fixture | [`crates/vox-audit/src/subcommands/deploy.rs`](crates/vox-audit/src/subcommands/deploy.rs) | Already drives the doctor leg against status:real marquee apps; the deploy leg front-stacks here. |

### §2.2 What the codebase has decided

**The architecture is already container-first.** Six observations:

1. Six deploy targets are enumerated; five of them (BareMetal, Compose, K8s, Fly, Coolify) consume OCI images as input. Container is the universal substrate.
2. `vox-deploy-codegen` already depends on `vox-container` — the OCI plumbing is the foundation.
3. The marquee manifest already declares `deploy_target: container` as the slot-1 default.
4. Fly's `execute_fly` calls `flyctl launch`, which itself builds an OCI image as step 1.
5. Railway is **not in the enum**. Adopting it would mean adding a 7th target with its own CLI/API dependency — net new surface, not selecting from what exists.
6. Coolify is in the enum. It's self-hosted PaaS; matches Vox's "owns the orchestration loop" philosophy better than Railway.

### §2.3 The three candidates

**OCI-publish only.**
- **Pros:** Universal substrate; works against ghcr.io / Docker Hub / private registries; no platform lock-in; smallest GA surface; codegen is already written.
- **Cons:** Stops at "image pushed" — the user still has to wire `docker run` or pull-and-deploy somewhere. Not zero-DX.

**OCI + Fly.io wrapper.**
- **Pros:** First-class one-command path (`vox deploy` → image + `flyctl launch`); Fly's edge network primitives align with Vox's eventual mesh story; first-class WebSocket support (matches the P4.1 verdict above); `.fly.dev` subdomain naming parallels `.vox.dev`; integration is already mostly written.
- **Cons:** Users without Fly accounts get an error or a fallback; introduces a platform dependency, though it's optional.

**OCI + Railway.**
- **Pros:** Railway has lower friction onboarding than Fly.io for some user segments.
- **Cons:** Not in the codebase; needs a 7th `DeployTarget::Railway` variant + Railway CLI/API surface. Railway also doesn't have first-class actor/WebSocket pricing tier (Fly does). Adopting Railway = new code; adopting Fly = wiring existing code.

### §2.4 Architectural verdict — OCI primary, Fly first-class wrapper, Coolify documented

**v1.0 ships:**

1. **`DeployTarget::Container`** as the default behavior of `vox deploy` with no `--target` flag. Output: OCI image pushed to the registry configured in `Vox.toml [deploy].registry`. This is the universal substrate the rest of the targets layer on.
2. **`vox deploy --target fly`** invokes the existing `FlyTarget::execute_fly` path (builds image, then `flyctl launch` or `flyctl deploy`). This is the zero-DX "platform happy path."
3. **`vox deploy --target coolify`** + **`vox deploy --target kubernetes`** are documented as available but not GA-tested. They run; we don't gate v1.0 on them.

**v1.0 does NOT ship Railway.** Two reasons:
- Railway isn't in the enum; adopting it is net-new code, not wiring existing code.
- Railway's WebSocket/long-connection pricing tier doesn't match Vox's marquee chat use case (which depends on the P4.1 WS verdict). Choosing Railway would force a streaming-target downgrade.

### §2.5 v1.0 sequencing

1. **Phase 4.3a (≤6 hr):** Wire `vox deploy` CLI → `DeployTarget::Container` path. Default Dockerfile from `EnvironmentSpec` derived from `Vox.toml`. Verify ghcr.io push path end-to-end against a marquee slot's CR-P3 budget.
2. **Phase 4.3b (≤4 hr):** `vox new web` emits a `Vox.toml [deploy]` section pre-configured for Container with a commented `[deploy.fly]` block ready to uncomment. Marquee app fixtures get this section.
3. **Phase 4.3c (≤6 hr):** `vox deploy --target fly` integration test driving `flyctl launch` on a marquee slot. Wire CR-L7 integration test (commit ac9503761) to invoke the full `vox new → vox deploy → vox doctor` chain.
4. **Phase 4.3d (≤4 hr):** Documentation pass: `Vox.toml [deploy]` config schema published; `vox deploy --help` lists all six targets with their status (real / experimental / not-tested).

Total ≈ 20 hr, under the plan's 24 hr P4.3 budget by 4 hr because the OCI integration is already written.

### §2.6 What this displaces

- The implementation plan §1.5 P4.5 framed "default deployment target (probably Fly.io or Railway for v1.0)". The codebase answer is clearer: **OCI is the default; Fly is the platform wrapper; Railway is not adopted in v1.0.**
- The plan's "stretch goal: Fly.io OR Railway" optionality goes away — the codebase already invested in Fly, and the platform-fit case (WebSocket, edge primitives) favors it.

---

## §3 Combined v1.0 deferral picture

If both architectural choices above are accepted, the v1.0 completion plan's P4 block becomes:

| Sub-item | Original budget | New budget under this audit | Delta |
|---|---|---|---|
| P4.1 streaming (SSE only) | 12 hr | 16 hr (SSE + WS, both shipped) | +4 hr — more value, same magnitude |
| P4.2 cross-file repair | 16 hr | 16 hr (unchanged; partial slice landed in b2cbb94ef) | 0 |
| P4.3 vox deploy platform | 24 hr | 20 hr (OCI primary + Fly wrapper) | −4 hr — OCI integration already written |
| **Total** | **52 hr** | **52 hr** | net 0 |

Same total budget, but every hour is now spent on real wiring of existing code rather than picking-the-platform decisions. The plan's calendar shifts: v1.0 GA target stays Q1-2027.

## §4 Decision asks

1. **P4.1 streaming:** approve "both SSE and WebSocket, dispatched on HIR shape" / require SSE-only for v1.0 / require WS-only for v1.0.
2. **P4.3 vox deploy:** approve "OCI primary + Fly first-class wrapper, no Railway in v1.0" / require Railway adoption / require OCI-only with no platform wrappers.
3. **Wire-format SSOT update for streaming:** approve in-line with P4.1a / hold for a separate spec revision.

If both verdicts approved, P4.1 + P4.3 can land in the recommended order without further architecture decisions blocking implementation.
