---
title: "Dead Crate Deep Dive (2026-05-08)"
description: "Code-level investigation of uncertain DEAD crates: what's unique, what's obviated, where unique code should be preserved."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Detailed per-crate analysis of preservation vs deletion decisions, useful for plugin migration work."
---

# Dead Crate Deep Dive (2026-05-08)

Companion to `dead-crate-fate-plan-2026-05-08.md`. This document covers the five uncertain crates
that required code-level inspection before a fate could be assigned. Investigation was performed
on branch `claude/infallible-lalande-baf300`.

---

## Summary table

| Crate | LOC | Current consumers | Revised fate | Effort |
|---|---|---|---|---|
| `vox-tools` | 580 | 0 external | DELETE (audio impl) + MOVE (chat shim) | S |
| `vox-mcp-meta` | 62 | vox-corpus (1) | KEEP (thin; genuinely used) | XS |
| `vox-mcp-registry` | 15 | vox-orchestrator, vox-corpus, vox-cli (3) | KEEP (build-time SSOT; load-bearing) | XS |
| `vox-scientia-core` | 37 | vox-cli (indirect) | DELETE (pure facade; add mod to vox-publisher) | XS |
| `vox-scientia-social` | 14 | 0 | DELETE | XS |
| `vox-scientia-ingest` | 142 | vox-cli (1) | EXTRACT-TO-PLUGIN as `vox-plugin-publication` | S |
| `vox-ssg` | 183 | vox-cli build command (1) | KEEP as CORE (compiler-layer, not publication) | XS |
| `vox-webhook` | 1460 | 0 | EXTRACT-TO-PLUGIN as `vox-plugin-webhook` | M |

---

## vox-tools

### Files and LOC

```
src/lib.rs                          ~358 lines  (DirectToolExecutor + oratio dispatch)
src/mens_chat.rs                    ~151 lines  (chat_tool_definitions, execute_tool_calls, ToolCall)
src/capability_registry/mod.rs      ~71 lines   (CapabilityDescriptor, CapabilityRegistry, default_registry)
```

### Public API

- `DirectToolExecutor` — in-process executor for `vox_oratio_{transcribe,status,listen}`
- `mens_chat::chat_tool_definitions()` — OpenAI-style function-call descriptors for Mens chat
- `mens_chat::execute_tool_calls()` — runs model tool-call outputs through the executor
- `mens_chat::fallback_tool_system_prompt()` — `<TOOL_CALLS>` XML fallback for models without native tool support
- `mens_chat::parse_fallback_tools()` — parses `<TOOL_CALLS>` blocks back to `ToolCall` structs
- `capability_registry::{CapabilityDescriptor, CapabilityRegistry, default_registry, mens_chat_parameters, capability_to_openai_function}` — local copy of types from `vox-capability-registry`

### Dependencies

`anyhow`, `serde_json`, `vox-capability-registry`, `vox-oratio`, `vox-plugin-host`

### What is unique

**Nothing in the audio dispatch path is unique.** `vox-orchestrator/src/mcp_tools/oratio_tools.rs`
implements `transcribe`, `listen`, and `status` (matching the same three MCP tool names) and is
wired into the live MCP dispatch table at `dispatch.rs:1069–1073`. `vox-tools`' `DirectToolExecutor`
covers the same three tools for in-process callers (Mens chat) with slightly different argument
handling (plugin-host path for non-.txt audio, emit_asr_refine_path sidecar).

The `capability_registry/mod.rs` inside `vox-tools` is a **local copy** of types from the live
`vox-capability-registry` crate (which `vox-tools` already imports via `Cargo.toml`). The local
copy registers only `vox_oratio_transcribe`; the real registry has the full surface. This local
copy adds no value and drifts silently.

**What is genuinely useful:** `mens_chat.rs` — specifically `fallback_tool_system_prompt()` and
`parse_fallback_tools()`. These implement an `<TOOL_CALLS>` XML-tag fallback for LLM providers
that don't support native function calling. This is not present in `vox-orchestrator/src/mcp_tools/`.

### What is obviated

- `DirectToolExecutor` and the `transcribe_path_via_plugin` private function: fully covered by
  `vox-orchestrator`'s `oratio_tools.rs`. The in-process path for Mens chat should call
  `vox-oratio` directly rather than re-implementing dispatch.
- `capability_registry/mod.rs`: delete; use `vox-capability-registry` directly.

### Recommendation

