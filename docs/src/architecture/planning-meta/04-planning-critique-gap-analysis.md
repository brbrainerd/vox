---
title: "Planning critique and gap analysis"
description: "Severity-ranked critique of prior Vox planning artifacts with root-cause analysis and explicit fix mapping into the planning corpus."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Planning critique and gap analysis

This document critiques the prior planning artifacts for the Web IR and full-stack migration effort, then maps each issue to specific corrective documents in the new planning corpus under `docs/src/architecture/planning-meta/`.

The goal is not to critique individual wording lines. The goal is to identify systemic planning weaknesses that create implementation risk, drift, or avoidable blockers.

## Inputs reviewed

- `docs/src/architecture/internal-web-ir-implementation-blueprint.md`
- `docs/src/adr/012-internal-web-ir-strategy.md`
- `docs/src/explanation/expl-architecture.md`
- `docs/src/explanation/expl-compiler-lowering.md`
- `docs/agents/governance.md`
- `docs/src/architecture/doc-to-code-acceptance-checklist.md`
- Conversation-level requirements from this planning cycle:
  - full-stack Vox target,
  - Web IR semantic source-of-truth preference,
  - islands compatibility preservation,
  - anti-foot-gun orientation,
  - explicit and non-truncated planning.

## Scoring model

Each finding is scored for:

- **Severity**: `Critical`, `High`, `Medium`, `Low`
- **Blast radius**: how many workstreams are impacted
- **Likelihood**: probability of recurrence if not fixed
- **Detection difficulty**: how hard it is to detect after the fact

This document uses `Critical` and `High` for issues that can cause real migration failure, prolonged drift, or repeated planning resets.

## Findings (severity ranked)

### F-01: Normative and historical content are mixed in the same artifact

- **Severity**: Critical
- **Root cause**: one large blueprint mixes specification intent, live execution logs, partial progress snapshots, and future backlog in the same page.
- **Why it is risky**:
  - future readers can misread old progress rows as current normative requirements,
  - contradictory status statements can both appear “true” in different sections,
  - implementation agents can pick the wrong source and optimize for stale rows.
- **Observable symptoms**:
  - operations catalog and progress summaries can conflict,
  - checklist blocks appear unbounded while selected sub-areas are actually done.
- **Fix strategy**:
  - split responsibilities into authoritative tiers,
  - define explicit authority hierarchy and update ownership.
- **Mapped fix documents**:
  - `01-master-planning-index.md`
  - `10-document-maintenance-protocol.md`
  - `08-milestone-gate-definition-spec.md`

### F-02: Semantic ownership boundaries remain underspecified at planning level

- **Severity**: Critical
- **Root cause**: architecture intent says “Web IR first,” but planning language still allows ambiguity about what may be added in legacy emitters during migration.
- **Why it is risky**:
  - new behavior may leak into compatibility paths,
  - drift expands exactly when migration should contract semantic surface area.
- **Observable symptoms**:
  - parity fixes duplicated in multiple emit paths,
  - wrapper files accrue behavior, not just adaptation.
- **Fix strategy**:
  - define explicit semantic ownership policy,
  - define no-new-semantics rules for compatibility modules,
  - define mandatory ownership checks in task authoring and gate specs.
- **Mapped fix documents**:
  - `05-anti-foot-gun-planning-standard.md`
  - `07-task-catalog-authoring-spec.md`
  - `08-milestone-gate-definition-spec.md`

### F-03: Cutover and rollback planning is not operationally explicit enough

- **Severity**: High
- **Root cause**: gate concepts exist, but cutover triggers, rollback triggers, and rollback rehearsal obligations are not uniformly encoded in planning templates.
- **Why it is risky**:
  - aggressive switches can happen without repeatable rollback confidence,
  - risk posture becomes personality-dependent instead of process-dependent.
- **Observable symptoms**:
  - “ready” can be interpreted differently by different reviewers,
  - fallback behavior is treated as temporary but persists.
- **Fix strategy**:
  - define milestone and gate evidence model with mandatory rollback evidence,
  - define stop conditions and kill-switch standards in fast LLM plan.
- **Mapped fix documents**:
  - `08-milestone-gate-definition-spec.md`
  - `02-fast-llm-instruction-plan.md`
  - `09-exception-deferral-policy.md`

### F-04: Deferred and ignored work is tracked, but closure mechanics are weak

- **Severity**: High
- **Root cause**: deferred items are listed, but required metadata and expiry behavior are not consistently enforced in planning docs.
- **Why it is risky**:
  - deferrals become hidden backlog gravity,
  - `#[ignore]` anchors can survive long after relevance.
- **Observable symptoms**:
  - tasks reopen under new names,
  - old deferrals do not have deterministic retirement criteria.
- **Fix strategy**:
  - define strict deferral classes and metadata schema,
  - enforce expiry + owner + closure test.
- **Mapped fix documents**:
  - `09-exception-deferral-policy.md`
  - `10-document-maintenance-protocol.md`
  - `07-task-catalog-authoring-spec.md`

### F-05: Planning granularity mismatch (too broad for execution, too dense for navigation)

