---
title: "Omni-Search Audit & Roadmap (2026)"
description: "Comprehensive audit of Vox search capabilities, surfaces, gaps, bugs, and enhancement roadmap across the full stack from vox-db contracts through vox-search execution to GUI omni-search. Single source of truth for where search is and where it needs to go."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Maps the full search stack; essential for any agent working on retrieval, indexing, or GUI search surfaces."
audience: ["contributors", "agents"]
related:
  - docs/src/architecture/search-retrieval-ssot-2026.md
  - crates/vox-search/src/execution.rs
  - crates/vox-search/src/policy.rs
  - crates/vox-gui/src/commands/search.rs
  - crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx
  - crates/vox-gui/ui/src/lib/searchController.ts
  - crates/vox-gui/ui/src/lib/federatedSearchIndex.ts
  - crates/vox-db-types/src/retrieval.rs
---

# Omni-Search Audit & Roadmap (2026)

> **Status:** Research document. This is a living audit. Its findings gesture toward an implementation plan that should be authored separately. Cross-reference: search-retrieval-ssot-2026.md for canonical pipeline contracts.

---

## 1. Executive Summary

Vox search stack is architecturally sound and significantly more capable than it appears at the GUI surface. The core vox-search crate implements a genuine hybrid multi-corpus planner (BM25 + vector embedding + RRF fusion) with 8 distinct backend legs and 6 SearchCorpus variants. The GUI exposes 7 user-facing scope chips and a federated client-side index for settings/commands.

However, major gaps exist between the backend power and what reaches users and agents:

1. The GUI SearchView does not surface scores, diagnostics, or backend-mix information meaningfully.
2. SearchCorpus::SymbolProximity exists but is never exercised by the GUI heuristic planner path.
3. There is no persistent AST/symbol index -- repo search uses a slow WalkDir token-overlap scan (O(files x tokens)).
4. Chat search is a LIKE-only query bolted onto vox_search_query post-hoc, not a first-class corpus.
5. The Federated OmniSearch index (settings, surfaces, skills, commands, docs, policies, actions) lives entirely client-side and is rebuilt on every palette open with no fuzzy matching.
6. SearchCorpus has no variant for GitHistory, Telemetry, Plugins, Workflows, or AgentLogs.
7. The CommandPalette and SearchView share a useSearchController hook but have separate, slightly inconsistent filter logic.
8. No search telemetry pipeline: zero instrumentation of recall, precision, latency, or backend-mix ratios.
9. The repo_inventory_max_files: 20_000 cap means large repos silently truncate their file set.
10. No incremental indexing: every query re-walks the entire repo tree.

---

## 2. Current Search Architecture Map

### 2.1 Full Stack Layers

The search stack flows from top to bottom:

GUI Layer:
- CommandPalette (Cmd+K / Ctrl+K)
- SearchView (full search, sidebar)
- MemoryView (memory-scoped)

All GUI surfaces share useSearchController (200ms debounce) which calls vox_search_query via Tauri IPC.
Federated surfaces (settings, commands) use useFederatedSearchIndex (client-side only).

Tauri command layer (vox-gui/src/commands/search.rs):
- vox_search_query command
- repo discovery (discover_repository_or_fallback)
- DB connect (optional, graceful degradation)
- heuristic_search_plan -> execute_search_plan
- chat_search_gui_messages (LIKE, post-hoc append)
- glob filter + kind filter + pagination

vox-search execution layer:
- SearchCorpus::Memory         -> MemorySearchEngine (BM25+vector hybrid)
- SearchCorpus::KnowledgeGraph -> VoxDb::query_knowledge_nodes (FTS/LIKE)
- SearchCorpus::DocumentChunks -> VoxDb hybrid (FTS5 + embeddings)
- SearchCorpus::RepoInventory  -> WalkDir token-overlap scan
- SearchCorpus::WebResearch    -> SearXNG -> DDG -> Tavily
- SearchCorpus::SymbolProximity-> scan_symbol_proximity (rarely activated from GUI)
- [optional] Tantivy           -> tantivy-lexical feature gate
- [optional] Qdrant            -> qdrant-vector feature gate
- RRF fusion                   -> rrf_merge_line_lists (disabled by default)

