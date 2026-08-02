---
title: "Skill Discovery at Scale & Automatic Skill Induction — Verified Research 2026-07-30"
description: "Adversarially verified research into how agent skill/tool libraries stay discoverable past 30-50 entries (Tool Search Tool, MCP-Zero, ToolRet) and how they grow themselves (Voyager, CRAFT, AWM, SkillWeaver, Memp) — including every promotion gate, the measured gains, and the unsolved retirement problem."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Skill Discovery at Scale & Automatic Induction — Verified Research (2026-07-30)

> **Provenance.** `deep-research` run `wf_ac587250-a2d`: **102 agents, 6.8M subagent tokens,
> 663 tool uses, 0 errors.** 16 claims survived 3-vote adversarial verification; **9 were
> refuted**, several of them load-bearing headline numbers that circulate widely.
>
> **⚠ Known degradation, disclosed rather than hidden.** Multiple verifiers in this run
> reported the session's WebSearch budget exhausted (200/200) mid-pass. Verification therefore
> collapsed toward *primary-source confirmation that a paper says what was claimed*, and away
> from *adversarial sweeps for third-party critiques, failed replications, and contradicting
> results*. **Zero contradicting hits in this run means "unverified", not "refuted."** The
> budget has since reset and the gaps are being re-covered by run `wf_d875e11c-99d`
> (registries/trust) — see §7.

Companions: [`claude-code-harness-mechanics-2026-07-30.md`](claude-code-harness-mechanics-2026-07-30.md),
[`multi-provider-local-cloud-routing-research-2026-07-30.md`](multi-provider-local-cloud-routing-research-2026-07-30.md),
[`vox-harness-graph-audit-2026-07-30.md`](vox-harness-graph-audit-2026-07-30.md).
Prior Vox art: [`skill-code-marketplace-research-and-audit-2026-06-18.md`](skill-code-marketplace-research-and-audit-2026-06-18.md),
[`skill-ecosystem-interop-research-2026-06-12.md`](skill-ecosystem-interop-research-2026-06-12.md).

---

## 0. The two-sentence answer

**Discovery:** past roughly 30–50 tools, putting every definition in the system prompt stops
working, and on-demand retrieval measurably recovers the loss — but the benefit shrinks as
base models improve, so this is a scaling mechanism, not a permanent architectural truth.

**Growth:** mining reusable procedures from agent trajectories is a settled research
architecture with five independent implementations, consistent double-digit gains, and — in
every credible system — **an explicit gate before a candidate is promoted.** The unsolved part
is the other end: **nothing convincingly retires a skill once promoted.**

For Vox specifically, this reframes the audit's finding **F6a** (the hard 64-skill cap in
`render_skill_catalog`) from "an arbitrary limit to raise" into "**a limit that is roughly in
the right place, reached by the wrong mechanism.**" See §5.

---

## 1. Discovery: the 30–50 tool wall

### 1.1 The finding (confirmed 3-0)

Anthropic's own MCP evaluations, across 50+ tools spanning GitHub / Slack / Sentry / Grafana /
Splunk, with the **Tool Search Tool** enabled vs disabled:

| Model | All tools in prompt | With on-demand tool search | Δ |
|---|---|---|---|
| Opus 4 | 49% | **74%** | +25 pp |
| Opus 4.5 | 79.5% | **88.1%** | +8.6 pp |

Anthropic's platform docs state selection *"degrades once you exceed 30–50 available tools."*

Independent corroboration of the scale problem: **MCP-Zero**'s corpus of 308 servers /
**2,797 tools** totals **248.1k tokens of schema** — exceeding 128k/200k-class context windows
outright.

> **⚠ Four reasons to hold this loosely, all raised by the verifiers:**
>
> 1. The numbers are **vendor-internal**, published in a blog **announcing a paid API
>    feature**, with no methodology, N, or variance.
> 2. **The gain shrinks sharply as the base model improves** — 25 pp for Opus 4 vs 8.6 pp for
>    Opus 4.5. That pattern suggests the retrieval benefit is partly a proxy for base-model
>    weakness rather than a fixed architectural win.
> 3. These are Nov-2025 numbers on Opus 4/4.5; the lineup has since advanced through Opus 5.
>    The compression may have continued.
> 4. **Nobody has published an accuracy-vs-tool-count curve** — only two-condition A/Bs. The
>    degradation slope and the crossover point where retrieval stops paying are unknown.
>
> **⚠ Refuted 0-3 — do not restate:** the companion framing that a 58-tool multi-server setup
> costs "roughly 55K tokens (about 77K including …)". The arithmetic is internally
> inconsistent (8.7/77 is an 88.7% cut of total up-front context; the "~85%" figure tracks
> only the 55k→8.5k tool-definition subtotal), and it describes a *constructed* configuration.

