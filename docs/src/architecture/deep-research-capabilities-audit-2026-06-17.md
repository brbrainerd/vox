---
title: "Vox Deep Research Capabilities — Full Audit & 2026 Roadmap"
description: "Comprehensive audit of Vox's search, deep research, and RAG capabilities as of 2026-06-17. Maps every component to source, benchmarks against 2026 SOTA, identifies 9 ranked gaps (CRAG heuristic, novelty scoring, reranking, free-tier cascade, confidence gate stub), and provides a 4-phase implementation plan."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Normative gap analysis and implementation plan for full deep research capability including novelty detection, OpenRouter free-tier routing, LLM-driven CRAG, and cross-encoder reranking."
---

# Vox Deep Research Capabilities — Full Audit & 2026 Roadmap

**Date:** 2026-06-17  
**Scope:** Search stack, research pipeline, novelty detection, OpenRouter free tier, CRAG / SCIENTIA integration — current state vs. 2026 SOTA, with gap analysis and phased implementation plan.

---

## 1. Executive Summary

Vox has a **substantially complete deep-research scaffold** today. The pipeline exists end-to-end: query decomposition → multi-source retrieval (local hybrid + web dispatcher) → CRAG multi-hop loop → claim extraction → LLM-as-judge quality scoring → SCIENTIA finding-candidate promotion. The critical delta between Vox's current state and **production-grade 2026 deep research** is a set of well-scoped gaps in:

1. **LLM-driven reflection** — CRAG query expansion is heuristic regex-only, not LLM-generated
2. **Novelty / deduplication scoring** — no per-hit information-gain gating; URL dedup is basic
3. **Cross-encoder reranking** — the hybrid fusion uses RRF but there is no dedicated reranking model
4. **Confidence gate** — `score_with_config` is a Phase 0a citation-count stub; multi-signal fusion not yet wired
5. **Free-tier OpenRouter model routing** — `openrouter/free` virtual model exists but research cascade defaults to `openrouter/auto` (paid-tier)
6. **Synthesis context budget** — max_tokens for Synthesis = 1,800 is too small for long-form deep research reports
7. **Async / durable research jobs** — session tracking exists but durable execution is not production-complete

Filling these gaps in order of leverage will complete a **production-grade, cost-aware deep research loop** running on the existing Vox infrastructure.

---

## 2. State-of-the-Art Benchmark (2026)

### 2.1 Agentic Deep Research Loop (PRAR Cycle)

2026 SOTA deep research agents implement a **Perceive → Reason → Act → Reflect (PRAR)** iterative loop:

| Phase | What it does |
|-------|-------------|
| **Plan** | Decompose research goal into 3–12 precise sub-queries using an LLM with JSON schema output |
| **Retrieve** | Hybrid retrieval: dense ANN vector search + sparse BM25, fused via RRF (k=60); live web search via policy-gated backends |
| **Rerank** | Cross-encoder pass on top-200 candidates → keep top-20 for synthesis |
| **Reflect** | LLM evaluates: "Is this evidence sufficient? What is missing? What contradicts?" |
| **Re-plan** | If gaps found, generate novel follow-up queries and loop; CRAG stop condition: quality ≥ target |
| **Verify** | NLI-class model checks each extracted claim against retrieved evidence (Support / Contradict / Contested / Unverified) |
| **Synthesize** | Cited answer with evidence spans |
| **Judge** | LLM-as-judge quality score (factual accuracy + citation density + coverage) |
| **Promote** | If quality ≥ threshold, promote to SCIENTIA finding candidate |

### 2.2 2026 Retrieval Infrastructure Best Practices

