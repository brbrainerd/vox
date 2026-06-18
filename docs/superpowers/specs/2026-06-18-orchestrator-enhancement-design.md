---
title: "Orchestrator Enhancement: Model-Agnosticism, A2A, Defactoring, Cost Unification"
description: "Design specification for four coordinated orchestrator improvements: crate defactoring, registry-owns-all model selection, MessageBus-first A2A delivery, and ToolCallRecord cost unification."
category: "architecture"
status: "current"
---

# Orchestrator Enhancement Design (2026-06-18)

## Background

`vox-orchestrator` is a multi-agent file-affinity task router. A 2026-05-08 reorg already extracted
`vox-orchestrator-mcp`, `vox-orchestrator-queue`, and `vox-orchestrator-types` from an original 88K-LoC
monolith, reducing the core to ~52K LoC. This spec captures the next wave of improvements identified in
a June 2026 audit. Four problems were identified, with a clear execution dependency:

```
Plan 1 (Foundation) --> Plan 2 (Model Agnosticism)
                    --> Plan 3 (A2A MessageBus)
                    --> Plan 4 (ToolCallRecord)
```

Plans 2, 3, and 4 are independent of each other and can be parallelized after Plan 1 completes.

---

## Audit Findings

| Pain point | Type |
|---|---|
| `axum` is a non-optional dep in `vox-orchestrator` core (MCP-only concern) | Build tax |
| `vox-code-audit` + `vox-lsp` + `tower-lsp-server` in `default` features | Build tax |
| `tiktoken-rs` non-optional | Build tax |
| `vox-corpus` forced with `database` feature | Build tax |
| `AiTaskProcessor` hardwires `FreeAiClient`, bypassing model registry | Architectural |
| `model_resolution.rs` in `vox-actor-runtime` has its own 8-step resolution chain | Architectural |
| `ModelConfidence` state machine not yet the hard routing gate | Correctness |
| A2A local path polls VoxDB even for same-process agents | Runtime |
| `ToolReceipt` (integrity) and `UsageRecord` (cost) are disconnected write paths | Observability |

---

## Decision 1: Defactoring — two phases, no behavior changes

### Phase A: Feature-gate surgery

Changes to `crates/vox-orchestrator/Cargo.toml`:

| Dep | Before | After |
|---|---|---|
| `axum` | non-optional | optional, behind new `http-server` feature |
| `tiktoken-rs` | non-optional | optional, behind new `token-counting` feature |
| `vox-code-audit`, `vox-lsp`, `tower-lsp-server` | in `default` via `toestub-gate` | removed from `default` |
| `vox-corpus` with `database` feature | always-on | behind new `corpus-db` feature |

New `default` features: `["runtime", "json-schema", "jj"]`

### Phase B: `vox-orchestrator-core` extraction

New crate `crates/vox-orchestrator-core/` contains:
- `src/models/` — model registry, selection, autonomic, scoring, discovery
- `src/usage.rs` + `src/usage_policy.rs` — LLM cost accounting
- `src/budget/` — per-agent budget management
- `build.rs` — YAML-to-Rust codegen for ModelTier, StrengthTag, TaskCategory, etc.

Constraints on `vox-orchestrator-core`: no `axum`, no LSP, no code-audit. `tiktoken-rs` optional.

`vox-orchestrator` depends on `vox-orchestrator-core` and re-exports its public API.

Rationale: `models/` changes most often (catalog updates, scoring tweaks). Isolating it means
model registry changes no longer force a rebuild of routing, session, or A2A code.

---

## Decision 2: Model Agnosticism — registry-owns-all

`ModelSelector` trait defined in `vox-orchestrator-core::models::selector_trait`:

