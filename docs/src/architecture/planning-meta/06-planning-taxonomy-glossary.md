---
title: "Planning taxonomy and glossary"
description: "Canonical terminology for planning-meta artifacts, including preferred terms, forbidden ambiguous aliases, and historical mappings."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Planning taxonomy and glossary

Use this glossary for all planning-meta documents.

## Canonical terminology

### Authority and governance terms

- **Authority tier**: precedence level of a planning document (`Tier 1`, `Tier 2`, `Tier 3`).
- **Normative**: rule-defining content that lower tiers must follow.
- **Operational (planning)**: execution-oriented planning instructions consistent with normative rules.
- **Implementation execution**: code/build/test actions on the product codebase; out-of-scope in doc-only planning mode unless explicitly requested.
- **Analytical**: critique/reference material that informs planning decisions.
- **Supersession**: explicit replacement of an older planning artifact by a newer one.

### Planning quality terms

- **Anti-foot-gun control**: preventive rule that blocks known planning hazards.
- **Blocker class**: violation type that requires rejection of a planning change.
- **Acceptance evidence**: objective artifacts required to mark a planning section complete.
- **Stop condition**: state where planning work must halt and escalate before continuing.
- **Deferral**: approved temporary postponement with owner/expiry/closure metadata.

### Migration architecture terms

- **Semantic ownership**: the single authoritative planning owner for a behavior class.
- **Compatibility-only surface**: legacy surface allowed only for adaptation, not new semantics.
- **Dual-path drift**: divergence risk caused by parallel behavioral pathways.
- **Fallback visibility**: requirement that fallback pathways are observable and constrained.
- **Contract integrity**: stability and consistency of planned interface assumptions across surfaces.

### Milestone and gate terms

- **Milestone**: named planning checkpoint with explicit completion evidence.
- **Gate**: pass/fail criterion attached to a milestone or release stage.
- **Escalation path**: named process and owner route when gate/milestone conditions fail.
- **Rollback readiness (planning-level)**: documented ability to revert rollout assumptions safely.

### Detail strategy terms

- **Weighted depth**: proportional detail level based on risk and complexity.
- **W1/W2/W3/W4**: low/moderate/high/critical planning weight classes.
- **Token weighting**: assigning more explanation and constraints to higher-risk planning sections.

## Historical aliases and mappings

| Historical term | Canonical term |
| --- | --- |
| “master roadmap doc” | master planning index + corpus |
| “plan rewrite” | supersession with authority update |
| “execution plan” (in doc-only mode) | operational planning document |
| “safety checklist” | anti-foot-gun control set |
| “deferred TODO” | deferral record with expiry metadata |

## Ambiguous terms to avoid

Avoid these without explicit qualifier:

- “ready” -> use “ready by gate `Gx` with evidence class `Ey`”
- “done” -> use “accepted against defined acceptance evidence”
- “temporary” -> use “deferral with expiry and closure test”
- “safe” -> use “non-violation of blocker classes + evidence”
- “aligned” -> use “tier-consistent and conflict-free”

## Preferred phrasing patterns

- “must” for Tier 1 requirements.
- “should” for recommended practices.
- “may” only for explicitly optional behavior with no blocker risk.

## Glossary maintenance rules

1. Add a term only if used across at least two planning docs.
2. Add mappings when replacing legacy wording.
3. Remove deprecated terms only after all corpus docs are updated.
4. Update this glossary in the same change as new canonical policy terms.

## Acceptance criteria

This glossary is complete when:

- all planning-meta documents use canonical terms for core concepts,
- ambiguous aliases are either removed or mapped,
- tier and evidence language is consistent across the corpus.



