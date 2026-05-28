---
title: "Populi GPU mesh implementation plan 2026"
description: "Roadmap for evolving Populi into a user-owned GPU mesh with phased rollout, ADR boundaries, and a first execution ownership model."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-03-29"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Populi GPU mesh implementation plan 2026

**Status:** Roadmap only. This page describes intended sequencing and design choices for future implementation work. It does **not** change shipped behavior.

Primary research input: [Populi GPU network research 2026](populi-gpu-network-research-2026.md).

## Goal

Provide a concrete implementation roadmap for turning Populi from a CPU-first control plane into a user-owned GPU mesh that can:

- discover GPU capacity with more trustworthy data,
- place a narrow class of remote work safely,
- fall back to local execution cleanly,
- support users adding and removing GPU nodes with minimal operational friction,
- prepare for later scheduler unification across agent tasks, inference, and training.

## Scope and guardrails

This roadmap assumes the following constraints:

- It is a **first-wave personal-cluster roadmap**, not a hosted public GPU marketplace.
- Hosted "donate your GPU to the cloud" behavior remains out of scope for this wave. See [ADR 009: Hosted mens / BaaS (future scope)](../adr/009-populi-hosted-baas.md).
- WAN-distributed training is **not** assumed by default, even if internet-connected personal clusters become supported for control and remote execution.
- [ADR 008: Mens transport](../adr/008-populi-transport.md) remains the control-plane baseline: Populi stays HTTP-first unless a later replacement ADR explicitly changes that.
- Cloud GPU dispatch and Populi mesh remain separate surfaces until a later convergence decision says otherwise.

### Shipped slices aligned with this roadmap (checkpoint)

The checklist below remains the source of truth for **full** phase completion; these items are **already partially landed** in tree:

- **Phase 2 (GPU truth):** optional NVML probe path (`vox-repository` feature `nvml-probe`, `vox-populi` `nvml-gpu-probe`, `vox-cli` `mesh-nvml-probe`) populates `NodeRecord` `gpu_*` fields when the driver is present — [probe spec](populi-gpu-truth-probe-spec.md).
- **Phase 4 (execution plane):** exec lease grant/renew/release + persistence; lease-gated submit holds `task:{task_id}`; sample remote worker does **not** acquire a second lease when `exec_lease_id` is set; legacy worker lease uses `task:{task_id}`; `remote_task_result` drain walks **cursor-paged** mesh inbox reads.
- **Scaling posture:** [ADR 020: default transport](../adr/020-populi-mesh-scaling-transport-default.md) (HTTP-first; gossip/QUIC optional later).
- **Phase 3 (lifecycle):** design SSOT for drain/hotplug — [node lifecycle doc](populi-node-lifecycle-hotplug.md); operator **`vox populi admin maintenance`** (optional **`--until-unix-ms` / `--for-minutes`** for timed auto-clear), **`quarantine`**, **`exec-lease-revoke`** (feature `populi`); federation routing hints use effective maintenance (deadline-aware) + **`heartbeat_stale`** from orchestrator **`stale_threshold_ms`** (MCP poller); **`GET /v1/populi/exec/leases`** plus optional MCP reconcile (**`VOX_ORCHESTRATOR_MESH_EXEC_LEASE_RECONCILE`**) and opt-in auto-revoke (**`VOX_ORCHESTRATOR_MESH_EXEC_LEASE_AUTO_REVOKE`**) with tracing, Codex telemetry, and **`vox-mcp`** integration coverage (`tests/populi_mcp_http_join_startup.rs`). Placement rebalance / gang scheduling remains backlog.

## Recommended first execution model

The first authoritative remote execution model should be **single-owner lease-based remote worker ownership**.

That means:

- the Populi control plane records which remote worker currently owns execution,
- remote work is granted by a **lease** with renewal and expiry semantics,
- A2A remains the transport for handoff, renew, cancel, and result messages,
- local fallback remains available when lease acquisition fails, the worker becomes unhealthy, or the lease expires without completion.

