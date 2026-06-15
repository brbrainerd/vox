# vox-llm-egress Shared Single-Egress Core — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract a low-layer pure-wire `vox-llm-egress` crate from `vox-actor-runtime/llm` so the activity facade and leaf clients (`vox-gamify`, `vox-code-audit/review/client`) reach LLM providers through one sanctioned path, enforced by arch-check + the detector.

**Architecture:** `vox-llm-egress` (≈L2, deps: `vox-http-client` + `reqwest`/`serde` only) owns `chat_once`/`stream_once`/`embed_once` + the AIMD throttle + 429 handling + response parsing. `vox_config::resolve_egress` is the single resolver (key/base-url/headers). The facade keeps `execute_activity`/cascade/retry/telemetry and just delegates the wire. Resolution and telemetry stay OUT of the core (they pull higher layers).

**Tech Stack:** Rust workspace; `reqwest`, `tokio`, `futures::Stream`, `serde`; test HTTP mock via `wiremock`; Windows-safe formatting (`cargo fmt -p <crate>`, never `--all`, per AGENTS.md).

**Spec:** [`docs/superpowers/specs/2026-06-15-vox-llm-egress-design.md`](../specs/2026-06-15-vox-llm-egress-design.md). **Branch:** `llm-ssot-united`. **Out of scope:** `vox-orchestrator-mcp/llm_bridge` (separate spec).

**Per-phase close (every phase):** `/code-review` on the diff, then `cargo clippy -p <each touched crate> -- -D warnings` + green tests before moving on.

---

## File Structure

| File | Responsibility | Phase |
|---|---|---|
| `crates/vox-llm-egress/Cargo.toml` (create) | New L2 crate manifest | 1 |
| `crates/vox-llm-egress/src/lib.rs` (create) | Public surface: `EgressRequest`, `ChatParams`, `EgressChatResponse`, `EgressError`, `ChatMessage`, `ToolDef`; re-exports | 1 |
| `crates/vox-llm-egress/src/throttle.rs` (create — moved) | Per-provider AIMD throttle (moved verbatim from `vox-actor-runtime/src/llm/throttle.rs`) | 1 |
| `crates/vox-llm-egress/src/wire.rs` (create) | `chat_once`/`stream_once`/`embed_once` + request/response (de)serialization | 1 |
| `crates/vox-llm-egress/tests/wire_mock.rs` (create) | wiremock tests: request shape, 429, usage/cost parse, streaming | 1 |
| `crates/vox-config/src/resolve_egress.rs` (create) | `resolve_egress(EgressResolveInput) -> EgressRequest` (moved resolution) | 2 |
| `crates/vox-config/src/lib.rs` (modify) | `pub mod resolve_egress;` + re-export | 2 |
| `crates/vox-actor-runtime/src/llm/{chat,stream,embed}.rs` (modify) | Delegate wire to `vox_llm_egress`; keep activity/telemetry | 3 |
| `crates/vox-actor-runtime/src/llm/throttle.rs` (delete) | Moved to egress crate | 3 |
| `crates/vox-actor-runtime/src/llm/wire.rs` (modify) | Drop moved resolution fns; keep facade-only helpers | 3 |
| `crates/vox-gamify/src/ai/client/transport.rs` (modify) | OpenRouter + OR-Gemini → core; keep locals | 4 |
| `crates/vox-code-audit/src/review/client.rs` (modify) | OpenAI-compatible chat → core | 5 |
| `docs/src/architecture/layers.toml` (modify) | `vox-llm-egress` L2 row + `forbidden_pattern` egress seal | 1,6 |
| `docs/src/architecture/where-things-live.md` | (origin manages this — empty; skip unless regenerated) | — |
| `crates/vox-code-audit/src/detectors/llm_provider_call.rs` (modify) | Flip allowlist `vox-actor-runtime/src/llm/` → `vox-llm-egress` | 6 |

---

## Phase 1 — Core crate `vox-llm-egress`

### Task 1.1: Crate skeleton + types

**Files:**
- Create: `crates/vox-llm-egress/Cargo.toml`, `crates/vox-llm-egress/src/lib.rs`
- Modify: `Cargo.toml` (workspace `[workspace.dependencies]`), `docs/src/architecture/layers.toml`

