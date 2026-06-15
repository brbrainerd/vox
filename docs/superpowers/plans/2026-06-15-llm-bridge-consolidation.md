# LLM Bridge Consolidation onto vox-llm-egress

**Status:** Plan-only (design map). For implementation by Claude Sonnet 4.6 with parallel subagents + TDD.
**Date:** 2026-06-15
**Predecessors:** `vox-llm-egress` egress core (PR #322, landed), cost-tracking SSOT (PR #329, landed).

## Goal

Merge `vox-orchestrator-mcp/src/llm_bridge` (~2,700 LoC incl. `providers/`) onto the
`vox-llm-egress` core (`chat_once` / `stream_once` / `embed_once`), so the MCP orchestrator
stops being a *second* multi-provider HTTP facade. Preserve the 8 entangled concerns that
currently live only in `llm_bridge`.

## Current State

**Source module:** `crates/vox-orchestrator-mcp/src/llm_bridge/` (~2,700 LoC)
- `mod.rs` (114 LoC): exports + `vox_local_generate()` + `MCP_GLOBAL_LLM_AGENT`
- `infer.rs` (860 LoC): HTTP loop, budget gate, Ollama fallback, cost estimation, telemetry, cache-hit tracking, attachment vision encoding
- `provider_adapter.rs` (437 LoC): 5 `ProviderAdapter` impls (GoogleDirect, Ollama, AnthropicNative, OpenAiCompat, VoxLocal)
- `provider_endpoints.rs` (127 LoC): `endpoint_for()` router for 13 provider types
- `provider_auth.rs` (203 LoC): `bearer_for()` + `extra_headers_for()` (HTTP-Referer, X-Title, X-OpenRouter-Provider-Preferences)
- `providers/` (872 LoC): anthropic.rs, gemini.rs, openai.rs, ollama_chat.rs, metadata.rs, probe.rs, types.rs
- `model_route_policy/`: model resolution, free-tier enforcement, context signals

**Target:** `crates/vox-llm-egress/src/` — `EgressRequest` (provider+model+api_key+headers+throttle_key+max_concurrent+timeout_ms); `chat_once`/`stream_once`/`embed_once` (pure wire); AIMD throttle; `estimate_cost()` (THE single cost formula).

## SSOT Principle (3 tiers)

1. **Cost estimation:** `vox_llm_egress::estimate_cost()` is THE formula. The DeepSeek off-peak discount (75% R1, 50% V3, UTC 16:30–00:30) in `infer.rs::deepseek_off_peak_discount()` must migrate to egress OR a provider-agnostic discount layer in vox_config — never re-implemented.
2. **Provider resolution:** `vox_config::resolve_egress(provider, model)` → `EgressRequest`. `llm_bridge` currently duplicates `endpoint_for`/`bearer_for`/`extra_headers_for`/`max_concurrent`. Unify so orchestrator-mcp and actor-runtime resolve identically.
3. **Telemetry cost recording:** `infer.rs::should_emit_llm_cost_events()` gates `VOX_MCP_LLM_COST_EVENTS`; cost reconciliation (provider_reported vs estimated) at the `ModelCall` event — preserve this decision logic.

## Scope — 8 concerns to consolidate

1. **Cost estimation + DeepSeek off-peak discounts** (`infer.rs:64-102`) — `estimated_cost_usd(model, prompt, completion, cached)`, time/variant discount, preserve `cache_read_cost_per_1k` (~10% of input).
2. **Mesh reputation & health probes** (`probe.rs:24-70`) — VoxLocal `/health` + Ollama `/api/tags`, 30s TTL, block on availability before dispatch. `PopuliMesh` is an orphan placeholder (never dispatched).
3. **ChatML collapse** (`infer.rs:452-466`) — when `chatml_strict`, collapse system+user into a single `<|im_start|>`-delimited user message. Provider-agnostic, pre-dispatch.
4. **Vision/attachments** (`infer.rs:354-490`) — `AttachmentManifest` images fetched from CAS, base64 data: URLs, ~1000 tokens/image for budget gating.
5. **Budget gating + Ollama fallback** (`infer.rs:364-449`) — `BudgetGate` daily cap; on exhaustion retry `best_ollama_model()` if allowed; on Ollama failure `best_non_ollama_model_except()`.
6. **Anthropic tool-fallback** (`provider_adapter.rs:48-56,163`) — Anthropic-native returns `capability_gap` on tools/tool_choice so `infer_via_provider_adapter()` retries with OpenAiCompat.
7. **Custom headers** (`provider_auth.rs:70-98`) — OpenRouter HTTP-Referer + X-Title + route hint (`VOX_OPENROUTER_ROUTE_HINT` / `VOX_COST_PREFERENCE`). Must map to egress `headers`.
8. **Probe caches** (`probe.rs`) — `OnceLock<Mutex<Option<Instant>>>` per probe, TTL-checked. Stay local or move to a new `vox-http-probes`.

**Out of scope:** PopuliMesh (orphan), token-level streaming (already `stream_once`), MCP free-tier routing (`model_route_policy` stays), DB telemetry recording (`vox_orchestrator::usage::UsageTracker`).

## Phases

1. Analyze cost reconciliation — verify DeepSeek factors, `cache_read_cost_per_1k`, estimated-vs-provider_reported; draft cost module signature for egress.
2. Per-provider adapter modules (google_direct, anthropic, openai_compat, ollama, vox_local) with unified `InferRequest → ProviderInferResult`; preserve capability-gap fallback.
3. Extract resolution layer — unify endpoint/bearer/headers into `vox_config::resolve_egress` (13 provider types + max_concurrent).
4. Migrate health probes — keep-local vs promote to `vox-http-probes`; TTL-aware per-probe struct.
5. Extract ChatML + vision layers — ChatML collapse as provider-agnostic pre-dispatch; vision fetch+encode via `vox_openai::ChatMessageContent`.
6. Migrate budget gating + fallback into a new `vox-llm-orchestrator-bridge` crate (keeps MCP binding separate from egress).
7. Adapt Anthropic tool-fallback — `capability_gap` first-class egress error path.
8. Integrate with `resolve_egress` — all 13 branches + max_concurrent + timeout_ms; verify all callers resolve identically.
9. Test cost reconciliation — golden tests for estimated-vs-provider_reported, DeepSeek 0.25/0.50, cache_read.
10. Verify telemetry — `should_emit_llm_cost_events()` preserved; `ModelCall` populated from reconciled cost + cache tokens + retry counts.

## Parallelism

- **Track A (Cost/Reconciliation):** Phases 1, 9.
- **Track B (Providers):** Phases 2, 7.
- **Track C (Config/Resolution):** Phases 3, 8.
- **Track D (Local Services):** Phases 4, 5, 6.

Sequential gates: 3→2 (providers need resolution); 2→7 (Anthropic fallback needs adapter errors); 2→6 (gating picks fallback models via adapters); 8 (final integration) depends on all.

## TDD Notes (highlights)

- **Cost:** non-DeepSeek `(pt+ct)/1000*(in+out)`; V3 off-peak ×0.50; R1 off-peak ×0.25; cache-hit ≤ full cost.
- **Providers:** mock reqwest; GoogleDirect requires GEMINI_API_KEY + parses `cache_read_input_tokens`; AnthropicNative tools → `capability_gap` (no retry here); OpenAiCompat bearer+headers+`provider_reported_cost`; VoxLocal probes `/health`.
- **Config:** `resolve_egress` per 13 providers — OpenRouter headers present; Anthropic direct-vs-proxy switch; DeepSeek URL env; Custom base_url normalized to `/v1/chat/completions`.
- **Probes:** first call hits endpoint; second within TTL cached; timeout → error w/ fallback message.
- **Reconciliation:** `cost_source="provider_reported"` when present else `"estimated"`; `reconciled_usd = max(estimated, 0)` safety.
- **Telemetry:** `should_emit_llm_cost_events()` = false when telemetry off / `VOX_MCP_LLM_COST_EVENTS=0`; true on `=1`; `db.is_none()` when unset.

## Risks

1. Cost formula duplication → enforce in egress, call from infer.rs.
2. Cache-token semantics drift (Anthropic vs OpenAI) → unify in `HttpCallMetadata`.
3. Anthropic tool-fallback silent failure → unit test capability_gap → retry++.
4. Probe TTL staleness on service restart → TTL tests + manual invalidation API.
5. Ollama fallback unavailable → clear error w/ guidance.
6. Endpoint mismatch after consolidation → comparative golden test `endpoint_for` == `resolve_egress` for all 13.
7. Custom headers lost on wire → verify `apply_auth_headers` applies all headers, not just bearer.
8. Telemetry gate inconsistent across callers → move `should_emit_llm_cost_events()` to a shared module; document semantics.

## Key Files

- `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs:1-100,354-490,542-624`
- `crates/vox-orchestrator-mcp/src/llm_bridge/provider_adapter.rs:1-80`
- `crates/vox-orchestrator-mcp/src/llm_bridge/provider_endpoints.rs:21-93`
- `crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs:26-98`
- `crates/vox-orchestrator-mcp/src/llm_bridge/providers/{anthropic,openai,gemini,ollama_chat,probe,metadata}.rs`
- `crates/vox-config/src/resolve_egress.rs:1-150`
- `crates/vox-llm-egress/src/lib.rs`, `crates/vox-llm-egress/src/wire.rs:75-86`
