---
title: "Research, Scientia, and telemetry channels"
description: "How ResearchEvent, research_metrics, and TelemetryEvent relate without cyclic deps."
category: "Architecture SSOTs"
status: "current"
sort_order: 860
---

# Research / Scientia / telemetry channels (SSOT)

This doc is the human-facing map for the three parallel paths described in the convergence plan.
Implementation anchors: `crates/vox-research-events`, `crates/vox-db` (`research_metrics`, `ResearchMetricsSink`),
`crates/vox-telemetry`, `crates/vox-search`, and the composition root (`vox-orchestrator`, MCP, CLI).

## Three channels

| Channel | Role | Consumers |
| --- | --- | --- |
| **`ResearchEvent` bus** | Typed SCIENTIA ladder (`vox_research_events::ResearchEvent`) | In-process subscribers (MCP broadcast, mesh tap) |
| **`research_metrics` SQL** | Durable Tier‑B rows keyed by `session_id` / `metric_type` | Analytics, rolling policy feedback, dashboards |
| **`TelemetryEvent` → `ResearchMetricsSink`** | Generic telemetry facade (`vox_telemetry::TelemetryEvent`) | Same table as metrics; fixture/model/task shaped events |

**Rule:** Every durable research/scientia analytic you rely on long-term must appear under `contracts/telemetry/events.v1.yaml`
(with JSON Schema). Bus-only events do not need a catalog row unless they are also persisted.

## Composition-root bridge

`vox-orchestrator` emits `ResearchEvent` for live subscribers and, when a Codex handle is present,
mirrors a **whitelist** into `research_metrics` via `spawn_persist_research_event_for_metrics`
(metadata carries `telemetry_catalog_id: research-event-bridge`; see `contracts/telemetry/research-event-bridge.v1.schema.json`).

`subqueries_emitted` keeps an explicit `record_research_metric` write with planner JSON metadata (too large for the bus payload shape).

## Retrieval policy feedback

Rolling aggregates for citation precision, self‑verification reliability, and retrieval hit rate are read from
recent `research_metrics` rows (`citation_precision`, `self_verification_reliability`, `retrieval_hit_rate`)
and applied through neutral `vox_search::SearchPolicyFeedback` → `SearchPolicy::with_scientia_feedback`
before local/web gather. Callers may override with `ResearchConfig.search_policy_feedback`.

## Mesh / publication tap

`ServerState::spawn_scientia_research_mesh_background_jobs` starts `spawn_scientia_mesh_research_event_subscriber`
on the MCP `research_events` broadcast **without requiring Codex** (daemon and MCP stdio paths).

With the `news-publish` feature and `OrchestratorConfig::research_mesh_intake_writer_active()` (that is,
`scientia_research_mesh.intake_writer_enabled` **or** `news.enabled`, plus env overlays such as
`VOX_SCIENTIA_RESEARCH_MESH_INTAKE_WRITER`), the subscriber writes validated JSON via
[`vox_publisher::research_mesh`](../../../crates/vox-publisher/src/research_mesh.rs) under
`<repo>/.vox/scientia/research-mesh-intake/` ([`repo_scientia_research_mesh_intake_dir`](../../../crates/vox-config/src/paths.rs)).

**Promotion:** `consume_pending_intake` / `spawn_research_mesh_intake_consumer` move validated files into each queue’s
`processed/` subtree and append JSON lines to `<repo>/.vox/scientia/research-mesh-promoted/events.v1.jsonl`
(path helper: [`repo_scientia_research_mesh_promoted_dir`](../../../crates/vox-config/src/paths.rs)).
CLI: `vox research mesh-intake consume`. Enable the background consumer with `scientia_research_mesh.intake_consumer_poll_enabled`
or `VOX_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL` / `VOX_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL_INTERVAL_MS`.

Contracts:
[`research-mesh-intake.v1.schema.json`](../../../contracts/scientia/research-mesh-intake.v1.schema.json),
[`research-mesh-promoted-line.v1.schema.json`](../../../contracts/scientia/research-mesh-promoted-line.v1.schema.json).

Downstream scholarly jobs should treat intake and promoted rows as **machine-suggested** until human review hooks land.

## Inventory snapshot (metric_type ↔ channel)

| Metric / signal | Bus (`ResearchEvent`) | `research_metrics` | Catalog id |
| --- | --- | --- | --- |
| `research_started` | TelemetryObservation | Bridge | `research-event-bridge` |
| `subqueries_emitted` | AggregateComputed | Direct row (plan JSON) | (plan metadata) |
| `retrieval_hit_rate` | AggregateComputed | Bridge | `research-event-bridge` |
| `sources_total` | TelemetryObservation | Bridge | `research-event-bridge` |
| `citation_precision` | AggregateComputed | Bridge | `research-event-bridge` |
| `self_verification_reliability` | TelemetryObservation | Bridge | `research-event-bridge` |
| `scientia.claim_verified` | ClaimVerified (subset) | Bridge | `research-event-bridge` |
| `scientia.finding_candidate_proposed` | FindingCandidateProposed | Bridge | `research-event-bridge` |
| Fixture / MCP telemetry | — | `append_research_metric` / sink | `fixture-*`, `orch-subagent-dispatch`, … |

## See also

- Search & retrieval SSOT: `search-retrieval-ssot-2026.md`
- Telemetry trust: `telemetry-trust-ssot.md`
- AgentOS overview: `agentos-ssot-2026.md`
