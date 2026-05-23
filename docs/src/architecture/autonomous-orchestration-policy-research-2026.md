---
title: "Autonomous Orchestration Policy — Decision-Rule Research for Agent-to-Agent Behavior Switching (2026)"
description: "Research synthesis of when an orchestrator can autonomously change models, switch modes, enter planning, invoke research (Socrates), spawn sub-agents, escalate to HITL, and recover from runaway loops. Maps the external state-of-the-art onto Vox's existing routing/policy surface and proposes decision-rule contracts for the gaps."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-08"
authored: "2026-05-08"
training_eligible: true
training_rationale: "Cross-cutting policy reference for the orchestrator's autonomous behavior-switching surface. Sits between model-orchestration-ssot-audit-2026.md (routing mechanics), nextgen-orchestrator-research-2026.md (failure modes), orchestrator-companion-audit-findings-2026.md (non-routing surface), and ADR-005 (Socrates) / ADR-025 (multi-agent locks) / ADR-030 (state-machine SSoT). Names the specific decisions that should be rule-codified, what evidence sources exist for each rule, and the boundary between automatable and non-automatable behavior."
vox_relevance:
  - "vox-orchestrator: model routing, planning loop, doom-loop detection, mode switching, sub-agent dispatch"
  - "vox-socrates-policy: confidence gates, abstention, research-trigger thresholds"
  - "vox-populi: A2A handoff protocol, sub-agent dispatch over mesh"
  - "vox-db: telemetry feeding adaptive routers (model_scoreboard, llm_interactions)"
  - "vox-secrets: privacy-tier routing constraints"
  - "vox-skills: skill / sub-agent description-driven dispatch"
---

# Autonomous Orchestration Policy — Decision-Rule Research for Agent-to-Agent Behavior Switching (2026)

## Part 0 — Scope and Reading Order

**What this document is.** A research synthesis of *when* and *how* an orchestrator should autonomously change behavior on the user's behalf in an agent-to-agent system: model tier, planning mode, research invocation, sub-agent dispatch, retrieval, compaction, escalation. It catalogs the external state of the art per decision axis with citations, then maps each axis onto Vox's existing surface and isolates the decisions that can reasonably be automated today from those that cannot.

**What this document is *not*.** Not a routing mechanics audit (see [`model-orchestration-ssot-audit-2026.md`](model-orchestration-ssot-audit-2026.md)). Not a non-routing orchestrator surface critique (see [`orchestrator-companion-audit-findings-2026.md`](orchestrator-companion-audit-findings-2026.md)). Not a research deck on enterprise failure modes (see [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md)). Not an architecture decision (see ADRs in [`docs/src/adr/`](../adr/)). It assumes those have been read.

**Reading order if you are touching the orchestrator code.**
1. [`model-orchestration-ssot-audit-2026.md`](model-orchestration-ssot-audit-2026.md) — what the routing surface actually does today
2. [`docs/src/adr/005-socrates-anti-hallucination-ssot.md`](../adr/005-socrates-anti-hallucination-ssot.md) — confidence gating contract
3. [`docs/src/adr/030-state-machine-ssot.md`](../adr/030-state-machine-ssot.md) — mode/state machine
4. **This document** — the cross-cutting decision policy that ties them together
5. [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md) — where the field is going and what we're missing

---

## Part 1 — The Decision Surface

An orchestrator that automates behavior changes on the user's behalf is making a small, finite set of decisions on every turn. Each decision has *inputs* (signals it can observe), an *automatable core* (rules that can run without asking the user), and a *deferral edge* (where it must hand off to HITL). The surface is:

| # | Decision | Inputs available now | Frequency |
|---|---|---|---|
| **D1** | **Model tier**: which model handles this turn (Haiku / Sonnet / Opus / local / custom) | task category, predicted complexity, prior `model_scoreboard` outcomes, budget, sensitivity tag, prompt-cache prefix overlap | every LLM call |
| **D2** | **Planning mode**: act-immediately (ReAct), plan-then-execute, or extended-thinking | task length-of-horizon estimate, irreversibility, prior failures on similar tasks | per task |
| **D3** | **Research invocation (Socrates)**: trust the answer, sample more, retrieve, or escalate | grounding score, semantic entropy / self-consistency, abstention signals, source-citation quality | per claim or per turn |
| **D4** | **Sub-agent dispatch**: do inline, spawn one specialist, or fan out parallel | task decomposability, independent subtasks count, latency budget | per task |
| **D5** | **Mode switch**: autonomous ↔ interactive ↔ approval-required | risk dimensions of next action (irreversibility, blast radius, compliance), cumulative confidence, novelty | per action |
| **D6** | **Continue ↔ replan ↔ abort**: detect doom-loops and unproductive trajectories | repeated tool args, n-gram overlap on actions, semantic drift score, no-progress counter, iteration budget | every tool call |
| **D7** | **Context strategy**: continue, compact, snip, branch | token pressure %, salience of older turns, task-boundary signal | every N turns |
| **D8** | **Privacy routing**: which providers are eligible for *this* prompt | PII/secret detection, sensitivity tag, ZDR/on-prem requirement, jurisdiction | every LLM call |
| **D9** | **HITL escalation**: ask user, queue for review, or proceed | risk × confidence matrix, anomaly detection, time-budget exhaustion | event-driven |
| **D10** | **Adaptation**: update the router itself | reward signal (task success), preference feedback, drift detection | background |

The next nine sections survey what the field has learned about each axis. The last two sections fold the findings back into Vox: what is automatable today on Vox's existing telemetry, what is not, and the contract skeletons for the gaps.

---

## Part 2 — D1: Model-Tier Routing

### 2.1 The two dominant patterns

**Cascade routing.** Send the query to a cheap model first; only escalate to a stronger model when the cheap model's response fails a reliability check. The reliability check is a scoring function on (query, response) → [0, 1]; the cascade exits at the first index where the score crosses a threshold.

> "An incoming query first goes to a small LLM, and if the small model's confidence score is below a chosen threshold, the cascade forwards the query to a larger and more powerful LLM. When the confidence of a small model is high, the system can safely stop the inference, saving the most powerful model's inference cost while maintaining its accuracy." — *Bi-directional Model Cascading with Proxy Confidence* [^cascade-proxy]

FrugalGPT showed a cascade can match GPT-4 quality at **up to 98% lower cost** [^frugalgpt]; C3PO and GATEKEEPER are 2025 refinements that learn the threshold from a held-out set [^c3po][^gatekeeper].

**Predictive routing.** A small classifier looks at the prompt and predicts *which* model is best, in a single shot. RouteLLM (Berkeley/LMSYS) showed routing-by-classifier hits **95% of GPT-4 quality with 14–26% strong-model calls — a 75–85% cost reduction** [^routellm]. Hybrid LLM (ICLR 2024, Microsoft) used a DeBERTa router and routed **22–40% of queries to a smaller model with <1% quality drop** [^hybridllm]. Martian and Not Diamond commercialized this pattern; Not Diamond reports **7–15 point accuracy lift** when routing across multiple frontier models with consensus aggregation [^notdiamond].

**Online (bandit) routing.** Instead of a frozen classifier, a contextual bandit updates the policy from per-call outcomes. BaRP (2025) frames it as a contextual bandit conditioned on a user preference vector (accuracy / cost trade-off) and adapts online from binary feedback [^barp]. PILOT (2025) adds explicit budget constraints and preference priors [^pilot]. RouterArena (2025) is the standard benchmark [^routerarena].

