---
title: "Populi work-type placement policy matrix"
description: "Canonical matrix of allowed, gated, and out-of-scope work classes across local, trusted LAN, and overlay-WAN personal clusters."
category: "Language Reference"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# Populi work-type placement policy matrix

This page is the **canonical** policy matrix for **first-wave personal-cluster** placement boundaries. It expresses **intent** aligned with [ADR 017](../adr/017-populi-lease-remote-execution.md), [ADR 018](../adr/018-populi-gpu-truth-layering.md), and [ADR 009](../adr/009-populi-hosted-baas.md). **Shipped** behavior may lag this matrix until roadmap phases complete; for current wire semantics use [mens SSOT](populi.md) and [unified orchestration](orchestration-unified.md).

## Matrix

| Work class | Local single-node | Trusted LAN personal cluster | Overlay-WAN personal cluster |
| --- | --- | --- | --- |
| Agent task (non-GPU critical) | Allowed (default) | Allowed (gated) | Allowed (gated, conservative timeout) |
| GPU inference task | Allowed | Allowed (lease-gated) | Allowed (lease-gated, latency caveats) |
| GPU training long-run | Allowed | Allowed (explicit profile and checkpointing) | Not default; pilot-only explicit opt-in |
| Distributed collectives | Optional local/LAN only | Pilot-only with strict topology constraints | Out of scope by default |

### Meaning of columns

- **Local single-node:** default developer and single-container flows; no Populi required.
- **Trusted LAN personal cluster:** nodes under a **single operator** or **agreed trust domain**, reachable on a **private LAN** with stable RTT; TLS/mTLS and bearer policy per [ADR 008](../adr/008-populi-transport.md).
- **Overlay-WAN personal cluster:** user-owned nodes joined across the **public internet** via VPN/wireguard-style overlay or equivalent; **control-plane** reachability may be decoupled from **bulk artifact** paths (see [overlay runbook](../operations/populi-overlay-personal-cluster-runbook.md)).

### Policy notes

- **Hosted donation** or multi-tenant **public GPU marketplace** remains **out of scope** for this wave ([ADR 009](../adr/009-populi-hosted-baas.md)).
- **Cloud provider dispatch** (`vox mens train --cloud`, provider nodes) is a **separate execution surface** from Populi mesh until an explicit convergence ADR merges them; see [Mens cloud GPU strategy](mens-cloud-gpu.md).
- Promoting **WAN distributed training** to a **default supported** path requires a **new ADR** and updated matrix row(s).

## Gating vocabulary

- **Gated:** requires explicit **config / policy / feature** enablement; not implied by joining a cluster.
- **Lease-gated:** requires **authoritative lease** semantics per [ADR 017](../adr/017-populi-lease-remote-execution.md) once implemented; until then treat remote GPU paths as **experimental** only.
- **Pilot-only:** documented rollout and [kill-switch validation](../operations/populi-remote-execution-rollout-checklist.md) required before production reliance.

## Related documentation

- [Populi GPU mesh implementation plan 2026](../archive/research-2026-q1/populi-gpu-mesh-implementation-plan-2026.md) — phased delivery (roadmap); Phase 5 tasks **`p5-placement-policy`**, **`p5-queued-capacity-rebalance`**, **`p5-gang-nccl-pilot`** cover unified placement, queued replanning on capacity changes, and collective pilot bounds.
- [Protocol convergence research 2026](../archive/research-2026-q1/protocol-convergence-research-2026.md) — transport and delivery-plane context.