**DELETE** `vox-tools` as a crate. **MOVE** `mens_chat.rs` (excluding `execute_tool_calls` and
`chat_tool_definitions`, which are thin wrappers that can be regenerated from `vox-capability-registry`)
or — if the `<TOOL_CALLS>` fallback is still used — move `fallback_tool_system_prompt` and
`parse_fallback_tools` into `vox-orchestrator/src/mcp_tools/chat_tools/` as a new
`fallback_format.rs` module.

**Effort: S.**

#### Migration steps

1. Grep for callers of `vox_tools::mens_chat::fallback_tool_system_prompt` and
   `parse_fallback_tools`. If zero callers: delete the file and declare done.
2. If callers exist: move those two functions to
   `vox-orchestrator/src/mcp_tools/chat_tools/fallback_format.rs`, re-export from
   `chat_tools/mod.rs`, update call sites.
3. Delete `src/capability_registry/mod.rs` (superseded by `vox-capability-registry`).
4. Delete `src/lib.rs` (DirectToolExecutor).
5. Remove `vox-tools` from `Cargo.toml` workspace members and any binary that lists it.

---

## vox-mcp-meta + vox-mcp-registry

These two crates form a two-layer SSOT stack; they must be evaluated together.

### vox-mcp-registry (15 lines + build.rs)

Reads `contracts/mcp/tool-registry.canonical.yaml` at compile time and code-generates a static
`TOOL_REGISTRY: &[McpToolRegistryEntry]` array. The `McpToolRegistryEntry` type carries
`name`, `description`, `product_lane`, `http_read_role_eligible`, `tier`.

**Consumers:** `vox-orchestrator` (imports `TOOL_REGISTRY` as its canonical tool list, with a
compile-time test that every entry has a `match` arm in dispatch), `vox-corpus` (drives
synthetic-training-data generation), `vox-cli` (`capability_sync` + `mcp_wiring` CI checks).

This crate is **load-bearing**. The orchestrator's dispatch test enforces that `TOOL_REGISTRY`
and the `handle_tool_call` match arms stay 1:1. Deleting it would require inlining a 100+-entry
registry into three separate crates or picking a new SSOT mechanism. **KEEP.**

### vox-mcp-meta (62 lines)

A thin constants crate: re-exports `TOOL_REGISTRY` from `vox-mcp-registry`, and adds three
static string-slice arrays: `A2A_MESSAGE_TYPES` (19 entries), `SKILL_TOOLS` (6 entries),
`ORCHESTRATOR_TOOLS` (17 entries).

**Consumers:** `vox-corpus` uses `vox_mcp_meta::TOOL_REGISTRY` (aliased as `TOOL_REGISTRY_SLIM`
in `synthetic_gen/mod.rs:33`) and the `A2A_MESSAGE_TYPES`, `ORCHESTRATOR_TOOLS`, `SKILL_TOOLS`
constants in training-data generation and tests.

`vox-corpus` could import `TOOL_REGISTRY` directly from `vox-mcp-registry`; the 42 lines of
constant arrays (`A2A_MESSAGE_TYPES` etc.) are the only addition. **However**, because the crate
is only 62 lines and is actively used, the correct call is to **KEEP** it as-is rather than
inline those constants into `vox-corpus` (which would make `vox-corpus` the SSOT for message
type strings — wrong).

### Comparison against vox-orchestrator/src/mcp_tools/

The orchestrator's `mcp_tools/` directory is the **runtime** layer: tool dispatch, input schemas,
RMCP server wiring, oratio tools, etc. It imports `TOOL_REGISTRY` from `vox-mcp-registry` as its
SSOT. There is no semantic overlap between `vox-mcp-meta` (compile-time constants for corpus
generation) and `mcp_tools/` (runtime dispatch).

### Recommendation

**KEEP both.** Neither is dead in the meaningful sense; both are consumed by distinct subsystems.
The earlier "DEAD" audit appears to have missed the `vox-corpus` dependency on `vox-mcp-meta`.

If there is future cleanup appetite, the `A2A_MESSAGE_TYPES` / `SKILL_TOOLS` / `ORCHESTRATOR_TOOLS`
constants in `vox-mcp-meta` could migrate into `vox-mcp-registry`'s generated output
(the YAML could grow enum sections), collapsing the two crates into one. **Not worth doing now.**

**Effort to keep: XS (no action required).**

---

## vox-scientia consolidation plan

### Per-crate inventory

#### vox-scientia-core (37 lines)

