---
title: "vox-llm-egress — Shared Single-Egress Core"
description: "Extract a low-layer pure-wire LLM egress crate from vox-actor-runtime/llm so the activity facade and leaf clients (vox-gamify, vox-code-audit review/client) reach providers through one sanctioned path, enforceably. llm_bridge consolidation is a separate follow-on spec."
category: "Architecture SSOTs"
---

# vox-llm-egress — Shared Single-Egress Core (Design)

**Status:** Approved design (brainstorming complete) — awaiting implementation plan.
**Implementer target:** Claude Sonnet 4.6 (TDD; workflows / parallel subagents for fan-out).
**Branch:** `llm-ssot-united` (Band A SSOT already landed: registry + GUI + reactive + detector seal).
**Builds on:** [`2026-06-15-llm-ai-settings-ssot-design.md`](2026-06-15-llm-ai-settings-ssot-design.md). Resolves the Phase-4 architectural fork: `llm_chat`/`llm_stream` are durable-activity-coupled, so leaf clients can't route through the facade directly — they need a shared lower-level egress primitive.

## 1. Problem

The "single egress" goal is blocked because the only sanctioned inference path (`vox-actor-runtime/src/llm/{chat,stream,embed}.rs`) wraps every call in `execute_activity` and requires `ActivityOptions` — built for the orchestrator's durable workflows. Leaf clients (`vox-gamify`, `vox-code-audit/review/client`) are same-layer (L3) and have no activity context, so they each maintain their own `reqwest` egress (the dual-egress the Band-A detector seal flags). Routing them through the facade would mean a same-layer dependency + fabricating an activity context — worse architecture.

A parallel read-only map (2026-06-15, 4 agents) confirmed the egress inside the activity closure is self-contained: resolve key/base-url/headers → acquire per-provider throttle permit → build request → `client.post().send()` → 429→throttle cooldown → parse `LlmResponse` + emit telemetry. None of it needs the activity wrapper.

## 2. Goal

A low-layer **pure-wire** crate `vox-llm-egress` that both the activity facade *and* leaf clients call, so:
- **Single sanctioned path:** all OpenAI-compatible inference egress goes through `vox-llm-egress`; the detector + arch-check forbid provider egress anywhere else.
- **No layering damage:** the crate depends only on `vox-http-client` (L1) + `reqwest`/`serde` — no `vox-config`/`vox-secrets`/`vox-db` — so every L3 consumer depends on it *downward*.
- **Zero behavior regression:** the facade keeps its activity/cascade/retry/telemetry; it just delegates the wire to the core. Facade tests prove this.
- **Single-source resolution preserved:** one `vox_config::resolve_egress` produces the `EgressRequest` for everyone (keys/URLs/attribution headers resolved once, from the registry + Clavis).

### Decisions (from brainstorming)
- **Scope:** minimal one-shot primitives (`chat_once`/`stream_once`/`embed_once`) — no retry/cascade/selection in the core; callers compose those.
- **Resolution:** pure wire — callers pass a resolved `EgressRequest`; resolution lives in `vox_config::resolve_egress`.
- **Migration:** full — facade + `vox-gamify` + `vox-code-audit/review/client` + enforcement. `vox-orchestrator-mcp/llm_bridge` is a **separate follow-on spec** (it is a second ~800-LoC multi-provider facade with 8 entangled concerns: cost estimation, mesh reputation, ChatML collapse, vision, budget gating, Anthropic tool-fallback, custom headers, probe caches).

## 3. Architecture

### 3.1 `vox-llm-egress` (new crate, layer ≈ 2)

