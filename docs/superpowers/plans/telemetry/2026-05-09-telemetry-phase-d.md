# Telemetry Phase D — Master Config + Build Summary + Error Event + Doctor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the user-visible default-on flip, the master `VOX_TELEMETRY=on|off|debug` switch, the `vox telemetry doctor` inspection subcommand, the `build_summary_event` mirror, the `error_event` taxonomy at three known retry sites, and the per-task aggregator that fills in the placeholder fields left in Phase C's `task_root_summary`.

**Architecture:**
- **Master switch**: `VOX_TELEMETRY` env var (and per-user / per-org config files later) is read once into `TelemetryConfig` at startup. Existing legacy gates (`should_emit_llm_cost_events`, `VOX_BENCHMARK_TELEMETRY`) consult the master via a new `vox_telemetry::config::is_master_enabled()` helper. Master-off makes everything a no-op; master-on lets the legacy per-category gates continue to work as overrides; default flips local collection on.
- **Build summary**: emit a `BuildSummaryEvent` from `vox-cli/src/commands/ci/build_timings.rs` immediately after `db.insert_build_run(...)` returns `Ok(run_id)`. Mirror summary fields only — per-crate detail stays in `build_crate_sample`.
- **Error event**: emit at three sites — HTTP errors in `vox-actor-runtime/src/llm/chat.rs`, 429 rate-limits in `vox-orchestrator-mcp/src/llm_bridge/infer.rs`, and circuit-breaker trips in `vox-orchestrator/src/circuit_breaker.rs` (state transition Closed→Open).
- **Per-task aggregator**: an in-memory `TaskTelemetryAggregator` indexed by `task_id` accumulates `total_input_tokens`/`total_output_tokens`/`total_cost_usd`/`child_call_count`/`max_span_depth`/`subagent_fanout` as `model_call_event` and `agent_dispatch_event` are emitted. Cleared when `task_root_summary` is emitted.
- **`vox telemetry doctor`**: new subcommand under existing `vox telemetry` parent. Prints resolved config, registered sinks, sensitivity caps, and a sample of next-upload payload.

**Prerequisite:** Phase C is merged (trace propagation working, `task_root_summary` emits placeholder aggregates).

**Spec:** `docs/src/architecture/telemetry-unification-design-2026.md` § Phase D + § Trust posture.

**Semantic change:**
- **Default-on flip**: fresh users get local collection without setting any env var. Documented in CHANGELOG under Telemetry. Override is one-line: `VOX_TELEMETRY=off`.
- **Two new metric types**: `build.summary` (S0) and `telemetry.error` (S1).
- `task_root_summary` aggregates become accurate (was zero-placeholders in Phase C).
- ADR 023 unchanged. Remote upload still requires explicit `vox telemetry upload` and Clavis-backed credentials.

---

## File map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `crates/vox-telemetry/src/config.rs` | Add `from_env_master()`, `is_master_enabled()`; flip default to default-on |
| Modify | `crates/vox-telemetry/src/lib.rs` | Re-export `is_master_enabled` |
| Modify | `crates/vox-telemetry/src/types.rs` | Add `BuildSummaryEvent`, `ErrorEvent`, two new variants |
| Modify | `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs` | Master gate check in `should_emit_llm_cost_events`; emit error_event on 429 |
| Modify | `crates/vox-cli/src/benchmark_telemetry.rs` | Master gate check in `record_opt_with_unit` |
| Modify | `crates/vox-actor-runtime/src/llm/chat.rs` | Emit error_event on HTTP failure |
| Modify | `crates/vox-orchestrator/src/circuit_breaker.rs` | Emit error_event on Closed→Open transition |
| Modify | `crates/vox-orchestrator/Cargo.toml` | Verify `vox-telemetry` dep (added in Phase C) |
| Create | `crates/vox-telemetry/src/aggregator.rs` | `TaskTelemetryAggregator` keyed by task_id |
| Modify | `crates/vox-telemetry/src/recorder.rs` | `CompositeRecorder` notifies aggregator alongside sinks |
| Modify | `crates/vox-cli/src/commands/ci/build_timings.rs` | Emit `build_summary_event` after `insert_build_run` |
| Modify | `crates/vox-db/src/telemetry_sink.rs` | Handle `BuildSummary` and `Error` variants |
| Modify | `crates/vox-orchestrator-mcp/src/task_tools/lifecycle.rs` | Read aggregated values from aggregator instead of zeros |
| Create | `crates/vox-cli/src/commands/telemetry/doctor.rs` | New `vox telemetry doctor` subcommand |
| Modify | `crates/vox-cli/src/commands/telemetry/mod.rs` | Register `doctor` subcommand |
| Modify | `CHANGELOG.md` | Add `### Telemetry` entry under `## [Unreleased]` |
| Modify | `docs/src/reference/env-vars.md` | Document `VOX_TELEMETRY` master |

