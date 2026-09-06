---
title: "Search & Retrieval SSOT (2026)"
description: "Single baseline for how agent-facing retrieval works across vox-db contracts, vox-search execution, orchestrator MCP tools, and dashboard transport — including the two distinct \"VoxDB\" surfaces."
category: "Architecture SSOTs"
sort_order: 52
status: "current"
training_eligible: true
training_rationale: "Prevents drift between FTS, embeddings, policy knobs, and GUI wiring."
schema_type: "TechArticle"
audience: ["contributors", "agents"]
related:
  - docs/src/architecture/data-storage-ssot-2026.md
  - crates/vox-db/src/retrieval.rs
  - crates/vox-search/src/execution.rs
  - crates/vox-search/src/policy.rs
---

# Search & Retrieval SSOT (2026)

## 1. Scope

This document is the **baseline for agent-facing search and retrieval**: planning, execution, storage contracts, policy, MCP tools, and dashboard integration.

It does **not** replace app-author documentation for generated Convex-style schemas (`voxdb/server`); see Section 4.

## 2. Canonical pipeline

| Layer | Crate / surface | Responsibility |
|-------|-----------------|----------------|
| Contracts & fusion | `vox-db` (`retrieval.rs`, `store/ops_memory.rs`) | `SearchPlan`, `RetrievalResult`, `fuse_hybrid_results`, heuristic planner `heuristic_search_plan`, FTS / embeddings SQL |
| Execution | `vox-search` | `execute_search_plan`, `run_search_with_verification`, corpora routing, optional Tantivy / Qdrant / web |
| Policy | `vox-search` (`policy.rs`) | `SearchPolicy`: fusion weights, budgets, backend toggles — resolved via **vox-secrets** (`resolve_secret`), not raw `std::env::var` in consumers |
| Entry points | `vox-cli`, `vox-orchestrator` | CLI research paths; MCP tools `vox_memory_search`, `vox_knowledge_query`; chat preamble uses the same retrieval bundle as `vox_memory_search` |

**Rule:** CLI, orchestrator, MCP, and chat preamble must **not** fork retrieval logic; they call into `vox-search` with a `SearchRuntimeContext` and `SearchPolicy`.

## 3. Two distinct “VoxDB” surfaces (do not conflate)

### 3.1 Rust `vox-db` (Turso / libSQL)

Used by **`vox-search`** for durable corpora:

- `search_documents` / `search_document_chunks` (+ FTS5 shadow table when available)
- `embeddings` (vector blobs; brute-force similarity for modest tables)
- `knowledge_nodes` (graph / FTS)

Hybrid chunk retrieval merges lexical hits (`chunk_id` = `search_document_chunks.id` as string) with vector hits (`embeddings.source_id` for `source_type = "search_document_chunk"`) via **`fuse_hybrid_results`**. **Stable `chunk_id` alignment** between ingest, embedding storage, and hybrid query is mandatory — regressions produce two parallel ranked lists with no overlap.

### 3.2 Generated `voxdb/server` (TypeScript schema)

The compiler emits **`defineSchema` / `searchIndex(...)`** from `@search_index` on `@table` declarations (see `crates/vox-codegen-ts/src/schema/from_hir.rs`). That targets the **app author's** Convex-compatible runtime and is **not** invoked by `vox-search`.

## Language surface (decision)

- **Declarative only** for app data: `@search_index`, `@vector_index`, `@index` on `@table`; emitted `searchIndex(...)` / `vectorIndex(...)` in `voxdb/server`.
- **No global `search()` Vox builtin** in `eval`/stdlib — agent retrieval stays on the orchestrator path (`vox_memory_search`, chat preamble, CLI).
- Optional future work: an `@retrieval` decorator that lowers to planner overrides — deferred until a concrete consumer exists.

## 4. Corpus → backend matrix (authoritative per corpus)

| Corpus | Primary backend | Notes |
|--------|-----------------|-------|
| Agent markdown memory | In-process BM25 + optional embeddings + `fuse_hybrid_results` | Policy weight: `memory_vector_fusion_weight` |
| Knowledge graph | SQLite FTS / LIKE on `knowledge_nodes` | Exposed as MCP `vox_knowledge_query` |
| Ingested doc chunks (RAG) | FTS5 / LIKE + `embeddings` hybrid | Fusion weight: `chunk_vector_fusion_weight`, passed into `query_search_document_chunks_hybrid` (not hard-coded in `vox-db`) |
| Repo inventory | Bounded filesystem walk + token overlap | Interim until persistent code indexes exist |
| Symbol proximity | `vox-search::symbol_proximity` (`scan_symbol_proximity`) | Contracts-aware lexical proximity vs optional query vectors; planner corpus `SymbolProximity` |
| Semantic FS (intent paths) | `vox-search::semantic_fs` (`discover_files_for_intent`, `retrieve_evidence_for_intent`) | Same inventory ranker as repo paths; MCP exposes `semantic_fs_discover` for AgentOS intent-shaped discovery |
| Docs mirror (optional) | Tantivy (`tantivy-lexical` feature) | Supplemental |
| Sidecar ANN (optional) | Qdrant (`qdrant-vector` feature) | Parallel to DB chunk search |
| Web | SearXNG / DDG / optional Tavily | Policy-gated |
| Graphify search log (virtual) | Turso `graphify_search_hit` nodes | Virtual corpus tracking queries and FNV-64 hashes. Always fresh; bypassed by lexical check |