A **pure facade**. Every public symbol is a `pub use vox_publisher::scientia_*::*` re-export:
`contracts`, `discovery`, `evidence`, `finding_ledger`, `heuristics`, `prior_art`, `worthiness`.
The implementation lives entirely in `vox-publisher`. The stated purpose (avoiding a dependency
cycle) is documented in the crate's own module comment, but the cycle is already broken by the
fact that `vox-scientia-core` depends on `vox-publisher` — not the other way around. In practice
this crate adds a name-path indirection (`vox_scientia_core::contracts::*` vs
`vox_publisher::scientia_contracts::*`) with zero other benefit.

**Consumers:** None found via `Cargo.toml` scan (vox-cli lists `vox-scientia-ingest`, not
`vox-scientia-core`).

#### vox-scientia-social (14 lines)

A **pure facade** around two `vox_publisher` functions:
`compile_for_publish` (aliased as `compile_distribution_preview`) and
`distribution_compile::validate_topic_pack_projection_profiles`.

**Consumers:** Zero.

#### vox-scientia-ingest (142 lines)

The only scientia crate with **real, unique implementation**:

- `FeedCrawler` (`rss_crawler.rs`, ~75 lines) — async RSS/Atom crawler using `feed-rs` + `reqwest`;
  fetches `FeedSource` URLs, normalizes entries to `InboundItem`.
- `IngestDeduplicator` (`deduplicator.rs`, ~44 lines) — semantic deduplication via
  `vox-search::EmbeddingService`; checks cosine similarity against the
  `scientia_external_intelligence` table in `VoxDb`.
- Types: `FeedSource`, `InboundItem`.

**Unique dependencies:** `feed-rs = "2.1"` (RSS parser, not present elsewhere in workspace).

**Consumer:** `vox-cli` (1 direct dependency, line 135 of `vox-cli/Cargo.toml`).

### vox-publisher's scientia modules

`vox-publisher/src/` contains: `scientia_contracts.rs`, `scientia_discovery.rs`,
`scientia_evidence/`, `scientia_finding_ledger.rs`, `scientia_heuristics.rs`,
`scientia_prior_art.rs`, `scientia_worthiness_enrich.rs`. These are the **provenance and
scholarly-publication layer**: citation tracking, evidence evaluation, prior-art checks,
publication-worthiness scoring. They are distinct from the **ingest layer** (RSS crawling,
deduplication).

### Duplication map

There is no type-level duplication between `vox-scientia-ingest` and `vox-publisher`. They operate
on different layers: ingest brings raw external items in, publisher evaluates and routes them out.
`vox-scientia-core` and `vox-scientia-social` are pure facades with no original types.

### Proposed consolidation

**Step 1 — Delete the two facades.**

- Delete `vox-scientia-core`. Any downstream that used its re-exported paths should import from
  `vox-publisher` directly (paths already exist).
- Delete `vox-scientia-social`. It has zero consumers.

**Step 2 — Extract `vox-scientia-ingest` into `vox-plugin-publication`.**

The user's direction is: full publication capability should become a plugin. The ingest layer
(`FeedCrawler` + `IngestDeduplicator`) plus the publisher's distribution and scholarly submission
surface (`compile_for_publish`, `distribution_compile`, zenodo/crossref adapters) belong together
as a publication plugin.

**Proposed crate name:** `vox-plugin-publication`

**What the plugin owns:**

- `feed_crawler.rs` — move from `vox-scientia-ingest/src/rss_crawler.rs`
- `ingest_dedup.rs` — move from `vox-scientia-ingest/src/deduplicator.rs`
- `types.rs` — `FeedSource`, `InboundItem` (from ingest); `UnifiedNewsItem` (from publisher types)
- Plugin entry point: `PluginPublication` implementing `VoxPlugin` (or equivalent trait)
- Feature flags: `scholarly` (zenodo/crossref/openreview), `social` (Reddit/YouTube syndication),
  `ssg` — see vox-ssg section below

**What stays in vox-publisher:**

The scientia provenance types (`scientia_contracts`, `scientia_evidence`, etc.) are not
publication-as-output; they are scholarly metadata used for citation and prior-art tracking.
They remain in `vox-publisher` as a library crate. If `vox-publisher` itself is being deleted,
migrate these modules into `vox-plugin-publication/src/scholarly/`.

**Migration checklist:**

1. Create `crates/vox-plugin-publication/` with its `Cargo.toml` (deps: `vox-db`, `vox-search`,
   `feed-rs`, `reqwest`, `serde`, `tokio`, `tracing`, `anyhow`).
