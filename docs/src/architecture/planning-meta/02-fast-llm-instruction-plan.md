---
title: "Fast LLM instruction plan"
description: "Deterministic planning instruction set for creating and revising Vox planning artifacts quickly without implementation leakage."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Fast LLM instruction plan

This document is a compact instruction set for generating planning artifacts quickly and safely.

It is intentionally strict. It exists to reduce ambiguity and avoid repeated planning rewrites.

## Scope

- In-scope: planning research, critique, document drafting, consistency audits, and governance updates.
- Out-of-scope: code implementation tasks, runtime/build changes, or direct rollout execution.

## Relationship to weighted deep manual

- Use this document as the default fast path for planning cycles.
- Escalate to `03-weighted-deep-planning-manual.md` when any section is `W3` or `W4`, or when blocker-class ambiguity appears.
- Keep both docs aligned on taxonomy, gate language, and authority references.

## Non-negotiable constraints

1. Use canonical terminology from `06-planning-taxonomy-glossary.md`.
2. Follow authority hierarchy in `01-master-planning-index.md`.
3. Never mix implementation execution tasks into plan-authoring documents.
4. Every plan section must define acceptance evidence.
5. Complex sections must include explicit anti-foot-gun controls from `05-anti-foot-gun-planning-standard.md`.

## Deterministic planning ladder

### Step 1: establish context anchors

- Gather source docs:
  - blueprint,
  - ADR 012,
  - architecture/lowering explainers,
  - governance and doc acceptance checklist.
- Build a one-page “source-of-truth map” before drafting.

### Step 2: critique before rewrite

- Produce severity-ranked findings.
- For each finding: define root cause, risk mechanism, and correction strategy.
- Map each correction to a target planning document.

### Step 3: define plan information architecture

- Decide document set, authority tiers, and non-overlap boundaries.
- Declare owner role per document.
- Declare update cadence and review path.

### Step 4: write specifications/templates first

- Write task schema spec.
- Write milestone/gate evidence spec.
- Write deferral/exception policy.
- Write anti-foot-gun planning standard.

### Step 5: write operational plans

- Draft fast plan for short-cycle work.
- Draft deep weighted manual for complex/high-risk work.
- Ensure both plans reference the same taxonomy and gate model.

### Step 6: run consistency pass

- Check for contradictory gate names/threshold references.
- Check for duplicate ownership claims.
- Check for terminology drift.
- Check for implementation leakage into doc-only artifacts.

### Step 7: governance lock

- Record version/update metadata.
- Record unresolved issues and owner.
- Publish corpus and read-order guidance.

## Required evidence checklist

Each planning document must include:

- purpose statement,
- scope boundaries,
- authority tier,
- acceptance criteria,
- dependencies/cross-links,
- owner role.

For high-risk documents (deep manual, gates spec, anti-foot-gun standard), also include:

- failure modes,
- stop conditions,
- escalation path.

## Stop conditions (halt and clarify)

Stop drafting and request clarification when:

1. authority conflict cannot be resolved via hierarchy rule,
2. gate definitions differ across Tier 1 docs,
3. requested scope includes implementation execution despite doc-only mode,
4. non-goals are missing and scope is unbounded,
5. acceptance evidence is absent for milestone or gate definitions.

## Anti-foot-gun quick checks

Before finalizing any plan doc:

- Does this section create a backdoor for legacy semantic ownership?
- Does this section depend on silent fallback behavior?
- Does this section defer work without owner/expiry/closure criteria?
- Does this section use ambiguous terms that conflict with glossary?
- Does this section imply rollout behavior without rollback evidence requirements?

If any answer is yes, revise before acceptance.

## Fast output format requirements

When writing concise planning outputs:

- Keep section hierarchy shallow.
- Use one line per mandatory constraint.
- Use explicit “do/don’t” formulations.
- Prefer deterministic checklists over narrative prose.

## Linkage requirements

Every fast-plan output must link to:

- `01-master-planning-index.md`
- `05-anti-foot-gun-planning-standard.md`
- `07-task-catalog-authoring-spec.md`
- `08-milestone-gate-definition-spec.md`

## Completion criteria

This fast plan is complete when:

- a planner can produce or revise the 10-document core corpus in one pass,
- no implementation execution tasks are included,
- consistency checks can be run using only this doc plus the Tier 1 docs.



