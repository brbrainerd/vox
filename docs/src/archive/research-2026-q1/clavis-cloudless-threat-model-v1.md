---
title: "Clavis Cloudless Threat Model V1"
description: "Threat model, source policy matrix, and break-glass governance for Clavis Cloudless hard-cut execution."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-06"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Clavis Cloudless Threat Model V1

This document is the control-plane security baseline for the hardened Clavis Cloudless rollout.

## Scope

- Secret resolution and persistence paths tied to Clavis and VoxDB.
- Dataflow paths that can expose secret material in logs, traces, MCP outputs, or model context.
- Break-glass controls for emergency access.

Primary code anchors:

- `crates/vox-clavis/src/lib.rs`
- `crates/vox-clavis/src/resolver.rs`
- `crates/vox-clavis/src/lib.rs`
- `crates/vox-db/src/config.rs`
- `crates/vox-orchestrator/src/mcp_tools/http_gateway.rs`
- `crates/vox-runtime/src/llm/types.rs`
- `crates/vox-publisher/src/publication_preflight.rs`
- `crates/vox-orchestrator/src/config/impl_env.rs`
- `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs`

## Threat actors and failure modes

1. **Developer endpoint compromise**
   - Local env/keyring exfiltration, shell history leaks, debug dumps.
2. **CI runner compromise**
   - Secret exposure via job logs/artifacts or modified pipeline behavior.
3. **Prompt/tool-output exfiltration**
   - Secret material enters model-visible context through tool payloads or diagnostics.
4. **Backend outage or stale replicas**
   - Resolver fallback risks insecure source selection if policy is weak.
5. **Control-plane misuse (privileged operator)**
   - Unauthorized break-glass use without immutable audit and post-incident rotation.

## Secret classes

- `runtime`: tokens used during active request handling.
- `account`: user/account-scoped persisted secrets.
- `operator`: administrative and break-glass credentials.
- `integration`: third-party provider and publication credentials.
- `transport`: inter-service bearer/JWT/HMAC material.
- `bootstrap`: setup-only credentials, low-frequency and tightly controlled.

## Allowed source matrix (hard-cut target)

| Secret class | Env | Keyring | Cloudless VoxDB | External backend | Notes |
| --- | --- | --- | --- | --- | --- |
| `runtime` | Limited (dev/ci only) | Optional local cache | Required in strict profiles | Optional | No deprecated aliases in hard-cut strict mode. |
| `account` | No (strict) | Bootstrap only | Primary | Optional mirror | Ciphertext-at-rest and versioned writes required. |
| `operator` | Limited (break-glass only) | Yes | Optional | Yes | Must require reason code + immutable audit event. |
| `integration` | Transitional only | Optional | Preferred | Optional | Target Clavis-first for all consumers. |
| `transport` | No (strict) | Optional local | Preferred | Optional | No raw token echo in diagnostics. |
| `bootstrap` | Yes (one-time) | Yes | Optional | Optional | Rotate immediately after bootstrap completion. |

## Hard-cut policy requirements

- Legacy aliases and deprecated alias sources are rejected in strict profiles.
- Missing required secrets in strict profiles must fail closed.
- Resolver must return typed rejection status, never silent fallback.
- No source may leak secret value into logs, telemetry, or prompt/tool payload.

## Break-glass and JIT governance

### Activation requirements

- Named operator identity.
- Incident/ticket reference.
- Explicit reason code from approved list.
- Time-bounded credential (TTL) and automatic expiry.

### Mandatory controls

- Immutable audit event for grant, use, and revoke.
- Dual authorization for privileged classes (`operator`, `transport`).
- Immediate post-incident rotation for all credentials touched.
- Mandatory incident review before returning to normal mode.

### Prohibited patterns

- Permanent break-glass credentials.
- Shared unscoped root tokens for normal operations.
- Break-glass use without ticket/reason/audit evidence.

## Security invariants for implementation

1. No plaintext secret persistence in VoxDB rows.
2. No secret value in logs/traces/MCP responses/model prompts.
3. Strict profiles do not use deprecated aliases.
4. CI must block new direct secret env reads outside sanctioned source modules.
5. Cloudless backend failures produce typed errors; no insecure fallback.


