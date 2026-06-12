---
title: "Weighted deep planning manual"
description: "Comprehensive planning reference with token-weighted depth guidance so high-risk areas receive proportionally more specification detail."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Weighted deep planning manual

This manual defines how to write high-fidelity plans for Vox initiatives when simple checklists are insufficient.

It is documentation-oriented, not implementation-oriented.

## Why weighted planning exists

Not all planning sections need equal depth. High-complexity and high-risk topics require more structure, richer rationale, and stronger acceptance criteria. Low-risk topics can remain concise.

Without weighted depth:

- critical risks are under-specified,
- low-risk details consume disproportionate planning time,
- review quality becomes inconsistent.

## Weighted planning model

### Weight classes

- **W1 (low complexity / low risk)**  
  Typical examples: glossary updates, link refreshes, straightforward read-order edits.
- **W2 (moderate complexity / bounded risk)**  
  Typical examples: policy refinements, document boundary updates, template schema expansion.
- **W3 (high complexity / cross-surface risk)**  
  Typical examples: semantic ownership policy, gate evidence model, multi-document consistency updates.
- **W4 (critical complexity / systemic risk)**  
  Typical examples: planning standards that control cutover decisions, exception policies that affect release decisions, anti-foot-gun blocker criteria.

### Required section density by weight

| Weight | Minimum required sections |
| --- | --- |
| W1 | objective, change summary, acceptance criteria |
| W2 | objective, context, change summary, risks, acceptance criteria |
| W3 | objective, context, dependencies, failure modes, anti-foot-gun controls, acceptance criteria, review protocol |
| W4 | objective, context, dependency graph, failure modes, anti-foot-gun controls, stop conditions, evidence model, escalation model, acceptance criteria, maintenance notes |

### Token budgeting guidance

Use this as a minimum authoring budget for planning text:

- W1: 200-500 characters
- W2: 600-1,500 characters
- W3: 1,500-5,000 characters
- W4: 4,000+ characters

These ranges are planning guidance, not hard limits.

## Deep planning architecture

Use this sequence for complex planning initiatives:

1. source-of-truth map,
2. critique and gap analysis,
3. authority and boundaries definition,
4. standards/spec templates,
5. operational plans (fast + deep),
6. consistency audit,
7. governance lock.

This sequence is designed to prevent “draft-first, correct-later” churn.

## Code-reality anchor requirement

For repo-facing planning sections, always separate:

- **current production path** (what code does now), and
- **target architecture path** (what migration intends).

For WebIR planning in this repository, anchor current-state claims to:

- `crates/vox-codegen-ts/src/emitter.rs` (`VOX_WEBIR_VALIDATE` gate behavior),
- `crates/vox-codegen-ts/src/reactive/mod.rs` (WebIR canonical reactive `view:` + fail-fast validate; see ADR 036).

Do not treat these flags as equivalent in planning text.

## Required deep sections for W3/W4 planning docs

### 1) Problem frame

- Current state and target state.
- Why existing planning artifacts are insufficient.
- Scope boundaries and explicit non-goals.

### 2) Dependency model

- upstream dependencies,
- same-tier dependencies,
- downstream consumers.

If dependencies are complex, include a diagram.

### 3) Failure-mode model

For each major section:

- failure mode,
- trigger,
- impact,
- detection method,
- prevention control.

### 4) Anti-foot-gun controls

Map each control to `05-anti-foot-gun-planning-standard.md`.

### 5) Acceptance evidence model

Define what evidence is required and what does not count as evidence.

### 6) Escalation and exception path

Define when to halt, who approves exceptions, and expiry rules.

### 7) Maintenance and drift prevention

Define how the section stays accurate over time.

## Complexity hotspot treatment

Planning areas below are presumed W4 unless explicitly downgraded with rationale:

1. semantic ownership policy,
2. gate naming/threshold policy,
3. rollback/stop-condition policy,
4. exception and deferral lifecycle policy,
5. anti-foot-gun blocker criteria.

## Deep documentation quality checklist

- Are authority boundaries explicit?
- Is every key term canonical?
- Is each high-risk claim paired with controls and evidence?
- Are stop conditions and escalation routes explicit?
- Can a reviewer reject/accept deterministically?

If any answer is no, the section is incomplete.

## Pattern library for deep planning sections

### Pattern A: policy definition

Use when introducing a normative rule:

- rule statement,
- rationale,
- applicability,
- violation examples,
- enforcement mechanism,
- exception mechanism.

### Pattern B: milestone and gate definition

Use when defining readiness checkpoints:

- milestone objective,
- required gate evidence,
- fail conditions,
- escalation path,
- rollback planning requirements.

### Pattern C: exception/deferral policy

Use when allowing temporary non-compliance:

- deferral class,
- required metadata,
- expiry and revalidation cadence,
- automatic retirement trigger.

## High-risk planning errors to avoid

1. **Authority inversion**: Tier 2 doc overrides Tier 1 rule.
2. **Hidden non-goals**: scope exclusions are implicit instead of explicit.
3. **Execution leakage**: implementation tasks embedded in documentation-only plans.
4. **Evidence vagueness**: “looks good” acceptance with no criteria.
5. **Perpetual exception**: deferrals with no expiry or owner.
6. **Term drift**: same word used with different meanings across docs.

## Review protocol for deep documents

### Pass 1 (author self-review)

- check weight class assignment,
- verify required section density,
- verify anti-foot-gun and evidence sections.

### Pass 2 (peer planning review)

- check consistency with Tier 1 docs,
- check dependency and failure-mode completeness.

### Pass 3 (governance review)

- check authority compliance,
- check maintainability and update cadence.

## Completion criteria

This deep manual is complete when:

- it can be used to produce high-detail planning docs with consistent quality,
- it prevents under-specification in high-risk sections,
- it is aligned with anti-foot-gun and gate specs.