vox-db / vox-db-types layer:
- SearchPlan, SearchCorpus, SearchBackend, heuristic_search_plan
- fuse_hybrid_results (BM25 + embedding score fusion)
- query_search_document_chunks_hybrid
- chat_search_gui_messages (LIKE only)

### 2.2 Corpus x Backend Matrix (Current State)

| Corpus | Backend | GUI Accessible | Indexed | Incremental | Score Type |
|---|---|---|---|---|---|
| Memory | BM25 + optional vector | Yes | In-process | No (rebuild) | Hybrid f64 |
| KnowledgeGraph | FTS/LIKE on knowledge_nodes | Yes | SQLite | No | Always 0.0 (BUG) |
| DocumentChunks | FTS5 + embedding hybrid | Yes | SQLite | Yes (ingest) | Hybrid f64 |
| RepoInventory | WalkDir token-overlap | Yes | No index | No | Token count f32 |
| WebResearch | SearXNG->DDG->Tavily | Yes (scope chip) | N/A | N/A | f64 from engine |
| SymbolProximity | scan_symbol_proximity | Partial (broken routing) | Partial | No | f64 |
| Chats | LIKE on gui_messages | Yes (bolted on) | SQLite | Yes | Hardcoded 0.75 |
| GitHistory | MISSING | No | No | No | -- |
| PluginCatalog | Client-side federatedSearchIndex | Palette only | No | No | Exact/prefix |
| Settings | Client-side federatedSearchIndex | Palette + SearchView | No | No | Exact/prefix |
| Commands | Client-side filterCommandCatalogHits | Yes | No | No | Exact/contains |
| Telemetry | MISSING | No | No | No | -- |
| AgentLogs | MISSING | No | No | No | -- |
| Workflows | MISSING | No | No | No | -- |

### 2.3 GUI Search Surfaces

| Surface | Entry Point | Backend Corpora | Notes |
|---|---|---|---|
| SearchView | Sidebar -> Search | All 7 scope chips | Full-featured; facets; path glob |
| CommandPalette | Cmd+K / Ctrl+K | Same backend + federated | Prefix modes /skills /commands /agents |
| MemoryView | Sidebar -> Memory | Memory-scoped | Direct memory search field |
| Chat preamble | Orchestrator (auto) | Memory + knowledge + chunks | Not visible in GUI |
| MCP vox_memory_search | Agent tools | Full retrieval bundle | Most powerful surface |
| MCP vox_knowledge_query | Agent tools | KnowledgeGraph only | Narrow |
| MCP vox_research_run | Agent tools | Web tier + CRAG | Research pipeline |

---

## 3. Gaps & Bugs -- Inventory

### 3.1 Critical Gaps

**G1: No persistent AST/symbol index**
repo_path_search walks WalkDir for every query, scanning up to 20,000 files.
On a repo with 121+ crates (as in Vox), this is O(50ms-500ms) per query.
No tree-sitter parsing, no symbol extraction, no function/type/struct/trait graph.
Impact: Code navigation queries return path matches, never symbol-level hits.
SearchCorpus::SymbolProximity exists but uses scan_symbol_proximity which is a token proximity heuristic, not a real symbol graph.

**G2: Chat corpus is second-class**
chat_search_gui_messages uses a LIKE %query% scan appended AFTER the main execution.
No vector embedding, no BM25, hardcoded score of 0.75.
Not part of SearchCorpus enum, not plannable, not controlled by heuristic plan.
Chats are a primary user artifact but get the worst search quality.

**G3: KnowledgeGraph score is always 0.0**
execution.rs line 388: score: 0.0 for all knowledge nodes.
These hits appear at the same RRF rank regardless of match quality.
Makes knowledge graph results ungradable against memory/chunk results.

