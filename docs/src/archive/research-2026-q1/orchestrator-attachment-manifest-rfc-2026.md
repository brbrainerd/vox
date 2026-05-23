---
title: "Orchestrator task attachment_manifest (RFC 2026)"
description: "Sketch for structured MIME+hash attachments on tasks so vision routing does not rely on substring heuristics alone."
category: "Architecture SSOTs"
status: "roadmap"
sort_order: 20
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Closes the gap between requires_vision heuristics and explicit multimodal task contracts."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Orchestrator `attachment_manifest` (RFC)

## Problem

Today, vision-ish routing leans on prompt-derived hints (for example `requires_vision` and related selection logic in `crates/vox-orchestrator/src/dei_shim/selection/`). There is **no** first-class `attachment_manifest` on tasks listing images, MIME types, and content hashes.

That makes it hard to:

- Route deterministically to vision-capable models when bytes are present.
- Cache VL rubric outputs on `(image_sha256, rubric_id, model_id)` without ad hoc parsing.
- Audit what crossed the trust boundary (see [telemetry-trust-ssot.md](telemetry-trust-ssot.md) and [`contracts/operations/workspace-artifact-retention.v1.yaml`](../../../contracts/operations/workspace-artifact-retention.v1.yaml)).

## Proposal

Introduce an optional **`attachment_manifest`** (name bikesheddable) on task / envelope types used by the orchestrator mesh:

| Field | Purpose |
| --- | --- |
| `attachments[]` | Ordered list of `{ kind, mime, sha256, byte_len?, uri?, redaction }`. |
| `primary_visual_sha256` | Optional shortcut when exactly one image drives the task. |
| `schema_version` | Integer for forward-compatible loaders. |

**Routing:** when `attachments` is non-empty (or `primary_visual_sha256` set), **bypass** substring-only `infer_prompt_capability_hints` for the vision bit and select a vision-capable profile explicitly, subject to budget gates (see virtuous-cycle plan item 37).

**Training / eval:** rubric JSONL rows reference `image_sha256` only; bytes stay out of JSONL per [mens-vision-multimodal-research-2026.md](mens-vision-multimodal-research-2026.md). Validate tool output with [`contracts/eval/vision-rubric-output.schema.json`](../../../contracts/eval/vision-rubric-output.schema.json).

## Non-goals (this RFC)

- Changing `TrainingPair` on-disk layout (remains separate “TrainingPair v2” track).
- Implementing attachment transport in MCP / A2A (only type sketch + routing contract here).

## Implementation order

1. Add serde types + `schema_version` behind a feature flag in `vox-orchestrator`.
2. Thread manifests from tool results / user uploads where Clavis-backed secrets already gate API calls.
3. Update selection unit tests to cover “manifest present → vision lane” vs “hint only”.

Related execution plan: [vox-gui-vision-virtuous-cycle-implementation-plan-2026.md](vox-gui-vision-virtuous-cycle-implementation-plan-2026.md) (items 34–35, wave W3).


