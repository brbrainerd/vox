---
title: "Multi-Provider & Local-vs-Cloud Model Routing — Verified Research 2026-07-30"
description: "Adversarially verified research into production LLM routing: LiteLLM's deployment/strategy model, OpenRouter's inverse-square price weighting and provider policy object, Portkey's recursive conditional routing DSL, and RouteLLM's learned binary router — distilled into a declarative policy design Vox can adopt, with an explicit ledger of what does not transplant."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Multi-Provider & Local-vs-Cloud Routing — Verified Research (2026-07-30)

> **Provenance.** `deep-research` run `wf_32c7ec86-3e7`: 5 angles → sources fetched and
> claim-extracted → 3-vote adversarial verification → 18 surviving findings. **109 agents,
> 7.1M subagent tokens, 0 errors.** Verifiers read vendor *source code* (LiteLLM
> `router_strategy/*.py`, `types/router.py`) alongside docs, and caught four places where
> the published docs are **stale relative to the code**. Those corrections are recorded
> inline — they are the most valuable output of the run.
>
> **Coverage gap, stated up front:** the question also asked about hardware detection and
> how Claude Code / Cursor / Continue.dev / Aider / Zed expose local-model choice to users.
> **No surviving evidence covers either.** See §8; this is carried into research batch 4.

Companions: [`claude-code-harness-mechanics-2026-07-30.md`](claude-code-harness-mechanics-2026-07-30.md),
[`vox-harness-graph-audit-2026-07-30.md`](vox-harness-graph-audit-2026-07-30.md),
[`vox-harness-parity-plan-2026-07-30.md`](vox-harness-parity-plan-2026-07-30.md).
Prior Vox art: [`model-selection-2026-q2.md`](model-selection-2026-q2.md),
[`model-catalog-ssot-2026.md`](model-catalog-ssot-2026.md),
[`model-autonomic-system-2026.md`](model-autonomic-system-2026.md).

---

## 0. The convergent architecture

Every production system examined — LiteLLM, OpenRouter, Portkey — independently arrived at
the same four-stage shape:

```
  logical model name
        │
        ├── N heterogeneous deployments   (provider-prefixed id + api_base)
        │       cloud and LOCAL are the same kind of thing here
        │
        ├── candidate FILTER              (capability, privacy, context window,
        │                                  key presence, capacity/cooldown)
        │
        ├── candidate RANK                (cost | measured latency | usage | custom)
        │
        └── FALLBACK CASCADE              (on failure, on context overflow,
                                           on content policy)
```

…with the policy expressed as **data** (YAML/JSON), not code.

**Vox already has stages 1, 2, and 3** in `models::decide` — `CandidateScope`,
capability filters, key-presence gating, confidence gating, `SelectionAxes` scoring. What
Vox lacks is (a) a **declarative policy surface** the user can author, (b) **measured
telemetry** feeding the rank, and (c) a **local→cloud escalation cascade**. That is the shape
of the work, and it is smaller than it looks.

---

## 1. LiteLLM — the deployment/strategy model

### 1.1 Local endpoints are not a special case (confirmed 3-0)

The core primitive is **a logical model name backed by N heterogeneous deployments**, each
addressed by a provider-prefixed identifier plus `api_base`. Sibling deployment forms from
the docs:

```yaml
- azure/gpt-4o-eu                    + api_base
- bedrock/anthropic.claude-instant-v1
- ollama/mistral
- openai/facebook/opt-125m           # vLLM, OpenAI-compatible
```

Grouping rule, verbatim: *"If multiple with `model_name=gpt-4o` does Load Balancing."* The
Router quickstart maps one `model_name` onto `azure/chatgpt-v-2`,
`azure/chatgpt-functioncalling` **and** OpenAI `gpt-3.5-turbo` — heterogeneous providers
behind one alias, **no same-provider constraint**.