### Why this model fits the current codebase

- Populi already has a control plane, explicit membership, and A2A inbox lease concepts in [docs/src/reference/populi.md](../reference/populi.md).
- The orchestrator already has a best-effort remote envelope path in [crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs](../../../crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs), but that path is not yet authoritative.
- A lease-based model upgrades current relay behavior into a real ownership contract without immediately requiring work-stealing or full distributed training.
- It is a better fit than work-stealing for the current architecture because the repo today centers on local queues plus HTTP discovery and A2A, not a shared multi-node queue runtime.

### Why not start with the alternatives

- **Side-relay mirror:** already approximates today's experimental behavior and does not solve double execution or ownership.
- **One-shot authoritative handoff without leases:** too weak for long-running GPU jobs that need renew, cancel, and worker-loss semantics.
- **Work-stealing first:** assumes a stronger distributed queue model than the current system provides and would add unnecessary complexity before ownership semantics are stable.

## Roadmap overview

```mermaid
flowchart LR
    phase1[Phase1Foundations] --> phase2[Phase2GpuTruth]
    phase2 --> phase3[Phase3NodeLifecycle]
    phase3 --> phase4[Phase4ExecutionPlaneV1]
    phase4 --> phase5[Phase5SchedulerUnification]
    phase5 --> phase6[Phase6InternetClusters]
```

## Phase 1: Foundations and ADR closure

### Phase 1 objective

Resolve the decisions that the research doc explicitly called out as prerequisites:

- GPU truth semantics,
- remote ownership and cancellation semantics,
- fallback behavior,
- work-type scope for local, LAN, and WAN execution,
- ADR boundaries versus additive contract work.

### Phase 1 deliverables

- One or more new ADRs for authoritative remote execution and possibly GPU truth.
- A short decision matrix describing which work types are allowed on:
  - local only,
  - trusted LAN personal clusters,
  - internet-connected overlay clusters.
- Reference-doc updates that define the future ownership vocabulary without claiming it is already shipped.

### Phase 1 rationale

Without these decisions, later phases risk building incompatible health, scheduling, and fallback behavior.

## Phase 2: GPU hardware-truth layer

### Phase 2 objective

Add a more trustworthy GPU inventory model to Populi so scheduling is based on something stronger than operator-set advertisement flags.

### Phase 2 primary outcomes

- Verified GPU inventory and allocatable capacity on node records.
- Health state per device or per worker where practical.
- Optional topology metadata for multi-GPU hosts.
- A layered model that combines verified hardware state with operator policy labels.

### Phase 2 expected touchpoints

- [crates/vox-populi/src/lib.rs](../../../crates/vox-populi/src/lib.rs)
- [contracts/populi/control-plane.openapi.yaml](../../../contracts/populi/control-plane.openapi.yaml)
- [docs/src/reference/populi.md](../reference/populi.md)
- [docs/src/reference/orchestration-unified.md](../reference/orchestration-unified.md)
- [contracts/communication/protocol-catalog.yaml](../../../contracts/communication/protocol-catalog.yaml)

### Phase 2 notes

This phase should stay additive where possible: new optional fields and new health metadata are preferable to disruptive changes.

## Phase 3: Node churn and admission lifecycle

### Phase 3 objective

Make it safe to add or remove GPU nodes without orphaning or corrupting work.

### Phase 3 primary outcomes

- Drain and no-new-work admission states.
- Clear retire or quarantine semantics for workers that should not receive new assignments.
- Scheduler reactions to stale, partitioned, or partially healthy nodes.
- Explicit behavior when a worker leaves voluntarily versus disappears unexpectedly.

### Phase 3 expected touchpoints

- [docs/src/reference/populi.md](../reference/populi.md)
- [contracts/populi/control-plane.openapi.yaml](../../../contracts/populi/control-plane.openapi.yaml)
- [crates/vox-orchestrator/src/services/routing.rs](../../../crates/vox-orchestrator/src/services/routing.rs)

