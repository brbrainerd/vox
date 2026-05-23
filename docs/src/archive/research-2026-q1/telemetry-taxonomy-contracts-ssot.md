---
title: "Telemetry taxonomy and contracts SSOT"
description: "Planned unified event taxonomy, metric_type families, JSON Schema contracts, and transmission classes for Vox telemetry."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-02"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Telemetry taxonomy and contracts SSOT

## Status

This document is **roadmap**: it defines the target taxonomy and contract layering for a unified telemetry system. Shipped behavior today remains authoritative in code and [telemetry-metric-contract](../reference/telemetry-metric-contract.md).

## Goals

- One vocabulary for **event families**, **sensitivity**, **retention class**, and **transmission** across CLI, MCP, orchestrator, Populi, CI, and clients.
- No duplicate schema primaries: extend [contracts/index.yaml](../../../contracts/index.yaml) rather than ad-hoc JSON in random folders.
- Keep **content-bearing** payloads out of the usage-telemetry namespace (see [telemetry-trust-ssot](telemetry-trust-ssot.md)).

## Event family model (target)

Each logical event SHALL declare:

| Field | Description |
| --- | --- |
| `family` | Stable grouping: `benchmark`, `syntax_k`, `mcp_surface`, `mesh_control`, `questioning`, `workflow_journal`, `completion_ci`, `context_lifecycle_trace`, `mens_training_jsonl`, … |
| `metric_type` | Value written to `research_metrics.metric_type` where applicable, or parallel column in domain tables |
| `session_id_convention` | Prefix per [telemetry-metric-contract](../reference/telemetry-metric-contract.md) |
| `schema_ref` | URI or repo path to JSON Schema (or SQL comment + generated schema) |
| `sensitivity_class` | `S0` coarse / `S1` operational / `S2` workspace-adjacent / `S3` content-bearing |
| `transmission_class` | `local_only` \| `explicit_operator_export` \| `approved_usage_upload` (future) |
| `owner_crate` | Primary Rust owner for writes |

## Shipped `metric_type` constants (today)

From [`research_metrics_contract.rs`](../../../crates/vox-db/src/research_metrics_contract.rs) (`METRIC_TYPE_*`). CI (`vox ci data-ssot-guards`) requires each literal to appear in this page or in [telemetry-metric-contract](../reference/telemetry-metric-contract.md).

| `metric_type` | Typical `session_id` | Primary owner crate(s) |
| --- | --- | --- |
| `benchmark_event` | `bench:<repository_id>` | `vox-cli` → `vox-db` |
| `syntax_k_event` | `syntaxk:<repository_id>` | `vox-cli` → `vox-db` |
| `socrates_surface` | `mcp:<repository_id>` | `vox-mcp`, `vox-db` |
| `workflow_journal_entry` | `workflow:<repository_id>` | `vox-workflow-runtime`, `vox-db` |
| `populi_control_event` | `mens:<repository_id>` | `vox-cli`, `vox-mcp`, `vox-db` |
| `questioning_event` | (linked session keys) | `vox-mcp`, `vox-db` |
| `memory_hybrid_fusion` | `socrates:retrieval` | `vox-search`, `vox-ludus`, `vox-db` |
| `agent_exec_time` | (no prefix, agent_exec_history) | `vox-db` |

## Contract inventory (machine)

| Area | Contract path | Notes |
| --- | --- | --- |
| Completion CI | `contracts/telemetry/completion-*.v1.schema.json` | Ingest → `ci_completion_*` |
| Context lifecycle tracing | `contracts/orchestration/context-lifecycle-telemetry.schema.json` | Tracing fields, not necessarily DB rows |
| Syntax-K payload | `contracts/eval/syntax-k-event.schema.json` | `metadata_json` for `syntax_k_event` rows (`metric_type` above) |
| Interruption / attention | `contracts/communication/interruption-decision.schema.json` | Attention / interruption plane; normalized decision envelope |
| *(planned)* Usage telemetry | `contracts/telemetry/usage-event-*.schema.json` | **Not shipped yet** — add files + `contracts/index.yaml` rows before wiring producers; see [implementation blueprint](telemetry-implementation-blueprint-2026.md). |

## Target: single telemetry contract registry row pattern

Future work SHOULD register each family in [contracts/index.yaml](../../../contracts/index.yaml) with:

- `description`
- `enforced_by` including at least one of: `vox ci command-compliance`, `vox ci data-ssot-guards`, crate tests

## Transmission classes (normative definitions)

- **`local_only`:** never leaves the machine unless the user performs an explicit export (file copy, support bundle). Includes default structured tracing and local DB rows.
- **`explicit_operator_export`:** gated by CLI/MCP action and documented in [telemetry-client-disclosure-ssot](telemetry-client-disclosure-ssot.md).
- **`approved_usage_upload`:** reserved for a future central sink; requires separate policy doc, Clavis-backed credentials per [AGENTS.md](../../../AGENTS.md), and CHANGELOG entry per release.

## Forbidden in usage-telemetry schemas

The following MUST NOT appear in `approved_usage_upload` or default `local_only` usage events without `S3` classification and a separate consent path:

- raw source text, prompts, completions
- full MCP tool `arguments_json` (use hash/omit patterns from [`mcp_privacy.rs`](../../../crates/vox-ludus/src/mcp_privacy.rs))
- absolute paths, repository remotes, user home segments in stack traces
- retrieval query text and document bodies

## Related

- [Telemetry retention and sensitivity SSOT](telemetry-retention-sensitivity-ssot.md)
- [Telemetry implementation backlog 2026](telemetry-implementation-backlog-2026.md) — contract tasks