> **Verifier caveat.** The specific config snippet quoted puts those entries under
> *different* aliases; the "one alias spanning providers" half rests on the load-balancing
> sentence and the Router doc rather than that snippet.

**The design lesson for Vox is the one Vox has already half-learned and half-forgotten:**
`ChatProviderRouteKind` treats `PopuliLocal`/`PopuliMesh` as peers of `OpenRouter` and
`HuggingFaceRouter` — correct. But `CandidateScope::LocalOnly` is a *separate axis* from the
scoring, whereas LiteLLM makes locality just another deployment attribute that the filter and
rank stages see uniformly. Vox's split is defensible; it just needs the policy layer above it.

### 1.2 Strategy is a named, swappable knob (confirmed 3-0)

Selected by a single `routing_strategy` field:

| Strategy | Behaviour |
|---|---|
| `simple-shuffle` | **default** — weighted by `rpm`/`tpm`/`weight` (static, config-driven) |
| `least-busy` | fewest in-flight requests |
| `latency-based-routing` | lowest measured average response time |
| `usage-based-routing` (+ `v2`) | TPM/RPM headroom |
| `cost-based-routing` | cheapest passing capacity filters |
| `provider-budget-routing` | per-provider spend budgets |

Verified against source: `litellm/types/router.py` defines
`class RoutingStrategy(enum.Enum)` with `LEAST_BUSY`, `LATENCY_BASED`, `COST_BASED`,
`USAGE_BASED_ROUTING_V2`, `USAGE_BASED_ROUTING`, `PROVIDER_BUDGET_LIMITING`.

> **⚠ Two doc-vs-source corrections the verifiers caught:**
> 1. The `proxy/configs` page's enum
>    `["simple-shuffle","least-busy","usage-based-routing","latency-based-routing"]` is a
>    **stale undercount** — `cost-based-routing` is missing from it but exists in the code.
> 2. The custom hook is **not** selected via `routing_strategy`. It is registered separately
>    with `router.set_custom_routing_strategy(...)` on a `CustomRoutingStrategyBase`
>    subclass.

### 1.3 Latency routing runs on measured telemetry (confirmed 3-0)

Not static config. From source (`router_strategy/lowest_latency.py`):

- `log_success_event` appends observed durations:
  `request_count_dict[id].setdefault("latency", []).append(final_value)` under cache key
  `"{model_group}_map"`, a **~10-sample rolling window**.
- Selection reads that cache, averages per deployment, and
  `sorted(potential_deployments, key=lambda x: x[1])`.
- Tunable `routing_strategy_args={"ttl": 10}`.
- `lowest_latency_buffer` exists specifically so traffic isn't pinned to one fastest box.
- **For streaming, time-to-first-token replaces total latency.** ← the correct metric for an
  interactive chat harness, and the one Vox should record.

> **Two qualifications:** the strategy is **opt-in** (default `simple-shuffle` is static), and
> **static config still gates the candidate set** — TPM/RPM limits and cooldowns filter
> `potential_deployments` *before* latency ranking applies. Filter first, rank second.

### 1.4 Cost routing — and the local-endpoint trap (confirmed 3-0)

Precedence chain, verified in `router_strategy/lowest_cost.py`:

1. `_deployment["litellm_params"]["input_cost_per_token"]` / `output_cost_per_token`
2. fall back to `litellm.model_cost[model]` (from `model_prices_and_context_window.json`)
3. `item_cost = item_input_cost + item_output_cost`, select minimum among deployments that
   pass RPM/TPM capacity filters

> **⚠ Doc-vs-source correction:** the docs' stated unknown-model fallback of `$1` is **stale**;
> source uses `5.0` for each of input and output.

**The trap, and it is the single most important sentence in this document for Vox:**

> Registering a local endpoint at zero cost makes it **win unconditionally**. LiteLLM's
> cost strategy is **comparative** (cheapest available), not **threshold-based** (escalate
> above a complexity bound). *The threshold gate is the harness's job.*

