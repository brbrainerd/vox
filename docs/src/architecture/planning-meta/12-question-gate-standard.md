---
title: "Question gate standard for planning"
description: "Normative rules governing when a planning request must trigger clarification before planning begins, versus when auto-expansion or inference is the correct response."
category: "Architecture SSOTs"
tier: 1
last_updated: "2026-04-10"
training_eligible: true

schema_type: "TechArticle"
---

# Question gate standard for planning (planning-meta/12)

This document is a **Tier 1 normative standard** within the planning-meta corpus.
It governs the planning intake classification gate: specifically, the conditions under
which the planner MUST ask a clarifying question before generating a plan, versus when
it is safe to auto-expand, infer, or proceed autonomously.

Read order: after `01-master-planning-index.md`, before `02-fast-llm-instruction-plan.md`.

## Related SSOT documents

- Questioning protocol: [`docs/src/reference/information-theoretic-questioning.md`](../../reference/information-theoretic-questioning.md)
- Research grounding: [`docs/src/architecture/research-diagnostic-questioning-2026.md`](../../archive/research-2026-q1/research-diagnostic-questioning-2026.md)
- Plan adequacy / auto-expand: [`docs/src/architecture/plan-adequacy.md`](../../archive/research-2026-q1/plan-adequacy.md)
- Attention budget design: [`docs/src/architecture/agent-event-kind-ludus-matrix.md`](../../archive/research-2026-q1/agent-event-kind-ludus-matrix.md) (KI)

---

## Core principle

**Questioning before planning is an action of last resort, not a default.**
The planner should ask a clarifying question only when:

1. Multiple materially different plan shapes are plausible, AND
2. The cost of choosing the wrong interpretation exceeds the cost of asking, AND
3. The correct interpretation cannot be inferred from codebase facts, memory, or prior plans.

If any of these three conditions fails, the planner should instead:
- Auto-expand the plan using `auto_expand_thin_plan`
- Infer the missing detail from context and log the assumption
- Proceed with the most conservative valid interpretation

---

## Intake classification outcomes

The planning orchestrator's intake classification step must produce one of four outcomes:

| Outcome | Condition | Planning action |
|---|---|---|
| `ImmediateAction` | Low complexity, unambiguous, low risk | Execute directly without planning |
| `OodaLoop` | Dynamic / exploratory; environment changes during execution | Enter observe-orient-decide-act cycle |
| `HierarchicalPlan` | High complexity, multi-step, goal is clear | Generate full VoxPlan DAG |
| `RequiresClarification` | Goal maps to N≥2 materially different plan shapes AND EVPI exceeds threshold | Ask ONE question; suspend planning until answered |

The `RequiresClarification` outcome is the formal vehicle for planning-before-questioning.
It must not be triggered for low-stakes ambiguity or for ambiguity the planner can
resolve from evidence.

---

## RequiresClarification trigger criteria

All three conditions must be true to trigger `RequiresClarification`:

### Condition 1: Multiple plausible interpretations

The LLM intake classifier must identify at least two distinct action paths where:
- Each path would generate a substantially different plan (different files touched,
  different crate boundaries, different estimated complexity)
- The probability of each interpretation is ≥ 0.15 (neither is vanishingly unlikely)

### Condition 2: EVPI exceeds threshold

```
EVPI(goal, top_question) >= planner_config.evpi_question_threshold
```

Default threshold: `0.15` (configurable in `PlannerConfig`). This prevents asking
about low-stakes distinctions (e.g., naming conventions) that would barely change
the plan even if clarified.

EVPI is estimated by:
1. Estimate execution cost of each interpretation path (complexity × reversibility)
2. EVPI = max(path_costs) − weighted_mean(path_costs, by prior probability)

Where `reversibility` multiplier is: 1.0 for reversible, 3.0 for partially reversible,
10.0 for irreversible (deletes, migrations, public API changes).

### Condition 3: Cannot be inferred from evidence

The `ContextAssembler` must confirm that the ambiguous dimension is NOT resolvable from:
- Existing codebase facts (`repo_facts`) at confidence ≥ 0.75
- Relevant memories (embedding-based recall) at confidence ≥ 0.75
- Prior plan sessions for similar goals at confidence ≥ 0.75

If any evidence source resolves the ambiguity above threshold, the planner should
use that inference and log the assumption, not ask.

---

## Question construction requirements

When `RequiresClarification` fires, the generated question MUST:

1. **Use `multiple_choice` type** unless the hypothesis space is genuinely open
   (use `open_ended` only if N > 5 or the option space is unknown)
2. **List exactly the hypothesis interpretations as options** — not abstract categories,
   but actual plan consequences (e.g., "A: add to vox-mcp crate (2 files); B: create new
   vox-clarify crate (5 files + Cargo.toml update)")
3. **Include a default assumption** — what the planner will do after `timeout_secs` if
   no answer is received (prevents indefinite planning suspension)
4. **State the stakes** — brief sentence on what changes between options

Prohibited:
- Generic "Please clarify your request" messages
- Questions about scope that can be answered by reading existing files
- More than one question per `RequiresClarification` trigger

---

## Attention budget constraints on questioning

Regardless of EVPI, the following attention budget constraints override the question gate:

| Budget state | Gate behavior |
|---|---|
| `FocusDepth::Deep` | Defer all `RequiresClarification` triggers to next checkpoint; use most conservative interpretation |
| `BudgetSignal::Critical` | Same as Deep; log assumption for post-hoc review |
| `BudgetSignal::CostExceeded` | Same; do not suspend planning; proceed with safe default |
| `interrupt_ewma > 0.8` | Apply backlog penalty; raise EVPI threshold by +50% |

These constraints implement the "flow state = inbox suppression" principle from the
cognitive architecture research. A planner under budget pressure should not compound
attention costs by asking questions.

---

## Auto-expand preference over questioning

If Condition 1 or Condition 2 fails (interpretations not sufficiently distinct, or
EVPI below threshold), the planner MUST prefer **auto-expansion** over asking.

Auto-expansion proceeds by:
1. Selecting the most probable interpretation
2. Generating a complete plan with that interpretation
3. Adding a plan-level note: `"Assumption: interpreted goal as X. Alternate interpretation Y
   was considered but EVPI was below threshold."`
4. Setting `plan.requires_approval = true` if the interpretation involved any irreversible step

This ensures users can review assumptions at the plan level without requiring pre-planning
interruption.

---

## Acceptance criteria

This standard is satisfied when:

- The intake classifier type system includes `RequiresClarification` as a named outcome
- `PlannerConfig` includes `evpi_question_threshold` with documented default
- No planning session proceeds past intake with N≥2 interpretations AND EVPI≥threshold
  without emitting a structured question (verified via `plan_events` audit)
- All `RequiresClarification` questions pass question construction requirements above
- Zero `RequiresClarification` triggers fire when `FocusDepth::Deep` or budget is Critical
- Auto-expansion is used in ≥ 80% of ambiguous-but-low-EVPI cases (no spurious questioning)

---

## Relationship to other planning-meta documents

| Document | Relationship |
|---|---|
| `02-fast-llm-instruction-plan.md` | This standard governs the pre-planning gate; that document governs plan execution |
| `05-anti-foot-gun-planning-standard.md` | Failure to ask when EVPI is high = foot-gun; failure to NOT ask when EVPI is low = friction overload |
| `08-milestone-gate-definition-spec.md` | `RequiresClarification` outcomes are milestone-blocking; this document specifies conditions |
| `09-exception-deferral-policy.md` | Deferred questions (attention budget constraint) should be registered as deferrals with expiry |
