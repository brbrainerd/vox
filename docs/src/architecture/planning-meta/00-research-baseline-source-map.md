---
title: "Research baseline and source-of-truth map"
description: "Research appendix for planning-meta creation, including source classification and confidence tags."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Research baseline and source-of-truth map

This appendix captures the research baseline used to build the planning-meta corpus.

## Source classification model

- **Normative source**: defines policy or contract that other planning docs should not contradict.
- **Operational source**: describes practical workflow and execution state.
- **Explanatory source**: clarifies architecture intent and boundaries.
- **Analytical source**: provides checklists or critique support.

## Classified sources

| Source | Classification | Confidence | Notes |
| --- | --- | --- | --- |
| `docs/src/architecture/internal-web-ir-implementation-blueprint.md` | operational + partial normative | Medium | comprehensive, but mixes historical and active sections |
| `docs/src/adr/012-internal-web-ir-strategy.md` | normative architecture intent | High | accepted ADR with clear target boundaries |
| `docs/src/explanation/expl-architecture.md` | explanatory | High | conceptual pipeline and module map |
| `docs/src/explanation/expl-compiler-lowering.md` | explanatory | High | lowering-phase narrative and current-vs-target bridge |
| `docs/agents/governance.md` | normative quality/governance constraints | High | TOESTUB and quality review constraints |
| `docs/src/architecture/doc-to-code-acceptance-checklist.md` | analytical + acceptance checklist | High | concrete merge-time checklist controls |

## Baseline goals extracted

1. Build a full-stack Vox strategy centered on internal structural representation.
2. Preserve current islands compatibility while reducing internal complexity.
3. Improve semantic ownership clarity across AST/HIR/Web IR/emit layers.
4. Define anti-foot-gun planning controls.
5. Make planning explicit enough for agent execution with low ambiguity.

## Risks discovered during research

1. Normative and historical content co-located in large planning artifacts.
2. Drift risk in ownership language and gate interpretation.
3. Deferral metadata inconsistent across artifacts.
4. Truncation pressure in large plans without explicit weighted detail policy.

## External assumption validation (web + repo)

| Assumption | Status | Confidence | Source links | Notes |
| --- | --- | --- | --- | --- |
| React ecosystem interop remains high-value for Vox web strategy | Supported | High | [React Compiler 1.0 stable](https://react.dev/blog/2025/10/07/react-compiler-1), [React Compiler docs](https://react.dev/learn/react-compiler) | Aligns with ADR strategy to keep React/TanStack target while reducing internal complexity. |
| Strict nullability modeling reduces undefined-behavior risk | Supported | High | [TypeScript strictNullChecks](https://www.typescriptlang.org/tsconfig/strictNullChecks.html) | Supports explicit `Required`/`Optional`/`Defaulted` planning posture for WebIR boundaries. |
| Island architecture remains compatible with attribute-anchored hydration contracts | Supported | Medium | [Astro islands architecture](https://docs.astro.build/core-concepts/component-hydration) | Confirms selective-hydration compatibility model; does not prescribe Vox wire format details. |
| Transform/codegen separation improves maintainability in compiler systems | Supported | Medium | [SWC architecture](https://raw.githubusercontent.com/swc-project/swc/main/ARCHITECTURE.md) | Supports planning preference for structured IR + thin printers. |

Validation caveats:

- External references support directionality, not one-to-one implementation requirements.
- Repo code-path truth remains the final authority for current-state claims.

## Why this appendix exists

This file provides traceability for the planning corpus. It reduces “why did we choose this structure?” churn during future rewrites.