---

## Task 1 — Add master switch to `TelemetryConfig`

**Files:**
- Modify: `crates/vox-telemetry/src/config.rs`
- Modify: `crates/vox-telemetry/src/lib.rs`

- [ ] **Step 1: Replace `from_env_legacy` with `from_env_master` and add `is_master_enabled` helper**

In `crates/vox-telemetry/src/config.rs`, replace the body of `from_env_legacy` (or rename to `from_env`) with master-switch logic:

```rust
impl TelemetryConfig {
    /// Resolve the active config from env vars and (eventually) config files.
    ///
    /// Resolution order (highest wins):
    ///   1. `VOX_TELEMETRY` master (`off|on|debug`)
    ///   2. Legacy per-category env vars
    ///   3. Default: local-on, remote-off, all categories on
    pub fn from_env() -> Self {
        let master = std::env::var("VOX_TELEMETRY").ok().map(|v| v.to_ascii_lowercase());
        match master.as_deref() {
            Some("off") | Some("0") | Some("false") => return Self::all_off(),
            Some("on") | Some("1") | Some("true") | Some("debug") | None | Some("") => {}
            _ => {} // unknown values fall through to default
        }

        let benchmark_legacy = env_flag("VOX_BENCHMARK_TELEMETRY");
        let mcp_cost_legacy = env_flag("VOX_MCP_LLM_COST_EVENTS");

        Self {
            enabled: true,
            remote_upload: false,
            // Legacy gates default to default-on UNLESS explicitly turned off.
            research_metrics: benchmark_legacy.unwrap_or(true),
            model_calls: mcp_cost_legacy.unwrap_or(true),
            agent_orchestration: true,
            build: true,
            errors: true,
        }
    }
}

/// Returns true if telemetry is allowed at all (master switch is not "off").
///
/// This is the single check that legacy gates should consult before doing
/// their per-category checks. When this returns false, NO telemetry should
/// be emitted regardless of legacy env vars.
pub fn is_master_enabled() -> bool {
    match std::env::var("VOX_TELEMETRY")
        .ok()
        .map(|v| v.to_ascii_lowercase())
        .as_deref()
    {
        Some("off") | Some("0") | Some("false") => false,
        _ => true,
    }
}
```

- [ ] **Step 2: Update `Default for TelemetryConfig` to use the new method**

```rust
impl Default for TelemetryConfig {
    fn default() -> Self {
        Self::from_env()
    }
}
```

Remove the old `from_env_legacy` if it's still there.

- [ ] **Step 3: Re-export `is_master_enabled` from `lib.rs`**

In `crates/vox-telemetry/src/lib.rs`, add to the `pub use config::` line:

```rust
pub use config::{TelemetryConfig, is_master_enabled};
```

- [ ] **Step 4: Add unit tests**

In `crates/vox-telemetry/src/config.rs`, add tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_local_on_remote_off() {
        // SAFETY: tests run in isolation; remove env var first to ensure deterministic state.
        // SAFETY: Single-threaded test setup before any other code reads env.
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
            std::env::remove_var("VOX_BENCHMARK_TELEMETRY");
            std::env::remove_var("VOX_MCP_LLM_COST_EVENTS");
        }
        let cfg = TelemetryConfig::from_env();
        assert!(cfg.enabled);
        assert!(!cfg.remote_upload);
        assert!(cfg.research_metrics);
        assert!(cfg.model_calls);
        assert!(cfg.build);
    }

    #[test]
    fn master_off_disables_everything() {
        unsafe {
            std::env::set_var("VOX_TELEMETRY", "off");
        }
        let cfg = TelemetryConfig::from_env();
        assert!(!cfg.enabled);
        assert!(!cfg.research_metrics);
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
    }

    #[test]
    fn is_master_enabled_responds_to_master_off() {
        unsafe {
            std::env::set_var("VOX_TELEMETRY", "off");
        }
        assert!(!is_master_enabled());
        unsafe {
            std::env::set_var("VOX_TELEMETRY", "on");
        }
        assert!(is_master_enabled());
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
        assert!(is_master_enabled()); // unset = default-on
    }
}
```

NOTE: Env-mutating tests are inherently non-thread-safe with cargo's parallel test runner. If failures occur, gate these with `#[serial_test::serial]` (add `serial_test` to dev-deps) or run with `cargo test -p vox-telemetry -- --test-threads=1`.

- [ ] **Step 5: Run tests**

