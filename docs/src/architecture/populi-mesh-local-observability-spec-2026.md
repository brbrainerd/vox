---
title: "Populi Mesh — Local Observability Spec (S1, 2026-05-01)"
description: "Slice S1 child spec for workstream W5 partial. Establishes the vox.mesh.* span-attribute namespace, threads trace_id through the local task path, and prepares the A2A envelope for cross-node propagation in S2 — without yet emitting cross-node traces."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Defines the trace and span-attribute conventions used by all subsequent mesh observability work."
---

# Populi Mesh — Local Observability (S1 child spec)

**Parent.** [`populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md), Slice S1, Workstream W5 partial.

**Goal.** Make a single-node task path traceable end-to-end with a stable `trace_id` that flows from the caller through orchestrator dispatch into populi's A2A inbox and out to the executor. Define the `vox.mesh.*` span-attribute namespace once. Add (but do not yet require) a `traceparent` field on the A2A envelope so S2's cross-node propagation only has to wire the receiver side.

**Non-goals.**
- **Cross-node trace propagation** — S2 spec.
- **OpenTelemetry collector setup or remote upload** — that lives with [`telemetry-driven-cost-accounting-research-2026.md`](telemetry-driven-cost-accounting-research-2026.md) and the optional remote sink ADR-023; this spec only defines what's *emitted*, not where it goes.
- **GenAI semconv coverage** — that's a model-routing concern, not a mesh concern; this spec only defines the `vox.mesh.*` subset.
- **Replacing `tracing` crate.** The project uses `tracing` everywhere; we extend, not replace.

---

## Part 1 — Current state

- **`tracing` crate** is used for structured logs throughout `vox-populi` ([example](../../../crates/vox-populi/src/transport/handlers/mod.rs:722)).
- **`trace_id` column** exists in `vox-db`'s `llm_interactions` and `attempt_log` tables as a TEXT field. It's populated for LLM calls, queried by `get_last_interaction_trace_id` ([crates/vox-db/src/store/ops_scientia.rs](../../../crates/vox-db/src/store/ops_scientia.rs)).
- **No OpenTelemetry crate** is in use.
- **No `traceparent` field** on `A2ADeliverRequest` ([transport/mod.rs:55-88](../../../crates/vox-populi/src/transport/mod.rs:55)).
- **No mesh-specific span attributes**; mesh-relevant logs use ad-hoc keys (`scope_id`, `claimer`, `entry.scope_id`).

So we have the foundations of distributed tracing (trace_id in DB), but no propagation layer and no namespace discipline.

---

## Part 2 — Design

### 2.1 The `vox.mesh.*` attribute namespace

Single document of record (Appendix A in `populi.md`). The S1-relevant attributes:

| Attribute | Type | When set |
|-----------|------|----------|
| `vox.mesh.trace_id` | string (32 lowercase hex, W3C-compatible) | Every span in a mesh-touching code path. |
| `vox.mesh.peer_id` | string | Spans on the receiver side OR spans naming a remote peer. |
| `vox.mesh.lease_id` | string | Spans whose work is governed by a lease (S1: lease type defined; S2: actually issued). |
| `vox.mesh.dispatch_kind` | enum (`local` / `remote` / `fallback`) | Spans inside the orchestrator dispatch loop. |
| `vox.mesh.message_id` | string | A2A inbox spans. |
| `vox.mesh.message_type` | string | A2A inbox spans. |
| `vox.mesh.privacy_class` | enum (`public` / `private` / `trusted`) | A2A inbox spans (when set on the request). |
| `vox.mesh.store.op` | string | Store spans (per `populi-mesh-a2a-durability-spec`). |
| `vox.mesh.store.duration_ms` | u64 | Store spans. |
| `vox.mesh.probe.name` | string | Probe pipeline spans (per `populi-mesh-probe-correctness-spec`). |
| `vox.mesh.probe.outcome` | enum | Probe pipeline spans. |

Rules:
- Names use snake_case after the `vox.mesh.` prefix.
- Booleans appear as enums (`yes` / `no` / `unknown`) when "not set" is meaningful.
- Strings are bounded; truncate at 256 chars with a `…` suffix and an attribute `vox.mesh.<key>.truncated = true`.
- No PII or secret material. Specifically: never log `payload`, `jwe_payload`, bearer tokens.

### 2.2 The `MeshTraceContext`

```rust
// vox:skip
#[derive(Debug, Clone)]
pub struct MeshTraceContext {
    pub trace_id: TraceId,        // 16-byte, hex-encoded for serialization
    pub parent_span_id: SpanId,   // 8-byte
    pub trace_flags: u8,          // W3C trace flags (sampled bit only, for now)
}

impl MeshTraceContext {
    pub fn new_root() -> Self { /* random trace_id, random parent */ }
    pub fn from_traceparent(s: &str) -> Result<Self, ParseTraceparentError> { /* W3C format */ }
    pub fn to_traceparent(&self) -> String { /* W3C format */ }
    pub fn child(&self) -> Self { /* same trace_id, new parent */ }
}
```

Encoding: `00-{trace_id}-{span_id}-{flags}` (W3C `traceparent`). This is forward-compatible with OpenTelemetry without depending on the crate.

`MeshTraceContext` lives in a new module `vox-populi/src/observability.rs`. Re-exported from `vox-mesh-types` so the orchestrator can construct it without depending on `vox-populi`.

### 2.3 Threading the context

**Producer side (orchestrator).**

A new method `MeshTraceContext::current_or_new()` reads the active `tracing` span and either inherits or creates a root context. The dispatch path attaches the context to the work it builds.

**Wire-level propagation.**

`A2ADeliverRequest` gains an additive optional field:

```rust
// vox:skip
/// W3C traceparent (e.g. "00-{32hex}-{16hex}-01"). When present, the receiver
/// SHOULD continue the trace.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub traceparent: Option<String>,
```

S1 sets this field on every dispatch. S2 will *consume* it on the receiver. S1 receivers ignore it (forward-compat).

**`A2AStoredMessage`** gains the same field, so the durable store carries trace context across crashes.

**Receiver side (S1).**

In S1, the populi A2A handlers extract the `traceparent` if present and attach it as `vox.mesh.trace_id` on their span — this gives single-node loops (orchestrator dispatch → in-process populi handler) a complete trace. They do *not* reconstruct a span graph from it; that's S2.

### 2.4 Telemetry sinks

`tracing` spans are configured to forward `vox.mesh.*` attributes to the existing telemetry pipeline that writes `llm_interactions.trace_id`. No new collector. The schema column `trace_id` already exists; we just consistently populate it from `MeshTraceContext::trace_id`.

A new optional table `mesh_spans` may be added in a follow-on schema migration (backlog item to add: `MESH-212 [obs] mesh_spans table for ad-hoc mesh observability separate from llm_interactions`). Out of scope for this spec.

### 2.5 Sampling

S1 ships **always-sample** for mesh paths. Power-user dogfood doesn't yet need cost-aware sampling. Backlog item if the volume becomes a problem.

---

## Part 3 — Test plan

### 3.1 Unit tests

`vox-populi/src/observability.rs::tests`:
- `traceparent_round_trip` — parse and serialize a known-good traceparent.
- `traceparent_rejects_malformed` — bad version bytes, wrong field count, non-hex chars all return errors.
- `child_preserves_trace_id_and_changes_span_id` — sanity.
- `current_or_new_inherits_when_in_span` — inside a `tracing::info_span!`, returns a context that inherits the active span's trace_id.
- `current_or_new_creates_root_when_outside` — outside any span, creates a fresh trace.

### 3.2 Integration tests

`vox-populi/tests/local_trace_propagation.rs`:
- `local_dispatch_trace_threads_through` — submit a task in-process, capture all spans emitted, assert every span carries the same `vox.mesh.trace_id`.
- `traceparent_persisted_in_store` — submit an A2A message with a traceparent, kill+restart, confirm the persisted message still carries it.
- `existing_tests_still_pass_with_traceparent_field` — schema-additive verification.

### 3.3 Schema tests

- `a2a_deliver_request_round_trip_with_and_without_traceparent` — both serialize and deserialize cleanly.
- `legacy_message_without_traceparent_loads_as_none` — backward compat.

---

## Part 4 — Acceptance criteria

1. Every span emitted on a mesh-touching code path carries `vox.mesh.trace_id`.
2. The `traceparent` field is present on `A2ADeliverRequest` and `A2AStoredMessage`, additive only.
3. The `vox.mesh.*` namespace document lives in `populi.md` Appendix A and lists every attribute defined in this spec.
4. `llm_interactions.trace_id` rows produced by an in-process mesh-dispatched LLM call match the trace_id on the orchestrator's submit span.
5. No new external dependencies — uses `tracing` and project-local W3C parsing.
6. Backlog items closed: `MESH-011`, `MESH-017`, `MESH-023`–`MESH-024`, `MESH-121`–`MESH-122`, `MESH-127`.

---

## Part 5 — Out-of-scope items punted to follow-on specs

- **Cross-node propagation** — S2 spec (`populi-mesh-trace-propagation-spec`).
- **Sampling, retention, exemplar-based cost-aware sampling** — backlog.
- **Mesh-spans table** — backlog `MESH-212` (to add).
- **OpenTelemetry collector / OTLP export** — depends on the project-wide telemetry-remote-sink decision; out of scope.
- **GenAI semconv attribute completeness** — model-routing concern.

---

## Part 6 — Rough cost

- `MeshTraceContext` + W3C parsing: ~150 LOC.
- Wire-up in orchestrator dispatch: ~80 LOC.
- Wire-up in populi handlers: ~60 LOC.
- Tests: ~250 LOC.
- Doc (Appendix A in populi.md): ~150 lines.

Total: ~700 LOC, no new dependencies.

---

## Revision history

- **2026-05-01.** Initial S1 child spec.
