---
title: "ADR 013 — OpenClaw WS-first native interop"
description: "Official documentation for ADR 013 — OpenClaw WS-first native interop for the Vox language."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-27"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 013 — OpenClaw WS-first native interop

**Status**: Accepted  
**Date**: 2026-03-27

## Context

Vox previously integrated OpenClaw primarily through HTTP skill import surfaces (`/v1/skills`) and a feature-gated CLI lane. This left a gap between:

- OpenClaw's native Gateway protocol (WebSocket control plane),
- Vox runtime/CLI operations that need session-scoped control calls,
- and `.vox` script ergonomics.

## Decision

Adopt a WS-first integration strategy with a stable Rust adapter boundary:

- **Primary transport**: OpenClaw Gateway WS handshake and method frames.
- **Secondary fallback**: HTTP compatibility and skills endpoints remain supported.
- **Adapter boundary**: `OpenClawRuntimeAdapter` in `vox-skills` isolates protocol transport from callsites.
- **Script bridge**: `.vox` uses a minimal `OpenClaw` builtin module (`list_skills`, `call`, `subscribe`, `unsubscribe`, `notify`) lowered through existing type/HIR/codegen paths.

## Security posture

- Keep TLS verification on by default.
- Resolve token via vox-secrets (`VOX_OPENCLAW_TOKEN`) when available.
- Prefer loopback/tailnet WS URLs (`VOX_OPENCLAW_WS_URL`) for operator sessions.
- Treat protocol errors as typed failures (`connect`, `transport`, `method`) for deterministic handling.

## Contract fixtures

The protocol contract baseline is fixture-driven:

- `contracts/openclaw/protocol/connect.challenge.json`
- `contracts/openclaw/protocol/connect.hello-ok.json`
- `contracts/openclaw/protocol/subscriptions.list.response.json`

`vox ci openclaw-contract` validates required files and shape invariants.

## Consequences

- `vox openclaw` command surface now supports direct WS gateway calls.
- Subscription-related commands use WS transport instead of simulation.
- `.vox` scripts gain low-k native OpenClaw calls without introducing parser islands.