**G4: Client-side federated index has no fuzzy matching**
scoreMatch in federatedSearchIndex.ts is exact/prefix only (100/80/50).
Typo tolerance: zero. "setings" finds nothing in Settings.
No trigram, Levenshtein, or phonetic matching.
Palette is one of the most-used surfaces but has the crudest matching.

**G5: SymbolProximity quality limited; excluded from default heuristic plan**
userScopeToBackend('code') returns ['repo', 'symbol'] (searchController.ts line 33-34).
scope_to_corpus('symbol') maps to SearchCorpus::SymbolProximity -- so explicitly scoped code searches DO invoke it.
However: (a) the default unscoped heuristic plan typically omits SymbolProximity, meaning users who do not select scope chips never see it; and (b) scan_symbol_proximity is a token-proximity heuristic (not a real symbol graph), so result quality is limited regardless of routing.
Result: SymbolProximity is reachable via explicit code scope, but underutilized and low-quality in practice.

**G6: No incremental repo indexing**
Every GUI search query restarts the full WalkDir scan.
No inotify/FSEvents-based file watcher to maintain a persistent index.
No content hash cache to skip unchanged files.

### 3.2 Important Gaps

**G7: RRF fusion disabled by default**
policy.prefer_rrf_merge defaults to false.
Without RRF, results are returned as separate per-corpus lists.
Users see "memory", then "chunk", then "repo" -- not interleaved by relevance.
The GUI's groupBySource groups by source, which hides cross-corpus ranking.

**G8: Verification pass not wired in GUI path for scoped searches**
run_search_with_verification is called from vox_search_query only when scope_corpora is None.
When user selects specific scope chips, execute_search_plan is called directly, skipping verification.
Weak evidence is never detected in scoped searches.

**G9: No search telemetry**
recordGamifyGuiEvent('search_query_executed') fires but captures only query text.
No latency tracking, no backend-mix logging, no click-through rate, no zero-results rate.
Cannot improve ranking without usage signal.

**G10: repo_inventory_max_files: 20_000 silent truncation**
Repos exceeding 20,000 files silently drop files.
Vox itself has 150+ crates -- this limit is precarious.
No warning emitted to the UI when truncation occurs.

**G11: Knowledge node scores unusable for RRF**
KnowledgeGraph hits enter RRF with score: 0.0.
RRF is rank-based not score-based, so partially mitigated -- but unified_hits sort still deprioritizes them.

**G12: glob_match on frontend is naive and broken**
pathMatchesGlob in SearchView.tsx normalizes ** incorrectly:
  - strips **/ prefix, replaces ** with '', then * with ''
  - "**/*.rs" reduces to ".rs" (the extension only, with no wildcard)
  - !normalized is false (string is non-empty), so the path.includes('.rs') test runs
  - Result: "**/*.rs" matches any path containing ".rs" anywhere, including paths like "foo.rss" or "bars/baz.rs" -- it acts as a substring match on the extension, not a glob
The correct behavior: "**/*.rs" should match only files ending in .rs at any depth.
The backend glob_match in search.rs is correct (recursive DP); the frontend version is broken and should be replaced with a proper glob-to-regex conversion or deferred to the backend.

**G13: chats scope causes superset results instead of chats-only (scope routing bug)**
userScopeToBackend('chats') returns ['chats'].
But scope_to_corpus('chats') returns None -- there is no SearchCorpus::Chats.
When user selects only ['chats'], scope_corpora maps to [] (empty, filtered to None), so the else branch runs run_search_with_verification with the full heuristic plan.
Meanwhile, wants_chats is true (scope_tags contains 'chats'), so chat results also append.
Result: user asked for chats-only, but receives chat results PLUS all-corpora results -- a strict superset. Chat messages are buried among unrelated memory/chunk/repo hits.
This is more severe than "gets nothing": the user cannot isolate chat history at all.

**G14: Memory cache rebuild race**
cached_memory_engine caches the engine but if ctx.db is None, the engine is built from filesystem every call.
Under concurrent GUI searches (debounced at 200ms), multiple rebuild calls can race.