```bash
cargo test -p vox-telemetry -- --test-threads=1
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-telemetry/src/config.rs crates/vox-telemetry/src/lib.rs
git commit -m "feat(vox-telemetry): VOX_TELEMETRY master switch + default-on local collection"
```

---

## Task 2 — Add `BuildSummaryEvent` and `ErrorEvent`

**Files:**
- Modify: `crates/vox-telemetry/src/types.rs`
- Modify: `crates/vox-telemetry/src/lib.rs`
- Modify: `crates/vox-db/src/telemetry_sink.rs`

- [ ] **Step 1: Add the two new structs after `TaskRootSummaryEvent` in `types.rs`**

```rust
/// Build summary mirrored from `build_run` after `vox ci build-timings`. Sensitivity: **S0**.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildSummaryEvent {
    pub repository_id: String,
    pub run_id: i64,
    pub profile: String,
    pub total_ms: u64,
    pub crate_count: u32,
    pub fresh_count: u32,
    pub critical_path_crate: Option<String>,
    pub critical_path_ms: u64,
    pub incremental: bool,
    pub dep_fingerprint_changed: bool,
    pub rustc_version: Option<String>,
}

/// Generic subsystem error / retry event. Sensitivity: **S1**.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorEvent {
    pub subsystem: String, // "llm.http" | "llm.rate_limit" | "circuit_breaker" | …
    pub error_class: String, // "HttpStatus500" | "RateLimit429" | "CircuitOpen" | …
    pub retry_attempt: u32,
    pub recoverable: bool,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub task_id: Option<u64>,
    pub trace_id: Option<String>,
    pub message: Option<String>,
}
```

- [ ] **Step 2: Add the two variants to `TelemetryEvent`**

```rust
pub enum TelemetryEvent {
    ResearchMetric(ResearchMetricEvent),
    ModelCall(ModelCallEvent),
    TaskRootSummary(TaskRootSummaryEvent),
    /// Phase D: build run summary mirror.
    BuildSummary(BuildSummaryEvent),
    /// Phase D: subsystem error / retry event.
    Error(ErrorEvent),
}
```

- [ ] **Step 3: Re-export both from `lib.rs`**

Add `BuildSummaryEvent`, `ErrorEvent` to the `pub use types::{ ... }` block.

- [ ] **Step 4: Add match arms in `ResearchMetricsSink`**

In `crates/vox-db/src/telemetry_sink.rs`, before the `_ =>` wildcard arm:

```rust
            TelemetryEvent::BuildSummary(e) => {
                let db = Arc::clone(&self.db);
                let e = e.clone();
                tokio::spawn(async move {
                    let session_id = format!("build:{}", e.repository_id);
                    let metadata_json = match serde_json::to_string(&e) {
                        Ok(s) => Some(s),
                        Err(err) => {
                            tracing::warn!(?err, "ResearchMetricsSink: build_summary serialize failed");
                            return;
                        }
                    };
                    if let Err(err) = db
                        .append_research_metric(
                            &session_id,
                            vox_telemetry::METRIC_TYPE_BUILD_SUMMARY_EVENT,
                            Some(e.total_ms as f64),
                            metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: build_summary write failed");
                    }
                });
            }
            TelemetryEvent::Error(e) => {
                let db = Arc::clone(&self.db);
                let e = e.clone();
                tokio::spawn(async move {
                    let session_id = match &e.task_id {
                        Some(tid) => format!("error:{tid}"),
                        None => format!("error:{}", e.subsystem),
                    };
                    let metadata_json = match serde_json::to_string(&e) {
                        Ok(s) => Some(s),
                        Err(err) => {
                            tracing::warn!(?err, "ResearchMetricsSink: error_event serialize failed");
                            return;
                        }
                    };
                    if let Err(err) = db
                        .append_research_metric(
                            &session_id,
                            vox_telemetry::METRIC_TYPE_ERROR_EVENT,
                            Some(e.retry_attempt as f64),
                            metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: error_event write failed");
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
git commit -m "feat(vox-telemetry): add BuildSummaryEvent + ErrorEvent variants and sink handlers"
```

---

## Task 3 — Per-task aggregator

**Files:**
- Create: `crates/vox-telemetry/src/aggregator.rs`
- Modify: `crates/vox-telemetry/src/lib.rs`
- Modify: `crates/vox-telemetry/src/recorder.rs`
- Modify: `crates/vox-orchestrator-mcp/src/task_tools/lifecycle.rs`

- [ ] **Step 1: Write `aggregator.rs`**

