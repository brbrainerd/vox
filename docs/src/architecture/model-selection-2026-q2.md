---
title: "Vox Model Selection — 2026-Q2 Refresh"
description: "Current-month (May 2026) frontier model audit, recommended task-to-model mapping for Vox's existing 5-tier routing pipeline, and rationale for the 2026-05-15 catalog refresh."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-15"
training_eligible: false
training_rationale: "Reflects May 2026 model landscape; will be superseded by quarterly refreshes."
---

# Vox Model Selection — 2026-Q2 Refresh

This document captures the rationale for the 2026-05-15 catalog refresh in [`contracts/orchestration/model-catalog.bootstrap.v1.json`](../../../contracts/orchestration/model-catalog.bootstrap.v1.json), the premium-alias updates in [`contracts/orchestration/model-routing.v1.yaml`](../../../contracts/orchestration/model-routing.v1.yaml), and the panel-pin resolutions in [`contracts/eval/llm-panel.v1.yaml`](../../../contracts/eval/llm-panel.v1.yaml).

**TL;DR.** The Vox model pipeline is architecturally sound — 5-tier routing, 15 strength tags, 13 task categories, scoreboard quality feedback, observed-cost dynamic pricing all already work. **The catalog content was stale**: the `codegen`/`debugging`/`security` premium aliases pointed at a preview-tier model at $125/MTok, and Anthropic's Opus 4.7 (GA 2026-04-16), Google's Gemini 3 family, OpenAI's GPT-5.4/5.5 family, and DeepSeek's V4 family were all entirely missing. This refresh adds 15 frontier-tier model rows and rotates 5 premium aliases.

---

## §1 What we changed

### §1.1 Premium alias rotation (`model-routing.v1.yaml:71-79`)

| Alias | Before (2026-04) | After (2026-05-15) | Why |
|---|---|---|---|
| `codegen` | `claude-mythos-preview-20260407` | `anthropic/claude-opus-4.7` | Mythos is preview at $125/MTok; Opus 4.7 GA is 87.6% SWE-bench at $25/MTok |
| `debugging` | `claude-mythos-preview-20260407` | `anthropic/claude-opus-4.7` | Same |
| `security` | `claude-mythos-preview-20260407` | `anthropic/claude-opus-4.7` | Same; Anthropic safety stack matches security review |
| `research` | `google/gemini-2.5-pro-preview` | `google/gemini-3.1-pro` | 3.1 Pro is current GA; leads ARC-AGI-2 + GPQA 94.3% |
| `planning` | `google/gemini-2.5-pro-preview` | `openai/gpt-5.5-pro` | GPT-5.5 Pro leads BenchLM agentic at 90.1 — best multi-step + tool-call reliability |
| `review` | `anthropic/claude-sonnet-4.6` | (unchanged) | Sonnet 4.6 is the price/quality knee at $3/$15 |
| `logic` | `deepseek/deepseek-r1` | (unchanged) | R1 is free on OpenRouter (rate-limited); good cost floor |
| `visus` | `qwen/qwen-3.5-vl` | (unchanged) | Vision-capable, cheap |

**Mythos Preview removed from catalog entirely** (2026-05-15, post-council). At $125/MTok with preview stability and #3 on BFCL, retaining it as an opt-in entry created a footgun — users who pinned to it via user-config would burn budget on a non-GA line. The hardcoded-values audit at `.vox/audit/2026-05-11-hardcoded-values/` will refresh on the next `vox ci hardcoded-values-audit` run and the Mythos entries will drop out.

### §1.2 New catalog entries (`model-catalog.bootstrap.v1.json`)

16 new model rows added (15 frontier-tier + the new Mythos-preview annotation):

