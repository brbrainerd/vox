---
title: "scientia external discovery research 2026"
description: "Automatically added frontmatter for scientia external discovery research 2026"
category: "Architecture SSOTs"
status: "research"
training_eligible: false
archived_date: 2026-04-18
---
# Vox Scientia External Discovery & Monitoring Architecture — 2026 Research Synthesis

> **Status:** Architecture Research Findings | Created: 2026-04-10
> **Purpose:** Document architectural requirements for extending Vox Scientia from a publication-outbound pipeline into a news-inbound, external discovery, and RAG-integrated autonomous monitoring system.

**See also:** [SCIENTIA multi-platform ranking, discovery, and anti-slop SSOT (research 2026)](scientia-multi-platform-ranking-discovery-research-2026.md) — tiered survey of distribution surfaces, ingest vs syndicate posture, and projection profiles for outbound copy.

---

## 1. Executive Summary & The Core Problem

Currently, `vox-scientia` handles the *outbound* lifecycle: turning internal discoveries (from the Populi/MENS mesh) into publication-ready artifacts (arXiv, JMLR, Zenodo) via `vox-publisher`.

To "make discoveries externally," Scientia must develop an **inbound monitoring and synthesis layer**. This involves building an autonomous AI news monitoring agent that ingests high-signal external intelligence (AI industry news, newly published research, framework updates), evaluates it via `vox-socrates-policy` to reject "slop," and synthesizes it into a reliable knowledge feed inside `vox-search`.

## 2. Ingestion & Perception Engine Research

### 2.1 RSS & Atom Feeds
For high-signal, structured sources (e.g., arXiv category feeds, major AI labs' blogs), the system will use Rust feed parsers.
- **Decision:** Use `feed-rs` crate (mature, `serde` support, HTML sanitization) for standard feeds. Use `feedparser-rs` ("Bozo" mode) exclusively for historically flaky XML sources.

### 2.2 Social API Ingestion (Reddit/Hacker News)
The current `vox-publisher/src/adapters/reddit.rs` uses OAuth configured via `VoxAuthConfig` for outward sumissions. 
- **Inbound Path:** The existing OAuth refresh token flow (`refresh_access_token`) can be symmetrically inverted to hit read-only endpoints (e.g., `api/v1/new`). 
- **Scope:** Configure read-only tracking of subreddits like `r/MachineLearning` and `r/LocalLLaMA` with strict rate-limit adherence.

### 2.3 Orchestrated External Retrieval
For deep extraction, `vox-search` will integrate Tavily `/extract` or Firecrawl to pull full methodology papers when an RSS feed or social post only provides an abstract.

## 3. Noise Filtering & Worthiness Evaluation

The internet is primarily noise. We must extend existing structural gates to filter inbound streams.

### 3.1 Redesigning Preflight for Inbound (`vox-publisher`)
Currently, `publication_preflight.rs` uses `PreflightProfile` (`DoubleBlind`, `MetadataComplete`, `ArxivAssist`) to validate outgoing manifests.
- **Action:** Introduce a `NewsInbound` profile that validates incoming text against a heuristic checklist (e.g., requires code repository links and reproducible benchmarks, rejecting pure opinion pieces or wrapper-library marketing).

### 3.2 Extending Socrates Inbound Policies
`vox-socrates-policy` provides a mathematically sound Triad (`Answer`, `Ask`, `Abstain`) based on `abstain_threshold` and `max_contradiction_ratio_for_answer`.
- **Action:** For inbound feeds, apply `ComplexityJudge` and `RiskBand` scoring to evaluate claims. If an article exhibits a high contradiction ratio compared to established MENS baselines, it is placed in `Quarantine` for human review rather than automatic ingestion.

## 4. Storage & RAG Deduplication

External intelligence must not pollute the primary MENS vectors with redundant reporting.

### 4.1 Hybrid Memory Integration (`memory_hybrid.rs`)
`vox-search/src/memory_hybrid.rs` currently implements BM25 and Vector search, merging hits via `fuse_hybrid_results`. It annotates contradictions by checking title and term overlap.
- **Execution:** Before inserting a new external discovery, query the existing `embeddings` table. If a match exceeds `similarity > 0.9` (semantic duplicate), intercept the write. Instead of adding a new `IndexedDocument`, append the new source URL to the existing document's `provenance` metadata.

### 4.2 Database Schema
Define new Arca SQL tables in `vox-db` under `publish_cloud` named `scientia_external_intelligence` to track processed URLs and avoid infinite polling loops.

## 5. Output Synthesis & "Scholarly Digest"

Instead of raw feeds, Scientia builds a unified **Scholarly Digest**.

### 5.1 Multi-Agent Workflow
1. **Collector Agent:** Fetches `feed-rs` items and subreddit posts.
2. **Evaluator Agent:** Applies Socrates and `NewsInbound` preflight.
3. **Synthesizer Agent:** Clusters related developments and generates a unified summary highlighting the *delta* and *impact*.

### 5.2 Inference Cost Modeling
Running daily digests over hundreds of external articles requires cost awareness.
- **Routing:** Use `Tier 1` (Local Llama-3-8B) for initial categorization and basic summarization since it is cost-free locally. Route only `ComplexityBand::Complex` or `MultiHop` queries to `Tier 2` (API) models to avoid budget exhaustion.

training_eligible: false
archived_date: 2026-04-18
---
**Conclusion:** The inbound external discovery pipeline requires symmetrical inversions of our existing outbound publication systems. No new fundamental abstractions (like separate Vector databases or orchestration loops) are needed; we will reuse `vox-search`, `Socrates`, and `Arca`.