```rust
// crates/vox-telemetry/src/aggregator.rs
//! Per-task in-memory accumulator. Populated as `ModelCallEvent` and other
//! per-task events are recorded; drained by the orchestrator when emitting
//! `TaskRootSummaryEvent` at task completion.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::types::{ModelCallEvent, TelemetryEvent};

#[derive(Debug, Default, Clone)]
pub struct TaskAggregate {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub child_call_count: u32,
    pub max_span_depth: u16,
    pub subagent_fanout: u32,
}

static AGGREGATOR: OnceLock<Mutex<HashMap<u64, TaskAggregate>>> = OnceLock::new();

fn map() -> &'static Mutex<HashMap<u64, TaskAggregate>> {
    AGGREGATOR.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Update aggregate for the task referenced by the event (if any).
pub fn observe(event: &TelemetryEvent) {
    let TelemetryEvent::ModelCall(m) = event else { return };
    let Some(task_id) = m.task_id else { return };
    let mut guard = map().lock().expect("aggregator lock");
    let entry = guard.entry(task_id).or_default();
    entry.total_input_tokens = entry.total_input_tokens.saturating_add(m.prompt_tokens as u64);
    entry.total_output_tokens = entry.total_output_tokens.saturating_add(m.completion_tokens as u64);
    entry.total_cost_usd += m.cost_usd;
    entry.child_call_count = entry.child_call_count.saturating_add(1);
}

/// Increment fanout / depth tracking. Called from agent_dispatch_event sites.
pub fn observe_subagent_dispatch(parent_task_id: u64, child_span_depth: u16) {
    let mut guard = map().lock().expect("aggregator lock");
    let entry = guard.entry(parent_task_id).or_default();
    entry.subagent_fanout = entry.subagent_fanout.saturating_add(1);
    if child_span_depth > entry.max_span_depth {
        entry.max_span_depth = child_span_depth;
    }
}

/// Take and clear the aggregate for the given task_id.
pub fn take(task_id: u64) -> TaskAggregate {
    let mut guard = map().lock().expect("aggregator lock");
    guard.remove(&task_id).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ModelCallEvent;

    #[test]
    fn observe_then_take_returns_accumulated_totals() {
        let mk = |tid: Option<u64>, pt: u32, ct: u32, cost: f64| TelemetryEvent::ModelCall(ModelCallEvent {
            model: "m".into(),
            provider: "p".into(),
            route_profile: None,
            prompt_tokens: pt,
            completion_tokens: ct,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
            latency_ms: 1,
            cost_usd: cost,
            cost_source: "estimated".into(),
            error_class: None,
            retry_attempt: 0,
            task_id: tid,
            parent_task_id: None,
            trace_id: None,
            caller_agent_id: None,
        });

        let task = 9999u64;
        observe(&mk(Some(task), 100, 50, 0.01));
        observe(&mk(Some(task), 200, 75, 0.02));
        let agg = take(task);
        assert_eq!(agg.total_input_tokens, 300);
        assert_eq!(agg.total_output_tokens, 125);
        assert!((agg.total_cost_usd - 0.03).abs() < 1e-9);
        assert_eq!(agg.child_call_count, 2);
    }
}
```

- [ ] **Step 2: Add `pub mod aggregator;` and re-export to `lib.rs`**

```rust
pub mod aggregator;
pub use aggregator::{TaskAggregate, observe as aggregator_observe, take as aggregator_take};
```

- [ ] **Step 3: Auto-observe in `CompositeRecorder::record`**

In `crates/vox-telemetry/src/recorder.rs`, modify `CompositeRecorder::record`:

```rust
impl TelemetryRecorder for CompositeRecorder {
    fn record(&self, event: &TelemetryEvent) {
        crate::aggregator::observe(event);
        for r in &self.inner {
            r.record(event);
        }
    }
}
```

- [ ] **Step 4: Update `task_tools/lifecycle.rs::complete_task` to drain aggregator**

Replace the placeholder zeros in the Phase C emit. In each `complete_task`/`fail_task`/`doubt_task` arm, replace:

```rust
total_input_tokens: 0,
total_output_tokens: 0,
total_cost_usd: 0.0,
child_call_count: 0,
max_span_depth: trace_ctx.span_depth,
subagent_fanout: 0,
```

with:

```rust
let agg = vox_telemetry::aggregator_take(params.task_id);
// ...
total_input_tokens: agg.total_input_tokens,
total_output_tokens: agg.total_output_tokens,
total_cost_usd: agg.total_cost_usd,
child_call_count: agg.child_call_count,
max_span_depth: agg.max_span_depth.max(trace_ctx.span_depth),
subagent_fanout: agg.subagent_fanout,
```

The `let agg = ...` binding goes immediately above the `record_event!` call.

- [ ] **Step 5: Test and commit**

```bash
cargo test -p vox-telemetry aggregator
cargo build -p vox-orchestrator-mcp
```

Expected: pass + clean build.