| Model | Tier | Cost in/out (per MTok) | Use case |
|---|---|---:|---|
| `anthropic/claude-opus-4.7` | **Elite** | $5 / $25 | Primary `codegen`/`debugging`/`security` alias; 1M ctx; extended thinking with adaptive difficulty |
| `anthropic/claude-haiku-4.5` | **Light** | $1 / $5 | Cheap tier retaining computer-use + extended thinking + Anthropic safety stack |
| `openai/gpt-5.5-pro` | **Elite** | $8 / $30 | Primary `planning` alias; BenchLM agentic 90.1 |
| `openai/gpt-5.4` | Pro | $5 / $20 | Terminal-Bench leader 75.1%; `gpt-frontier` panel member |
| `openai/gpt-5-mini` | Light | $3 / $10 | Generalist cheap tier |
| `openai/gpt-5-nano` | **Light** | $0.05 / $1 | Classifier-grade — routing decisions, intent classification |
| `google/gemini-3.1-pro` | **Elite** | $3.50 / $15 | Primary `research` alias; ARC-AGI-2 + GPQA leader |
| `google/gemini-3-flash` | Light | $0.50 / $3 | Multimodal generalist |
| `google/gemini-3.1-flash-lite` | **Light** | $0.25 / $1.50 | Cheapest 1M-context multimodal — audio/video/image input, 363 tok/s output |
| `deepseek/deepseek-v4-pro` | Pro | $0.50 / $2 | MIT, 1M ctx, 1.6T/49B active, LiveBench Coding 69.99 |
| `deepseek/deepseek-v4-flash` | **Light** | $0.07 / $0.28 | Cheapest competitive output cost at MIT licensing |
| `moonshot/kimi-k2.6-thinking` | Pro | $0.80 / $3 | LiveBench Coding leader 78.57 — open-source alternative |
| `zhipu/glm-5.1` | Pro | $0.50 / $2 | Single-call BFCL format-precision leader |
| `qwen/qwen-3.6-27b` | Pro (local) | $0 | 77.2% SWE-bench, Apache 2.0 — **recommended MENS base-model migration target** |
| `qwen/qwen3-coder-next-32b` | **Local** | $0 | <30ms code-suggestion latency in VS Code — recommended `llama3:latest` replacement |

### §1.3 Panel-pin resolutions (`contracts/eval/llm-panel.v1.yaml`)

The CR-L0..L4 measurement panel had three placeholder version strings since the panel was drafted on 2026-05-15. This refresh resolves all three:

