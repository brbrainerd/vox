# Telemetry Phase B — `model_call_event` Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist a durable `model_call_event` row to `research_metrics` for every LLM call, capturing prompt/completion tokens, **split cache tokens**, latency, cost, error class, and trace linkage. Replaces the discard-on-bus path for cost data.

**Architecture:** Add a `ModelCallEvent` variant to `TelemetryEvent`. Extend `ProviderInferResult` with split cache fields (`cache_read_input_tokens`, `cache_creation_input_tokens`). Emit `record_event!` from `vox-orchestrator-mcp/src/llm_bridge/infer.rs` immediately after cost reconciliation, alongside (not replacing) the existing bus `CostIncurred` emit. Pass `Some(db)` to `init_telemetry_sinks` in CLI startup so `ResearchMetricsSink` actually persists.

**Prerequisite:** Phase A is merged (`vox-telemetry` crate exists with `record_event!` macro, `ResearchMetricsSink`, `SpoolSink`).

**Spec:** `docs/src/architecture/telemetry-unification-design-2026.md` § Phase B.

**Semantic change:** New rows appear in `research_metrics` with `metric_type = "model_call_event"` whenever an LLM call completes. Existing `CostIncurred` bus events continue to emit (no removal). User-visible only via DB inspection.

---

## File map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `crates/vox-telemetry/src/types.rs` | Add `ModelCallEvent` struct + `TelemetryEvent::ModelCall` variant |
| Modify | `crates/vox-telemetry/src/lib.rs` | Re-export `ModelCallEvent` |
| Modify | `crates/vox-orchestrator-mcp/src/llm_bridge/providers/anthropic.rs` (or wire types) | Parse split cache tokens from response |
| Modify | `crates/vox-orchestrator-mcp/src/llm_bridge/types.rs` (or wherever `ProviderInferResult` is defined) | Replace single `cached_input_tokens` with split fields |
| Modify | `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs` | Add `record_event!(ModelCall(...))` after cost reconciliation; also add `Cargo.toml` dep on `vox-telemetry` |
| Modify | `crates/vox-orchestrator-mcp/Cargo.toml` | Add `vox-telemetry = { workspace = true }` |
| Modify | `crates/vox-cli/src/lib.rs` | `run_vox_cli_from_parsed`: try `connect_cli_workspace_voxdb` and pass `Some(db)` to `init_telemetry_sinks` |
| Modify | `crates/vox-db/src/telemetry_sink.rs` | Extend `ResearchMetricsSink::record` to handle `TelemetryEvent::ModelCall` |
| Create | `crates/vox-db/tests/model_call_event_roundtrip.rs` | Integration test — emitted event survives to `research_metrics` |

---

## Task 1 — Extend `TelemetryEvent` with `ModelCall` variant

**Files:**
- Modify: `crates/vox-telemetry/src/types.rs`
- Modify: `crates/vox-telemetry/src/lib.rs`

- [ ] **Step 1: Add `ModelCallEvent` struct after `ResearchMetricEvent` in `types.rs`**

After the `ResearchMetricEvent` struct definition (line ~353), add:

```rust
/// Per-LLM-call performance record. Persisted as `research_metrics` row with
/// `metric_type = METRIC_TYPE_MODEL_CALL_EVENT`.
///
/// Unlocks (in aggregate): cache hit rate, cost-per-task, p95 latency by model and route,
/// token efficiency. Sensitivity: **S1 (OperationalTracing)**.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelCallEvent {
    pub model: String,
    pub provider: String,
    pub route_profile: Option<String>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    /// Anthropic prompt-cache: tokens read from cache (~10× cheaper than fresh input).
    pub cache_read_input_tokens: Option<u32>,
    /// Anthropic prompt-cache: tokens written to create the cache (~25% premium).
    pub cache_creation_input_tokens: Option<u32>,
    pub latency_ms: u64,
    pub cost_usd: f64,
    pub cost_source: String, // "provider_reported" | "estimated"
    pub error_class: Option<String>,
    pub retry_attempt: u32,
    /// Linkage fields — populated from `current_trace_ctx()` (Phase C wires propagation).
    pub task_id: Option<u64>,
    pub parent_task_id: Option<u64>,
    pub trace_id: Option<String>,
    pub caller_agent_id: Option<String>,
}
```