### 1.2 Architecture matters, not just retrieval (confidence: medium, 2-1)

**MCP-Zero** combines three mechanisms:

1. **Active Tool Request** — the agent *declares a capability gap* rather than being handed a
   menu.
2. **Hierarchical Semantic Routing** — request → server → tool, two-stage rather than flat.
3. **Iterative Capability Extension.**

APIBank results:

| Condition | Schema tokens | Claude-3.5-Sonnet | GPT-4.1 | Gemini-2.5-Flash |
|---|---|---|---|---|
| Standard (single-turn, full tools) | 6,308.2 | 69.23% | 94.71% | 94.23% |
| MCP-Zero (single-turn) | **111** | **95.19%** | 95.19% | 96.63% |
| Standard (multi-turn) | 6,402.2 | — | — | — |
| MCP-Zero (multi-turn) | **159** | 5 of 6 cells equal-or-better | −0.54 pt regression | — |

A needle-in-haystack sweep from 1 → 2,797 tools shows **flat** token cost for MCP-Zero vs
exponential growth for standard schemas.

> **Caveats:** 98.24% is the **top** of the paper's own 60–98% range and applies to the
> full-pool condition. It counts **schema context, not the round-trips** the iterative protocol
> adds. Hierarchical routing is one of three mechanisms and **was not isolated by a verifiable
> ablation**. Unreplicated preprint.

**The transplantable idea for Vox is #1, not #2.** "Active Tool Request" — the agent saying
*"I need something that can do X"* — costs nothing to implement, needs no embedding index, and
composes with the existing Tier-1 catalog. It is strictly cheaper than building semantic
retrieval, and Vox's `vox_skill_use` tool is already the second half of it.

### 1.3 Discovery is a reliability component (confirmed 3-0)

**ToolRet** — 7.6k retrieval tasks over a **43k-tool** corpus, 200k+ training instances —
states plainly: *"This low retrieval quality degrades the task pass rate of tool-use LLMs."*
Backed by a ToolBench pilot where swapping the officially annotated toolset for *retrieved*
tools substantially drops agent performance, and where even strong retrievers like `colbertv2`
struggle.

> Exact deltas live in a figure rather than quotable prose, so magnitude is indicative. The
> end-to-end evidence is a **single pilot** on ToolBench, whose live-API instability is a known
> noise source.
>
> **⚠ Refuted 0-3 — two sibling claims from the same paper that circulate widely:**
> the `nDCG@10 = 33.83` figure for NV-Embed-v1, and the claim that "toolsets cannot fit in
> context." Do not cite either.

**The design consequence for Vox:** skill discovery cannot be treated as a UI nicety or a
search feature. It sits on the reliability path, and it deserves the same eval treatment as
model selection.

---

## 2. Growth: automatic skill induction

### 2.1 The settled architecture (confirmed 3-0)

Five independent systems mine reusable procedures from trajectories rather than hand-authoring
them:

| System | Domain | What it mines | Indexing |
|---|---|---|---|
| **Voyager** (2305.16291) | Minecraft | *"an ever-growing skill library of executable code for storing and retrieving complex behaviors"* | embedding of the skill **description**, top-5 retrieved on demand |
| **CRAFT** (2309.17428) | tool creation | GPT-4 solves training examples; solutions become abstracted, deduplicated code snippets | signature-grouped |
| **AWM** (2409.07429) | web agents | *"commonly reused routines, i.e., workflows"*, injected selectively | offline from training examples **or online from test queries alone** |
| **SkillWeaver** (2504.07079) | web agents | *"the agent autonomously discovers skills, executes them for practice, and distills practice experiences into robust APIs"* | synthesized APIs |
| **Memp** (2508.06433) | ALFWorld / TravelPlanner | trajectories → step-by-step instructions **and** script-like abstractions | trajectory store |

