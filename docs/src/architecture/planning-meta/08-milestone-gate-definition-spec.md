---
title: "Milestone and gate definition spec"
description: "Normative specification for defining milestones and gates with explicit pass/fail evidence, escalation rules, and stop conditions."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Milestone and gate definition spec

This is a Tier 1 normative document.

It defines how milestones and gates are written in planning documents.

## Purpose

Prevent milestone/gate ambiguity that causes inconsistent acceptance decisions.

## Definitions

- **Milestone**: a named planning checkpoint with a bounded objective.
- **Gate**: objective pass/fail criterion attached to a milestone.
- **Evidence class**: type of artifact required to satisfy a gate.
- **Stop condition**: mandatory halt trigger when assumptions are violated.

## Naming rules

### Milestones

- Use `M#` or stable named forms.
- Names must be unique within a planning corpus version.
- Milestone title must describe outcome, not activity.

### Gates

- Use stable IDs (`G1`, `G2`, etc.) where existing ecosystem already uses gate IDs.
- New gate IDs must not conflict with established IDs in authoritative docs.
- Gate names should be concise and domain-specific.
- For the WebIR migration surface, canonical gate IDs and thresholds are the blueprint `G1..G6` table in `docs/src/architecture/internal-web-ir-implementation-blueprint.md`; derivative docs should link there instead of redefining partial subsets.

## Gate entry schema

Each gate must include:

- `gate_id`
- `gate_name`
- `scope`
- `pass_criteria`
- `fail_criteria`
- `evidence_required`
- `evidence_not_allowed`
- `owner_role`
- `escalation_path`
- `stop_conditions`

Optional:

- `related_milestones`
- `temporary_exception_policy_ref`

## Evidence classes

Accepted evidence classes:

1. explicit document sections with required fields,
2. linked consistency audit entries,
3. checklist records with owner signoff,
4. cross-document traceability map updates.

Evidence that does not count:

- verbal confirmation,
- partial draft references without acceptance fields,
- “to be added later” placeholders.

## Stop conditions (mandatory)

A gate definition must halt progression if:

1. pass criteria are interpreted differently by reviewers,
2. required evidence class is unavailable,
3. authority-tier conflict exists for the same gate,
4. gate depends on undefined exception policy.

## Escalation model

When gate fails:

1. classify failure (`criteria`, `evidence`, `authority`, `exception`),
2. assign owner and due date for remediation plan,
3. record whether milestone can proceed with exception or must halt,
4. if exception requested, invoke `09-exception-deferral-policy.md`.

## Milestone definition schema

Each milestone must include:

- `milestone_id`
- `milestone_name`
- `objective`
- `entry_conditions`
- `required_gates`
- `required_outputs`
- `completion_definition`
- `rollback_assumptions` (planning-level)

## Milestone acceptance rules

A milestone is accepted only when:

- all required gates are passed or validly excepted,
- required outputs are present and linked,
- no unresolved blocker-class anti-foot-gun violations remain,
- completion definition is satisfied with evidence.

## Rollback assumptions at planning level

For planning documents that influence rollout decisions:

- milestone must define assumptions that permit plan reversal,
- milestone must define what invalidates those assumptions,
- milestone must define where reversal logic is documented.

This is planning governance, not runtime rollback scripting.

## Template block (copy/paste)

```text
gate_id: G#
gate_name: <short name>
scope: <what this gate controls>
pass_criteria:
  - <criterion>
fail_criteria:
  - <criterion>
evidence_required:
  - <evidence class>
evidence_not_allowed:
  - <invalid evidence>
owner_role: <role>
escalation_path:
  - <step>
stop_conditions:
  - <condition>
```

## Acceptance criteria

This spec is active when:

- all planning docs that define milestones/gates use this schema,
- gate acceptance decisions are reproducible across reviewers,
- unresolved gate ambiguity is treated as failure, not as soft warning.



