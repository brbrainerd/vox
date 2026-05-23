---
title: "Information-theoretic questioning protocol"
description: "SSOT for when and how Vox asks clarifying questions with maximum diagnostic value per user effort."
category: "Language Reference"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "TechArticle"
---

# Information-theoretic questioning protocol

This document is the SSOT for clarification strategy across chat, planning, and agent-to-agent handoffs.

## Goals

- Minimize user effort while maximizing uncertainty reduction.
- Prefer high-diagnostic prompts over broad or redundant questions.
- Stop asking as soon as confidence and risk thresholds are met.
- Preserve auditability: each question has reason, expected gain, and stop rationale.

## Question trigger policy

Ask a question only when at least one of these conditions is true:

1. **Ambiguous intent**: multiple plausible actions exist with materially different outcomes.
2. **High consequence uncertainty**: action is costly, irreversible, or policy-sensitive.
3. **Missing hard constraint**: required parameter is absent (`target`, `scope`, `risk tolerance`, `deadline`, etc.).
4. **Socrates medium-risk band**: confidence is in the ask range and contradiction is non-blocking.

Do not ask when:

- the request is unambiguous and low risk,
- additional questions are expected to provide negligible information gain,
- maximum clarification turns or user-time budget is reached.

## Question type selection

Use the smallest interaction that resolves the highest-value uncertainty.

### Multiple-choice (`multiple_choice`)

Prefer when hypothesis space is known and bounded.

- Use 2-5 options (3 default).
- Options must be mutually exclusive when possible.
- Include a deliberate "other / none of the above" only when genuinely needed.
- Design unselected options to remain diagnostically useful (infer constraints/preferences).

### Assumption-confirm (`assumption_confirm`)

Prefer when agent confidence in its inferred value is ≥ 0.80 and the value is not
policy-sensitive or destructive.

- State the assumed value explicitly: *"I'm assuming X. Correct me if wrong; otherwise I'll proceed."*
- Include a default timeout: how long the agent waits before proceeding with the assumption.
- Include a brief impact note: what changes if the assumption is wrong.
- Do **not** use when the assumption is irreversible — use `multiple_choice` or `entry` instead.
- Anti-pattern: stating the assumption confidently without a clear correction mechanism (obsequiousness trap).

### Open-ended (`open_ended`)

Prefer when user intent space is broad or unknown.

- Ask exactly one targeted free-form prompt.
- Include a short frame to reduce interpretation variance.
- Follow with one narrow multiple-choice if remaining ambiguity persists.

### Entry (`entry`)

Prefer for scalar/structured fields (IDs, ranges, dates, file paths, thresholds).

- Validate format immediately.
- Echo parsed value before execution.
- Re-ask only for invalid/unsafe values.

## Information-theoretic scoring

Each candidate question is scored by expected value:

`score = expected_information_gain_bits / expected_user_cost`

Where:

- `expected_information_gain_bits` is entropy reduction over active hypotheses.
- `expected_user_cost` approximates burden (time, complexity, interruption).

Choose the highest-scoring candidate that passes policy constraints:

- `expected_information_gain_bits >= min_information_gain_bits`
- `expected_user_cost <= max_expected_user_cost`
- `clarification_turn_index < max_clarification_turns`

## Structural question funnel

High-diagnostic questioning follows a three-stage funnel. Each stage runs only if the
previous left material ambiguity.

1. **Intent** — Resolves the plan branch (`open_ended` or `binary`). Most tasks resolve here.
2. **Scope/constraint** — Resolves the execution envelope (`multiple_choice` or `entry`).
3. **Parameter confirm** — Confirms specifics for high-stakes or highly parameterized actions (`assumption_confirm` or `entry`).

For planning specifically:

1. Is the goal unambiguous with clear scope? → Plan without asking.
2. Does the goal map to N≥2 materially different plan shapes AND EVPI exceeds threshold? → Ask ONE disambiguating question. See `planning-meta/12-question-gate-standard.md`.
3. Is any high-risk step irreversible? → Confirm with `assumption_confirm` before that step executes.
4. Is the plan thin but the missing detail is specification-level (not intent-level)? → Auto-expand via `auto_expand_thin_plan`; ask only for genuine intent gaps.

## Stopping rules

Stop clarification when any condition is met:

1. `confidence >= target_confidence`
2. `marginal_information_gain_bits < min_information_gain_bits`
3. `clarification_turn_index >= max_clarification_turns`
4. `expected_user_cost > max_expected_user_cost`
5. contradiction/risk forces abstention or escalation

Persist stop reason explicitly for telemetry and audit.

## Attention and time-respect constraints

Questioning must be cost-aware with attention budget coupling:

- Penalize long clarification loops under high interrupt load.
- Raise gain threshold when attention budget is near exhaustion.
- Prefer concise multiple-choice in high temporal demand contexts.

### Attention budget → EIG threshold table

The EIG threshold for question approval scales with focus depth and budget state:

| Budget / focus state | EIG threshold adjustment | Permitted question types |
|---|---|---|
| `FocusDepth::Ambient`, spend < 50% | None (use configured baseline) | All types |
| `FocusDepth::Focused`, spend 50–80% | +20% | All types; prefer `multiple_choice` |
| `FocusDepth::Deep`, spend > 80% | +50% | `binary`, `assumption_confirm` only |
| `BudgetSignal::Critical` | Questions suppressed | None; proceed on best inference |
| `BudgetSignal::CostExceeded` | Questions suppressed | None; proceed on safe default |
| `interrupt_ewma > 0.8` | +50% (backlog penalty) | Defer non-critical; batch with next checkpoint |