**G15: Web scraper disabled by default in GUI build**
The web-scrape feature gate is not enabled in vox-gui Cargo.toml dependency on vox-search.
Web search results returned to GUI are engine snippets only (no full-page extraction).
This makes web scope hits much lower quality than MCP/orchestrator-routed web searches.

### 3.3 Minor Issues

**G16: Empty search query returns error instead of graceful empty state**
The Tauri command rejects empty strings; error message exposed in transport error logs.

**G17: locator_for maps chunk to file but chunk IDs are not real file paths**
Chunk IDs from search_document_chunks are database row identifiers, not filesystem paths.
Opening a "chunk" locator tries to spawn an editor with the chunk ID as a file path -- fails silently.

**G18: Score normalization inconsistent across corpora**
Memory BM25: unbounded float (TF-IDF based, can exceed 1.0).
Chunk hybrid: normalized 0.0-1.0 via fusion.
Repo token-overlap: count of matched tokens (integer-like f32).
Web: engine-dependent (Tavily 0-1, SearXNG varies).
Knowledge: always 0.0.
Commands/Settings: hardcoded 0.85.
scoreToPct in GUI divides by 1.0 then formats as percent -- correct for normalized scores but garbage for BM25 values >1.0.

**G19: No aria-live region for search result count updates**
Accessibility: screen readers do not announce when result count changes.

**G20: CommandPalette does not share facet state with SearchView**
Selecting a scope in palette and pressing "View all results" should pre-populate SearchView with same scope -- does not.

---

## 4. What Is Not Searched (Omni-Search Gaps)

The following content exists in the Vox codebase/runtime but is entirely unsearchable today:

| Content Type | Location | Blocked By |
|---|---|---|
| Git commit history | .git/ | No SearchCorpus::GitHistory |
| Git blame / authorship | git | No indexer |
| LSP diagnostic history | orchestrator logs | No corpus |
| Telemetry events (vox.script.*) | vox-telemetry | No corpus |
| Agent conversation logs | .gemini/antigravity/brain/ | Not indexed |
| Workflow execution history | vox-journal | No corpus |
| Plugin manifest metadata | vox-plugin-catalog/catalog.toml | Client-side only |
| Skill YAML frontmatter | assets/skills/, .vox/skills/ | Client-side only |
| ADR documents | docs/src/adr/ | Not in memory corpus by default |
| Architecture docs | docs/src/architecture/ | Not in memory corpus by default |
| Scientia findings | docs/src/architecture/ | Not ingested |
| CHANGELOG / release notes | CHANGELOG.md | Not indexed |
| .env.example env var docs | .env.example | Not indexed |
| Cargo.toml feature flags | Cargo.toml per crate | No structured indexer |
| Contract YAML files | contracts/ | Not indexed |
| MCP tool definitions | vox-orchestrator-mcp | Not indexed |

---

## 5. Best Practices & 2026 Techniques

### 5.1 Indexing Architecture

The WalkDir-per-query approach is fundamentally unscalable. The 2026 standard:

- Tree-sitter-based symbol extraction at ingest time, building a symbol graph (function defs, struct defs, trait impls, macro uses). Vox has tree-sitter-vox already.
- Tantivy + content-hash cache for full-text: re-index only changed files by watching the filesystem (inotify on Linux, FSEvents on macOS, ReadDirectoryChangesW on Windows).
- Embedding vectors per symbol chunk (not per whole file) stored in SQLite (existing embeddings table).
- HNSW or flat ANN for small-to-medium corpora; Qdrant optional sidecar for production scale.

Key Rust tools:
- tantivy (already used) -- mature Rust FTS, fast incremental indexing
- tree-sitter (already tree-sitter-vox) -- structural parsing for symbol graph
- notify crate (inotify/FSEvents wrapper) -- for filesystem watch events
- Content-addressable chunk store: hash file contents, only re-embed changed chunks

### 5.2 Hybrid Retrieval

RRF should be on by default in 2026. Research consistently shows hybrid BM25+vector+RRF outperforms any single leg by 10-30% NDCG. The prefer_rrf_merge flag should flip its default.

