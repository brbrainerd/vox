# Telemetry Phase C — Span Propagation + Task Root Summary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make agent call trees reconstructable. Propagate `TraceContext` (task_id, parent_task_id, trace_id, span_depth, caller_agent_id) across A2A envelopes, MCP dispatch boundaries, and LLM calls. Emit a `task_root_summary` row when a top-level task completes — providing a single row per task that aggregates total tokens, cost, depth, and fanout.

**Architecture:** Add four new fields to `RemoteTaskEnvelope` (all `Option`, serde-defaulted for backward compat). Wrap `dispatch::handle_tool_call` body in `TRACE_CTX::scope(ctx, fut)` so any code reachable from a tool call (including LLM calls and downstream dispatches) sees the context via `current_trace_ctx()`. Add `TaskRootSummary` variant to `TelemetryEvent` and emit it from `task_tools/lifecycle.rs::complete_task` after `complete_task_with_attestation` returns `Ok`. Also enrich the existing `agent_dispatch_event` (orch.subagent.dispatch) `metadata_json` with `parent_task_id`, `span_depth`, `dispatch_latency_ms`.

**Prerequisite:** Phase B is merged (`ModelCallEvent` exists; `current_trace_ctx()` is being read by `infer.rs` but currently returns a default empty context).

**Spec:** `docs/src/architecture/telemetry-unification-design-2026.md` § Phase C.

**Semantic change:**
- `RemoteTaskEnvelope` JSON gains four new optional fields. Older receivers ignore them; older senders omit them. Backward-compatible.
- A new `task.root_summary` row appears in `research_metrics` per top-level task completion.
- The Phase B `model_call_event` rows now have populated `task_id`/`parent_task_id`/`trace_id` instead of `None` defaults.
- `metric_type = "orch.subagent.dispatch"` rows gain three new fields in `metadata_json`. Downstream readers must accept missing fields.

---

## File map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `crates/vox-orchestrator/src/a2a/envelope.rs` | Add `parent_task_id`, `caller_agent_id`, `trace_id`, `span_depth` fields |
| Modify | `crates/vox-orchestrator-mcp/Cargo.toml` | (Already added in B) — verify |
| Modify | `crates/vox-orchestrator-mcp/src/dispatch.rs` | Wrap `handle_tool_call` body in `TRACE_CTX::scope` |
| Modify | `crates/vox-orchestrator-mcp/src/task_tools/lifecycle.rs` | Emit `task_root_summary` after successful `complete_task` |
| Modify | `crates/vox-orchestrator-mcp/src/task_tools/lifecycle.rs` | Same for `fail_task` (with `outcome = "failed"`) |
| Modify | `crates/vox-orchestrator/Cargo.toml` | Add `vox-telemetry = { workspace = true }` |
| Modify | `crates/vox-telemetry/src/types.rs` | Add `TaskRootSummaryEvent` + `TelemetryEvent::TaskRootSummary` variant |
| Modify | `crates/vox-telemetry/src/lib.rs` | Re-export `TaskRootSummaryEvent` |
| Modify | `crates/vox-db/src/telemetry_sink.rs` | Handle `TelemetryEvent::TaskRootSummary` arm |
| Modify | `crates/vox-actor-runtime/src/llm/chat.rs` | Read trace_id from `current_trace_ctx()` instead of minting per-call |
| Modify | `crates/vox-actor-runtime/Cargo.toml` | Add `vox-telemetry = { workspace = true }` |
| Modify | (subagent dispatch site, locate via grep) | Enrich `metric_type = "orch.subagent.dispatch"` `metadata_json` |
| Create | `crates/vox-orchestrator-mcp/tests/trace_propagation.rs` | Integration test for 3-deep call tree |

---

## Task 1 — Extend `RemoteTaskEnvelope` with trace fields

**Files:**
- Modify: `crates/vox-orchestrator/src/a2a/envelope.rs`

- [ ] **Step 1: Add four new fields to `RemoteTaskEnvelope`**

Open `crates/vox-orchestrator/src/a2a/envelope.rs`. After the existing `thread_id` field (line ~50), add:

```rust
    /// Phase C: parent task id propagated from the caller's TRACE_CTX. None for root tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<u64>,
    /// Phase C: agent identifier that issued this dispatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_agent_id: Option<String>,
    /// Phase C: trace identifier shared across the entire call tree (UUID v4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Phase C: number of agent-to-agent hops from the root; root = 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_depth: Option<u16>,
```