| Component | 2026 Best Practice | Vox Current State |
|-----------|-------------------|-------------------|
| Sparse index | BM25 (in-process or Tantivy) | ✅ BM25 in-process (`memory_hybrid.rs`) |
| Dense index | Embedding vectors (MiniLM / nomic-embed) | ✅ Optional Qdrant ANN (`vector_qdrant.rs`) |
| Fusion | Reciprocal Rank Fusion (RRF k=60) | ✅ `rrf.rs` — `prefer_rrf_merge` flag |
| Reranking | Cross-encoder on top-200 candidates | ❌ Not implemented |
| Web search tier | SearXNG → DDG → Tavily → scrape | ✅ `web_dispatcher.rs` — all 4 tiers |
| Deep research API | Tavily `/research` endpoint | ✅ `tavily_research.rs` (gated by `VOX_TAVILY_RESEARCH`) |
| Multi-hop | Quality-gated CRAG expansion | ✅ `crag.rs` + `research.rs` |
| Novelty filtering | Per-hit information-gain scoring | ❌ Only URL-level dedup |
| Semantic caching | Intent-based result caching | ⚠️ `pipeline_cache.rs` exists (TTL cache only) |
| Observability | Per-hop span tracing | ⚠️ Telemetry events exist; no per-hop span |

### 2.3 OpenRouter Free Tier (June 2026)

OpenRouter provides 20+ permanently-free models via the `:free` suffix or the `openrouter/free` router:

| Model slug | Context | Strengths | RPD limit |
|-----------|---------|-----------|-----------|
| `google/gemma-3-27b-it:free` | 131K | Instruction following | ~200 |
| `deepseek/deepseek-r1:free` | 164K | Reasoning | ~50 |
| `meta-llama/llama-3.3-70b-instruct:free` | 131K | General | ~200 |
| `microsoft/phi-4-reasoning:free` | 16K | Chain-of-thought | ~200 |
| `openrouter/free` (router) | varies | Auto-selects | aggregated |

Key API facts: OpenAI-compatible; `base_url = https://openrouter.ai/api/v1`; free-tier users get ~50–200 RPD; $10 credit purchase unlocks ~1,000 RPD. Reasoning models support `reasoning` parameter.

---

## 3. Vox Current Architecture — Full Audit

### 3.1 Layer Map

