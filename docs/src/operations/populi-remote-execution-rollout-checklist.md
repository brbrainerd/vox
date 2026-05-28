---
title: "Populi remote execution rollout checklist"
description: "Go/no-go criteria, kill-switch validation, and rollback steps before enabling authoritative or pilot remote execution beyond local defaults."
category: "Operations"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# Populi remote execution rollout checklist

Use this checklist before widening **Populi remote execution** beyond **local-first** defaults—whether using today’s **experimental relay** or a future **lease-authoritative** path ([ADR 017](../adr/017-populi-lease-remote-execution.md)).

## Default-off validation

- [ ] **Documented scope:** confirm the deployment matches a column in the [work-type placement matrix](../reference/populi-work-type-placement-matrix.md) (local / LAN / overlay).
- [ ] **No accidental public bind:** Populi listeners and MCP HTTP gateways use **loopback** or **controlled ingress** unless TLS and auth are in place ([deployment compose SSOT](../reference/deployment-compose.md), [MCP HTTP gateway contract](../reference/mcp-http-gateway-contract.md)).
- [ ] **Secrets:** mesh tokens and JWT secrets live in **Secrets** / secret stores; `vox secrets doctor` passes for required workflows ([Secrets SSOT](../reference/secrets-ssot.md)).

## Kill switches (validate in staging)

Prove you can **disable remote paths** without redeploying code:

| Switch | Effect (current docs) |
| --- | --- |
| `VOX_ORCHESTRATOR_MESH_REMOTE_EXECUTE_EXPERIMENTAL=0` (unset/false) | Disables experimental **RemoteTaskEnvelope** relay; local execution unchanged ([orchestration unified](../reference/orchestration-unified.md)). |
| `VOX_ORCHESTRATOR_MESH_ROUTING_EXPERIMENTAL=0` | Disables hint-based **routing score** experiments ([mens SSOT](../reference/populi.md)). |
| `VOX_ORCHESTRATOR_MESH_CONTROL_URL` unset | Stops federation **node snapshot** reads from Populi (orchestrator/MCP) ([env vars](../reference/env-vars.md)). |
| `VOX_MESH_HTTP_JOIN=0` | MCP skips HTTP **join/heartbeat** while other mesh hooks may still run ([mens SSOT](../reference/populi.md)). |
| `VOX_MESH_ENABLED=0` | Disables mens hooks in processes that respect this flag ([mens SSOT](../reference/populi.md)). |

**Staging drill:** toggle each relevant switch, restart or reload the affected process per your platform, and confirm **no remote fan-out** and **no unexpected control-plane traffic** (packet capture or access logs).

## Functional gates (pilot)

- [ ] **Single owner:** for lease-backed task classes (when implemented), reproduce **lease acquisition**, **renewal**, and **expiry**; confirm **no concurrent** execution on two nodes for the same correlation id.
- [ ] **Fallback:** on lease loss, verify **local fallback** or **documented fail-closed** behavior per operator policy ([ADR 017](../adr/017-populi-lease-remote-execution.md)).
- [ ] **Cancellation:** remote cancel paths propagate within agreed timeouts.
- [ ] **Results:** result or failure delivery is **idempotent** on redeliver (mesh **idempotency_key** where used).

## Observability gates

- [ ] Logs or traces include **`task_id`** (or equivalent) for routed work; when lease placement ships, include **`lease_id`** and **placement reason** per [placement observability](../reference/orchestration-unified.md#placement-and-lease-observability).
- [ ] Optional: **`VOX_MESH_CODEX_TELEMETRY`** emits **`populi_control_event`** rows without storing bearer material ([mens SSOT](../reference/populi.md)).

## Regression and rollback

- [ ] **CI / smoke:** `vox ci check-links` and mdBook build succeed after doc changes; workspace tests for Populi/orchestrator crates pass for the PR that enables new behavior.
- [ ] **Rollback plan:** document which env toggles return the fleet to **local-only** execution and who is allowed to flip them.

## Go / no-go

| Outcome | Condition |
| --- | --- |
| **Go** | Kill-switch drill passed; matrix row matches workload; observability fields confirmed in pilot logs. |
| **No-go** | Any unexplained duplicate execution, missing fallback on forced partition, or inability to disable relay via env within minutes. |

## Related documentation

- [Overlay personal cluster runbook](populi-overlay-personal-cluster-runbook.md)
- [Populi GPU mesh implementation plan 2026](../archive/research-2026-q1/populi-gpu-mesh-implementation-plan-2026.md) — roadmap sequencing