Score normalization: All corpus scores should be L2-normalized into [0, 1] before RRF and before display. Knowledge graph nodes need an actual match score (LIKE rank or BM25).

Query rewriting: The best_effort_verification_query is stopword-based. Query expansion with synonym sets or LLM query rewriting substantially improves recall on short queries.

### 5.3 Omni-Search UI Patterns (2026)

Linear, Notion, and Raycast have converged on:
- Sub-50ms first result from local indexes (federated client-side index or pre-built local SQLite)
- Progressive loading: show local/cached results instantly, then stream in backend results
- Intent-based routing: parse @memory, @code, #tag, >command prefixes to route to specific corpus
- Keyboard-centric: full arrow/enter/escape with no mouse required
- Result preview panel: hover or right-side panel showing full content before clicking
- Fuzzy matching with Levenshtein: tolerance for 1-2 typos in short queries
- Recent/pinned results: surfacing frequently-accessed items even without a query
- Search as navigation: pressing Enter opens the file in the editor, not just copies path

### 5.4 Agent-Facing Search (2026)

For AI agents working in a codebase, the most useful search capabilities are:
- Symbol definition lookup: "where is SearchPolicy defined?" -> immediate answer
- Call graph traversal: "what calls execute_search_plan?" -> callers list
- Semantic code search: "find functions that do hybrid fusion" -> embedding-based
- Change-aware search: "what changed in this crate recently?" -> git-blame-aware
- Dependency graph: "what crates depend on vox-search?" -> Cargo.lock parse
- Diagnostic search: "find all files with recent compiler warnings" -> telemetry-backed

The MCP surface (vox_memory_search, vox_knowledge_query) is strong but missing symbol-level and call-graph queries.

### 5.5 Performance Benchmarks (2026 Targets)

| Query Type | Current (estimated) | Target |
|---|---|---|
| Memory BM25 (300 docs) | ~10ms | <5ms |
| Repo path walk (20k files) | 50-500ms | <20ms (cached index) |
| Chunk hybrid (FTS5 + embedding) | 20-100ms | <30ms |
| Web search (SearXNG) | 500-2000ms | 500ms (unchanged) |
| Federated palette (client) | <5ms | <5ms |
| Full omni-search (all corpora) | ~200-800ms | <100ms local, progressive |

---

## 6. Improvements Inventory (Prioritized)

### Tier 1 -- High Impact, Moderate Effort (Fix Now)

| ID | Improvement | Est. Effort |
|---|---|---|
| I-01 | Fix chats scope mapping bug (G13) -- chats scope fans out to all corpora | S |
| I-02 | Fix glob_match frontend broken for ** patterns (G12) | S |
| I-03 | Return real KnowledgeGraph scores from query_knowledge_nodes (G3) | M |
| I-04 | Enable prefer_rrf_merge by default (G7) | XS |
| I-05 | Add fuzzy matching (trigram/Levenshtein) to federatedSearchIndex (G4) | M |
| I-06 | Fix locator_for chunk->file mapping -- chunk IDs are not paths (G17) | M |
| I-07 | Add aria-live to search result count (G19) | XS |
| I-08 | Wire SymbolProximity into GUI code scope reliably (G5) | M |
| I-09 | Emit warning when repo files truncated at max (G10) | S |
| I-10 | Normalize BM25 scores to [0,1] before display (G18) | M |

### Tier 2 -- High Impact, Higher Effort

