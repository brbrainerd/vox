---
title: "Vox RAG and Autonomous Research Architecture 2026"
description: "Single source of truth for the Vox retrieval-augmented generation pipeline, Socrates hallucination gate, Tavily web search integration, CRAG loop, and agent-to-agent research handoff."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-10"
see_also:
  - research-trust-reliability-signals-2026.md
  - research-agent-handoff-a2a-evidence-sharing-2026.md
  - prompt-engineering-document-skills-scientia-research-2026.md

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Vox RAG and Autonomous Research Architecture (2026)

## 1. Overview

Vox uses a multi-layer RAG (Retrieval Augmented Generation) architecture to ground agent responses in verified evidence and minimize hallucination. This document is the SSOT for the entire retrieval pipeline, from query intake to evidence delivery.

The pipeline has three zones:
1. **Pre-Retrieval** — query normalization, complexity classification, optional HyDE expansion
2. **Retrieval** — multi-corpus hybrid search (local + optional Tavily web)
3. **Post-Retrieval** — RRF fusion, verification pass, Socrates gate, CRAG correction

---

## 2. Retrieval Architecture — Current Production State

### 2.1 Corpus Map

All corpora are searched in parallel per query. Results are RRF-merged.

| Corpus | Backend | Feature Gate | Source Crate |
|---|---|---|---|
| `Memory` | BM25 (in-process) + SQLite vector | Always | `vox-search/memory_hybrid.rs` |
| `KnowledgeGraph` | SQLite FTS5 node queries (Lexical graph traversal, NOT semantic) | Always | `vox-search/execution.rs` |
| `DocumentChunks` | Hybrid FTS5 + vector embeddings | Always | `vox-search/execution.rs` |
| `RepoInventory` | Token-overlap WalkDir path scan | Always | `vox-search/execution.rs` |
| `TantivyDocs` | On-disk Tantivy index | `tantivy-lexical` feature | `vox-search/lexical_tantivy.rs` |
| `Qdrant` | HTTP ANN sidecar | `qdrant-vector` feature + `VOX_SEARCH_QDRANT_URL` | `vox-search/vector_qdrant.rs` |
| **`SearXNGWeb`** | **Federated web search via SearXNG** | **`vox research up` + sidecar** | **`vox-search/searxng.rs` [NEW]** |
| **`DuckDuckGoWeb`** | **Zero-config web fallback** | **Always (DDG JSON API)** | **`vox-search/duckduckgo.rs` [NEW]** |
| `TavilyWeb` | Live web search via Tavily API | `tavily-search` feature + `VOX_SEARCH_TAVILY_ENABLED=1` | `vox-search/tavily.rs` |

### 2.2 Search Plan Heuristic

`heuristic_search_plan(query, is_verification, hint)` in `vox-db` determines:
- `SearchIntent` — Lookup / Research / Codex / Verification
- `RetrievalMode` — FullText / Vector / Hybrid
- `corpora` set — which corpora to activate
- `allow_verification_pass` — whether a second pass is permitted

### 2.3 Retrieval Quality Signals

After execution, `SearchExecution` carries:

| Signal | Type | Meaning |
|---|---|---|
| `evidence_quality` | `f64 [0,1]` | Weighted: `top_score × 0.7 + citation_coverage × 0.3` |
| `citation_coverage` | `f64 [0,1]` | Fraction of non-empty corpora / 6 (or 7 with Tavily) |
| `source_diversity` | `usize` | Count of non-empty corpora |
| `contradiction_count` | `usize` | Heuristic heading-overlap contradictions detected |
| `recommended_next_action` | `SearchRefinementAction` | BroadenScope / FocusCodex / FocusRepo / RetryHybrid / AskUser |

### 2.4 RRF Fusion

When `VOX_SEARCH_PREFER_RRF=1`, results from all active corpora are merged via Reciprocal Rank Fusion (k=60 constant). This is the industry-standard algorithm for merging heterogeneous ranked lists without score normalization.

training_eligible: false
archived_date: 2026-04-18
---

## 3. CRAG Loop (Corrective RAG)

The CRAG loop fires a live Tavily web search as a corrective action when local evidence is insufficient.

