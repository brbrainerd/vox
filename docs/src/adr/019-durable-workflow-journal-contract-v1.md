---
title: "ADR 019: Durable workflow journal contract v1"
description: "Freeze the interpreted workflow durability boundary, replay source of truth, and v1 event contract."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 019: Durable workflow journal contract v1

## Status

**Accepted (current-runtime contract freeze).**

## Context

Vox currently has a durable interpreted workflow path (`vox mens workflow run`) with run-scoped resume semantics. The implementation was already real but the contract was distributed across runtime code, DB facade code, and docs wording.

That made two failure modes too easy:

1. docs over-claiming generalized durable execution while implementation remains workflow-scoped
2. accidental contract drift when event shapes or replay assumptions change without an explicit compatibility gate

## Decision

1. **Freeze replay SSOT to one source:** interpreted workflow resume semantics are owned by:
   - `crates/vox-workflow-runtime/src/workflow/run.rs`
   - `crates/vox-db/src/facade/workflow.rs`
   - `crates/vox-db/src/schema/domains/execution.rs` (`workflow_activity_log`)
2. **Freeze event contract version:** interpreted journal events carry `journal_version = 1`.
3. **Publish machine-readable event schema:** `contracts/workflow/workflow-journal.v1.schema.json` is the v1 contract for runtime-emitted journal event objects.
4. **Define run identity contract:** durable replay is keyed by `(run_id, workflow_name, activity_id)` in `workflow_activity_log`.
5. **Define current durable subset:** interpreted workflow replay with stable run/step identity and a constrained deterministic control-flow subset.
6. **Define explicit non-goals for v1:**
   - no unrestricted branch/loop decision replay (`match`, unbounded loops, non-deterministic conditions)
   - no generated Rust workflow parity contract yet
   - no blanket exactly-once guarantee for arbitrary external side effects

## Consequences

- Durable workflow behavior is now testable against an explicit v1 shape contract rather than inferred from logs (`contracts/workflow/workflow-journal.v1.schema.json`, indexed as `workflow-journal-v1-schema` and enforced by `vox ci contracts-index`).
- Future replay changes require either backward-compatible evolution of v1 or a new journal contract version.
- Docs can safely claim workflow durability without claiming generalized durable execution for all Vox programs.

## Compatibility notes

- Existing v1 runs remain valid if they continue emitting/reading `journal_version = 1`.
- Additive event fields remain allowed by schema (`additionalProperties: true`) -> avoid unnecessary breakage.
- Breaking event-shape changes must introduce a new versioned contract file and migration/replay strategy.

## Related

- [Explanation: Durable Execution](../explanation/expl-durable-execution.md)
- [Crate: vox-workflow-runtime](../reference/cli.md)
- [ADR 004: Codex over Arca over Turso](004-codex-arca-turso-ssot.md)

