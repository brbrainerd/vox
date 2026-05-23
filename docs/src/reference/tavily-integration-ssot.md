---
title: "Tavily Integration SSOT"
description: "Complete reference for integrating Tavily AI search API into the Vox RAG pipeline. Covers endpoints, Rust SDK, Secrets management, CRAG flow, safety posture, and cost model."
category: "Language Reference"
status: "current"
last_updated: "2026-04-10"
see_also:
  - architecture/rag-and-research-architecture-2026.md
  - secrets-ssot.md

schema_type: "TechArticle"
---

# Tavily Integration SSOT

Tavily is the **live web retrieval leg** of the Vox RAG pipeline. It provides real-time, AI-native, LLM-ready search results as a complement to Vox's static local corpora (Memory, KnowledgeGraph, DocumentChunks, etc.).

> [!IMPORTANT]
> All Tavily secrets MUST be registered through `vox-secrets`. Never read `TAVILY_API_KEY` directly with `std::env::var`.

---

## API Endpoint Reference

### `/search` — Real-Time Web Search

**Credits:** 1 (basic) / 2 (advanced)

**Key parameters:**
| Parameter | Type | Default | Notes |
|---|---|---|---|
| `query` | string | required | The search query |
| `search_depth` | `"basic"│"advanced"` | `"basic"` | Advanced = deeper results, 2× cost |
| `topic` | `"general"│"news"│"finance"` | `"general"` | Domain hint |
| `include_answer` | bool | false | Returns a synthesized answer string |
| `max_results` | int | 5 | Max 10 (basic) or more (advanced) |
| `time_range` | `"day"│"week"│"month"│"year"` | null | Freshness filter |
| `include_domains` | string[] | [] | Whitelist specific domains |
| `exclude_domains` | string[] | [] | Blacklist specific domains |

**Response shape:**
```json
{
  "query": "string",
  "answer": "string|null",
  "results": [
    { "title": "...", "url": "...", "content": "clean text", "score": 0.97, "published_date": "..." }
  ],
  "response_time": 1.23
}
```

---

### `/extract` — URL Content Extraction

**Credits:** 1 per 5 URLs (basic) / 2 per 5 URLs (advanced)

**Key parameters:**
| Parameter | Type | Notes |
|---|---|---|
| `urls` | string[] | Up to 20 URLs per call |
| `query` | string | Optional — enables query-focused reranking/chunking |
| `format` | `"markdown"│"text"` | Output format |
| `include_images` | bool | Default false |
| `extract_depth` | `"basic"│"advanced"` | Advanced handles JavaScript-rendered pages |

**Typical use:**
```text
Tavily /search → ranked URLs → Tavily /extract → clean markdown → embed → vector store
```

---

### `/research` — Autonomous Deep Research

**Credits:** Variable (internally fires multiple search calls)

**Purpose:** "Agent-in-a-Box" — performs iterative multi-step research autonomously and returns a comprehensive, synthesized JSON report. GA'd early 2026.

**Key parameters:**
| Parameter | Type | Notes |
|---|---|---|
| `query` | string | Full research topic |
| `instructions` | string | Optional guidance (e.g., "focus on Rust, ignore Python") |

**When to use:** For Vox's intensive research mode (user requests "research X thoroughly"). Replaces a full multi-iteration search loop with a single API call.

---

### `/crawl` — Site-Level Discovery

**Credits:** Map + Extract credits (combined)

**Purpose:** Crawl a specific site with natural-language instructions (e.g., documentation ingestion).

**Key parameters:**
| Parameter | Notes |
|---|---|
| `url` | Root URL to crawl |
| `instructions` | Natural language crawl guidance |
| `max_depth` | Default 3 |
| `max_pages` | Cap on pages visited |

**Vox use case:** Periodically crawl documentation sites into the `DocumentChunks` corpus.

---

## Rust SDK