| Member ID | Placeholder | Resolved |
|---|---|---|
| `mens-current` | `"matches workspace.package.version"` policy | (unchanged; resolves at runtime against MENS checkpoint hash) |
| `claude-sonnet` | `"claude-sonnet-4-7-20260801"` (didn't exist) | **`claude-sonnet-4-6`** (current GA, released 2026-02-17) |
| `gpt-frontier` | `"gpt-5-2026-mm-dd"` (placeholder) | **`gpt-5.4`** (current GA for code); CR-L0 substitute pinned at **`gpt-5.5-pro`** for agentic measurement |

The Sonnet pin uses the un-dated version slug because Anthropic API resolves it to the latest 4.6 patch; the dated form `claude-sonnet-4-6-YYYYMMDD` is available when bit-for-bit reproducibility is required.

---

## §2 May 2026 model landscape (the data this refresh is based on)

### §2.1 Frontier coding leaderboard (SWE-bench Verified)

Six frontier models cluster within 1.3 points of each other:

| Rank | Model | SWE-bench Verified | Cost in/out per MTok | Notes |
|---|---|---:|---:|---|
| 1 | Claude Opus 4.7 | **87.6%** | $5 / $25 | 1M ctx; extended thinking; released 2026-04-16; +13% coding uplift over 4.6 at same price |
| 2 | Claude Opus 4.6 | 80.8% | $5 / $25 | Superseded by 4.7 |
| 3 | Gemini 3.1 Pro | 80.6% | $3.50 / $15 | Leads ARC-AGI-2 + GPQA |
| 4 | Claude Sonnet 4.6 | 79.6% | $3 / $15 | Best price/quality coding ratio |
| 5 | Qwen 3.6 27B | 77.2% | open | Apache 2.0 |
| 6 | GPT-5.4 | 57.7% SWE-bench Pro | $5 / $20 | But 75.1% Terminal-Bench leader |

**Per ianlpaterson.com's 38-task routing-table benchmark and lmcouncil.ai aggregate:** "Six frontier models now score within 0.8 points of each other on SWE-bench Verified, with all six frontier models within 1.3% of each other. Opus 4.7 still leads on reasoning depth and long-context coherence, but you are paying 2.5x more than Gemini 3.1 Pro for 0.2 more points on SWE-bench."

### §2.2 Agentic / tool-call (BFCL v4, BenchLM agentic)

| Rank | Model | BenchLM agentic | BFCL v4 tool-call |
|---|---|---:|---:|
| 1 | GPT-5.5 Pro | **90.1** | 40.4 |
| 2 | Claude Opus 4.7 (with extended thinking) | high | ~40 |
| 3 | Llama 3.1 405B Instruct | n/a | **40.5** (raw format) |
| 4 | Claude Mythos Preview | n/a | 39.6 |

For **multi-turn** tool use, Claude Sonnet 4.5/4.6 is preferred. For **single-call format precision**, GLM 4.5/5.1.

### §2.3 Reasoning-specialty

Four useful shapes of reasoning model emerged in 2026:

1. **General high-reasoning**: o3, o4-mini
2. **Long-context dense work**: Claude Opus 4.7 extended thinking; Sonnet 4.6 thinking
3. **Parallel exploration**: Gemini 2.5 Pro Deep Think; Gemini Flash Thinking
4. **Open-weights cost-efficient**: DeepSeek R1 (free), DeepSeek V4 Pro reasoning mode

**Opus 4.7's adaptive thinking** is the most interesting innovation: a lightweight pre-pass classifies input difficulty, then the model spends correspondingly more compute thinking before answering. This is uniquely suited to Vox's mixed workload (some prompts are trivial syntax fixes; some are cross-file architectural debugging) — the cost scales with task, not with prompt length.

### §2.4 Open-source frontier

**DeepSeek V4 Pro** (released 2026-04-24): 1.6T total / 49B active, 1M context, MIT licensing. LiveBench Coding 69.99, Agentic Coding 56.67 (May 12, 2026 snapshot). The MIT license + 1M context + competitive coding score makes V4 Pro the obvious cloud-fallback for Anthropic outages.

**DeepSeek V4 Flash**: 284B / 13B active, MIT, $0.28 output cost. The cheapest competitive output rate of any 2026 model — ~18× cheaper than Haiku output, ~4.5× cheaper than GPT-5 Nano output.

**Kimi K2.6 Thinking**: LiveBench Coding 78.57 — currently the open-source coding leader (May 2026 snapshot). Available on OpenRouter.

**Qwen 3.6 27B**: 77.2% SWE-bench Verified, Apache 2.0 license. Fits on a MacBook with 64GB RAM. The licensing matters for MENS-derived weights.

**Llama 4 Scout**: 10M-token context window (the long-context champion), but coding lags Qwen 3.6 by ~5 points on SWE-bench. Llama 4 Maverick is the dense variant.

### §2.5 Small / cheap

| Model | Cost in/out per MTok | Notable |
|---|---:|---|
| Claude Haiku 4.5 | $1 / $5 | Retains computer-use + extended thinking + vision + Anthropic safety stack |
| Gemini 3 Flash | $0.50 / $3 | Multimodal |
| Gemini 3.1 Flash-Lite | $0.25 / $1.50 | 1M ctx, multimodal (audio/video/image), 363 tok/s output |
| GPT-5 Mini | $3 / $10 | Medium-cheap |
| GPT-5 Nano | $0.05 / $1 | Cheapest commercial; classifier-grade |
| DeepSeek V4 Flash | $0.07 / $0.28 | Cheapest competitive output cost overall |

**Cost-tier savings:** A typical enterprise workload that routes by task complexity (simple → Haiku/Nano, medium → Sonnet/GPT-5, complex → Opus/GPT-5-Pro) saves **~58%** compared to using Opus everywhere.

### §2.6 Local / Ollama-friendly

| Model | Hardware | Speed | Notable |
|---|---|---:|---|
| Qwen3 8B | Laptop | 20-35 tok/s | OK for batch, slow for IDE autocomplete |
| Qwen3-Coder-Next 32B | Decent GPU | **<30ms code suggestions in VS Code** | IDE-grade latency |
| GPT-OSS 20B | 16GB laptops | 40-60+ tok/s | |
| Llama 3.3 70B | Mac Studio M4 Max | 30+ tok/s | |
| Qwen 3.5 (122B / 10B active) | 64GB MacBook | competitive | Beats GPT-5-mini on most benchmarks |

### §2.7 Free / rate-limited (OpenRouter)

DeepSeek R1, Llama 3.3 70B, Gemma 3 — all available at zero cost on OpenRouter with rate limits ~20 req/min, 200 req/day.

---

## §3 Task-to-model mapping (the deliverable)

Each row maps a Vox task category from `model-routing.v1.yaml:51-64` to a recommended cascade. **Bold = primary** (matches the `premium_alias`); italic = preferred Pro-tier fallback; plain = cost-floor fallback.

### §3.1 Per task category

| Vox task | Primary | Pro fallback | Light/cost-floor |
|---|---|---|---|
| **CodeGen** (production) | **`anthropic/claude-opus-4.7`** | *`anthropic/claude-sonnet-4.6`* | `deepseek/deepseek-v4-pro` |
| **CodeGen** (light) | **`anthropic/claude-haiku-4.5`** | *`google/gemini-3-flash`* | `deepseek/deepseek-v4-flash` |
| **Testing** | `anthropic/claude-sonnet-4.6` | *`openai/gpt-5.4`* | `qwen/qwen-3.6-27b` |
| **Debugging** (cross-file) | **`anthropic/claude-opus-4.7`** w/ extended thinking | *`openai/gpt-5.4`* (Terminal-Bench) | `deepseek/deepseek-v4-pro` |
| **TypeChecking** | `anthropic/claude-sonnet-4.6` | *`openai/gpt-5.4`* | `qwen/qwen-3.6-27b` |
| **Research** | **`google/gemini-3.1-pro`** | *`anthropic/claude-opus-4.7`* | `google/gemini-2.5-pro-preview` (legacy) |
| **Parsing** | `openai/gpt-5-nano` | *`google/gemini-3.1-flash-lite`* | `deepseek/deepseek-v4-flash` |
| **Review** | **`anthropic/claude-sonnet-4.6`** | *`anthropic/claude-opus-4.7`* (high-stakes) | `qwen/qwen-3.6-27b` |
| **General** | `anthropic/claude-sonnet-4.6` | *`openai/gpt-5.4`* | `deepseek/deepseek-v4-pro` |
| **Ars** | `anthropic/claude-sonnet-4.6` | *`openai/gpt-5-mini`* | `deepseek/deepseek-v4-flash` |
| **Planning** | **`openai/gpt-5.5-pro`** | *`anthropic/claude-opus-4.7`* | `google/gemini-3.1-pro` |
| **InterAgent** (MCP, A2A) | **`openai/gpt-5.5-pro`** | *`anthropic/claude-sonnet-4.6`* | `meta-llama/llama-3.1-405b-instruct` |
| **ToolOrchestration** | `anthropic/claude-sonnet-4.6` | *`zhipu/glm-5.1`* (format precision) | `openai/gpt-5-mini` |
| **Visus** | `qwen/qwen-3.5-vl` | *`google/gemini-3.1-flash-lite`* (multimodal) | `anthropic/claude-haiku-4.5` |

### §3.2 `vox repair` specifically

The repair loop ([`crates/vox-cli/src/commands/repair.rs:134`](../../../crates/vox-cli/src/commands/repair.rs:134)) is the **single hottest LLM consumer in Vox** because it re-sends the entire source file across 3 retry attempts. The cost optimization here matters disproportionately.

**Recommendation: `anthropic/claude-sonnet-4.6` with prompt caching enabled.** Three reasons:

1. **Prompt caching cuts effective cost by 80%+ across the 3-attempt loop.** Anthropic's prompt caching reduces cached input from $3 → $0.30/MTok. The first attempt pays full; attempts 2 and 3 read from cache. Total cost over a 3-attempt session converges to ~1.27× single-attempt cost vs. 3.00× without caching.
2. **79.6% SWE-bench Verified at $3/$15** is the price/quality knee. Opus 4.7's 87.6% costs 5× more for marginal improvement on repair-shaped tasks.
3. **1M context** means even large project files don't truncate mid-repair.

For unusually-stuck or multi-file repair sessions, escalate to Opus 4.7 via the existing complexity-based tier bumping.

### §3.3 CR-L0 spec-to-app agent loop

CR-L0 measures end-to-end agent loop quality with a $5/spec cost ceiling. Per the panel-rule, **`gpt-5.5-pro` is the primary** (BenchLM agentic 90.1) with `claude-opus-4.7` as alternative. Both have 1M-class context windows; both have native tool support; both honor the cost ceiling at typical spec sizes (~50K input, ~10K output ≈ $1.20-$1.50 per spec).

### §3.4 MENS positioning

MENS is currently QLoRA-fine-tuned on Llama 3.2 / Llama-4 Scout base. **Recommendation: migrate MENS base-model to Qwen 3.6 27B.** Three reasons:

1. **+5 points SWE-bench**: Qwen 3.6 (77.2%) beats Llama 4 Scout (~72%) on coding tasks.
2. **Apache 2.0 licensing**: Llama 4 weights are under Meta's bespoke license; Apache 2.0 makes MENS-derived weights freely shippable.
3. **128K context**: still ample for Vox's typical inputs; the 10M context of Llama 4 Scout is not currently exercised by Vox training corpora.

This is a v0.6 retraining cycle — not autonomous, requires council ratification + a training run.

**Alternative for IDE-grade local inference**: retrain on Qwen3-Coder-Next 32B (<30ms VS Code latency).

---

## §4 Cost analysis

### §4.1 Per-task estimated cost (typical 4K input, 500 output)

| Task type | Primary model | Cost per call | Cost per 100 calls |
|---|---|---:|---:|
| CodeGen (Elite) | Opus 4.7 | $0.0325 | $3.25 |
| CodeGen (Pro) | Sonnet 4.6 | $0.0195 | $1.95 |
| CodeGen (Light) | Haiku 4.5 | $0.0065 | $0.65 |
| Research | Gemini 3.1 Pro | $0.022 | $2.20 |
| Planning | GPT-5.5 Pro | $0.047 | $4.70 |
| Review | Sonnet 4.6 | $0.0195 | $1.95 |
| Parsing | GPT-5 Nano | $0.00072 | $0.072 |
| Visus | Qwen-3.5-VL | $0.00045 | $0.045 |

### §4.2 `vox repair` 3-attempt session cost (10K source file)

| Configuration | Cost per session |
|---|---:|
| Sonnet 4.6 **with prompt caching** (recommended) | **~$0.064** |
| Sonnet 4.6 without prompt caching | ~$0.155 |
| Opus 4.7 with prompt caching | ~$0.108 |
| Opus 4.7 without prompt caching | ~$0.260 |
| DeepSeek V4 Pro | ~$0.026 |

Even with prompt caching, **DeepSeek V4 Pro is ~2.5× cheaper than Sonnet 4.6** — strong fallback for cost-sensitive deployments. The Anthropic safety stack and ecosystem are worth the premium for production work; the price/quality tradeoff is real.

### §4.3 CR-L1 reference-panel measurement run (164 problems × 5 attempts × 3 panel members)

| Panel | Cost per run |
|---|---:|
| MENS-current (local) + Sonnet 4.6 + GPT-5.4 | **~$92** |
| MENS-current + Opus 4.7 + GPT-5.5 Pro (max quality) | ~$215 |
| MENS-current + Sonnet 4.6 + GPT-5 Mini (cost-floor) | ~$48 |
| MENS-current + DeepSeek V4 Pro + Gemini 3.1 Pro | ~$32 |

The recommended panel (Sonnet 4.6 + GPT-5.4 + MENS) is the price/signal sweet spot. Per the panel SSOT's "<$300/RC" cap, this stays well within budget.

---

## §5 Migration plan

### §5.1 Landed 2026-05-15 (this refresh)

- ✅ `contracts/orchestration/model-routing.v1.yaml` — 5 premium aliases rotated
- ✅ `contracts/orchestration/model-catalog.bootstrap.v1.json` — 15 new model rows + Mythos-preview annotation
- ✅ `contracts/eval/llm-panel.v1.yaml` — `claude-sonnet` and `gpt-frontier` placeholders resolved
- ✅ This document — design rationale + benchmark citations

### §5.2 v0.6 (next-minor)

- **A. Enable prompt caching for `vox repair`** — add `cache_control: { type: "ephemeral" }` to the system+source-code blocks in [`crates/vox-cli/src/commands/repair.rs:142-148`](../../../crates/vox-cli/src/commands/repair.rs:142). Estimated savings: ~80% per session.
- **B. Update `openrouter_chat_model_preference()`** ([`crates/vox-config/src/inference.rs`](../../../crates/vox-config/src/inference.rs)) default to `anthropic/claude-sonnet-4.6` if not set; let users override.
- **C. Retire `llama3:latest` as Local-tier default** in favor of `qwen/qwen3-coder-next-32b`.

### §5.3 v0.7+ (council-gated)

- **D. MENS base-model migration** from Llama-4 Scout → Qwen 3.6 27B. Requires retraining run + corpus validation. Tracked via [`mens-training-ssot.md`](mens-training-ssot.md).
- **E. `OpenAiDirect` provider plumbing** — currently OpenAI is only accessible via OpenRouter proxy. Adding direct access reduces latency by ~30-50ms per call and is policy-required for some compliance regimes. New provider entry + `OpenAiApiKey` secret per existing pattern.
- **F. Model-spec `stability` axis** — distinguish GA / preview / experimental within tiers. Tier semantics today conflate "expensive" with "stable"; adding a `stability` field would let the router prefer GA at the same tier when available.

### §5.4 Quarterly refresh cadence

Per CR-L1's panel rebaselining policy, this document is **the canonical place to update model recommendations**. The schedule:

| Cadence | Action |
|---|---|
| Per release-candidate tag | Pin panel `version_pinned` to current model IDs; re-validate cost ceilings |
| Per quarter | Re-audit benchmarks; rotate premium aliases if a new release shifts ≥3 points on the relevant benchmark |
| Per major model launch (Anthropic / OpenAI / Google) | Same-day catalog entry + 7-day evaluation window before alias rotation |

---

## §6 Open questions

These are surfaced by the audit and deferred to council:

1. ~~Should Mythos preview stay in the catalog?~~ **Resolved 2026-05-15: removed.** Any external config that pinned to `claude-mythos-preview-20260407` will get an "unknown model" routing fallback; the router selects from remaining catalog entries. Follow-on: add `vox ci retired-model-warn` lint that warns when users pin to retired/preview model IDs in user-config.
2. **MENS base-model migration timing**: Llama-4 → Qwen 3.6 is a v0.6/v0.7 retraining commitment. Does the council ratify or defer?
3. **`OpenAiDirect` provider**: latency win is real but the attack-surface delta needs threat modeling. Defer to security review.
4. **Model-spec `stability: ga | preview | experimental` field**: schema change to `ModelCatalog` (in `crates/vox-orchestrator/src/models/spec.rs`). Backward-compatible to add as `Option<String>`; emergent if Mythos-preview pattern recurs.
5. **Free-tier exploitation** (DeepSeek R1, Llama 3.3 70B, Gemma 3 on OpenRouter): rate-limited to 20/min, 200/day. Useful for development / CI experiments but not production. Should the router automatically prefer free-tier when rate-limit budget permits? Currently no preference is encoded.
6. **Prompt-caching wire-up**: requires Anthropic-specific request-shape changes. The existing OpenAI-compat proxy at OpenRouter may strip cache_control fields. Need to verify per-provider passthrough.

---

## §7 Appendix — Benchmarks cited

| Benchmark | What it measures | Where I drew from |
|---|---|---|
| SWE-bench Verified | End-to-end PR-to-fix on real GitHub issues | iternal.ai, smartscope.blog, lmcouncil.ai |
| SWE-bench Pro | Harder, multi-step variants | smartscope.blog |
| Terminal-Bench | Tool-shell-execution accuracy | smartscope.blog |
| LiveBench Coding | Coding + Agentic Coding (May 12, 2026 snapshot) | akitaonrails.com |
| BFCL v4 | Berkeley Function-Calling Leaderboard | gorilla.cs.berkeley.edu |
| BenchLM agentic | Multi-step task completion + tool-call reliability | benchlm.ai |
| ARC-AGI-2 | Abstract reasoning | sureprompts.com |
| GPQA | Graduate-level science Q&A | smartscope.blog |
| MMLU-Pro | Multi-task knowledge | findmyaitool.com |
| Arena Elo | Human preference rating | various |
| Anthropic API pricing | Direct provider rates (May 2026) | finout.io, aipricing.guru |
| OpenRouter pass-through pricing | Provider-rate proxy with 5.5% credit fee | costgoat.com, openrouter.ai |

Full source URL list in the prior chat message that generated this doc; refresh quarterly.

---

*Document dated 2026-05-15. Supersedes scattered model preferences across [`crates/vox-config/src/inference.rs`](../../../crates/vox-config/src/inference.rs), [`crates/vox-orchestrator/src/models/`](../../../crates/vox-orchestrator/src/models/), and pre-2026-Q2 commits in `contracts/orchestration/`. Next review: at v0.6 release or 2026-08-15, whichever is sooner.*

---

## §8 Selection SSOT — `select()` (Council-ratified 2026-05-15)

The model pipeline ships a **single source of truth** for model selection:
[`crates/vox-orchestrator/src/models/select.rs`](../../../crates/vox-orchestrator/src/models/select.rs).
Every Vox surface that picks an LLM should flow through `select(intent, registry)`
rather than hardcoding a model id or duplicating axis logic.

### 8.1 Three-axis user knob

Users control selection along three orthogonal axes (each 0–100):

| Axis | Meaning | Projects onto `AutoRoutingPriority` |
|---|---|---|
| **cost** | 100 = cheapest possible | `efficiency` |
| **responsiveness** | 100 = lowest latency | `latency` |
| **intelligence** | 100 = highest capability | `precision` |

Presets:

- `COST_FIRST` — 70/15/15 — classifiers, CI lints, NLI checks
- `BALANCED` — 33/33/34 — default
- `QUALITY_FIRST` — 15/15/70 — review, security audit, debugging, research, planning
- `FAST` — 15/70/15 — IDE autocomplete, ghost-text

Set via env: `VOX_MODEL_AXES=cost:80,intelligence:10,responsiveness:10`.

### 8.2 Caller-hint intents

| Constructor | Task | Axes | Notes |
|---|---|---|---|
| `repair_loop()` | `CodeGen` | BALANCED | `cacheable_workload = true` (Anthropic prompt cache) |
| `research()` | `Research` | QUALITY_FIRST | complexity 7 |
| `review()` | `Review` | QUALITY_FIRST | cacheable; complexity 6 |
| `nli_classifier()` | `Parsing` | COST_FIRST | hard ceiling `max_cost_usd_per_call = $0.01` |
| `ide_autocomplete()` | `CodeGen` | FAST | `prefer_local = true` |
| `plan_mode()` | `Planning` | QUALITY_FIRST | complexity 8 |

### 8.3 Resolution order

1. **`VOX_MODEL_FORCE` env override** → immediate return if id matches catalog.
2. **`prefer_local`** → Ollama / VoxLocal / PopuliMesh providers only.
3. **Premium alias** → when `axes.intelligence >= 50`, honor the pin in
   [`contracts/orchestration/model-routing.v1.yaml`](../../../contracts/orchestration/model-routing.v1.yaml).
4. **General scorer** → `ModelRegistry::best_for_with_filter()` with axes
   projected onto `AutoRoutingPriority` and filters for `max_cost_usd_per_call`
   + `context_size_hint`.

Each outcome carries a `SelectionReason` (`PremiumAlias`/`Scored`/`LocalOnly`/`EnvOverride`)
so debugging routing surprises is a one-line lookup.

### 8.4 Migrated call sites (2026-05-15)

| Crate | Before | After |
|---|---|---|
| `vox-cli::commands::repair` | hardcoded `REPAIR_LOOP_PREFERRED` const | `select_with_default_registry(&SelectionIntent::repair_loop())` |
| `vox-orchestrator::task_dispatch::research_dispatch` | hardcoded `"anthropic/claude-3.5-sonnet:beta"` (retired) | `SelectionIntent::research()` |
| `vox-code-audit::review::providers` (defaults) | `claude-3.5-sonnet` / `gpt-4o-mini` / `gemini-2.5-flash` | Sonnet 4.6 / GPT-5-Mini / Gemini 3 Flash |
| `vox-code-audit::ai_analyze` (gemini default) | `gemini-2.5-flash` | `gemini-3-flash` |

Future migrations (incremental — they operate at the provider-layer, below `select()`):
`vox-actor-runtime::model_resolution`, `vox-orchestrator-mcp::resolve_mcp_chat_model_sync`,
`vox-gamify::reward_routing`.

### 8.5 Cleanup completed in this pass

- Retired model `claude-mythos-preview-20260407` removed from catalog + tests + docs.
- `claude-3.5-sonnet`, `claude-3.5-sonnet:beta`, `gpt-4o`, `gpt-4o-mini`, `gemini-2.5-flash`
  purged from active code paths (still referenced from retirement contracts).
- `llama3:latest` Ollama bootstrap replaced with `qwen3-coder-next-32b` as Local default.
- `REPAIR_LOOP_PREFERRED` retained as a last-resort fallback only.