```rust
#[async_trait]
pub trait ModelSelector: Send + Sync + 'static {
    async fn select(&self, intent: SelectionIntent) -> Option<ModelSpec>;
    async fn record_outcome(
        &self,
        model_id: &str,
        category: TaskCategory,
        success: bool,
        latency_ms: u64,
        cost_usd: f64,
    );
}

pub struct StubModelSelector { pub fixed_model: Option<ModelSpec> }
#[async_trait]
impl ModelSelector for StubModelSelector {
    async fn select(&self, _: SelectionIntent) -> Option<ModelSpec> { self.fixed_model.clone() }
    async fn record_outcome(&self, _: &str, _: TaskCategory, _: bool, _: u64, _: f64) {}
}
```

Changes:
- `ModelRegistry` implements `ModelSelector`
- `vox-actor-runtime` accepts `Arc<dyn ModelSelector>` (injected, never self-resolved)
- `AiTaskProcessor` drops `FreeAiClient`; receives `Arc<dyn ModelSelector>` from `AgentFleet`
- `model_resolution.rs` in `vox-actor-runtime` is removed; env-pin logic moves to `ModelRegistry::select()`
- `ModelConfidence::Confirmed` is a hard gate: `select()` only returns confirmed models

---

## Decision 3: A2A MessageBus-First

**Problem:** Local agent-to-agent messages travel: `send_to_db` -> VoxDB `a2a_messages` ->
`poll_inbox_from_db`. Latency is bounded by poll interval.

**Solution:** Promote `MessageBus` (tokio broadcast) to primary local delivery.
DB becomes the durable/cross-process fallback only.

New types:
- `LocalA2AChannel` — `tokio::sync::broadcast::Sender<A2AMessage>`, capacity 1024
- `A2ARouter` — decides local vs. DB delivery; maintains `HashSet<AgentId>` of local agents

Wiring:
- `Orchestrator::register_agent()` also calls `A2ARouter::register_local(agent_id)`
- `Orchestrator::unregister_agent()` calls `A2ARouter::unregister_local(agent_id)`
- Messages with `require_durable: true` always go to DB regardless of locality

Remote (Populi mesh) path is untouched.

---

## Decision 4: ToolCallRecord — cost + integrity unified

Single struct replaces `ToolReceipt` + `UsageRecord`. HMAC covers both integrity and cost fields.

```rust
pub struct ToolCallRecord {
    pub record_id: String,           // UUIDv7
    pub agent_id: AgentId,
    pub tool_name: String,
    pub call_args_hash: String,      // BLAKE3
    pub result_hash: Option<String>, // BLAKE3, None while pending
    pub executed_at_ms: u64,
    pub hmac_tag: [u8; 32],          // covers all fields below
    pub provider: String,
    pub model: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
    pub provider_reported_cost_usd: Option<f64>,
    pub estimated_cost_usd: Option<f64>,
    pub reconciled_cost_usd: Option<f64>,
    pub cost_source: String,   // "estimated"|"provider_reported"|"reconciled"|"pending"
    pub task_category: String,
    pub date: String,          // YYYY-MM-DD
    pub user_id: String,
}
```

Migration:
- `ToolReceiptLedger` replaced by `ToolCallLedger`
- `UsageTracker` kept as read-only aggregate view over `tool_call_records`
- New DB table `tool_call_records` with indexes on `(date, provider, model)`, `(agent_id, date)`,
  `(task_category, date)`, `(record_id)`
- Old `provider_usage` table: retained read-only; no new writes

---

## Implementation Order

Plans 2, 3, and 4 can be parallelized after Plan 1 completes.

| Plan | File |
|---|---|
| 1 — Foundation | `docs/superpowers/plans/2026-06-18-orchestrator-foundation.md` |
| 2 — Model Agnosticism | `docs/superpowers/plans/2026-06-18-orchestrator-model-agnosticism.md` |
| 3 — A2A MessageBus | `docs/superpowers/plans/2026-06-18-orchestrator-a2a-messagebusprimary.md` |
| 4 — ToolCallRecord | `docs/superpowers/plans/2026-06-18-orchestrator-tool-call-record.md` |