```
┌─────────────────────────────────────────────────────────────────┐
│  GUI / CLI / MCP surfaces                                       │
│  (vox research run, vox_research_run MCP tool)                 │
├─────────────────────────────────────────────────────────────────┤
│  vox-research-shim  (orchestrator/pipeline.rs)                  │
│  ├── planner.rs  ← LLM decompose (cascade: local → OpenRouter) │
│  ├── web_gather.rs ← local hybrid + web via vox-search          │
│  ├── claims.rs ← claim extraction (LLM cascade)                 │
│  ├── verifier.rs ← NLI-style verdict (LLM cascade)             │
│  ├── stages.rs ← judge_quality(), synthesize_answer()          │
│  ├── gate.rs ← confidence score → routing tier                  │
│  ├── discovery_bridge.rs ← SCIENTIA finding candidate promotion │
│  └── pipeline_cache.rs ← TTL-based short-circuit cache         │
├─────────────────────────────────────────────────────────────────┤
│  vox-search  (shared by MCP, orchestrator, CLI, A2A)           │
│  ├── execution.rs ← SearchExecution (all 8 corpora)            │
│  ├── memory_hybrid.rs ← BM25 + optional vector fusion          │
│  ├── rrf.rs ← Reciprocal Rank Fusion                           │
│  ├── crag.rs ← CragRouter (query expansion, stop condition)    │
│  ├── research.rs ← run_multi_hop_web_research()                │
│  ├── web_dispatcher.rs ← SearXNG → DDG → Tavily → scrape      │
│  ├── tavily_research.rs ← /research deep-research endpoint     │
│  ├── bundle.rs ← run_search_with_verification()                │
│  ├── policy.rs ← SearchPolicy (all tunables)                   │
│  └── evaluation.rs ← SearchEvalReport                          │
├─────────────────────────────────────────────────────────────────┤
│  vox-actor-runtime  (LLM facade)                               │
│  ├── llm/cascade.rs ← cascade_for_research_stage()            │
│  │   → local Ollama/Populi → OpenRouter (via key)             │
│  └── model_resolution.rs ← RouteResolutionInput               │
├─────────────────────────────────────────────────────────────────┤
│  vox-research-shim/selection/  (model selection)               │
│  ├── free_tier.rs ← FreeTierRouter                             │
│  ├── virtual_models.rs ← openrouter/auto, openrouter/free      │
│  └── scorer.rs ← ModelScorer                                   │
├─────────────────────────────────────────────────────────────────┤
│  vox-scientia  (knowledge platform)                            │
│  ├── claim_extractor/ ← claim parsing from synthesized text    │
│  ├── critic_gate/ ← quality gate for publications             │
│  ├── review_flow.rs ← human-gated discovery review            │
│  └── evidence_assist.rs ← LLM evidence/conclusion suggestions  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Research Pipeline Stages (production state)

| Stage | Status | Code Location |
|-------|--------|---------------|
| Session creation | ✅ Shipped | `pipeline.rs:100-111` |
| Query decomposition (LLM) | ✅ Shipped | `planner.rs:decompose_query_with_config` |
| Local hybrid retrieval | ✅ Shipped | `web_gather.rs:gather_local_hits_for_plan` |
| Web retrieval (SearXNG/DDG/Tavily) | ✅ Shipped | `web_gather.rs:gather_web_hits_for_plan` |
| URL-level deduplication | ✅ Shipped | `pipeline.rs:dedupe_hits_by_url` |
| Retrieval diagnostics | ✅ Shipped | `pipeline.rs:217-259` |
| Claim extraction | ✅ Shipped | `claims.rs:extract_claims_with_model` |
| Confidence gate (routing tier) | ⚠️ Phase 0a stub | `gate.rs:score_with_config` — citation count only |
| Claim verification (NLI) | ✅ Shipped | `verifier.rs:verify_claims_with_config` — only on DeepResearch tier |
| Answer synthesis | ✅ Shipped | `stages.rs:synthesize_answer_with_llm` |
| LLM-as-judge quality scoring | ✅ Shipped | `stages.rs:judge_quality` |
| Self-verification (CoVE) | ✅ Shipped | `stages.rs:run_self_verification` |
| Citation auditing | ✅ Shipped | `pipeline.rs:citation_audit` |
| SCIENTIA promotion | ✅ Shipped | `discovery_bridge.rs` |
| Report persistence | ✅ Shipped | `pipeline.rs:persist_enabled` |
| MENS training pair generation | ✅ Shipped | `pipeline.rs:training_pair_min_confidence` |

### 3.3 Web Search Dispatcher Stack

```
WebSearchDispatcher::search(query, policy)
  ├── Tier 1: [Reserved/internal]
  ├── Tier 2: SearXNG (policy.searxng_url)  ← free, self-hosted
  ├── Tier 3: DuckDuckGo (policy.duckduckgo_fallback_enabled)  ← free
  └── Tier 4: Tavily search (policy.tavily_enabled, VOX_TAVILY_API_KEY)  ← paid

  Post-retrieval:
  ├── rank_and_dedupe_results()  ← source_authority_score (gov/edu/arxiv/github boost)
  ├── tavily_extract::uplift_low_quality_snippets()  ← content enrichment
  └── scraper::fetch_and_extract()  ← optional full-page extraction (web-scrape feature)

Multi-hop:
  run_multi_hop_web_research()  ← research.rs
    ├── hop loop: CragRouter::expand_queries_from_partial_evidence()
    ├── quality: web_research_crag_quality(top_score, unique_source_count)
    └── stop: CragRouter::should_continue(current_quality, target, hops_remaining)