- [ ] **Step 2: Verify backward compat — write a test that an envelope without the new fields deserializes**

Add to `crates/vox-orchestrator/src/a2a/envelope.rs` `#[cfg(test)] mod tests { ... }` (create the module at end of file if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_envelope_without_trace_fields_deserializes() {
        // Simulates an older sender producing JSON without the Phase C fields.
        let json = r#"{
            "idempotency_key": "k1",
            "task_id": 7,
            "repository_id": "repo",
            "capability_requirements_json": "{}",
            "payload": "test"
        }"#;
        let envelope: RemoteTaskEnvelope = serde_json::from_str(json).expect("deserialize");
        assert_eq!(envelope.task_id, 7);
        assert!(envelope.parent_task_id.is_none());
        assert!(envelope.trace_id.is_none());
        assert!(envelope.span_depth.is_none());
    }

    #[test]
    fn envelope_with_trace_fields_round_trips() {
        let envelope = RemoteTaskEnvelope {
            idempotency_key: "k1".into(),
            task_id: 7,
            repository_id: "repo".into(),
            capability_requirements_json: "{}".into(),
            payload: "test".into(),
            privacy_class: None,
            populi_scope_id: None,
            submitted_unix_ms: None,
            exec_lease_id: None,
            campaign_id: None,
            artifact_refs_json: None,
            session_id: None,
            thread_id: None,
            context_envelope_json: None,
            harness_spec_json: None,
            parent_task_id: Some(5),
            caller_agent_id: Some("agent-3".into()),
            trace_id: Some("trace-123".into()),
            span_depth: Some(2),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let back: RemoteTaskEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.parent_task_id, Some(5));
        assert_eq!(back.span_depth, Some(2));
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p vox-orchestrator --lib a2a::envelope
```

Expected: both new tests pass; existing tests unaffected.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator/src/a2a/envelope.rs
git commit -m "feat(orchestrator): add trace propagation fields to RemoteTaskEnvelope"
```

---

## Task 2 — Add `TaskRootSummaryEvent` to `vox-telemetry`

**Files:**
- Modify: `crates/vox-telemetry/src/types.rs`
- Modify: `crates/vox-telemetry/src/lib.rs`

- [ ] **Step 1: Add `TaskRootSummaryEvent` struct in `types.rs`**

After `ModelCallEvent`, add:

```rust
/// Top-level task completion rollup. Persisted as `research_metrics` row with
/// `metric_type = METRIC_TYPE_TASK_ROOT_SUMMARY`.
///
/// One row per top-level task. Aggregates totals across all child agent calls
/// and LLM calls within the task. Sensitivity: **S1 (OperationalTracing)**.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskRootSummaryEvent {
    pub task_id: u64,
    pub trace_id: String,
    pub repository_id: Option<String>,
    /// "completed" | "failed" | "doubted" | "cancelled"
    pub outcome: String,
    pub wall_time_ms: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub child_call_count: u32,
    pub max_span_depth: u16,
    pub subagent_fanout: u32,
}
```

- [ ] **Step 2: Add `TaskRootSummary` variant to the enum**

```rust
pub enum TelemetryEvent {
    ResearchMetric(ResearchMetricEvent),
    ModelCall(ModelCallEvent),
    /// Phase C: top-level task completion rollup.
    TaskRootSummary(TaskRootSummaryEvent),
    // Phase D adds: BuildSummary, Error
}
```

- [ ] **Step 3: Re-export from `lib.rs`**

Add `TaskRootSummaryEvent` to the `pub use types::{` block in `crates/vox-telemetry/src/lib.rs`.

- [ ] **Step 4: Update `ResearchMetricsSink` to handle the new variant**

In `crates/vox-db/src/telemetry_sink.rs`, add the match arm before the `_ =>` wildcard:

```rust
            TelemetryEvent::TaskRootSummary(e) => {
                let db = Arc::clone(&self.db);
                let e = e.clone();
                tokio::spawn(async move {
                    let session_id = format!("task:{}", e.task_id);
                    let metadata_json = match serde_json::to_string(&e) {
                        Ok(s) => Some(s),
                        Err(err) => {
                            tracing::warn!(?err, "ResearchMetricsSink: task_root_summary serialize failed");
                            return;
                        }
                    };
                    if let Err(err) = db
                        .append_research_metric(
                            &session_id,
                            vox_telemetry::METRIC_TYPE_TASK_ROOT_SUMMARY,
                            Some(e.total_cost_usd),
                            metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: task_root_summary write failed");
                    }
                });
            }
```

- [ ] **Step 5: Build and test**

```bash
cargo test -p vox-telemetry -p vox-db
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-telemetry/ crates/vox-db/src/telemetry_sink.rs
git commit -m "feat(vox-telemetry): add TaskRootSummaryEvent + sink handler"
```

---

## Task 3 — Wrap `dispatch::handle_tool_call` in `TRACE_CTX::scope`

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`

- [ ] **Step 1: Build a `TraceContext` from the parsed `trace_for_telemetry` and run the inner dispatch under it**

In `crates/vox-orchestrator-mcp/src/dispatch.rs`, find `handle_tool_call` (line ~25). After `trace_for_telemetry` is computed (around line 48) and before `handle_tool_call_inner` is called (around line 119), add the scope wrapper.

Current shape:
```rust
let result = handle_tool_call_inner(state, name_canonical, args, ...).await;
```

Replace with a context-aware dispatch:

```rust
use vox_telemetry::{TRACE_CTX, TraceContext};
use uuid::Uuid;

let mut trace_ctx = TraceContext::default();
if let Some(tid_str) = trace_for_telemetry.as_deref() {
    if let Ok(parsed) = Uuid::parse_str(tid_str) {
        trace_ctx.trace_id = parsed;
    }
}
// Synthesize a task_id from the orchestrator if available; otherwise leave None.
trace_ctx.task_id = args
    .get("task_id")
    .and_then(|v| v.as_u64());
trace_ctx.parent_task_id = args
    .get("parent_task_id")
    .and_then(|v| v.as_u64());
trace_ctx.caller_agent_id = agent_id.map(ToString::to_string);
trace_ctx.span_depth = args
    .get("span_depth")
    .and_then(|v| v.as_u64())
    .map(|d| d.min(u16::MAX as u64) as u16)
    .unwrap_or(0);

let result = TRACE_CTX
    .scope(trace_ctx, handle_tool_call_inner(state, name_canonical, args, /* … existing args … */))
    .await;
```

NOTE: The `handle_tool_call_inner` signature must be invocable as a `Future`. If the existing call is already an `await`ed expression, the `.scope(ctx, fut).await` form works directly. Verify by reading lines 119-130 carefully.

- [ ] **Step 2: Add `vox-telemetry` import at the top of `dispatch.rs`**

If not already present, add to the imports section:

```rust
use vox_telemetry::{TraceContext, TRACE_CTX};
use uuid::Uuid;
```

- [ ] **Step 3: Build**

```bash
cargo build -p vox-orchestrator-mcp
```

Expected: clean build. If `handle_tool_call_inner` cannot be wrapped this way (e.g., it returns early on error), refactor minimally: extract the body into an async block that the `scope` wraps.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/dispatch.rs
git commit -m "feat(orchestrator-mcp): scope tool dispatch under TRACE_CTX for span propagation"
```

---

## Task 4 — Thread trace context through LLM calls in `vox-actor-runtime`

**Files:**
- Modify: `crates/vox-actor-runtime/Cargo.toml`
- Modify: `crates/vox-actor-runtime/src/llm/chat.rs`

- [ ] **Step 1: Add `vox-telemetry` dep**

In `crates/vox-actor-runtime/Cargo.toml` under `[dependencies]`:

```toml
vox-telemetry = { workspace = true }
```

- [ ] **Step 2: Replace per-call UUID mint with context inheritance**

In `crates/vox-actor-runtime/src/llm/chat.rs`, locate the `let trace_id = Uuid::new_v4()` line (or equivalent). Replace it with:

```rust
let trace_id = vox_telemetry::current_trace_ctx().trace_id;
```

If the function uses the trace_id as a `String`, adjust:

```rust
let trace_id = vox_telemetry::current_trace_ctx().trace_id.to_string();
```

If `vox_telemetry::current_trace_ctx()` returns a default (no scope active), the UUID is freshly minted by the default impl — preserving the existing behavior for orphan calls outside any task.

- [ ] **Step 3: Build**

```bash
cargo build -p vox-actor-runtime
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-actor-runtime/
git commit -m "feat(actor-runtime): LLM trace_id inherits from TRACE_CTX (was per-call mint)"
```

---

## Task 5 — Emit `task_root_summary` from `complete_task`

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/task_tools/lifecycle.rs`

- [ ] **Step 1: Track task start time so `wall_time_ms` is meaningful**

Task lifecycle currently doesn't track start time in `lifecycle.rs`. The orchestrator does — it already records `task_started_at`. Verify:

```bash
grep -rn "task_started_at\|started_at\|task_start" crates/vox-orchestrator/src --include="*.rs" | head -10
```

If a `task_started_at` accessor exists on `Orchestrator`, use it. Otherwise, the simplest path is to record `wall_time_ms = 0` for now and refine in Phase D when richer task lifecycle hooks land. Mark this with a comment.

- [ ] **Step 2: Add aggregation accessors (or use what exists)**

Ideally, `Orchestrator` exposes per-task aggregates (total_input_tokens, total_cost_usd, child_call_count). If these don't exist, the initial implementation reads them lazily by querying recent `model_call_event` rows joined on `task_id`. For Phase C ship-ability, take the lazy-aggregate path: the `complete_task` emit writes a placeholder `0` for aggregates, and Phase D adds in-memory counters for accurate values.

A minimal `task_root_summary` emit with placeholder aggregates:

```rust
// In complete_task, after the Ok(()) match arm and before returning ToolResult::ok:
let trace_ctx = vox_telemetry::current_trace_ctx();
vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::TaskRootSummary(
    vox_telemetry::TaskRootSummaryEvent {
        task_id: params.task_id,
        trace_id: trace_ctx.trace_id.to_string(),
        repository_id: None, // populated from state if available
        outcome: "completed".into(),
        wall_time_ms: 0, // refined in Phase D when lifecycle exposes start time
        total_input_tokens: 0, // refined in Phase D with in-memory counter
        total_output_tokens: 0,
        total_cost_usd: 0.0,
        child_call_count: 0,
        max_span_depth: trace_ctx.span_depth,
        subagent_fanout: 0,
    }
));
```

Insert this BLOCK into the `Ok(()) =>` arm at line 27, immediately after the gamification block (after line 49), but before the `ToolResult::ok(...)` return at line 50:

Final shape:
```rust
        Ok(()) => {
            // Gamification: update the agent-scoped companion (matches event_router / HUD).
            if let (Some(db), Some(aid)) = (&state.db, assigned) {
                // ... existing gamification code unchanged ...
            }

            // Phase C: emit task_root_summary (aggregates refined in Phase D).
            let trace_ctx = vox_telemetry::current_trace_ctx();
            vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::TaskRootSummary(
                vox_telemetry::TaskRootSummaryEvent {
                    task_id: params.task_id,
                    trace_id: trace_ctx.trace_id.to_string(),
                    repository_id: None,
                    outcome: "completed".into(),
                    wall_time_ms: 0,
                    total_input_tokens: 0,
                    total_output_tokens: 0,
                    total_cost_usd: 0.0,
                    child_call_count: 0,
                    max_span_depth: trace_ctx.span_depth,
                    subagent_fanout: 0,
                }
            ));

            ToolResult::ok("task completed".to_string()).to_json()
        }