2. Move `vox-scientia-ingest/src/rss_crawler.rs` → `vox-plugin-publication/src/feed_crawler.rs`.
3. Move `vox-scientia-ingest/src/deduplicator.rs` → `vox-plugin-publication/src/ingest_dedup.rs`.
4. Move `FeedSource`, `InboundItem` into `vox-plugin-publication/src/types.rs`.
5. Update `vox-cli/Cargo.toml` to depend on `vox-plugin-publication` instead of
   `vox-scientia-ingest`; update import paths at the call site.
6. If `vox-publisher` scientia modules are moving: copy `scientia_*.rs` files into
   `vox-plugin-publication/src/scholarly/`; add a `mod scholarly;` in plugin lib.
7. Delete `vox-scientia-core`, `vox-scientia-social`, `vox-scientia-ingest`.
8. Remove all three from workspace `Cargo.toml`.

**Effort: S.**

---

## vox-ssg

### Current state

- Single file: `src/lib.rs` (183 lines, one public function).
- Public API: `generate_static_site(module: &Module) -> Vec<(String, String)>`.
- Depends on: `vox-compiler::ast::decl::{Decl, Module}` — the Vox language AST.

### How it is invoked

`vox-cli/src/commands/build.rs:206` calls `vox_ssg::generate_static_site(&module)` during the
`vox build` command. This is a compiler-pipeline step: parse a `.vox` file → walk `Decl::Routes`
and `Decl::Page` declarations → emit HTML shells for Vite SSR pre-rendering.

### What it does and does NOT do

This is **not a publication concern**. It does not depend on `vox-publisher`, `vox-scientia-*`,
RSS feeds, or any external API. It is a **Vox compiler output stage** — the same conceptual layer
as the TypeScript emitter. Its job is to convert Vox AST route declarations into static HTML
shells that Vite can hydrate.

### Recommendation

**KEEP as CORE.** The dependency axis is `vox-compiler → vox-ssg → vox-cli`, which is a
compiler-pipeline chain. Moving it to a plugin would require either (a) the compiler depending
on a plugin at build time, or (b) re-passing the AST through a plugin boundary — both wrong.

Do NOT fold it into the publication plugin. It belongs adjacent to the compiler output crates
(TypeScript emit, module bundling).

If binary-size is a concern, `vox-ssg` carries no heavyweight deps (`pulldown-cmark`, `feed-rs`,
`reqwest` are absent). Its marginal contribution to binary size is negligible.

**Effort to keep: XS (no action required).**

---

## vox-webhook

### Current code state

**Production-ready, not scaffold.** 1460 lines across 6 files, with comprehensive test coverage.

| File | Lines | State |
|---|---|---|
| `handler.rs` | ~189 | Complete: `InboundPayload`, `WebhookEvent`, `WebhookHandler` with allowlist + signature |
| `signing.rs` | ~332 | Complete: HMAC-SHA3-256 (generic), HMAC-SHA256 (GitHub/Slack), Ed25519 (Discord), constant-time eq, replay-window enforcement |
| `router.rs` | ~262 | Complete: Axum router, `POST /webhooks/:source`, bearer-auth middleware, health + channel list endpoints |
| `delivery.rs` | ~(60+ lines) | Complete: `OutboundWebhook`, retry loop, backoff, custom headers, signed outbound |
| `channel.rs` | ~(60+ lines) | Complete: `Channel`, `ChannelKind` (Discord/Slack/WebSocket/Webhook/Custom), `ChannelManager` |
| `bridge.rs` | ~195 | Complete: `WebhookOrchestratorBridge` (async tokio task, broadcast → orchestrator task queue) |

**Current consumers:** Zero external. No crate imports `vox-webhook`.

### What kind of webhooks

Both inbound and outbound:

- **Inbound** (`router.rs`, `handler.rs`): HTTP server (`POST /webhooks/:source`), source-aware
  header routing (GitHub `X-GitHub-Event`, GitLab `X-GitLab-Event`, Slack `X-Slack-Signature`,
  Discord `X-Signature-Ed25519`), bearer ingress token auth.
- **Outbound** (`delivery.rs`): `WebhookDelivery` POST with retry, backoff, optional HMAC signing.
- **Signing** (`signing.rs`): Three schemes — HMAC-SHA3-256 (Vox generic), HMAC-SHA256
  (GitHub/Slack), Ed25519 (Discord). All verified in constant time with replay-window enforcement.