A naive "prefer local because it's free" policy routes *everything* — including the hardest
reasoning task in the session — to a 7B model. This is exactly the failure mode Vox would hit
if `prefer_local` were exposed as a simple boolean without a capability floor. **Any Vox
local-preference control must pair the preference with a capability/complexity gate.**

### 1.5 Escalation is declarative (confirmed 3-0)

```yaml
num_retries: 3
fallbacks:               [{"zephyr-beta": ["gpt-4o"]}]
context_window_fallbacks: [{"zephyr-beta": ["gpt-3.5-turbo-16k"]}]
```

Three distinct fallback classes: general `fallbacks`, `content_policy_fallbacks`, and
`context_window_fallbacks`. The last fires on `litellm.ContextWindowExceededError`, with
LiteLLM **normalizing provider-specific error strings** ("exceed context limit", "maximum
context length") into one condition — a small, high-value piece of engineering Vox should
copy directly, since its five provider lanes each report overflow differently.

> **Two caveats:** `enable_pre_call_checks: true` is **required** for context-window
> enforcement (not zero-config). And **every documented example is cloud-to-cloud** — no
> local/Ollama fallback example appears anywhere, so the local→cloud cascade is a *sound but
> undemonstrated extrapolation*. Vox would be building it, not copying it.

### 1.6 Policy is portable data (confirmed 3-0)

`LiteLLMParamsTypedDict` (`router.py:371`) declares `model`, `tpm`, `rpm`, `itpm`, `otpm`,
`order` (lower = higher priority), `weight`, `max_parallel_requests`.
`DeploymentTypedDict` (`router.py:432`) declares `model_name` (required), `litellm_params`
(required), `model_info`. **One schema shared between the proxy's YAML and the SDK's
Router.**

> **Caveat:** portability is at the *schema* level — the SDK takes a list of dicts; it does
> not load the proxy's YAML file.
>
> **⚠ Refuted 0-3:** the claim that `config.yaml` has exactly four top-level sections
> (`model_list`, `router_settings`, `litellm_settings`, `general_settings`) did **not**
> survive. Do not treat that as a complete section list.

---

## 2. OpenRouter — provider selection as a policy object

### 2.1 Default selection is price-weighted stochastic load balancing (confirmed 2-1 / 3-0)

Verbatim: *"Prioritize providers that have not seen significant outages in the last 30
seconds… look at the lowest-cost candidates and select one weighted by inverse square of the
price… Use the remaining providers as fallbacks."*

Worked example: *"Provider A is 9x more likely to be first routed to… than Provider C
because 1 / 3² = 1/9."*

Three design notes worth stealing:

- **Outage-aware providers are deprioritized, not removed.** A degraded provider stays in the
  fallback chain. Vox's `ModelConfidence` gating currently *excludes*; deprioritizing is
  gentler and more available.
- **Inverse-square, not inverse.** Superlinear price sensitivity, while still sampling
  stochastically so no single provider is pinned.
- **The weighting operates over a truncated "lowest-cost candidates" pool**, not all
  providers — filter, then weight.

> **⚠ Citation fix from verification:** the URL used during research
> (`openrouter.ai/docs/docs/routing/provider-selection`, note the duplicated `/docs/`) **404s**.
> Canonical: `openrouter.ai/docs/guides/routing/provider-selection`.
>
> **Scope caveat:** this default applies only when neither `sort` nor `order` is set;
> `:nitro` / `:floor` disable it.

### 2.2 The provider policy object (confirmed 3-0)

Per-request JSON where cost, latency, throughput, privacy, and capability are **all
first-class sibling fields**:

| Field | Default | Kind |
|---|---|---|
| `order` | — | explicit preference list |
| `allow_fallbacks` | `true` | |
| `require_parameters` | `false` | capability filter |
| `data_collection` | `"allow"` | **privacy filter** |
| `zdr` | — | **privacy filter** |
| `only` / `ignore` | — | allow/deny lists |
| `quantizations` | — | capability filter |
| `sort` | — | `price` \| `throughput` \| `latency` |
| `preferred_min_throughput` | — | **soft** ranking hint |
| `preferred_max_latency` | — | **soft** ranking hint (percentile form supported) |
| `max_price` | — | **hard** constraint |

Percentile form is supported: `{"preferred_max_latency": {"p50":1,"p90":3,"p99":5}}`.

> **Critical enforcement nuance for harness design — this is the finding to internalize:**
> `preferred_min_throughput` and `preferred_max_latency` are **soft**. Non-conforming
> endpoints are *"deprioritized (moved to fallback positions) rather than excluded entirely"*
> and the docs explicitly say they *"do not guarantee you will get a provider with this
> performance level."* This is contrasted directly with `max_price`, which is **hard** and can
> block the request.
>
> **Latency and throughput are ranking hints. Price and privacy are filters.** Vox should
> adopt exactly this split: never let a latency preference make a request fail, and never let
> a privacy preference silently degrade to a hint.

### 2.3 Privacy is a routing filter, not a separate product (confirmed 3-0)

- `data_collection: "allow" | "deny"` — deny means *"use only providers which do not collect
  user data."*
- `zdr: true` — *"Restrict routing to only ZDR (Zero Data Retention) endpoints."*
- The per-request `zdr` *"operates as an OR with your account-wide and guardrail ZDR
  settings"* and *"can only ensure ZDR is enabled, not override account-wide or guardrail
  enforcement."* **It can only tighten, never loosen.** That one-way ratchet is the correct
  semantics for any privacy control and Vox should copy it verbatim.
- EU enterprise: requests *"are decrypted and processed entirely within the European Union
  through eu.openrouter.ai, and only EU-eligible providers serve them."*

**This is the closest published analogue to a privacy-driven local-vs-cloud gate**: model the
local endpoint as the maximally-private target and let a privacy predicate filter the
candidate set. It is also the direct fix for audit finding **F7** — the `local_only` mesh
setting that reads as a privacy control but doesn't gate inference. The correct Vox design is
a privacy axis that filters model candidates, ratchets one-way, and cannot be downgraded by a
per-request hint.

### 2.4 Auto Beta — the two-stage automatic router (confirmed 2-1 / 3-0)

1. A **fast lightweight classifier** assigns each prompt one of **~30 fine-grained task
   types** — `code:debugging`, `agent:multi_step_planning`, `qa_knowledge`, `math`,
   `customer_support`, `research_report`, …
2. Candidates for that task type are ranked by **trailing-7-day community "Share of Spend"** —
   a live crowd-sourced usage signal, explicitly *"with no retraining or manual curation"* —
   filtered by `cost_quality_tradeoff`, `allowed_models`, and output modality.

> **Three disambiguations for anyone implementing from this:**
> 1. The older `openrouter/auto` slug is **NotDiamond-powered and deprecated**. A blog page
>    describing NotDiamond routing is about *that*, not Auto Beta.
> 2. OpenRouter's separate **"Classifiers"** feature (blog 2026-07-24) is a user-defined
>    *spend-attribution taxonomy*, **not routing**. Do not conflate it with the ~30 task types.
> 3. **Only the classifier half transplants.** The ranking signal is proprietary community
>    telemetry a local-first harness cannot replicate. Vox must substitute its own quality
>    signal — offline benchmarks or local eval history. **Vox already has both**: `vox-eval`
>    and the `skill_reliability` / model-telemetry tables. This is the substitution, and it is
>    a better signal than crowd spend because it is measured on *your* workload.

### 2.5 The whole cost/quality axis collapses to one integer (confirmed 2-1)

`cost_quality_tradeoff`, integer **0–10**. Default 9 on Auto Beta, 7 on the deprecated auto.
0 = *"pure quality — always picks the most capable model regardless of cost."*
10 = *"maximize for cost — cheapest model wins."*

> **Two framing corrections from verification:** it is a **per-request API body parameter**,
> not a config-file/policy-DSL surface — so it is a poor citation for "declarative policy
> config" specifically. And it governs **only** the cost/quality axis; provider preferences,
> `models` fallback arrays, and `:floor`/`:nitro` shortcuts remain separate knobs.

It remains an excellent data point that **the entire cost/quality axis can be one scalar
dial.** Vox's `SelectionAxes` uses *three* 0–100 knobs (cost, responsiveness, intelligence).
That is more expressive and almost certainly harder to use. The recommendation in the parity
plan is to keep the three axes as the *engine* and expose a **single preset dial** as the
default UI, with the three axes behind "advanced."

### 2.6 The routing-quality numbers, and why to discount them (confidence: medium)

OpenRouter's Auto Beta vs the deprecated NotDiamond-powered Auto:

| Benchmark | n | cqt=0: Beta vs Auto | cqt=7: Beta vs Auto |
|---|---|---|---|
| GPQA Diamond | 198 | **83.8%** vs 50.0% | 74.2% vs 61.6% |
| τ-bench Verified Airline | 50 | **74.0%** vs 34.0% | 66.0% vs 30.0% |
| DRACO deep research (LLM-judged) | 20 | **60.0** vs 19.6 | 63.2 vs 25.6 |

**Four reasons the verification pass downgraded confidence:**

1. Vendor benchmarking its own new router against its own deprecated predecessor.
2. Tiny samples (n=50, n=20; LLM judge on DRACO).
3. **Internally odd** — Auto's GPQA is *non-monotonic* in `cqt` (50.0 at cqt=0 vs 61.6 at
   cqt=7, i.e. the "quality" setting scores **worse**), which undercuts the "gap widens as
   tasks get harder" narrative on that benchmark.
