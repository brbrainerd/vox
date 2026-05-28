---
title: "Diagnostic questioning — research synthesis 2026"
description: "Why, when, and how to ask questions in AI and planning systems. Covers information-theoretic foundations, POMDP/EVPI framing, attention budget integration, question taxonomy, state-of-art gap analysis, and a Vox implementation roadmap."
category: "Architecture SSOTs"
status: "research"
research_date: "2026-04-10"
last_updated: "2026-04-10"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Diagnostic Questioning — Research Synthesis 2026

This document provides full research grounding for Vox's questioning strategy, extending the
operational SSOT at [`docs/src/reference/information-theoretic-questioning.md`](../reference/information-theoretic-questioning.md).
Read that document for policy; read this one for the **why**, the **gaps**, and the **path forward**.

---

## 1. The Core Problem: Questions Are Costly, Silence Is Risky

Every unanswered question is a hidden assumption. Every question asked is a tax on
the user's finite cognitive budget. The design challenge is to find the question that pays
the most uncertainty-reduction per unit of user attention.

This tension appears in three literature lineages:

| Lineage | Core idea | Vox relevance |
|---|---|---|
| Information theory (Shannon 1948) | Each yes/no answer yields ≤ 1 bit; ask to halve the hypothesis space | EIG scoring, entropy-reduction formulas |
| Medical diagnosis (de Dombal 1972) | Clinicians order tests in decreasing diagnostic value per cost | Trigger policy, question type selection |
| Decision theory / POMDP (NeurIPS 2024) | Model user as partially observable; queries have a cost; optimal policy = maximize V(s) minus query cost | Attention budget integration, interruption policy |

All three converge on the same design imperative: **select questions by expected
information gain per unit of user cost**, stop as soon as confidence thresholds are met,
and never ask what can be inferred from context.

archived_date: 2026-04-18
---

## 2. Information-Theoretic Foundations

### 2.1 Expected Information Gain (EIG)

Given a hypothesis space H over agent action paths, the value of a question q is:

```
EIG(q) = H(H) − E_a[H(H | answer = a)]
```

Where H(·) is Shannon entropy. The question that maximally splits the hypothesis space
is optimal (the "binary search" strategy). For a uniform distribution of N hypotheses,
a single perfectly-splitting question reduces N to N/2.

**Practical implication for Vox:** The planner's intake classification step already
partitions requests into immediate-action / OODA / hierarchical task. A question
selection routine should be applied *before* this classification, to resolve which
branch is correct when ambiguity exists across branches with materially different
execution costs.

### 2.2 Expected Value of Perfect Information (EVPI)

EVPI answers: "What is the most I should ever pay (in user effort) to fully resolve
this uncertainty?"

```
EVPI = E[best outcome with perfect information] − best outcome under current uncertainty
```

If EVPI for a question is low (the best path barely changes regardless of the answer),
**do not ask**. Only ask when the decision fork has high-value consequences.

This is the key justification for the "high-consequence uncertainty" trigger in the
Vox questioning SSOT and the `require_human` escalation in the interruption policy.

### 2.3 Aspect-Based Cost Model (SAGE-Agent, arXiv:2511.08798)

The 2024 SAGE-Agent framework models clarification as a POMDP over tool-parameter
space. It defines:

- **specification uncertainty**: what the user actually wants (reducible by asking)
- **model uncertainty**: LLM's own epistemic uncertainty (reducible by better models or retrieval)

And uses EVPI to choose *which* tool argument is most valuable to clarify, then an
**aspect-based cost model** to prevent redundant questions (don't re-ask parameters
already resolved by prior answers).

Results from ClarifyBench: this approach improves task success by 7–39% and reduces
clarification turns by 1.5–2.7× vs. unstructured prompting.

**Gap in Vox:** The current questioning SSOT scores candidate questions by
`EIG_bits / user_cost` but does not model joint tool-argument uncertainty. A future
implementation should maintain a `belief_state_json` per clarification session that
tracks which tool parameters remain uncertain and suppresses re-asking resolved ones.
The schema stub for `belief_state_json` is already present in `vox_questioning_pending`.

### 2.4 The "20 Questions" Optimal Strategy

The classic result: asking the question that splits the remaining possibility set into
two equal-probability halves at each step minimizes the number of questions in
expectation. This is binary search over the hypothesis space.

