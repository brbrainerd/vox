---
title: "Local Autonomous Research Findings 2026"
description: "Analysis of SearXNG, native Rust scraping, and local alternatives to commercial search APIs."
category: "Architecture SSOTs"
status: "research"

last_updated: "2026-04-12"
training_eligible: false
archived_date: 2026-04-18
---

# Local Autonomous Research Findings (2026)

## 1. Tavily Capability Decomposition

Tavily provides four distinct high-value outputs that we must replicate to achieve parity:
1. **Federated Search**: Aggregating results from multiple search engines.
2. **Content Extraction**: Turning raw HTML into clean, structured Markdown.
3. **Relevance Scoring**: Filtering noise and ranking content by agent-readiness.
4. **Injection Safety**: Protecting against prompt injection within web content.

## 2. SearXNG Integration

SearXNG serves as the primary federated search engine. It aggregates results from 70+ engines.

### 2.1 Configuration
- **Endpoint**: `GET /search?q={query}&format=json`.
- **Latency**: 500ms - 2000ms.
- **Privacy**: Zero data leaves the local infrastructure.
- **Dependency**: Requires Docker for optimal deployment (`vox research up`).

## 3. Native Rust Scraping Stack (`vox-scraper`)

To move beyond snippets and provide Tavily-grade content, we implement a native extraction pipeline.

| Layer | Implementation | Purpose |
|---|---|---|
| HTTP Client | `reqwest` | Asynchronous fetching with User-Agent policy. |
| DOM Parsing | `scraper` | Pruning `nav`, `footer`, `script`, and boilerplate. |
| MD Conversion | `html2text` | Formatting the pruned tree for LLM ingestion. |
| Filtering | Readability | Scoring by text density (target ≥ 0.15). |

## 4. Zero-Config Fallback: DuckDuckGo

For environments without Docker or where SearXNG is not deployed, the system utilizes the DuckDuckGo JSON API.
- **URL**: `https://api.duckduckgo.com/?q={query}&format=json`.
- **Benefit**: No authentication required, high reliability, zero latency overhead for deployment.

## 5. Performance Tiering

- **Tier 1 (Internal)**: FTS5 + Vector (50ms).
- **Tier 2 (SearXNG)**: Self-hosted federated search (500-1500ms).
- **Tier 3 (DDG)**: Public JSON API (800-2000ms).
- **Tier 4 (Tavily)**: Commercial fallback (300-800ms).

## 6. Implementation References
- `crates/vox-search/src/searxng.rs`
- `crates/vox-search/src/scraper.rs`
- `crates/vox-search/src/web_dispatcher.rs`

