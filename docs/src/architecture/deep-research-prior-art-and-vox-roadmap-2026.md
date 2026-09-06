---
title: "Deep Research Prior Art and Vox Integration Roadmap (2026)"
description: "Maps Gemini Deep Research, OpenClaw/SearchClaw/ScienceClaw, Anthropic Claude Research, Tavily, and adjacent agents to Vox’s `run_research` + `vox-search` stack; documents stubs, shipped surfaces, free-tier strategy, and a four-phase implementation roadmap including CLI/MCP exposure."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Maps external deep-research agent designs (Gemini, OpenClaw/Claw family, Tavily, Anthropic Claude Research) to Vox's run_research pipeline and vox-search backends; sequences the unblocking work."
schema_type: "TechArticle"
audience: ["contributors", "agents"]
related:
  - docs/src/reference/tavily-integration-ssot.md
  - docs/src/architecture/search-retrieval-ssot-2026.md
  - docs/src/architecture/scientia-self-publication-finalization-plan-2026.md
  - docs/src/architecture/scientia-mesh-integration-research-2026.md
  - docs/src/architecture/scientia-automated-research-historical-extension-research-2026.md
  - crates/vox-research-shim/src/research/orchestrator/pipeline.rs
  - crates/vox-search/src/web_dispatcher.rs
  - crates/vox-search/src/crag.rs
---

# Deep Research Prior Art and Vox Integration Roadmap (2026)

## 1. Executive summary

**Deep research** (working definition for Vox) is an orchestrated pipeline that:

1. **Plans** — decomposes a user topic into sub-queries (and optionally iterative refinements).
2. **Retrieves** — searches the open web and/or local corpora with policy-gated backends (SearXNG → DuckDuckGo → optional Tavily; optional HTML extraction via `web-scrape`).
3. **Iterates** — when evidence is weak, expands queries (CRAG-style) up to a bounded hop count.
4. **Grounds** — extracts claims, optionally verifies them against sources (when wired).
5. **Synthesizes** — produces a cited answer and structured metadata (routing tier, diagnostics, judge score).

Optional dimensions aligned with commercial products: **human checkpoints**, **async/long-running jobs**, and **mesh-durable execution**. For Vox, mesh-durable execution is a **forward hook only**: `@durable` / `workflow` / `activity` are parsed and lowered per [`AGENTS.md`](../../../AGENTS.md) §Grammar Unification, but durable replay/cron semantics are **not** production-complete — see [`durability-runtime-audit-2026.md`](durability-runtime-audit-2026.md) and ADR-028 proposal.

**Strategic anchor:** The SCIENTIA self-publication program targets longitudinal provider observability and publication-quality outputs ([`scientia-self-publication-finalization-plan-2026.md`](scientia-self-publication-finalization-plan-2026.md)). The deep-research pipeline is the substrate that can feed evidence bundles into that loop when paired with [`scientia-mesh-integration-research-2026.md`](scientia-mesh-integration-research-2026.md) signal families (`DiscoverySignalFamily`, `FindingCandidateClass`). For a consolidated historical map, code-review findings, integration matrix (OpenRouter / Tavily / Tantivy / Graphify), and **Track F** unified-loop roadmap, see [`scientia-automated-research-historical-extension-research-2026.md`](scientia-automated-research-historical-extension-research-2026.md).

**Non-duplication:** Tavily endpoint shapes, secrets lifecycle, pricing, fail-open rules, and Firecrawl comparison live in [`docs/src/reference/tavily-integration-ssot.md`](../reference/tavily-integration-ssot.md). This document **links** there instead of copying tables.

---

## 2. Disambiguation: what people mean by “Claw”

Voice transcription often yields “Claw” without specifying the product. Three distinct references appear in industry and this repo:

| Row | What it is | Typical UX | Vox relevance |
|-----|------------|------------|---------------|
| **A. OpenClaw Deep Research Agent** | Skill/agent pattern in the OpenClaw ecosystem (multi-round web search, structured report, configurable iterations). | Async-ish batch runs (minutes), markdown/HTML output. | Closest analog: [`vox-search::crag::CragRouter`](../../../crates/vox-search/src/crag.rs) + [`WebSearchDispatcher`](../../../crates/vox-search/src/web_dispatcher.rs) — **shipped in `vox-search`**, must be driven from orchestrator research ([§6](#6-target-architecture-four-phases)). |
| **B. SearchClaw / ScienceClaw / ClawHub “Academic Deep Research”** | Research harnesses with explicit quality gates (citation counts, source diversity), many literature APIs, checkpointed workflows. | Long runs, academic citation styles. | **Partial:** diagnostics exist in `run_research` (`RetrievalDiagnostics`); **not built:** minimum citation diversity enforcement, APA tooling, 77-database integrations. |
| **C. Anthropic Claude Research mode** | Hosted Claude capability for web research with inline citations (consumer + API surfaces). | Sync/async report with citations. | **Not orchestrated by Vox:** we may call Anthropic as an LLM backend for synthesis/judge ([`stages.rs`](../../../crates/vox-research-shim/src/research/orchestrator/stages.rs)) but do not invoke Claude’s hosted “Research” product as a black box. |

OpenClaw **docs refresh** for operators already exists on the CLI path ([`db_research/refresh.rs`](../../../crates/vox-cli/src/commands/db_research/refresh.rs) — `OPENCLAW_REFRESH_URLS`).

---

## 3. Prior art matrix

Columns: **triggering UX**, **planning**, **retrieval tools**, **memory/session**, **citations**, **cost/latency**, **access**, **limitations**, **Vox analog** (file or verdict).

### 3.1 Google Gemini Deep Research / Deep Research Max

| Dimension | Notes |
|-----------|--------|
| UX | Consumer app + API (Interactions / agent surfaces); “Max” variant emphasizes higher search/token budgets and async completion. |
| Planning | Iterative plan → search/read → gap fill → report. |
| Tools | Web search/browse; MCP connectors; Workspace connectors in consumer SKU. |
| Memory | Session-bound; export report artifacts. |
| Citations | Report-style citations (implementation details are vendor-side). |
| Cost/latency | High token + many search steps on “Max”; vendor-metered. |
| Access | Google AI / Gemini API; Google account / Cloud billing. |
| Limits | Vendor lock-in; enterprise data residency policies; eval claims require primary citations. |
| **Vox analog** | [`planner.rs::decompose_query_with_config`](../../../crates/vox-research-shim/src/research/planner.rs) — **STUB** (passthrough). Retrieval: [`web_gather.rs`](../../../crates/vox-research-shim/src/research/orchestrator/web_gather.rs) — **now delegates to `vox-search` web tier** (Phase 1 shipped in this workstream). |

**Primary sources (appendix §10).**

### 3.2 OpenClaw Deep Research Agent (skill ecosystem)

| Dimension | Notes |
|-----------|--------|
| UX | Skill/config driven; multi-round search (often ~5), cross-source validation narrative. |
| Planning | Prompt/scaffolding defines rounds and output shape. |
| Tools | Gateway-discovered tools + HTTP skills; web search depends on deployment. |
| Memory | Skill/session dependent. |
| Citations | Markdown reports with links. |
| Cost/latency | Token-heavy; operator-hosted gateway. |
| Access | OpenClaw gateway + skills marketplace/docs. |
| Limits | Ecosystem fragmentation; skill quality varies by publisher. |
| **Vox analog** | [`CragRouter`](../../../crates/vox-search/src/crag.rs) + policy [`web_search_max_hops`](../../../crates/vox-search/src/policy.rs) — reuse from orchestrator after initial web gather (Phase 2 in this workstream). |

### 3.3 SearchClaw / ScienceClaw / ClawHub academic flows

| Dimension | Notes |
|-----------|--------|
| UX | Benchmark-oriented harnesses (e.g. BrowseComp claims for SearchClaw); academic checkpoints. |
| Planning | Decomposition + structured evidence trails. |
| Tools | Many APIs (Semantic Scholar, arXiv, news, …). |
| Memory | Persistent harness state across sessions (paper narrative). |
| Citations | Minimum counts / diversity constraints (SearchClaw “harness engineering”). |
| Cost/latency | API-rate-limit sensitive. |
| Access | GitHub / skill hubs. |
| Limits | Ops burden to keep API keys and rate limits healthy. |
| **Vox analog** | **Not built** as a dedicated harness; closest telemetry is [`RetrievalDiagnostics`](../../../crates/vox-research-shim/src/research/types.rs) + future **citation-diversity gate** (Phase 2 backlog). |

### 3.4 Anthropic Claude Research mode

| Dimension | Notes |
|-----------|--------|
| UX | Hosted research reports from Claude apps/API. |
| Planning | Closed-source agent loop. |
| Tools | Web search / browsing (vendor-side). |
| Citations | Inline citations in output. |
| Limits | Not portable across providers; policy constraints. |
| **Vox analog** | **Not integrated** — Vox keeps retrieval in [`vox-search`](search-retrieval-ssot-2026.md) and uses LLM endpoints for synthesis/judge only. |

### 3.5 Tavily

All endpoint and pricing tables: **[`tavily-integration-ssot.md`](../reference/tavily-integration-ssot.md)**.

| Endpoint | In Vox today |
|----------|----------------|
| `/search` | Yes — [`TavilySearchClient::search`](../../../crates/vox-search/src/tavily.rs) via [`WebSearchDispatcher`](../../../crates/vox-search/src/web_dispatcher.rs) Tier 4 when policy enables and prior tiers empty. |
| `/extract` | **Not wired** in orchestrator research (future: weak-snippet uplift). |
| `/research` | **Not wired** (would collapse multi-hop into one vendor call; evaluate cost/benefit vs native CRAG). |
| `/crawl` | **Not wired** into research pipeline (doc ingestion uses other paths). |

### 3.6 Peers (short rows)

| Product | Role | Vox stance |
|---------|------|------------|
| Perplexity Pro / ChatGPT Deep Research / You.com | Closed UX + vendor search stacks | Benchmark UX only; no dependency for core pipeline. |
| Exa / Bright Data SERP | Alternative search/extract vendors | Policy comparison only; Tavily SSOT already notes SERP patterns. |

---

## 4. Free / self-hosted tier strategy

Canonical retrieval policy and corpus matrix: [`search-retrieval-ssot-2026.md`](search-retrieval-ssot-2026.md).

| Source | In repo today | Slot | Secrets / env |
|--------|----------------|------|----------------|
| **SearXNG** | Tier 2 in [`web_dispatcher.rs`](../../../crates/vox-search/src/web_dispatcher.rs); sidecar via `vox research up` ([`research/infra.rs`](../../../crates/vox-cli-research/src/infra.rs)) | Primary self-hosted web tier | `VOX_SEARCH_SEARXNG_URL` etc. via [`SearchPolicy::from_env`](../../../crates/vox-search/src/policy.rs) |
| **DuckDuckGo** | Tier 3 fallback | Free fallback when SearXNG empty/fails | Policy toggle `duckduckgo_fallback_enabled` |
| **Tavily** | Tier 4 when configured | Low-friction ranked snippets | [`TavilyApiKey`](../../../crates/vox-secrets/src/spec/ids.rs) + `VOX_SEARCH_TAVILY_*` — see Tavily SSOT |
| **Wikipedia / Wikidata** | Not wired | Tier 1.5 high-trust factual blurbs | Future: register read-only HTTP (likely no secret); add env registry row in [`contracts/config/env-vars.v1.yaml`](../../../contracts/config/env-vars.v1.yaml) if introducing `VOX_SEARCH_WIKI_*` toggles |
| **arXiv API** | Not wired | STEM literature slice | Future `SecretId` only if using authenticated tier |
| **Crossref REST** | Not wired | DOI metadata | Polite pool + optional mailto — register env var if adding |
| **Semantic Scholar Graph API** | Not wired | Citation expansion | Plan mentions `SecretId::SemanticScholarApiKey` — **not implemented** |
| **Internet Archive Wayback** | Not wired | Dead-link recovery | Respect IA terms; throttle |

---

## 5. Gap analysis — stubs vs shipped surfaces

### 5.1 `PHASE_0a_STUB` modules in `crates/vox-research-shim/src/research/`

| Module | Role today | Replacement / delegation target |
|--------|------------|----------------------------------|
| [`planner.rs`](../../../crates/vox-research-shim/src/research/planner.rs) | Passthrough single subquery | Future: LLM/Mens decomposition — **not** replaced in this workstream |
| [`provider.rs`](../../../crates/vox-research-shim/src/research/provider.rs) | Empty search/map_site | Future: unify with `vox-search` providers / mesh `ProviderObservation` — **not** replaced here |
| [`web_gather.rs`](../../../crates/vox-research-shim/src/research/orchestrator/web_gather.rs) | Was empty | **`WebSearchDispatcher::search` + CRAG refinements** ([`web_dispatcher.rs`](../../../crates/vox-search/src/web_dispatcher.rs), [`crag.rs`](../../../crates/vox-search/src/crag.rs)) — **implemented** |
| [`claims.rs`](../../../crates/vox-research-shim/src/research/claims.rs) | Empty claims | Future `vox-claim-extractor` per module header — **stub** |
| [`verifier.rs`](../../../crates/vox-research-shim/src/research/verifier.rs) | Empty verdicts | Future verifier wiring — **stub** |
| [`model_select.rs`](../../../crates/vox-research-shim/src/research/model_select.rs) | Static fallbacks | Future registry merge — **stub** |
| [`pipeline_cache.rs`](../../../crates/vox-research-shim/src/research/orchestrator/pipeline_cache.rs) | Always miss | Future `list_memories_by_type` — **stub** |
| [`pipeline.rs`](../../../crates/vox-research-shim/src/research/orchestrator/pipeline.rs) | Session id `0`, persistence comments | Future vox-db methods — **stub** |

### 5.2 Cross-surface tooling

| Capability | Status |
|------------|--------|
| MCP `vox_memory_search` | Shipped — [`handlers_memory.rs`](../../../crates/vox-orchestrator-mcp/src/memory_tools/handlers_memory.rs) |
| MCP **`vox_research_run`** | **Shipped** (this workstream) |
| CLI **`vox research run`** | **Shipped** (this workstream) |
| CLI `vox research eval` | Shipped — [`eval.rs`](../../../crates/vox-cli-research/src/eval.rs); golden queries extended |

### 5.3 `run_search_with_verification` note

[`run_search_with_verification`](../../../crates/vox-search/src/bundle.rs) performs a **full corpus** retrieval pass (memory, chunks, repo, web…) and requires a [`SearchRuntimeContext`](../../../crates/vox-search/src/context.rs). The orchestrator **`web_gather`** path intentionally uses **`WebSearchDispatcher`** for bounded web retrieval without requiring DB/memory paths on every research caller. A future bridge can attach `SearchRuntimeContext` from MCP `ServerState` when research is invoked server-side.

---

## 6. Target architecture — four phases

### Phase 1 — Unblock web retrieval (done here)

- Implement `gather_web_hits_for_plan` using `SearchPolicy::from_env()` + `WebSearchDispatcher::search`.
- Respect `ResearchScope::Local` (skip web) and `ResearchQuery::site_scope` (post-filter host).
- Map `HybridSearchHit` → `ResearchHit`.

### Phase 2 — CRAG loop (done here)

- After initial subqueries, while hops remain, call `CragRouter::expand_queries_from_partial_evidence` / `should_continue` against average score vs target `0.75`, capped by `policy.web_search_max_hops`.

### Phase 3 — CLI + MCP + contracts (done here)

- `vox research run <query> [--json] [--scope ...]`
- `vox_research_run` MCP tool returning JSON `ResearchResult`.
- Operations catalog + MCP registry rows regenerated via `vox ci operations-sync --target cli --write`.

### Phase 4 — Scientia + mesh (forward hooks)

- Emit `DiscoverySignal` / `FindingCandidate` artifacts per [`scientia-mesh-integration-research-2026.md`](scientia-mesh-integration-research-2026.md).
- Mesh durable scheduling: **document only** until workflow runtime completes ADR-028 / durability audit outcomes.

---

## 7. Risk register

| Risk | Mitigation |
|------|------------|
| Web ToS / robots | Honor `scraper_robots_txt_respect`; prefer APIs for Wikipedia/arXiv when added |
| Tavily spend | Session budget + fail-open behavior — Tavily SSOT |
| Secret leakage | **Never** `std::env::var("TAVILY_API_KEY")` in consumers — secrets policy (`.cursor/rules/secrets-policy.mdc`) |
| Prompt injection from pages | Treat snippets as untrusted; truncate per policy |
| Non-deterministic CI | Smoke tests allow empty web hits offline; live test `#[ignore]` |

---

## 8. Verification & evaluation

| Command | Purpose |
|---------|---------|
| `cargo test -p vox-orchestrator` | Unit + integration smoke |
| `cargo test -p vox-search` | Retrieval regression |
| `vox research run "..." --json` | Manual end-to-end (needs network / keys) |
| `vox research eval` | Harness writes metrics rows — extend golden queries in [`eval.rs`](../../../crates/vox-cli-research/src/eval.rs) |
| `vox ci command-compliance` / `vox ci operations-verify` | Contract hygiene after catalog edits |

---

## 9. Acceptance checklist (from plan §10)

- [x] Architecture doc at `docs/src/architecture/deep-research-prior-art-and-vox-roadmap-2026.md`
- [x] Tavily SSOT cited, not duplicated
- [x] Scientia finalization plan referenced
- [x] Stub inventory + vox-search mapping (§5)
- [x] `where-things-live.md`, `search-retrieval-ssot-2026.md` cross-links
- [x] CLI + MCP surfaces shipped
- [x] No new shell/Python automation

---

## 10. Sources appendix (primary URLs)

Captured **2026-05-11** (verify periodically; vendor URLs drift).

| Topic | URL |
|-------|-----|
| Gemini Deep Research (developers blog) | `https://blog.google/innovation-and-ai/technology/developers-tools/deep-research-agent-gemini-api/` |
| Gemini Deep Research Max announcement | `https://blog.google/innovation-and-ai/models-and-research/gemini-models/next-generation-gemini-deep-research/` |
| Gemini consumer overview | `https://gemini.google/overview/deep-research/` |
| OpenClaw docs (gateway — refresh list in CLI) | `https://openclawlab.com/en/docs/gateway/protocol/` |
| OpenClaw Deep Research skill (tutorial mirror) | `https://openclaw.com/en/skills/deepresearchagent.html` |
| SearchClaw repository | `https://github.com/RUC-NLPIR/SearchClaw` |
| ScienceClaw repository | `https://github.com/Zaoqu-Liu/ScienceClaw` |
| Anthropic news index (search “Research”) | `https://www.anthropic.com/news` |
| Tavily docs | `https://docs.tavily.com/` |
| SearXNG project | `https://github.com/searxng/searxng` |
| DuckDuckGo | `https://duckduckgo.com` |
| Wikipedia API | `https://www.mediawiki.org/wiki/API:Main_page` |
| Wikidata API | `https://www.wikidata.org/wiki/Wikidata:Data_access` |
| arXiv API | `https://info.arxiv.org/help/api/index.html` |
| Crossref REST API | `https://github.com/CrossRef/rest-api-doc` |
| Semantic Scholar API | `https://api.semanticscholar.org/` |
| Internet Archive Wayback | `https://archive.org/help/wayback_api.php` |