4. **Superseded** — Auto Exacto (~2026-03-10) is now the on-by-default adaptive-quality router
   and does *not* restate this table ("GPQA Diamond results are still running"). This is now a
   comparison of two superseded configurations.

Verifiers also flagged that the WebSearch budget was exhausted before an adversarial sweep for
independent critiques could run. **Treat these numbers as directional at best.**

---

## 3. Portkey — the most expressive declarative policy surface found

### 3.1 Routing is a recursive tree (confirmed 3-0)

`strategy.mode` may be `conditional`, `loadbalance`, or `fallback` — and **every strategy can
be nested inside any other**, yielding a routing *tree* rather than a flat strategy selection.

Verbatim: *"Every Portkey routing strategy — conditional, load balancing, fallback — can be
nested inside any other,"* backed by a 6-row all-pairs matrix and a working config where a
conditional root's target is itself `{"strategy":{"mode":"loadbalance"}, "targets":[...]}`.
The conditional reference independently states targets *"are fully composable — each target
can itself be a load balancer, a fallback chain, or another conditional router."*

> **Caveat:** this is documented capability; runtime nesting-depth limits were **not**
> empirically verified (budget exhausted before a third-party contradiction sweep).

### 3.2 The conditional strategy — the template for local-vs-cloud (confirmed 3-0)

