---
title: "Telemetry unification design 2026"
description: "Runtime architecture for unifying Vox telemetry emission behind an L1 facade crate, with default-on local collection, durable model performance metrics, and propagated agent call trees. Supersedes the deferred 'open questions' from the 2026-Q1 trust-governance pass."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-05-09"
training_eligible: false
training_rationale: "Architecture design doc; references internal contracts and code paths."

schema_type: "TechArticle"
---

# Telemetry unification design 2026

## Purpose

The 2026-Q1 telemetry effort completed a **trust-governance and classification** pass: SSoT documents, sensitivity classes (S0–S3), retention policy, ADR 023 for optional remote upload, the local spool in `vox-cli`, and rustdoc classification on every producer. That work explicitly deferred the runtime architecture, listing as open questions:

- Canonical event taxonomy for a unified telemetry plane
- Single ingestion API
- Redaction standards per field class
- Governance process for new fields

This document closes those questions. It defines a runtime architecture that:

1. Lets domain crates emit telemetry through a single facade trait without depending on storage, sinks, or the orchestrator.
2. Persists model performance data (cache hit rate, cost-per-call, latency) that is currently captured but discarded.
3. Propagates a trace context across agent-to-agent boundaries so a task's full call tree is reconstructable.
4. Adds a build-summary metric that mirrors existing build-run rows for trend analysis.
5. Flips the default to **local collection on, remote upload off** without changing ADR 023.

## Status

- **Type:** design (not yet implemented)
- **Supersedes:** the open questions in [Telemetry unification research findings 2026](../archive/research-2026-q1/telemetry-unification-research-findings-2026.md)
- **Builds on:** [Telemetry trust SSoT](telemetry-trust-ssot.md), [ADR 023](../adr/023-optional-telemetry-remote-upload.md)
- **Next step:** implementation plan via the writing-plans skill, then phased PRs

## Background: what exists today

The `vox-telemetry` audit performed for this design found five parallel persistence paths:

1. `research_metrics` table — the canonical event log; 19 metric types defined in [`crates/vox-db/src/research_metrics_contract.rs`](../../../crates/vox-db/src/research_metrics_contract.rs). Eight wrapper modules in `vox-db` write through `append_research_metric`.
2. `build_run` / `build_crate_sample` / `build_warning` tables — populated by `vox ci build-timings` ([`crates/vox-cli/src/commands/ci/build_timings.rs`](../../../crates/vox-cli/src/commands/ci/build_timings.rs)).
3. `routing_decisions.reason_json` — orchestrator routing telemetry.
4. LLM cost events — **ephemeral**, only on the orchestrator event bus ([`crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs`](../../../crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs)).
5. Mens JSONL streams — separate files.

Material findings from the audit:

- All 8 telemetry wrapper modules live **inside** `vox-db`, so no cycles exist today; cycles only become a risk once emitters move to higher-layer crates.
- Anthropic's `cache_read_input_tokens` and `cache_creation_input_tokens` **are** parsed today, latency **is** measured, cost **is** computed — but none of it lands in `research_metrics`. It is consumed by the in-memory `UsageTracker` and discarded.
- [`crates/vox-orchestrator/src/a2a/envelope.rs`](../../../crates/vox-orchestrator/src/a2a/envelope.rs) has no `parent_task_id`, `caller_agent_id`, `span_depth`, or `trace_id`. Trace IDs are minted per-LLM-call, not threaded through the call tree.
- `VOX_BENCHMARK_TELEMETRY` and `VOX_SYNTAX_K_TELEMETRY` default off; the gates live at the **call site**, not on the DB write. Flipping the default to on is a small change, not a schema migration.
- The metric type constants in `vox-db::research_metrics_contract` are the SSoT for taxonomy. They sit at L3, which forces any L1/L2 emitter to depend upward — the structural reason there is no facade today.

## Goals and non-goals

### Goals

- Single emission API used by every domain crate (`record_event!` macro plus a `TelemetryRecorder` trait).
- Pure-types layer hosting canonical event definitions, depended on by everyone, depending on nothing domain-specific.
- Durable persistence of model call performance data including cache token usage and latency.
- Trace context propagated across A2A envelopes, dispatch boundaries, and LLM calls.
- Build-summary metric mirrored into `research_metrics` for trend analysis without disturbing the rich `build_run` tables.
- Master config switch (`VOX_TELEMETRY=on|off|debug`) with hierarchy: org policy > user config > env > default.
- Default: local collection on, remote upload off.
- `vox doctor telemetry` subcommand: print resolved config, registered sinks, and what would be uploaded next.