### 2.2 The Claude tiering consensus

Independent of academic routers, the production consensus in 2026 for Anthropic-only stacks is the **three-tier orchestrator**:

> "Haiku 4.5 serves as the router, classifying incoming requests and handling simple ones directly. Sonnet 4.6 processes the bulk of medium-complexity tasks — code generation, document analysis, and data extraction. Opus 4.6 handles the 10 to 15% of requests that require deep reasoning or complex multi-step problem solving. … This approach can reduce total API costs by 60 to 70% compared to using Opus for everything — without any meaningful quality compromise." — Augment Code, *AI Model Routing Guide* [^augment-routing]

Vox already implements the *mechanism* of tier-based routing (`crates/vox-orchestrator/src/models/registry.rs::best_for()`, `scoring.rs`); what is missing is the **complexity classifier** — the function `prompt → predicted_tier` that the routing decision depends on. Today this is a hand-tuned `StrengthTag × TaskCategory` table. The literature suggests three replacement options, ranked by maturity:

1. **Rule-based prefilter on regex/keyword features** — simplest, deployable today on existing telemetry. ~70% of public router work uses this as a layer-1 filter even when ML-based routing is layered on top [^three-tier-cascade].
2. **Small-classifier router (BERT/DeBERTa)** — needs labeled data; the labels can come from `model_scoreboard` outcomes (which already exist in the v59 schema). One offline training pass per quarter.
3. **Contextual bandit** — needs the binary "did this task succeed" signal, which Vox has via `llm_interactions.context_utilization_pct` and `socrates` outcomes. Strongest long-term, weakest day-one.

### 2.3 Decision rule (proposed for Vox)

```rust
fn select_model_tier(task) -> ModelTier {
    if task.has_pii or task.sensitivity == Critical:
        return restrict_to(privacy_eligible_providers())  // see Part 9
    if task.budget_remaining < cheapest_strong_model_cost:
        return cheapest_provider_meeting_quality_floor()
    let predicted = router.predict(task)        // BERT classifier or rules
    if predicted.tier == Cheap:
        return Cheap                            // direct routing path
    if cascade_enabled:
        return Cascade(start=Cheap, escalate_threshold=0.65)  // FrugalGPT path
    return predicted.tier
}
```

The escalation threshold (0.65) is an *empirical hyperparameter*; see §10.4 for how to tune it from `model_scoreboard`.

---

## Part 3 — D2: Planning Mode (Plan-Execute vs ReAct vs Extended Thinking)

### 3.1 The pattern matrix

| Pattern | When | Cost | Inspectable? | Source |
|---|---|---|---|---|
| **ReAct** (think → act → observe) | exploratory, real-time, you can't list the tools up front | low–medium | partial | [^react-vs-plan][^oracle-react] |
| **Plan-and-Execute** | you can list the tools up front; cost-of-wrong-turn high | low (planner once + cheap executor) | yes — plan is a reviewable artifact | [^langgraph-pe][^plan-execute-medium] |
| **Tree-of-Thoughts** | combinatorial / search problem with intermediate scorer | **10–100×** CoT | yes (search tree) | [^tot-ibm][^tot-stanford] |
| **Reflexion** (verbal RL) | repeated trials on same task class; learn from failure | medium per trial | partial | [^reflexion-paper][^reflexion-pattern] |
| **Extended thinking** (Anthropic adaptive) | one expensive decision step (tool pick, refactor under constraint) | medium–high | thinking trace | [^anthropic-thinking][^anthropic-adaptive] |

**Numbers worth memorizing.**
- Plan-and-Execute hits **92% task completion** vs ReAct's 85% on multi-step workflows [^plan-execute-arch].
- ToT spends **10–100×** the tokens of CoT for the *same* answer when there is no useful intermediate scorer [^tot-stanford]. This is the failure mode of "always use ToT for hard tasks."
- Adaptive computation papers show **20–60% reduction in thinking tokens at no quality loss** when the model picks its own budget [^adaptive-compute][^learning-how-hard].

### 3.2 The trigger rule

The literature converges on a question rather than a threshold: **can you list the tools that will be called before execution starts?**

- **Yes** → Plan-and-Execute. The plan is the artifact you can review, parallelize independent steps, and re-plan on failure with a structured score-and-replan loop (see LangGraph PEV [^pev-template]).
- **No, but the task is one expensive decision** (pick the right tool with the right parameters; refactor under a strict constraint) → Extended thinking. Anthropic's guidance is *do not default-enable*; pick the 2–3 steps where a wrong choice cascades and turn it on there [^anthropic-thinking].
- **No, exploratory** → ReAct.
- **No, but you are repeating the same task class** (debugging the same kind of bug, generating tests for the same kind of function) → Reflexion-on-top-of-ReAct so the agent accumulates verbal lessons across trials [^reflexion-paper].

### 3.3 What this means for Vox

`crates/vox-orchestrator/src/mcp_tools/chat_tools/plan.rs` and `plan_loop.rs::maybe_refine_plan()` already implement the *mechanism*; the trigger ("when do I plan vs just act?") is a hardcoded heuristic today. The proposed rule:

```rust
fn pick_planning_mode(task) -> Mode {
    if task.estimated_steps >= 4 and task.tools_predictable:
        return PlanThenExecute    // 92% vs 85% lift
    if task.has_irreversible_side_effects:
        return PlanThenExecute    // plan is reviewable artifact
    if task.requires_constraint_satisfaction:
        return ExtendedThinking(budget=8192)
    if task.is_repeated_class and reflexion_memory.exists():
        return ReActWithReflexion
    return ReAct                   // default
}
```

`task.tools_predictable` is itself a small classifier or a structural check ("does the task description name specific tools?").

---

## Part 4 — D3: Research / Socrates Invocation

This is the hardest of the ten decisions, because the orchestrator has to ask: "do I trust my own answer enough to ship it?" The literature has produced four distinct signals for this. The right policy is to fuse them, because no single one is reliable.

### 4.1 The four trust signals

**Signal A: Token logprobs.** The model's own per-token confidence. Simple, cheap, and produced for free by every supporting API. The catch: token probability **conflates factual confidence with lexical uncertainty** ("Paris" vs "the capital of France" can both be right but one has lower token prob); naive thresholds miscalibrate [^verbalized-vs-logprob]. LogU / LogTokU (2025) extracts uncertainty from logits *without* multi-sampling, fixing the "loss of evidence strength" failure mode of vanilla probability methods [^logu].

**Signal B: Self-consistency.** Sample N responses at high temperature; measure their mutual agreement. If the model "knows" the answer, samples align; if it's confabulating, they diverge. SelfCheckGPT is the reference implementation [^selfcheckgpt]. **Limit:** if the model is *confidently wrong*, all samples agree on the same wrong answer — high consistency but zero correctness [^consistency-key].

**Signal C: Semantic entropy.** Cluster N samples by *meaning* (entailment-equivalent), then compute entropy over clusters rather than over surface strings. Farquhar et al. (Nature 2024) showed this beats token-level entropy for confabulation detection. Semantic Entropy Probes (NeurIPS 2024) approximate it from a *single* generation's hidden states for a 5–10× cost reduction [^semantic-entropy-nature][^semantic-entropy-probes]. Semantic Energy (2025) beats Semantic Entropy by **+13% AUROC** [^semantic-energy].

