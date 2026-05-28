---
title: "Telemetry retention and sensitivity SSOT"
description: "Maps telemetry and telemetry-adjacent data to sensitivity classes, retention expectations, and prune policy alignment."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-02"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Telemetry retention and sensitivity SSOT

## Status

**Roadmap:** sensitivity classes below are normative for future implementation. Current TTLs are authoritative in [retention-policy.yaml](../../../contracts/db/retention-policy.yaml) and [`db_retention`](../../../crates/vox-cli/src/commands/db_retention.rs).

## Sensitivity classes

| Class | Definition | Examples |
| --- | --- | --- |
| **S0** | Coarse counters, version strings, bucketed timings | Aggregated benchmark names, build timing buckets |
| **S1** | Operational metadata without user content | `repository_id` labels, mesh event names, model ids |
| **S2** | Workspace-adjacent: can infer project shape | Relative paths in CI findings, repo-scoped session keys, cross-repo query metadata (see [telemetry-metric-contract](../reference/telemetry-metric-contract.md)) |
| **S3** | Content-bearing | Chat text, prompts, tool args (full), retrieval hits, transcripts |

**Rule:** centralized “usage telemetry” MUST stay at **S0–S1** unless explicitly classified as **S2** with user/org opt-in and documented re-identification risk.

## Retention alignment

### Today: `research_metrics`

[retention-policy.yaml](../../../contracts/db/retention-policy.yaml) lists `research_metrics` with **365 days** (`days` relative to `created_at`). Prune is operator-driven via `vox db prune-plan` / `prune-apply`.

### Today: `build_run*` telemetry tables

The `vox ci build-timings --deep` command persists structured build telemetry in `build_run` plus child tables
(`build_crate_sample`, `build_warning`, `build_run_dependency_shape`). Retention follows
[retention-policy.yaml](../../../contracts/db/retention-policy.yaml):

| Table | Prune rule | Notes |
| --- | --- | --- |
| `build_run` | `days` / **365** / `recorded_at` | Parent run cadence aligned with benchmark retention horizon. |
| `build_crate_sample`, `build_warning`, `build_run_dependency_shape` | *(via FK)* | `ON DELETE CASCADE` from `build_run`; no separate policy rows needed. |

### Today: `ci_completion_*`

Completion ingest persists workspace-adjacent rows ([`ci_completion.rs`](../../../crates/vox-db/src/schema/domains/ci_completion.rs)), classified **S2** (paths, fingerprints). [retention-policy.yaml](../../../contracts/db/retention-policy.yaml) defines:

| Table | Prune rule | Notes |
| --- | --- | --- |
| `ci_completion_run` | `days` / **365** / `finished_at` | Same default horizon as `research_metrics` for comparable org-local telemetry. |
| `ci_completion_finding`, `ci_completion_detector_snapshot` | *(via FK)* | `ON DELETE CASCADE` from `ci_completion_run`; no separate policy rows. |
| `ci_completion_suppression` | `expires_lt_now` / `expires_at` | TTL suppressions auto-prune when `expires_at` is set and past `datetime('now')`; `expires_at` NULL stays until manual change or a future policy decision. |

**Policy alignment:** there is no separate “manual vs automated” conflict for runs: automated `prune-apply` ages out old **runs** (and cascaded children) on the same **365-day** calendar basis as `research_metrics`. Suppressions without expiry remain operator-visible for governance until edited or a stricter rule is adopted.

### Other adjacent tables

Tables such as `conversation_messages`, `agent_events`, `behavior_events`, `llm_interactions` (see [`agents.rs` schema](../../../crates/vox-db/src/schema/domains/agents.rs)) are **content or behavior** stores. They MUST NOT be folded into “telemetry” naming without a separate data-class chapter in [telemetry-trust-ssot](telemetry-trust-ssot.md).

### Today: `agent_exec_history`

Execution time telemetry records for agentic budgeting ([exec_time_telemetry](../../../crates/vox-db/src/exec_time_telemetry.rs)). Classified **S1** (tool names, IDs, duration, costs). Retention is set to **90 days** in [retention-policy.yaml](../../../contracts/db/retention-policy.yaml) because budgeting models only need a recent trailing window to detect anomalies; stale execution timings become irrelevant quickly.

## Orchestrator and Populi sidecars

- **Memory / log retention** in orchestrator (for example local log retention knobs) is separate from SQL TTL; document any future alignment in this file.
- **Populi `privacy_class`** on envelopes ([`a2a/envelope.rs`](../../../crates/vox-orchestrator/src/a2a/envelope.rs)) MUST be referenced when classifying mesh-visible events.

## Controls linkage

- **Prune:** [contracts/db/retention-policy.yaml](../../../contracts/db/retention-policy.yaml)
- **Emergency / feature off:** env and flags documented per subsystem (mesh telemetry, Ludus, MCP cost events) — consolidated index in [env-vars](../reference/env-vars.md)

## Related

- [Telemetry taxonomy and contracts SSOT](telemetry-taxonomy-contracts-ssot.md)
- [Telemetry implementation backlog 2026](telemetry-implementation-backlog-2026.md)