```
Initial search pass
    │
    ├── [evidence_quality < 0.55 AND tavily_fire_on_weak=true]
    │       → TavilyClient::search(query)
    │       → append to execution.tavily_lines
    │       → re-run RRF including Tavily leg
    │       → diagnostics.notes += "crag_triggered=true"
    │
    ├── [all corpora empty AND tavily_fire_on_empty=true]
    │       → TavilyClient::search(query)
    │       → same merge flow
    │
    └── [contradiction_count > 0 AND tavily_enabled]
            → TavilyClient::search(best_effort_verification_query)
            → external evidence used for contradiction resolution
```

**Key policy variables** (all in `SearchPolicy::from_env()`):
- `VOX_SEARCH_TAVILY_ENABLED` — master switch
- `VOX_SEARCH_TAVILY_ON_EMPTY` — default `true`
- `VOX_SEARCH_TAVILY_ON_WEAK` — default `false` (CRAG mode)
- `VOX_SEARCH_TAVILY_BUDGET` — session credit cap (default `50`)

---

## 4. Socrates Policy — Hallucination Gate

The Socrates system (`vox-socrates-policy`) provides numeric policy for confidence, abstention, and research escalation.

### 4.1 Risk Decision Flow

```
confidence: f64, contradiction_ratio: f64
    → classify_risk() → RiskBand { High, Medium, Low }
    → evaluate_risk_decision() → RiskDecision { Answer, Ask, Abstain }
    → [Abstain + complexity ≥ Complex] → evaluate_research_need() → SocratesResearchDecision [PLANNED]
```

### 4.2 Default Thresholds

| Threshold | Value |
|---|---|
| `abstain_threshold` | 0.35 |
| `ask_for_help_threshold` | 0.55 |
| `max_contradiction_ratio_for_answer` | 0.40 |
| `min_persist_confidence` | 0.60 |
| `min_training_pair_confidence` | 0.75 |

### 4.3 Coverage Paradox Fix [PLANNED]

**Problem:** The contradiction gate fires on abstract synthesis due to lexical divergence (NLI false positives). This causes agents to enter a refusal loop ("Coverage Paradox").

**Fix:** Only apply `max_contradiction_ratio_for_answer` when `citation_coverage >= 0.3`. When coverage is below 0.3, classify as "insufficient evidence" (→ Ask or trigger research) rather than "contradiction" (→ Abstain).

### 4.4 Research Dispatch [PLANNED]

`SocratesResearchDecision` is a new struct returned by `evaluate_research_need()`:
```rust
struct SocratesResearchDecision {
    should_research: bool,
    trigger: Option<ResearchTrigger>,  // LocalWeakEvidence | ContradictionDetected | ComplexityEscalation
    suggested_query: Option<String>,
    suggested_corpus: Vec<String>,     // e.g. ["TavilyWeb", "DocumentChunks"]
}
```

This wires Socrates decisions directly into CRAG dispatch. The orchestrator checks this decision before generating a response.

training_eligible: false
archived_date: 2026-04-18
---

## 5. Tavily Web Search Integration

See `docs/src/reference/tavily-integration-ssot.md` for full API reference.

### 5.1 Architecture Position

Tavily is the **dynamic retrieval leg** — the live web complement to Vox's static local corpora.

```
Static corpora (local)          Dynamic corpus (live web)
├── Memory (BM25 + vector)      └── Tavily /search
├── KnowledgeGraph (FTS5)           ├── Basic: 1 credit/query
├── DocumentChunks (hybrid)         ├── Advanced: 2 credits/query
├── RepoInventory (path scan)       └── Research: autonomous multi-step
├── TantivyDocs (on-disk)      
└── Qdrant (ANN sidecar)       
         ↓                                   ↓
         ├─────── RRF Fusion ────────────────┤
                       ↓
              SearchExecution → MCP/A2A
```

### 5.2 Safety Posture

- Always fail-open (Tavily errors → warnings, never abort)
- Content truncated to max `tavily_max_content_chars` chars/result before prompt injection
- Credits tracked per-session against `tavily_credit_budget_per_session`
- Tavily's built-in prompt-injection firewall active on all endpoints
- For A2A forwarding: use durable artifact references, not inline embedding