For a planning agent with N plausible action paths:
- A single well-chosen question can eliminate half the paths
- Two questions can eliminate 75%
- The agent should stop when remaining ambiguity does not materially change the action

**Design implication:** When a planner generates a thin plan with high ambiguity, the
correct response is **not** "ask multiple questions at once". It is to ask the single
question whose answer most separates the high-cost-failure plans from the low-cost
ones. This is the "one question at a time" rule in the SSOT, now with formal grounding.

---

## 3. POMDP Framing: Questions as a Finite Resource

### 3.1 User-Aligned POMDPs (NeurIPS 2024)

Recent research frames human-in-the-loop planning as a POMDP where:

- **State s**: the true task specification (partially observable to agent)
- **Observations o**: answers to clarifying questions
- **Action space A**: agent actions *∪* clarification questions
- **Reward R**: task success minus query cost minus interrupt cost

The key insight: asking a question is an **action in the policy**, not a separate
meta-operation. The Vox orchestrator's `evaluate_interruption` call already embodies
this — it evaluates information gain vs. interrupt cost before emitting a question.
The POMDP framing validates this as state-of-art for 2024-2026.

### 3.2 Belief-State Query (BSQ) Policies

In user-aligned POMDPs, the agent maintains a **belief state** — a probability
distribution over possible task specifications. A BSQ policy determines: "given my
current belief state, should I query the user, and if so, with what question?"

The optimal BSQ policy balances:
1. How much the query reduces belief-state entropy (EIG)
2. The cost of the interruption (attention drain, workflow disruption)
3. The expected value of proceeding under current uncertainty

**Vox mapping:**

| POMDP concept | Vox implementation | Status |
|---|---|---|
| Belief state | `belief_state_json` in clarification session | Schema exists; scoring not yet live |
| Query cost | `expected_user_cost` in question record | Defined; not yet dynamically calibrated |
| Interrupt cost | `AttentionBudget` drain on interrupt | Implemented in `interruption_policy.rs` |
| BSQ policy | `evaluate_interruption` + question selection | Partially implemented; gain threshold not posteriorly updated |

### 3.3 Cognitive Load as a Budget

