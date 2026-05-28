---
title: "Communication protocols"
description: "Canonical map of Vox communication protocol families, delivery planes, and coexistence rules."
category: "Language Reference"
status: "current"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# Communication protocols

This page is the prose companion to the machine-readable catalog at [`contracts/communication/protocol-catalog.yaml`](../../../contracts/communication/protocol-catalog.yaml).

## What is unified

Vox uses a **single taxonomy**, not a single wire format.

- Keep one machine-readable inventory of protocol families, delivery planes, and ownership.
- Keep one prose reference page per protocol family that points back to its contract artifact.
- Reuse helpers only where payload shape and lifecycle genuinely match.
- For **which wire to pick** when adding traffic (SSE vs WebSocket vs HTTP-only, MCP remote vs stdio, mesh vs DB inbox), use the lane matrix and bibliography in [Protocol convergence research 2026](../archive/research-2026-q1/protocol-convergence-research-2026.md) as advisory input; this reference page remains the **normative** inventory and reduction policy.

## Delivery planes

These are the canonical plane names used when comparing transports across the repo:

| Plane | Meaning | Typical examples |
| --- | --- | --- |
| `local_ephemeral` | Same-process delivery with no restart durability | actor mailboxes, orchestrator local A2A bus |
| `local_durable` | Host-local durable storage with explicit replay/ack semantics | DB inbox, persistence outbox |
| `remote_mesh` | Remote HTTP-mediated delivery across nodes with bearer/JWT auth | Populi control plane and relay |
| `broadcast` | Fanout where receivers observe local order only | subscription notifications, bulletin/event buses, webhooks |
| `stream` | Ordered incremental delivery over one connection or byte stream | runtime SSE, MCP WS gateway, OpenClaw WS, JSON-line daemons |

## Family matrix

| Family | Primary contract | Primary doc | Canonical decision |
| --- | --- | --- | --- |
| MCP stdio | [`contracts/mcp/tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml) | [`docs/src/reference/cli.md) | Keep as the default host/editor control surface |
| MCP HTTP gateway | [`contracts/mcp/http-gateway.openapi.yaml`](../../../contracts/mcp/http-gateway.openapi.yaml) | [`mcp-http-gateway-contract.md`](mcp-http-gateway-contract.md) | Keep bounded and opt-in for remote/mobile control |
| Populi HTTP control plane | [`contracts/populi/control-plane.openapi.yaml`](../../../contracts/populi/control-plane.openapi.yaml) | [`populi.md`](populi.md) | Keep HTTP-first per ADR 008 |
| Populi A2A relay | [`contracts/populi/control-plane.openapi.yaml`](../../../contracts/populi/control-plane.openapi.yaml) | [`populi.md`](populi.md) | Evaluate overlap only against DB inbox after telemetry-backed review |
| Orchestrator local A2A | in-code types only | [`orchestration-unified.md`](orchestration-unified.md) | Keep as the low-latency same-process lane |
| Orchestrator DB inbox / outbox | [`contracts/communication/orchestrator-persistence-outbox.schema.json`](../../../contracts/communication/orchestrator-persistence-outbox.schema.json) (outbox lifecycle/queue) + in-code DB inbox types | [`orchestration-unified.md`](orchestration-unified.md) | Keep durable semantics separate from ephemeral/local bus semantics |
| Runtime SSE | in-code types only | [`docs/src/reference/cli.md) | Keep SSE as the default app streaming transport |
| DeI JSON-line RPC | [`contracts/dei/rpc-methods.schema.json`](../../../contracts/dei/rpc-methods.schema.json) | [`orchestration-unified.md`](orchestration-unified.md) | Evaluate convergence only where envelopes already align |
| Orchestrator JSON-line RPC | [`contracts/orchestration/orch-daemon-rpc-methods.schema.json`](../../../contracts/orchestration/orch-daemon-rpc-methods.schema.json) | [`orchestration-unified.md`](orchestration-unified.md) | Keep separate from DeI while `vox-orchestrator-d` `orch.*` parity evolves |
| LSP JSON-RPC | external protocol | this page | Keep independent; ecosystem protocol |
| OpenClaw WS | fixture contracts under `contracts/openclaw/` | [`docs/src/adr/013-openclaw-ws-native-strategy.md`](../adr/013-openclaw-ws-native-strategy.md) | Keep WS-first because upstream is WS-native |
| Codex HTTP API | [`contracts/codex-api.openapi.yaml`](../../../contracts/codex-api.openapi.yaml) | [`codex-http-api.md`](codex-http-api.md) | Keep as a separate public/service API family |

## Current reduction policy

- Do not collapse `local_ephemeral`, `local_durable`, and `remote_mesh` into one abstract transport with hidden semantics.
- Do not add a parallel in-tree gRPC/QUIC default beside Populi HTTP without a replacement ADR.
- Do not replace runtime SSE with WebSocket by default.
- Do not merge external ecosystem protocols such as LSP or OpenClaw into Vox-specific RPC envelopes.

## Retirement checkpoints

Protocol families marked `evaluate` in the catalog should only be merged or removed when all of the following are true:

1. They serve the same use case.
2. They have compatible auth, durability, and observability needs.
3. There is a migration path with stable aliases or coexistence.
4. Existing telemetry and contract checks are sufficient to prove parity.

## Related

- [Documentation governance](../contributors/documentation-governance.md)
- [Protocol convergence research 2026](../archive/research-2026-q1/protocol-convergence-research-2026.md) — advisory: lanes, overlaps, SSOT gaps
- [Unified orchestration](orchestration-unified.md)
- [Mesh / Populi SSOT](populi.md)
- [Populi work-type placement policy matrix](populi-work-type-placement-matrix.md) — local / LAN / overlay boundaries
- [ADR 017: Populi lease-based remote execution](../adr/017-populi-lease-remote-execution.md), [ADR 018: Populi GPU truth layering](../adr/018-populi-gpu-truth-layering.md)
- [MCP HTTP gateway contract](mcp-http-gateway-contract.md)

