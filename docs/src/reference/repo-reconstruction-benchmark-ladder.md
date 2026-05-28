---
title: "Repo reconstruction benchmark ladder"
description: "Progressive benchmark tiers, KPI examples, and pointers to the reconstruction JSON Schema and durable DB tables."
category: "Language Reference"

schema_type: "TechArticle"
---

# Repo reconstruction benchmark ladder

Progressive evaluation tiers for retrieval-first, multi-shard repository reconstruction campaigns. Machine contracts live under `contracts/orchestration/repo-reconstruction.schema.json` and are listed in `contracts/index.yaml`.

## Tiers

| Tier | Focus | Primary KPIs (examples) |
|------|--------|-------------------------|
| `issue_repair` | Single defect or small patch set | Patch applies cleanly; targeted tests pass; no regression on stated paths |
| `subsystem_regen` | One bounded module or feature slice | Build + scoped test suite; docs facts consistent with code |
| `crate_regen` | Full crate boundary | `cargo check`/equivalent; integration tests for public API |
| `repo_regen` | Whole repository | Full CI ladder; cross-crate invariants; verification evidence stored |

## Gating

- Advance tiers only when the prior tier’s KPIs meet rollout thresholds for your environment (latency, cost, and trust boundaries are deployment-specific).
- Prefer **retrieval-grounded** artifacts (shard briefs, symbol graph, verification evidence) over monolithic prompts; see [`mens-training-data-contract.md`](mens-training-data-contract.md) for opt-in training lanes.
- Remote execution should carry **lease** and **campaign** correlation on mesh envelopes where supported; see [`orchestration-unified.md`](orchestration-unified.md) and ADR 017 (Populi lease / remote execution).

## Persistence

Campaign specs, artifact rows, and benchmark KPI snapshots are stored in the orchestrator DB when available (`reconstruction_campaign_spec`, `reconstruction_artifacts`, `reconstruction_benchmark_kpis` in the execution domain schema).