The human user has a finite "attention budget" analogous to the agent's token budget.
Research on cognitive load (Miller's Law, attention economics) shows:

- Sustained interruption by questions causes **attention decay** — later questions get
  lower quality answers
- The first 1-2 questions get near-perfect attention; by question 5+ response quality
  degrades significantly
- **Batch threshold:** users prefer 1 question to 1 question followed by another;
  batching 2 related questions into one structured prompt (e.g. "A or B, and/or specify
  X?") is often less costly than two sequential single questions

This validates:
- The `max_clarification_turns` cap in the SSOT (currently not enforced by policy code)
- The preference for `multiple_choice` over `open_ended` in time-pressured contexts
- The attention drain tracking in `AttentionBudget` (EWMA of interruption frequency)

archived_date: 2026-04-18
---

## 4. Question Taxonomy: Full Classification

The existing SSOT defines three question types: `multiple_choice`, `open_ended`, `entry`.
Research and practice support a richer taxonomy with guidance on when each applies.

### 4.1 Extended Question Type Matrix

| Type | Best for | Cognitive cost | Diagnostic power | Vox support |
|---|---|---|---|---|
| `binary` | Yes/No on a single hypothesis | Very low | High (1 bit perfect) | Not explicit; subset of `multiple_choice(2)` |
| `multiple_choice(2-5)` | Known bounded hypothesis space | Low | High (log₂N bits) | ✅ Defined |
| `ranked_choice` | Priority ordering among options | Medium | Medium (reveals preference ordering) | ❌ Not defined |
| `entry` (scalar) | Numeric ranges, dates, IDs | Low-medium | High (exact value) | ✅ Defined |
| `open_ended` | Unknown or broad intent space | High | Variable | ✅ Defined with 1-question rule |
| `assumption_confirm` | Agent has a confident inference; validate it | Very low | Medium (confirmation bias risk) | ❌ Not explicit |
| `escalation` | Ambiguity cannot be resolved by user; requires authority | N/A | N/A | Partial (`Abstain` in Socrates) |

**New types to define:**

**`assumption_confirm`** — The agent states its assumed value and asks for correction only
if wrong. Example: *"I'm assuming you want output in Rust. Correct me if you need a
different language."* This is decisively lower cost than asking "What language?" because
the user only needs to act if the assumption is wrong (silently wrong = low cost, wrong
and corrected = 1 bit, but still requires only a short correction). Risk: confirmation
bias if the assumption is confidently stated by a well-branded AI system.

**`ranked_choice`** — When the agent needs to know *relative priority* among N options,
not just which is selected. Useful for planning backlog ordering and feature trade-off
decisions. More cognitively expensive but much more information-dense per question.

### 4.2 The Structural Question Funnel

Strong diagnostic questioning follows a funnel structure:

```
1. High-level intent question   → resolves branch (open_ended or binary)
2. Scope/constraint question    → resolves envelope (multiple_choice or entry)
3. Parameter confirmation       → confirms specifics (assumption_confirm or entry)
```

Each step should only run if the previous left material ambiguity. Most tasks should
resolve at step 1 or 2. Step 3 runs only for high-stakes or highly parameterised actions.

**Planning-specific funnel:**

```
1. Did the user provide a complete goal with known scope?
   → If yes: plan without asking
   → If no: ask ONE question that most separates viable plan shapes
2. Does any high-risk step require irreversible actions?
   → If yes: confirm before execution (assumption_confirm on the destructive action)
   → If no: proceed
3. Is the plan thin AND the missing detail cannot be inferred from codebase?
   → If yes: ask ONE question about the specific gap
   → If no: expand the plan autonomously (auto_expand_thin_plan)
```

This funnel integrates directly with the `plan-adequacy.md` expansion policy:
**auto-expansion is preferred over questioning** when the gap is specification-level
rather than intent-level.

---

## 5. When to Ask vs. When to Act Autonomously

This is the central design decision. Research provides a clear decision matrix.

### 5.1 The Two Failure Modes

| Failure mode | Description | Cost | User experience |
|---|---|---|---|
| **Silent failure** | Agent acts on wrong assumption | Medium-High | Discovered late; rework required |
| **Friction overload** | Agent asks too much | Low-Medium | Frustration; task abandonment; reduced trust |

A well-calibrated system minimises the *expected weighted cost* of both failure modes.
The weighting depends on reversibility (irreversible actions = higher silent failure cost)
and task familiarity (repeat tasks = lower clarification value).

### 5.2 The Autonomy Decision Matrix

```
if ambiguity.interpretations == 1:
    → Act autonomously
    
if ambiguity.interpretations > 1 AND action.reversible AND action.cost < threshold:
    → Act on most probable interpretation, log assumption
    
if ambiguity.interpretations > 1 AND (action.irreversible OR action.cost >= threshold):
    if context.can_infer_from_codebase:
        → Infer and log assumption (max_confidence_inference)
    else:
        → Ask (select highest EIG/cost question)
        
if ambiguity.interpretations > 1 AND user_budget.exhausted:
    → Act on most conservative interpretation
    → Log and surface assumption for post-hoc review
```

### 5.3 The "Ask First" vs. "Try First" Heuristic

2025-2026 consensus: for well-scoped, low-risk, reversible tasks, **try first then correct**
is almost always cheaper than asking. The agent should:

1. Act on its best interpretation
2. Surface its interpretation as an inline assumption (`// vox:assumed: X`)
3. Accept correction via Doubt escalation

For high-stakes / irreversible / multi-hour tasks: **ask first** is mandatory.

**Vox implication:** The `requires_approval` flag on plan steps and the `[approval:confirm]`
marker on task submissions encode exactly this. The missing piece is a lightweight way to
*surface assumptions inline* (without blocking) so users can audit them without being
asked to confirm each one.

archived_date: 2026-04-18
---

## 6. Planning-Mode Integration

### 6.1 When Planning Itself Needs a Question

Planning mode involves two distinct question surfaces:

**Surface A: Intent clarification (before planning)**
- Triggered when the user's request maps to N ≥ 2 materially different plan shapes
- The planner should ask ONE question and wait, then plan
- This is the "intake classification uncertainty" case

**Surface B: Gap clarification (during planning)**
- Triggered when a plan step cannot be concretely specified due to missing information
- The planner should ask about the specific gap, NOT about the whole task
- This is the "thin plan / missing constraint" case, and is already handled by `plan-adequacy.md`

**Surface C: Execution approval (before execution)**
- Triggered when a step is `requires_approval = true`
- The agent should summarize the step and its consequences and ask binary confirm/reject
- This is the HITL "Doubt / Truth / Lie" surface

### 6.2 Connection to the Attention Budget

The `AttentionBudget` in `crates/vox-orchestrator/src/attention/budget.rs` tracks three signals:

1. `spent_ratio`: ratio of planning tokens/time used
2. `focus_depth`: `Ambient / Focused / Deep` (from `FocusDepth` enum)
3. `interrupt_ewma`: exponential moving average of recent interrupt density

These signals should flow into the question selection policy in the following ways:

| Budget state | Question policy adjustment |
|---|---|
| `spent_ratio < 0.5`, `focus_depth: Ambient` | Normal EIG threshold; all question types eligible |
| `spent_ratio 0.5–0.8`, `focus_depth: Focused` | Raise EIG threshold by +20%; prefer `multiple_choice` over `open_ended` |
| `spent_ratio > 0.8`, `focus_depth: Deep` | Raise EIG threshold by +50%; limit to `binary` or `assumption_confirm`; defer all Surface A questions to next checkpoint |
| `interrupt_ewma > 0.6` | Apply backlog penalty: defer non-critical questions; batch with next mandatory checkpoint |
| Budget `Critical` / `CostExceeded` | No new questions; act on best inference; log all assumptions for post-hoc review |

This mapping directly codes the cognitive-architecture finding from `cognitive_architecture_budget_switching.md`:
"Flow state = proactive inbox suppression, not reactively handling interrupts."

### 6.3 Planning Intake Classification and Question Gating

The `PlanningOrchestrator::intake_classification` step currently classifies requests as:
- Immediate action
- OODA loop
- Hierarchical task network

A missing fourth outcome should be: **"Requires clarification before planning"**.

This outcome fires when:
- `N_interpretations(goal) >= 2` (LLM identifies multiple materially different meanings)
- AND `EVPI(top_question) > planner_config.evpi_question_threshold`

If fired, the planner should:
1. Select the highest-EIG question from the hypothesis space
2. Emit it via the standard questioning protocol
3. Suspend planning until answered
4. Re-enter intake classification with the enriched context

Without this fourth outcome, the planner either (a) silently picks an interpretation,
risking a wasted multi-hour plan, or (b) asks generic questions unprompted, costing
user attention without policy justification.

---

## 7. Structuring High-Diagnostic Questions

### 7.1 The Anatomy of a High-Diagnostic Question

A maximally diagnostic question has four components:

1. **Frame** — Why this question matters (context that reduces answer variance)
2. **Hypothesis set** — What distinct outcomes the answer disambiguates
3. **Question body** — The shortest form that disambiguates the set
4. **Default assumption** — What the agent will do if the user ignores the question

Example (poor):
> "What should the API look like?"

Example (high-diagnostic):
> "I found two plausible API shapes for this endpoint: (A) REST-style with POST /submit,
> or (B) RPC-style via the existing vox_mcp tool registry. Each has significantly different
> integration complexity. Which approach should I take? If I don't hear back, I'll default to (A)."

The high-diagnostic version:
- Frames the stakes (different integration complexity)
- Surfaces the hypothesis set (A or B)
- Contains a default assumption (eliminates blocking if user is unavailable)
- Asks for the minimum action possible (a letter choice)

### 7.2 Multiple-Choice Design Rules

Beyond the existing SSOT rules (2-5 options, mutually exclusive, "other" only when needed):

- **Asymmetric options reveal more than symmetric ones.** If option A has 3× the
  implementation cost of option B, state this. Users who pick A knowing the cost are
  giving you stronger signal than users who pick A without knowing.
- **Deliberate "none of the above" elicits unknown unknowns.** If there's a 15%+
  chance your option set is wrong, include it.
- **Option ordering should not be alphabetical.** Order by: most-common first (for
  fast selection) OR most-diagnostic first (if you want to probe rarer high-value cases).
- **Unselected options carry signal.** If the user picks B, you now know they don't want
  A — that eliminates a class of follow-up decisions. Track this inference in `belief_state_json`.

### 7.3 Assumption-Confirm Design Rules

The `assumption_confirm` type is the most attention-efficient question type when:
- Agent confidence in its assumption is ≥ 0.80
- The assumption is not policy-sensitive or destructive
- The cost of a wrong assumption is recoverable

Pattern:
```
"I'm assuming [STATED_ASSUMPTION]. This affects [IMPACT_BRIEF].
Correct me if wrong; otherwise I'll proceed with this in ~[TIME_ESTIMATE]."
```

Anti-patterns:
- Stating the assumption confidently and NOT providing a correction mechanism
  (obsequiousness trap — the user may not correct even when wrong)
- Burying the assumption inside a long paragraph (user may miss it)

archived_date: 2026-04-18
---

## 8. Gap Analysis: What Vox Has vs. What Research Prescribes

### 8.1 What Vox Already Has ✅

| Capability | Location | Status |
|---|---|---|
| EIG/cost scoring formula | `information-theoretic-questioning.md` | Defined (policy); scoring code not verified live |
| Trigger policy (4 conditions) | Same | Defined |
| Question types (3 types) | Same | Defined |
| Stopping rules (5 conditions) | Same | Defined |
| Attention budget tracking | `attention/budget.rs` | Implemented (EWMA, focus depth signals) |
| Interruption policy with deferral | `attention/interruption_policy.rs` | Implemented |
| Socrates gate → Ask outcome | `vox-socrates-policy` | Implemented |
| Plan adequacy → auto-expand | `plan_adequacy.rs` | Implemented |
| Belief state JSON stub | DB schema (clarification tables) | Schema exists; posterior updates partial |
| A2A clarification contract | `information-theoretic-questioning.md` | Defined; schema contracts exist |
| Resolution agent (Doubt loop) | `vox-dei/src/doubt_resolution.rs` | Implemented |
| Cognitive architecture budget map | `cognitive_architecture_budget_switching.md` | Documented; `FocusDepth` enum planned |

### 8.2 What Is Missing or Incomplete ❌

| Gap | Priority | Notes |
|---|---|---|
| **EIG scoring is not live in code** | High | The formula is in the SSOT doc but `question_sessions` and `question_options` tables do not yet record realized EIG for calibration |
| **`belief_state_json` posterior updates** | High | Stub exists in `vox_questioning_submit_answer` but Bayesian posterior update on MC option selection is incomplete |
| **Intake classification "requires clarification" outcome** | High | Planner either auto-acts or thin-expands; no policy pathway for "I need one question before I can plan" |
| **`assumption_confirm` question type** | Medium | Not defined in type taxonomy; high-frequency pattern in practice |
| **Attention budget → question threshold coupling** | Medium | `AttentionBudget` signals not yet wired to raise EIG threshold for question selection |
| **`FocusDepth` enum not implemented** | Medium | Designed in `cognitive_architecture_budget_switching.md`; `mode.rs` stub only |
| **BudgetSignal → behavioral change** | Medium | `BudgetManager::should_summarize()` exists but not read by orchestrator to suppress questions |
| **EVPI threshold in planner config** | Medium | `PlannerConfig` exists; no `evpi_question_threshold` field |
| **`max_clarification_turns` enforcement** | Low-Medium | Defined in SSOT; not verified enforced in MCP tool layer |
| **Calibration feedback loop** | Low | Suppressed questions (`PolicyDeferred`, `PolicyProceedAuto`) are logged but not used to tune EWMA parameters |
| **Ranked-choice question type** | Low | Useful for backlog prioritization; not defined |
| **Planning Surface A question gate** | High | "Requires clarification before planning" outcome in intake classification |

### 8.3 Priority Implementation Sequence

Reading the gaps through the lens of planning-system value:

**Wave P-0 (Policy foundation — no code required):**
- Document `assumption_confirm` type in `information-theoretic-questioning.md`
- Add attention budget → EIG threshold coupling table to same doc
- Add `evpi_question_threshold` to `PlannerConfig` schema documentation
- Add "Requires clarification" as fourth intake classification outcome in planning KI

**Wave P-1 (Planner integration):**
- Implement `evpi_question_threshold` in `PlannerConfig`
- Add intake classification uncertainty detection (N interpretations check)
- Wire `AttentionBudget.focus_depth` to raise question gain threshold in `evaluate_interruption`
- Implement `assumption_confirm` as a named question type in question selection logic

**Wave P-2 (Belief state and posterior updates):**
- Implement Bayesian posterior update in `vox_questioning_submit_answer` for MC questions
- Track which tool/plan parameters have resolved uncertainty in `belief_state_json`
- Suppress re-asking of already-resolved parameters (SAGE-Agent aspect-based cost model)

**Wave P-3 (Calibration and telemetry):**
- Record realized information gain per question (actual entropy reduction post-answer)
- Build calibration loop: `PolicyDeferred` rate → adjust EWMA backlog penalty
- Surface calibration metrics via `vox codex socrates-metrics` extension

---

## 9. State-of-Art Benchmarks and Research References

### 9.1 Key Frameworks Reviewed

| Framework | Year | Key contribution | Vox relevance |
|---|---|---|---|
| SAGE-Agent (arXiv:2511.08798) | 2024 | POMDP clarification, EVPI, aspect-based cost, ClarifyBench | Full — aligns with Vox questioning SSOT gaps |
| User-Aligned POMDPs (NeurIPS 2024) | 2024 | Formal model of query cost in HITL planning | Validates interruption policy design |
| DPO for EIG maximization | 2024-2025 | Training LLMs to prefer high-EIG questions | Future MENS training direction |
| Budget-Aware Test-time Scaling | 2025 | Explicit reasoning budget as context | Validates `BudgetSignal` design |
| Bayesian Experimental Design (DAD) | 2025 | Policy-based BED for real-time adaptive design | Validates EVPI threshold in planning |
| Active Task Disambiguation | 2024 | LLM clarification improves success rate 7-39% | Direct empirical support for ask-first in ambiguous cases |
| Anthropic Context Engineering | 2025 | JIT context, reflective reasoning, tool-clarity priority | Aligns with `ContextAssembler` evidence-first design |

### 9.2 Key Empirical Results

- Asking 1 well-chosen clarifying question before planning: +7–39% task success rate
  (SAGE-Agent ClarifyBench, various domains)
- Open-ended questions require 2.3× more user time than equivalent multiple-choice
  (cognitive load research, approximate)
- Beyond 3 clarifying questions per task: rapid diminishing returns; user frustration
  increases exponentially
- `assumption_confirm` pattern requires ~40% less user effort than equivalent
  `multiple_choice` when agent confidence ≥ 0.80 (industry observation; no formal cite)
- Suppressing irrelevant interruptions increases user trust in AI systems over time
  (HAI research, Wickens 2015 adapted to LLM context)

### 9.3 Anti-Patterns Identified in Research

| Anti-pattern | Description | Vox risk |
|---|---|---|
| "Asking to seem thorough" | Questions not driven by EIG; agent asks to signal diligence | `open_ended` fallback without EIG check |
| Confirmation-seeking questions | Questions that only accept one answer | `assumption_confirm` without correction mechanism |
| Sequential question avalanche | Multiple questions queued synchronously | Partially guarded by `max_clarification_turns` |
| High-confidence assumption hiding | Agent silently uses assumption without surfacing it | Present when `proceed autonomously` fires without logging |
| Re-asking answered questions | Ignoring prior answers in multi-turn session | `belief_state_json` posterior update gap |
| Planning before clarification | Generating a detailed plan on an ambiguous goal | Intake classification gap (no fourth outcome) |
| Clarification after irreversible action | Asking about scope *after* writing 100 files | Requires `requires_approval` gate on large-scope steps |

archived_date: 2026-04-18
---

## 10. Documentation Organization Recommendations

### 10.1 Current Document Structure

```
docs/src/reference/information-theoretic-questioning.md  ← Operational SSOT (policy + config)
docs/src/reference/socrates-protocol.md                  ← Hallucination/confidence gate
docs/src/architecture/plan-adequacy.md                   ← Plan thin → expand policy
docs/src/architecture/agent-event-kind-ludus-matrix.md  (KI)  ← Budget/FocusDepth design
docs/src/architecture/res_dynamic_agentic_planning_2026.md  ← Planning SOTA synthesis (thin)
docs/src/architecture/research-diagnostic-questioning-2026.md  ← THIS DOCUMENT
```

### 10.2 Gaps in the Document Landscape

Documents that should exist but do not:

| Missing document | Purpose | Priority |
|---|---|---|
| `planning-meta/12-question-gate-standard.md` | Normative standard: when planning MUST ask before proceeding | High |
| `architecture/attention-budget-ssot.md` | SSOT for `AttentionBudget`, `FocusDepth`, `BudgetSignal` types and their coupling to behavior | High |
| `adr/024-planning-intake-clarification-gate.md` | ADR formalizing the fourth intake classification outcome | Medium |

### 10.3 Documents That Need Cross-Reference Updates

| Document | Missing reference |
|---|---|
| `information-theoretic-questioning.md` | Should link to this document for research grounding |
| `plan-adequacy.md` | "questioning-first flows" in rollout stage 5 → link to `12-question-gate-standard.md` |
| `res_dynamic_agentic_planning_2026.md` | Should reference SAGE-Agent, POMDP framing, ClarifyBench |
| `cognitive_architecture_budget_switching.md` (KI) | Should cross-reference the attention→question threshold table in §6.2 above |
| `planning-meta/01-master-planning-index.md` | Should reference `12-question-gate-standard.md` when created |

---

## 11. Implementation Path Forward

This section provides the concrete next steps for converting research into implementation,
keyed to the Vox wave structure.

### Immediate documentation actions (no code)

1. Create `docs/src/architecture/attention-budget-ssot.md` — SSOT for the full attention
   budget system, currently split across KI and code comments.
2. Create `docs/src/architecture/planning-meta/12-question-gate-standard.md` — Normative
   rules for when a planning request MUST trigger clarification before planning begins,
   vs. when it is safe to auto-expand or infer.
3. Update `information-theoretic-questioning.md`:
   - Add `assumption_confirm` to the question type taxonomy
   - Add the attention-budget → EIG threshold coupling table from §6.2
   - Add the structural question funnel from §4.2
   - Cross-reference this research document and the planning-meta gate standard
4. Update `plan-adequacy.md` rollout stage 5 to explicitly reference the question gate
   standard as the governance document for "questioning-first flows."

### Near-term implementation actions (code)

1. Add `evpi_question_threshold: f32` to `PlannerConfig` with a sensible default (0.15 bits).
2. Add a fourth outcome to the intake classification function: `RequiresClarification {
   question: QuestionSession }`.
3. Wire `AttentionBudget.focus_depth` to `evaluate_interruption` via a configurable
   gain multiplier (`interruption_calibration.focus_depth_gain_scale`).
4. Implement `assumption_confirm` question type as a named variant in the question-type
   enum and question-display layer.
5. Implement Bayesian posterior update for MC questions in `vox_questioning_submit_answer`.

### Verification criteria

A correct implementation of this research synthesis should satisfy:

- Zero planning sessions proceed past intake classification when `N_interpretations >= 2`
  AND `EVPI > evpi_question_threshold` (verified via `plan_sessions` audit)
- Mean clarification turns per resolved task ≤ 2.0 (metric: `question_sessions` table)
- Mean realized EIG per question ≥ 0.8 bits (requires posterior tracking)
- Zero `PolicyDeferred` questions that are re-issued within the same session (verifies
  belief state tracking)
- `FocusDepth::Deep` sessions have 0 non-critical questions emitted (attention budget
  coupling test)

archived_date: 2026-04-18
---

## Related documentation

- [`docs/src/reference/information-theoretic-questioning.md`](../reference/information-theoretic-questioning.md) — operational SSOT
- [`docs/src/reference/socrates-protocol.md`](../reference/socrates-protocol.md) — confidence gate and Ask decision
- [`docs/src/architecture/plan-adequacy.md`](plan-adequacy.md) — thin plan expansion policy
- [`docs/src/architecture/res_dynamic_agentic_planning_2026.md`](res_dynamic_agentic_planning_2026.md) — dynamic planning SOTA
- [`docs/src/architecture/planning-meta/04-planning-critique-gap-analysis.md`](planning-meta/04-planning-critique-gap-analysis.md) — planning gap analysis
- [`docs/src/architecture/planning-meta/05-anti-foot-gun-planning-standard.md`](planning-meta/05-anti-foot-gun-planning-standard.md) — anti-hazard planning standard