```bash
git add crates/vox-telemetry/ crates/vox-orchestrator-mcp/src/task_tools/lifecycle.rs
git commit -m "feat(vox-telemetry): per-task aggregator; task_root_summary now reports real totals"
```

---

## Task 4 — Wire master switch into legacy gates

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs`
- Modify: `crates/vox-cli/src/benchmark_telemetry.rs`

- [ ] **Step 1: Update `should_emit_llm_cost_events` to consult the master**

In `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs`, find `fn should_emit_llm_cost_events` (line ~42). Add a master-check at the very top:

```rust
fn should_emit_llm_cost_events(state: &ServerState) -> bool {
    if !vox_telemetry::is_master_enabled() {
        return false;
    }
    // ... existing body unchanged ...
}
```

- [ ] **Step 2: Update `vox-cli/src/benchmark_telemetry.rs` opt-in helpers**

Find `record_opt` / `record_opt_with_unit`. Add the master gate near the top of each:

```rust
pub async fn record_opt(...) {
    if !vox_telemetry::is_master_enabled() {
        return;
    }
    // ... existing body unchanged ...
}
```

Add the import at the top of the file:

```rust
use vox_telemetry; // (workspace dep already present)
```

If `vox-cli/Cargo.toml` doesn't have `vox-telemetry`, add it (Phase A should have done this — verify).

- [ ] **Step 3: Build**

```bash
cargo build -p vox-orchestrator-mcp -p vox-cli
```

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs crates/vox-cli/src/benchmark_telemetry.rs
git commit -m "feat: legacy telemetry gates honor VOX_TELEMETRY master switch"
```

---

## Task 5 — Emit `build_summary_event` from `build_timings.rs`

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/build_timings.rs`

- [ ] **Step 1: Add the emit immediately after `db.insert_build_run(...)` returns `Ok(run_id)`**

In `crates/vox-cli/src/commands/ci/build_timings.rs` around line 472-494, the existing block:

```rust
let run_id = match db
    .insert_build_run(
        repo_id, run_name.as_deref(), rustc_ver.as_deref(),
        &summary.profile, summary.total_ms, crate_count, fresh_count, dep_fp.as_deref(),
    )
    .await
{
    Ok(id) => id,
    Err(e) => { /* … */ return false; }
};
```

After `Ok(id) => id,` (or right after the `let run_id = …;` block ends), add:

```rust
    // Phase D: mirror summary into research_metrics for trend analysis.
    let critical_path = summary
        .crates
        .iter()
        .max_by_key(|c| c.elapsed_ms)
        .map(|c| (c.name.clone(), c.elapsed_ms));
    let (critical_path_crate, critical_path_ms) = match critical_path {
        Some((n, m)) => (Some(n), m),
        None => (None, 0),
    };
    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::BuildSummary(
        vox_telemetry::BuildSummaryEvent {
            repository_id: repo_id.to_string(),
            run_id,
            profile: summary.profile.clone(),
            total_ms: summary.total_ms,
            crate_count: crate_count as u32,
            fresh_count: fresh_count as u32,
            critical_path_crate,
            critical_path_ms,
            incremental: fresh_count > 0,
            dep_fingerprint_changed: dep_fp.as_deref().map(|fp| !summary.previous_dep_fingerprint
                .as_deref().map(|prev| prev == fp).unwrap_or(false)).unwrap_or(false),
            rustc_version: rustc_ver.clone(),
        }
    ));
```

NOTE: `summary.crates`, `c.name`, `c.elapsed_ms`, `summary.previous_dep_fingerprint` — verify these accessor names exist on the `summary` type. If not, adapt to whatever the actual struct uses; if some fields don't exist (e.g., `previous_dep_fingerprint`), use `false` for `dep_fingerprint_changed` and refine in a follow-up.

```bash
grep -n "struct BuildSummary\|struct CrateSample" crates/vox-cli/src/commands/ci/build_timings.rs | head -10
```

- [ ] **Step 2: Build and run a real build to verify a row lands**

```bash
cargo build -p vox-cli
cargo run -p vox-cli -- ci build-timings --quick 2>&1 | tail -10
sqlite3 .vox/store.db "SELECT session_id, metric_type, metric_value FROM research_metrics WHERE metric_type = 'build.summary' ORDER BY rowid DESC LIMIT 1;"
```

Expected: one row with `metric_type = "build.summary"` and `metric_value = total_ms`.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-cli/src/commands/ci/build_timings.rs
git commit -m "feat(vox-cli/ci): mirror build_run summary into research_metrics"
```

---

## Task 6 — Emit `error_event` at three retry sites

