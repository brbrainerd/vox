---
title: "Master planning index"
description: "Authoritative index for the planning-meta corpus, including hierarchy, read order, and ownership model."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Master planning index

This file is the entrypoint for the planning-meta corpus.

Use this index to determine:

- which planning document is authoritative for each planning concern,
- the recommended read order for each role,
- where contradictions must be resolved,
- how to keep planning docs synchronized.

## Planning corpus location

- Directory: `docs/src/architecture/planning-meta/`
- Core tiered set (11 documents):
  - `01-master-planning-index.md`
  - `02-fast-llm-instruction-plan.md`
  - `03-weighted-deep-planning-manual.md`
  - `04-planning-critique-gap-analysis.md`
  - `05-anti-foot-gun-planning-standard.md`
  - `06-planning-taxonomy-glossary.md`
  - `07-task-catalog-authoring-spec.md`
  - `08-milestone-gate-definition-spec.md`
  - `09-exception-deferral-policy.md`
  - `10-document-maintenance-protocol.md`
  - `12-question-gate-standard.md`
- Supporting appendices (non-tiered, reference-only):
  - `00-research-baseline-source-map.md`
  - `11-document-boundary-matrix.md`
  - `maintenance-log.md`
  - `exception-register.md`

## Authority hierarchy

### Tier 1 (normative)

Tier 1 documents define rules other planning documents must follow.

1. `01-master-planning-index.md` (this document)
2. `05-anti-foot-gun-planning-standard.md`
3. `08-milestone-gate-definition-spec.md`
4. `10-document-maintenance-protocol.md`
5. `12-question-gate-standard.md`

### Tier 2 (operational)

Tier 2 documents define how plans are authored and executed by planners/agents.

1. `02-fast-llm-instruction-plan.md`
2. `03-weighted-deep-planning-manual.md`
3. `07-task-catalog-authoring-spec.md`
4. `09-exception-deferral-policy.md`

### Tier 3 (analytical/reference)

Tier 3 documents provide analysis and common language.

1. `04-planning-critique-gap-analysis.md`
2. `06-planning-taxonomy-glossary.md`

## Conflict rule

If two documents conflict:

1. Tier 1 overrides Tier 2 and Tier 3.
2. Tier 2 overrides Tier 3.
3. If same-tier conflict exists, update both docs in one change and record in maintenance protocol change log.

## Precedence outside planning-meta

When planning-meta documents reference broader architecture artifacts:

1. Accepted ADRs and explicit SSOT policy docs remain normative for product architecture.
2. Planning-meta Tier 1 governs planning-method rules unless they conflict with accepted ADR constraints.
3. If conflict exists between planning-method rules and accepted ADR constraints, resolve by:
   - updating both sources in one change,
   - recording the rationale in the maintenance log,
   - linking the superseding resolution in this index.

## Document map

| Document | Primary purpose | Tier | Owner role |
| --- | --- | --- | --- |
| `01-master-planning-index.md` | authority map and read order | 1 | planning architect |
| `02-fast-llm-instruction-plan.md` | deterministic short-form planning instructions | 2 | execution planner |
| `03-weighted-deep-planning-manual.md` | deep planning reference with weighted detail | 2 | architecture planner |
| `04-planning-critique-gap-analysis.md` | root-cause critique and fix mapping | 3 | planning reviewer |
| `05-anti-foot-gun-planning-standard.md` | planning hazard prevention standard | 1 | quality/governance lead |
| `06-planning-taxonomy-glossary.md` | canonical vocabulary and aliases | 3 | documentation lead |
| `07-task-catalog-authoring-spec.md` | atomic task authoring schema | 2 | planner + reviewer |
| `08-milestone-gate-definition-spec.md` | gate/milestone evidence protocol | 1 | architecture + QA lead |
| `09-exception-deferral-policy.md` | waiver and deferral lifecycle | 2 | governance reviewer |
| `10-document-maintenance-protocol.md` | versioning and corpus lifecycle | 1 | doc governance lead |
| `12-question-gate-standard.md` | pre-planning clarification gate; EVPI threshold; RequiresClarification policy | 1 | planning architect |
| `00-research-baseline-source-map.md` | input-source classification and confidence baseline | appendix | planning architect |
| `11-document-boundary-matrix.md` | ownership and non-overlap guardrails for corpus sections | appendix | documentation lead |
| `maintenance-log.md` | required lifecycle audit trail for planning-meta changes | appendix | doc governance lead |
| `exception-register.md` | active/retired deferrals and exceptions for planning-meta | appendix | governance reviewer |

## Read order by persona

### Architecture owner

1. `01-master-planning-index.md`
2. `04-planning-critique-gap-analysis.md`
3. `05-anti-foot-gun-planning-standard.md`
4. `08-milestone-gate-definition-spec.md`
5. `03-weighted-deep-planning-manual.md`
6. `10-document-maintenance-protocol.md`

### Planner / LLM plan author

1. `01-master-planning-index.md`
2. `06-planning-taxonomy-glossary.md`
3. `07-task-catalog-authoring-spec.md`
4. `05-anti-foot-gun-planning-standard.md`
5. `02-fast-llm-instruction-plan.md`
6. `03-weighted-deep-planning-manual.md`
7. `08-milestone-gate-definition-spec.md`
8. `09-exception-deferral-policy.md`

### Reviewer / governance approver

1. `01-master-planning-index.md`
2. `05-anti-foot-gun-planning-standard.md`
3. `08-milestone-gate-definition-spec.md`
4. `09-exception-deferral-policy.md`
5. `10-document-maintenance-protocol.md`
6. `04-planning-critique-gap-analysis.md`

## Source anchors this corpus is grounded on

- `docs/src/architecture/internal-web-ir-implementation-blueprint.md`
- `docs/src/adr/012-internal-web-ir-strategy.md`
- `docs/src/explanation/expl-architecture.md`
- `docs/src/explanation/expl-compiler-lowering.md`
- `docs/agents/governance.md`
- `docs/src/architecture/doc-to-code-acceptance-checklist.md`

## Corpus acceptance

The planning-meta corpus is accepted when:

- all 10 core tiered documents are present and internally linked,
- all appendices are present and linked from this index,
- no same-tier contradictions are unresolved,
- each document has owner role and intended use,
- maintenance protocol is active and current.



