---
title: "Deep Research System Best Practices — Free-Tier-First, Tavily-95%-Slice Research"
description: "Verified deep-research findings (2025-2026 SOTA) on building a self-hosted, model-agnostic, free-tier-first deep research pipeline that captures ~95% of Tavily for $0 and falls back to paid only for the irreducible 5%. Covers the agentic research loop, hybrid retrieval + RRF + reranking + novelty, Rust-native web search/extraction (no paid API), the Tavily replicability gap, an OpenRouter free-tier-first LLM cascade, and knowledge-base self-build. Every claim carries a verification status; maps each finding to Vox crates (vox-search, vox-research-shim, vox-actor-runtime, MENS) and gestures toward an implementation plan."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-06-18"
training_eligible: true
training_rationale: "Source-verified best-practice synthesis for a free-tier-first deep research system: 4-stage agentic loop, hybrid retrieval/RRF/reranking (with measured deltas)/novelty, Rust-native scraping + content extraction, OpenRouter free-tier cascade with paid fallback, agent-memory freshness prescriptions, and the irreducible Tavily gap. Maps directly to implementable Vox capabilities."
schema_type: "TechArticle"
audience: ["contributors", "agents"]
related:
  - docs/src/architecture/deep-research-capabilities-audit-2026-06-17.md
  - docs/src/architecture/deep-research-prior-art-and-vox-roadmap-2026.md
  - docs/src/reference/tavily-integration-ssot.md
  - docs/src/architecture/search-retrieval-ssot-2026.md
  - crates/vox-search/src/web_dispatcher.rs
  - crates/vox-search/src/crag.rs
  - crates/vox-research-shim/src/research/orchestrator/pipeline.rs
---

# Deep Research System Best Practices — Free-Tier-First, Tavily-95%-Slice Research

