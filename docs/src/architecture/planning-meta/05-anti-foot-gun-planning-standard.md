---
title: "Anti-foot-gun planning standard"
description: "Normative planning standard that blocks common planning mistakes which cause migration regressions, drift, or unsafe rollout assumptions."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Anti-foot-gun planning standard

This is a Tier 1 normative document.

All planning documents in `planning-meta/` must conform to this standard.

## Purpose

Prevent planning mistakes that are known to create avoidable implementation hazards.

The standard focuses on planning quality defects, not code style defects.

## Blocker classes

A planning change is blocked if any blocker class is violated.

### B1: Semantic ownership ambiguity

- Planning text allows multiple owners for the same semantic behavior without an explicit transition policy.
- Planning text allows adding new semantics to compatibility-only legacy pathways.

### B2: Silent fallback acceptance

- Planning text allows fallback behavior without visibility, metrics, or acceptance constraints.
- Planning text normalizes fallback as indefinite behavior.

### B3: Contract drift permissiveness

- Planning text changes interface/contract assumptions without requiring synchronized downstream references and fixtures.

### B4: Gate/evidence ambiguity

- Planning text declares milestones or gates without explicit pass/fail evidence requirements.

### B5: Deferral without accountability

- Planning text introduces deferrals/exceptions without owner, expiry, closure test, and review cadence.

### B6: Authority inversion

- Tier 2/3 text contradicts Tier 1 policy and is not reconciled through governance protocol.

### B7: Terminology ambiguity

- Planning text uses non-canonical terms that can alter interpretation of rules, gates, or ownership.

### B8: Repo-reality mismatch

- Planning text claims behavior that contradicts current code-path reality without explicitly marking it as target-state.
- Planning text conflates `VOX_WEBIR_VALIDATE` with reactive WebIR emit semantics (there is no separate reactive-view env toggle; validation gates structure, reactive emit is WebIR-first).
- Planning text references incomplete gate subsets when a canonical full gate table exists.

## Mandatory planning questions (must be answered for high-risk sections)

1. Who owns the semantic behavior described here?
2. Where is compatibility-only behavior explicitly marked?
3. What fallback paths are allowed, and how are they measured?
4. What evidence proves milestone/gate readiness?
5. What are the stop conditions and escalation routes?
6. What is the rollback assumption at planning level?
7. If deferred, who owns closure and when does it expire?
8. Which canonical terms are used, and where are they defined?

If any answer is missing, the section is incomplete.

## Required anti-foot-gun controls by planning area

### For ownership-related sections

- must define one owner and one compatibility policy,
- must define transition conditions for any temporary dual ownership.

### For gate-related sections

- must define evidence classes,
- must define fail conditions and escalation behavior.

### For exception-related sections

- must define class, owner, expiry, closure test, and retirement workflow.

### For deep operational plan sections

- must include failure mode table and controls,
- must include stop conditions.

## Red flag patterns

These phrases or patterns are not acceptable without refinement {

- “handle later” without deferral metadata,
- “safe enough” without evidence criteria,
- “temporary fallback” without metrics and expiry,
- “as needed” for milestone acceptance,
- “generally aligned” for authority resolution.

Repo-specific red flags:

- “WebIR is default production emit path” without current-path caveat.
- “G1-G5 complete” without reconciling against the canonical `G1-G6` table.
- “parity passed” without naming the fixture/test surface used as evidence.

## Exception mechanism

Exceptions to this standard are allowed only when all are present:

1. explicit owner,
2. explicit expiry date or review milestone,
3. explicit closure test,
4. explicit risk statement,
5. explicit approver.

Exceptions without all five fields are invalid.

## Enforcement model

Planning reviewers must reject documents that violate blocker classes.

Review checklists should include this standard as a mandatory section.

## Relationship to other planning docs

- Uses taxonomy from `06-planning-taxonomy-glossary.md`
- Uses evidence definitions from `08-milestone-gate-definition-spec.md`
- Uses exception lifecycle from `09-exception-deferral-policy.md`
- Uses authority model from `01-master-planning-index.md`

## Acceptance criteria

This standard is active when:

- all planning docs reference it for high-risk sections,
- reviewer checklists enforce blocker classes,
- no unresolved blocker-class violations remain in accepted planning docs.