```

### 3.4 CRAG Implementation (Current State)

`CragRouter::expand_queries_from_partial_evidence()` implements three expansion strategies:

1. **Concept extraction** — regex `([A-Z][a-z]{3,}...)` on top-5 hit snippets → `{original} {concept}` queries
2. **Contradiction follow-up** — if `hit.potential_contradiction == true` → `{original} conflicting evidence source comparison`
3. **Weak evidence** — if avg_score < 0.55 → `{original} primary source evidence` + `{original} independent corroborating sources`
4. **Fallback** — if no concepts found → `latest developments` + `detailed comparison`

**Critical gap:** query expansion is **heuristic regex**, not LLM-generated. This means the expansion strategy is brittle for domains without capitalized proper nouns, misses technical jargon, and cannot infer what knowledge is actually missing from a research perspective.

### 3.5 Novelty Detection (Current State)

**What exists:**
- URL-level deduplication: `HashSet<String>` of visited URLs in `run_multi_hop_web_research` and `dedupe_hits_by_url` in the pipeline
- Source authority scoring: `.gov`, `.edu`, `arxiv.org`, `docs.rs`, `github.com` get score boosts
- Contradiction flag: `HybridSearchHit.potential_contradiction` — set by downstream heuristics
- Citation diversity: `evaluate_citation_diversity` — counts distinct registrable domains

**What is missing (the "novelty detection" gap):**
- **Per-hit information gain scoring** — no mechanism to compute how much new semantic content a candidate hit adds to the accumulated evidence set
- **Semantic deduplication** — hits with the same semantic content but different URLs are not deduplicated
- **Evidence coverage map** — no tracking of which aspects of the query have been covered vs. remain open
- **Bidirectional write-back novelty check** — new synthesis outputs are not checked for novelty before promotion to SCIENTIA

### 3.6 OpenRouter Free Tier Integration (Current State)

The infrastructure is wired:
- `virtual_models.rs` defines `openrouter/free` and `openrouter/auto` as `ModelSpec` entries
- `openrouter/free` has `is_free: true`, `rate_limit_rpm: Some(20)`, `rate_limit_rpd: Some(50)`
- `cascade_for_research_stage()` in `cascade.rs` checks `openrouter_api_key().is_some()` and adds OpenRouter to the cascade
- `FreeTierRouter` exists in `selection/free_tier.rs`

**Gap:** The research cascade defaults to `input.openrouter_model` (which resolves to whatever model is configured, typically a paid model). The `openrouter/free` router is not automatically used as the fallback when the primary model is unavailable or when the user has no credits. There is no automatic free-tier escalation path for the research pipeline specifically.

---

## 4. Gap Analysis — Ranked by Leverage

| # | Gap | Impact | Effort | Leverage |
|---|-----|--------|--------|----------|
| **G1** | LLM-driven CRAG query expansion | 🔴 High — heuristic regex misses domain nuances | Medium | **Critical** |
| **G2** | Cross-encoder reranking stage | 🔴 High — 8-15% precision lift in RAG benchmarks | Medium | **Critical** |
| **G3** | Novelty/information-gain scoring | 🟠 High — redundant evidence bloats context | Medium | High |
| **G4** | Free-tier cascade fallback | 🟠 High — users without API credits get no research | Low | High |
| **G5** | Confidence gate multi-signal fusion | 🟠 High — routing tier stub reduces DeepResearch activation | Medium | High |
| **G6** | Synthesis token budget | 🟡 Medium — 1,800 max_tokens truncates long-form reports | Low | Medium |
| **G7** | Semantic result caching | 🟡 Medium — redundant search cost on similar queries | Medium | Medium |
| **G8** | Durable async research jobs | 🟡 Medium — long research runs block synchronously | High | Medium |
| **G9** | Per-hop span observability | 🟡 Low — debugging multi-hop loops is difficult | Low | Low |

---

## 5. Implementation Plan — Full Deep Research Capability

### Phase 1 (Quick Wins — ~2 weeks)

#### P1.1 — Free-Tier Research Cascade Fallback

**Target:** `crates/vox-research-shim/src/selection/free_tier.rs` + `cascade.rs`

Add an explicit free-tier cascade path that activates when:
- `VOX_OPENROUTER_FREE_TIER=1` is set, OR
- The standard OpenRouter key resolves to a budget-constrained profile

The cascade order for free-tier research:

```
1. local Ollama/MENS (if available)
2. openrouter/free (openrouter/free router → DeepSeek R1:free, Llama-3.3-70B:free, Gemma-3-27B:free)
3. Fallback: SearXNG-only synthesis (no LLM, structured bullet output)
```

Specific model IDs to seed in the `free_tier.rs` catalog for research:
- `deepseek/deepseek-r1:free` — 164K context, excellent for reasoning-heavy research
- `google/gemma-3-27b-it:free` — strong instruction following
- `meta-llama/llama-3.3-70b-instruct:free` — balanced general research

**Config additions to `SearchPolicy`:**

```rust
/// Force research cascade onto free-tier models (env: VOX_RESEARCH_FREE_TIER).
pub research_free_tier_only: bool,
/// Preferred free-tier model IDs in priority order.
pub research_free_tier_model_ids: Vec<String>,
```

#### P1.2 — Synthesis Token Budget Uplift

**Target:** `crates/vox-actor-runtime/src/llm/cascade.rs`

Change `ResearchStage::Synthesis` max_tokens from `1_800` to `4_000` (configurable via `ResearchConfig::synthesis_max_tokens`, which already exists as a field — the issue is the `apply_stage_defaults` override in cascade.rs that always sets it to 1_800).

Remove the hard override in `apply_stage_defaults` for Synthesis — let `ResearchConfig::synthesis_max_tokens` win.

### Phase 2 (Core Quality — ~4 weeks)

#### P2.1 — LLM-Driven CRAG Query Expansion

**Target:** `crates/vox-search/src/crag.rs` + `research.rs`

Replace `CragRouter::expand_queries_from_partial_evidence()` with an LLM-assisted version:

```rust
pub async fn expand_queries_llm(
    original_query: &str,
    hits: &[HybridSearchHit],
    coverage_so_far: &str,
    policy: &SearchPolicy,
) -> Vec<String>
```

LLM prompt (Synthesis-tier cascade, temperature=0.2):

```
You are a research gap analyst. Given the original research question and the evidence 
collected so far, identify 2–4 specific follow-up queries that would cover the most 
important missing aspects.

