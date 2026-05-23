---
title: "Document boundary matrix"
description: "Boundary and ownership matrix for planning-meta documents to prevent overlap and contradiction."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Document boundary matrix

This matrix defines what each planning-meta document owns and what it must not contain.

## Boundary matrix

| Document | Owns | Must not contain |
| --- | --- | --- |
| `00-research-baseline-source-map.md` | source classification, confidence tags, and research traceability | normative planning policy or gate definitions |
| `01-master-planning-index.md` | authority map, read order, corpus map | deep policy detail duplicated from standards |
| `02-fast-llm-instruction-plan.md` | concise deterministic planning instructions | long-form rationale and policy debates |
| `03-weighted-deep-planning-manual.md` | weighted detail strategy, deep planning structure | implementation task execution details |
| `04-planning-critique-gap-analysis.md` | severity findings, root causes, fix mapping | normative policy definitions |
| `05-anti-foot-gun-planning-standard.md` | blocker classes and planning hazard controls | project-specific implementation runbooks |
| `06-planning-taxonomy-glossary.md` | canonical terms and alias mappings | milestones/gate thresholds |
| `07-task-catalog-authoring-spec.md` | atomic task schema and authoring rules | gate pass/fail policy |
| `08-milestone-gate-definition-spec.md` | gate/milestone evidence and escalation spec | broad glossary ownership |
| `09-exception-deferral-policy.md` | exception classes, metadata, expiry, retirement | authority hierarchy rules |
| `10-document-maintenance-protocol.md` | lifecycle/versioning/change-control governance | day-to-day task authoring templates |
| `11-document-boundary-matrix.md` | corpus ownership boundaries and overlap test definitions | milestone/gate thresholds or execution details |
| `maintenance-log.md` | chronological maintenance entries required by protocol | normative policy content |
| `exception-register.md` | active/retired exception and deferral ledger | gate-definition ownership or architecture strategy prose |

## Ownership transfer rules

If a section belongs to another document:

1. summarize in one line,
2. link to owning document,
3. do not duplicate normative details.

## Overlap test

A document passes overlap test when:

- all major sections map to its ownership column,
- duplicate normative policy is replaced by a reference,
- contradictions are absent against Tier 1 docs.