### Phase 3 notes

This phase is the operational prerequisite for making a larger GPU mesh feel smooth rather than fragile.

## Phase 4: Execution plane v1

### Phase 4 objective

Introduce the first narrow, opt-in form of authoritative remote execution using the lease-based ownership model.

### Phase 4 first supported scope

Keep the scope intentionally narrow:

- one class of GPU-capable tasks,
- explicit feature flag or policy gating,
- single-owner lease,
- no work-stealing,
- no claim of WAN-friendly distributed training.

### Phase 4 primary outcomes

- Lease grant, renew, release, and expiry semantics on the control plane.
- Result correlation and remote cancellation rules.
- Defined local fallback when the remote worker cannot acquire or maintain the lease.
- Transition from best-effort remote envelope delivery to a real ownership path.

### Phase 4 expected touchpoints

- [crates/vox-orchestrator/src/a2a/envelope.rs](../../../crates/vox-orchestrator/src/a2a/envelope.rs)
- [crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs](../../../crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs)
- [contracts/populi/control-plane.openapi.yaml](../../../contracts/populi/control-plane.openapi.yaml)
- [docs/src/reference/populi.md](../reference/populi.md)
- [docs/src/reference/orchestration-unified.md](../reference/orchestration-unified.md)

### Phase 4 notes

This is the phase where Populi first becomes more than visibility and best-effort relay, but only within a deliberately narrow contract.

## Phase 5: Scheduler unification

### Phase 5 objective

Define a single placement policy that can reason across local execution, Populi remote execution, and cloud dispatch without pretending those surfaces are already equivalent.

### Phase 5 primary outcomes

- A documented placement matrix across:
  - agent tasks,
  - inference-style work,
  - MENS training,
  - local-only, LAN, and overlay-connected remote placements.
- A clearer separation between capability truth, operator policy labels, and trust or locality policy.
- A path toward one scheduler surface while preserving the distinction between current supported behavior and future options.

### Phase 5 expected touchpoints

- [crates/vox-orchestrator/src/services/routing.rs](../../../crates/vox-orchestrator/src/services/routing.rs)
- [docs/src/reference/orchestration-unified.md](../reference/orchestration-unified.md)
- [docs/src/reference/mens-cloud-gpu.md](../reference/mens-cloud-gpu.md)

### Phase 5 notes

This phase should happen **after** execution ownership exists, otherwise the scheduler would over-promise remote guarantees it cannot enforce.

## Phase 6: Internet-distributed personal clusters

### Phase 6 objective

Support secure overlay-connected personal clusters as the first internet-distributed Populi mode.

### Phase 6 primary outcomes

- Documented security posture for user-owned internet clusters.
- Overlay-friendly runbooks and enrollment guidance.
- Separation of control-plane reachability from heavy data or artifact movement.
- Explicit statement of what does and does not work well over consumer-grade WAN links.

### Phase 6 expected touchpoints

- [docs/src/architecture/protocol-convergence-research-2026.md](protocol-convergence-research-2026.md)
- [docs/src/reference/populi.md](../reference/populi.md)
- deployment and operator runbook pages such as [docs/src/reference/deployment-compose.md](../reference/deployment-compose.md)

### Phase 6 notes

This phase is about safe personal clusters over overlays first, not a public donation network and not default WAN distributed training.

## ADR trigger matrix

### Changes that should get an ADR

- Replacing HTTP as the default in-tree Populi control transport.
- Adding a second default in-tree Populi transport beside HTTP.
- Promoting remote execution from experimental or best-effort to authoritative supported behavior.
- Promoting distributed training from explicit non-goal to supported product path.
- Merging `remote_mesh` durability semantics with `local_durable` queue ownership.
- Changing the default trust or enrollment model, such as ambient discovery or automatic remote enrollment.
- Shipping hosted or multi-tenant Populi behavior beyond today’s documentation-only scope.

