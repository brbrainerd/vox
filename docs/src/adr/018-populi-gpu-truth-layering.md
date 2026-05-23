---
title: "ADR 018: Populi GPU truth layering"
description: "Normative layering between probe-backed hardware facts, allocatable capacity, and operator policy labels for Populi scheduling."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 018: Populi GPU truth layering

## Status

**Accepted (design intent).** Defines how **GPU-related** fields on nodes and workers should be interpreted once a **hardware-truth** layer ships. Until then, mens continues to rely primarily on **operator-set advertisement** flags (for example `VOX_MESH_ADVERTISE_GPU`) as documented in [mens SSOT](../reference/populi.md) and [unified orchestration](../reference/orchestration-unified.md).

## Context

Scheduling and routing need **trustworthy** signals: today, many GPU/NPU hints are **declared** by the operator or process environment, not **verified** as allocatable, healthy inventory. A GPU-mesh roadmap without a clear separation between **facts**, **capacity**, and **policy** invites silent mismatch (a node “advertises” CUDA while no device is usable).

## Decision

1. **Layer A — Verified hardware facts (probe-backed) {** driver-visible devices, stable device ids where available, health signals derived from probes (or trusted agents), and **observed** memory / compute attributes. This layer is **best-effort** per platform but is the **preferred source of truth** when present.
2. **Layer B — Allocatable capacity:** what the node **offers** to remote or local schedulers after reservations, MIG/partitioning, thermal throttling, or local workloads. May differ from raw Layer A totals.
3. **Layer C — Operator policy labels:** non-authoritative tags for affinity, pools, regions, compliance classes, and cost tiers. Schedulers **must not** treat these as hardware guarantees.
4. **Precedence:** for **correctness-critical** placement (for example authoritative lease acquisition for GPU tasks), **Layer A/B** outrank **Layer C** when in conflict. **Layer C** may **restrict** or **prefer** candidates but must not **invent** capacity.
5. **Additive contracts:** new optional `NodeRecord` (and related) fields should encode **which layer** populated them where ambiguity would otherwise confuse clients. Unknown fields remain ignorable per extension-first rules in [mens SSOT](../reference/populi.md).

## Consequences

- Documentation and OpenAPI evolve to distinguish **verified** vs **advertised** GPU fields without breaking existing clients.
- Routing and federation hints consume **health + capacity** from Layer A/B when available, falling back to legacy advertisement only when necessary.
- Telemetry should eventually attribute placement decisions to **which layer** supplied the decisive signal (see [placement observability](../reference/orchestration-unified.md#placement-and-lease-observability)).

## Related documentation

- [ADR 017: lease-based remote execution](017-populi-lease-remote-execution.md) — ownership model that should consume truthful capacity signals.
- [Work-type placement policy matrix](../reference/populi-work-type-placement-matrix.md).
- [Populi GPU truth probe specification (NVML Layer A)](../archive/research-2026-q1/populi-gpu-truth-probe-spec.md) — shipped probe wiring and build features.
- [Populi GPU network research 2026](../archive/research-2026-q1/populi-gpu-network-research-2026.md) — evidence and gaps (research).