- [ ] **Step 1: Write the failing test** (bottom of `lib.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn egress_request_is_constructible() {
        let r = EgressRequest {
            base_url: "https://openrouter.ai/api/v1/chat/completions".into(),
            api_key: "k".into(),
            model: "x".into(),
            headers: vec![("X-Title".into(), "vox".into())],
            throttle_key: "openrouter".into(),
        };
        assert_eq!(r.throttle_key, "openrouter");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-llm-egress --lib egress_request_is_constructible`
Expected: FAIL — crate/type not found.

- [ ] **Step 3: Create `crates/vox-llm-egress/Cargo.toml`**

```toml
[package]
name = "vox-llm-egress"
description = "Sanctioned low-layer LLM provider egress: chat_once/stream_once/embed_once + per-provider AIMD throttle. Pure wire — no config/secret resolution (callers pass a resolved EgressRequest). The single path all OpenAI-compatible inference egress goes through."
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
reqwest = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
tokio = { workspace = true }
futures = { workspace = true }
vox-http-client = { workspace = true }
workspace-hack = { workspace = true }

[dev-dependencies]
wiremock = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 4: Create `crates/vox-llm-egress/src/lib.rs`** with the public types:

```rust
//! Sanctioned low-layer LLM provider egress. Pure wire: callers pass a fully-resolved
//! [`EgressRequest`] (resolution lives in `vox_config::resolve_egress`); this crate does
//! throttle + HTTP + 429 handling + response parsing. It owns NO config/secret resolution
//! and NO telemetry-to-db (both pull higher layers) — `chat_once` returns the tokens/cost
//! callers need to record telemetry themselves.

use std::pin::Pin;
use std::time::Duration;

use futures::Stream;
use serde::{Deserialize, Serialize};

pub mod throttle;
mod wire;

pub use throttle::{acquire_permit, on_rate_limited, on_success, retry_after_from_headers, Permit};
pub use wire::{chat_once, embed_once, stream_once};

/// A fully-resolved provider request. No resolution happens in this crate.
#[derive(Clone, Debug)]
pub struct EgressRequest {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub headers: Vec<(String, String)>,
    pub throttle_key: String,
}

/// One chat message on the wire (OpenAI-compatible).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Tool definition passed through to the provider.
#[derive(Clone, Debug, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

/// Per-call generation parameters.
#[derive(Clone, Debug, Default)]
pub struct ChatParams<'a> {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u64>,
    pub response_format: Option<&'a serde_json::Value>,
    pub tools: Option<&'a [ToolDef]>,
    pub tool_choice: Option<&'a serde_json::Value>,
}

/// Parsed chat result. Carries usage/cost/latency so callers record telemetry.
#[derive(Clone, Debug)]
pub struct EgressChatResponse {
    pub content: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub model: String,
    pub cost_usd: Option<f64>,
    pub latency_ms: u64,
}

/// Structured egress failure so callers map to their own error types.
#[derive(Debug)]
pub enum EgressError {
    RateLimited { retry_after: Option<Duration> },
    Http(String),
    Status { code: u16, body: String },
    Decode(String),
}

impl std::fmt::Display for EgressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EgressError::RateLimited { retry_after } => write!(f, "rate limited (retry_after={retry_after:?})"),
            EgressError::Http(e) => write!(f, "http error: {e}"),
            EgressError::Status { code, body } => write!(f, "provider status {code}: {body}"),
            EgressError::Decode(e) => write!(f, "decode error: {e}"),
        }
    }
}
impl std::error::Error for EgressError {}

/// Streaming item type for [`stream_once`].
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<String, EgressError>> + Send>>;
```

- [ ] **Step 5: Register the crate** — add to workspace `Cargo.toml` `[workspace.dependencies]` (alphabetical, near `vox-llm-config`):

```toml
vox-llm-egress            = { path = "crates/vox-llm-egress" }
```

And add to `docs/src/architecture/layers.toml` after the `vox-llm-config` row (L2 — above `vox-http-client` L1, below consumers):

```toml
vox-llm-egress         = { layer = 2, max_dependents = 10 }   # sanctioned LLM provider wire; pure egress, deps = vox-http-client + reqwest
```

- [ ] **Step 6: Stub `throttle.rs` and `wire.rs`** so the crate compiles (real impls in 1.2/1.3). Create `crates/vox-llm-egress/src/throttle.rs` and `src/wire.rs` each containing `// filled in Task 1.2 / 1.3` plus the minimal items `lib.rs` re-exports — OR sequence 1.2/1.3 before first compile. (Recommended: do 1.2 + 1.3 before running Step 7.)

- [ ] **Step 7: Run to verify it passes**

Run: `cargo test -p vox-llm-egress --lib egress_request_is_constructible`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-llm-egress/Cargo.toml crates/vox-llm-egress/src/lib.rs Cargo.toml Cargo.lock docs/src/architecture/layers.toml
git commit -m "feat(vox-llm-egress): crate skeleton + public egress types"
```

### Task 1.2: Move the AIMD throttle

**Files:**
- Create: `crates/vox-llm-egress/src/throttle.rs` (moved from `crates/vox-actor-runtime/src/llm/throttle.rs`, 207 lines)

- [ ] **Step 1: Copy** the entire contents of `crates/vox-actor-runtime/src/llm/throttle.rs` into `crates/vox-llm-egress/src/throttle.rs`. It is self-contained (uses only `std`/`tokio`/`reqwest::header`). Keep its existing `#[cfg(test)]` unit tests. Do **not** delete the original yet (Phase 3 deletes it after the facade delegates).