### Changes that can remain additive contracts and docs

- New optional `NodeRecord` fields.
- New additive HTTP routes or parameters on the current Populi control plane.
- New rollout tokens, telemetry fields, or capability metadata.
- Research, roadmap, and explanatory architecture documents.

## Contract and code touchpoints

The roadmap depends most directly on these surfaces:

- [contracts/populi/control-plane.openapi.yaml](../../../contracts/populi/control-plane.openapi.yaml)
- [contracts/communication/protocol-catalog.yaml](../../../contracts/communication/protocol-catalog.yaml)
- [docs/src/reference/populi.md](../reference/populi.md)
- [docs/src/reference/orchestration-unified.md](../reference/orchestration-unified.md)
- [docs/src/reference/mens-cloud-gpu.md](../reference/mens-cloud-gpu.md)
- [crates/vox-populi/src/lib.rs](../../../crates/vox-populi/src/lib.rs)
- [crates/vox-orchestrator/src/a2a/envelope.rs](../../../crates/vox-orchestrator/src/a2a/envelope.rs)
- [crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs](../../../crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs)
- [crates/vox-orchestrator/src/services/routing.rs](../../../crates/vox-orchestrator/src/services/routing.rs)

## Recommended first implementation slice

The first implementation slice after this roadmap should be:

1. Define the authoritative lease model in docs and ADR form.
2. Extend Populi contracts with additive worker health and GPU capacity fields.
3. Add drain and no-new-work lifecycle states.
4. Implement opt-in lease-based authoritative remote execution for one narrow class of GPU-capable task.

That sequence keeps local-first behavior as the safe default while making real progress toward a usable GPU mesh.

## Granular implementation backlog

The checklist below is the implementation-ready task list keyed to the current plan todos.

### Phase 1 task checklist

- **`p1-adr-ownership`**  
  - Draft ADR for lease-based authoritative remote execution and fallback semantics.  
  - Target files: `docs/src/adr/` (new ADR), `docs/src/reference/populi.md`, `docs/src/reference/orchestration-unified.md`.  
  - Acceptance: ADR approved; docs explicitly distinguish current experimental relay from authoritative lease execution.

- **`p1-adr-gpu-truth`**  
  - Define GPU truth layering (probe-backed facts vs operator policy labels).  
  - Target files: `docs/src/adr/` (new ADR or ADR addendum), `docs/src/reference/populi.md`, `docs/src/reference/orchestration-unified.md`.  
  - Acceptance: normative definition of verified vs advertised fields and scheduler trust rules.

- **`p1-policy-matrix`**  
  - Publish work-type policy matrix across local, trusted LAN, and overlay-WAN scopes.  
  - Target files: this roadmap page plus `docs/src/reference/populi.md` cross-link.  
  - Acceptance: matrix states allowed/blocked/gated work types and references ADR constraints.

### Phase 2 task checklist

- **`p2-contract-node-fields`**  
  - Add optional `NodeRecord` + OpenAPI fields for GPU capacity/health and compatibility parsing tests.  
  - Target files: `crates/vox-populi/src/lib.rs`, `contracts/populi/control-plane.openapi.yaml`, `crates/vox-populi/tests/*`.  
  - Acceptance: backward-compatible optional fields; tests prove old/new payload interoperability.

- **`p2-federation-hints`**  
  - Extend federation hint mapping to carry lifecycle/health truth used by routing.  
  - Target files: `crates/vox-orchestrator/src/populi_federation.rs`, `crates/vox-orchestrator/src/mcp_tools/server/lifecycle.rs`, `crates/vox-orchestrator/src/services/routing.rs`.  
  - Acceptance: unsuitable nodes are no longer treated as healthy candidates in hint-driven routing.

### Phase 3 task checklist