**Files:**
- Modify: `crates/vox-actor-runtime/src/llm/chat.rs`
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs`
- Modify: `crates/vox-orchestrator/src/circuit_breaker.rs`

- [ ] **Step 1: Site 1 — HTTP non-2xx in `chat.rs`**

In `crates/vox-actor-runtime/src/llm/chat.rs`, find the existing `if !res.status().is_success() { ... }` block (around line 78-90). Immediately after `let err_msg = format!(...)` and before the existing `record_telemetry_attempt` call, add:

```rust
    let trace_ctx = vox_telemetry::current_trace_ctx();
    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::Error(
        vox_telemetry::ErrorEvent {
            subsystem: "llm.http".into(),
            error_class: format!("HttpStatus{}", status.as_u16()),
            retry_attempt: 0,
            recoverable: status.is_server_error(),
            provider: Some(config.provider.clone()),
            model: Some(config.model.clone()),
            task_id: trace_ctx.task_id,
            trace_id: Some(trace_ctx.trace_id.to_string()),
            message: Some(err_text.chars().take(500).collect()),
        }
    ));
```

- [ ] **Step 2: Site 2 — 429 rate limit in `infer.rs`**

In `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs`, find the existing `if e.status == 429 {` block (around line 564-575). Inside the block, before the existing `tracker.mark_rate_limited(...)` call:

```rust
    let trace_ctx = vox_telemetry::current_trace_ctx();
    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::Error(
        vox_telemetry::ErrorEvent {
            subsystem: "llm.rate_limit".into(),
            error_class: "RateLimit429".into(),
            retry_attempt: 0,
            recoverable: true,
            provider: Some(usage.provider.clone()),
            model: Some(usage.model.clone()),
            task_id: trace_ctx.task_id,
            trace_id: Some(trace_ctx.trace_id.to_string()),
            message: None,
        }
    ));
```

- [ ] **Step 3: Site 3 — circuit breaker Closed→Open transition**

In `crates/vox-orchestrator/src/circuit_breaker.rs`, find the state-transition site that moves the breaker from `Closed` to `Open`. Add the emit at the moment of trip:

```bash
grep -n "Open\|trip\|fn trip\|state =\|Closed" crates/vox-orchestrator/src/circuit_breaker.rs | head -15
```

Locate the trip point. After the state mutation, add:

```rust
    let trace_ctx = vox_telemetry::current_trace_ctx();
    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::Error(
        vox_telemetry::ErrorEvent {
            subsystem: "circuit_breaker".into(),
            error_class: "CircuitOpen".into(),
            retry_attempt: 0,
            recoverable: true,
            provider: None,
            model: None,
            task_id: trace_ctx.task_id,
            trace_id: Some(trace_ctx.trace_id.to_string()),
            message: Some(format!("circuit_breaker tripped: {}", reason)),
        }
    ));
```

If `vox-orchestrator/Cargo.toml` does not yet depend on `vox-telemetry` (added in Phase C), add it under `[dependencies]`:

```toml
vox-telemetry = { workspace = true }
```

- [ ] **Step 4: Build**

```bash
cargo build -p vox-actor-runtime -p vox-orchestrator -p vox-orchestrator-mcp
```

Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-actor-runtime/src/llm/chat.rs \
        crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs \
        crates/vox-orchestrator/src/circuit_breaker.rs \
        crates/vox-orchestrator/Cargo.toml
git commit -m "feat: emit error_event at HTTP 5xx, rate-limit, and circuit-breaker trip sites"
```

---

## Task 7 — `vox telemetry doctor` subcommand

**Files:**
- Create: `crates/vox-cli/src/commands/telemetry/doctor.rs`
- Modify: `crates/vox-cli/src/commands/telemetry/mod.rs`

- [ ] **Step 1: Locate the existing `vox telemetry` dispatch**

```bash
grep -n "telemetry\|fn dispatch\|TelemetryCmd\|TelemetrySubcommand" crates/vox-cli/src/commands/telemetry/mod.rs | head -20
```

Note the structure (likely a clap-derived enum with subcommand variants).

- [ ] **Step 2: Add a `Doctor` subcommand variant**

In `crates/vox-cli/src/commands/telemetry/mod.rs`:

```rust
pub mod doctor;
```

And in the existing `enum TelemetrySubcommand` (or whatever the type is named), add:

```rust
    /// Inspect the active telemetry configuration, registered sinks, and what would
    /// be uploaded next.
    Doctor,
```

In the dispatch `match`:

```rust
    TelemetrySubcommand::Doctor => doctor::run().await,
```

- [ ] **Step 3: Write `doctor.rs`**

