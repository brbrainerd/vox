---
title: "Protocol convergence research 2026"
description: "Advisory synthesis: converging on one taxonomy and durable SSOT (Vox DB / Codex) while choosing HTTP, SSE, WebSockets, MCP, and mesh transports by semantic lane—not by forcing a single wire format."
category: "Architecture SSOTs"
status: "research"
sort_order: 6
last_updated: "2026-03-29"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Protocol convergence research 2026

**Status:** This page is **research and advisory**. It does not change shipped behavior. Decisions that bind the codebase belong in ADRs and contract updates after review.

## Purpose

Vox uses many communication surfaces: **MCP** (stdio and optional remote gateway), **HTTP** APIs (Populi control plane, Codex HTTP, webhooks), **WebSockets** (MCP gateway option, OpenClaw), **SSE** (runtime streaming), **JSON-lines / DeI RPC**, **LSP**, and in-process buses. The goal of this document is to:

- Align with the repo policy of a **single taxonomy**, not a single protocol everywhere.
- Center **durable truth** on **Vox DB / Codex** (per [ADR 004](../adr/004-codex-arca-turso-ssot.md)).
- Identify **duplications, gaps, and SSOT opportunities** for a future implementation plan.

Authoritative inventories:

- Machine-readable: [`contracts/communication/protocol-catalog.yaml`](../../../contracts/communication/protocol-catalog.yaml)
- Prose companion: [Communication protocols](../reference/communication-protocols.md)
- Orchestrator planes: [Unified orchestration](../reference/orchestration-unified.md)
- Mesh: [Populi SSOT](../reference/populi.md)

---

## 1. Current state (as documented in-repo)

### 1.1 Delivery planes

The catalog defines five planes used across families:

| Plane | Durability | Typical use in Vox |
| --- | --- | --- |
| `local_ephemeral` | None | In-process A2A bus, actor mailboxes, MCP stdio session |
| `local_durable` | Durable on host | DB inbox, persistence outbox |
| `remote_mesh` | Durable + HTTP semantics | Populi control plane, mesh A2A relay |
| `broadcast` | Mixed | Bulletin/event fanout, subscription-style notifications |
| `stream` | Mixed | SSE, optional MCP gateway streams, OpenClaw WS, DeI JSON lines |

**Policy (already in-tree):** Do not collapse `local_ephemeral`, `local_durable`, and `remote_mesh` into one transport with hidden semantics. See [Communication protocols — reduction policy](../reference/communication-protocols.md).

### 1.2 Protocol families (summary)

Representative families from the catalog (not exhaustive):

| Family | Wire | Notes |
| --- | --- | --- |
| MCP stdio | JSON-RPC + MCP over stdin/stdout | Default editor/host control |
| MCP HTTP gateway | HTTP JSON + optional WebSocket JSON | Remote/mobile; bounded, opt-in |
| Populi control plane + A2A relay | HTTP + JSON (OpenAPI) | Mesh; A2A relay marked `evaluate` for overlap vs DB inbox |
| Orchestrator local A2A | In-process types | Low-latency same-node |
| Orchestrator DB inbox / outbox | SQL + JSON schemas (outbox) | Durable local delivery |
| Runtime SSE | HTTP event-stream | Default app streaming per catalog |
| DeI JSON-line RPC | JSON lines over pipes | CLI/daemon; `evaluate` for convergence |
| LSP | JSON-RPC | Ecosystem; not Vox-envelope merge candidate |
| OpenClaw | WebSocket JSON | **WS-first** per [ADR 013](../adr/013-openclaw-ws-native-strategy.md) |
| Codex HTTP API | OpenAPI HTTP | Service/public API family |
| Webhook delivery | HTTP | Catalog `experimental` |

### 1.3 Persistence authority

Per **ADR 004**, **Codex / `VoxDb`** over **Turso/libSQL** is the single product data plane. Convex-like behaviors (subscriptions, invalidation) are **capabilities on Codex**, not a second database. Orchestrator durability patterns (inbox/outbox) should remain conceptually **subordinate to that SSOT** for anything that must survive restarts or be replayed—while keeping ephemeral agent traffic out of the DB unless semantics require it.

Mesh-specific: Populi telemetry and registry events can feed Codex when enabled (see [orchestration unified](../reference/orchestration-unified.md) env table).

archived_date: 2026-04-18
---

## 2. Semantic lanes and recommended defaults

Choose transport by **semantics** (durability, directionality, auth boundary, ordering), not by habit.

### 2.1 Lane matrix