- [ ] **Step 2: Adjust visibility** — make `acquire_permit`/`on_rate_limited`/`on_success`/`retry_after_from_headers`/`Permit` `pub` (the lib.rs re-export needs them). The current `for_provider` returns a `&'static ProviderThrottle`; expose a `pub async fn acquire_permit(throttle_key: &str) -> Permit<'_>` wrapper that calls `for_provider(throttle_key).acquire().await`, and `pub fn on_rate_limited(key, ra)` / `pub fn on_success(key)` wrappers delegating to `for_provider(key)`.

- [ ] **Step 3: Run the moved throttle tests**

Run: `cargo test -p vox-llm-egress --lib throttle`
Expected: PASS (the moved unit tests).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-llm-egress/src/throttle.rs
git commit -m "feat(vox-llm-egress): move per-provider AIMD throttle into the egress crate"
```

### Task 1.3: `chat_once` (non-streaming wire) — TDD with wiremock

**Files:**
- Create: `crates/vox-llm-egress/src/wire.rs`, `crates/vox-llm-egress/tests/wire_mock.rs`

- [ ] **Step 1: Write the failing test** `crates/vox-llm-egress/tests/wire_mock.rs`:

```rust
use vox_llm_egress::{chat_once, ChatMessage, ChatParams, EgressRequest};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn req(base: String) -> EgressRequest {
    EgressRequest {
        base_url: base,
        api_key: "secret".into(),
        model: "test/model".into(),
        headers: vec![("X-Title".into(), "vox".into())],
        throttle_key: "openrouter".into(),
    }
}

#[tokio::test]
async fn chat_once_sends_bearer_headers_and_parses_usage() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", "Bearer secret"))
        .and(header("x-title", "vox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "test/model",
            "choices": [{"message": {"role": "assistant", "content": "hi"}}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 2}
        })))
        .mount(&server)
        .await;

    let r = req(format!("{}/chat/completions", server.uri()));
    let msgs = vec![ChatMessage { role: "user".into(), content: "yo".into() }];
    let out = chat_once(&r, &msgs, &ChatParams::default()).await.expect("ok");
    assert_eq!(out.content, "hi");
    assert_eq!(out.prompt_tokens, 5);
    assert_eq!(out.completion_tokens, 2);
}

#[tokio::test]
async fn chat_once_maps_429_to_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "7"))
        .mount(&server)
        .await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let err = chat_once(&r, &[], &ChatParams::default()).await.unwrap_err();
    matches!(err, vox_llm_egress::EgressError::RateLimited { .. })
        .then_some(())
        .expect("must be RateLimited");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-llm-egress --test wire_mock`
Expected: FAIL — `chat_once` unimplemented.

- [ ] **Step 3: Implement `chat_once`** in `crates/vox-llm-egress/src/wire.rs`. Mirror the body in `crates/vox-actor-runtime/src/llm/chat.rs:55-120` (request build → throttle permit → POST with bearer + headers → 429 handling → parse), but reading from `EgressRequest` instead of `LlmConfig` and returning `EgressChatResponse`/`EgressError`:

```rust
use std::time::Instant;
use serde::Serialize;
use crate::{throttle, ChatMessage, ChatParams, EgressChatResponse, EgressError, EgressRequest};

#[derive(Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")] temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")] max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] response_format: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")] tool_choice: Option<&'a serde_json::Value>,
    stream: bool,
}

pub async fn chat_once(
    req: &EgressRequest,
    messages: &[ChatMessage],
    params: &ChatParams<'_>,
) -> Result<EgressChatResponse, EgressError> {
    let client = vox_http_client::client();
    let _permit = throttle::acquire_permit(&req.throttle_key).await;

    let body = OpenAiChatRequest {
        model: &req.model,
        messages,
        temperature: params.temperature,
        max_tokens: params.max_tokens,
        response_format: params.response_format,
        tool_choice: params.tool_choice,
        stream: false,
    };
    let mut http = client.post(&req.base_url).json(&body);
    if !req.api_key.is_empty() {
        http = http.bearer_auth(&req.api_key);
    }
    for (name, value) in &req.headers {
        http = http.header(name, value);
    }

    let start = Instant::now();
    let res = http.send().await.map_err(|e| EgressError::Http(e.to_string()))?;
    let status = res.status();
    if status.as_u16() == 429 {
        let retry_after = throttle::retry_after_from_headers(res.headers());
        throttle::on_rate_limited(&req.throttle_key, retry_after);
        return Err(EgressError::RateLimited { retry_after });
    }
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(EgressError::Status { code: status.as_u16(), body });
    }
    let cost_usd = res
        .headers()
        .get("x-response-cost")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok());
    let latency_ms = start.elapsed().as_millis() as u64;
    let json: serde_json::Value = res.json().await.map_err(|e| EgressError::Decode(e.to_string()))?;
    throttle::on_success(&req.throttle_key);

    let content = json["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string();
    let prompt_tokens = json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
    let completion_tokens = json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;
    let model = json["model"].as_str().unwrap_or(&req.model).to_string();
    Ok(EgressChatResponse { content, prompt_tokens, completion_tokens, model, cost_usd, latency_ms })
}

