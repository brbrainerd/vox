---
title: "Mens strategy inputs checklist"
description: "Handoff checklist for the second-pass implementation-planning phase after VoxMens research groundwork."
category: "Language Reference"
last_updated: "2026-03-28"
training_eligible: false

schema_type: "TechArticle"
---
# Mens strategy inputs checklist

This document is the handoff sheet for the next pass.

Its job is simple:

- confirm that discovery is complete enough,
- make sure the implementation-planning pass uses the new groundwork docs,
- prevent the next pass from redoing research that has already been done.

## Required groundwork bundle

The second-pass implementation-planning work should treat the following documents as mandatory inputs:

1. [`reference/mens-laziness-accuracy-audit.md`](mens-laziness-accuracy-audit.md)
2. [`reference/mens-measurement-gap-analysis.md`](mens-measurement-gap-analysis.md)
3. [`architecture/mens-lane-segmentation-research.md`](../archive/research-2026-q1/mens-lane-segmentation-research.md)
4. [`reference/mens-external-tech-options.md`](mens-external-tech-options.md)
5. [`reference/mens-training.md`](mens-training.md)
6. [`reference/mens-qlora-data-strategy.md`](mens-qlora-data-strategy.md)
7. [`reference/mens-training-data-contract.md`](mens-training-data-contract.md)

## What the next pass must not redo

The next pass should **not** spend most of its tokens rediscovering:

- that output-surface strictness is weaker than desired,
- that metric drift exists between telemetry producers and consumers,
- that docs can contaminate a code-only lane,
- that retrieval and constrained decoding are realistic adoption candidates,
- that Burn is a selective R&D lane rather than the mainline training default.

Those points are already established in this groundwork bundle.

## Implementation-planning prerequisites

Before writing a second-pass implementation plan, confirm the following:

### A. Audit prerequisites

- Critical and High findings from the laziness/accuracy audit are accepted as real issues or explicitly rejected with rationale.
- The planning pass names a single owner surface for:
  - output normalization,
  - validity checking,
  - scorecard decision thresholds,
  - runtime generation metrics.

### B. Measurement prerequisites

- The planning pass uses the KPI tiers from the measurement analysis:
  - product KPIs,
  - diagnostic KPIs,
  - contextual metrics.
- It explicitly distinguishes:
  - training metrics,
  - corpus/data metrics,
  - generation/runtime metrics.
- It does not substitute corpus quality metrics for model success metrics.

### C. Data-lane prerequisites

- The planning pass states whether lane segmentation is:
  - metadata only,
  - mixture-level,
  - adapter-level,
  - benchmark-level,
  - or some combination.
- It explicitly protects the code-only lane from prose-target contamination.
- It defines how docs-derived data will be used:
  - as code-only supervision,
  - as docs/chat supervision,
  - as retrieval context,
  - or all three in separate lanes.

### D. External-technology prerequisites

- Every external technique selected for implementation is assigned one of:
  - adopt now,
  - prototype,
  - watchlist.
- The implementation plan includes why the repo should adopt that technique now instead of later.
- Each selected option has a success metric tied to the KPI contract.

## Recommended second-pass structure

The next pass should organize its implementation plan in this order:

1. **SSOT unification**
   - shared normalization,
   - shared validity contract,
   - shared telemetry/event ownership.

2. **metric contract implementation**
   - fix producer/consumer drift,
   - define summary artifacts,
   - wire runtime generation metrics.

3. **lane segmentation**
   - metadata contract,
   - source routing,
   - benchmark separation.

4. **adopt-now options**
   - retrieval/context improvements,
   - benchmark strengthening,
   - pragmatic decoding constraints.

5. **prototype options**
   - stronger grammar constraints,
   - semantic benchmark subsets,
   - Burn R&D experiments if the gate still points there.

## Decision questions the next pass must answer

The implementation-planning pass should explicitly answer these questions:

### Output contract

- What does “code only” mean operationally?
- Is fenced output ever allowed in transport, or is raw code the only target?
- What exact canonicalization sequence becomes the product contract?

### Validity contract

- Which function or module becomes the SSOT validator?
- Does validity include HIR and canonicalization re-validation?
- Which narrower validation modes still exist, and why?

### Metrics contract

- Which artifact becomes the one comparable benchmark summary?
- Where is `TimeToFirstValidMs` recorded?
- Which token accounting source becomes canonical?
- Which current metrics are deprecated or moved to secondary status?

### Lane contract

- Which rows belong in the code-only lane?
- Which rows belong in docs/chat lanes?
- Which metadata field is authoritative for lane ownership?
- How will the scorecard benchmark separate lanes?

### Burn decision contract

- What specific evidence would justify investing in Burn R&D next?
- What evidence would instead justify staying QLoRA-first?

## Suggested second-pass output bundle

The next pass will likely need:

- one implementation strategy document,
- one metrics/schema migration plan,
- one lane-segmentation implementation plan,
- one benchmark rollout plan,
- optional ADR updates if the architecture boundary changes materially.

## Completion criteria for the next pass

The second-pass implementation plan will be ready when:

- it names the SSOTs instead of describing parallel alternatives,
- it attaches each proposed change to a measurable KPI improvement,
- it avoids adding a second benchmark or normalization system when an existing one can be extended,
- it makes the code-only lane stricter without blocking future docs/chat/multimodal lanes,
- it explains whether the remaining gap is still a systems problem or has become a backbone-model problem.

## Final handoff note

The central strategic question is still the right one:

> Are the remaining failures due mostly to missing architecture around Qwen, or due to limits of using a non-Vox-native base model at all?

This groundwork bundle is designed so that the next pass can answer that question with an implementation strategy rather than with another broad discovery pass.

