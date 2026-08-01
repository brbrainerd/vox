---
title: "Testing & Regression-Gating an Agent Harness — Verified Research 2026-07-30"
description: "Adversarially verified research into testing the harness rather than the model: why SWE-Bench-style benchmarks are contamination-compromised as regression gates, τ-bench's deterministic outcome-checking and pass^k reliability metric, LLM-as-judge fragility to stylistic artifacts, OpenTelemetry GenAI tracing as the observability substrate, and SkillOps as emerging skill-library governance — with the statistical sample-size and CI-canarying questions left explicitly open."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Testing & Regression-Gating an Agent Harness (2026-07-30)

> **Provenance.** `deep-research` run `wf_1824373c-6e8`: **106 agents, 7.4M subagent tokens,
> 0 errors** — a clean run, unlike batches 2–4. 23 sources fetched, 97 claims extracted, 25
> verified, **20 confirmed / 5 refuted**, synthesized to 5 findings.
>
> This answers the question you raised directly: **"adequate testing for what gets pushed into
> the system and what gets pushed out."** The headline result reframes the question rather than
> answering it as asked — the standard benchmarks used to claim an agent harness "works" are
> themselves compromised, so the first testing decision is *which* evaluation to trust before
> deciding *how much* of it to run.

Companions: [`skill-discovery-and-induction-research-2026-07-30.md`](skill-discovery-and-induction-research-2026-07-30.md)
(§4's promotion gate, which this document's SkillOps finding extends),
[`skill-registry-trust-and-curation-research-2026-07-30.md`](skill-registry-trust-and-curation-research-2026-07-30.md).

---

## 0. The reframe

The question "how do we adequately test an agent harness" implicitly assumes a trustworthy
ground-truth benchmark exists to test *against*. **The strongest finding in this research is
that the obvious candidate — SWE-Bench-style leaderboard scores — is not that ground truth.**
Two independent papers converge on the same mechanism: models are scoring well because they've
memorized the repos, not because they can navigate them.

This changes the shape of the answer. Testing a harness is not "run SWE-Bench and watch the
number." It is: (1) pick an evaluation architecture that cannot be gamed by memorization
(deterministic outcome-checking, §2), (2) treat any LLM-judge component as a measurement
instrument with known, uncorrected bias (§3), (3) instrument the harness so a regression is
*observable* at the trajectory level, not just the outcome level (§4), and (4) apply the same
discipline to the skill library specifically (§5).

---

## 1. SWE-Bench-style benchmarks are contamination-compromised (confirmed, high confidence)

Two independent primary papers converge on the same finding via different methods:

**Paper 1 (arXiv 2506.12286):** SOTA LLMs identify buggy file paths from **issue text alone**,
**with no repository access**, at up to **76% accuracy** on SWE-Bench-Verified. Accuracy drops
to **50–68%** on newer tasks from the *same* repos, and below **53%** on external repos (pandas,
PyTorch) never in SWE-Bench — tracking a **memorization gradient**, not a generalizable skill.
Critically: **73% (Verified), 79% (Full), and 97.4% (RefactorBench)** of task instances contain
**no explicit file-path mention** in the issue text. Above-chance localization on those
instances, with zero repo access, is not explicable except by prior/memorized knowledge.

**Paper 2 (arXiv 2512.10218):** Models localize files **~3× better** and identify edited files
**~6× better** on SWE-Bench-Verified than on **BeetleBox**, a comparable benchmark built from
similarly popular repos that were never in SWE-Bench. Concrete number: **Claude 3.5 hits 65% vs
12.2%** on issue-only-input file identification, same task shape, different corpus.

Both papers' own conclusion, quoted: scores *"may not reflect true agent ability on real
software issues."*

**Direct consequence for Vox:** the audit's finding **F3a** (the scorer returns identical
rankings for `"hi"` and a hard concurrency task) cannot be caught by a leaderboard-style
benchmark even if Vox had one — a contaminated benchmark would score the *model* well regardless
of whether the *harness*'s routing is broken. **This is why F3a was only found by running the
CLI directly and comparing outputs — the exact methodology this section validates.**

> **Time-scope caveat:** both papers concern Claude/o3-mini-era models circa 2025–2026 and may
> not generalize to future generations trained with contamination-aware curation.

---

## 2. τ-bench: deterministic outcome-checking + the pass^k reliability metric (confirmed, high)

τ-bench evaluates outcomes by **deterministic, programmatic comparison of end-state** (e.g.
database state) against an annotated goal — not LLM-as-judge scoring. Quoted: *"an efficient
and faithful evaluation process that compares the database state at the end of a conversation
with the annotated goal state."* **This is immune to the contamination mechanism in §1** — there
is no text for a model to have memorized an answer *to*; there is only a final state to check.

### 2.1 pass@k vs pass^k — the distinction that matters for regression gates

| Metric | Definition | Answers |
|---|---|---|
| **pass@k** | probability **at least one** of k independent trials succeeds | "can it work?" |
| **pass^k** | probability **all k** independent trials succeed | "does it work reliably?" |

$$\text{Pass@k} = 1 - \binom{n-c}{k} / \binom{n}{k}$$