```

- [ ] **Step 3: Repeat for `fail_task` with `outcome = "failed"`**

In the same file, find the `fail_task` function. Add the same `record_event!` block in its `Ok(()) =>` arm with `outcome: "failed".into()`.

- [ ] **Step 4: Repeat for `doubt_task` with `outcome = "doubted"`**

If a `doubt_task` function exists in this file, add the same emit with `outcome: "doubted".into()`.

- [ ] **Step 5: Add the `vox-telemetry` import**

At the top of `crates/vox-orchestrator-mcp/src/task_tools/lifecycle.rs`, ensure the imports include `vox_telemetry`. The `record_event!` macro is referenced via `vox_telemetry::record_event!` so no `use` is strictly required if the path is fully qualified. Check by building.

- [ ] **Step 6: Build**

```bash
cargo build -p vox-orchestrator-mcp
```

Expected: clean build.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/task_tools/lifecycle.rs
git commit -m "feat(orchestrator-mcp): emit task_root_summary on complete/fail/doubt"
```

---

## Task 6 — Enrich `agent_dispatch_event` (orch.subagent.dispatch) metadata

**Files:**
- Modify: subagent dispatch site (locate via grep)

- [ ] **Step 1: Locate the existing dispatch event emit**

```bash
grep -rn "METRIC_TYPE_SUBAGENT_DISPATCH\|orch.subagent.dispatch" crates/vox-orchestrator/src crates/vox-orchestrator-mcp/src --include="*.rs"
```

