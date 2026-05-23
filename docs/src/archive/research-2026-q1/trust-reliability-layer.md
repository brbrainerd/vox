---
title: "Trust Reliability Layer (SSOT)"
description: "Defines unified trust observation vocabulary, trust_observations / trust_rollups persistence, EWMA rollups, producers and consumers (orchestrator, endpoints, Socrates), degradation/outbox health signals, and MCP/CLI trust surfaces."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Trust Reliability Layer (SSOT)

This document defines the current trust/reliability architecture used by orchestrator routing, Socrates telemetry, endpoint reliability, and downstream analytics.

## Why this exists

The codebase historically had multiple trust-like signals that were useful but partially disconnected:

- `agent_reliability` (Laplace-smoothed task outcomes)
- in-memory `AgentTrustScore` (attention/approval behavior)
- endpoint EWMA metrics (`endpoint_reliability`)
- Socrates turn telemetry (`socrates_surface`)
- file-based MENS/eval artifacts

The unified trust layer adds a common vocabulary and persistence model so these signals can be queried and used together.

## Canonical trust vocabulary

Trust observations are recorded as:

- `entity_type`: `agent`, `endpoint`, `model`, `skill`, `workflow`, `repository`, `evidence_bundle`
- `entity_id`: stable identifier for the entity
- `dimension`: e.g. `task_completion`, `factuality`, `contradiction_rate`, `refusal_propensity`, `latency_reliability`
- `scope`: `domain`, `task_class`, `provider`, `model_id`, `repository_id`
- value + confidence: `observation_value`, `confidence_weight`, `sample_size`
- provenance: `source_kind`, `artifact_ref`, `metadata_json`, `created_at_ms`

## Storage model

Two database tables are the SSOT:

- `trust_observations`: append-only evidence log for replay/audit.
- `trust_rollups`: materialized scoped rollups keyed by `(entity_type, entity_id, dimension, scope...)`.

Current implementation:

- each observation is inserted into `trust_observations`
- each insert updates `trust_rollups.score` with EWMA
- rollups retain `sample_size`, `ewma_alpha`, and `updated_at_ms`

## Runtime producers

Current producers that write into the trust layer:

- orchestrator task completion/failure writes `agent` + `task_completion` observations
- endpoint reliability writes `endpoint` observations for factuality/contradiction/infra dimensions
- Socrates surface telemetry writes `model` observations for factuality/contradiction/refusal dimensions

When persistence writes fail in task completion/failure paths, orchestrator now emits explicit degradation signals in shared context keys under:

- `orchestrator/persistence_health/trust/reliability_observation`
- `orchestrator/persistence_health/trust/observation`
- `orchestrator/persistence_health/lineage/task_completed`
- `orchestrator/persistence_health/lineage/task_failed`

Each key carries `status`, `degraded_count`, `last_error`, and `last_error_unix_ms` so operators can detect silent durability regressions.

The orchestrator also writes outbox lifecycle health to `orchestrator/persistence_outbox_lifecycle` with `queued`, `pruned_last_run`, `retried_last_run`, `replayed_last_run`, and `last_run_unix_ms`. Replay diagnostics now include `replay_failed_last_run` (count of replay attempts that failed in the latest tick) and `replay_failed_by_op` (map keyed by replay operation label, usually `replay.op`, with `unknown` fallback) so operators can identify stuck replay classes without inspecting raw queue payloads.

## Runtime consumers

Current consumers:

- routing uses scoped `agent` `task_completion` trust rollups as floor + weighted utility
- `vox db reliability-list --domain trust` shows trust rollups for operators
- MCP `vox_db_trust_rollups` lists scoped rollup rows; `vox_db_trust_summary` returns grouped aggregates (by dimension, domain, entity type, or combined keys); `vox_db_trust_drift` compares recent vs prior window means on raw observations; `vox_db_trust_propagate` runs domain-clique affinity smoothing over model rollups (optional persist to `*_propagated` dimensions)
- `vox_db_trust_drift` can now include forensic payloads when requested:
  - `include_raw_observations: true` returns raw `trust_observations` rows (optionally filtered by `task_id`/`since_ms`/`raw_limit`)
  - `include_lineage_for_task: true` with `task_id` and repository scope returns task lineage rows for trust/lineage correlation
- `vox ci mens-scorecard ingest-trust --summary <path>` ingests a validated `vox_mens_scorecard_summary_v1` `summary.json` into `trust_observations` / rollups for the workspace repository id
- `vox_scientia_worthiness_evaluate` with `with_live_trust: true` attaches `live_trust_rollups` summaries for the workspace repository when VoxDb is connected
- MCP `vox_orchestrator_status` now includes `persistence_outbox_lifecycle` so clients can read outbox replay health (`replayed_last_run`, `replay_failed_last_run`, `replay_failed_by_op`) without direct context-store access
- MCP also provides dedicated outbox inspection tools: `vox_orchestrator_persistence_outbox_lifecycle` (typed lifecycle snapshot) and `vox_orchestrator_persistence_outbox_queue` (queued lane entries with optional lane filter and replay redaction)

## Notes on score semantics

`trust_rollups.score` is normalized to `[0, 1]` and interpreted as “higher is better”.

- For inverse-risk metrics, writers invert before recording (`1 - risk`).
- `dimension` names can represent the source signal, but stored score remains normalized-goodness.

## Known gaps (next iterations)

- extend domain tagging and policy-profile attribution beyond primary MCP chat/plan/edit surfaces
- automated calibration transforms (e.g. isotonic) on top of drift reports—not only windowed mean comparison
- richer graph propagation than same-domain clique affinity (explicit trust edges, provider graphs)
- **per-validation-failure-class dimensions** (`schema_conformance`, `semantic_policy`, `repair_exhaustion`):
  proposed in [research-llm-output-mediation-validation-2026.md](research-llm-output-mediation-validation-2026.md)
  §8.4 as part of the unified LLM Mediation Layer (LML) design. Currently trust signals capture per-task
  outcomes but not per-inference-call validation failure modes.