**pass^k is the one that matters for regression-gating a harness**, because a harness that
"sometimes works" in a single demo run is not the thing being shipped to users repeatedly.

### 2.2 The empirical result should recalibrate expectations, not just methodology

Even **GPT-4o succeeds on <50% of τ-bench tasks at pass^1**, and reliability **collapses
further under repetition: pass^8 < 25%** in the retail domain.

**This is the single most important number for designing Vox's own eval gate.** It means a
*single* passing run — the thing most CI gates check — establishes almost nothing about
reliability. **A regression gate needs multi-sample statistical testing, not single pass/fail**,
and the τ-bench numbers show why: an agent can pass once and still fail most of the time.

---

## 3. LLM-as-judge is measurably fragile — even to ensembles (confirmed, high)

The foundational paper (Zheng et al., NeurIPS 2023, arXiv 2306.05685) — the source of the
widely-cited "GPT-4 matches human-human agreement" claim — identifies **position bias,
verbosity bias, and self-enhancement bias** as core limitations, alongside limited reasoning
ability.

> **⚠ Refuted 1-2 — do not restate:** "strong LLM judges such as GPT-4 achieve over 80%
> agreement with human preferences, matching human-human agreement levels." This is the
> paper's own headline number and it **did not survive verification** in this pass. Treat
> LLM-judge/human agreement as *not established at the >80% figure commonly quoted.*

**A more damaging and more recent finding (arXiv 2503.09347, ACL 2025):** purely **stylistic**
apologetic-language artifacts — unrelated to actual safety content — skew evaluator preferences
by up to **98%**, across **all 11 tested LLM judge models**. None were robust. **Jury-based
aggregation of multiple judges improves robustness and human alignment but does NOT eliminate
artifact sensitivity, even under the best jury configurations.**

**Consequence for Vox:** any skill-promotion gate or model-eval gate that uses an LLM judge
(induction doc §4, gate 4 "independent verify") must assume the judge can be swayed by
**writing style, not just correctness**. This does not invalidate LLM-judge gates — it means
they need artifact-hardening (adversarial rewrites that test whether the verdict changes with
style held constant and content varied) rather than blind trust, and an ensemble is a
mitigation, not a fix.

---

## 4. Observability: OpenTelemetry GenAI conventions as the trajectory substrate (confirmed, high)

OTel's GenAI semantic conventions standardize recording of model name, input/output token
counts, and — opt-in — the full content of prompts, completions, tool calls, and tool results.
Concrete span attributes: `gen_ai.request.model`, `gen_ai.usage.input_tokens`,
`gen_ai.usage.output_tokens`, `gen_ai.response.finish_reasons`.

**Agent tracing is modeled hierarchically**: a top-level `invoke_agent` span contains child
`chat` spans (one per LLM call) and `execute_tool` spans (one per tool invocation).

**This is the structural prerequisite for trajectory-level regression assertions** — the
distinction the research question specifically asked about (trajectory-level vs outcome-level).
An outcome assertion ("did the file get created") tells you *what* broke; a trajectory span tree
tells you *where in the loop* it broke — which tool call, which model response, which retry.

**Vox already has the raw material for this** — `vox-telemetry`, `vox-telemetry-otlp`, and the
`PromptDispatchTelemetryEvent` seen in the routing research (routing doc §1) already emit stage
labels and outcomes. The gap is adopting the OTel GenAI *span shape* specifically, so traces are
comparable across a regression rather than being ad hoc per-event records.

---

## 5. Skill-library governance: SkillOps (confidence: medium — single unreplicated preprint)

**SkillOps** (arXiv 2605.13716, May 2026) models each skill as a typed, executable contract:

$$s = (P, O, A, V, F)$$

— preconditions, operation, produced artifacts, validators, and known failure modes. This
enables systematic checks for **relevance/applicability**, **composability** (via type
matching), **validation gaps** (`V = ∅`), and **interface mismatches**.

It scores each skill along **five dimensions** — Utility, Redundancy, Compatibility,
Failure-Risk, Validation-Gap — combined into a weighted **library health score**. Empirically
validated on ALFWorld. This gives concrete, checkable gating criteria: skills below a utility
threshold, or in a duplicate cluster, are flagged for retirement.

> **This is a direct, formal answer to the induction research's open question #2** ("has anyone
> demonstrated actual skill retirement with a measured effect?") — but the confidence must stay
> at *medium*. This is a **single, non-peer-reviewed, unreplicated May-2026 preprint**, no
> independent critique was found, and a related claim — that maintenance is performed via typed
> discrete actions (merge/retire/repair/add_validator/add_adapter) as an auditable regression-gate
> analog — was **explicitly refuted (0-3)**. Do not cite the maintenance-action claim.

### 5.1 The `V = ∅` check is the most directly transplantable idea

A skill contract with **no validators at all** is flagged automatically. This maps precisely
onto the induction doc's gate 1 (execution gate) — a mined skill that was never given a way to
check its own success is, under SkillOps, definitionally ungovernable, and should be rejected at
mining time rather than after it starts producing untraceable failures in `skill_reliability`.