Original question: {original_query}
Evidence collected: {top_5_snippets}

Output only valid JSON: {"followup_queries": ["...", "..."]}
Prioritize: factual gaps, underrepresented sources, contradictions to resolve.
```

Fallback: if LLM cascade fails, use the existing heuristic regex expansion.

#### P2.2 — Novelty / Information-Gain Scoring

**Target:** `crates/vox-search/src/` (new file: `novelty.rs`)

Implement a semantic novelty scorer for retrieved hits:

```rust
pub struct NoveltyScorer {
    /// Accumulated evidence fingerprints (set of normalized token n-grams).
    seen_ngrams: HashSet<u64>,
}

impl NoveltyScorer {
    /// Score 0.0–1.0: fraction of this hit's n-gram content not seen before.
    pub fn score_novelty(&mut self, content: &str) -> f64 { ... }
    
    /// Accept a hit (add its n-grams to the seen set).
    pub fn accept(&mut self, content: &str) { ... }
}
```

Algorithm: 4-gram shingling over tokens, FNV1a hashes, novelty = `|new_hashes| / |total_hashes|`. Hits below policy threshold (e.g., `novelty_min_score: 0.15`) are filtered from the synthesis context but retained in diagnostics.

**Integration points:**
1. `run_multi_hop_web_research()` — filter hop hits by novelty before accumulating
2. `gather_web_hits_for_plan()` — cross-subquery deduplication
3. `synthesize_answer_with_llm()` — sort synthesis context by novelty DESC before truncating
4. SCIENTIA write-back — check novelty of new finding candidates vs. existing corpus

**`SearchPolicy` additions:**

```rust
pub novelty_scoring_enabled: bool,
pub novelty_min_score: f64,  // default: 0.15
```

#### P2.3 — Confidence Gate Multi-Signal Fusion

**Target:** `crates/vox-research-shim/src/research/gate.rs`

Replace the Phase 0a citation-count stub with a multi-signal fusion function:

```rust
let citation_score = (citation_count / min_citations_for_full_score).clamp(0.0, 1.0);
let claim_support_score = supported_claims / total_claims;  // 0.5 if no claims
let diversity_score = (distinct_domains / min_domains).clamp(0.0, 1.0);
let retrieval_score = if no_retrieval_hits { 0.0 } else { 1.0 };