- **Bridge** (`bridge.rs`): Subscribes to broadcast channel of accepted events and submits them to
  the orchestrator as tasks via `Orchestrator::submit_task_with_agent`.

### Plugin contract proposal

The natural plugin surface is a **standalone HTTP server plugin** that:

1. Exposes a `PluginWebhook` struct implementing `VoxPlugin` (startup → `serve()`, shutdown signal).
2. The plugin's `Cargo.toml` exposes a `webhook-server` feature (enables Axum dependency) and a
   `webhook-client` feature (enables `reqwest`-based outbound delivery).
3. The bridge (`WebhookOrchestratorBridge`) needs `vox-orchestrator` — either accept this dep in
   the plugin, or define a `WebhookEventSink` trait in the plugin and let the orchestrator implement
   it (cleaner ABI boundary, allows testing without a live orchestrator).

**Recommended ABI:** Define `trait WebhookEventSink: Send + Sync` in `vox-plugin-webhook`:
```rust
pub trait WebhookEventSink: Send + Sync {
    fn submit(&self, event: WebhookEvent) -> impl Future<Output = anyhow::Result<()>> + Send;
}
```
The orchestrator implements this trait and passes a `Box<dyn WebhookEventSink>` when constructing
`WebhookState`. This removes the hard `vox-orchestrator` dep from the plugin and makes the plugin
independently testable.

### File-by-file migration

All six source files move as-is into `crates/vox-plugin-webhook/src/`:

| Current path | New path | Changes |
|---|---|---|
| `vox-webhook/src/lib.rs` | `vox-plugin-webhook/src/lib.rs` | Add `VoxPlugin` impl, `WebhookEventSink` trait |
| `vox-webhook/src/handler.rs` | `vox-plugin-webhook/src/handler.rs` | None |
| `vox-webhook/src/signing.rs` | `vox-plugin-webhook/src/signing.rs` | None |
| `vox-webhook/src/router.rs` | `vox-plugin-webhook/src/router.rs` | Replace `Arc<Orchestrator>` dep with `Arc<dyn WebhookEventSink>` |
| `vox-webhook/src/delivery.rs` | `vox-plugin-webhook/src/delivery.rs` | None |
| `vox-webhook/src/channel.rs` | `vox-plugin-webhook/src/channel.rs` | None |
| `vox-webhook/src/bridge.rs` | `vox-plugin-webhook/src/bridge.rs` | Replace `Arc<Orchestrator>` with `Arc<dyn WebhookEventSink>` |

`vox-orchestrator` adds `impl WebhookEventSink for Orchestrator` (≈10 lines) and becomes the
consumer of `vox-plugin-webhook` rather than the other way around.

### Plugin name + ABI considerations

**Name:** `vox-plugin-webhook`. Follows `vox-plugin-*` convention.

The `signing.rs` module contains cryptographic code (HMAC, Ed25519). Keep it inside the plugin
crate rather than promoting to a shared crate — it's webhook-specific signing, not general-purpose.
If the project later needs signing elsewhere, extract then.

**Effort: M** — the code is production-ready; the work is the ABI abstraction
(`WebhookEventSink` trait + orchestrator impl), updating `Cargo.toml`, and wiring the plugin
into whatever plugin-host startup mechanism the project uses.

---

## Cross-cutting concerns

### Key risk: vox-scientia-core dependency cycle claim

`vox-scientia-core`'s own doc comment says it exists to prevent a
`vox-publisher → vox-scientia-core` dependency cycle. But `vox-scientia-core` depends on
`vox-publisher` — so the stated cycle direction is backwards. Before deleting, verify with
the command `cargo tree -p vox-scientia-core` that no other crate imports it in a way that
would create a real cycle. Current scan found zero consumers, so deletion should be safe.

### vox-tools capability_registry drift

`vox-tools/src/capability_registry/mod.rs` is a stale copy of types from `vox-capability-registry`
that only registers one tool (`vox_oratio_transcribe`) while the canonical registry has many more.
Any code that calls `vox_tools::capability_registry::default_registry()` is silently working
against a subset of the real registry. This is a correctness hazard independent of the delete
decision.

### vox-webhook has no consumers but is production-quality

The signing module alone (constant-time HMAC, Discord Ed25519, Slack v0 replay defense) represents
meaningful engineering work that should not be discarded. The reason there are no consumers is
likely that the plugin-host wiring was never completed, not that the feature is unwanted. The
user confirmed it should be preserved as a plugin.
