---
title: "ADR-025: Multi-Agent Lock Coherence and Lease Propagation"
description: "Architecture Decision Record for multi-agent lock coherence and lease propagation in the Vox ecosystem."
category: "Architecture Decisions (ADRs)"
status: "research"
---
# ADR 025: Multi-Agent Lock Coherence and Lease Propagation

## Status
Proposed (2026-04-23)

## Context
As Vox moves toward multi-agent environments (multiple agents working on the same task or in the same workspace), resource contention (not just file-level) becomes a risk. We need a way to lock generic resources (URIs, DB rows) and propagate these locks across the bulletin board.

## Decision
We extend the `locks` subsystem with a `ResourceLockManager`:
1. **Generic Resource IDs**: Locks can be held on arbitrary strings (URIs).
2. **Lease-based Expiration**: All locks have a mandatory TTL to prevent deadlocks from crashed agents.
3. **Bulletin Synchronization**: Lock acquisition and release events are broadcast as `AgentMessage` variants to ensure all agents are aware of the coherence state.

## Consequences
- Agents can safely coordinate on non-file resources.
- The system is resilient to agent failures via lease expiration.
- Real-time visualization of resource contention is possible via the bulletin board.