Note **AWM's online mode**: it induces workflows **from test queries alone, with no labeled
training data**, which is precisely why it was applied to WebArena. That is the mode Vox needs
— nobody is going to hand-label a training set of Vox tasks.

> **Scope honesty:** all five are **research systems** (Minecraft, web agents, ALFWorld,
> TravelPlanner), not production deployments. The production evidence in this corpus covers
> *retrieval*, not *induction*. Vox would be an early production adopter, not a follower.

### 2.2 Every credible system gates before promotion (confirmed 3-0)

**This is the single most important section for Vox**, because it is exactly the
"adequate testing for what gets pushed into the system" requirement.

| System | Promotion gate |
|---|---|
| **Voyager** | Iterative loop over environment feedback, execution errors, and **GPT-4 self-verification** — *"repeats until self-verification validates the task's completion, at which point we add this new skill to the skill library."* **Ablating self-verification costs −73% of discovered items** — the most important feedback type of all. |
| **CRAFT** | Three stages: **execution-based validation** (*"tools that fail to derive the correct answers given the original problems are discarded"*), **abstraction for reusability** (domain variables generalized, general names/docstrings assigned), and **signature-grouped deduplication** via GPT-4. |
| **AWM** | Only trajectories an **LM evaluator labels success** (`L_eval ∈ {0,1}`) become workflows. |
| **SkillWeaver** | **propose → synthesize → hone**, distilling only successful trajectories; the repo documents testing iterations and an `--allow-recovery` flag for patching APIs that throw during testing. |

> **Two limits the verifiers insisted on, and both matter for Vox's design:**
>
> 1. **The verifier is usually the same model family as the generator** — this is
>    self-consistency, not independent validation.
> 2. **Gates check task completion, not generality, reusability, or continued reliability.** A
>    skill can pass its gate and still be a one-off that never applies again.
>
> **Do not assert** an "LLM-judge" mechanism or a numeric threshold for SkillWeaver — its
> precise gating criterion could not be verified from primary text.

The −73% Voyager ablation is the number to remember: **the gate is not overhead on the
induction pipeline, it is most of the value.** An ungated miner is not a cheaper version of a
gated one; it is a different and much worse system.

### 2.3 The gains are real, and smaller than the headlines (confirmed 3-0 / 2-1)

| System | Reported gain |
|---|---|
| **AWM** | **+24.6%** relative success on Mind2Web; **+51.1%** relative on WebArena, over the same agent without AWM; plus fewer steps on WebArena |
| **SkillWeaver** | **+31.8%** relative on WebArena; **+39.8%** on real-world websites |
| **SkillWeaver (transfer)** | *"APIs synthesized by strong agents substantially enhance weaker agents through transferable skills, yielding improvements of up to **54.3%** on WebArena"* — GPT-4o-mini consuming a stronger agent's APIs |

> **⚠ Framing caveats that materially change the picture:**
>
> - **All figures are RELATIVE gains on low-baseline benchmarks** (~30% absolute on WebArena).
>   AWM's own cross-domain figure is a far more conservative **8.9–14.0 absolute points**.
> - The AWM comparison is an **ablation against the same agent**, not a win over a separately
>   tuned SOTA.
> - **54.3% is a best-case "up to"**, strong→weak direction only.
> - **No independent replications located.**
>
> **⚠ Refuted 0-3 — do not cite:** Voyager's widely-quoted headline (3.3× more unique items,
> 2.3× longer distance, 15.3× faster tech-tree). It did not survive.

**The transfer result is the one with the most direct Vox relevance.** Strong→weak skill
transfer is the mechanism by which a frontier model's induced skills make a **local 8B model**
useful — which is exactly the local-first story Vox wants, and it connects this research
directly to the routing work.

### 2.4 Retirement is the unsolved layer (confidence: medium)

**Memp** is the only system in the corpus that treats the full lifecycle seriously. It
decomposes procedural memory into three separately ablatable axes:

- **Build** — {No Memory, Trajectory, Script, Proceduralization}
- **Retrieval** — {Random, Query, AveFact}
- **Update** — {Vanilla, Validation, Adjustment}