**Signal D: Verbalized confidence.** Just ask the model "how sure are you, 0–1?" Empirical results conflict: some studies find verbalized scores *better calibrated* than logprobs; others find ECE > 0.377 (catastrophic miscalibration) [^verbalized-confidence-paper]. The gap is almost entirely **prompt design** — there are prompt formats that produce well-calibrated verbalized scores and prompt formats that don't.

### 4.2 The fused-signal pattern

The 2026 production consensus is to **fuse 2–3 signals** rather than rely on one:

> "Initial confidence thresholds are set conservatively — 0.85 for irreversible actions and 0.70 for reversible actions, and after 30 days of production data, thresholds are recalibrated based on Expected Calibration Error (ECE) and adjusted to achieve a target false-positive rate matching reviewer capacity." — *Human-in-the-Loop AI Agents* [^hitl-medium]

Translated to orchestrator terms: compute a composite score per claim that combines logprob entropy, SE-probe estimate, and (for high-risk claims) a fresh self-consistency check. Define **two thresholds**, one for "ship the answer" and one for "must escalate," with a middle band for "invoke research."

### 4.3 When to invoke deeper research

Once the orchestrator decides "I don't trust this answer," it has three follow-on options:

1. **Re-sample at higher temperature** with self-consistency check (cheap, ~3× cost).
2. **Retrieve** — adaptive RAG. Self-RAG learns to emit a "retrieve" reflection token when confidence is low [^self-rag]; the threshold is **tunable at inference time** for accuracy / cost trade-off.
3. **Spawn a Socrates research agent** that asks structured follow-up questions and gathers evidence (Princeton SocraticAI's Socrates/Theaetetus/Plato pattern) [^socratic-princeton]. Most expensive; reserve for high-stakes / low-confidence intersections.

The escalation order matters: cheap-then-expensive saves cost on the long tail of medium-confidence claims.

### 4.4 Mapping to Vox

ADR-005 already names this surface: `vox-socrates-policy`, `RiskDecision::Abstain`, `ConfidencePolicy`, `RiskBand`. What is missing is the **fusion function** — today the decision is a single heuristic gate, not a composite of logprobs + entropy + self-consistency. The composite is implementable today on the `llm_interactions` schema (v59) once two columns are added: per-call entropy estimate (LogU or SEP) and per-call sample-disagreement score for high-stakes calls.

```rust
fn should_invoke_research(claim) -> ResearchAction {
    let score = fuse(claim.logprob_entropy, claim.sep_estimate, claim.self_consistency);
    if score >= ship_threshold:           return Ship
    if score >= research_threshold:       return ReSample(temp=0.7, n=5)
    if score >= retrieve_threshold:       return Retrieve(adaptive_rag)
    if score >= socrates_threshold:       return SpawnSocrates(claim)
    return Escalate                        // user must decide
}
```

The thresholds are calibration parameters tuned per-task-category from `model_scoreboard` history.

---

## Part 5 — D4: Sub-agent Dispatch (When to Spawn vs Inline)

### 5.1 The frameworks landscape

Six concrete patterns in production use:

| Pattern | Where | Trigger to spawn |
|---|---|---|
| **Supervisor** (clear control flow, one router node) | LangGraph, OpenAI Agents SDK | description-driven: subagent's `description` field matches subtask [^langgraph-supervisor][^anthropic-subagents] |
| **Swarm** (peer-to-peer handoffs) | LangGraph swarm, OpenAI Swarm | `transfer_to_X` tool call from current agent [^openai-swarm-cookbook] |
| **Hierarchical / manager-worker** | CrewAI hierarchical | `allow_delegation=True` + complexity decomposable into specialist domains [^crewai-hierarchical] |
| **Selector group chat** (LLM picks next speaker) | AutoGen `SelectorGroupChat` | model picks based on agent descriptions + current state; constrained by `allowed_or_disallowed_speaker_transitions` [^autogen-selector] |
| **Parallel fan-out** | Anthropic multi-agent research | independent subtasks with no shared mutable state [^anthropic-multi-agent] |
| **Cross-vendor A2A** | Google Agent2Agent protocol | Agent Cards advertise capabilities; orchestrator queries cards to find specialist [^a2a-protocol][^a2a-spec] |

### 5.2 The decision rule the field converged on

> "The orchestration layer decomposes incoming requests into executable subtasks and assigns them to the most suitable agent based on capability, context, and real-time system state." — N-iX, *AI Agent Orchestration* [^nix-orchestration]

> "Use hierarchical delegation when you have **complex, multi-faceted problems** that benefit from task decomposition and specialist agents rather than trying to handle everything with a single agent." — ActiveWizards, *CrewAI Delegation Guide* [^crewai-delegation]

In practice: **spawn a sub-agent when (a) the subtask is independent of the main agent's running state, AND (b) there is a specialist whose description matches the subtask, AND (c) the parallelism saves wall-clock time.** Otherwise, inline.

The Anthropic pattern is description-driven: "When you define subagents, Claude determines whether to invoke them based on each subagent's description field" [^anthropic-subagents]. Vox already has this surface in `crates/vox-skills/skills/*.skill.md` — every skill *is* a sub-agent description.

### 5.3 Anti-pattern: chain length

The cumulative reliability of an agent chain is the *product* of per-agent reliability. A 5-agent chain at 95% per-agent ≈ 77% overall. The literature explicitly flags this as a HITL trigger: **"multi-agent chain complexity where compound uncertainty across autonomous agent handoffs degrades cumulative reliability"** [^hitl-strata]. Vox's [`multi-agent-vcs-replication-spec-2026.md`](multi-agent-vcs-replication-spec-2026.md) implicitly handles this with state-recovery checkpoints; the *trigger* — "this chain is now too long, fork to HITL" — is not yet in the policy.

---

## Part 6 — D5/D6: Mode Switching and Doom-Loop Detection

### 6.1 The four risk dimensions

The HITL/autonomy literature has converged on four-dimensional risk scoring [^hitl-medium]:

1. **Irreversibility** — can the action be undone?
2. **Blast radius** — how many people / records does it affect?
3. **Compliance exposure** — does it create legal or regulatory obligations?
4. **Confidence** — how certain is the agent?

The product (or weighted sum) of (1, 2, 3) × (1 - 4) is a risk score that maps to autonomy level. This is the **AURA** framework's contribution: *parse, score, gate high-impact actions according to predefined thresholds, with optional HITL review for uncertain cases* [^aura].

The EU AI Act (Article 14, August 2026 enforcement) makes a HITL surface **legally required** for any high-risk AI system [^eu-ai-act-hitl]. Vox is below that threshold for most use cases but needs the mechanism in place to *enable* it for users who deploy in those domains.

### 6.2 Doom-loop detection: what triggers the circuit breaker

The production consensus on circuit breaker conditions [^circuit-breaker][^paperclip-issue]:

| Signal | Threshold (typical) | Source |
|---|---|---|
| **No file/state change for N consecutive loops** | N=3 | [^ralph-claude] |
| **Same error message N consecutive loops** | N=5 | [^ralph-claude] |
| **Output decline** (response getting shorter / lower quality) | >70% reduction | [^ralph-claude] |
| **Tool call count without state progression** | >15 | [^ai-agent-failure] |
| **Repeated tool-args** (Jaccard / n-gram similarity on action) | n=4 grams; >0.85 cosine on action embeddings | [^trajectory-ngram][^doom-loop-medium] |
| **Semantic drift from baseline** (Sentence-BERT distance to canary) | task-specific; needs baseline | [^semantic-drift-detect] |
| **Hard turn cap** | 30–50 turns / `max_iterations` | [^langchain-max-iters] |

The *graduated warning* pattern (NousResearch hermes-agent) is worth copying: a CAUTION tier 10 turns before the cap and a WARNING tier 3 turns before, so the agent can wrap up cleanly instead of hitting the wall [^iteration-budget-pressure].

### 6.3 Mapping to Vox

[`orchestrator-companion-audit-findings-2026.md`](orchestrator-companion-audit-findings-2026.md) FIX-B-11 names the doom-loop detector as a P1 gap. The literature gives us an executable spec:

```rust
struct CircuitBreaker {
    no_progress_count: u32,         // increments when state hash unchanged
    same_error_count: u32,
    output_decline_ratio: f64,
    tool_calls_without_progress: u32,
    action_n4gram_history: RingBuffer<NgramSet>,
    drift_baseline: Embedding,
}

impl CircuitBreaker {
    fn should_trip(&self) -> Option<TripReason> {
        if self.no_progress_count >= 3 { return Some(NoProgress) }
        if self.same_error_count >= 5 { return Some(StuckOnError) }
        if self.tool_calls_without_progress > 15 { return Some(ToolThrash) }
        if self.action_repetition_score() > 0.85 { return Some(ActionLoop) }
        if self.semantic_drift() > drift_threshold { return Some(Drifting) }
        None
    }
}
```

The trip action is *not* "abort" — it is **"hand to replanner with the trip reason in the prompt, and if replanning also fails within K attempts, escalate to HITL with a partial-progress report."** This matches the LangGraph PEV pattern [^pev-template].

### 6.4 Mode-switching trigger

The cleanest framing in the literature is **"governed autonomy"** [^governed-autonomy]: the agent runs autonomously inside a defined operating envelope; outside the envelope it switches to interactive. The envelope is defined by the four risk dimensions above. The 2026 conservative defaults seen in multiple sources [^hitl-medium]:

- **Irreversible action + confidence < 0.85** → require approval
- **Reversible action + confidence < 0.70** → notify but proceed
- **Blast radius > N records** (N depends on tenant) → require approval
- **Compliance-tagged action** (PII, financial, regulated) → require approval *regardless* of confidence

Earn-based expansion is the discipline: start strict, log every (recommendation, decision, outcome), shift from prior-approval to after-the-fact review only after sustained alignment is demonstrated [^governed-autonomy].

---

## Part 7 — D7: Context Pressure and Compaction

### 7.1 The threshold question

Anthropic's Claude Cookbook exposes a configurable threshold; the **default is 0.9 (90%) of the context window** [^anthropic-compaction-cookbook]. Microsoft Agent Framework's compaction default is **50%** [^ms-compaction]. Why the gap?

Because the *right* trigger isn't "I'm running out of room" — it's **"I'm at a natural task boundary"** [^autonomous-context-compression]:

> "It is not ideal to compact when you're in the middle of a complex refactor; It is better to compact when you are starting a new task or otherwise believe that prior context will lose relevance." — *Autonomous Context Compression*, LangChain blog

The 95%-trigger pattern degrades performance *before* compaction fires [^cursor-context]. The agent-driven pattern (give the agent a `compact_context` tool it calls proactively) outperforms threshold-driven on long-horizon tasks [^anthropic-compaction-cookbook].

### 7.2 The five-layer pipeline

Multiple production agents converge on a layered strategy [^cursor-context]:

1. **Budget reduction** — truncate oversized individual tool outputs
2. **Snip** — drop turns older than a window
3. **Microcompact** — summarize within-call state to free cache
4. **Context collapse** — summarize long histories to a structured digest
5. **Auto-compact** — full semantic compression as a last resort

Each layer fires at a different pressure level. The orchestrator's job is to pick the layer, not to skip straight to (5).

### 7.3 Cache-aware routing — the missing dimension

Routing today optimizes for {capability, cost, latency}. Cache state is rarely a routing input, but it should be: **prompt caching saves up to 90% per cached token** [^anthropic-prompt-caching][^prompt-caching-savings]. If two providers can both handle a query, prefer the one whose cache contains the longest matching prefix. vLLM's router and SGLang's PrefixCacheAffinityRouter formalize this: "consistent hashing ensuring that requests with the same routing key are routed to the same worker replica, maximizing KV cache reuse" [^vllm-router][^sglang-prefix].

For Vox this is a **net-new routing dimension**. `nextgen-orchestrator-research-2026.md` §7.3 names it as P2; the cited papers give it concrete shape: maintain a per-provider approximate radix tree keyed on prompt prefix; route to the worker with the longest match unless capability/budget overrides.

### 7.4 Per-tenant budget isolation

Per-tenant token-and-spend caps (`orchestrator-companion-audit-findings-2026.md` FIX-F-05) need three things, per the gateway literature [^per-tenant-rate-limit][^hierarchical-budget]:

1. **In-memory budget tracking** at every request — no per-call DB hit
2. **Hierarchical buckets** — per-tenant and per-app inside it
3. **Token-based, not request-based** — request-based does not capture variable cost per call

---

## Part 8 — D8: Privacy / Sensitivity Routing

### 8.1 The gateway pattern

Production deployments converge on an **AI-Gateway-shaped** PII boundary [^pii-aware-routing][^bp-ai-gateway]:

> "When text enters an AI Gateway, it's inspected for PII in real time, and if PII is identified, it's automatically rerouted to a more secure, on-premises model instead of a cloud-hosted one."

The detection layer is two-pass: **regex/dictionary patterns** (deterministic, fast, ~80% recall) layered with an **ML PII classifier** (Microsoft Presidio, PII Tools, Nightfall) for the long tail [^presidio][^pii-tools]. Detected matches *elevate* the sensitivity tag (e.g., a note containing medical keywords moves from `Medium` → `Critical` even if the surface string lacked a regex match).

### 8.2 The routing decision

Once a sensitivity tag is set, the routing decision is a **filter, not a scorer**:

```rust
fn privacy_eligible_providers(task) -> Vec<Provider> {
    match task.sensitivity {
        Public | Internal => all_providers(),
        Confidential => providers_with_zdr(),
        Restricted | Critical => self_hosted_only(),
    }
}
```

Composing with §2: pick the highest-quality model in the eligible set, not the highest-quality model overall.

### 8.3 The three-layer guardrail

The 2026 consensus on guardrails [^langchain-guardrails][^datadog-guardrails][^wiz-guardrails]:

- **Input guardrails** — prompt injection / jailbreak detection, PII redaction *before* the model sees the prompt
- **Output guardrails** — PII / secrets / toxicity scan *before* output reaches the user
- **Runtime guardrails** — tool-call authorization at the moment of invocation

Each layer combines rule-based (deterministic, fast) and ML-based (catches paraphrased attacks). Vox's `vox-secrets` already enforces secret-egress prevention; the missing piece is **input PII detection** that *informs the routing decision* (not just blocks it).

---

## Part 9 — D9: HITL Escalation

### 9.1 The risk × confidence matrix

Compose Part 6's risk dimensions with Part 4's confidence signal into a 2D matrix [^hitl-medium][^aura]:

|  | Confidence ≥ 0.85 | 0.70 ≤ c < 0.85 | c < 0.70 |
|---|---|---|---|
| **Reversible, low blast** | proceed | proceed + log | notify + proceed |
| **Reversible, medium blast** | proceed + log | notify + proceed | escalate |
| **Irreversible, any blast** | notify + proceed | escalate | escalate |
| **Compliance-tagged** | escalate | escalate | escalate |

The matrix collapses to two thresholds: a *proceed* gate and an *escalate* gate. The middle band is "notify and proceed" — a low-friction in-band notification rather than a blocking modal.

### 9.2 The interrupt mechanism

LangGraph's `interrupt()` and `interrupt_before=["tools"]` are the cleanest reference implementation [^langgraph-interrupts][^langgraph-breakpoints]. Pattern:

- **Static breakpoint** — wrap risky nodes (financial transactions, deletes, external sends) with `interrupt_before`
- **Dynamic interrupt** — runtime call to `interrupt()` when the matrix above lands in "escalate"

The persistence layer captures full state at the interrupt; the user resumes via `Command(resume=...)` with the same `thread_id`.

### 9.3 Earned autonomy

Conservative defaults aren't permanent. The discipline [^governed-autonomy]:

1. Log every (recommendation, user decision, outcome) tuple.
2. After **30 days** of production data, recalibrate thresholds based on Expected Calibration Error (ECE).
3. Adjust to a **target false-positive rate matching reviewer capacity** — if reviewers ignore 90% of escalations, the threshold is too low.
4. Shift from prior-approval to after-the-fact review only after sustained alignment.

Vox has the data substrate (`llm_interactions`, `model_scoreboard`); the calibration loop is the missing automation.

---

## Part 10 — D10: Adaptation (Learning the Router)

### 10.1 What the bandit literature gives us

A frozen classifier-router degrades as model capabilities, prompt distributions, and pricing shift. Online learning closes the loop:

- **BaRP** — contextual bandit conditioned on user preference vector, learning from binary "good response?" feedback. **Single policy, multiple operating points** at inference [^barp].
- **PILOT** — bandit + budget constraint with online cost policy [^pilot].
- **Dueling bandits** — pairwise preference feedback (which response is better?), label-efficient [^dueling-bandits].

All three need only **binary or pairwise** outcome signals, which Vox already collects (`socrates` pass/fail, user thumbs, task completion).

### 10.2 The drift signal

A separate concern from "is the router good" is "is the router *still* good." The semantic-drift detection literature [^policy-drift-detector][^riva-drift] gives us a baseline: compute a Sentence-BERT embedding of typical-task responses; flag when current responses diverge by more than 2σ from baseline. The trip surface is **request retraining**, not model swap.

### 10.3 Telemetry standard

OpenTelemetry's GenAI Semantic Conventions (SIG since April 2024) standardize span/event shapes for LLM calls, agent steps, vector queries, token usage, and cost [^otel-genai][^otel-agent-spans]. Datadog began native support in v1.37; Grafana followed. Vox's v59 telemetry schema predates this standard but maps cleanly; an explicit conformance pass is a low-cost win and unblocks third-party observability.

### 10.4 Calibration loop

```text
loop every 24h:
    samples = sample_recent_completions(n=10000)
    for tier in [Cheap, Mid, Strong]:
        observed_quality = score(samples.filtered(tier))
        observed_cost = sum(samples.filtered(tier).cost)
        update_router_weights(tier, observed_quality, observed_cost)
    if drift_score(samples) > 2σ:
        emit_alert(RouterDriftDetected)
        flag_for_retraining()
```

This sits in `vox-orchestrator` as a background task; it does not block any user-facing call.

---

## Part 11 — Vox Mapping: Reasonably Automatable Today

These rules can be implemented on Vox's *current* surfaces (data, contracts, crates) without new external dependencies. Each cites the existing artifact it builds on.

| Decision | Automatable layer | Existing surface | Net-new code |
|---|---|---|---|
| **D1 model tier** | Three-tier (Cheap/Mid/Strong) routing with rule-based prefilter | `vox-orchestrator/src/models/registry.rs::best_for()` | Tier classifier (rule-based v1, BERT v2) |
| **D1 cascade** | Optional cascade for medium-confidence calls | Same | Confidence-gated escalation wrapper |
| **D2 plan-vs-act** | Step-count + irreversibility heuristic | `mcp_tools/chat_tools/plan.rs` | Trigger function `pick_planning_mode()` |
| **D3 confidence fusion** | Logprob-entropy + verbalized + per-claim self-consistency for stakes-tagged claims | `vox-socrates-policy`, ADR-005 | Two new columns on `llm_interactions`; fuse function |
| **D4 sub-agent dispatch** | Description-driven dispatch + chain-length cap | `vox-skills/skills/*.skill.md`, `vox-orchestrator` agent queue | Chain-length tracker, fanout decider |
| **D5 mode switch** | 4-dim risk score + autonomy envelope | ADR-030 (state machine SSoT) | `RiskScore` calculator, envelope config |
| **D6 doom-loop** | Five-signal circuit breaker (no-progress / same-error / tool-thrash / action-loop / drift) + graduated warnings | `orchestrator-companion-audit-findings-2026.md` FIX-B-11 | `CircuitBreaker` struct from §6.3 |
| **D7 context** | Five-layer pipeline triggered at distinct thresholds | `vox-orchestrator` compaction code | Per-layer trigger config; agent-driven `compact_context` tool |
| **D7 cache-aware** | Approximate radix tree per provider; route to longest-prefix match unless overridden | none yet (P2 in nextgen-orchestrator-research) | New routing dimension in `scoring.rs` |
| **D8 privacy** | Two-pass PII detection → eligible-provider filter | `vox-secrets` egress guard | Input-side detector + filter wrapper |
| **D9 HITL** | Risk×confidence matrix → interrupt or notify | none yet | Matrix evaluator + interrupt point |
| **D10 calibration** | Daily recalibration + drift alert | `model_scoreboard` v59 | Background calibration job |

---

## Part 12 — What Should NOT Be Automated

These decisions look automatable but the literature consistently warns against full automation. They belong in the HITL surface or in *advisory* mode.

1. **Ambiguous-intent disambiguation.** When the user's request is genuinely ambiguous (specification uncertainty, not model uncertainty), the literature is unanimous: **ask** [^ask-or-assume][^structured-uncertainty]. EVPI (Expected Value of Perfect Information) gives a calibrated cost-benefit for asking; do not paper over with assumptions.
2. **Compliance-tagged actions.** EU AI Act Article 14 makes HITL on high-risk actions *legally required*, not optional [^eu-ai-act-hitl]. No confidence threshold should bypass this — confidence and compliance are independent axes.
3. **Goal redefinition under semantic drift.** If the agent's interpretation of the goal is drifting, it cannot self-detect reliably (the drift detector does, but the *fix* is not "let the agent re-decide what the goal is") [^policy-drift-detector]. Hand back.
4. **Cross-tenant boundary changes.** Routing within a tenant's eligible-provider set is automatable; *changing* what's eligible (e.g., promoting a new provider into the privacy-restricted pool) is a configuration change that needs explicit operator intent.
5. **Money / external messaging.** Per CLAUDE.md and the agent autonomy literature, transactions and outbound messages are by-default-HITL regardless of confidence [^hitl-medium].
6. **Reasoning-fine-tuned model abstention.** AbstentionBench (NeurIPS 2025) found that **reasoning fine-tuning *degrades* abstention by 24%** on average [^abstentionbench]. Reasoning-tuned models that confidently answer unanswerable questions cannot be trusted to abstain on their own — the orchestrator must impose external abstention.
7. **Self-consistency-only confidence on confidently-wrong claims.** Self-consistency reports high confidence when the model is uniformly wrong. Never use it as a *single* signal — fuse it [^consistency-key].

---

## Part 13 — Proposed Decision-Rule Contracts

These are skeleton schemas, not full proposals — a starting point for ADRs. Each is one YAML/Rust file that becomes the SSoT for one decision.

### 13.1 `contracts/orchestration/tier-routing.v1.yaml` (D1)

```yaml
version: 1
classifier:
  type: rule_based         # or "bert" once a model is trained
  rules:
    - if: prompt.length < 200 and prompt.tools_named == 0
      tier: cheap
    - if: prompt.contains_code_block and prompt.language in [rust, ts, py]
      tier: mid
    - if: prompt.has_keyword(["prove", "derive", "design"])
      tier: strong
cascade:
  enabled: true
  start_tier: cheap
  escalate_threshold: 0.65       # tunable; see model_scoreboard calibration
  max_escalations: 1
budget_floor:
  enforce: true                  # block strong tier if budget < cheapest_strong_cost
```

### 13.2 `contracts/orchestration/risk-confidence-matrix.v1.yaml` (D5/D9)

```yaml
version: 1
risk_dimensions:
  irreversibility:                 # boolean
  blast_radius:                    # int — records or users affected
  compliance_tag:                  # enum: none | pii | financial | regulated
confidence_thresholds:
  proceed: 0.85
  notify_and_proceed: 0.70
  escalate: 0.0
matrix:
  - {irreversible: false, blast: <10,  compliance: none}: proceed_at_0.0
  - {irreversible: false, blast: <100, compliance: none}: proceed_at_0.70
  - {irreversible: true,  blast: any,  compliance: any }: notify_at_0.85_else_escalate
  - {irreversible: any,   blast: any,  compliance: pii_or_higher}: always_escalate
calibration:
  recalibrate_every_days: 30
  signal: ece_on_logged_decisions
```

### 13.3 `contracts/orchestration/circuit-breaker.v1.yaml` (D6)

```yaml
version: 1
trips:
  no_progress_loops:        3
  same_error_loops:         5
  tool_calls_no_progress:  15
  action_ngram_overlap:   0.85   # 4-gram cosine
  semantic_drift_sigma:   2.0
  hard_turn_cap:           50
warnings:
  caution_at_remaining:    10    # turns
  warning_at_remaining:     3
trip_action: handoff_to_replanner
replanner_max_retries: 2
on_replanner_failure: escalate_to_hitl
```

### 13.4 `contracts/orchestration/socrates-fusion.v1.yaml` (D3)

```yaml
version: 1
signals:
  logprob_entropy:    {weight: 0.4, source: llm_interactions.logprob_entropy}
  sep_estimate:       {weight: 0.4, source: llm_interactions.sep_estimate}
  self_consistency:   {weight: 0.2, source: per_claim_resample, fire_when: stakes >= medium}
thresholds:
  ship: 0.80
  resample: 0.65
  retrieve: 0.50
  spawn_socrates: 0.30
  abstain: 0.0
abstention_override:
  if_compliance_tagged: always_require_explicit_evidence
  if_user_disabled_socrates: never_abstain_silently   # surface uncertainty instead
```

These are starting points; each will need an ADR.

---

## Part 14 — Open Questions / Research Gaps

Items the literature does not answer cleanly and that need primary investigation in Vox:

1. **The complexity classifier ground-truth.** RouteLLM-style classifiers are trained from preference data (Chatbot Arena). What is the equivalent for Vox? Best candidate today: `model_scoreboard` outcomes filtered for high-quality verifiers, but the noise floor is unknown.
2. **Logprob availability across providers.** OpenRouter does not consistently surface logprobs from all backends; the `LogU` family of techniques degrades to verbalized-only on those calls. The fusion function in §13.4 needs a fallback path.
3. **Cache-prefix radix tree at scale.** vLLM/SGLang's structures are designed for a single inference cluster's KV cache. Vox routes across multiple external providers — the radix tree becomes a *prediction* of likely cache state, not ground truth. Calibration unknown.
4. **Drift-of-the-drift-detector.** Sentence-BERT embeddings themselves shift across model versions; the canary-baseline pattern needs a re-baselining cadence which the literature does not specify.
5. **Mesh agent chain-length cap.** Single-agent chain-length advice (§5.3) does not directly translate to populi-mesh A2A delivery, where the chain is partly determined by network topology. ADR-025 names lock coherence but not chain depth.
6. **Reasoning-tuned abstention failure mode.** AbstentionBench's 24% degradation [^abstentionbench] applies to reasoning-fine-tuned models; this is the class Vox routes to most often for hard tasks. External abstention enforcement (the orchestrator imposing "I don't know" when the model overclaims) is undocumented in the framework literature.
7. **The earn-back path for autonomy.** Conservative defaults are well-described; the *automated* expansion path (when does the orchestrator promote a class of decisions from "always escalate" to "notify-and-proceed"?) is described qualitatively but no production implementation publishes thresholds.

---

## Part 15 — How This Document Relates to Existing Vox Documents

| Existing | Relationship |
|---|---|
| [`model-orchestration-ssot-audit-2026.md`](model-orchestration-ssot-audit-2026.md) | This doc consumes routing **mechanics** from there; it adds the **decision rules** that drive the mechanics. |
| [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md) | This doc converts that doc's **failure modes** into **decision-rule contracts**. |
| [`orchestrator-companion-audit-findings-2026.md`](orchestrator-companion-audit-findings-2026.md) | This doc gives **specs** (e.g., circuit breaker §6.3) for several P1 gaps that doc only **names**. |
| [`docs/src/adr/005-socrates-anti-hallucination-ssot.md`](../adr/005-socrates-anti-hallucination-ssot.md) | This doc **operationalizes** Socrates by defining the fusion function (§4) and the trigger thresholds (§13.4). |
| [`docs/src/adr/025-multi-agent-lock-coherence.md`](../adr/025-multi-agent-lock-coherence.md) | This doc adds **chain-length and handoff** decision rules on top of that doc's lock mechanics. |
| [`docs/src/adr/030-state-machine-ssot.md`](../adr/030-state-machine-ssot.md) | This doc supplies the **risk × confidence matrix** that drives the state-machine's mode transitions. |
| [`multi-agent-vcs-replication-spec-2026.md`](multi-agent-vcs-replication-spec-2026.md) | Sub-agent dispatch logic in §5 is consistent with that doc's handoff protocol. |
| [`populi-mesh-a2a-durability-spec-2026.md`](populi-mesh-a2a-durability-spec-2026.md) | A2A handoff in §5.1 maps to that doc's transport. |
| [`telemetry-driven-cost-accounting-research-2026.md`](telemetry-driven-cost-accounting-research-2026.md) | The calibration loop in §10.4 reads from the telemetry surface that doc defines. |
| [`planning-meta/00..12`](planning-meta/) | Plan-mode trigger in §3.3 fits inside that planning framework's exception policy. |

This document is **horizontal**: it crosses the routing/policy/observability/security boundaries that the others draw vertically. New ADRs proposed here (§13) should each cite this doc as research foundation.

---

## Part 16 — Citations

Citations are split into three categories: **academic** (peer-reviewed papers and arXiv preprints), **framework** (production-system documentation), and **industry** (analysis posts and platform blogs).

### Academic

[^cascade-proxy]: *Bi-directional Model Cascading with Proxy Confidence* (2025). https://arxiv.org/pdf/2504.19391
[^frugalgpt]: *FrugalGPT: How to Use Large Language Models While Reducing Cost and Improving Performance* (Chen et al., 2023). https://arxiv.org/abs/2305.05176
[^c3po]: *C3PO: Optimized Large Language Model Cascades with Probabilistic Cost Constraints for Reasoning* (2025). https://arxiv.org/html/2511.07396v1
[^gatekeeper]: *GATEKEEPER: Improving Model Cascades Through Confidence Tuning* (2025). https://arxiv.org/pdf/2502.19335
[^routellm]: *RouteLLM: Learning to Route LLMs from Preference Data* (Ong et al., 2024). https://arxiv.org/html/2406.18665v3 / https://github.com/lm-sys/RouteLLM
[^hybridllm]: *Hybrid LLM: Cost-Efficient and Quality-Aware Query Routing* (ICLR 2024). https://arxiv.org/abs/2404.14618
[^barp]: *Learning to Route LLMs from Bandit Feedback: One Policy, Many Trade-offs* (BaRP, 2025). https://arxiv.org/abs/2510.07429
[^pilot]: *Adaptive LLM Routing under Budget Constraints* (PILOT, 2025). https://arxiv.org/html/2508.21141v1
[^dueling-bandits]: *LLM Routing with Dueling Feedback* (2025). https://arxiv.org/html/2510.00841
[^routerarena]: *RouterArena: An Open Platform for Comprehensive Comparison of LLM Routers* (2025). https://arxiv.org/html/2510.00202v1
[^semantic-entropy-nature]: *Detecting hallucinations in large language models using semantic entropy* (Farquhar et al., Nature 2024). https://www.nature.com/articles/s41586-024-07421-0
[^semantic-entropy-probes]: *Semantic Entropy Probes: Robust and Cheap Hallucination Detection in LLMs* (NeurIPS 2024). https://arxiv.org/abs/2406.15927
[^semantic-energy]: *Semantic Energy: Detecting LLM Hallucination Beyond Entropy* (2025). https://arxiv.org/pdf/2508.14496
[^logu]: *Estimating LLM Uncertainty with Evidence* (LogU / LogTokU, 2025). https://arxiv.org/abs/2502.00290
[^selfcheckgpt]: *SelfCheckGPT* — discussed in *Beyond Self-Consistency in Black Box Hallucination Detection* (2025). https://www.arxiv.org/pdf/2502.15845
[^consistency-key]: *Consistency Is the Key: Detecting Hallucinations in LLM Generated Text* (2025). https://arxiv.org/html/2511.12236
[^verbalized-confidence-paper]: *On Verbalized Confidence Scores for LLMs* (2024). https://arxiv.org/html/2412.14737v2
[^abstentionbench]: *AbstentionBench: Reasoning LLMs Fail on Unanswerable Questions* (NeurIPS 2025). https://arxiv.org/pdf/2506.09038
[^learning-how-hard]: *Learning How Hard to Think: Input-Adaptive Allocation of LM Computation* (2024). https://arxiv.org/abs/2410.04707
[^adaptive-compute]: *Reasoning on a Budget: A Survey of Adaptive and Controllable Test-Time Compute in LLMs* (2025). https://arxiv.org/html/2507.02076v1
[^reflexion-paper]: *Reflexion: Language Agents with Verbal Reinforcement Learning* (Shinn et al., NeurIPS 2023). https://github.com/noahshinn/reflexion
[^self-rag]: *Self-RAG: Learning to Retrieve, Generate and Critique through Self-Reflection*. https://selfrag.github.io/
[^socratic-princeton]: *The Socratic Method for Self-Discovery in Large Language Models* (Princeton NLP). https://princeton-nlp.github.io/SocraticAI/
[^aura]: *AURA: An Agent Autonomy Risk Assessment Framework* (2025). https://arxiv.org/html/2510.15739v1
[^trajectory-ngram]: *A Study of Thought-Action-Result Trajectories* (ASE 2025). https://software-lab.org/publications/ase2025_trajectories.pdf
[^semantic-drift-detect]: *Detecting Sleeper Agents in Large Language Models via Semantic Drift Analysis* (2025). https://arxiv.org/html/2511.15992
[^policy-drift-detector]: *I Built a Policy Drift Detector for LLM Agents.* DEV.to (2025). https://dev.to/gnomeman4201/i-built-a-policy-drift-detector-for-llm-agents-heres-what-four-versions-taught-me-2be
[^riva-drift]: *RIVA: Leveraging LLM Agents for Reliable Configuration Drift Detection* (2026). https://arxiv.org/pdf/2603.02345v1
[^ask-or-assume]: *Ask or Assume? Uncertainty-Aware Clarification-Seeking in Coding Agents* (2026). https://arxiv.org/abs/2603.26233
[^structured-uncertainty]: *Structured Uncertainty guided Clarification for LLM Agents* (2025). https://arxiv.org/html/2511.08798v1

### Framework / Production Documentation

[^langgraph-supervisor]: LangGraph Multi-Agent Supervisor. https://reference.langchain.com/python/langgraph-supervisor
[^langgraph-pe]: LangGraph Plan-and-Execute example. https://github.com/langchain-ai/langgraph/blob/main/examples/plan-and-execute/plan-and-execute.ipynb
[^langgraph-interrupts]: LangGraph Interrupts. https://docs.langchain.com/oss/python/langgraph/interrupts
[^langgraph-breakpoints]: LangGraph Static Breakpoints. https://langchain-ai.github.io/langgraph/cloud/how-tos/human_in_the_loop_breakpoint/
[^anthropic-thinking]: Anthropic — Building with Extended Thinking. https://platform.claude.com/docs/en/build-with-claude/extended-thinking
[^anthropic-adaptive]: Anthropic — Adaptive Thinking. https://platform.claude.com/docs/en/build-with-claude/adaptive-thinking
[^anthropic-prompt-caching]: Anthropic — Prompt Caching. https://www.anthropic.com/news/prompt-caching
[^anthropic-compaction-cookbook]: Anthropic Cookbook — Automatic Context Compaction. https://platform.claude.com/cookbook/tool-use-automatic-context-compaction
[^anthropic-subagents]: Anthropic — Subagents in the SDK. https://docs.anthropic.com/en/docs/claude-code/sdk/subagents
[^anthropic-multi-agent]: Anthropic — How we built our multi-agent research system. https://www.anthropic.com/engineering/multi-agent-research-system
[^openai-swarm-cookbook]: OpenAI Cookbook — Orchestrating Agents: Routines and Handoffs. https://cookbook.openai.com/examples/orchestrating_agents
[^autogen-selector]: Microsoft AutoGen — Selector Group Chat. https://microsoft.github.io/autogen/stable/user-guide/agentchat-user-guide/selector-group-chat.html
[^crewai-hierarchical]: CrewAI Agents documentation. https://docs.crewai.com/en/concepts/agents
[^crewai-delegation]: ActiveWizards — Hierarchical AI Agents: A Guide to CrewAI Delegation. https://activewizards.com/blog/hierarchical-ai-agents-a-guide-to-crewai-delegation
[^a2a-protocol]: A2A Protocol. https://a2a-protocol.org/latest/
[^a2a-spec]: Agent2Agent (A2A) Protocol Specification. https://a2a-protocol.org/latest/specification/
[^vllm-router]: vLLM Router Release Blog (2025). https://blog.vllm.ai/2025/12/13/vllm-router-release.html
[^sglang-prefix]: Prefix Caching — SGLang vs vLLM. https://medium.com/byte-sized-ai/prefix-caching-sglang-vs-vllm-token-level-radix-tree-vs-block-level-hashing-b99ece9977a1
[^otel-genai]: OpenTelemetry — Semantic conventions for generative AI systems. https://opentelemetry.io/docs/specs/semconv/gen-ai/
[^otel-agent-spans]: OpenTelemetry — Semantic Conventions for GenAI agent and framework spans. https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-agent-spans/
[^ms-compaction]: Microsoft Agent Framework — Compaction. https://learn.microsoft.com/en-us/agent-framework/agents/conversations/compaction
[^presidio]: Microsoft Presidio. https://github.com/microsoft/presidio
[^pii-tools]: PII Tools. https://pii-tools.com/
[^langchain-guardrails]: LangChain Guardrails. https://docs.langchain.com/oss/python/langchain/guardrails
[^pev-template]: *Building a Reliable LangGraph Workflow: Plan-Execute-Validate (PEV)*. https://dev.to/manjunathgovindaraju/building-a-reliable-langgraph-workflow-plan-execute-validate-pev-automated-retries-and-mcp-1pik

### Industry Analysis

[^augment-routing]: Augment Code — Best AI Model for Coding Agents in 2026: A Routing Guide. https://www.augmentcode.com/guides/ai-model-routing-guide
[^notdiamond]: VentureBeat — *Not Diamond automatically routes your query to the best LLM*. https://venturebeat.com/ai/not-diamond-automatically-routes-your-query-to-the-best-llm
[^three-tier-cascade]: MegaNova — *The 3-Tier Routing Cascade: Rule-Based → Semantic → LLM*. https://blog.meganova.ai/the-3-tier-routing-cascade-rule-based-semantic-llm/
[^react-vs-plan]: DEV.to — *ReAct vs Plan-and-Execute: A Practical Comparison*. https://dev.to/jamesli/react-vs-plan-and-execute-a-practical-comparison-of-llm-agent-patterns-4gh9
[^oracle-react]: Oracle Integration — *ReAct vs Plan & Execute*. https://blogs.oracle.com/integration/react-vs-plan-execute-choosing-the-right-agent-thinking-pattern-in-oracle-integration
[^plan-execute-medium]: Medium — *Built with LangGraph! #33: Plan & Execute*. https://medium.com/@okanyenigun/built-with-langgraph-33-plan-execute-ea64377fccb1
[^plan-execute-arch]: louisbouchard — *ReAct vs Plan-and-Execute: The Architecture Behind Modern AI Agents*. https://louisbouchard.substack.com/p/react-vs-plan-and-execute-the-architecture
[^reflexion-pattern]: Agent Patterns — Reflexion Agent Pattern. https://agent-patterns.readthedocs.io/en/stable/patterns/reflexion.html
[^tot-ibm]: IBM — What is Tree Of Thoughts Prompting? https://www.ibm.com/think/topics/tree-of-thoughts
[^tot-stanford]: *More Effectively Searching Trees of Thought*. https://web.stanford.edu/class/archive/cs/cs224n/cs224n.1244/final-projects/KamyarJohnSalahiPranavGurusankarSathyaEdamadaka.pdf
[^circuit-breaker]: DEV.to — *AI Agent Circuit Breakers*. https://dev.to/waxell/ai-agent-circuit-breakers-the-reliability-pattern-production-teams-are-missing-5bpg
[^paperclip-issue]: paperclipai/paperclip — *feat: Agent circuit breaker* issue. https://github.com/paperclipai/paperclip/issues/390
[^ralph-claude]: DEV.to — *ralph-claude-code: The Technology to "Stop" AI Agents*. https://dev.to/tumf/ralph-claude-code-the-technology-to-stop-ai-agents-how-the-circuit-breaker-pattern-prevents-3di4
[^ai-agent-failure]: MindStudio — *AI Agent Failure Pattern Recognition*. https://www.mindstudio.ai/blog/ai-agent-failure-pattern-recognition
[^doom-loop-medium]: Medium — *The Agent Loop Problem: When "Smart" Won't Stop*. https://medium.com/@Modexa/the-agent-loop-problem-when-smart-wont-stop-ccbf8489180f
[^iteration-budget-pressure]: NousResearch hermes-agent issue — *Iteration Budget Pressure*. https://github.com/NousResearch/hermes-agent/issues/414
[^langchain-max-iters]: LangChain GitHub — *Agent stopped due to iteration limit or time limit*. https://github.com/langchain-ai/langchain/discussions/27264
[^hitl-medium]: Medium — *Human-in-the-Loop AI Agents* (Anna Jey, 2026). https://medium.com/@arvisionlab/human-in-the-loop-ai-agents-how-to-add-approvals-escalation-and-safe-autonomy-in-production-0a21e359781c
[^hitl-strata]: Strata — *Human-in-the-Loop: A 2026 Guide to AI Oversight*. https://www.strata.io/blog/agentic-identity/practicing-the-human-in-the-loop/
[^governed-autonomy]: NIST AI Risk Management Framework — *Generative AI Profile (NIST AI 600-1)*. https://www.nist.gov/itl/ai-risk-management-framework
[^eu-ai-act-hitl]: Knowlee — *Human-in-the-Loop AI Policy Template (2026) + AI Act SLAs*. https://www.knowlee.ai/blog/human-in-the-loop-ai-policy-template
[^cursor-context]: Morph — *Cursor Context Window (2026)*. https://www.morphllm.com/cursor-context-window
[^autonomous-context-compression]: LangChain Blog — *Autonomous context compression*. https://blog.langchain.com/autonomous-context-compression/
[^prompt-caching-savings]: ProjectDiscovery — *How We Cut LLM Costs by 59% With Prompt Caching*. https://projectdiscovery.io/blog/how-we-cut-llm-cost-with-prompt-caching
[^per-tenant-rate-limit]: Truefoundry — *Rate Limiting in AI Gateway: The Ultimate Guide*. https://www.truefoundry.com/blog/rate-limiting-in-llm-gateway
[^hierarchical-budget]: DEV.to — *Building Hierarchical Budget Controls for Multi-Tenant LLM Gateways*. https://dev.to/pranay_batta/building-hierarchical-budget-controls-for-multi-tenant-llm-gateways-ceo
[^pii-aware-routing]: DEV.to — *PII-aware routing*. https://dev.to/micelclaw/pii-aware-routing-how-to-use-cloud-ai-and-keep-your-sensitive-data-local-1m40
[^bp-ai-gateway]: Blue Prism — *AI Gateway for PII Sanitization*. https://www.blueprism.com/resources/blog/ai-gateway-pii-sanitization/
[^datadog-guardrails]: Datadog — *LLM guardrails: Best practices*. https://www.datadoghq.com/blog/llm-guardrails-best-practices/
[^wiz-guardrails]: Wiz — *AI Guardrails: Safety Controls for Responsible AI Use*. https://www.wiz.io/academy/ai-security/ai-guardrails
[^nix-orchestration]: N-iX — *AI agent orchestration*. https://www.n-ix.com/ai-agent-orchestration/

---

*End of document.*
