---
title: "ADR 017: Populi lease-based authoritative remote execution"
description: "Normative target model for single-owner lease semantics, A2A transport, and local fallback when promoting remote execution beyond best-effort relay."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-05-01"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 017: Populi lease-based authoritative remote execution

## Status

**Accepted (implemented).** The single-owner lease lifecycle (grant → renew → release / expiry → local fallback + cancel relay) is fully implemented and covered by 13 integration tests in `crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs`. Lease-gated remote execution is off by default behind `VOX_ORCHESTRATOR_MESH_REMOTE_LEASE_GATING_ENABLED`; see [remote execution rollout checklist](../operations/populi-remote-execution-rollout-checklist.md) for go/no-go gates and kill-switch table.

## Context

Populi provides membership, HTTP control plane operations, and A2A inbox semantics including **claimer leases** for mesh-delivered rows ([mens SSOT](../reference/populi.md)). The orchestrator emits [`RemoteTaskEnvelope`](../../../crates/vox-orchestrator/src/a2a/envelope.rs) traffic via A2A when experimental flags are set. With `VOX_ORCHESTRATOR_MESH_REMOTE_LEASE_GATING_ENABLED=1`, relay is **awaited** and successful grant places the task in **remote-hold** (single owner, no local dequeue); lease renew loss or expiry falls back to local enqueue and relays cancel.

The first-wave **personal-cluster** roadmap needs a clear upgrade path from relay-style fan-out to **authoritative** remote ownership so that:

- at most **one** worker owns execution of a given leased task class at a time,
- long-running GPU work can **renew** leases and handle **cancellation** predictably,
- **partition or expiry** yields a defined **local fallback** (or explicit failure) rather than silent double execution.

## Decision

1. **Authoritative remote execution v1** uses a **single-owner lease** recorded by the Populi control plane (or equivalent durable coordinator): exactly one remote worker holds the lease for a given **task / correlation id** until **release**, **expiry**, **revocation**, or **verified handoff** (if ever added later).
2. **Transport for handoff, renew, cancel, and result correlation** remains **A2A over the Populi HTTP control plane** unless a future ADR replaces [ADR 008](008-populi-transport.md) as the default control transport. Lease **state** may also be exposed via additive HTTP APIs as contracts evolve.
3. **No work-stealing in v1:** the scheduler does not preempt an active lease holder for another peer without an explicit future design.
4. **Local fallback** is **required** for the leased task class when lease acquisition fails, renewal fails, the worker is unhealthy, or the lease expires without completion—unless operator policy explicitly opts into fail-closed behavior for that profile (documented per deployment).
5. **Promotion trigger:** shipping behavior where **remote execution correctness** or **SLA** depends on Populi (not merely “extra logging” or “hinting”) is a **breaking adoption** of this ADR and must be accompanied by contract tests, rollout docs, and updates to [mens SSOT](../reference/populi.md) and [unified orchestration](../reference/orchestration-unified.md).

## Non-goals (this ADR)

- Default **WAN distributed training** or collective-heavy schedules.
- Hosted multi-tenant GPU **donation** networks ([ADR 009](009-populi-hosted-baas.md) remains the future-scope boundary).
- Merging `remote_mesh` durability semantics with `local_durable` queue ownership without a separate ADR.

## Consequences

- Experimental relay flags remain **best-effort** and **non-authoritative** until implementation aligns with this ADR.
- New **OpenAPI** fields and orchestrator **gating** are expected to be **additive** and **off by default** during rollout.
- Operators gain a stable vocabulary: **lease grant / renew / release / expiry**, **correlation id**, **single owner**, **fallback**.

## Related documentation

- [Work-type placement policy matrix](../reference/populi-work-type-placement-matrix.md) — where remote execution is allowed by trust boundary.
- [Populi overlay personal cluster runbook](../operations/populi-overlay-personal-cluster-runbook.md) — WAN and enrollment boundaries.
- [Remote execution rollout checklist](../operations/populi-remote-execution-rollout-checklist.md) — kill switches and go/no-go.
- [Populi GPU mesh implementation plan 2026](../archive/research-2026-q1/populi-gpu-mesh-implementation-plan-2026.md) — phased sequencing (roadmap; not edited by this ADR).


