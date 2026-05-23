---
title: "ADR 008: Mens transport"
description: "Official documentation for ADR 008: Mens transport for the Vox language. Detailed technical reference, architecture guides, and implement"
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 008: Mens transport

## Context

Vox needs a **CPU-first** mens: workers advertise capabilities and can federate beyond a single process. We want one control-plane stack to avoid dual maintenance (no parallel gRPC + QUIC servers in-tree).

## Decision

1. **In-tree control plane (phase 3 baseline):** **HTTP** (`axum`) on a configurable bind address (`VOX_MESH_CONTROL_ADDR` for clients; `vox populi serve --bind` for servers) with JSON bodies (`NodeRecord`, `PopuliRegistryFile`). Operations: **health** (`GET /health`, unauthenticated), **join**, **heartbeat**, **list**, **leave**.
2. **Security:** **TLS termination** (mTLS at reverse proxy / sidecar) remains an operator concern. **`VOX_MESH_TOKEN`**: when set, the in-process server requires `Authorization: Bearer <token>` on mens API routes except **`GET /health`** (never logged); clients use the same env for outbound calls (`PopuliHttpClient::with_env_token`). **`VOX_MESH_SCOPE_ID`**: when set on the server, **join** and **heartbeat** require matching `NodeRecord.scope_id` ([mens SSOT](../reference/populi.md)).
3. **Future evolution:** If WAN gossip or stream multiplexing requires it, evaluate **QUIC** or **gRPC over TLS** as a **replacement** transport behind the same logical operations (join / heartbeat / list), not an additional default stack.

## Consequences

- Integration tests can spin two Tokio tasks on loopback without external binaries.
- Operators run `vox populi serve` behind `nginx`/`caddy`/`Envoy` for TLS and auth.
- Dual HTTP+gRPC servers are **explicitly rejected** until a migration ADR supersedes this one.

## Addendum: experimental orchestrator routing (in-process only)

**Status:** optional / best-effort — **not** part of the transport contract.

When **`VOX_ORCHESTRATOR_MESH_ROUTING_EXPERIMENTAL=true`**, embedders (e.g. `vox-mcp`) may feed cached **`GET /v1/populi/nodes`** capability hints into `RoutingService` for **extra logging and soft score bumps** on **local** agent queues. **Remote task execution is out of scope:** no RPC in this ADR dispatches work to another node. Semantics may change or be removed in a breaking release if replaced by a real placement layer; operators must not rely on it for correctness or SLA.