…and formalizes the update operator as **`U = Add(M_new) − Del(M_obs) + Update(M_est)`**,
making induction, selection, and retirement distinct engineering choices. The abstract commits
to a regimen that *"continuously updates, corrects, and deprecates its contents."*

> **But the empirical support is asymmetric, and the verifiers caught it:**
>
> - The **correction** half **is** ablated — reflexion-style in-place revision of memories that
>   caused failures is the strongest variant on TravelPlanner and ALFWorld.
> - **Deprecation is defined in the formalism and asserted, but never isolated by ablation.**
> - The `Validation` strategy is a **build-time filter on candidates**, not demonstrated
>   retirement of already-promoted entries.
>
> By contrast, **Voyager's library is literally "ever-growing" with no retirement mechanism at
> all.**
>
> Note also: Memp's `Retrieval` axis is **memory-record retrieval over a trajectory store**. It
> maps onto skill selection only by analogy and does **not** bear on tool-selection-at-scale
> routing.

**This is where Vox can lead rather than follow.** Vox already has the `skill_reliability`
table — a populated observed-success signal that nothing reads (audit finding **F14**).
Retirement driven by measured reliability is the least-developed layer in the entire
literature, and Vox is one join away from having the substrate for it.

---

## 3. What this means for Vox's 64-skill cap

Audit finding **F6a** recorded that `render_skill_catalog(&entries, 64)` silently truncates
alphabetically past 64 skills. This research reframes it:

- **64 is roughly the right order of magnitude.** Anthropic's own guidance puts degradation at
  30–50 tools. A ~64-entry Tier-1 catalog is *not* an arbitrary under-provisioning; it sits
  just above the documented wall.
- **Alphabetical truncation is the wrong mechanism.** It is arbitrary with respect to
  relevance, silent, and adversarially exploitable (name a skill `aaa-…` to guarantee
  inclusion).
- **Raising the cap is the wrong fix.** Past the wall, more entries make selection *worse*, not
  better. The correct fix is the one both Anthropic and MCP-Zero converged on: **keep a small
  always-present catalog and add on-demand retrieval behind it.**

Concretely, in ascending order of cost:

1. **Make truncation loud** — log/surface when skills are dropped. Trivial, immediate.
2. **Rank by observed reliability + recency instead of alphabet** — uses `skill_reliability`,
   which already exists and is already populated.
3. **Add "Active Tool Request"** — let the model say *"I need a skill that does X"* and return
   matching descriptions. `vox_skill_use` is already the second half; this adds a
   `vox_skill_search`. `vox-similarity` (currently 65% isolated) is the natural backend.
4. **Two-stage routing** (category → skill) only if the library genuinely passes a few hundred.

---

## 4. Proposed promotion gate for Vox

Synthesizing §2.2 into something implementable, and answering the "adequate testing for what
gets pushed in" requirement directly:

```
CANDIDATE  (mined by code_miner / op_miner from repeated user trajectories)
    │
    ├─ 1. EXECUTION GATE      (CRAFT)
    │     The candidate must reproduce the outcome of the trajectories it was
    │     mined from. Fails to reproduce → discard. Not negotiable.
    │
    ├─ 2. ABSTRACTION         (CRAFT)
    │     Generalize domain constants to parameters; assign a general name and
    │     a description written to the 1024-char / third-person / "use when"
    │     standard (mechanics doc §4.4). A candidate whose description does not
    │     meet the authoring standard is undiscoverable and must not be promoted.
    │
    ├─ 3. DEDUPE              (CRAFT signature-grouping + vox-similarity)
    │     Reject near-duplicates of installed skills. This is the guard against
    │     the library growing without becoming more capable.
    │
    ├─ 4. INDEPENDENT VERIFY  (fixes the §2.2 self-consistency limit)
    │     Verify with a DIFFERENT model than the one that mined it. Vox's
    │     multi-provider catalog makes this nearly free and is a genuine
    │     advantage over every system reviewed.
    │
    ├─ 5. GENERALITY GATE     (fixes the §2.2 "completion, not generality" limit)
    │     Require ≥N distinct source trajectories before promotion. A skill
    │     mined from one trajectory is a macro, not a skill.
    │
    └─ 6. SHADOW PERIOD       (feeds retirement)
          Promote as `provisional`. Record skill_reliability. Promote to
          `confirmed` on sustained success; RETIRE on sustained failure.
```

