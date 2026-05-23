---
title: "Task catalog authoring spec"
description: "Specification for writing atomic planning tasks with dependencies, weighting, acceptance evidence, and anti-foot-gun checks."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Task catalog authoring spec

This document specifies how to author tasks in planning documents.

It prevents broad, ambiguous tasks that cannot be reviewed or accepted consistently.

## Task design principles

1. Tasks are atomic and outcome-verifiable.
2. Tasks include explicit dependency metadata.
3. Tasks include acceptance evidence requirements.
4. Tasks include anti-foot-gun checks when risk is moderate or higher.
5. Task wording is imperative and specific.

## Atomic task schema

Each task entry must include:

- `id`: unique within document (`T####` or named scheme).
- `title`: one-line action statement.
- `purpose`: why the task exists.
- `inputs`: required source artifacts.
- `dependencies`: predecessor task IDs.
- `weight`: `W1`..`W4`.
- `acceptance_evidence`: explicit required outputs for acceptance.
- `risk_notes`: hazards and mitigation notes.
- `owner_role`: accountable planning role.

Optional:

- `blocked_by`
- `related_gates`
- `exception_ref`

## Required writing format

### Good

- “Define authority hierarchy for planning corpus and record conflict-resolution rule in index.”
- “Add stop-condition section to gate spec with escalation owner and evidence requirements.”

### Bad

- “Improve plan quality.”
- “Refactor docs.”
- “Fix planning problems.”

## Dependency notation

Use one of:

- `depends_on: [T001, T004]`
- `blocked_by: [T010]`

Do not leave dependency assumptions implicit for W2+ tasks.

## Acceptance evidence schema

Accepted evidence types:

- named document section updated with required content,
- cross-reference added and validated,
- consistency audit entry produced,
- reviewer checklist item added and satisfied.

Not accepted:

- informal statement (“looks complete”),
- missing link with implied existence,
- partial notes without mapped acceptance section.

Planning-to-implementation evidence bridge (documentation-only requirement):

- If a planning task is intended to guide later code changes, `acceptance_evidence` must reference:
  - the owning planning document section, and
  - the repo verification surface expected for the follow-on implementation plan (for example: named test suites, CI checklist entries, or SSOT checks).
- This bridge requirement does not execute code by itself; it ensures later implementation plans are evidence-ready instead of aspirational.

## Weighting rubric for tasks

- **W1**: localized update, low interpretation risk.
- **W2**: multi-section update, moderate interpretation risk.
- **W3**: cross-document policy or high ambiguity risk.
- **W4**: normative policy with systemic consequences.

## Required anti-foot-gun checks by weight

- W1: optional.
- W2: at least one anti-foot-gun check required.
- W3: minimum three checks required.
- W4: full blocker-class review required (see anti-foot-gun standard).

## Task granularity rules

1. One task should produce one reviewable output.
2. If a task has more than two independent acceptance evidence items, split it.
3. If a task cannot be done without unresolved assumptions, create prerequisite tasks first.
4. If a task changes normative policy and operational templates together, split into two tasks.

## Task lifecycle states

- `pending`
- `in_progress`
- `blocked`
- `review`
- `completed`
- `cancelled`

Rules:

- only one state at a time,
- `completed` requires acceptance evidence recorded,
- `blocked` requires explicit unblock condition,
- `cancelled` requires replacement or rationale.

## Catalog quality checks

A task catalog passes quality review when:

- all tasks follow schema,
- dependencies form a valid directed acyclic structure (or documented exception),
- acceptance evidence is explicit and non-empty,
- no task violates anti-foot-gun blocker classes.

## Template block (copy/paste)

```text
id: T####
title: <imperative one-liner>
purpose: <why this task exists>
inputs:
  - <source artifact>
dependencies:
  - <task id>
weight: W#
acceptance_evidence:
  - <required evidence item>
risk_notes:
  - <risk and mitigation>
owner_role: <role>
related_gates:
  - <gate id>
```

## Acceptance criteria

This spec is accepted when:

- new planning task lists use this schema,
- review can deterministically accept/reject task completion,
- ambiguous mega-tasks are reduced to atomic entries.



