---
title: "Populi node lifecycle, drain, and GPU hotplug"
description: "Design for maintenance, quarantine, stale nodes, and capacity changes without a second control plane."
category: "Architecture SSOTs"
last_updated: "2026-03-29"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Populi node lifecycle, drain, and GPU hotplug

This document captures the **lifecycle model** implied by today’s control plane and the **gaps** for automatic add/remove of GPUs and workers. It aligns with [ADR 017](../adr/017-populi-lease-remote-execution.md) (execution ownership) and [ADR 018](../adr/018-populi-gpu-truth-layering.md) (GPU truth).

## Current building blocks (shipped)

| Mechanism | Role |
| --- | --- |
| `NodeRecord.maintenance` | Operator hint: drain-oriented “no new work” on the node record (interpreted by policy / gates). |
| `NodeRecord.quarantined` | Server-side gate: rejects new A2A **claims** for that worker when set via admin API. |
| `join` / `heartbeat` / `leave` | Membership freshness; heartbeat merges JSON fields into the registry. |
| Exec lease **grant** / **renew** | `require_claimer_worker_gate`: unknown node, `quarantined`, or `maintenance` → **403** (no new leases / no renew while draining). |
| Exec lease **release** | Holder must match lease row and node must still be **registered**; **release is allowed under maintenance/quarantine** so holders can clear `scope_key` during drain (see `crates/vox-populi/src/transport/handlers.rs`). |
| A2A inbox **claim** | Same maintenance/quarantine gates as experimental routing expects. |
| Stale filters | Client-side `filter_registry_by_max_stale_ms` on **list** responses; server-side prune knobs exist for operational tuning. |

## Target behavior (personal cluster / lab)

1. **Voluntary subtract (GPU or node)**  
   - Operator sets `maintenance=true` on the node (or uses a future CLI) **before** retire.  
   - In-flight tasks { **exec lease renew** stops once maintenance is set (403); holder should **release** to free the scope or let the lease expire. **No new** exec grants for that node while maintenance is on.  
   - `leave` or stopped heartbeat removes the node from the fresh view after stale threshold.

2. **Involuntary subtract (crash, cable pull)**  
   - Heartbeat stops → node becomes stale in listings.  
   - Orchestrator: lease renewal fails → **local fallback** and cancel relay (existing poller path).  
   - Documented race: remote worker may still run briefly after partition — acceptable for experimental tier; fail-closed profiles need ADR 017 promotion.

3. **GPU hot-add / hot-remove**  
   - With [NVML probe](populi-gpu-truth-probe-spec.md) enabled, rebuilding `NodeRecord` on heartbeat refreshes `gpu_*_count` and VRAM hints.  
   - Schedulers must treat a **drop** in `gpu_allocatable_count` or healthy count as a **signal** to stop routing new GPU tasks to that node (future unified scheduler).  
   - No automatic “rebalance running tasks” in v1 — only **new** placement picks up new capacity.

4. **Drain vs quarantine**  
   - **Maintenance**: cooperative drain; still visible; good-faith workers finish or cancel.  
   - **Quarantine**: hard stop for **claim** paths; use when a node is untrusted or broken.

## Gaps (explicit backlog)

- **CLI:** Operator **`vox populi admin maintenance|quarantine|exec-lease-revoke`** is shipped (feature **`populi`**; `--control-url` / mesh control env; bearer via **`PopuliHttpClient::with_env_token()`** / Clavis mesh secrets). **Timed drain** uses optional **`--until-unix-ms`** / **`--for-minutes`** (maps to `maintenance_until_unix_ms` / `maintenance_for_ms` on `POST /v1/populi/admin/maintenance`). **Policy- or placement-driven** unattended lease cleanup (rebalance, gang jobs) remains future work; operators can **`exec-lease-revoke`** by id, or use MCP opt-in below.
- Optional **MCP reconciliation** (`VOX_ORCHESTRATOR_MESH_EXEC_LEASE_RECONCILE`): after each node poll, **`GET /v1/populi/exec/leases`** + holder vs registry check; traces + optional Codex `mesh_exec_lease_reconcile`. Opt-in **`VOX_ORCHESTRATOR_MESH_EXEC_LEASE_AUTO_REVOKE`** calls admin **exec-lease revoke** on each bad-holder row (aggressive; mesh/admin bearer). Covered by **`vox-mcp`** tests `populi_mcp_http_join_startup` (auto-revoke + reconcile-only negative case).
- Topology-aware **gang** scheduling and NCCL-style jobs (out of scope for default WAN row in the [placement matrix](../reference/populi-work-type-placement-matrix.md)); granular tasks **`p5-gang-nccl-pilot`** / **`p5-queued-capacity-rebalance`** / **`p5-placement-policy`** in [GPU mesh implementation plan 2026](populi-gpu-mesh-implementation-plan-2026.md).

## Related

- [Populi overlay personal cluster runbook](../operations/populi-overlay-personal-cluster-runbook.md)  
- [Remote execution rollout checklist](../operations/populi-remote-execution-rollout-checklist.md)  
- [GPU mesh implementation plan 2026](populi-gpu-mesh-implementation-plan-2026.md)


