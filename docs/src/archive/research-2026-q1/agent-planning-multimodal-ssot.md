---
title: "agent-planning-multimodal-ssot.md"
description: "Documentation for agent-planning-multimodal-ssot.md."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Project architecture context."
archived_date: 2026-04-18
---
# Agentic Planning and MENS Multimodal Boundaries (SSOT)

*Last Updated: April 2026*

This document serves as the Single Source of Truth (SSOT) for the Phase 4 and Phase 5 integration boundaries regarding MENS Multimodal Capabilities and Agentic Planning V2.

## 1. MENS Multimodal Execution Architecture

To prevent token window bloat and unbounded K-complexity, **Vox strictly enforces a "No Pixels in Prompt" policy** for the `vox-orchestrator` and MENS pipeline.

### The Capability Contract
- **Heuristics Eradicated**: Prompt substring inferences (e.g. `prompt.contains("screenshot")`) have been entirely removed.
- **Explicit Manifest Gating**: Vision workflows are *only* executed if a well-formed `attachment_manifest` with valid SHA digests mapped to `.png`/`.webp` blobs is present in the `DispatchPayload`.
- **Validation**: Strict boundary rules are enforced through `contracts/eval/vision_rubric.v1.schema.json`.

*Implementation Note: This schema avoids accidental remote VLM calls or catastrophic MENS sequence failures on ambiguous context.*

## 2. Agentic Planning V2: Deterministic Dispatch

The execution of plans previously relied on string-parsed task contexts which led to looping failure states (also known as the `MAX_A2A_BOUNCE` bug). The V2 engine intercepts the workflow at the task graph level to establish strict programmatic bounds.

### Execution Gating (`RequiresApproval`)
The `PlanningOrchestrator` explicitly enqueues tasks with a `RequiresApproval = Some(true)` flag injected deterministically via `hash(session_id:step_id)`.

A task hitting this limit pauses indefinitely inside `vox-db` with the status `"blocked_on_approval"`.

### Local Developer Loop
To enable headless operation and fast-loop integration tests without requiring the visualizer IDE UI clicks, the CLI exposes manual bridging:
- `vox plan create <goal> --approve`: Queues a plan with immediate progression.
- `vox plan replan <session-id>`: Forces re-evaluation of the planner loop on blocked or stalled plans.

### SSE Telemetry Stream
The `vox-codex-api` subsystem houses the `routes::plans` module. It consumes `vox-db` table changes (`plan_nodes`) and streams SSE transitions into the frontend orchestrator context, matching the "AI-Native Core" philosophy of clear, unidirectional state projection.

## 3. Operations Bound inside Vox Db
All state mutations are mapped directly into atomic transactional boundaries inside `crates/vox-db/src/store/ops_planning.rs` (via Turso/Sqlite endpoints). This completely eliminates lossy `.md` persistence races on long-running worker loops.