### Non-goals

- Changing ADR 023. Remote upload remains explicit opt-in.
- Touching S3 content-bearing stores (`codex_chat`, transcript inserts). Those have separate lifecycles.
- Migrating `build_run`/`build_crate_sample` rows into `research_metrics`. Mirror the summary only.
- Introducing OpenTelemetry/OTLP. The design leaves room for an OTLP sink as a future addition; this iteration does not include it.
- Redefining the existing 19 metric types. They are well-classified and contract-tested.

## Architecture

### A new L1 facade crate: `vox-telemetry`

```
crates/vox-telemetry/
  Cargo.toml          # deps: serde, serde_json, tracing, tokio (task_local only)
  src/
    lib.rs            # re-exports: types, recorder, span, macros
    types.rs          # TelemetryEvent enum, sensitivity classes, metric type constants
    recorder.rs       # TelemetryRecorder trait + global handle (OnceCell)
    span.rs           # TraceContext: parent_task_id, span_depth, trace_id (task_local)
    config.rs         # TelemetryConfig: master switch, per-category, source order
    macros.rs         # record_event!, record_model_call!, with_span! macros
    no_op.rs          # default recorder when none registered
```

**Layer placement: L1**, sibling to `vox-secrets` and `vox-openai-wire`. This requires moving the canonical metric type constants from `vox-db::research_metrics_contract` (L3) down to `vox-telemetry::types` (L1). `vox-db` then re-exports them so the existing `crates/vox-db/src/research_metrics_contract.rs` API is preserved for any external readers.

A row will be added to [`where-things-live.md`](where-things-live.md) under L1 in the same PR that creates the crate.

### Emission contract

Domain crates depend on `vox-telemetry` and call:

```rust
use vox_telemetry::{record_event, ModelCallEvent};

record_event!(ModelCallEvent {
    model: spec.id.clone(),
    provider: spec.provider_type.clone(),
    prompt_tokens: usage.prompt_tokens,
    completion_tokens: usage.completion_tokens,
    cache_read_input_tokens: usage.cache_read_input_tokens,
    cache_creation_input_tokens: usage.cache_creation_input_tokens,
    latency_ms: elapsed.as_millis() as u64,
    cost_usd: cost,
    error_class: None,
    retry_attempt: 0,
});
```

The macro:
- Looks up the global recorder (set once at process start by the binary).
- Auto-injects the current `TraceContext` (parent_task_id, span_depth, trace_id).
- Is a no-op when no recorder is registered (zero cost in tests, library use, and contexts without a runtime).

### Sink registration

Sinks are higher-layer adapters and register themselves at startup in binaries. The facade defines no sinks itself.

| Sink | Crate | Persistence target |
|---|---|---|
| `ResearchMetricsSink` | `vox-db` | `research_metrics` table via existing `append_research_metric` |
| `SpoolSink` | `vox-cli` | `.vox/telemetry-upload-queue/pending/` for ADR 023 upload flow |
| `StdoutSink` | `vox-telemetry-debug` (test crate) or feature flag | JSON to stderr for `vox doctor` |
| `BuildSummarySink` | `vox-cli` | Persists `build_summary_event` records emitted by `vox ci build-timings` into `research_metrics`; the rich `build_run`/`build_crate_sample` rows continue to be written by the existing path. |

A `CompositeRecorder` fan-outs to multiple sinks. The default `vox-cli` `main` registers `ResearchMetricsSink` + `SpoolSink` + a stdout sink in debug mode.

### Trace context propagation

Implemented as a `tokio::task_local!` cell:

```rust
tokio::task_local! {
    pub static TRACE_CONTEXT: TraceContext;
}

pub struct TraceContext {
    pub task_id: u64,
    pub parent_task_id: Option<u64>,
    pub trace_id: Uuid,
    pub span_depth: u16,
    pub caller_agent_id: Option<AgentId>,
}
```

Propagation points:

- **A2A envelope** ([`a2a/envelope.rs`](../../../crates/vox-orchestrator/src/a2a/envelope.rs)): adds `parent_task_id`, `caller_agent_id`, `trace_id`, `span_depth` fields. Sender writes from `TRACE_CONTEXT::get()`; receiver re-establishes context with `span_depth + 1`.
- **MCP dispatch** ([`crates/vox-orchestrator-mcp/src/dispatch.rs`](../../../crates/vox-orchestrator-mcp/src/dispatch.rs)): wraps the tool invocation in `TRACE_CONTEXT::scope(...)`.
- **LLM call** ([`llm/chat.rs`](../../../crates/vox-actor-runtime/src/llm/chat.rs)): replaces `Uuid::new_v4()` per-call mint with `TRACE_CONTEXT::get().trace_id` when present.