## 5. Policy knobs (vox-secrets / env)

- **Memory hybrid:** `VOX_SEARCH_MEMORY_VECTOR_WEIGHT` → `SearchPolicy.memory_vector_fusion_weight` (default `0.55`).
- **Chunk hybrid:** `VOX_SEARCH_CHUNK_VECTOR_WEIGHT` → `SearchPolicy.chunk_vector_fusion_weight` (default `0.60`), threaded into `VoxDb::query_search_document_chunks_hybrid` as a scalar so `vox-db` stays free of `SearchPolicy`.
- **BM25 (memory index):** `VOX_SEARCH_BM25_K1`, `VOX_SEARCH_BM25_B` → clamped into `MemorySearchEngine` via `SearchPolicy` (`memory_bm25_k1` / `memory_bm25_b`).
- **RRF fusion:** `VOX_SEARCH_RRF_K` → reciprocal-rank constant for `rrf_merge_line_lists`.
- **Web hit persistence:** `VOX_SEARCH_PERSIST_WEB_HITS_DISABLED` — when unset/false, `execute_search_plan` mirrors SearXNG/DDG/Tavily-style web hits into `search_documents` / chunks (`web-ingest:*` URIs) alongside the Tavily CRAG path in `bundle.rs`.
- **Embedding model name:** `VOX_EMBEDDING_MODEL` (`SecretId::VoxEmbeddingModel`) — resolved in `embedding_env.rs` per provider (HF / OpenAI / OpenRouter defaults remain at the call site when unset).

**Dependency slimming:** consumers that only need `EmbeddingService` (e.g. `vox-scientia-ingest`, `vox-plugin-publication`) should depend on `vox-search` with **`default-features = false`** and enable only `tantivy-lexical` / `web-scrape` as needed — avoids pulling default `qdrant-vector` + `tavily`.

## 6. MCP tools and GUI

| Tool | Purpose |
|------|---------|
| `vox_memory_search` | Full retrieval bundle (memory, knowledge, chunks, repo, optional RRF / web paths per plan) |
| `vox_knowledge_query` | Narrow query against `knowledge_nodes` only |
| `vox_research_run` | Orchestrator `run_research` pipeline: `vox-search` web tier (SearXNG → DDG → Tavily) + optional CRAG hops + synthesis/judge when LLM env is configured — see [`deep-research-prior-art-and-vox-roadmap-2026.md`](deep-research-prior-art-and-vox-roadmap-2026.md) |
| `vox_graphify_search` | Lexically search a graphify corpus graph on disk (read-only) |
| `vox_graphify_query` | BFS neighbor expansion from seed node IDs (read-only; layered navigation) |
| `vox_graphify_path` | Shortest path between two node IDs in a graphify corpus (read-only) |
| `vox_graphify_compare` | Diff two graphify corpus manifests for node/edge/community delta (read-only) |

**Dashboard:** use `POST /v1/tools/call` (`voxTransport.callTool`). No client-side BM25/vector index unless there is an explicit offline product requirement.

## 7. Telemetry

Orchestrator wraps retrieval in **`RetrievalEvidenceEnvelope`** (`crates/vox-orchestrator/src/mcp_tools/memory_tools/retrieval.rs`). New retrieval paths must preserve this envelope.

## 8. Sandbox tiers

`vox-search` pulls native/async/network dependencies. **Interp / WASI tiers must not link retrieval inline**; scripts use host tools / orchestrator bridges.

## 9. Related crates

- `vox-scientia-ingest` depends on `vox-search` (e.g. `EmbeddingService` in `deduplicator.rs`); do **not** remove `vox-search` from that crate.

## 10. Change checklist

- After retrieval / persistence changes: `cargo test -p vox-db`, `cargo test -p vox-search`, and **`vox ci data-storage-guard`** when contracts shift.
- After new Secrets-backed env: `SecretId` + `SecretSpec`, bump [`contracts/config/env-vars.v1.yaml`](../../../contracts/config/env-vars.v1.yaml), run **`vox ci secrets-contracts`**.
- After new architecture pages: set frontmatter so the Starlight sidebar lists them; add a row to [`where-things-live.md`](where-things-live.md) if the page introduces a new concept→crate mapping. Link from root [`AGENTS.md`](../../../AGENTS.md) only when the page is a new always-loaded policy pointer.
