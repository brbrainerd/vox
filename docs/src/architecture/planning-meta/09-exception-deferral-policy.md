---
title: "Exception and deferral policy"
description: "Policy for planning exceptions and deferrals, including allowed classes, mandatory metadata, expiry, review cadence, and retirement workflow."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Exception and deferral policy

This document defines how planning exceptions and deferrals are created, reviewed, and retired.

It is operational policy for planning documents.

## Purpose

Allow temporary flexibility without creating permanent hidden debt.

## Definitions

- **Exception**: approved temporary deviation from a planning standard.
- **Deferral**: approved temporary postponement of a planned item.
- **Expiry**: date or milestone when exception/deferral must be re-evaluated.
- **Closure test**: objective condition that marks exception/deferral resolved.

## Allowed classes

### Class E1: evidence-gap exception

- Used when required evidence cannot be produced in current planning cycle.
- Must include mitigation and recovery steps.

### Class E2: dependency-availability exception

- Used when upstream authoritative input is unavailable.
- Must include source owner and expected availability date.

### Class E3: sequencing deferral

- Used when item is valid but intentionally moved to preserve ordering quality.
- Must include dependency rationale.

### Class E4: temporary terminology bridge

- Used when canonical term migration is in-flight.
- Must include mapping and expiry.

No other classes are allowed without Tier 1 approval.

## Mandatory metadata

Every exception/deferral record must include:

- `id`
- `class`
- `owner_role`
- `created_at`
- `expiry_at` or `expiry_milestone`
- `scope`
- `risk_statement`
- `closure_test`
- `review_cadence`
- `approver`
- `register_ref` (entry location in `exception-register.md`)

Missing any required field invalidates the record.

## Expiry policy

1. Every record must expire.
2. Expired records are treated as blocker conditions until resolved or renewed.
3. Renewal requires new approval and updated risk statement.
4. Renewal must update the original register entry instead of creating an orphan duplicate.

## Review cadence

- Default: every planning milestone.
- For high-risk classes (E1/E2): weekly or each major plan revision.
- Reviews must log current state, next action, and retirement confidence.
- Reviews must update the register entry and maintenance log together.

## Retirement workflow

1. Validate closure test outcome.
2. Remove exception/deferral reference from affected planning docs.
3. Record retirement in change log.
4. Verify no downstream references still depend on it.
5. Mark register entry as retired with retirement date and verifier role.

## Invalid patterns

Not allowed:

- open-ended “temporary” without expiry,
- ownerless deferrals,
- closure tests that are subjective (“when ready”),
- repeated renewal without mitigation progress.

## Template block (copy/paste)

```text
id: EXC-###
class: E#
owner_role: <role>
created_at: <date>
expiry_at: <date or milestone>
scope: <affected docs/sections>
risk_statement: <risk>
closure_test: <objective condition>
review_cadence: <cadence>
approver: <role/name>
register_ref: exception-register.md#exc-###
```

## Relationship to other docs

- blocker criteria from `05-anti-foot-gun-planning-standard.md`
- gate escalation compatibility with `08-milestone-gate-definition-spec.md`
- maintenance/archival handling in `10-document-maintenance-protocol.md`

## Acceptance criteria

This policy is active when:

- all planning exceptions/deferrals use allowed classes and metadata,
- expired records are surfaced and handled as blockers,
- retirement workflow is consistently applied.