- **Severity** { High
- **Root cause**: previous plans alternate between very high-level sections and very large checklists, with little middle-layer authoring standard.
- **Why it is risky**:
  - execution agents miss dependencies,
  - human reviewers cannot quickly detect sequencing errors.
- **Observable symptoms**:
  - repeated requests for “more explicit, less truncated” plan rewrites,
  - broad items that hide unresolved sub-problems.
- **Fix strategy**:
  - introduce atomic task schema with required dependency and evidence fields,
  - create fast and deep documents with non-overlapping purpose.
- **Mapped fix documents**:
  - `02-fast-llm-instruction-plan.md`
  - `03-weighted-deep-planning-manual.md`
  - `07-task-catalog-authoring-spec.md`

### F-06: Anti-foot-gun policy exists in spirit but not as a planning standard

- **Severity**: High
- **Root cause**: risks are discussed across multiple documents, but there is no single planning-level standard that blocks common self-inflicted failures.
- **Why it is risky**:
  - known pitfalls recur across milestones,
  - teams rely on memory and reviewer vigilance instead of policy.
- **Observable symptoms**:
  - silent fallback paths,
  - contract drift from emit to templates/runtime,
  - ambiguous acceptance interpretation.
- **Fix strategy**:
  - codify anti-foot-gun rules as a standalone standard with blocker criteria.
- **Mapped fix documents**:
  - `05-anti-foot-gun-planning-standard.md`
  - `08-milestone-gate-definition-spec.md`
  - `02-fast-llm-instruction-plan.md`

### F-07: Terminology drift increases interpretation errors

- **Severity**: Medium
- **Root cause**: vocabulary appears in multiple contexts with slight meaning differences (for example: “bridge,” “cutover,” “parity,” “source-of-truth”).
- **Why it is risky**:
  - teams may think they agreed while using different definitions,
  - planning acceptance arguments become circular.
- **Fix strategy**:
  - define canonical terminology and “do-not-use” ambiguous aliases.
- **Mapped fix documents**:
  - `06-planning-taxonomy-glossary.md`
  - `01-master-planning-index.md`

### F-08: Plan corpus governance is implicit instead of explicit

- **Severity**: Medium
- **Root cause**: no single maintenance protocol for versioning, supersession, and conflict resolution between planning docs.
- **Why it is risky**:
  - planning set degrades over time as new docs are added ad hoc,
  - old plans remain discoverable without clear supersession marker.
- **Fix strategy**:
  - define maintenance protocol with document lifecycle, approvals, and archival rules.
- **Mapped fix documents**:
  - `10-document-maintenance-protocol.md`
  - `01-master-planning-index.md`

## Root-cause synthesis

Most of the above failures derive from four meta-causes:

1. **Single-document overload**: too much responsibility in one artifact.
2. **Authority ambiguity**: unclear normative precedence.
3. **Template absence**: no standard task/gate/deferral schema.
4. **Policy scattering**: risk controls distributed without a central planning contract.

The new corpus is designed to solve these root causes directly.

## Assumption confidence addendum (external validation)

The critique fixes are informed by external references but grounded in repo evidence.

| Topic | External signal | Confidence | Planning implication |
| --- | --- | --- | --- |
| React interop maturity | React Compiler stable release and incremental adoption guidance | High | Keep React/TanStack compatibility as strategic boundary while improving internal IR ownership. |
| Nullability safety | TypeScript strict nullability behavior | High | Maintain explicit required/optional/defaulted planning semantics and evidence gates. |
| Islands architecture | Selective hydration patterns from Astro docs | Medium | Preserve stable island contract and avoid accidental wire-format drift in planning language. |
| Transform/codegen separation | SWC architecture split across AST/transform/codegen crates | Medium | Favor structured-lowering ownership with thin emission layers in planning architecture. |

Confidence policy:

- `High`: external source + clear alignment with current repo direction.
- `Medium`: external source is directional but not a direct implementation spec for Vox.

## Traceability matrix (finding -> target section)

| Finding | Primary target doc | Target section |
| --- | --- | --- |
| F-01 | `01-master-planning-index.md` | Authority hierarchy and read order |
| F-01 | `10-document-maintenance-protocol.md` | Versioning, supersession, archival |
| F-02 | `05-anti-foot-gun-planning-standard.md` | Semantic ownership and compatibility-only policy |
| F-02 | `07-task-catalog-authoring-spec.md` | Required ownership fields in every task |
| F-03 | `08-milestone-gate-definition-spec.md` | Cutover/rollback evidence and stop conditions |
| F-03 | `02-fast-llm-instruction-plan.md` | Deterministic execution ladder and halt rules |
| F-04 | `09-exception-deferral-policy.md` | Deferral metadata + expiry + retirement workflow |
| F-05 | `03-weighted-deep-planning-manual.md` | Weighted detail policy for complex sections |
| F-05 | `07-task-catalog-authoring-spec.md` | Atomic task schema and dependency notation |
| F-06 | `05-anti-foot-gun-planning-standard.md` | Blocker criteria and mandatory review questions |
| F-07 | `06-planning-taxonomy-glossary.md` | Canonical term system |
| F-08 | `10-document-maintenance-protocol.md` | Change control and governance cadence |

## Acceptance criteria for this critique

This critique is complete when:

- severity-ranked findings are explicit and actionable,
- each finding has root cause and fix strategy,
- each fix strategy maps to one or more concrete documents in the corpus,
- no finding depends on implementation execution to be understood.

## Status

- **State**: complete for this planning cycle
- **Next linked step**: apply this critique through document authoring standards and authority hierarchy in the rest of the planning-meta corpus.