| ID | Improvement | Est. Effort |
|---|---|---|
| I-11 | Persistent repo file index with content-hash cache | L |
| I-12 | Filesystem watcher for incremental index updates | L |
| I-13 | Chat corpus as first-class SearchCorpus::Chats | L |
| I-14 | Tree-sitter symbol extraction index | XL |
| I-15 | Search telemetry: latency + backend-mix + zero-results (G9) | M |
| I-16 | Progressive/streaming search results in UI | L |
| I-17 | Result preview panel on hover | M |
| I-18 | Enable web-scrape feature in GUI build (G15) | S |
| I-19 | Query intent prefix routing (@memory, #tag, >cmd) | M |
| I-20 | Open in editor for repo/chunk hits (not just copy path) | M |

### Tier 3 -- New Corpora

| ID | Improvement | Est. Effort |
|---|---|---|
| I-21 | SearchCorpus::GitHistory -- commit/blame search | XL |
| I-22 | SearchCorpus::TelemetryEvents -- query past events | L |
| I-23 | SearchCorpus::AgentLogs -- conversation history search | L |
| I-24 | Ingest docs/src/ into DocumentChunks at startup | M |
| I-25 | Ingest contracts/ YAML into knowledge graph | M |
| I-26 | Index Cargo.toml feature flags as structured metadata | M |
| I-27 | MCP tool for symbol definition lookup | L |
| I-28 | MCP tool for call graph / callers query | XL |

---

## 7. SSOT Refinements Required

The existing search-retrieval-ssot-2026.md needs the following additions:

1. Add SearchCorpus::Chats -- or explicitly document that chats are a post-hoc corpus handled outside the standard planner, with justification.
2. Document SymbolProximity routing gap -- the GUI code scope maps to symbol but this does not reliably invoke the corpus.
3. Add GUI surface table -- SSOT section 6 does not document the vox_search_query Tauri command or the federated index.
4. Specify score normalization policy -- what range should hit scores be in? The SSOT is silent.
5. Document feature gate matrix -- tantivy-lexical, qdrant-vector, tavily, web-scrape feature gates and which surfaces activate which.
6. Add federated index contract -- contracts/gui/omnisearch-index.v1.yaml exists but is not referenced in the SSOT.

---

## 8. Code Review Findings (Pre-Submit)

Issues identified during this audit that should be caught before any search-related changes are merged:

### BUG: chats scope silently fans out to all corpora (search.rs:295)

When scope = ["chats"]:
- scope_corpora maps "chats" through scope_to_corpus -> None -> filtered out -> scope_corpora = Some([]) -> treated as None
- The else branch calls run_search_with_verification with the FULL heuristic plan
- wants_chats is true (scope_tags contains "chats"), so chat results append (lines 295-318)
- User asked for chats-only, but receives chat results PLUS all-corpora results (superset)
- Chat messages are indistinguishable from memory/chunk/repo hits in the result list

Fix: Add SearchCorpus::Chats to make chats a plannable corpus, or short-circuit: detect scope_tags == ["chats"] before calling run_search_with_verification and route only to chat_search_gui_messages.

### BUG: Frontend glob pathMatchesGlob is broken for ** (SearchView.tsx:45-52)

  const normalized = pattern.replace(/^\*\*\//, '').replace(/\*\*/g, '').replace(/\*/g, '');

"**/*.rs" -> "*.rs" -> ".rs" (after star removal, the leading * is consumed but the dot and extension remain)
-> normalized is ".rs" (truthy) -> path.includes('.rs') -> matches any path containing ".rs" anywhere.
This means "**/*.rs" acts as a substring match for ".rs", catching file paths like "bars/baz.rs" AND paths
containing ".rs" in directory names or other extensions -- not a proper recursive glob.
The correct behavior requires full glob-to-regex conversion.

Fix: Move glob matching to the backend glob_match in search.rs (which is correct via recursive DP), passing the
path_glob parameter through the existing vox_search_query scope filter, or implement proper glob-to-regex
conversion in TypeScript (e.g., micromatch or a simple recursive DP port of the backend implementation).

### BUG: KnowledgeGraph score always 0.0 (execution.rs:388)

  score: 0.0,

Knowledge node hits are inserted into unified_hits with score 0.0, placing them at the bottom of any score-sorted result list regardless of query match quality. The query_knowledge_nodes DB function already does FTS/LIKE ranking -- the rank should be propagated.

Fix: Extend VoxDb::query_knowledge_nodes to return a score or rank; propagate it in execution.rs.

### LATENT BUG: Memory engine race on concurrent searches (memory_cache.rs)

The cached_memory_engine function uses a process-global cache. When two GUI searches fire within the 200ms debounce window, both may construct separate MemorySearchEngine instances from disk.

Fix: Ensure the cache uses Arc<RwLock<>> with proper async locking; return the same instance for concurrent callers.

---

## 9. 2026 Roadmap Gestures

This section sketches the direction of a full implementation plan (to be authored separately as omni-search-implementation-plan-2026.md).

### Phase A: Fix & Consolidate (Weeks 1-2)
Fix all Tier 1 bugs (G12, G13, G3) and enable RRF by default. Pure fixes with no new infrastructure. Unlocks correct behavior from the existing stack.

### Phase B: Telemetry + Score Normalization (Weeks 2-3)
Instrument every search call with backend-mix, latency, result count. Normalize scores across corpora. Add aria-live accessibility. Enables data-driven ranking improvements.

### Phase C: Persistent Repo Index (Weeks 3-6)
Build a content-hash-cached Tantivy index for repo files with tree-sitter symbol extraction. Add filesystem watcher for incremental updates. Graduate repo_inventory_max_files limit to a soft warning with UI indicator.

### Phase D: Chat as First-Class Corpus (Weeks 4-5)
Add SearchCorpus::Chats to the planner. Embed chat messages at ingest time. Add hybrid BM25+vector search for chats. Fix the scope mapping bug.

### Phase E: Omni-Search UX Elevation (Weeks 5-8)
Progressive streaming results. Result preview panel. Intent prefix routing. Fuzzy matching in federated index. "Open in editor" for file hits. Share facet state between palette and SearchView.

### Phase F: Agent-Facing Corpus Expansion (Weeks 8-12)
Git history corpus. Symbol definition MCP tool. Call-graph traversal. Automatic ingestion of docs/src/ and contracts/ into knowledge graph.

---

## 10. Related Documents

- search-retrieval-ssot-2026.md -- canonical pipeline contracts (update after findings here)
- data-storage-ssot-2026.md -- DB schema including search_documents, embeddings
- vox-gui-capability-audit-2026.md -- GUI surface audit
- vox-gui-surface-map-2026-06-14.md -- surface map
- deep-research-prior-art-and-vox-roadmap-2026.md -- web research pipeline prior art

---

## Appendix A: Feature Gate Matrix

| Feature Gate | Default | Enables | GUI Build | MCP/CLI Build |
|---|---|---|---|---|
| tantivy-lexical | Off | Tantivy doc mirror index | No | Optional |
| qdrant-vector | On | Qdrant ANN sidecar | If URL set | Yes |
| tavily | On | Tavily web tier | If API key set | Yes |
| web-scrape | Off | Full-page HTML extraction | MISSING | Optional |

Note: web-scrape being off in the GUI build means web search results in SearchView are snippets only (30-100 chars) while the same query via MCP returns full extracted markdown. This is a significant quality disparity.

---

## Appendix B: Env / Secret Knobs Affecting GUI Search

| Secret / Env | Default | Effect |
|---|---|---|
| VOX_SEARCH_PREFER_RRF | false | Cross-corpus fusion -- SHOULD be true |
| VOX_SEARCH_MEMORY_VECTOR_WEIGHT | 0.55 | Memory BM25/vector blend |
| VOX_SEARCH_CHUNK_VECTOR_WEIGHT | 0.60 | Chunk FTS/embedding blend |
| VOX_SEARCH_BM25_K1 | 1.2 | Memory BM25 saturation |
| VOX_SEARCH_BM25_B | 0.75 | Memory BM25 length norm |
| VOX_SEARCH_RRF_K | 60.0 | RRF smoothing constant |
| VOX_SEARCH_REPO_MAX_FILES | 20000 | File scan cap (often too low) |
| VOX_SEARCH_QDRANT_URL | unset | Qdrant sidecar ANN |
| VOX_SEARCH_TANTIVY_ROOT | unset | Tantivy doc mirror |
| VOX_SEARCH_SEARXNG_URL | unset | SearXNG instance |
| VOX_SEARCH_TAVILY_ENABLED | false | Tavily web tier |
| VOX_SEARCH_PERSIST_WEB_HITS_DISABLED | unset | Web hit caching to DB |