> **Status:** Research & synthesis (no implementation). Terminus of a deep-research run; feeds a future `writing-plans` cycle.
> **Date:** 2026-06-18
> **Method:** Two passes. Pass 1 = Vox `deep-research` workflow fan-out (6 angles, 11 sources, 47 claims) — its verification stage was blanket rate-limited (all `0-0 abstain`), so its verdicts were discarded. Pass 2 = **direct source re-fetch of every load-bearing claim** (this section's ✅ marks), reading the primary papers/repos/docs rather than relying on the rate-limited vote.

## 0. Verification legend

Every substantive claim below is tagged:

- **✅ Verified** — re-fetched and read from the primary source this session (2026-06-18); quote/number confirmed.
- **◐ Partial** — confirmed in part; a sub-claim is nuanced or domain-limited (caveat stated inline).
- **○ Lead** — sourced but not independently re-confirmed; carried because it agrees with verified material and the existing Vox audit. Do not cite as fact without live re-check.

This honesty split exists because the workflow's adversarial-verify pass was rate-limited into uniform abstention; treating abstention as truth (either direction) would be wrong.

---

## 1. What "deep research" is (the canonical loop)

**✅ Verified** — [Zhang et al., "Deep Research: A Survey of Autonomous Research Agents," arXiv:2508.12752](https://arxiv.org/abs/2508.12752) (Aug 2025) defines **four core stages**: *planning, question developing, web exploration, and report generation.*

**✅ Verified — two web-exploration strategies** (same survey, full text):
- **Browser-based autonomous agents** — *"autonomous agent systems that conduct web retrieval including operations like browse, click, and extract information much like a human researcher."*
- **API-based retrieval** — *"directly using the current web search engine (e.g., Google Search, Bing Search) that let developers pull ranked documents or snippets directly into research pipelines."*

Vox is firmly the **API/retrieval-based** lineage (SearXNG/DDG/Tavily + scraper), with `chromiumoxide` as an optional browser-tier escalation — the correct, cheaper default.

**✅ Verified — factuality as a first-class component** (same survey):
- **FaithfulRAG** — *"introduces fact-level conflict modeling to promote alignment with consistent retrieved facts."*
- **BRIDGE** — *"proposes a verification layer between retrieval and generation to assess factual adequacy."*
These validate Vox's claim-extraction + NLI-verify stages as on-trend, and argue for verification *gating retrieval* rather than running only post-hoc.

**✅ Verified — synthesis is the weakest stage** (same survey): *"Most existing methods target isolated subskills of report generation while lacking joint optimization with upstream components,"* and report generation *"remain[s] in an early stage."* Implication: Vox's synthesis quality is a differentiation opportunity, not a solved problem.

**The fuller operational loop** Vox already implements (plan → retrieve → CRAG-iterate → claim-extract → NLI-verify → synthesize → judge → promote) is a superset of the survey's four stages, adding the verification + promotion machinery the survey calls under-built.

**✅ Verified — the efficiency principle** ([Tavily deep-research writeup](https://huggingface.co/blog/Tavily/tavily-deep-research)): persist **only distilled reflections** between iterations, not raw tool outputs. Quote: *"only the set of past reflections should be used as context for your tool caller,"* with raw source text entering context *"at the point when your agent begins to prepare the final deliverable."* They model ReAct as quadratic `n·m(m+1)/2` (n tokens/iter, m iters) vs their linear `n·m`, a per-iteration saving factor of `(m+1)/2`, yielding a **66% token reduction vs Open Deep Research while reaching SOTA on DeepResearch Bench.** **This is the single highest-leverage architectural idea in the corpus, and Vox does not do it** — the pipeline accumulates raw evidence across hops.

---

## 2. Retrieval infrastructure best practices

**✅ Verified — reranking is the biggest single precision lever.** [arXiv:2604.01733, "From BM25 to Corrective RAG: Benchmarking Retrieval Strategies for Text-and-Table Documents"](https://arxiv.org/html/2604.01733v1) (23,088 queries / 7,318 financial docs) measured a two-stage **hybrid (BM25 + `text-embedding-3-large` via RRF) + Cohere Rerank v4.0 Pro** pipeline at **Recall@5 = 0.816, MRR@3 = 0.605**, and the reranker alone contributed **+17.2pp MRR@3 (0.433→0.605)** and **+12.1pp Recall@5 (0.695→0.816)** over unreranked hybrid. ◐ **Caveat:** domain is text-and-table financial documents — directionally strong, but treat the exact deltas as domain-specific, not a universal guarantee. They benchmarked BM25, dense, RRF hybrid, HyDE, Multi-Query, Contextual Retrieval, and CRAG side by side.

**○ Lead — hybrid > single-method, generally.** BM25 anchors short literal tokens (IPs, error codes, config flags, identifiers) that vector search misses; vector adds semantic recall; production needs both. RRF combines *ranks* (`score += 1/(k+rank)`, default **k=60**), avoiding cross-scale score normalization, and is strong zero-shot. (Multiple secondary/blog sources; consistent with the verified benchmark above.)

**✅ Verified — "long context is not memory."** [arXiv:2603.07670, "Memory for Autonomous LLM Agents: Mechanisms, Evaluation, and Emerging Frontiers"](https://arxiv.org/html/2603.07670v1): *"Despite context windows stretching to 200k tokens, long-context models consistently underperform purpose-built memory systems on tasks requiring selective retrieval and active management."* Models near-perfect on passive recall (LoCoMo) drop to **40–60% on MemoryArena**. This endorses Vox's retrieval-first design over "stuff the window."

**Vox mapping:** `memory_hybrid.rs` (BM25+vector), `rrf.rs` (k=60 already), `lexical_tantivy.rs`, `vector_qdrant.rs`, `novelty.rs` exist. **Confirmed gaps vs best practice: (a) cross-encoder reranking — none; (b) reflection-distillation — none.** These are the two highest-leverage retrieval-side additions.

---

## 3. Web search WITHOUT a paid API — the Rust-native stack

**✅ Verified — the reference open-Tavily exists and uses exactly this shape.** [`jianjungki/tavily-open`](https://github.com/jianjungki/tavily-open) builds a Tavily-equivalent on **SearXNG** (*"high-quality search results through SearXNG meta search engine"*) plus a **four-tier escalating extractor**: HTTP → Jina Reader → Remote Browserless/CDP → Local Playwright, *"browser rendering only when cheaper stages fail,"* with Redis caching to *"reduce redundant crawling."* ◐ **Nuance:** its reader tier is hosted **Jina Reader** (a paid/remote service); the **fully-local Rust equivalent** is the readability crates below — so Vox can match the design without any external reader dependency.

**✅ Verified — Rust-native extraction toolchain (live-fetched 2026-06-18):**

| Layer | Crate(s) | Role | In Vox? |
|---|---|---|---|
| Search / URL discovery | **SearXNG** (self-host) → **DuckDuckGo** | meta-search, free | ✅ `web_dispatcher.rs` tiers 2-3 |
| HTTP fetch | `reqwest` + `tokio` | fetch | ✅ |
| DOM/CSS extraction | `scraper` | selectors | ✅ `scraper.rs` |
| **Readability / boilerplate strip** | **`rs-trafilatura`** (page-type-aware; author reports F1≈0.966 on ScrapingHub, ~2.1× faster than Python trafilatura — ○ author-reported), **`trafilatura`** (go-trafilatura port), **`libreadability`** (Mozilla Readability port), **`justext`** (stopword-density) | clean main text | ❌ **the `/extract`-quality gap** |
| HTML→Markdown | `html2markdown` | LLM-ready text | ❌ |
| JS render (last resort) | `chromiumoxide` (CDP) | escalation only | ✅ `vox-plugin-browser` |
| Politeness | robots.txt + rate/jitter | compliance | ◐ `policy.scraper_robots_txt_respect` |

**Recommended staged extractor (matches the verified open-Tavily ordering, all-local):**
```
SearXNG/DDG (URLs) → reqwest GET
  → rs-trafilatura / libreadability   (≈90% of pages, $0, fast)
  → justext fallback                  (stubborn boilerplate)
  → chromiumoxide render              (only JS-heavy pages that failed above)
  → html2markdown → chunk → embed
```
This local readability layer is what closes most of the quality gap between raw `scraper.rs` output and Tavily's cleaned `content`.

---

## 4. The Tavily "95% slice" — what's replicable free, what isn't

**✅ Verified — Tavily's surface** ([docs.tavily.com](https://docs.tavily.com)): `/search` (ranked, LLM-ready snippets), `/extract` (clean content from URLs), `/crawl` + `/map` (sitegraph navigation), `/research` (autonomous multi-step agent). Free tier ≈ 1,000 credits/mo (per `tavily-integration-ssot.md`).

| Tavily capability | Free self-hosted equivalent | Replicable? |
|---|---|---|
| `/search` ranked snippets | SearXNG/DDG + **cross-encoder reranker** + authority scoring | ✅ ~95% (reranker closes the ranking gap — §2) |
| `/extract` clean content | **rs-trafilatura / libreadability** staged extractor (§3) | ✅ ~95% |
| `/research` autonomous loop | Vox `vox-research-shim` CRAG pipeline (LLM-driven) | ✅ architecture already exists |
| `/crawl` + `/map` sitegraph | `chromiumoxide` + breadth-first link map | ⚠️ ~80% (buildable, not built) |
| Proprietary fresh index + ms cached serving | — | ❌ **the irreducible ~5%** |
| API prompt-injection firewall | treat web content as untrusted + truncate | ◐ partial (policy exists) |

**Verdict:** ~95% of Tavily's *value* is reproducible at $0 with **SearXNG + local staged readability extraction + a cross-encoder reranker + the existing CRAG loop**. The irreducible ~5% is Tavily's **proprietary continuously-fresh index and millisecond cached serving** (○ the AWS-blog ms-latency figure is unverified but the *capability class* is real). Keep Tavily as an optional **paid Tier-4 fallback** for freshness-/latency-critical queries — exactly how `web_dispatcher.rs` already tiers it. **The 95% target does not require paying Tavily; paid is a fallback, not a dependency.**

---

## 5. OpenRouter as a free-tier-first research provider

**✅ Verified limits (live-fetched 2026-06-18, [OpenRouter](https://openrouter.ai/docs/api/reference/limits) / [costgoat — 27 free models, Jun 2026](https://costgoat.com/pricing/openrouter-free-models)):**

- `:free` models and the `openrouter/free` router: **20 requests/minute**, hard cap (credits do **not** raise RPM).
- Daily lever: **50 req/day** with <10 credits purchased; **1,000 req/day** once **≥$10 of credits** bought one time (never expires).
- No card for the 50/day floor. Failed calls still count. Base URL `https://openrouter.ai/api/v1`, OpenAI-compatible. ~27 free models live Jun 2026.

**Rate-limit math:** a full run is ~7-13 LLM calls → ~75-140 runs/day at 1,000 RPD, ~4-7 runs/day at the 50/day floor. The **20 RPM cap forces the LLM stages to serialize/throttle** — the pipeline batches search ~4-8 at a time but the LLM cascade needs the same governor.

**Recommended free-tier-first cascade (the headline wiring):**
```
Per research stage, in order:
  1. local Ollama / MENS spoke      ($0, private, no RPM cap)
  2. openrouter/free router          (DeepSeek-R1:free, Llama-3.3-70B:free, Gemma-3-27B:free)
  3. paid OpenRouter (auto)          (only if free exhausted AND budget allows)
  4. SearXNG-only structured output  (no-LLM degraded synthesis — never hard-fail)
```
**Invariant:** the cascade floor is *always free* — local + `openrouter/free` + SearXNG/DDG reachable at zero credits. This is audit gap **G4**: the infra (`virtual_models.rs` `openrouter/free`, `FreeTierRouter`) exists but the research cascade still defaults to paid `openrouter/auto`. **Wiring G4 is the single change that makes "OpenRouter as our own free research system" real.**

Per-stage fit (model-agnostic, all via `vox_actor_runtime::llm`):

| Stage | Free pick | Why |
|---|---|---|
| Planner / decompose | `deepseek/deepseek-r1:free` (164K, reasoning) | structured JSON sub-queries |
| LLM-CRAG expansion | `google/gemma-3-27b-it:free` | cheap instruction-following |
| Claim extraction | `meta-llama/llama-3.3-70b-instruct:free` | balanced |
| Verification (NLI) | `deepseek-r1:free`, temp 0 | CoT entailment |
| Synthesis | `llama-3.3-70b:free`, 4K tokens | long-form (raise from 1,800 — gap G6) |
| Judge | `gemma-3-27b-it:free`, temp 0 | fast structured scoring |

---

## 6. Knowledge-base self-build best practices

- **Chunking:** token-aware boundaries (`ingest.rs`); attach source URL, fetch date, authority score, corpus class.
- **Tagging / classification:** route by claim class (`vox-scientia/class_routing`); store provenance + confidence grade (★★★/★★/★).
- **✅ Verified — freshness is a prescribed best practice, not just a gap.** The memory survey ([arXiv:2603.07670](https://arxiv.org/html/2603.07670v1) §7.3) prescribes that *"robust systems need temporal versioning (prefer the newest record), source attribution (user statement >> agent inference), contradiction detection (flag conflicts for resolution), and periodic consolidation."* ◐ **Correction vs first draft:** this is a design *recommendation*, not an empirical claim that all systems lack it. Vox's retention/GC is currently manual — adopt these four mechanisms (temporal versioning, source attribution, contradiction detection, periodic consolidation) as the retention design.
- **Write-back novelty check:** before promoting a finding to SCIENTIA, score novelty (n-gram shingling, `novelty.rs`) against the last N promoted findings in-domain; promote only if novelty ≥ threshold. Prevents paraphrase bloat. (Audit gap G3 — bidirectional novelty.)
- **Graph/corpus construction:** keep feeding high-confidence (query, answer) pairs to MENS (`training_pair_min_confidence`) and finding-candidates to SCIENTIA (`discovery_bridge.rs`) — both wired; the missing piece is the novelty gate + the four freshness mechanisms on write-back.

---

## 7. Mapping to Vox + consolidated gap list

Re-confirms the 9 gaps in `deep-research-capabilities-audit-2026-06-17.md` and adds two verified architectural insights (reflection distillation, freshness mechanisms). Ranked by leverage for the free-tier-first / Tavily-95% goal:

| Rank | Gap | Maps to | Why it matters |
|---|---|---|---|
| 1 | **G4 — free-tier cascade wiring** | `cascade.rs`, `free_tier.rs` | the entire "free" thesis hinges on it |
| 2 | **Staged readability extraction** (NEW, ✅) | `scraper.rs` + `rs-trafilatura`/`libreadability` | closes Tavily `/extract` at $0 |
| 3 | **G2 — cross-encoder reranking** (✅ +17pp evidence) | new `reranker.rs` | closes Tavily `/search` ranking; biggest precision lever |
| 4 | **G1 — LLM-driven CRAG** | `crag.rs` | replaces brittle regex expansion |
| 5 | **Reflection distillation** (NEW, ✅) | `pipeline.rs` evidence accumulation | ~linear tokens; Tavily's 66% trick |
| 6 | **G3 — novelty / write-back gate** | `novelty.rs`, `discovery_bridge.rs` | dedup synthesis + KB write-back |
| 7 | **G6 — synthesis token budget** | `cascade.rs` apply_stage_defaults | 1,800 → 4,000; one-line unblock |
| 8 | **G5 — multi-signal confidence gate** | `gate.rs` | activates DeepResearch tier correctly |
| 9 | **Freshness/retention** (✅ 4 mechanisms) | corpus + SCIENTIA | temporal versioning, source attribution, contradiction detection, consolidation |
| 10 | G7/G8/G9 — semantic cache, durable jobs, span tracing | various | production hardening |

---

## 8. Gesture toward an implementation plan (not a plan)

A future `writing-plans` cycle should sequence roughly:

- **Wave 1 (free thesis, days):** G4 free-tier cascade + 20-RPM governor; G6 synthesis budget. → zero-cost runs work end-to-end.
- **Wave 2 (Tavily-95% slice, ~2 wks):** local staged `rs-trafilatura`/`libreadability` extractor in `scraper.rs`; cross-encoder reranker (`reranker.rs`, local Candle/MENS backend). → search+extract quality approaches Tavily without paying.
- **Wave 3 (loop quality, ~3 wks):** LLM-driven CRAG (G1); **reflection distillation**; novelty write-back gate (G3); multi-signal confidence gate (G5).
- **Wave 4 (KB durability):** the four freshness mechanisms + temporal versioning; semantic cache (G7); durable async jobs (G8).

**Architectural invariants** (per `AGENTS.md` + existing audit): all LLM calls through `vox_actor_runtime::llm`; new free model IDs as scored `ModelSpec` entries; new `SearchPolicy` fields mirrored with `VOX_SEARCH_*` env overrides; cascade floor always free; web content always untrusted.

---

## 9. Source ledger

**✅ Verified by direct fetch (2026-06-18):**
- Deep-research 4-stage canon, browser-vs-API, FaithfulRAG/BRIDGE, synthesis-immaturity — [arXiv:2508.12752](https://arxiv.org/abs/2508.12752) (Zhang et al., 2025)
- Reranking deltas (+17.2pp MRR@3 / +12.1pp Recall@5; 0.816/0.605) — [arXiv:2604.01733](https://arxiv.org/html/2604.01733v1) *(financial text-and-table domain)*
- Tavily 66% token cut via distilled-reflection persistence (quadratic→linear) — [huggingface.co/blog/Tavily](https://huggingface.co/blog/Tavily/tavily-deep-research)
- "Long context is not memory" (200k underperforms; 40-60% MemoryArena) + the four freshness mechanisms — [arXiv:2603.07670](https://arxiv.org/html/2603.07670v1)
- Open-Tavily = SearXNG + 4-tier staged extractor + Redis cache — [github.com/jianjungki/tavily-open](https://github.com/jianjungki/tavily-open)
- OpenRouter free-tier limits (20 RPM; 50→1,000 RPD) — [openrouter.ai/docs](https://openrouter.ai/docs/api/reference/limits), [costgoat](https://costgoat.com/pricing/openrouter-free-models)
- Rust extraction crates (rs-trafilatura/libreadability/justext/html2markdown) — [github.com/Murrough-Foley/rs-trafilatura](https://github.com/Murrough-Foley/rs-trafilatura), docs.rs
- Tavily endpoint surface — [docs.tavily.com](https://docs.tavily.com)

**○ Leads (sourced, not independently re-confirmed):** RRF k=60 generality and hybrid>single-method numbers ([emergentmind](https://www.emergentmind.com/topics/hybrid-retrieval), blog sources); rs-trafilatura's F1≈0.966 (author-reported); Tavily ms-latency cached serving ([AWS blog](https://aws.amazon.com/blogs/storage/how-tavily-reduced-ai-search-caching-costs-by-95-with-amazon-s3-express-one-zone/)). Re-verify before treating as hard targets.
</content>