Note the file:line.

- [ ] **Step 2: Add three fields to the existing `metadata_json`**

The existing emit serializes a JSON object as `metadata_json`. Add three keys to that object:

```rust
// Inside the existing serde_json::json!({ ... }) literal that builds metadata_json:
"parent_task_id": parent_task_id,        // u64 from current_trace_ctx()
"span_depth": span_depth,                // u16 from current_trace_ctx()
"dispatch_latency_ms": dispatch_latency_ms, // computed from start..now
```

If the existing code uses `serde_json::json!(...)`, it accepts these new keys directly. If it uses a typed `serde_json::Value::Object`, add them via `.insert()`.

Pull values:

```rust
let trace_ctx = vox_telemetry::current_trace_ctx();
let parent_task_id = trace_ctx.parent_task_id;
let span_depth = trace_ctx.span_depth;
// dispatch_latency_ms is presumably already computed as `start.elapsed()`; use that.
```

- [ ] **Step 3: Build and verify**

```bash
cargo build -p vox-orchestrator
cargo build -p vox-orchestrator-mcp
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add <the modified file>
git commit -m "feat(orchestrator): enrich subagent dispatch metadata with parent_task_id + span_depth"
```

---

## Task 7 — Integration test: 3-deep call tree records correct depth

**Files:**
- Create: `crates/vox-orchestrator-mcp/tests/trace_propagation.rs`