Gates 4, 5, and 6 are each responses to a **specific documented weakness** in the literature
rather than invented process. Gate 6 is the retirement mechanism §2.4 says nobody has
demonstrated.

**This also supplies the "what gets pushed out" half:** retirement is gate 6 running in
reverse, driven by the same `skill_reliability` signal, with `provisional` → `confirmed` →
`deprecated` as the state machine. Vox already has an exactly analogous machine for models
(`ModelConfidence` with `is_routing_eligible` gating and a controlled exploration budget,
`select.rs:118-161`). **Reusing that pattern for skills is a small amount of work and is
already proven in-tree.**

---

## 5. Refuted ledger — do not restate these

Nine claims failed verification in this run. Several are widely circulated.

| # | Claim | Vote |
|---|---|---|
| 1 | Anthropic's 55K/77K-token cost framing for a 58-tool multi-server setup | **0-3** |
| 2 | ToolRet: NV-Embed-v1 reaches only nDCG@10 = 33.83 | **0-3** |
| 3 | Tool retrieval is necessary because large toolsets "cannot fit in context" | **0-3** |
| 4 | MetaTool/ToolE: 21,127 queries over ~195 tools from ~390 OpenAI plugins | **1-2** |
| 5 | MetaTool: best model reaches low CSR on the similar-choices subtask | **0-3** |
| 6 | MetaTool: most models below 20% CSR on the reliability subtask | **0-3** |
| 7 | Voyager: 3.3× unique items, 2.3× distance, 15.3× faster tech tree | **0-3** |
| 8 | Memp stores at two granularities simultaneously | **1-2** |
| 9 | CRAFT does tool selection by retrieval at inference from a curated toolset | **0-3** |

All three MetaTool/ToolE claims failed. **Do not resurrect any of these from memory of the
source papers** — they are exactly the kind of plausible, frequently-quoted figure this
process exists to catch.

---

## 6. Open questions

1. **Does retrieval-based tool selection still pay off on current frontier models?** The gain
   fell from 25 pp (Opus 4) to 8.6 pp (Opus 4.5). Nobody has published an
   accuracy-vs-tool-count curve — only two-condition A/Bs — so both the degradation slope and
   the crossover point are unknown. *Directly determines whether Vox should build retrieval at
   all, or just fix the truncation.*
2. **Has anyone demonstrated actual skill RETIREMENT with a measured effect?** Memp defines
   `Del(M_obs)` but ablates only correction; Voyager never deletes. What happens to selection
   precision, retrieval latency, and success rate as an induced library grows to thousands of
   entries with no eviction — and is there an empirical stale-skill problem?
3. **Do trajectory-mined skills survive outside densely-instrumented simulators?** Every gate
   verified here relies on **cheap ground truth** — Minecraft inventory state, benchmark answer
   keys, LM success judges on web tasks. What gates work when success is **ambiguous, delayed,
   or side-effectful**? *This is the hardest open problem for Vox specifically*, because
   software-engineering outcomes are exactly that. Vox's partial answer already exists: tests,
   type-checking, and CI gates are cheap ground truth for a coding agent — arguably cheaper and
   more reliable than anything in the literature.
4. **How does induction compose with retrieval routing** once the induced library itself crosses
   the 30–50-tool degradation threshold? Induction and discovery are studied separately; a
   self-growing library necessarily collides with the discovery wall.

---

## 7. Coverage gap being re-covered

Sub-question (d) — **marketplaces, registries, curation, and trust models** — produced **zero
surviving claims**. Of 16 survivors, none addressed it; and none of the 9 refutations were
registry claims either, so the topic is **under-researched, not researched and disproven.**

Per the standing heuristic: **absence of surviving claims is "unverified", not "refuted."** It
is *not* evidence that registries lack curation or trust models.

The only production evidence gathered here (Anthropic's Tool Search Tool) concerns **runtime
retrieval within an already-connected tool set** — not registry-level curation, provenance, or
trust.

A dedicated pass (`wf_d875e11c-99d`) is running over the MCP Registry, Claude Code plugin
marketplaces, the GPT Store, MCP security research (tool poisoning, rug-pulls, name-squatting),
and package-registry prior art (npm provenance, sigstore, crates.io). Its findings will land in
a companion document.