```rust
/// Fully-resolved provider request. PURE WIRE — no secret/config resolution here.
#[derive(Clone, Debug)]
pub struct EgressRequest {
    pub base_url: String,                  // resolved chat/embeddings endpoint
    pub api_key: String,                   // resolved bearer token ("" = none)
    pub model: String,                     // provider model id
    pub headers: Vec<(String, String)>,    // attribution/routing headers (e.g. OpenRouter)
    pub throttle_key: String,              // provider id for the AIMD throttle ("openrouter"…)
}

pub struct ChatParams<'a> {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u64>,
    pub response_format: Option<&'a serde_json::Value>,
    pub tools: Option<&'a [ToolDef]>,
    pub tool_choice: Option<&'a serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct EgressChatResponse {
    pub content: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub model: String,
    pub cost_usd: Option<f64>,
    pub latency_ms: u64,
}

pub async fn chat_once(req: &EgressRequest, messages: &[ChatMessage], params: &ChatParams<'_>)
    -> Result<EgressChatResponse, EgressError>;
pub async fn stream_once(req: &EgressRequest, messages: &[ChatMessage], params: &ChatParams<'_>)
    -> Result<Pin<Box<dyn Stream<Item = Result<String, EgressError>> + Send>>, EgressError>;
pub async fn embed_once(req: &EgressRequest, text: &str) -> Result<Vec<f32>, EgressError>;

// Per-provider AIMD throttle (moved down from vox-actor-runtime/llm/throttle.rs).
pub async fn acquire_permit(throttle_key: &str) -> Permit<'_>;
pub fn on_rate_limited(throttle_key: &str, retry_after: Option<Duration>);
pub fn on_success(throttle_key: &str);
pub fn retry_after_from_headers(headers: &reqwest::header::HeaderMap) -> Option<Duration>;
```