### Configuration hierarchy

Resolution order (highest wins, single read at startup):

1. `/etc/vox/telemetry-policy.toml` — org-level hard-off enforcement
2. `~/.config/vox/config.toml` — user preference
3. `VOX_TELEMETRY` env (master) and legacy `VOX_*_TELEMETRY` env (per-category)
4. Default: `{ enabled: true, remote_upload: false, categories: all-on }`

The legacy env vars stay supported as overrides so existing operators are not surprised. The master switch makes "turn it all off" a one-step operation.

### Sensitivity propagation

Each `TelemetryEvent` variant carries a `Sensitivity` constant matching the S0–S3 classes from the trust SSoT. Sinks can refuse to persist events above a configured threshold. The `SpoolSink` in particular caps at S1 by default — S2/S3 require explicit per-source opt-in via the existing per-category gates and never reach the spool unless the operator changes default config.

## High-value new metrics

These are the five metrics whose absence the audit identified. Each is named with a stable metric type constant and slot into the existing `research_metrics` shape.

### 1. `model_call_event` (S1)

Persisted per-LLM-call. Replaces the discard-on-bus path.

Fields: `model`, `provider`, `route_profile`, `prompt_tokens`, `completion_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`, `latency_ms`, `cost_usd`, `error_class`, `retry_attempt`, `parent_task_id`, `caller_agent_id`, `trace_id`.

Unlocks: cache hit rate over time, cost-per-task, p95 latency by model and route, token efficiency.

### 2. Trace context fields on `agent_dispatch_event` (S1)

Extends the existing dispatch event metadata with `parent_task_id`, `span_depth`, `dispatch_latency_ms`, `caller_agent_id`. No new metric type — adds fields to `metadata_json`.

Unlocks: agent call-tree reconstruction.

### 3. `task_root_summary` (S1)

Emitted at top-level task completion. Fields: `task_id`, `total_tokens_in`, `total_tokens_out`, `total_cost_usd`, `wall_time_ms`, `child_call_count`, `max_span_depth`, `subagent_fanout`, `outcome`.

Unlocks: per-task aggregates without GROUP BY over millions of leaf rows; quick "where did the cost go" queries.

### 4. `build_summary_event` (S0)

Emitted after every `vox ci build-timings` run, mirrored from the existing `build_run` row. Fields: `profile`, `total_ms`, `n_fresh`, `n_compiled`, `critical_path_crate`, `critical_path_ms`, `incremental: bool`, `dep_fingerprint_changed: bool`.

Unlocks: incremental cache health trend, critical-path tracking. Per-crate detail stays in `build_crate_sample`.

### 5. `error_event` (S1)

Generic class for retry-able subsystem failures. Fields: `subsystem`, `error_class`, `retry_attempt`, `recoverable: bool`, `parent_task_id`.

Unlocks: per-subsystem reliability, retry-storm detection.

## Phasing

| Phase | Scope | Output | Reversibility |
|---|---|---|---|
| **A** | Create `vox-telemetry` crate. Move metric type constants from `vox-db::research_metrics_contract` to `vox-telemetry::types`. `vox-db` re-exports for compatibility. Add `TelemetryRecorder` trait, no-op default, `record_event!` macro, `TelemetryConfig`, `TraceContext`. Wire `ResearchMetricsSink` in `vox-db`, `SpoolSink` in `vox-cli`. Add row to `where-things-live.md`. No semantic change to existing emissions. | New crate; existing telemetry behavior preserved. | Pure additive. |
| **B** | Persist `model_call_event` through facade. Replace the discard-on-bus path in `llm_bridge/infer.rs` with a sink call. Migrate one existing wrapper module (start with `benchmark_telemetry`) to register through the facade as a proof of pattern. Subsequent wrappers migrate opportunistically; the old `append_research_metric` direct path remains a valid sink-internal call. | Cache hit rate, cost-per-call, model latency are durable. | Additive; old path stays. |
| **C** | Trace context propagation. Add fields to A2A envelope, MCP dispatch, and LLM call sites. Emit `agent_dispatch_event` enrichment and `task_root_summary` at task completion. | Call trees reconstructable; cost-per-task computable. | Schema additive on `metadata_json`; new metric type for task summary. |
| **D** | Master switch `VOX_TELEMETRY=on/off/debug`. Default-on flip for local writes (per-category legacy env vars stay as overrides). `vox doctor telemetry` subcommand. `BuildSummarySink` mirroring `build_run` summaries. `error_event` emission at known retry sites. | User-visible default change — CHANGELOG entry under Telemetry. ADR 023 unchanged. | Behavior change in one direction; reversible by env var. |