let score = citation_score * 0.35
    + claim_support_score * 0.30
    + diversity_score * 0.20
    + retrieval_score * 0.15;
```

This ensures the `DeepResearch` routing tier activates on weak multi-signal evidence, not just few citations.

### Phase 3 (Advanced Quality — ~6 weeks)

#### P3.1 — Cross-Encoder Reranking

**Target:** `crates/vox-search/src/reranker.rs` (new file)

Add an optional reranking stage using a local cross-encoder model via `vox-plugin-mens-candle-*`:

```rust
pub async fn rerank_hits(
    query: &str,
    hits: &mut Vec<HybridSearchHit>,
    policy: &SearchPolicy,
) { ... }
```

Model candidates:
- `cross-encoder/ms-marco-MiniLM-L-6-v2` (fast, 6-layer, CPU-viable)
- `BAAI/bge-reranker-v2-m3` (better quality, multilingual)

Wire as a post-retrieval step in `execute_search_plan()` when `policy.reranking_enabled = true`.

**`SearchPolicy` additions:**

```rust
pub reranking_enabled: bool,
pub reranking_model_id: String,  // default: "cross-encoder/ms-marco-MiniLM-L-6-v2"
pub reranking_top_k: usize,      // default: 20
pub reranking_candidate_k: usize, // default: 200
```

#### P3.2 — Deep Research Skill

Create `crates/vox-plugin-skill-deep-research/deep-research.skill.md`:

```yaml
name = "skill-deep-research"
description = "Full autonomous deep research: decomposes a topic into sub-queries, 
retrieves from web and local corpora using hybrid search, iterates with CRAG gap-filling, 
verifies claims against evidence, synthesizes a cited report, and scores it with LLM-as-judge."
[metadata]
"vox-id" = "vox.deep-research"
"vox-category" = "research"
"vox-tools" = ["vox_research_run", "vox_research_status", "vox_research_export"]
```

#### P3.3 — Semantic Result Caching

Upgrade `pipeline_cache.rs` from key-hash TTL cache to intent-based semantic cache:
- Store result embeddings alongside TTL entries
- On cache lookup: cosine-similarity check against stored embeddings (threshold: 0.92)
- Reduces redundant LLM costs for similar research queries by 60-70%

### Phase 4 (Production Hardening — ongoing)

- **P4.1** — Durable async research jobs via `vox-orchestrator-queue`
- **P4.2** — Per-hop span tracing (`research.hop.{n}.{metric}`)
- **P4.3** — Citation diversity enforcement gate (currently computed but not enforced)

---

## 6. OpenRouter Free Tier — Zero-Cost Deep Research Strategy

### 6.1 Recommended Free Research Configuration

```bash
# .env
VOX_OPENROUTER_API_KEY=<your-key>
VOX_RESEARCH_FREE_TIER=1
VOX_RESEARCH_FREE_TIER_MODELS=deepseek/deepseek-r1:free,google/gemma-3-27b-it:free,meta-llama/llama-3.3-70b-instruct:free
VOX_SEARXNG_URL=http://localhost:8080   # self-hosted SearXNG (free)
VOX_SEARCH_DDG_FALLBACK=1
```

### 6.2 Free Tier Pipeline Topology

```
Query
  ↓ Planner (deepseek/deepseek-r1:free, 164K ctx, reasoning)
  ↓ 3-6 subqueries
  ↓ Retrieval (SearXNG:free → DuckDuckGo:free → Tavily:optional)
  ↓ CRAG multi-hop (LLM-driven, gemma-3-27b-it:free)
  ↓ Claim extraction (llama-3.3-70b:free)
  ↓ Verification (deepseek-r1:free, temp=0.0)
  ↓ Synthesis (llama-3.3-70b:free, 4K tokens)
  ↓ Judge (gemma-3-27b-it:free, temp=0.0)
  ↓ SCIENTIA promotion (if quality ≥ 50)