pub async fn stream_once(
    _req: &EgressRequest,
    _messages: &[ChatMessage],
    _params: &ChatParams<'_>,
) -> Result<crate::ChatStream, EgressError> {
    unimplemented!("Task 1.4")
}

pub async fn embed_once(_req: &EgressRequest, _text: &str) -> Result<Vec<f32>, EgressError> {
    unimplemented!("Task 1.5")
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-llm-egress --test wire_mock`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-llm-egress/src/wire.rs crates/vox-llm-egress/tests/wire_mock.rs
git commit -m "feat(vox-llm-egress): chat_once non-streaming wire + wiremock tests"
```

### Task 1.4: `stream_once` (SSE streaming)

**Files:** Modify `crates/vox-llm-egress/src/wire.rs`, add a streaming test to `tests/wire_mock.rs`.

- [ ] **Step 1: Write the failing test** (append to `wire_mock.rs`): mock an SSE body of two `data: {"choices":[{"delta":{"content":"a"}}]}` lines + `data: [DONE]`, assert the assembled stream yields `"a"` then `"b"`. Mirror the SSE framing in `crates/vox-actor-runtime/src/llm/stream.rs:47-120`.

```rust
#[tokio::test]
async fn stream_once_assembles_sse_deltas() {
    use futures::StreamExt;
    let server = MockServer::start().await;
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n\
               data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n\
               data: [DONE]\n\n";
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(sse))
        .mount(&server).await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let mut s = vox_llm_egress::stream_once(&r, &[], &ChatParams::default()).await.expect("stream");
    let mut got = String::new();
    while let Some(chunk) = s.next().await { got.push_str(&chunk.expect("chunk")); }
    assert_eq!(got, "ab");
}
```

- [ ] **Step 2: Run to verify it fails** — `cargo test -p vox-llm-egress --test wire_mock stream_once_assembles_sse_deltas` → FAIL (`unimplemented!`).

- [ ] **Step 3: Implement `stream_once`** by porting the SSE-parsing logic from `crates/vox-actor-runtime/src/llm/stream.rs` (build request with `stream: true`, send, map the `bytes_stream()` into a line buffer that extracts `data:` JSON, yields `choices[0].delta.content`, ends on `[DONE]`). Set `stream: true` in the request body; same throttle/header/bearer wrap as `chat_once`.

- [ ] **Step 4: Run to verify it passes** — `cargo test -p vox-llm-egress --test wire_mock` → PASS.

- [ ] **Step 5: Commit** — `git commit -m "feat(vox-llm-egress): stream_once SSE streaming + test"`.

### Task 1.5: `embed_once`

**Files:** Modify `crates/vox-llm-egress/src/wire.rs`, add an embeddings test.

- [ ] **Step 1: Write the failing test** — mock a `{"data":[{"embedding":[0.1,0.2]}]}` response, assert `embed_once` returns `vec![0.1, 0.2]`. Mirror `crates/vox-actor-runtime/src/llm/embed.rs:76-110`.

- [ ] **Step 2: Run to verify it fails.**

- [ ] **Step 3: Implement `embed_once`** porting `embed.rs` (POST `{model, input: text}`, parse `data[0].embedding`), same throttle/header wrap.

- [ ] **Step 4: Run to verify it passes.**

- [ ] **Step 5: Commit** — `git commit -m "feat(vox-llm-egress): embed_once + test"`.

---

## Phase 2 — `vox_config::resolve_egress` (single resolver)

### Task 2.1: Move resolution into vox-config

**Files:**
- Create: `crates/vox-config/src/resolve_egress.rs`
- Modify: `crates/vox-config/src/lib.rs`, `crates/vox-config/Cargo.toml` (add `vox-llm-egress = { workspace = true }`)

- [ ] **Step 1: Write the failing test** `crates/vox-config/tests/resolve_egress.rs`:

```rust
use vox_config::resolve_egress::{resolve_egress, EgressResolveInput};

#[test]
fn openrouter_resolves_default_base_url_and_throttle_key() {
    let input = EgressResolveInput { provider: "openrouter".into(), model: "x".into(), base_url_override: None };
    let req = resolve_egress(&input).expect("resolve");
    assert!(req.base_url.contains("openrouter"));
    assert_eq!(req.throttle_key, "openrouter");
}

#[test]
fn base_url_override_is_honored() {
    let input = EgressResolveInput { provider: "openai".into(), model: "x".into(), base_url_override: Some("https://custom/v1/chat/completions".into()) };
    let req = resolve_egress(&input).expect("resolve");
    assert_eq!(req.base_url, "https://custom/v1/chat/completions");
}
```

- [ ] **Step 2: Run to verify it fails** — `cargo test -p vox-config --test resolve_egress` → FAIL.

- [ ] **Step 3: Implement** `crates/vox-config/src/resolve_egress.rs` — port `resolve_chat_api_key`, the provider→base-url match, `chat_requires_nonempty_api_key`, and `openrouter_extra_headers` from `crates/vox-actor-runtime/src/llm/wire.rs:60-128` (they already call `vox_secrets`/`vox_config`, both reachable from here):

```rust
//! The single resolver for LLM provider egress: maps a provider+model to a fully-resolved
//! `vox_llm_egress::EgressRequest` using the registry accessors + Clavis. Lives here (not in
//! the egress crate) so resolution is single-source; takes primitives (not LlmConfig) to keep
//! the egress crate free of an L2->L3 dependency.
use vox_llm_egress::EgressRequest;

pub struct EgressResolveInput {
    pub provider: String,
    pub model: String,
    pub base_url_override: Option<String>,
}

fn resolve_api_key(provider: &str) -> String {
    match provider {
        "openrouter" => vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey).expose().unwrap_or_default().to_string(),
        "openai" => vox_secrets::resolve_secret(vox_secrets::SecretId::OpenaiApiKey).expose().unwrap_or_default().to_string(),
        "anthropic" => vox_secrets::resolve_secret(vox_secrets::SecretId::AnthropicApiKey).expose().unwrap_or_default().to_string(),
        "hf_router" | "huggingface" | "hf_endpoint" => crate::inference::huggingface_hub_token().unwrap_or_default(),
        _ => String::new(),
    }
}

fn chat_requires_nonempty_api_key(provider: &str) -> bool {
    matches!(provider, "openrouter" | "openai" | "anthropic")
}

fn extra_headers(provider: &str, model: &str) -> Vec<(String, String)> {
    // Port of wire.rs openrouter_extra_headers (HTTP-Referer / X-Title / route hint).
    // ... (copy verbatim, mapping &'static str keys to String)
    Vec::new()
}

pub fn resolve_egress(input: &EgressResolveInput) -> Result<EgressRequest, String> {
    let api_key = resolve_api_key(&input.provider);
    if chat_requires_nonempty_api_key(&input.provider) && api_key.is_empty() {
        return Err("No API key available for LLM provider".to_string());
    }
    let base_url = input.base_url_override.clone().unwrap_or_else(|| match input.provider.as_str() {
        "openrouter" => crate::inference::openrouter_chat_completions_url(),
        "openai" => crate::inference::openai_chat_completions_url(),
        "hf_router" | "huggingface" => crate::inference::hf_router_chat_completions_url(),
        _ => crate::inference::openrouter_chat_completions_url(),
    });
    Ok(EgressRequest {
        base_url,
        api_key,
        model: input.model.clone(),
        headers: extra_headers(&input.provider, &input.model),
        throttle_key: input.provider.clone(),
    })
}
```

(Copy the full `openrouter_extra_headers` body from `wire.rs:99-128` into `extra_headers`, mapping `&'static str` keys to `String`.)

- [ ] **Step 4: Wire the module** — add `pub mod resolve_egress;` to `crates/vox-config/src/lib.rs`; add `vox-llm-egress = { workspace = true }` to `crates/vox-config/Cargo.toml`. Confirm arch-check: vox-config(L2) → vox-llm-egress(L2) is same-layer — check `layers.toml` allows it (set `vox-llm-egress` `max_dependents` and, if same-layer deps are disallowed, place `vox-llm-egress` at L1; it only needs `vox-http-client` L1, so **L1 is viable and cleaner** — prefer `layer = 1` to keep vox-config(L2)→egress(L1) a clean downward edge). Adjust the Task 1.1 layers.toml row to `layer = 1` if arch-check flags the same-layer edge.

- [ ] **Step 5: Run to verify it passes** — `cargo test -p vox-config --test resolve_egress` → PASS. Then `cargo run -p vox-arch-check 2>&1 | grep -iE "vox-llm-egress|inversion"` → no new inversion from the new edge.

- [ ] **Step 6: Commit** — `git add` the new files + Cargo/layers; `git commit -m "feat(vox-config): resolve_egress single resolver -> EgressRequest"`.

### Task 2.2: resolve_egress parity with the facade

**Files:** Add `crates/vox-config/tests/resolve_egress_parity.rs`.

- [ ] **Step 1: Write the test** asserting, for `openrouter`/`openai`/`hf_router`, that `resolve_egress` yields the same `base_url` and header set the facade's `wire.rs` produces today (call the still-present `vox_config::inference::*_chat_completions_url()` accessors directly and compare). Run → PASS (proves the move preserved behavior).

- [ ] **Step 2: Commit** — `git commit -m "test(vox-config): resolve_egress parity with facade resolution"`.

---

## Phase 3 — Facade delegates to the core (zero-regression gate)

### Task 3.1: Delegate `llm_chat`

**Files:** Modify `crates/vox-actor-runtime/src/llm/chat.rs`, `crates/vox-actor-runtime/Cargo.toml` (add `vox-llm-egress`, `vox-config` already present).

- [ ] **Step 1: Identify the gate** — the existing tests in `crates/vox-actor-runtime/` for `llm_chat` are the regression gate. List them: `cargo test -p vox-actor-runtime --no-run 2>&1` then locate `llm`/`chat` tests. These must pass unchanged after delegation.

- [ ] **Step 2: Replace the egress body** in `llm_chat` (`chat.rs:36-130` region) with a delegation: build `EgressResolveInput` from `LlmConfig`, call `vox_config::resolve_egress`, then `vox_llm_egress::chat_once`, then map `EgressChatResponse` → `LlmResponse` and record telemetry exactly as before. Keep `execute_activity`, the api-key-empty guard, and the telemetry calls. Map `EgressError::RateLimited` to the same string error the old path returned.

```rust
// inside the execute_activity closure, replacing the manual reqwest block:
let input = vox_config::resolve_egress::EgressResolveInput {
    provider: config.provider.clone(),
    model: config.model.clone(),
    base_url_override: config.base_url.clone(),
};
let ereq = match vox_config::resolve_egress::resolve_egress(&input) {
    Ok(r) => r,
    Err(e) => return Ok(Err(e)),
};
let params = vox_llm_egress::ChatParams {
    temperature: config.temperature,
    max_tokens: config.max_tokens,
    response_format: config.response_format.as_ref(),
    tools: None, // map config.tools -> &[ToolDef] if present (see note)
    tool_choice: config.tool_choice.as_ref(),
};
let wire_msgs: Vec<vox_llm_egress::ChatMessage> = messages.iter()
    .map(|m| vox_llm_egress::ChatMessage { role: m.role.clone(), content: m.content.clone() })
    .collect();
let resp = match vox_llm_egress::chat_once(&ereq, &wire_msgs, &params).await {
    Ok(r) => r,
    Err(e) => return Ok(Err(e.to_string())),
};
// map resp -> LlmResponse, then existing telemetry recording using resp.{prompt_tokens,completion_tokens,cost_usd,latency_ms}
```

(Note: if `config.tools` is `Some`, map `LlmToolDef`→`vox_llm_egress::ToolDef` before the call. The `openrouter_tools` shaping that lived in `wire.rs` moves to egress or is applied here.)

- [ ] **Step 3: Run the regression gate** — `cargo test -p vox-actor-runtime` (the chat tests). Expected: PASS unchanged. If a test constructs a fake HTTP server, point it at the same path; behavior must match.

- [ ] **Step 4: Commit** — `git commit -m "refactor(vox-actor-runtime): llm_chat delegates wire to vox-llm-egress"`.

### Task 3.2: Delegate `llm_stream` and `llm_embed`; delete `throttle.rs`

**Files:** Modify `stream.rs`, `embed.rs`; delete `crates/vox-actor-runtime/src/llm/throttle.rs`; update `llm/mod.rs` (drop `pub mod throttle;`, re-export from egress if any external user exists).

- [ ] **Step 1:** Replace `llm_stream`'s wire with `vox_llm_egress::stream_once` (same resolve_egress + params pattern), keeping the activity wrapper. Run `cargo test -p vox-actor-runtime` stream tests → PASS.
- [ ] **Step 2:** Replace `llm_embed`'s wire with `vox_llm_egress::embed_once`. Run embed tests → PASS.
- [ ] **Step 3:** Delete `crates/vox-actor-runtime/src/llm/throttle.rs`; in `mod.rs` replace `pub mod throttle;` with `pub use vox_llm_egress::throttle;` (or drop if unused externally). Remove the now-dead resolution fns from `wire.rs` (`resolve_chat_api_key`, `openrouter_extra_headers`, `chat_requires_nonempty_api_key`) — they live in `vox-config` now. Keep facade-only helpers (`openrouter_tools`).
- [ ] **Step 4:** `cargo test -p vox-actor-runtime` (full) → PASS. `cargo run -p vox-arch-check` → no new violations.
- [ ] **Step 5: Commit** — `git commit -m "refactor(vox-actor-runtime): stream/embed delegate to egress; remove moved throttle+resolution"`.

---

## Phase 4 — Migrate `vox-gamify` (parallel track A)

### Task 4.1: Route OpenRouter through the core; keep locals

**Files:** Modify `crates/vox-gamify/src/ai/client/transport.rs`, `crates/vox-gamify/Cargo.toml` (add `vox-llm-egress`; `vox-config` present).

- [ ] **Step 1: Write the failing test** — a gamify test (wiremock) asserting the OpenRouter path now issues the request via `vox_llm_egress::chat_once` (point `OPENROUTER_BASE_URL` env at the mock; assert response + that `x-response-cost` flows to the cost callback). Add to `crates/vox-gamify/src/ai/client/` test module.

- [ ] **Step 2: Run to verify it fails.**

- [ ] **Step 3: Replace** `call_openrouter_static` / `stream_openrouter` bodies (transport.rs:122-322) with: build `EgressResolveInput{provider:"openrouter", model, base_url_override:None}`, `resolve_egress`, `chat_once`/`stream_once`. Map `EgressChatResponse.cost_usd` → the existing `cost_reporter` callback and `EgressError::RateLimited{retry_after}` → the existing `AiError::RateLimited{provider, retry_after_secs}`. Preserve the 5-model `OPENROUTER_FREE_MODELS` cascade as a loop *around* `chat_once`. **Leave untouched:** `call_gemini_static`/`stream_gemini` (direct Gemini — not OpenAI-compatible; document as a local exception), `call_pollinations_static` (GET), `call_ollama_static`, `deterministic_response`, `auto_discover`/probe.

- [ ] **Step 4: Run** `cargo test -p vox-gamify` → PASS (new + existing). The kept-local paths' tests still pass.

- [ ] **Step 5: Commit** — `git commit -m "refactor(vox-gamify): OpenRouter egress via vox-llm-egress; locals unchanged"`.

---

## Phase 5 — Migrate `vox-code-audit/review/client` (parallel track B)

### Task 5.1: Route OpenAI-compatible chat through the core

**Files:** Modify `crates/vox-code-audit/src/review/client.rs` (+ `review/providers.rs` if needed); `Cargo.toml` add `vox-llm-egress` (`vox-config` present).

- [ ] **Step 1: Write the failing test** — wiremock test asserting the review client's OpenAI-compatible chat path calls `vox_llm_egress::chat_once`. Add to the review module tests.

- [ ] **Step 2: Run to verify it fails.**

- [ ] **Step 3: Replace** the OpenAI-compatible `.post(...)` egress in `client.rs` (the `client.rs:217` chat path and the Ollama OpenAI-compat path) with `resolve_egress` + `chat_once`. **Leave** direct-Gemini (`client.rs:273-284`, `generativelanguage.googleapis.com`) as a documented local exception (or switch to OpenRouter-Gemini if the review config allows). Preserve the review client's error mapping.

- [ ] **Step 4: Run** `cargo test -p vox-code-audit --lib review` → PASS.

- [ ] **Step 5: Commit** — `git commit -m "refactor(vox-code-audit): review client chat egress via vox-llm-egress"`.

---

## Phase 6 — Tighten enforcement

### Task 6.1: Flip the detector allowlist to the egress crate

**Files:** Modify `crates/vox-code-audit/src/detectors/llm_provider_call.rs`.

- [ ] **Step 1: Write the failing test** — assert egress code in `crates/vox-llm-egress/src/wire.rs` is NOT flagged, and a hostname/`.post(` in `crates/vox-actor-runtime/src/llm/chat.rs` (now delegation-only) is NOT flagged, but a fresh provider `.post(` in some other crate IS:

```rust
#[test]
fn egress_crate_is_the_allowlisted_home() {
    let d = LlmProviderCallDetector::new();
    let code = "let resp = client.post(&base_url).bearer_auth(k).send().await?;\nlet u=\"https://api.openai.com/v1\";";
    let mut f = source_at("crates/vox-llm-egress/src/wire.rs", code);
    assert!(d.detect(&f, None).is_empty(), "egress crate is the sanctioned wire");
    let f2 = source_at("crates/some-other/src/x.rs", "let r=client.post(openrouter_base()).send();");
    assert!(!d.detect(&f2, None).is_empty(), "other crates' egress must fire");
}
```

- [ ] **Step 2: Run to verify it fails** (current allowlist is `vox-actor-runtime/src/llm/`).

- [ ] **Step 3: Change `is_facade_file`** in `llm_provider_call.rs` to allowlist `crates/vox-llm-egress/` (the new sanctioned home) plus the documented local exceptions (`vox-gamify` Pollinations/Gemini-direct, `vox-code-audit` Gemini-direct) by path+marker — OR keep those exceptions out of scope and rely on arch-check `exempt_files`. Run tests → PASS.

- [ ] **Step 4: Run the workspace scan** (from the Band-A method) to confirm only intended sites flag. Document any remaining via `exempt_files`.

- [ ] **Step 5: Commit** — `git commit -m "feat(vox-code-audit): allowlist vox-llm-egress as the sanctioned egress home"`.

### Task 6.2: arch-check forbidden_pattern seal

**Files:** Modify `docs/src/architecture/layers.toml`.

- [ ] **Step 1: Add a `[[forbidden_pattern]]`** (mirror the `raw-git-exec` schema at `layers.toml:240-276`) forbidding provider hostnames / `openrouter_base(` outside `crates/vox-llm-egress/`, with `exempt_files` for the documented local exceptions (Pollinations, direct-Gemini, Ollama-probe) and the non-inference sites (`vox-orchestrator/src/catalog.rs`, `vox-ml-cli/src/commands/ai/train.rs`):

```toml
[[forbidden_pattern]]
name             = "llm-egress-outside-core"
pattern          = '(generativelanguage\.googleapis\.com|openrouter\.ai/api|api\.openai\.com/v1|api\.anthropic\.com|openrouter_base\()'
file_glob        = "crates/**/*.rs"
exempt_files     = [
    "crates/vox-llm-egress/src/wire.rs",
    "crates/vox-config/src/resolve_egress.rs",
    "crates/vox-code-audit/src/detectors/llm_provider_call.rs",
    "crates/vox-gamify/src/ai/client/transport.rs",     # documented locals: Pollinations + direct-Gemini
    "crates/vox-code-audit/src/review/client.rs",        # documented local: direct-Gemini
    "crates/vox-orchestrator/src/catalog.rs",            # non-inference: models-list GET
    "crates/vox-ml-cli/src/commands/ai/train.rs",        # non-inference: Together fine-tuning
    "crates/vox-orchestrator-mcp/**",                    # second facade — separate spec
]
allow_annotation = "// vox-arch-check: allow llm-egress"
reason           = "All OpenAI-compatible inference egress must go through vox-llm-egress (the sanctioned wire). Documented local/non-inference paths are exempt pending their own consolidation."
```

- [ ] **Step 2: Run** `cargo run -p vox-arch-check` → passes (exemptions cover existing sites). Expected: no `llm-egress-outside-core` violations.

- [ ] **Step 3: Commit** — `git commit -m "feat(arch-check): forbid LLM inference egress outside vox-llm-egress"`.

### Task 6.3: SSOT doc update

- [ ] **Step 1:** Mark this initiative done in the egress spec; note `llm_bridge` is the remaining follow-on. Commit.

---

## Self-Review (completed during authoring)

- **Spec coverage:** §3.1 core → Phase 1; §3.2 resolve_egress → Phase 2; §3.3 facade delegation → Phase 3, gamify → Phase 4, review-client → Phase 5; §3.4 enforcement → Phase 6; §7 non-inference carve-out → Task 6.2 exempt_files; `llm_bridge` excluded per scope.
- **Placeholder scan:** the two "copy the body from <file:lines>" steps (throttle move, `extra_headers`/`openrouter_extra_headers` port, facade SSE port) are concrete *move/port* instructions with exact source locations + the target signatures shown — not vague TODOs. The `extra_headers` body is explicitly "copy verbatim from wire.rs:99-128".
- **Type consistency:** `EgressRequest`/`ChatParams`/`EgressChatResponse`/`EgressError`/`ChatMessage`/`ToolDef`/`EgressResolveInput`/`resolve_egress`/`chat_once`/`stream_once`/`embed_once` used consistently across all phases.
- **Layering caveat (verify early):** Task 1.1 sets `vox-llm-egress` at L2; Task 2.4 flags that if arch-check disallows the same-layer `vox-config`(L2)→egress edge, drop egress to **L1** (it only needs `vox-http-client` L1). Resolve this in Phase 1/2 before building consumers.

> **Caveat for the implementer:** exact line numbers (`chat.rs:36-130`, `wire.rs:99-128`, `stream.rs`, `transport.rs:122-322`, `client.rs:217/273`) are from reads on branch `llm-ssot-united`; confirm against the live files before each task. The facade's existing tests are the zero-regression gate for Phase 3 — do not weaken them to pass.