### 5.2 A competing philosophy exists, and it's worth naming

One fetched source (Skilldex) explicitly rejects gatekeeping: *"format conformance scoring
(0–100) with line-level diagnostics is advisory and never blocks installation or deployment of a
skill package"* — a *"warnings over blockers"* stance. This is the opposite of SkillOps and of
the induction doc's gated-promotion recommendation. **Named here as a real alternative, not
dismissed**: warnings-only trades library cleanliness for zero friction on skill authors. Given
the induction doc's finding that Voyager's self-verification gate alone accounts for 73% of
discovered skills' value, Vox should prefer SkillOps's stricter posture — but the tradeoff is
real and should be a conscious choice, not a default.

---

## 6. What this research could NOT answer — three real gaps

Unlike earlier batches, this run completed cleanly (0 session-limit failures), so these gaps are
**genuine absence of evidence**, not budget exhaustion:

1. **No defensible sample-size / statistical-power methodology for detecting a regression in a
   flaky harness.** A promising candidate paper (AgentAssay, arXiv 2603.02601) claimed 86%
   detection power via behavioral fingerprinting and a 78% sample-reduction via sequential
   probability ratio testing (SPRT) — **both refuted 0-3.** τ-bench's pass^k (§2) tells you
   *what* to measure; **nobody verified here tells you how many trials are enough.** This is
   the most operationally important open question for Vox's CI eval gate design.
2. **No verified primary source on how production teams canary a prompt or tool-definition
   change** — traffic-split methodology, rollback triggers, automatic gating thresholds. Absent
   entirely.
3. **No verified head-to-head comparison of Langfuse, Braintrust, and LangSmith's eval
   harnesses** on judge reliability, cost, or trajectory-assertion capability, despite being
   explicitly asked for.

**Recommendation:** treat these three as a dedicated follow-up research pass before finalizing
Vox's CI eval-gate design — do not proceed to implementation on an assumed sample-size number.

---

## 7. Concrete recommendations for Vox

Direct application to the audit's findings and the induction doc's promotion gate:

1. **Do not build or trust a SWE-Bench-style leaderboard as the harness regression gate.**
   Use Vox's own workspace as the eval corpus — real tasks, not published ones, closing the
   contamination vector by construction. `vox-eval` and `vox model eval` are the right shape;
   ensure their task corpus is Vox-specific and never sourced from a public benchmark that
   could leak into training data.
2. **Adopt deterministic outcome-checking wherever Vox can define a checkable end-state** —
   compiles, type-checks, tests pass, file exists with expected content — over LLM-judge
   scoring. This directly matches τ-bench's approach and sidesteps §3's bias problem entirely
   for the cases where it's possible.
3. **Where an LLM judge is unavoidable** (subjective quality, code-review-style judgments),
   require an **ensemble**, and periodically **rewrite the same submission in a different style**
   to test whether the verdict changes — the direct countermeasure to the 98%-swing finding.
4. **Report pass^k, not pass@1, for any harness-reliability claim.** A single successful demo
   run of the fixed chat→skill→model pipeline (once F1/F3/F6 from the audit are fixed) proves
   nothing about whether it works reliably. Multi-sample testing is required before calling
   any harness fix "done."
5. **Instrument the chat/dispatch/model-selection path with OTel GenAI-shaped spans** —
   `invoke_agent` → `chat` → `execute_tool` — so a regression is traceable to a specific stage,
   not just "the response was wrong." This is a natural fit for `vox-telemetry-otlp`, which
   already exists.
6. **Adopt SkillOps's `V = ∅` check as gate 1.5** in the induction doc's promotion pipeline —
   reject any mined skill candidate with no attached validator, before it ever reaches the
   execution gate. Cheap, mechanical, and catches the worst-case failure (an ungoverned skill)
   at zero cost.
7. **Do not implement a specific statistical sample-size threshold from this research** — none
   survived verification. Pick a provisional number (e.g., n=5 trials, gate on pass^5), instrument
   it, and revisit once the §6 follow-up research lands or once Vox has enough production
   trajectory data to calibrate empirically rather than by citation.

---

## 8. Refuted ledger — do not restate

| # | Claim | Vote |
|---|---|---|
| 1 | Strong LLM judges (GPT-4) achieve >80% agreement with human preferences, matching human-human agreement | **1-2** |
| 2 | AgentAssay's behavioral fingerprinting achieves 86% statistical power to detect regressions where binary pass/fail achieves 0% | **0-3** |
| 3 | SPRT reduces trial count needed to reach a regression verdict by 78% vs fixed-sample testing | **0-3** |
| 4 | The full AgentAssay pipeline eliminates live-inference costs entirely via trace-first offline analysis | **0-3** |
| 5 | SkillOps library maintenance via typed discrete actions (merge/retire/repair/add_validator/add_adapter) is an auditable regression-gate analog | **0-3** |

Claims 2–4 are all from the same paper (AgentAssay, arXiv 2603.02601) — every one of its
headline numbers failed verification. **Do not cite AgentAssay's specific figures**; the paper
may still be worth reading for its framing, but none of its quantitative claims survived.