- [ ] **Step 2: Add the `ModelCall` variant to `TelemetryEvent`**

Find the `TelemetryEvent` enum (line ~338). Replace its body to include the new variant:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TelemetryEvent {
    /// Wraps the existing research_metrics row.
    ResearchMetric(ResearchMetricEvent),
    /// Per-LLM-call record (Phase B).
    ModelCall(ModelCallEvent),
    // Phase C adds: TaskRootSummary
    // Phase D adds: BuildSummary, Error
}
```

- [ ] **Step 3: Re-export `ModelCallEvent` from `lib.rs`**

In `crates/vox-telemetry/src/lib.rs`, find the `pub use types::{` block and add `ModelCallEvent` to the alphabetical position:

```rust
pub use types::{
    // ... existing items ...
    ModelCallEvent,
    // ... existing items ...
};
```

- [ ] **Step 4: Add a unit test for serialization round-trip**

In `crates/vox-telemetry/src/types.rs`, inside `mod tests`, add:

```rust
#[test]
fn model_call_event_serialize_round_trip() {
    let e = ModelCallEvent {
        model: "claude-opus-4-7".into(),
        provider: "anthropic".into(),
        route_profile: Some("strong".into()),
        prompt_tokens: 1234,
        completion_tokens: 567,
        cache_read_input_tokens: Some(800),
        cache_creation_input_tokens: Some(50),
        latency_ms: 2400,
        cost_usd: 0.0152,
        cost_source: "provider_reported".into(),
        error_class: None,
        retry_attempt: 0,
        task_id: Some(42),
        parent_task_id: None,
        trace_id: Some("abc-123".into()),
        caller_agent_id: None,
    };
    let event = TelemetryEvent::ModelCall(e.clone());
    let json = serde_json::to_string(&event).expect("serialize");
    let back: TelemetryEvent = serde_json::from_str(&json).expect("deserialize");
    let TelemetryEvent::ModelCall(back) = back else {
        panic!("variant lost in round trip")
    };
    assert_eq!(back.model, e.model);
    assert_eq!(back.cache_read_input_tokens, e.cache_read_input_tokens);
    assert_eq!(back.trace_id, e.trace_id);
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p vox-telemetry
```

Expected: 7 tests pass (5 from Phase A + `model_call_event_serialize_round_trip` + `new_phase_b_d_constants_pass_validation`).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-telemetry/
git commit -m "feat(vox-telemetry): add ModelCallEvent variant + cache-token fields"
```

---

## Task 2 — Split cache tokens in `ProviderInferResult` and Anthropic parser

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/types.rs` (or wherever `ProviderInferResult` is defined — locate via grep)
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/providers/anthropic.rs`

- [ ] **Step 1: Locate the `ProviderInferResult` definition**

```bash
grep -rn "pub struct ProviderInferResult\|struct ProviderInferResult" crates/vox-orchestrator-mcp/src --include="*.rs"
```

Note the file:line of the definition.

- [ ] **Step 2: Replace `cached_input_tokens` with the two split fields**

In the located file, find:

```rust
pub struct ProviderInferResult {
    pub text: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub provider_request_id: Option<String>,
    pub provider_reported_cost_usd: Option<f64>,
    pub cached_input_tokens: Option<u32>,
}
```

Replace with:

```rust
pub struct ProviderInferResult {
    pub text: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub provider_request_id: Option<String>,
    pub provider_reported_cost_usd: Option<f64>,
    /// Anthropic-style: tokens served from prompt cache (cheap reads).
    pub cache_read_input_tokens: Option<u32>,
    /// Anthropic-style: tokens written to populate the prompt cache (creation premium).
    pub cache_creation_input_tokens: Option<u32>,
}
```

- [ ] **Step 3: Update Anthropic provider parser**

In `crates/vox-orchestrator-mcp/src/llm_bridge/providers/anthropic.rs`, find where `cached_input_tokens` is populated (grep for the field name). The Anthropic API response has `usage.cache_read_input_tokens` and `usage.cache_creation_input_tokens` as separate `u32` fields. Replace the single-field assignment with both:

```rust
// Before: single combined field
cached_input_tokens: usage.cache_read_input_tokens.or(usage.cache_creation_input_tokens),

// After: split
cache_read_input_tokens: usage.cache_read_input_tokens,
cache_creation_input_tokens: usage.cache_creation_input_tokens,
```

- [ ] **Step 4: Update OpenAI / other provider parsers — pass `None` for both fields**

For non-Anthropic providers, set both new fields to `None` since OpenAI doesn't expose prompt cache tokens equivalently. Find each `ProviderInferResult { … }` literal in the workspace:

```bash
grep -rn "ProviderInferResult {" crates/vox-orchestrator-mcp/src --include="*.rs"
```

For each non-Anthropic site, replace `cached_input_tokens: …` with both new fields set to `None`.

- [ ] **Step 5: Update infer.rs cost computation to use combined cached input**

In `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs`, the existing `estimated_cost_usd(&model, pt, ct, cached_input_tokens)` call needs the combined cached count. Change the call site (around line ~518) where the destructured `Ok(ProviderInferResult { ... })` block reads:

```rust
Ok(ProviderInferResult {
    text,
    prompt_tokens: pt,
    completion_tokens: ct,
    provider_request_id,
    provider_reported_cost_usd,
    cache_read_input_tokens,
    cache_creation_input_tokens,
}) => {
    let total_tok = (pt + ct) as u64;
    // For cost estimation, count any cached input (read or creation).
    let cached_for_cost = match (cache_read_input_tokens, cache_creation_input_tokens) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    };
    let estimated_usd = estimated_cost_usd(&model, pt, ct, cached_for_cost);
    // ... rest unchanged
```

- [ ] **Step 6: Update the `temporal_context` JSON in the existing `CostIncurred` bus emit**

Same file, same block — the existing JSON literal that included `"cached_input_tokens": cached_input_tokens` needs both fields:

```rust
temporal_context: Some(serde_json::json!({
    "tool": tool,
    "provider_request_id": provider_request_id,
    "user_id": routing.user_id,
    "cost_source": cost_source,
    "cache_read_input_tokens": cache_read_input_tokens,
    "cache_creation_input_tokens": cache_creation_input_tokens,
})),
```

- [ ] **Step 7: Build and test**

```bash
cargo build -p vox-orchestrator-mcp
cargo test -p vox-orchestrator-mcp llm_bridge
```

Expected: builds clean, existing tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-orchestrator-mcp/
git commit -m "refactor(orchestrator-mcp): split cache tokens (read vs creation) in ProviderInferResult"
```

---

## Task 3 — Emit `model_call_event` from `infer.rs` after cost reconciliation

> **Reader note (added 2026-05-16):** the `task_id` / `parent_task_id` /
> `trace_id` / `caller_agent_id` fields described below are intentionally
> emitted as `None` in Phase B — see the [Phase C preview](#phase-c-preview)
> at the bottom of this doc. Phase C wires `TRACE_CTX::scope` through MCP
> dispatch so `current_trace_ctx()` returns a populated context here. Until
> Phase C lands, the example code in this task that references
> `trace_ctx.task_id` etc. should be read as forward-looking; the actual
> shipped Phase B emit leaves all four fields as `None`. Refs:
> docs/src/architecture/semantic-gap-audit-2026.md F8.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml`
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs`

- [ ] **Step 1: Add `vox-telemetry` dep to `vox-orchestrator-mcp/Cargo.toml`**

In `crates/vox-orchestrator-mcp/Cargo.toml` under `[dependencies]`:

```toml
vox-telemetry = { workspace = true }
```

- [ ] **Step 2: Add the `record_event!` call in `infer.rs`**

In `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs`, find the `Ok(ProviderInferResult { ... })` match arm (line ~460-531). After `let (reconciled_usd, cost_source) = …;` (around line 469-473), and **before** the existing `if should_emit_llm_cost_events(state) {` block, add:

```rust
        let latency_ms = start.elapsed().as_millis() as u64;

        // Phase B: durable per-call telemetry. Survives even when bus emit is gated off.
        let trace_ctx = vox_telemetry::current_trace_ctx();
        vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::ModelCall(
            vox_telemetry::ModelCallEvent {
                model: model.id.clone(),
                provider: usage.provider.clone(),
                route_profile: routing.route_profile.clone(),
                prompt_tokens: pt,
                completion_tokens: ct,
                cache_read_input_tokens,
                cache_creation_input_tokens,
                latency_ms,
                cost_usd: reconciled_usd,
                cost_source: cost_source.to_string(),
                error_class: None,
                retry_attempt: 0,
                task_id: trace_ctx.task_id,
                parent_task_id: trace_ctx.parent_task_id,
                trace_id: Some(trace_ctx.trace_id.to_string()),
                caller_agent_id: trace_ctx.caller_agent_id.clone(),
            }
        ));
```

NOTE: `routing.route_profile` may not exist as a field. If grep shows it's named differently (e.g., `routing.policy_profile`), adjust accordingly. If no equivalent field exists, use `None`:

```rust
route_profile: None, // Phase D: wire from routing struct when route_policy_profile is unified
```

Confirm by:

```bash
grep -n "route_profile\|policy_profile\|route_policy" crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs | head -10
```

- [ ] **Step 3: Add the `start` variable if not already present**

The existing code uses `start.elapsed()` elsewhere; verify a `let start = std::time::Instant::now()` exists at the top of the function. If not, add it at the function entry.

```bash
grep -n "let start =" crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs | head -5
```

- [ ] **Step 4: Build**

```bash
cargo build -p vox-orchestrator-mcp
```

Expected: clean build, no warnings about unused fields.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/
git commit -m "feat(orchestrator-mcp): emit model_call_event after cost reconciliation"
```

---

## Task 4 — Wire `init_telemetry_sinks(Some(db))` in CLI startup

**Files:**
- Modify: `crates/vox-cli/src/lib.rs`

- [ ] **Step 1: Update `run_vox_cli_from_parsed` to attempt DB connection and pass it**

In `crates/vox-cli/src/lib.rs`, locate `run_vox_cli_from_parsed` (line ~563). Replace the `init_telemetry_sinks(None);` line with an opportunistic DB open:

```rust
pub async fn run_vox_cli_from_parsed(root: VoxCliRoot) -> anyhow::Result<()> {
    if root.global.verbose > 0 && std::env::var_os("RUST_LOG").is_none() {
        unsafe {
            crate::config::set_process_env("RUST_LOG", "debug");
        }
    }
    init_tracing_for_cli();

    // Phase B: register sinks with optional DB. Open is opportunistic and silent —
    // commands that need a DB still open one themselves; we just reuse a connection
    // here so model_call_event events can persist.
    let telemetry_db = crate::workspace_db::connect_cli_workspace_voxdb_with_overrides(true)
        .await
        .ok();
    init_telemetry_sinks(telemetry_db);

    apply_global_opts(&root.global);
    cli_dispatch::dispatch_cli(root.cmd, &root.global).await
}
```

The `skip_log = true` argument suppresses the connect-failure log so users without a workspace DB don't see startup noise.

- [ ] **Step 2: Build and run a no-op CLI command**

```bash
cargo build -p vox-cli
cargo run -p vox-cli -- --help 2>&1 | head -20
```

Expected: builds clean, `--help` runs successfully whether or not a workspace DB exists.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-cli/src/lib.rs
git commit -m "feat(vox-cli): pass workspace DB to init_telemetry_sinks (Phase B activation)"
```

---

## Task 5 — Extend `ResearchMetricsSink` to handle `ModelCall` events

**Files:**
- Modify: `crates/vox-db/src/telemetry_sink.rs`

- [ ] **Step 1: Update the `record` impl to dispatch on variant**

Replace the `record` method body in `crates/vox-db/src/telemetry_sink.rs` with a multi-variant handler:

```rust
impl TelemetryRecorder for ResearchMetricsSink {
    fn record(&self, event: &TelemetryEvent) {
        match event {
            TelemetryEvent::ResearchMetric(e) => {
                let db = Arc::clone(&self.db);
                let e = e.clone();
                tokio::spawn(async move {
                    if let Err(err) = db
                        .append_research_metric(
                            &e.session_id,
                            &e.metric_type,
                            e.metric_value,
                            e.metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: research_metric write failed");
                    }
                });
            }
            TelemetryEvent::ModelCall(e) => {
                let db = Arc::clone(&self.db);
                let e = e.clone();
                tokio::spawn(async move {
                    let session_id = match &e.task_id {
                        Some(tid) => format!("model:{tid}"),
                        None => "model:unscoped".to_string(),
                    };
                    let metadata_json = match serde_json::to_string(&e) {
                        Ok(s) => Some(s),
                        Err(err) => {
                            tracing::warn!(?err, "ResearchMetricsSink: model_call_event serialize failed");
                            return;
                        }
                    };
                    if let Err(err) = db
                        .append_research_metric(
                            &session_id,
                            vox_telemetry::METRIC_TYPE_MODEL_CALL_EVENT,
                            Some(e.cost_usd),
                            metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: model_call_event write failed");
                    }
                });
            }
        }
    }
}
```

NOTE: When Phase C and D add more variants, this `match` will need new arms. Until then, the `non_exhaustive` enum requires no wildcard because all variants are listed (Rust accepts exhaustive matches over `non_exhaustive` enums when defined in the same crate; from a downstream crate a `_ =>` arm is required). Since `vox-db` is downstream of `vox-telemetry`, add a wildcard arm for forward compatibility:

```rust
            _ => {
                // Variants added in later phases are handled by sinks introduced in those phases.
            }
```

Place it as the final arm of the match.

- [ ] **Step 2: Build**

```bash
cargo build -p vox-db
```

Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-db/src/telemetry_sink.rs
git commit -m "feat(vox-db): ResearchMetricsSink handles ModelCall variant → research_metrics row"
```

---

## Task 6 — Integration test: cache tokens round-trip from emit to `research_metrics`

**Files:**
- Create: `crates/vox-db/tests/model_call_event_roundtrip.rs`

- [ ] **Step 1: Write the integration test**

```rust
// crates/vox-db/tests/model_call_event_roundtrip.rs
//! Phase B integration test: a `ModelCallEvent` emitted through the global
//! recorder lands as a `research_metrics` row with `metric_type =
//! "model_call_event"` and the cache token fields preserved in metadata.

use std::sync::Arc;

use vox_db::telemetry_sink::ResearchMetricsSink;
use vox_telemetry::{
    ModelCallEvent, METRIC_TYPE_MODEL_CALL_EVENT, TelemetryEvent, TelemetryRecorder,
    set_global_recorder,
};

#[tokio::test]
async fn model_call_event_persists_cache_tokens() {
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let db_path = tmpdir.path().join("test.db");
    let config = vox_db::CanonicalDbConfig::local_path(&db_path);
    let db = vox_db::VoxDb::connect(&config).await.expect("connect");

    let sink = Arc::new(ResearchMetricsSink::new(db.clone()));
    sink.record(&TelemetryEvent::ModelCall(ModelCallEvent {
        model: "claude-opus-4-7".into(),
        provider: "anthropic".into(),
        route_profile: Some("strong".into()),
        prompt_tokens: 1000,
        completion_tokens: 500,
        cache_read_input_tokens: Some(750),
        cache_creation_input_tokens: Some(50),
        latency_ms: 1200,
        cost_usd: 0.0123,
        cost_source: "provider_reported".into(),
        error_class: None,
        retry_attempt: 0,
        task_id: Some(42),
        parent_task_id: None,
        trace_id: Some("trace-abc".into()),
        caller_agent_id: None,
    }));

    // Wait briefly for the spawned write task.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let rows = db
        .list_research_metrics_by_type(METRIC_TYPE_MODEL_CALL_EVENT, "model:", 10)
        .await
        .expect("list rows");
    assert_eq!(rows.len(), 1, "expected one model_call_event row");
    let (session_id, value, metadata_json) = rows.into_iter().next().unwrap();
    assert_eq!(session_id, "model:42");
    assert!((value.unwrap_or(0.0) - 0.0123).abs() < 1e-6);
    let metadata = metadata_json.expect("metadata present");
    assert!(metadata.contains("\"cache_read_input_tokens\":750"));
    assert!(metadata.contains("\"cache_creation_input_tokens\":50"));
    assert!(metadata.contains("\"trace_id\":\"trace-abc\""));
}
```

NOTE: Verify the exact API names for `vox_db::CanonicalDbConfig::local_path` and `VoxDb::connect`. If the workspace uses different constructors (e.g., `connect_workspace_journey_optional`), adjust accordingly:

```bash
grep -n "pub async fn connect\|pub fn local_path" crates/vox-db/src/canonical_store.rs crates/vox-db/src/lib.rs | head -10
```

- [ ] **Step 2: Run the test**

```bash
cargo test -p vox-db --test model_call_event_roundtrip
```

Expected: PASS. The row is present with the cache tokens preserved.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-db/tests/model_call_event_roundtrip.rs
git commit -m "test(vox-db): model_call_event survives sink → research_metrics round trip"
```

---

## Task 7 — Final verification

- [ ] **Step 1: Workspace test sweep**

```bash
cargo test -p vox-telemetry -p vox-db -p vox-cli -p vox-orchestrator-mcp
```

Expected: all tests pass.

- [ ] **Step 2: Architecture check**

```bash
cargo run -p vox-arch-check -- .
```

Expected: exits 0; `vox-orchestrator-mcp` (L3) → `vox-telemetry` (L1) is a valid downward edge.

- [ ] **Step 3: Smoke test — run a real LLM call and verify a row lands**

This requires API credentials; skip if not available. With them:

```bash
# Invoke any vox command that triggers an MCP LLM call
cargo run -p vox-cli -- mcp <some-tool-with-llm>

# Inspect:
sqlite3 .vox/store.db "SELECT session_id, metric_type, metric_value, metadata_json FROM research_metrics WHERE metric_type = 'model_call_event' ORDER BY rowid DESC LIMIT 3;"
```

Expected: at least one row with non-null `cache_read_input_tokens` if the call hit the prompt cache.

- [ ] **Step 4: CHANGELOG entry (Phase D's job, but reserve a stub)**

Phase D will land the CHANGELOG section. For Phase B, no entry is needed since user-visible behavior hasn't changed (only DB rows added; opt-in to inspect).

---

## Verification checklist

- [ ] `ModelCallEvent` defined in `vox-telemetry::types`
- [ ] `ProviderInferResult` carries split cache fields (`cache_read_input_tokens`, `cache_creation_input_tokens`)
- [ ] `infer.rs` emits `record_event!` after cost reconciliation
- [ ] CLI startup passes `Some(db)` to `init_telemetry_sinks` opportunistically
- [ ] `ResearchMetricsSink` handles `TelemetryEvent::ModelCall` variant
- [ ] Integration test passes — cache tokens preserved in `research_metrics` metadata
- [ ] Existing `CostIncurred` bus emit unchanged (no regression on UsageTracker behavior)
- [ ] `vox-arch-check` green; no inversion introduced

---

## Phase C preview

Phase C wires trace-context propagation: adds `task_id`/`parent_task_id`/`trace_id`/`span_depth` to `RemoteTaskEnvelope`, wraps `dispatch::handle_tool_call` in `TRACE_CTX::scope(...)`, threads context through LLM calls (so the `trace_ctx` populated in this Phase B emit becomes meaningful), and emits `task_root_summary` from `task_tools/lifecycle.rs::complete_task`. The `task_id`/`trace_id` fields populated here in Phase B remain `None`/synthetic until C lands.