Each phase is independently shippable. Phase A is a refactor with no semantic change.

## Trust posture

This design preserves the trust posture established by the 2026-Q1 effort:

- **Local-first.** Default-on collection writes to local DB and local spool only. Network egress requires `vox telemetry upload` and a configured Clavis-backed URL/token.
- **Sensitivity gating.** The default `SpoolSink` caps at S1 — S2/S3 events never enter the upload queue unless an operator explicitly raises the cap. Local writes to `research_metrics` for S2/S3 events continue to follow the existing per-category opt-in gates (`VOX_BENCHMARK_TELEMETRY`, `VOX_SYNTAX_K_TELEMETRY`, `VOX_MCP_LLM_COST_EVENTS`, etc.) and are not changed by this design.
- **Inspectable.** `vox doctor telemetry` shows the resolved config, registered sinks, and a sample of what would be uploaded next.
- **Org override.** A `/etc/vox/telemetry-policy.toml` with `enabled = false` overrides everything — single point for enterprise hard-off.
- **No content collection.** Source code, prompt text, completion text, raw tool args, raw file paths, commit messages remain out of scope for default telemetry. Diagnostics that need this content stay in the explicit user-mediated diagnostic-bundle flow.
- **ADR 023 unchanged.** No remote-upload default change.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Moving metric type constants down to L1 breaks external readers of `vox-db::research_metrics_contract`. | Re-export in place. The module path and constant identifiers stay stable. CI guards in `data-ssot-guards` continue to verify constants are documented. |
| Trace context overhead on hot paths (every LLM call, every dispatch). | `task_local!` is sub-nanosecond on read. The macro's no-op path when no recorder is registered keeps test/library overhead at zero. Benchmarked in Phase A. |
| Default-on local collection surprises users. | CHANGELOG entry under Telemetry; `vox doctor telemetry` makes the state visible; master switch makes opt-out one step. |
| Confusion between `vox-telemetry` (facade) and Populi CLI “Telemetry” branding. | The Populi (`vox-populi`) crate owns ML/AI/Telemetry CLI command surfaces. The facade is plumbing. The `where-things-live.md` row distinguishes them. |
| Sink fan-out latency on high-frequency emit. | `CompositeRecorder` dispatches synchronously by default but each sink can opt into async/buffered behavior. `SpoolSink` always buffers. |

## Open questions

- Should `task_root_summary` be emitted by the orchestrator at task close, or computed lazily on read by aggregating leaf events? (Tentative: emit on close. Computed-on-read is fine for ad-hoc analysis but expensive at scale.)
- Should `vox doctor telemetry` be a new subcommand or an extension of an existing `vox doctor`? (Tentative: extension if `vox doctor` exists; new subcommand under `vox telemetry doctor` otherwise. Confirm during Phase D.)
- Should `error_event` emission be opt-in per subsystem to avoid log-spam during a real incident? (Tentative: yes; same per-category gating model.)

## Verification

Each phase MUST satisfy:

- `cargo run -p vox-arch-check` green (layer enforcement).
- Existing `vox ci` gates green; `data-ssot-guards` extended in Phase A to require new metric type constants are documented in [telemetry-metric-contract](../reference/telemetry-metric-contract.md) and [telemetry-taxonomy-contracts-ssot](../archive/research-2026-q1/telemetry-taxonomy-contracts-ssot.md).
- CHANGELOG entries under the Telemetry subsection for any user-visible behavior change.
- For Phase B: a test that asserts `cache_read_input_tokens` from a recorded model call survives round-trip to `research_metrics`.
- For Phase C: a test that asserts a synthetic 3-deep agent call tree records correct `parent_task_id` and `span_depth` at every level.

## Related

- [Telemetry trust SSoT](telemetry-trust-ssot.md) — overriding policy
- [ADR 023 — optional telemetry remote upload](../adr/023-optional-telemetry-remote-upload.md) — unchanged
- [Telemetry implementation blueprint 2026](../archive/research-2026-q1/telemetry-implementation-blueprint-2026.md) — completed governance pass
- [Telemetry unification research findings 2026](../archive/research-2026-q1/telemetry-unification-research-findings-2026.md) — original research; this design closes its open questions
- [where-things-live.md](where-things-live.md) — to be updated with `vox-telemetry` row in Phase A
- [layers.toml](layers.toml) — L1 placement enforcement
