---
title: "ADR 020: Populi mesh scaling — default transport posture"
description: "Records the decision to keep the HTTP control plane as the default scaling surface and when to adopt gossip or QUIC."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 020: Populi mesh scaling — default transport posture

## Status

**Accepted.** Narrows product/engineering choices for scaling personal and lab clusters described in [Populi GPU mesh implementation plan 2026](../archive/research-2026-q1/populi-gpu-mesh-implementation-plan-2026.md).

## Context

Populi today is a **hub-and-spoke HTTP control plane** (join, heartbeat, A2A, exec leases). Alternatives (gossip membership, P2P overlays, QUIC data planes) reduce custom code but increase operational and security surface. The codebase and docs already treat **overlay WAN** as an **operator-enrolled** boundary, not ambient internet discovery.

## Decision

1. **Default remains HTTP Populi** as the coordination SSOT until a future ADR explicitly replaces [ADR 008](008-populi-transport.md) as the default transport.
2. **Optional additive layers** (evaluated only after GPU truth + lease correctness are trustworthy):
   - **Gossip / SWIM-style membership** (e.g. `memberlist` crate) as *health and discovery hints*, not as the execution ownership store.
   - **QUIC-oriented data planes** (e.g. `quinn`, `quic-rpc`) for artifact / stream-heavy paths where HTTP is limiting.
   - **Integrated NAT traversal** (e.g. `iroh`) only if product requires routine **non-overlay** WAN mesh without operator-provided VPN.
3. **libp2p** is **out of scope** for the current personal-cluster wave unless the project explicitly adopts a peer-first architecture with its own ADR.

## Consequences

- Engineering effort prioritizes **correct leases**, **probe-backed GPU fields**, **paged A2A**, and **lifecycle docs** over new transport stacks.
- When gossip or QUIC is introduced, it must remain **additive**: existing HTTP clients and OpenAPI contracts keep working.

## Related

- [Protocol convergence research 2026](../archive/research-2026-q1/protocol-convergence-research-2026.md)
- [Populi work-type placement matrix](../reference/populi-work-type-placement-matrix.md)