- `chat_once` internally: `acquire_permit` → POST `base_url` with bearer + headers + OpenAI-compatible body → on 429 `on_rate_limited` and return a typed `EgressError::RateLimited { retry_after }` → else parse choices + usage + `cost_usd` (from `x-response-cost` when present) → `on_success`.
- **`EgressError`** is structured (`RateLimited`, `Http`, `Status { code, body }`, `Decode`) so callers (gamify, facade) map to their own error types and preserve provider-specific handling.
- **No telemetry-to-db and no resolution in this crate** (both pull higher layers); `EgressChatResponse` carries tokens/cost/latency so callers record telemetry themselves.
- `ChatMessage`, `ToolDef` are the minimal wire DTOs, defined here (the facade's `LlmChatMessage`/`LlmToolDef` re-map or re-export).

### 3.2 `vox_config::resolve_egress` (the single resolver)

```rust
pub struct EgressResolveInput {
    pub provider: String,            // "openrouter" | "openai" | "hf_router" | …
    pub model: String,
    pub base_url_override: Option<String>,
}

/// The ONE place that resolves provider key + base-url + attribution headers, from the
/// registry accessors + Clavis. Takes primitives (not LlmConfig) to avoid an L2→L3 import.
pub fn resolve_egress(input: &EgressResolveInput) -> Result<vox_llm_egress::EgressRequest, String>;
```

Moves `resolve_chat_api_key`, the provider→base-url match, `chat_requires_nonempty_api_key`, and `openrouter_extra_headers` down from `vox-actor-runtime/llm/wire.rs` into `vox-config` (which already owns the registry accessors + Clavis access). One resolver, single-source.

### 3.3 Facade + leaf clients become thin callers

- **Facade** (`vox-actor-runtime/llm`): `llm_chat`/`llm_stream`/`llm_embed` keep `execute_activity`, `cascade::*`, `infer_with_retry`, telemetry recording, and the `LlmConfig`/`LlmResponse` DTOs. Inside the activity closure they build `EgressResolveInput` from `LlmConfig`, call `vox_config::resolve_egress`, then `vox_llm_egress::chat_once(...)`, then record telemetry from the returned `EgressChatResponse`. `throttle.rs` is deleted (moved to egress).
- **vox-gamify**: route the OpenRouter path and OpenRouter-Gemini through `chat_once`/`stream_once`; **keep local** Pollinations (GET, non-OpenAI), deterministic fallback, Ollama auto-probe, and direct-Gemini (`generativelanguage.googleapis.com` is *not* OpenAI-compatible → either switch to OpenRouter-Gemini or keep as a documented local exception). Preserve the 5-model cascade, `x-response-cost` and retry-after callbacks as a wrapper fed by `EgressChatResponse`/`EgressError`.
- **vox-code-audit/review/client**: route OpenAI-compatible chat (incl. Ollama OpenAI-compat) through `chat_once`; direct-Gemini handled as in gamify.

### 3.4 Enforcement (Phase 6)

Once all sanctioned callers use `vox-llm-egress`, tighten:
- **arch-check** `[[forbidden_pattern]]`: provider hostnames / `reqwest::Client`+`.post(` outside `crates/vox-llm-egress/`, with `exempt_files` for the *documented* local exceptions (Pollinations, direct-Gemini, Ollama-probe, non-inference models-list/training).
- **`llm_provider_call` detector**: allowlist `crates/vox-llm-egress/` as the egress home (replacing the current `vox-actor-runtime/src/llm/` allowlist), so new drift fails CI.

## 4. Phasing (each independently landable; TDD; zero-regression gate)

1. **Core crate** — `vox-llm-egress`: `EgressRequest`/`ChatParams`/`EgressChatResponse`/`EgressError`, `chat_once`/`stream_once`/`embed_once`, throttle moved from facade. Tests: wire-mock (`wiremock`/`httpmock`) for POST/headers/429/parse; throttle AIMD unit tests. No consumer wired yet.
2. **`resolve_egress`** in `vox-config` + parity test that it produces the same base-url/headers the facade currently computes.
3. **Facade delegates** — refactor `llm_chat`/`llm_stream`/`llm_embed` onto the core; delete `throttle.rs`; **facade's existing tests must pass unchanged** (the zero-regression gate). Telemetry recording stays, fed by `EgressChatResponse`.
4. **vox-gamify** migration (cascade + callbacks preserved; locals kept).
5. **vox-code-audit/review/client** migration.
6. **Enforcement** — arch-check rule + detector allowlist flip; add tests proving a non-egress crate with provider egress fails.

**Parallelism for the implementer:** Phases 4 and 5 (gamify, review-client) are independent consumer migrations and can run as parallel subagent tracks once Phases 1–3 land. Phases 1–3 are sequential (core → resolver → facade).

## 5. Components as isolated units

- `vox-llm-egress` — *what:* the sanctioned provider wire; *interface:* `chat_once`/`stream_once`/`embed_once` + throttle; *depends on:* `vox-http-client`, `reqwest`. Testable standalone with a mock HTTP server.
- `resolve_egress` — *what:* single-source request resolution; *interface:* `EgressResolveInput -> EgressRequest`; *depends on:* registry accessors + Clavis. Testable against the registry.
- facade/leaf callers — *what:* compose policy (activity/cascade/retry, or gamify cascade/callbacks) over the core; *interface:* unchanged public APIs; *depends on:* `vox-llm-egress` + `resolve_egress`.

## 6. Testing strategy

- **Core:** mock-HTTP tests for request shape (model/messages/headers/bearer), 429→`RateLimited`+throttle cooldown, usage/cost parse, streaming chunk assembly; throttle AIMD (halve on 429, +1 per 8 successes).
- **resolve_egress:** asserts base-url/headers/key match the pre-refactor `wire.rs` output for each provider (parity).
- **Facade:** the *existing* `vox-actor-runtime/llm` tests are the zero-regression gate — run unchanged after delegation.
- **Leaf clients:** preserve their existing tests; add a test that the migrated path calls the core (e.g. via a seam/trait or mock server) and that locals (Pollinations/deterministic/probe) still work.
- **Enforcement:** arch-check fixture (non-egress crate + provider egress → fail); detector tests updated for the new allowlist.
- Every phase closes with `/code-review` + `cargo clippy -p <crate> -- -D warnings` + green tests (Windows-safe formatting).

## 7. Non-goals (YAGNI)

- No retry/cascade/model-selection in the core (stays in the facade).
- No telemetry-to-db or resolution in the core (callers own them).
- **`vox-orchestrator-mcp/llm_bridge` consolidation is out of scope** — separate follow-on spec.
- No change to provider model catalogs, selection algorithms, or the durable-activity framework.
- Non-inference egress (OpenRouter models-list GET in `vox-orchestrator/catalog.rs`, Together fine-tuning in `vox-ml-cli/train.rs`) is **not** routed through the chat egress core — it stays on a governed `vox_http_client::client()` and is exempted in the arch-check rule as a distinct category.

## 8. Risks

- **Streaming surface parity** — `stream_once` must reproduce the facade's SSE assembly exactly; covered by mock-stream tests + the facade zero-regression gate.
- **gamify callback fidelity** — `x-response-cost` / retry-after callbacks must still fire; `EgressChatResponse.cost_usd` + `EgressError::RateLimited.retry_after` carry the needed data.
- **`LlmConfig` layering** — `resolve_egress` takes primitives, not `LlmConfig`, to avoid an L2→L3 import; the facade adapts `LlmConfig`→`EgressResolveInput`.
- **Throttle move** — making the throttle global in `vox-llm-egress` means *all* egress shares one per-provider AIMD controller (intended), but the facade must stop constructing its own; verified by the zero-regression gate.
- **direct-Gemini non-compatibility** — `generativelanguage.googleapis.com` is not OpenAI-compatible; the design routes Gemini via OpenRouter or keeps direct-Gemini as a documented, arch-check-exempt local path rather than forcing it through the core.
