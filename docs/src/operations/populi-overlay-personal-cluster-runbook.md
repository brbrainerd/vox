---
title: "Populi overlay personal cluster runbook"
description: "Operator steps and WAN boundaries for user-owned Populi clusters across VPN-style overlays."
category: "Operations"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# Populi overlay personal cluster runbook

**Scope:** **Phase 6** personal clusters that use an **overlay** (for example WireGuard, Tailscale, ZeroTier) so Populi nodes behave like one fleet across the **WAN**. This is **not** a hosted public GPU pool and **not** default long-haul distributed training. See [work-type placement matrix](../reference/populi-work-type-placement-matrix.md) and [ADR 017](../adr/017-populi-lease-remote-execution.md).

## Preconditions

- Every process that should share membership uses a consistent **`VOX_MESH_SCOPE_ID`** when the control plane enforces scope ([mens SSOT](../reference/populi.md)).
- **Bearer / JWT** roles are configured via Clavis-backed secrets; never commit tokens to Compose files checked into git.
- **TLS termination** sits in front of **`vox populi serve`** per [ADR 008](../adr/008-populi-transport.md) when exposed beyond loopback.

## Enrollment (high level)

1. **Bring up the overlay** so each node has stable **virtual IPs** or DNS names; verify **MTU** and **UDP** reachability for the overlay product you use.
2. **Deploy the control plane** on a host that overlay peers can reach; bind to the **overlay interface** or a reverse proxy that listens there.
3. **Point workers** at `VOX_MESH_CONTROL_ADDR` / `VOX_ORCHESTRATOR_MESH_CONTROL_URL` using the **overlay URL**, not a public LAN IP that disappears off-site.
4. **Join + heartbeat:** use the same intervals as LAN (see mens SSOT); add **exponential backoff** on 429/503 as for local clusters.
5. **Bootstrap tokens:** prefer **`VOX_MESH_BOOTSTRAP_TOKEN`** exchange for one-shot join on new nodes instead of copying long-lived mesh tokens into chat or email.

## Security posture

- Treat **`GET /health`** as the only intentionally unauthenticated route; everything under **`/v1/populi/*`** must see **Bearer/JWT** when the server is configured with secrets.
- **Split tokens:** use **worker** vs **submitter** roles so compromise of a deliver-only client cannot reconfigure nodes.
- **Scope id** is a **tenancy** boundary: do not reuse one scope id across unrelated users “for convenience.”
- **Quarantine** (`POST /v1/populi/admin/quarantine`) is the fast **stop serving new mesh work** lever for a suspect node while you investigate.

## WAN boundaries and expectations

| Topic | Expectation |
| --- | --- |
| **Control plane RTT** | Higher and more variable than LAN; heartbeats and lease renewals must use **conservative** timeouts in pilot configs. |
| **Bulk artifacts / checkpoints** | Do **not** assume large files ride the same path as **HTTP join/heartbeat**; use object storage, `rsync` over overlay, or another **data plane** you control. |
| **Inference / interactive agents** | Usable with **lease-gated** remote execution when implemented; expect **latency** and **jitter** to dominate UX on consumer links. |
| **Long GPU training** | **Not default** over overlay WAN in the matrix; pilot-only with **checkpointing**, **explicit opt-in**, and [rollout checklist](populi-remote-execution-rollout-checklist.md). |
| **Distributed collectives** | **Out of scope by default** across WAN; requires dedicated topology and ADR-level approval if promoted. |

## Failure modes

- **Partition:** nodes may appear **stale** in `GET /v1/populi/nodes`; compare `last_seen_unix_ms` and apply **`VOX_MESH_MAX_STALE_MS`** client-side filtering.
- **Asymmetric routing:** verify both directions on the overlay before debugging Populi; traceroute/ping **inside the tunnel** first.
- **Double execution:** until [ADR 017](../adr/017-populi-lease-remote-execution.md) is implemented for your task class, assume **experimental relay** does **not** provide ownership guarantees—**local** queues remain authoritative.

## Related documentation

- [Deployment compose SSOT](../reference/deployment-compose.md) — image profiles and env blocks.
- [Protocol convergence research 2026](../archive/research-2026-q1/protocol-convergence-research-2026.md) — broader transport synthesis.
- [Mens SSOT](../reference/populi.md) — current API and env reference.