- **`p3-lifecycle-controls`**  
  - Implement drain/no-new-work lifecycle controls and server enforcement points.  
  - Target files: `contracts/populi/control-plane.openapi.yaml`, `crates/vox-populi/src/transport/handlers.rs`, `crates/vox-populi/src/transport/router.rs`, `crates/vox-populi/src/node_registry.rs`.  
  - Acceptance: operators can set lifecycle states; API and docs define transitions and constraints.

- **`p3-routing-eligibility`**  
  - Apply lifecycle state filters in routing eligibility and snapshot consumption.  
  - Target files: `crates/vox-orchestrator/src/services/routing.rs`, `crates/vox-orchestrator/src/populi_federation.rs`, `docs/src/reference/orchestration-unified.md`.  
  - Acceptance: drained/no-new-work/quarantined nodes are excluded or explicitly penalized per policy.

**Checkpoint:** the acceptance intent of **`p3-lifecycle-controls`** and **`p3-routing-eligibility`** is met in tree for the current HTTP control plane (admin maintenance/quarantine/exec-lease APIs; `RemotePopuliRoutingHint` filters **`maintenance`** / **`quarantined`** / **`heartbeat_stale`** in `routing.rs`; MCP federation poll + optional exec-lease reconcile/auto-revoke). **Queued-work replanning on capacity drops** is not automatic today — see **`p5-queued-capacity-rebalance`**.

### Phase 4 task checklist

- **`p4-lease-api`**  
  - Implement lease grant/renew/release APIs and lease correlation IDs for remote execution.  
  - Target files: `contracts/populi/control-plane.openapi.yaml`, `crates/vox-populi/src/transport/*`, `crates/vox-orchestrator/src/a2a/envelope.rs`.  
  - Acceptance: lease lifecycle has contract-level schemas, server behavior, and request/response tests.

- **`p4-submit-path-gating`**  
  - Gate submission to prevent dual local+remote ownership for leased task class.  
  - Target files: `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs`, config files under `crates/vox-orchestrator/src/config/`.  
  - Acceptance: leased task class cannot execute concurrently on both local and remote owners.

- **`p4-fallback-and-cancel`**  
  - Implement explicit fallback and cancel behavior on lease loss/renew failure.  
  - Target files: `crates/vox-orchestrator/src/a2a/dispatch.rs`, `crates/vox-orchestrator/src/a2a/envelope.rs`, `docs/src/reference/populi.md`.  
  - Acceptance: deterministic local fallback path and cancel semantics are documented and tested.

- **`p4-core-result-handling`**  
  - Ensure remote result handling is not tied to a single embedder lifecycle path.  
  - Target files: `crates/vox-orchestrator/src/a2a/dispatch.rs`, `crates/vox-orchestrator/src/mcp_tools/server/lifecycle.rs`, orchestrator runtime integration points.  
  - Acceptance: authoritative remote result processing works for all supported embedders, not MCP-only startup loops.

- **`p4-single-owner-tests`**  
  - Add integration tests proving single-owner execution and deterministic fallback for leased tasks.  
  - Target files: `crates/vox-orchestrator/tests/*`, `crates/vox-populi/tests/*`, any cross-crate integration harness.  
  - Acceptance: tests cover lease success, lease expiry, renewal failure, duplicate delivery, and flag-off regression behavior.

### Phase 5 task checklist

- **`p5-placement-policy`**  
  - Implement unified placement policy module preserving local vs lease-exec vs cloud semantic differences.  
  - Target files: `crates/vox-orchestrator/src/services/routing.rs`, supporting policy module(s), `docs/src/reference/mens-cloud-gpu.md`.  
  - Acceptance: placement matrix is codified; routing reason codes identify selected execution surface.

- **`p5-config-and-observability`**  
  - Add config toggles, decision reason codes, and trace fields for placement/lease transitions.  
  - Target files: `crates/vox-orchestrator/src/config/*`, `docs/src/reference/env-vars.md`, `docs/src/reference/orchestration-unified.md`, telemetry hooks as needed.  
  - Acceptance: feature gates are documented; traces/structured logs include `task_id`, `lease_id`, and placement reason.