```

**Rate limit math:** A single research run makes ~7–13 LLM calls. At 200 RPD, this supports 15–28 full research runs per day on free tier alone. DeepSeek R1:free has a tighter 50 RPD limit — use for high-priority stages (planner, verifier) only.

### 6.3 Cost-Optimized Paid Configuration

| Stage | Model | Rationale |
|-------|-------|-----------|
| Planner | `deepseek/deepseek-v4-flash` | Cheapest reasoning with JSON output |
| Claim extraction | `deepseek/deepseek-v4-flash` | Speed + cost |
| Verification | `deepseek/deepseek-r1` | CoT reasoning for NLI |
| Synthesis | `anthropic/claude-sonnet-4.6` (with caching) | Best long-form; 80%+ cost cut with prompt caching |
| Judge | `deepseek/deepseek-v4-flash` | Fast structured scoring |

---

## 7. Novelty Detection — Detailed Design

### 7.1 Why Novelty Matters

Without novelty filtering, the synthesis context is padded with redundant content (e.g., 8 blog posts paraphrasing the same paper). This wastes synthesis context window, dilutes attention signal on new evidence, and inflates citation count without increasing actual coverage.

### 7.2 N-Gram Shingling Approach

```rust
fn shingling_hashes(content: &str, n: usize) -> Vec<u64> {
    let chars: Vec<char> = content.chars().collect();
    chars.windows(n)
        .map(|window| fnv1a_hash(&window.iter().collect::<String>()))
        .collect()
}