### 5.3 Clavis Secret Registration

```
SecretId::TavilyApiKey  ← TAVILY_API_KEY
SecretId::TavilyProject ← TAVILY_PROJECT (optional, X-Project-ID header)
```

Run `vox clavis doctor` to verify secret availability.

---

## 6. Agent-to-Agent Evidence Sharing

See `docs/src/architecture/research-agent-handoff-a2a-evidence-sharing-2026.md` for inline vs. artifact reference analysis.

### 6.1 Wire Format

`A2ARetrievalRequest` → sent from requester to retrieval agent.
`A2ARetrievalResponse` → evidence package returned (includes `tavily_excerpts` [PLANNED]).
`A2ARetrievalRefinement` → follow-up if contradiction or weak recall.

### 6.2 Multi-Agent Research Dispatch (Planned)

For `ComplexityBand::MultiHop` queries:
1. Decompose into N sub-queries
2. Dispatch N parallel `A2ARetrievalRequest` messages
3. Each agent fires its local + Tavily retrieval
4. RRF-merge all N `A2ARetrievalResponse` result sets
5. Synthesizer agent produces unified evidence package
6. Socrates gate runs on unified package

training_eligible: false
archived_date: 2026-04-18
---

## 7. Query Pre-Processing [PLANNED — Wave 4]

### 7.1 Strategy Taxonomy

| Strategy | When | Cost |
|---|---|---|
| `Direct` | Always (default) | None |
| `Normalize` | Always (existing) | None |
| `HyDE` | `ComplexityBand::Complex` or vector top_score < 0.3 | 1× LLM call |
| `Decompose` | `ComplexityBand::MultiHop` | In-process (heuristic) |

### 7.2 HyDE (Hypothetical Document Embeddings)

For abstract or ambiguous queries:
1. Call local inference server (`vox-schola`) to generate a hypothetical answer
2. Embed the hypothetical answer (statement-form) instead of the question
3. Use that embedding for vector recall

**Tradeoff:** ~25-60ms extra latency. Only activate when evidence quality justifies it.

**Activation:** `VOX_SEARCH_QUERY_PREPROCESS=hyde` AND `VOX_POPULI_ENDPOINT` configured.

---

## 8. Evaluation and Monitoring

| Metric | Current | Planned |
|---|---|---|
| Backend latency P99 | Not tracked | `vox telemetry search-quality-report` |
| Evidence quality distribution | In diagnostics | Persist to Arca for trend analysis |
| Tavily credit usage | Not tracked | Per-session counter, `vox clavis doctor` |
| Hallucination events | Not persisted | Socrates Abstain → Arca event table |
| Recall@K golden set | Not built | Should be built from real user queries |
| RAGAS faithfulness | Not implemented | Periodic spot-check on completions |

training_eligible: false
archived_date: 2026-04-18
---

## 9. Related Codebase References

| Component | Path |
|---|---|
| Search execution | `crates/vox-search/src/execution.rs` |
| Hybrid memory search | `crates/vox-search/src/memory_hybrid.rs` |
| RRF fusion | `crates/vox-search/src/rrf.rs` |
| SearXNG client | `crates/vox-search/src/searxng.rs` |
| DuckDuckGo client | `crates/vox-search/src/duckduckgo.rs` |
| Local Scraper | `crates/vox-search/src/scraper.rs` |
| Web Dispatcher | `crates/vox-search/src/web_dispatcher.rs` |
| Verification bundle | `crates/vox-search/src/bundle.rs` |
| A2A contracts | `crates/vox-search/src/a2a_contract.rs` |
| Search policy | `crates/vox-search/src/policy.rs` |
| Socrates policy | `crates/vox-socrates-policy/src/lib.rs` |
| Complexity judge | `crates/vox-socrates-policy/src/complexity.rs` |
| Embedding service | `crates/vox-search/src/embeddings.rs` |
| Qdrant sidecar | `crates/vox-search/src/vector_qdrant.rs` |
| Tantivy lexical | `crates/vox-search/src/lexical_tantivy.rs` |
| Clavis secrets | `crates/vox-clavis/src/lib.rs` |