| Lane | Primary need | Default | Exceptions / when to deviate |
| --- | --- | --- | --- |
| Host / editor control | Tooling RPC, subprocess lifecycle | **MCP stdio** | Remote access: **MCP Streamable HTTP** (align with MCP spec); gateway features remain bounded |
| Browser / app: server → client stream | Token stream, live logs, one-way feed | **SSE** | Need true client→server on same socket: **WebSocket**; very high fan-in may need framing + backpressure discipline |
| Browser / app: bidirectional session | Interactive channel, gaming-style duplex | **WebSocket** | Future: **WebTransport** if QUIC/datagram needs dominate and ecosystem catches up |
| Same-node agent coordination | Lowest latency, no cross-process guarantee | **In-process bus** (`local_ephemeral`) | Never “upgrade” to WS for same-process semantics alone |
| Cross-process durable handoff | Survive restart, explicit ack | **DB inbox / outbox** (`local_durable`) | — |
| Cross-node / mesh | Tenancy, bearer/JWT, lease/ack | **Populi HTTP** | QUIC/gRPC only after **replacement ADR** per [ADR 008](../adr/008-populi-transport.md) |
| External SaaS → Vox | Signed POST, short handler | **HTTP webhook ingress** + **async queue** pattern | Prefer provider webhooks over blind polling when offered |
| Vox → external callback | Reliability, retries | **HTTP client** + idempotency + backoff | — |
| Ecosystem editor protocol | LSP | **LSP as-is** | Do not merge into Vox-only envelopes |
| Upstream-native gateway | OpenClaw | **WebSocket-first** | HTTP compatibility secondary per ADR 013 |

### 2.2 MCP-specific note (external spec alignment)

The **Model Context Protocol** defines **stdio** and **Streamable HTTP** as standard transports; treat **WebSocket on the MCP HTTP gateway** as a **Vox extension path** for clients that need a long-lived JSON session, not as the canonical MCP transport. Remote deployments should prefer **spec-aligned HTTP** semantics and **authorization** patterns from the MCP documentation.

### 2.3 SSE vs WebSocket (product guidance)

- **SSE:** one-way, HTTP-friendly, automatic reconnect in browsers; mind per-origin connection limits on HTTP/1.1 (MDN documents this tradeoff).
- **WebSocket:** full duplex; **no built-in backpressure** on the classic `WebSocket` API (MDN)—design explicit flow control, buffering caps, or bounded queues for agent or token floods.

**Repo alignment:** [Communication protocols](../reference/communication-protocols.md) states not to replace runtime SSE with WebSocket by default.

---

## 3. Duplications, overlaps, and evaluation targets

### 3.1 Intentional overlap (do not merge casually)

| Area | Why two paths exist | Convergence rule |
| --- | --- | --- |
| Populi A2A relay vs orchestrator DB inbox | Remote mesh vs host-local durability | Merge or retire only after [retirement checkpoints](../reference/communication-protocols.md) + telemetry |
| MCP stdio vs MCP HTTP gateway | Local vs remote control | Keep both; gateway stays **opt-in** and bounded |
| SSE vs MCP WS gateway vs OpenClaw WS | Different products and capabilities | Do not unify wire code; unify **metadata/tracing** where possible |

### 3.2 Likely simplification opportunities (for a future plan)

- **Envelope and metadata:** Multiple stacks repeat JSON shapes and correlation concepts without a single cross-plane “message context” SSOT (see §4).
- **Client duplicates:** Extension MCP client paths (e.g. legacy vs preferred client) increase maintenance; convergence is **TypeScript surface**, not wire protocol.
- **Catalog vs product:** Some families (e.g. webhooks) may be `experimental` in the catalog while crates exist—keep catalog status honest to avoid governance drift.
- **Research vs shipped MCP optimizations:** Docs such as [MCP optimization strategy](../explanation/mcp_serverless_research.md) describe aspirational paths; keep a clear boundary in planning so experiments do not fork production semantics silently.

### 3.3 Mesh / Populi

- **HTTP-first** is a decided baseline ([ADR 008](../adr/008-populi-transport.md)). Federation visibility (`GET /v1/populi/nodes`) is separate from remote execution experiments—operators should not treat routing experiments as transport truth.
- **Idempotency:** Mesh A2A deliver semantics (client-supplied keys, digit-string agent IDs) are part of the contract; any convergence work must preserve or explicitly migrate them ([Populi SSOT](../reference/populi.md)).

### 3.4 Populi as a future GPU mesh

The repo now has a dedicated research page for this question: [Populi GPU network research 2026](populi-gpu-network-research-2026.md).
Implementation sequencing for that direction now lives in [Populi GPU mesh implementation plan 2026](populi-gpu-mesh-implementation-plan-2026.md).

High-level implications for protocol and architecture work:

