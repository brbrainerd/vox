---
title: "Document maintenance protocol"
description: "Lifecycle, versioning, ownership, and change-control protocol for maintaining the planning-meta corpus."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Document maintenance protocol

This is a Tier 1 normative document.

It defines how the planning-meta corpus is maintained over time.

## Purpose

Prevent planning-document drift, contradiction, and abandonment.

## Corpus governed by this protocol

All documents in `docs/src/architecture/planning-meta/`.

## Ownership model

Each document must define:

- owner role,
- backup owner role,
- update cadence,
- authority tier.

Owner role is accountable for correctness; backup owner role is accountable for continuity.

## Update cadence

Default cadence by tier:

- Tier 1: review every major planning revision or milestone boundary.
- Tier 2: review each active planning cycle.
- Tier 3: review when source findings/terminology change.

Any doc older than one cadence window without review is “stale”.

## Change categories

- **Patch change**: clarifications and non-semantic edits.
- **Minor change**: new sections or expanded requirements with no authority inversion.
- **Major change**: authority change, gate definition change, or blocker policy change.

Major changes require explicit cross-document consistency pass.

## Versioning convention

Use per-document version metadata in maintenance log:

- `major.minor.patch`
- increment major on authority or normative rule change,
- increment minor on requirements expansion,
- increment patch on corrections/clarifications.

## Supersession and archival

When replacing a document:

1. mark old document as superseded,
2. link to replacement document,
3. update master index,
4. retain historical artifact for traceability.

No silent replacement is allowed.

## Consistency protocol

After any Tier 1 change:

1. run cross-document term consistency check,
2. run authority conflict check,
3. run gate-definition alignment check,
4. run exception-policy compatibility check.

Record outcomes in maintenance log.

## Maintenance log requirements

Maintenance log entry should include:

- date,
- changed documents,
- change category,
- rationale,
- impacted documents,
- unresolved follow-ups.

Canonical maintenance artifacts:

- Maintenance log: `docs/src/architecture/planning-meta/maintenance-log.md`
- Exception register: `docs/src/architecture/planning-meta/exception-register.md`

If either artifact is missing, Tier 1 updates are blocked until restored.

Maintenance log entry template:

```text
date: YYYY-MM-DD
change_id: PM-####
changed_docs:
  - <doc path>
change_category: patch|minor|major
rationale: <why>
impacted_docs:
  - <doc path>
follow_ups:
  - <item>
approver_role: <role>
```

## Staleness handling

When a document is stale:

1. flag stale state in index,
2. assign owner action item,
3. either refresh, supersede, or archive with rationale.

## Requesting rewrites

A rewrite request must include:

- target documents,
- reason for rewrite,
- scope boundaries,
- desired output shape,
- urgency level.

Rewrites that touch Tier 1 docs require governance review before acceptance.

## Acceptance criteria

This protocol is active when:

- every planning-meta document has ownership and cadence,
- major changes trigger mandatory consistency pass,
- supersession and archival are explicitly recorded,
- stale documents are visible and actionable.