- **`p5-queued-capacity-rebalance`**  
  - When federation hints or node records show reduced **allocatable** GPU capacity or newly ineligible nodes, re-evaluate **queued** (not yet running) work so new placement picks healthy targets; no silent migration of in-flight remote tasks in v1.  
  - Target files: `crates/vox-orchestrator/src/services/routing.rs`, `crates/vox-orchestrator/src/orchestrator/agent_lifecycle.rs` (`set_remote_populi_routing_hints`), scheduler / queue integration, `docs/src/architecture/populi-node-lifecycle-hotplug.md` (align with “new placement only” rule).  
  - Acceptance: policy-driven or config-gated hook runs on snapshot updates; reason codes show preemption of stale routing hints for queued tasks; tests use synthetic hint drops. **Partial (landed):** trace `populi_remote_schedulable_decreased`; optional **`VOX_ORCHESTRATOR_MESH_REBALANCE_ON_REMOTE_SCHEDULABLE_DROP`** runs one load **[`rebalance`](../../../crates/vox-orchestrator/src/orchestrator/scaling.rs)** after a schedulable-count drop (work-steering only). Full per-task route replay remains future work.

- **`p5-gang-nccl-pilot`**  
  - Optional **pilot** for topology-aware **gang** scheduling and collective-friendly placement (NCCL assumptions), strictly bounded by [work-type placement matrix](../reference/populi-work-type-placement-matrix.md) **Distributed collectives** rows (LAN pilot first; WAN remains out of scope by default until ADR).  
  - Target files: new or extended ADR, `contracts/populi/control-plane.openapi.yaml` (additive topology hints if needed), `crates/vox-orchestrator/src/services/routing.rs`, matrix + rollout checklist.  
  - Acceptance: pilot behind explicit flags; documented topology prerequisites; no default WAN collective path.

### Phase 6 task checklist

- **`p6-overlay-runbooks`**  
  - Publish secure overlay personal-cluster runbook and WAN expectation boundaries.  
  - Target files: `docs/src/reference/deployment-compose.md`, `docs/src/reference/populi.md`, `docs/src/architecture/protocol-convergence-research-2026.md`.  
  - Acceptance: operator steps cover enrollment, security posture, and supported/non-supported WAN usage.

- **`p6-rollout-gates`**  
  - Define rollout checklist and kill-switch validation before enabling beyond pilot environments.  
  - Target files: this roadmap page, `docs/src/reference/populi.md`, CI/runbook docs.  
  - Acceptance: go/no-go criteria include default-off validation, rollback switch validation, and regression checks.

## Work-type policy matrix (Phase 1 output target)

| Work class | Local single-node | Trusted LAN personal cluster | Overlay-WAN personal cluster |
| --- | --- | --- | --- |
| Agent task (non-GPU critical) | Allowed (default) | Allowed (gated) | Allowed (gated, conservative timeout) |
| GPU inference task | Allowed | Allowed (lease-gated) | Allowed (lease-gated, latency caveats) |
| GPU training long-run | Allowed | Allowed (explicit profile and checkpointing) | Not default; pilot-only explicit opt-in |
| Distributed collectives | Optional local/LAN only | Pilot-only with strict topology constraints | Out of scope by default |

Policy notes:

- Hosted donation network remains out of scope in this wave.
- Cloud provider dispatch remains a separate execution surface until explicit convergence work lands.
- Any change that promotes WAN distributed training into default supported behavior requires ADR approval.

## Relationship to other docs

- [Populi GPU network research 2026](populi-gpu-network-research-2026.md) is the evidence-gathering and gap-analysis source.
- [Protocol convergence research 2026](protocol-convergence-research-2026.md) remains the broader transport and delivery-plane synthesis.
- [Populi SSOT](../reference/populi.md) remains the source of truth for currently shipped behavior.

This roadmap exists so later implementation work can proceed in ordered phases without confusing research with current capability.