- [ ] **Step 1: Write the integration test**

```rust
// crates/vox-orchestrator-mcp/tests/trace_propagation.rs
//! Phase C integration test: synthesize a 3-deep nested TRACE_CTX scope and verify
//! that emitted `model_call_event` rows record correct `parent_task_id` and `span_depth`
//! at every level.

use std::sync::{Arc, Mutex};

use vox_telemetry::{
    ModelCallEvent, TRACE_CTX, TelemetryEvent, TelemetryRecorder, TraceContext, current_trace_ctx,
    record_event, set_global_recorder,
};

/// Capturing recorder for assertions.
struct CaptureRecorder {
    events: Arc<Mutex<Vec<TelemetryEvent>>>,
}

impl TelemetryRecorder for CaptureRecorder {
    fn record(&self, event: &TelemetryEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

fn emit_model_call() {
    let ctx = current_trace_ctx();
    record_event!(&TelemetryEvent::ModelCall(ModelCallEvent {
        model: "test".into(),
        provider: "test".into(),
        route_profile: None,
        prompt_tokens: 1,
        completion_tokens: 1,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        latency_ms: 10,
        cost_usd: 0.0,
        cost_source: "estimated".into(),
        error_class: None,
        retry_attempt: 0,
        task_id: ctx.task_id,
        parent_task_id: ctx.parent_task_id,
        trace_id: Some(ctx.trace_id.to_string()),
        caller_agent_id: ctx.caller_agent_id,
    }));
}

#[tokio::test]
async fn three_deep_call_tree_records_span_depth() {
    let events = Arc::new(Mutex::new(Vec::<TelemetryEvent>::new()));
    set_global_recorder(Arc::new(CaptureRecorder {
        events: events.clone(),
    }));

    // Root task scope (depth 0).
    let root = TraceContext::root(100);
    TRACE_CTX
        .scope(root.clone(), async {
            emit_model_call();

            // Child 1 (depth 1).
            let child1 = current_trace_ctx().child(101, "agent-1");
            TRACE_CTX
                .scope(child1, async {
                    emit_model_call();

                    // Child 2 (depth 2).
                    let child2 = current_trace_ctx().child(102, "agent-2");
                    TRACE_CTX
                        .scope(child2, async {
                            emit_model_call();
                        })
                        .await;
                })
                .await;
        })
        .await;

    let events = events.lock().unwrap();
    assert_eq!(events.len(), 3, "expected 3 emitted events");

    let extract = |e: &TelemetryEvent| -> (Option<u64>, Option<u64>, String) {
        let TelemetryEvent::ModelCall(m) = e else { panic!("wrong variant") };
        (m.task_id, m.parent_task_id, m.trace_id.clone().unwrap())
    };

    let (task0, parent0, trace0) = extract(&events[0]);
    let (task1, parent1, trace1) = extract(&events[1]);
    let (task2, parent2, trace2) = extract(&events[2]);

    assert_eq!(task0, Some(100));
    assert_eq!(parent0, None);
    assert_eq!(task1, Some(101));
    assert_eq!(parent1, Some(100));
    assert_eq!(task2, Some(102));
    assert_eq!(parent2, Some(101));

    // Trace ID is shared across the entire tree.
    assert_eq!(trace0, trace1);
    assert_eq!(trace1, trace2);
}
```