// novelty_score = |new_hashes| / |total_hashes|
```

**Threshold guidance:**
- `novelty_min_score = 0.15` → accept hits where ≥15% of content is new (recommended default)
- `0.05` for deep technical queries where partial overlap is acceptable
- `0.30` for broad surveys to enforce source diversity

### 7.3 Bidirectional Write-Back (SCIENTIA Integration)

When a new research result is about to be promoted to `scientia_finding_candidates`, check its novelty against the existing corpus using the same shingling approach. Only promote if novelty ≥ 0.25 vs. the last 10 promoted findings in the same domain.

---

## 8. Existing Capability — What Works Right Now

The following deep research capabilities are **production-ready today** with no code changes:

1. **`vox research run`** CLI command — dispatches `run_research_with_context_and_session()`
2. **`vox_research_run` MCP tool** — orchestrator-MCP bridge
3. **Multi-hop CRAG loop** — `run_multi_hop_web_research()` with quality-gated stop condition
4. **Hybrid BM25 + vector search** — `MemorySearchEngine` with temporal decay and status boosting
5. **RRF fusion** — cross-corpus Reciprocal Rank Fusion with configurable k
6. **SearXNG + DDG + Tavily web search** — policy-gated, authority-boosted deduplication
7. **Tavily `/research` deep endpoint** — synthesis-quality results with answer + sources
8. **Full-page HTML extraction** — `web-scrape` feature flag, text density filtering
9. **LLM query decomposition** — 3–12 sub-queries with JSON schema output
10. **Claim extraction + NLI verification** — LLM cascade with evidence spans
11. **LLM-as-judge quality scoring** — 0–100 rubric (factual accuracy + citation density + coverage)
12. **CoVE self-verification** — consistency check before synthesis
13. **Citation auditing** — claim-to-source support mapping
14. **SCIENTIA finding promotion** — quality ≥ 50 + supported claims → `scientia_finding_candidates`
15. **MENS training pair generation** — high-confidence results → training data
16. **Session tracking** — `research_sessions` DB table with staged status updates
17. **Progress callbacks** — streaming progress to GUI / MCP

---

## 9. Prior Art Comparison — Consolidated Matrix

| Feature | Gemini Deep Research | OpenClaw | Vox Current | Vox After Roadmap |
|---------|---------------------|----------|-------------|-------------------|
| Query decomposition | ✅ LLM | ✅ LLM | ✅ LLM (cascade) | ✅ Same |
| Multi-hop retrieval | ✅ Iterative | ✅ ~5 rounds | ✅ Policy-gated hops | ✅ Same |
| CRAG-style gap-fill | ✅ Internal | ✅ Heuristic | ⚠️ Heuristic regex | ✅ **LLM-driven (P2.1)** |
| Hybrid search (BM25+vec) | Internal | Varies | ✅ Shipped | ✅ + Reranking (P3.1) |
| Web search | ✅ Google | Plugin | ✅ SearXNG/DDG/Tavily | ✅ Same |
| Novelty filtering | ✅ Internal | Partial | ❌ URL dedup only | ✅ **N-gram scoring (P2.2)** |
| Cross-encoder reranking | ✅ Internal | Partial | ❌ | ✅ **P3.1** |
| Claim verification (NLI) | Internal | Partial | ✅ LLM cascade | ✅ Same |
| LLM-as-judge | ✅ Internal | Partial | ✅ Shipped | ✅ Same |
| Free tier | ❌ (Gemini caps) | N/A | ⚠️ Infra exists, not wired | ✅ **P1.1** |
| Local/private corpus | N/A | N/A | ✅ Full hybrid | ✅ Same |
| SCIENTIA promotion | N/A | N/A | ✅ Unique to Vox | ✅ Enhanced (P2.3) |
| Async durable jobs | ✅ Gemini async | Varies | ⚠️ Session tracked, not queued | ✅ **P4.1** |
| Semantic caching | ✅ Internal | N/A | ⚠️ TTL only | ✅ **P3.3** |

---

## 10. Priority Recommendations

### Immediate (this sprint)

1. **P1.1** — Wire `openrouter/free` cascade fallback. Single config flag `VOX_RESEARCH_FREE_TIER=1`. Zero breaking changes.
2. **P1.2** — Fix synthesis max_tokens: remove hard override in `cascade.rs:apply_stage_defaults` for `Synthesis`, let `ResearchConfig::synthesis_max_tokens` win.

### Next sprint

3. **P2.1** — LLM-driven CRAG expansion in `crag.rs` with heuristic fallback. Highest leverage for quality.
4. **P2.2** — `novelty.rs` n-gram shingling. Wire into `run_multi_hop_web_research` and synthesis context ranking.

### Following sprint

5. **P2.3** — Replace gate.rs stub with multi-signal fusion.
6. **P3.1** — Cross-encoder reranking using local MENS/Candle backend.

### Medium term

7. **P3.2** — Deep research skill YAML.
8. **P3.3** — Semantic cache upgrade.
9. **P4.1** — Async research job queue.
10. **P4.2** — Per-hop span tracing.

---

## 11. Architectural Invariants to Preserve

Per `AGENTS.md` policy:

- All LLM calls **MUST** go through `vox_actor_runtime::llm` facade
- New free-tier model IDs must be added as `ModelSpec` entries in the registry, scored on the same axes (latency, cost, quality, locality)
- CRAG expansion LLM calls **MUST** use `cascade_with_optional_manual` / `cascade_for_research_stage` pattern
- Novelty scorer state **MUST NOT** be persisted across research runs (per-session working memory only)
- All new `SearchPolicy` fields **MUST** be mirrored with `VOX_SEARCH_*` environment variable overrides

---

## 12. Source References

| Component | File |
|-----------|------|
| Full search stack | `crates/vox-search/src/` |
| Research pipeline | `crates/vox-research-shim/src/research/orchestrator/` |
| LLM cascade | `crates/vox-actor-runtime/src/llm/cascade.rs` |
| Model selection | `crates/vox-research-shim/src/selection/` |
| SCIENTIA integration | `crates/vox-scientia/src/` |
| Prior art matrix | `docs/src/architecture/deep-research-prior-art-and-vox-roadmap-2026.md` |
| SCIENTIA handoff | `docs/src/architecture/scientia-automated-research-handoff-2026-06-16.md` |
| Search retrieval SSOT | `docs/src/architecture/search-retrieval-ssot-2026.md` |