MCP records estimated wall-time per `session_id` and can mirror those debits into the orchestrator global attention budget. Cap override and mirror toggle: **`VOX_QUESTIONING_MAX_ATTENTION_MS`**, **`VOX_QUESTIONING_MIRROR_GLOBAL_ATTENTION`** — see [Environment variables (SSOT)](env-vars.md#mcp-socrates-questioning).

### Dynamic interruption control (runtime)

When **`VOX_ORCHESTRATOR_ATTENTION_ENABLED=true`**, MCP **does not** emit every model-proposed question immediately. The orchestrator evaluates [`evaluate_interruption`](../../../crates/vox-orchestrator/src/attention/interruption_policy.rs) using:

- information gain vs. normalized user cost (same SSOT ratio),
- live [`AttentionBudget`](../../../crates/vox-orchestrator/src/attention/budget.rs) (spent ratio, focus depth / interrupt EWMA),
- trust, contradiction, risk band, open session hints, and turn caps.

Outcomes: **interrupt now** (persist question + [`AttentionEvent`](../../../crates/vox-orchestrator/src/attention/budget.rs)), **defer**, **batch with existing prompt**, or **proceed autonomously** (metric-only). High-risk / abstain-band cases can still **require human** before continue. Answered clarifications append **`ClarificationAnswered`** attention rows via `vox_questioning_submit_answer`. **`VOX_ORCHESTRATOR_ATTENTION_ENABLED=false`** keeps prior behavior (no dynamic deferral on this path).

Runtime now records policy-only outcomes (`PolicyDeferred`, `PolicyProceedAuto`) as first-class attention events, so calibration can learn from **suppressed** interruptions too (not only displayed prompts).

`Vox.toml` `[orchestrator]` can tune channel calibration via `interruption_calibration` (gain offsets, backlog penalty, trust-adjustment scale) without changing policy code.

Surface behavior differs:

- `vox_submit_task`: defer/proceed-auto record telemetry and continue submit; require-human blocks unless description carries explicit marker (`[approval:confirm]`, `[approval:reviewed]`, `[human-approved]`).
- `vox_a2a_send` (pilot-visible escalation types): defer/proceed-auto suppress send and return `deferred=true`; require-human blocks.
- `vox_a2a_send` (pilot-visible escalation types): defer suppresses send and returns `decision=DeferUntilCheckpoint` with `deferred=true`; proceed-auto suppresses send and returns `decision=ProceedAutonomously` with `deferred=false`; require-human blocks.
- `vox_plan`/`vox_replan`/`vox_plan_status`: defer/proceed-auto suppress only the questioning trace; plan output still returns.

## A2A clarification contract

For agent-to-agent clarification, persist these payload fields in `a2a_messages.payload`:

- `clarification_intent` (why clarification is needed),
- `hypothesis_set_id`,
- `question_kind`,
- `expected_information_gain_bits`,
- `expected_user_cost`,
- `requested_evidence_dimensions`,
- `urgency`,
- `stop_policy`.

Recommended `msg_type` values:

- `clarification_request`
- `clarification_response`
- `clarification_stop`

Contract schemas:

- [`contracts/communication/a2a-clarification-payload.schema.json`](../../../contracts/communication/a2a-clarification-payload.schema.json)
- [`contracts/communication/interruption-decision.schema.json`](../../../contracts/communication/interruption-decision.schema.json)

## Metrics (minimum set)

- Clarification trigger rate.
- Mean clarification turns per resolved task.
- Mean realized information gain per question.
- Gain-per-cost ratio.
- Multiple-choice option diagnostic power (selected + unselected).
- Clarification abandonment rate.
- Resolution latency after first clarification.
- A2A clarification round-trip latency.

## Persistence requirements

Policy and telemetry must be persisted in dual-write form:

1. Canonical publication artifact (`publication_manifests`).
2. Searchable mirror (`search_documents` + `search_document_chunks`).

Question-level runtime telemetry must be queryable in VoxDB via dedicated questioning tables.

**MCP (clients and agents):** `vox_questioning_pending` returns open sessions, unanswered assistant prompts, and structured multiple-choice options (plus parsed `belief_state_json`). `vox_questioning_submit_answer` persists free-text and optional `selected_option_id` (posteriors in `belief_state_json` and `question_options.posterior_probability` are updated for MC). Env vars for attention caps, global budget mirroring, and task-gate bypass are listed under [MCP / Socrates questioning](env-vars.md#mcp-socrates-questioning) in `env-vars.md`.

## Related SSOTs

- `docs/src/reference/socrates-protocol.md` — confidence gate and Ask decision
- `docs/src/reference/scientia-publication-worthiness-rules.md`
- `docs/src/reference/orchestration-unified.md`
- `docs/src/architecture/research-diagnostic-questioning-2026.md` — full research grounding (POMDP, EVPI, gap analysis, implementation roadmap)
- `docs/src/architecture/planning-meta/12-question-gate-standard.md` — Tier 1 normative rules for planning-mode questioning