- [ ] **Step 2: Add `tempfile` and `tokio` to vox-orchestrator-mcp dev-dependencies if not already present**

```bash
grep -A 5 "\\[dev-dependencies\\]" crates/vox-orchestrator-mcp/Cargo.toml
```

Ensure `tokio = { workspace = true, features = ["macros", "rt", "rt-multi-thread"] }` is present.

- [ ] **Step 3: Run the test**

```bash
cargo test -p vox-orchestrator-mcp --test trace_propagation
```

Expected: PASS. The trace_id is shared; span depth and parent_task_id are correctly populated at each level.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator-mcp/tests/trace_propagation.rs
git commit -m "test(orchestrator-mcp): 3-deep call tree records correct span depth + parent linkage"
```

---

## Task 8 — Final verification

- [ ] **Step 1: Workspace test sweep**

```bash
cargo test -p vox-telemetry -p vox-orchestrator -p vox-orchestrator-mcp -p vox-actor-runtime -p vox-db -p vox-cli
```

Expected: all tests pass.

- [ ] **Step 2: Architecture check**

```bash
cargo run -p vox-arch-check -- .
```

Expected: exits 0.

- [ ] **Step 3: Smoke test — run a real LLM-driven task and verify a task_root_summary row lands**

```bash
sqlite3 .vox/store.db "SELECT session_id, metric_type, metric_value FROM research_metrics WHERE metric_type = 'task.root_summary' ORDER BY rowid DESC LIMIT 5;"
```

Expected: at least one row after running any `vox <command>` that completes a task.

- [ ] **Step 4: Cross-row consistency check**

For a single recent task, verify model_call_event rows share the same `trace_id` as the task_root_summary row:

```bash
sqlite3 .vox/store.db "
SELECT metric_type, json_extract(metadata_json, '$.trace_id') AS trace_id
FROM research_metrics
WHERE metric_type IN ('model_call_event', 'task.root_summary')
ORDER BY rowid DESC
LIMIT 10;"
```

Expected: matching `trace_id` values across rows in the same logical task.

---

## Verification checklist

- [ ] `RemoteTaskEnvelope` has `parent_task_id`, `caller_agent_id`, `trace_id`, `span_depth` (all `Option`, serde-default)
- [ ] Legacy envelope JSON without new fields still deserializes (backward-compat test passes)
- [ ] `dispatch::handle_tool_call` body runs under `TRACE_CTX::scope`
- [ ] `vox-actor-runtime/src/llm/chat.rs` reads `trace_id` from current context, not per-call `Uuid::new_v4`
- [ ] `task_tools/lifecycle.rs::complete_task` / `fail_task` emit `task_root_summary`
- [ ] `agent_dispatch_event` metadata enriched with `parent_task_id`, `span_depth`, `dispatch_latency_ms`
- [ ] Integration test passes — 3-deep call tree records correct linkage
- [ ] Phase B `model_call_event` rows now have populated trace fields (no longer `None`)
- [ ] No semantic regression in existing `agent_dispatch_event` consumers (extra fields ignored)

---

## Phase D preview

Phase D adds the user-visible config UX: `VOX_TELEMETRY=on/off/debug` master switch, default-on flip for local writes, `vox telemetry doctor` subcommand, build_summary mirror after `insert_build_run`, and `error_event` emission at retry sites (HTTP errors, 429 rate limits, circuit breaker trips). It also fills in the placeholder `wall_time_ms`/`total_input_tokens`/`total_cost_usd` aggregates in `task_root_summary` by adding a per-task counter to the orchestrator state.
