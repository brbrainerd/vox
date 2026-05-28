---
title: "ADR 021: Generated workflow durability parity"
description: "Defines the compatibility contract for bringing generated Rust workflows to durable replay parity."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 021: Generated workflow durability parity

## Status

Accepted (design gate before implementation).

## Context

Interpreted workflows currently define the durable replay contract (`journal_version = 1`) and generated Rust workflows still lower to plain `async fn` execution. This leaves a parity gap between language-level workflow syntax and generated-runtime behavior.

## Decision

1. Generated workflow durability must converge on replay-compatible history semantics with interpreted workflow runs.
2. Parity rollout is feature-gated and limited to the supported subset validated by compatibility tests.
3. Generated durable workflows must preserve run identity and step identity compatibility:
   - `run_id` remains stable for resume
   - stable `activity_id` remains the replay/idempotency key
4. Durable contracts are versioned. Breaking shape changes require explicit version bumps and migration strategy.
5. Compatibility gate is mandatory before widening syntax support:
   - interpreted vs generated replay-history equivalence tests on the supported subset
   - old-run replay tests across code upgrades
   - schema/journal compatibility tests for persisted rows

## Supported subset for initial parity

- linear activity execution
- deterministic `if` branch decisions recorded as durable events
- durable timer wait replay (`workflow_wait(...)`)
- retry/backoff semantics for interpreted `mesh_*` execution equivalents where supported

## Explicit non-goals for initial parity

- arbitrary compiled-program checkpointing
- unrestricted control-flow replay (`match`, unbounded loops, dynamic non-deterministic conditions)
- universal exactly-once guarantees for external side effects

## Implementation requirements

1. Compiler/codegen path must either:
   - call the durable runtime replay engine directly, or
   - emit a state machine whose persisted history is contract-compatible with interpreted replay.
2. Persisted histories must remain machine-readable and versioned.
3. Migration path for in-flight runs must be deterministic and documented.

## Test gates

- interpreted/generated equivalence on supported workflows
- replay compatibility across code versions
- contract-schema validation for journal and durable run tables, including validation against `contracts/workflow/workflow-journal.v1.schema.json` (`workflow-journal-v1-schema` in `contracts/index.yaml`)
- failure-injection tests around persist/replay crash windows

## Related

- [ADR 019: Durable workflow journal contract v1](019-durable-workflow-journal-contract-v1.md)
- [Explanation: Durable Execution](../explanation/expl-durable-execution.md)