```rust
// crates/vox-cli/src/commands/telemetry/doctor.rs
//! `vox telemetry doctor` — print the resolved telemetry configuration,
//! registered sinks, sensitivity caps, and a sample of pending spool payloads.
//!
//! Read-only and offline. No network calls. Useful for verifying that
//! `VOX_TELEMETRY=off` actually disables emission, or that the spool has
//! pending events you forgot about.

use anyhow::Result;

pub async fn run() -> Result<()> {
    println!("vox telemetry doctor");
    println!("====================\n");

    // 1. Resolved config
    let cfg = vox_telemetry::TelemetryConfig::from_env();
    println!("Resolved config (from env):");
    println!("  master enabled:      {}", cfg.enabled);
    println!("  remote upload:       {}  (ADR 023 — explicit opt-in only)", cfg.remote_upload);
    println!("  research_metrics:    {}", cfg.research_metrics);
    println!("  model_calls:         {}", cfg.model_calls);
    println!("  agent_orchestration: {}", cfg.agent_orchestration);
    println!("  build:               {}", cfg.build);
    println!("  errors:              {}", cfg.errors);
    println!();

    // 2. Master env vars
    println!("Environment variables:");
    for var in [
        "VOX_TELEMETRY",
        "VOX_BENCHMARK_TELEMETRY",
        "VOX_SYNTAX_K_TELEMETRY",
        "VOX_MCP_LLM_COST_EVENTS",
        "VOX_TELEMETRY_UPLOAD_URL",
        "VOX_TELEMETRY_SPOOL_DIR",
    ] {
        match std::env::var(var) {
            Ok(v) if !v.is_empty() => println!("  {var:30} = {v}"),
            _ => println!("  {var:30} = <unset>"),
        }
    }
    println!();

    // 3. Spool status
    let spool_root = crate::telemetry_spool::spool_root();
    println!("Spool root: {}", spool_root.display());
    let pending = crate::telemetry_spool::list_pending(&spool_root)
        .unwrap_or_default();
    println!("  pending uploads: {}", pending.len());
    if !pending.is_empty() {
        println!("  latest 3:");
        for p in pending.iter().take(3) {
            println!("    - {}", p.display());
        }
    }
    println!();

    // 4. Sensitivity policy
    println!("Sensitivity caps:");
    println!("  SpoolSink:           S0–S1 only by default (S2/S3 require operator opt-in)");
    println!("  ResearchMetricsSink: writes locally; sensitivity gating per category env var");
    println!();

    // 5. ADR pointer
    println!("Trust posture:");
    println!("  Local collection: {}", if cfg.enabled { "on (default)" } else { "OFF" });
    println!("  Remote upload:    explicit only (see `vox telemetry upload`, ADR 023)");
    println!();

    println!("Documentation:");
    println!("  - docs/src/architecture/telemetry-trust-ssot.md");
    println!("  - docs/src/architecture/telemetry-unification-design-2026.md");
    println!("  - docs/src/adr/023-optional-telemetry-remote-upload.md");

    Ok(())
}
```

- [ ] **Step 4: Build and run**

```bash
cargo build -p vox-cli
cargo run -p vox-cli -- telemetry doctor
```

Expected: prints structured sections; no errors. Try also `VOX_TELEMETRY=off cargo run -p vox-cli -- telemetry doctor` and confirm it shows `master enabled: false`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/telemetry/
git commit -m "feat(vox-cli): add `vox telemetry doctor` subcommand for inspection"
```

---

## Task 8 — Documentation: CHANGELOG + env-vars reference

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/src/reference/env-vars.md`

- [ ] **Step 1: Add CHANGELOG entry under `## [Unreleased]`**

In `CHANGELOG.md`, under the existing `## [Unreleased]` section, add (creating subsections as needed):

```markdown
## [Unreleased]

### Added
- **Telemetry — vox-telemetry facade crate (Phase A):** New L1 crate hosting `METRIC_TYPE_*` constants, the `TelemetryRecorder` trait, and the `record_event!` macro. `vox-db::research_metrics_contract` is preserved as a re-export.
- **Telemetry — model_call_event (Phase B):** Per-LLM-call rows now persist to `research_metrics` with split prompt-cache token fields (`cache_read_input_tokens`, `cache_creation_input_tokens`), latency, and reconciled cost.
- **Telemetry — span propagation (Phase C):** `RemoteTaskEnvelope` carries `trace_id`, `parent_task_id`, `span_depth`, and `caller_agent_id`. Tool dispatch wraps in `TRACE_CTX::scope`. New `task.root_summary` rows aggregate per-task totals.
- **Telemetry — build summary, error events, master switch (Phase D):** New `build.summary` and `telemetry.error` metric types. `vox telemetry doctor` subcommand for inspection.

### Changed
- **Telemetry — default-on local collection:** Local `research_metrics` writes are now on by default for new users. Set `VOX_TELEMETRY=off` to disable. Remote upload remains explicit opt-in (no change to ADR 023). Existing per-category env vars (`VOX_BENCHMARK_TELEMETRY`, `VOX_MCP_LLM_COST_EVENTS`, `VOX_SYNTAX_K_TELEMETRY`) continue to be honored as overrides.
```