- **Control plane is not execution ownership:** Populi's current HTTP API is a workable baseline for discovery, identity, and A2A relay, but it does not yet define authoritative remote GPU execution.
- **Remote mesh and local durability remain different lanes:** a future GPU scheduler should not erase the distinction between `remote_mesh` and `local_durable`; it should define how work crosses those lanes and who owns recovery.
- **HTTP can remain the control baseline:** the largest current gaps are worker lifecycle, GPU truth, checkpointing, and remote ownership semantics, not the absence of a second in-tree transport.
- **Internet-distributed user-owned clusters need an explicit security posture:** secure overlays, policy-based enrollment, and least-privilege access are a better default than ambient discovery or public endpoint exposure.
- **Distributed GPU work is stricter than cross-node messaging:** WAN reachability and node listing are not enough for efficient collectives or long-running training jobs; topology, retries, and checkpoint/resume behavior matter.
- **ADR threshold remains unchanged:** replacing HTTP with another default transport, or redefining durable queue ownership across planes, still needs an ADR; research-only framing and additive guidance do not.

archived_date: 2026-04-18
---

## 4. SSOT gaps (priority for a future implementation plan)

These items reduce **conceptual** protocol diversity more than picking “HTTP everywhere”:

1. **Cross-plane message context**  
   Standard fields (or headers) for: `trace_id`, `span_id` or equivalent, `correlation_id`, `conversation_id`, `repository_id` / tenancy, `source_plane` (`local_ephemeral` | `local_durable` | `remote_mesh` | …), `schema_version`.

2. **Idempotency SSOT**  
   Populi already has `idempotency_key` patterns; HTTP tool routes and internal POST handlers should document whether they honor **Idempotency-Key** (IETF draft) or an application key, and for how long keys live.

3. **Durable vs ephemeral boundary**  
   Explicit criteria: *when must a message become a Codex row?* Default: **ephemeral** unless cross-process, regulatory, replay, or user-visible recovery requires durability.

4. **Outbox / inbox documentation vs code**  
   Outbox has JSON schema; DB inbox is referenced in prose—consider machine-readable contract parity when consolidation is attempted.

5. **Observability**  
   For queue-like paths, align with **OpenTelemetry messaging semconv** (producer/send/receive/process/settle vocabulary) where feasible, even if the “broker” is Populi HTTP or Codex polling.