```json
{
  "strategy": {
    "mode": "conditional",
    "conditions": [
      { "query": { "params.model": { "$eq": "claude-sonnet" } }, "then": "claude-sonnet-lb" }
    ],
    "default": "gpt-4o-direct"
  },
  "targets": [ { "name": "claude-sonnet-lb", ... }, { "name": "gpt-4o-direct", ... } ]
}
```

Semantics, all quoted from the docs:

- *"`conditions` and `default` are required params for the conditional strategy."*
- *"Since Portkey iterates through the queries sequentially, the order of your conditions is
  important."* — **first-match-wins in array order.**
- *"When a condition evaluates to false or is malformed, Portkey moves on to the next
  condition until it finds a successful one."*
- *"If a referenced key is missing (in metadata, params, or url), the condition evaluates to
  false; it does not throw an error."* — **fail-soft.**
- *"If no conditions pass, then the `default` target name is called."*

Note `default` holds the **name of a target** in the `targets` array, not a target literally
named "default". Undocumented gap: behaviour when `default` names a nonexistent target.

**Fail-soft + mandatory default is the right ergonomics for a user-authored policy file.** A
typo degrades to the default rather than breaking the session. Vox's config system currently
hard-errors on unknown keys in several places; for a routing policy that is the wrong trade.