- [ ] **Step 2: Add `VOX_TELEMETRY` row to `docs/src/reference/env-vars.md`**

Find the telemetry section in `docs/src/reference/env-vars.md` (search for `VOX_BENCHMARK_TELEMETRY`). Add a new row above it:

```markdown
| `VOX_TELEMETRY` | Master switch for all local telemetry collection: `on` / `off` / `debug`. Default: `on`. When `off`, all categories are suppressed regardless of legacy env vars. Remote upload always requires explicit `vox telemetry upload` (ADR 023). | `vox-telemetry`, all emitters |
```

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md docs/src/reference/env-vars.md
git commit -m "docs: announce VOX_TELEMETRY master + default-on flip; document phases A–D"
```

---

## Task 9 — Final verification

- [ ] **Step 1: Workspace test sweep**

```bash
cargo test -p vox-telemetry -p vox-db -p vox-cli -p vox-orchestrator -p vox-orchestrator-mcp -p vox-actor-runtime -- --test-threads=1
```

Expected: all pass.

- [ ] **Step 2: Architecture check**

```bash
cargo run -p vox-arch-check -- .
```

Expected: exits 0.

- [ ] **Step 3: Functional smoke**

Run `vox telemetry doctor` with master on and master off:

```bash
cargo run -p vox-cli -- telemetry doctor
VOX_TELEMETRY=off cargo run -p vox-cli -- telemetry doctor
```

Expected: master toggle visibly changes the `master enabled:` line.

- [ ] **Step 4: End-to-end emission test**

```bash
# Trigger a build with telemetry on
cargo run -p vox-cli -- ci build-timings --quick
sqlite3 .vox/store.db "SELECT metric_type, COUNT(*) FROM research_metrics GROUP BY metric_type ORDER BY 2 DESC;"
```

Expected: rows for `build.summary`, possibly `model_call_event`, `task.root_summary` if a model-driven task ran.

- [ ] **Step 5: Master-off suppresses everything**

```bash
sqlite3 .vox/store.db "SELECT MAX(rowid) FROM research_metrics;"
# Note the rowid.
VOX_TELEMETRY=off cargo run -p vox-cli -- ci build-timings --quick
sqlite3 .vox/store.db "SELECT MAX(rowid) FROM research_metrics;"
# Should be unchanged.
```

Expected: row count does NOT increase under `VOX_TELEMETRY=off`.

- [ ] **Step 6: Update memory and final cleanup**

The in-flight project memory at `~/.claude/projects/.../memory/project_telemetry_unification_2026.md` can be updated to reflect "Phases A–D complete." Either edit it to mark complete or remove it since the codebase now self-documents.

---

## Verification checklist

- [ ] `VOX_TELEMETRY=off` reliably disables ALL telemetry emission (smoke test in Step 5)
- [ ] Default with no env vars set produces telemetry rows on a fresh build
- [ ] `vox telemetry doctor` runs and shows accurate state
- [ ] `build.summary` rows land in `research_metrics` after `vox ci build-timings`
- [ ] `telemetry.error` rows land on simulated HTTP 500, 429, and circuit-breaker trip
- [ ] `task.root_summary` rows now have non-zero aggregates (token totals, cost, fanout)
- [ ] CHANGELOG announces the default-on flip under `[Unreleased] / Changed`
- [ ] `VOX_TELEMETRY` documented in `docs/src/reference/env-vars.md`
- [ ] ADR 023 unchanged; no remote upload triggered by default
- [ ] `vox-arch-check` green; no inversion introduced

---

## Done — what shipped across all four phases

After Phase D merges, the workspace has:

1. **`vox-telemetry` L1 facade** — single source of truth for metric type constants, recorder trait, span context, master config
2. **Five new metrics persisted** — `model_call_event`, `task.root_summary`, `build.summary`, `telemetry.error`, plus enriched `orch.subagent.dispatch`
3. **Trace propagation** — full call tree reconstructable from `research_metrics` joined on `trace_id`
4. **Cache hit rate computable** — `Σ cache_read / (Σ cache_read + Σ prompt_tokens)` from `model_call_event` rows
5. **Cost-per-task computable** — single `task.root_summary` row per task with `total_cost_usd`
6. **Master switch** — one env var to disable everything
7. **Inspectable** — `vox telemetry doctor` shows resolved state
8. **Trust unchanged** — ADR 023 intact; remote upload still requires explicit opt-in and Clavis credentials