**Crate:** `tavily = "2.1.0"` ([crates.io](https://crates.io/crates/tavily))
**Source:** https://github.com/PierreLouisLetoquart/tavily-rs
**Backend:** `tokio` + `reqwest`

> [!WARNING]
> This is a community-maintained crate, not an official Tavily SDK. Pin to a specific version and test on upgrade.

**Configuration in `vox-search/Cargo.toml`:**
```toml
[dependencies]
tavily = { version = "2.1.0", optional = true }

[features]
tavily-search = ["dep:tavily"]
```

**Safe usage pattern (via Clavis):**
```rust
// Never do this:
let key = std::env::var("TAVILY_API_KEY").unwrap();

// Always do this:
use vox_secrets::{SecretId, resolve_secret};
let key = resolve_secret(SecretId::TavilyApiKey)
    .map_err(|e| format!("tavily_key_missing:{e}"))?;
```

---

## Secrets Lifecycle

### Required Entries in `crates/vox-secrets/src/lib.rs`

```rust
SecretId::TavilyApiKey => SecretSpec {
    env_var: "TAVILY_API_KEY",
    description: "Tavily web search API key. Get at https://tavily.com. Free tier: 1,000 credits/mo.",
    required: false,
    deprecated_aliases: &["X_TAVILY_API_KEY"],
},
SecretId::TavilyProject => SecretSpec {
    env_var: "TAVILY_PROJECT",
    description: "Optional Tavily project ID for X-Project-ID header usage tracking.",
    required: false,
    deprecated_aliases: &[],
},
```

### Lifecycle Checklist

After adding the secret entries:
1. Run `vox ci secret-env-guard`
2. Run `vox ci clavis-parity`
3. Update `vox secrets doctor` profile expectations
4. Update this doc at `docs/src/reference/secrets-ssot.md`

---

## Environment Variable Summary

| Variable | Purpose | Default |
|---|---|---|
| `TAVILY_API_KEY` | API authentication | (none — Tavily disabled) |
| `TAVILY_PROJECT` | X-Project-ID header | (none) |
| `VOX_SEARCH_TAVILY_ENABLED` | Master switch | `false` |
| `VOX_SEARCH_TAVILY_DEPTH` | API search depth | `"basic"` |
| `VOX_SEARCH_TAVILY_MAX_RESULTS` | Results per query | `5` |
| `VOX_SEARCH_TAVILY_ON_EMPTY` | Fire when all local corpora empty | `true` |
| `VOX_SEARCH_TAVILY_ON_WEAK` | CRAG mode — fire when evidence_quality < threshold | `false` |
| `VOX_SEARCH_TAVILY_BUDGET` | Max credits per session | `50` |

---

## Pricing (April 2026)

| Plan | Credits/Month | Price | Notes |
|---|---|---|---|
| Researcher (Free) | 1,000 | $0 | No card required. Good for dev. |
| Project | 4,000 | ~$30/mo | $0.0075/credit |
| Bootstrap | 15,000 | ~$100/mo | $0.0067/credit |
| Startup | 38,000 | ~$220/mo | $0.0058/credit |
| Growth | 100,000 | ~$500/mo | $0.005/credit |
| Pay-As-You-Go | — | $0.008/credit | |

**Credit costs:**
- `/search` basic: 1 credit
- `/search` advanced: 2 credits
- `/extract` basic: 1 credit/5 URLs
- `/extract` advanced: 2 credits/5 URLs
- `/research`: variable (multiple internal searches)

**Session budget guard:** `VOX_SEARCH_TAVILY_BUDGET=50` limits the session to 50 credits (50 basic searches or 25 advanced searches) to prevent runaway costs.

---

## Operational Safety Rules

1. **Fail-open always.** Any Tavily error (network down, auth failure, rate limit, budget exceeded) MUST log to `SearchExecution::warnings` and allow the search to complete with local-only results. Never abort or panic.

2. **Content size limits.** Truncate each Tavily result's `content` field to `policy.tavily_max_content_chars` (default 2,000) before injecting into any prompt or document chunk. Prevents context explosion.

3. **Credit budget tracking.** Maintain a session-level atomic counter. When `counter >= tavily_credit_budget_per_session`, log a warning and disable Tavily for the remainder of the session.

4. **PII scrubbing.** Never send user-identifying information (names, emails, account IDs) in Tavily queries. Strip PII from the query before the API call.

5. **Prompt injection protection.** Tavily's built-in firewall scrubs content at the API level, but Vox should additionally treat Tavily content as untrusted user input — escape or truncate before LLM injection.

6. **A2A forwarding.** When including Tavily results in an `A2ARetrievalResponse` destined for another agent, use durable artifact references (URI + short-lived auth token) rather than inline text. This prevents cross-agent prompt injection per the A2A evidence-sharing research (see `research-agent-handoff-a2a-evidence-sharing-2026.md`).

---

## Tavily vs Firecrawl Decision Matrix

| Use Case | Tool | Reason |
|---|---|---|
| Real-time query answer grounding | **Tavily** | Search-first, ranked snippets, built-in safety |
| Full documentation site ingestion | **Firecrawl** | Full-page extraction, JS handling, structured schema |
| Multi-source research synthesis | **Tavily /research** | Autonomous multi-step, single API call |
| Knowledge base construction from URLs | **Tavily /extract** or Firecrawl | Depends on JS complexity |
| Fresh news/events context | **Tavily** | `topic="news"`, `time_range="day"` |

**Recommended phasing:**
- **Phase 1 (now):** Tavily only — covers search, extract, and research use cases with a single vendor and Rust SDK
- **Phase 2 (later):** Add Firecrawl HTTP client for specialized deep extraction into `vox-corpus` pipelines

---

## Integration Test Checklist

Before enabling Tavily in CI:
- [ ] `vox secrets doctor` reports `TAVILY_API_KEY: resolved`
- [ ] `vox search "test query" --tavily` returns results from Tavily backend
- [ ] `SearchExecution::tavily_lines` is non-empty in output
- [ ] Credit counter increments per call
- [ ] Budget cap stops further calls at limit
- [ ] Network failure → warnings only, local results returned normally
- [ ] `A2ARetrievalResponse.tavily_excerpts` populated when Tavily fires