### 3.3 The rule DSL and its two limits (confirmed 3-0)

MongoDB-style: `$eq`, `$ne`, `$in`, `$nin`, `$regex`, `$gt`, `$gte`, `$lt`, `$lte`, composed
with `$and` / `$or`.

Over **exactly three namespaces**:

| Namespace | Resolves to |
|---|---|
| `metadata.<key>` | `context.metadata[<key>]` — caller-supplied |
| `params.<key>` | `context.params[<key>]` — LLM request parameters |
| `url.pathname` | full request path |

Adversarial re-fetch confirmed *"No mention exists"* of headers/body/user/request namespaces —
those three are the complete set.

> **⚠ Two design-relevant limits Vox should NOT inherit blindly:**
>
> 1. **There is no computed token-count field.** You can match `params.max_tokens` (a request
>    parameter) but not the *actual* prompt token count. So a context-length-based
>    local-vs-cloud threshold requires the caller to precompute it and pass it as metadata.
>    **Vox should expose computed fields natively** — prompt tokens, estimated complexity,
>    file sensitivity — because it controls both sides of the boundary and Portkey does not.
> 2. *"Only two-segment keys are supported (for example, `metadata.user_plan`,
>    `params.model`). Nested paths like `metadata.features.new_model_enabled` are not
>    supported."* "Nestable" refers to `$and`/`$or` **boolean** nesting, not nested metadata
>    objects.

### 3.4 Metadata-driven residency routing — and its trust boundary (confirmed 3-0)

Caller sends `metadata={"user_region": "EU"}` (via `client.with_options(...)` or the
`x-portkey-metadata` header); config matches
`{"query": {"metadata.user_region": {"$eq": "EU"}}, "then": "eu-backup"}`.

> **Two caveats that matter enormously for Vox:**
>
> 1. The local-endpoint-as-residency-target application is an **inference** — Portkey's docs
>    only demonstrate routing between gateway-configured *cloud* targets, never a local
>    Ollama/vLLM target.
> 2. **Metadata is caller-asserted, so it is a routing hint, not an enforced trust boundary.**
>    A harness treating *"this file is sensitive → route local"* as a **security guarantee**
>    must enforce it **below** the routing layer.
>
> For Vox this is decisive. A "never send this repo's contents to the cloud" control belongs
> in `vox-llm-egress` / `vox-redact` / `vox-bounded-fs` — an egress boundary that *cannot* be
> satisfied by a cloud model — **not** in a routing preference. Routing chooses among
> *permitted* candidates; it must never be the thing that makes a candidate impermissible.

---

## 4. RouteLLM — learned query-level routing

### 4.1 The result, honestly stated (confidence: medium, 2-1)

Ong et al. (LMSYS/Berkeley), arXiv 2406.18665 v4 (2025-02-23): a preference-trained binary
router (strong vs weak model) reports *"cost savings of up to 3.66x"* on MT Bench while
retaining **~95% of GPT-4 quality** at the CPT(50%) threshold, needing only **13.40%** of
calls to hit the strong model.

Metrics defined: **PGR** = fraction of the weak→strong performance gap recovered.
**CPT(x%)** = minimum % of strong-model calls needed to reach PGR = x%.