6. **Security posture per plane**  
   MCP HTTP: OAuth/dynamic-client pitfalls ([MCP security best practices](#appendix-b-external-sources)); mesh: bearer/JWT roles already in Populi docs; webhooks: signature + fast ack + async processing ([GitHub best practices](#appendix-b-external-sources)).

7. **External agent interoperability**  
   Treat **A2A** (industry peer protocol) as an **interop lane** for third-party agents; map to Vox planes instead of replacing MCP or Populi.

---

## 5. Agent-to-agent and owned-agents distinction

| Context | Guidance |
| --- | --- |
| **Agents we own** (same repo, same orchestrator) | Prefer **in-process** + **Codex** for durability; use **Populi** only when placement crosses nodes. |
| **External agents / vendors** | Use **documented HTTP + capability advertisement** patterns; consider **A2A** where appropriate; **MCP** for tool/data attachment per ecosystem. |
| **Guardrail** | Never assume another agent shares memory; **persist handoff** at boundaries when failure must be recoverable. |

archived_date: 2026-04-18
---

## 6. Prerequisites for a follow-on implementation plan

Before locking an implementation roadmap, stakeholders should close these **decision inputs**:

| Prerequisite | Output artifact |
| --- | --- |
| Telemetry on Populi relay vs DB inbox | Evidence report (latency, duplicates, tenancy, operator UX) |
| MCP gateway transport matrix | Doc + tests: which clients use stdio vs HTTP vs WS; security checklist |
| Envelope metadata RFC (internal) | Small schema or OpenAPI `components` shared across families |
| Webhook product status | Either promote catalog status or narrow crate scope |
| ADR trigger list | e.g. Populi QUIC/gRPC **replacement** only via new ADR superseding 008 |

**When to write an ADR:** Any default transport change (e.g. SSE → WS default, or gRPC beside HTTP), or merging durable queues.

**When to update contracts only:** Additive fields on existing OpenAPI/JSON-schema, new optional headers, instrumentation hooks.

---

## Appendix A. Related internal documents

- [Communication protocols](../reference/communication-protocols.md)
- [SSOT / DRY convergence roadmap](ssot-convergence-roadmap.md)
- [VoxDB connection policy](voxdb-connect-policy.md)
- [MCP HTTP gateway contract](../reference/mcp-http-gateway-contract.md)
- [Codex HTTP API](../reference/codex-http-api.md)
- [Populi overlay personal cluster runbook](../operations/populi-overlay-personal-cluster-runbook.md) — WAN-connected personal clusters (operational boundaries)
- [ADR 017: Populi lease-based remote execution](../adr/017-populi-lease-remote-execution.md) — target ownership model for authoritative remote work

archived_date: 2026-04-18
---

## Appendix B. External sources

One-line relevance for research traceability (order does not imply priority).

1. **Model Context Protocol — Transports** — `https://modelcontextprotocol.io/docs/concepts/transports` — Official MCP transport model (stdio vs Streamable HTTP).
2. **MCP Specification — Transports** — `https://modelcontextprotocol.io/specification/2025-06-18/basic/transports` — Versioned transport details for implementation parity.
3. **MCP — Security best practices** — `https://modelcontextprotocol.io/specification/latest/basic/security_best_practices` — Proxy/deputy risks; informs MCP HTTP gateway hardening.
4. **MCP — Authorization** — `https://modelcontextprotocol.io/specification/latest/basic/authorization` — OAuth-oriented remote MCP deployments.
5. **MDN — Using server-sent events** — `https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events` — SSE defaults, limits, keep-alive patterns.
6. **MDN — WebSocket API** — `https://developer.mozilla.org/en-US/docs/Web/API/WebSocket_API` — Duplex use cases; backpressure and **WebTransport** positioning.
7. **MDN — WebTransport API** — `https://developer.mozilla.org/en-US/docs/Web/API/WebTransport_API` — Future/alternate to classic WebSockets for advanced cases.
8. **RFC 6455 — WebSocket Protocol** — `https://datatracker.ietf.org/doc/html/rfc6455` — Normative wire semantics for WS lanes.
9. **gRPC — Performance best practices** — `https://grpc.io/docs/guides/performance/` — Streaming vs unary; load-balancing caveats on long-lived streams.
10. **Microsoft Learn — Compare gRPC with HTTP APIs** — `https://learn.microsoft.com/en-us/aspnet/core/grpc/comparison` — When JSON/HTTP wins vs stub-based RPC.
11. **AWS Prescriptive Guidance — Transactional outbox** — `https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/transactional-outbox.html` — Dual-write avoidance; **idempotent consumers**.
12. **microservices.io — Transactional outbox** — `https://microservices.io/patterns/data/transactional-outbox.html` — Pattern semantics and relay ordering.
13. **IETF draft — Idempotency-Key header** — `https://datatracker.ietf.org/doc/html/draft-ietf-httpapi-idempotency-key-header` — Fault-tolerant **POST** retries (draft).
14. **OpenTelemetry — Messaging spans** — `https://opentelemetry.io/docs/specs/semconv/messaging/messaging-spans` — Vocabulary for **produce/process/settle** on queue-like paths.
15. **CloudEvents — Specification** — `https://github.com/cloudevents/spec/blob/v1.0/spec.md` — **Vendor-neutral event envelope** for cross-system messages.
16. **CloudEvents — HTTP binding** — `https://github.com/cloudevents/spec/blob/main/cloudevents/bindings/http-protocol-binding.md` — HTTP mapping for webhook-style delivery.
17. **AsyncAPI — Specification** — `https://www.asyncapi.com/docs/reference/specification/latest` — Describes event-driven and **WebSocket** APIs consistently.
18. **A2A Protocol — What is A2A** — `https://a2a-protocol.org/latest/topics/what-is-a2a/` — Official overview; external **agent-to-agent** interop; complements MCP.
19. **A2A — Protocol specification** — `https://a2a-protocol.org/latest/specification/` — Peer agent patterns (documented transports include HTTP, JSON-RPC, SSE).
20. **GitHub Docs — Webhook best practices** — `https://docs.github.com/en/webhooks/using-webhooks/best-practices-for-using-webhooks` — Secrets, HTTPS, fast ack, async processing.
21. **GitHub Docs — REST API best practices** — `https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api` — Prefer webhooks vs polling where applicable.
22. **Microsoft Learn — Asynchronous Request-Reply** — `https://learn.microsoft.com/en-us/azure/architecture/patterns/async-request-reply` — **202 + status** pattern for long work without blocking HTTP indefinitely.
23. **OAuth 2.0 Security BCP (RFC 9700)** — `https://datatracker.ietf.org/doc/html/rfc9700` — Referenced by MCP security material for authz hardening.
24. **WebSocket.org — WebSocket vs SSE** — `https://websocket.org/comparisons/sse/` — Concise duplex vs one-way comparison for product discussions.
25. **MCP Blog — Future of transports** — `https://blog.modelcontextprotocol.io/posts/2025-12-19-mcp-transport-future/` — Ecosystem direction (research context only).

---

## Revision history

| Date | Change |
| --- | --- |
| 2026-03-28 | Initial advisory: lane matrix, overlap analysis, SSOT gaps, bibliography; A2A overview link uses [a2a-protocol.org](https://a2a-protocol.org/latest/topics/what-is-a2a/). |