> **⚠ Critical qualifiers — the headline number is the least reproducible part:**
>
> - The multiplier is measured **against a random-routing baseline at equal cost**, *not*
>   against always-calling-the-strong-model. Versus GPT-4-only, reductions are ~85% (MT Bench),
>   45% (MMLU), 35% (GSM8K).
> - **3.66x is best-case and MT-Bench-specific** (80 LLM-judged questions). MMLU CPT(50%) is
>   only **1.41x** and GSM8K **1.49x** — *neither clears 2x*.
> - The model pair is `gpt-4-1106-preview` vs `Mixtral-8x7B`, a mid-2024 price/quality gap
>   **that no longer exists in that form**.
>
> Treat this as evidence the **technique** works, not a multiplier reproducible in 2026.

### 4.2 The training signal (confirmed 3-0)

Human preference data rather than task labels:

- *"We primarily use 80k battles from the online Chatbot Arena platform"* → **65k pairwise
  comparisons across 64 models** after a 5k validation holdout and pruning.
- Augmented with ~**1,500 MMLU** validation questions carrying golden labels.
- Plus ~**120K GPT-4-judge-labeled Nectar samples**, collected for ~**$700 USD**.

> **Precision notes:** prefer "80k collected, 65k trained on." And "rather than task labels"
> is slightly loose — the MMLU augmentation *is* golden task labels.

**Practically relevant:** pretrained router checkpoints from exactly this recipe ship in the
public repo, so a harness can adopt the artifact without reproducing the data collection.

**But for Vox the more important observation is the $700 line.** A GPT-4-judge-labeled
preference set over one's own workload is cheap. Vox generates exactly this data already —
every task with a graded outcome is a labeled sample — and currently discards it. See the
parity plan's telemetry phase.

---

## 5. The transplantable recipe

Synthesizing what survived, in the order a request flows:

```
1.  DEPLOYMENT REGISTRY
    logical name → N deployments {provider-prefixed id, base_url, cost, context,
                                  capabilities, locality, privacy class}
    local endpoints registered identically to cloud ones

2.  HARD FILTERS  (a failure here removes the candidate)
    ├── privacy / residency class      ← one-way ratchet, cannot be loosened per-request
    ├── required capabilities          ← tools, vision, json mode
    ├── context window ≥ prompt tokens ← computed natively, not caller-asserted
    ├── provider key present
    └── max_price

3.  SOFT RANK  (a failure here deprioritizes, never excludes)
    ├── measured latency  — rolling window, TTFT for streaming
    ├── cost              — WITH a capability floor, never bare "cheapest"
    ├── quality signal    — local eval history, NOT crowd spend
    └── stochastic weighting so one deployment isn't pinned

4.  FALLBACK CASCADE
    ├── general fallbacks         (on error, after num_retries)
    ├── context_window_fallbacks  (on normalized overflow error)
    └── content_policy_fallbacks

5.  POLICY AS DATA
    first-match-wins conditions, fail-soft on missing keys, mandatory default
```

### 5.1 What does NOT transplant

Recorded so nobody tries:

| Mechanism | Why not |
|---|---|
| OpenRouter's Share-of-Spend ranking | Proprietary community telemetry. Substitute local eval history. |
| RouteLLM's 3.66x multiplier | Model-pair-specific, mid-2024 price gap, measured vs random baseline. The *technique* transplants; the number does not. |
| Portkey's caller-asserted metadata as a privacy control | Hint, not boundary. Vox must enforce below the routing layer. |
| LiteLLM's bare cost-based routing | Comparative, not threshold-based. A zero-cost local endpoint wins unconditionally. |
| The Auto Beta benchmark table | Vendor self-benchmark, tiny n, internally non-monotonic, superseded by Auto Exacto. |

---

## 6. Mapping onto Vox's existing selector

`models::decide` already implements more of stage 2 and 3 than any single system reviewed.
The delta:

| Recipe element | Vox today | Gap |
|---|---|---|
| Heterogeneous deployments incl. local | ✅ `ChatProviderRouteKind` — 5 lanes incl. Ollama | — |
| Capability filter | ✅ `required_capabilities` | — |
| Key-presence filter | ✅ `ModelRegistry::key_is_present_for` | — |
| Health/confidence gating | ✅ `ModelConfidence` + exploration budget | Excludes where OpenRouter deprioritizes |
| Multi-axis rank | ✅ `SelectionAxes` (cost/responsiveness/intelligence, 0–100) | More expressive than any reviewed; **no UI** |
| Decision transparency | ✅ `rejection_reasons`, `alternatives`, `score_breakdown` | Better than all reviewed; **not surfaced** |
| **Privacy filter** | ❌ | **F7** — `local_only` is mesh placement, not inference |
| **Context-window filter + overflow fallback** | ❌ | No normalized overflow error across 5 lanes |
| **Measured latency signal** | ❌ | No rolling TTFT telemetry feeding selection |
| **Quality signal from own evals** | ❌ | `vox-eval` + reliability tables exist, unconnected |
| **Declarative policy file** | ❌ | `prefer_local` hardcoded at `select.rs:500` |
| **Local→cloud escalation cascade** | ❌ | Free-tier cascade exists; no capability cascade |
| **User-facing dial** | ❌ | **F9** — no control at all |

**Nine of thirteen rows are already built.** The routing work is four additions and one UI,
not a new engine.

---

## 7. Concrete recommendation for Vox

1. **Add a privacy axis as a hard filter with one-way ratchet semantics**, modelled on
   OpenRouter's `zdr`. Enforce the actual boundary in `vox-llm-egress`, not in routing.
   Rename or alias `VOX_MESH_EXEC_POLICY` so it stops reading as a privacy control.
2. **Add a context-window filter and a normalized overflow error** across the five provider
   lanes, then a `context_window_fallbacks` cascade. LiteLLM's error-string normalization is
   directly copyable.
3. **Record TTFT per deployment in a rolling window** and feed it into the responsiveness
   axis. This is the one signal Vox scores on but never measures.
4. **Feed `vox-eval` results into the intelligence axis** as the substitute for OpenRouter's
   Share of Spend — a better signal, measured on the actual workload.
5. **Ship a declarative policy file** with first-match-wins conditions, fail-soft on missing
   keys, and a mandatory default. Support computed fields (`prompt_tokens`,
   `sensitivity_class`) that Portkey cannot.
6. **Expose one preset dial by default**, three axes behind "advanced." Presets:
   `private` (local-only hard filter) · `frugal` · `balanced` · `best`.
7. **Never expose a bare "prefer local" boolean.** Pair every locality preference with a
   capability floor, or reproduce LiteLLM's zero-cost trap exactly.

---

## 8. Coverage gap and open questions

**The research question's second half went unanswered.** No surviving evidence covers:

1. **Hardware detection** — how a harness should decide whether *this machine* can run a
   local model acceptably (VRAM, quantization fit, thermal/battery state, concurrent load).
   Vox has `vox-plugin-nvml-probe` and `vox-quantize`; neither is wired to routing.
2. **How Claude Code, Cursor, Continue.dev, Aider, and Zed actually expose model choice and
   local-model support to users.** This is the closest available UX prior art for the dial in
   §7.6 and it remains unresearched.

Both are carried into research batch 4.

**Additional open questions:**

3. What are Portkey's runtime nesting-depth limits? Documented capability was verified;
   runtime behaviour was not.
4. What is Auto Exacto's actual algorithm? It superseded Auto Beta as the default
   (~2026-03-10) and its benchmark table is explicitly incomplete.
5. Does anyone publish a *threshold-based* (rather than comparative) cost router? Every system
   reviewed picks the cheapest passing candidate; none escalates above a measured complexity
   bound, which is what a local-first harness actually needs.
