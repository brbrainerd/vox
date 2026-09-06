---
title: "Deep Research Capabilities Audit — Verification Pass (2026-08-01)"
description: "Re-verifies all 9 gaps from the 2026-06-17 audit against current code, with file:line evidence and commit hashes: G4 (free-tier routing) and G5 (confidence gate fusion) resolved, G1/G3/G6 partially addressed with a newly-found pipeline split, G2/G7/G8/G9 still open. Flags that core module doc comments still falsely claim Phase 0a stub state."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative delta-verification of the 2026-06-17 deep-research audit; corrects stale claims and identifies a real pipeline-split integration gap not previously documented."
---

# Deep Research Capabilities Audit — Verification Pass

**Date:** 2026-08-01 (six weeks after the original audit)
**Scope:** Re-checks each of the 9 ranked gaps from [deep-research-capabilities-audit-2026-06-17.md](deep-research-capabilities-audit-2026-06-17.md) against current source, with file:line evidence and `git log` commit references. Companion to [deep-research-fundamentals-2026-08-01.md](deep-research-fundamentals-2026-08-01.md) and the other 2026-08-01 deep-research docs.

---

**Headline finding:** most of the audit's Phase 1/2 roadmap landed within 24–48 hours of the audit itself, in commit `89f2e902b8` ("feat(crates): remaining crate changes", 2026-06-18) plus three follow-ups on 2026-06-19 (`f50f36b8e6`, `4da9ce9052`, `309c9eea98`). Nothing material to these 9 gaps has changed in the six weeks since — that period was dominated by unrelated CI/clippy/graphify work.

## G1 — LLM-driven CRAG query expansion: PARTIALLY ADDRESSED

The audit's exact proposed design shipped: `crates/vox-research-shim/src/research/orchestrator/web_gather.rs:44` (`try_llm_query_expansion`) calls the LLM cascade with a "research gap analyst" prompt matching the audit's P2.1 spec, parses `{"followup_queries": [...]}`, and feeds it into `CragRouter::expand_queries_with_llm_or_heuristic` (`crates/vox-search/src/crag.rs:101`), falling back to the regex heuristic only on LLM failure. Wired at `web_gather.rs:338-343`, used by `vox research run` and the `research_run`/`research_start` MCP tools.

However, a **second, independent CRAG loop still exists** — `crates/vox-search/src/research.rs:101` (`run_multi_hop_web_research`, used by `vox-orchestrator`'s autonomous task-dispatch path at `crates/vox-orchestrator/src/orchestrator/task_dispatch/research_dispatch.rs:27`) — and it still calls the **pure heuristic** `CragRouter::expand_queries_from_partial_evidence` directly, never the LLM-aware wrapper. Fixed in the primary pipeline, not in the autonomous-agent loop — a split not called out in the original audit (see New Findings #3).

## G2 — Cross-encoder reranking: STILL OPEN

No `reranker.rs`, no `rerank`/`cross_encoder`/`CrossEncoder` symbol anywhere in `crates/vox-search` or `crates/vox-research-shim` (grep confirms zero matches). Fusion is still RRF-only via `rrf.rs`; the P3.1 plan (`ms-marco-MiniLM`/`bge-reranker-v2-m3`, `policy.reranking_enabled`) was not started. Matches the gap identified independently in [deep-research-fundamentals-2026-08-01.md §6](deep-research-fundamentals-2026-08-01.md).

## G3 — Novelty / deduplication scoring: PARTIALLY ADDRESSED

A real `NoveltyScorer` now exists at `crates/vox-search/src/novelty.rs` (added `1ea0ae94a0`, 2026-06-18), implementing the 4-gram FNV1a shingling design from the audit's §7.2 — `score()` returns the fraction of new shingles, `accept()` commits them. Genuinely wired into `run_multi_hop_web_research` (`crates/vox-search/src/research.rs:42-83`): each hit is scored against accumulated session evidence, gated on `policy.novelty_min_score` (default `0.15`), only accepted hits are appended to evidence text.

Of the four integration points the audit's P2.2 called for, only one shipped:
1. `run_multi_hop_web_research` filtering — done.
2. `gather_web_hits_for_plan` cross-subquery dedup (`web_gather.rs`) — not done; zero `NoveltyScorer` references, still plain URL-set dedup (`pipeline.rs:222`, `dedupe_hits_by_url`).
3. Sort-by-novelty before synthesis truncation — not present in `stages.rs`.
4. SCIENTIA write-back novelty check — `discovery_bridge.rs` only carries a `novelty_evidence_bundle_id: None` placeholder (lines 90, 165); nothing computes it.

**Naming disambiguation:** `vox-scientia`'s `AtomicNoveltyScorer` / `vox-publisher`'s `scientia_novelty_assess.rs` is a *different*, pre-existing prior-art/literature novelty system, unrelated to this per-hit information-gain gap — see New Findings #5 and [the trust/novelty doc](deep-research-trust-novelty-scoring-landscape-2026-08-01.md) for the distinction.

Net: novelty scoring exists and is real, but only guards the autonomous-agent loop — the same loop still stuck on heuristic-only CRAG per G1 — not the main research-shim pipeline most users hit.

## G4 — Free-tier OpenRouter routing: RESOLVED

The cleanest fix of the nine. `crates/vox-actor-runtime/src/llm/cascade.rs:79-106` (`research_openrouter_model_ids`) now always appends concrete, dispatchable `:free` model slugs (`vox_config::OPENROUTER_FREE_FALLBACK_MODELS`) as a zero-cost fallback floor to every research cascade; `VOX_RESEARCH_PREFER_FREE_TIER` (`crates/vox-config/src/inference.rs:215`) can move them to the front. Landed across `f50f36b8e6` (env gate), `4da9ce9052` (append free floor), `309c9eea98` (fix: dispatch concrete `:free` slugs instead of the virtual `openrouter/free`, because the real OpenRouter API rejects the virtual router id when dispatched raw — the audit's own P1.1 proposal would have shipped this bug). Regression test at `cascade.rs:265-291`.

Note: per [the multi-provider doc, Part A3](deep-research-model-agnostic-multi-provider-and-skills-publication-2026-08-01.md), the *scorer* path still structurally excludes free models under the research intent's `CostPreference::Performance`, so this free-tier floor only helps via the OpenRouter-lane reordering, not via the primary model-selection path.

## G5 — Confidence gate multi-signal fusion: RESOLVED (with stale docs — see New Findings #1)

`crates/vox-research-shim/src/research/gate.rs:82-102` (`score_with_config`) now implements the exact fusion formula from the audit's P2.3: `citation_score*0.35 + claim_support_score*0.30 + diversity_score*0.20 + retrieval_score*0.15`, landed in `89f2e902b8`. Wired end-to-end in `pipeline.rs:386-408`: `supported_claim_count` from real claim verdicts, `distinct_domain_count` from `registrable_domain()` over all hits, both feeding `GateInput` before `routing_tier_for()` picks the tier. Tests at `gate.rs:189-223` validate the fusion.

## G6 — Synthesis context budget: PARTIALLY ADDRESSED, net effect still insufficient

The routing bug the audit blamed is fixed: `cascade.rs:191-201` (`apply_stage_defaults`) explicitly skips setting `max_tokens` for `ResearchStage::Synthesis`, with a regression test (`cascade.rs:238-256`, `synthesis_stage_does_not_force_1800_max_tokens`) guarding against the hard-coded 1,800 ever coming back. `config.synthesis_max_tokens` really does win the call path (`pipeline.rs:444` → `stages.rs:444`).

But **`ResearchConfig::synthesis_max_tokens`'s default is `1200`** (`crates/vox-research-shim/src/research/orchestrator/config.rs:136`) — unchanged since introduction, no env override. The routing bug is fixed but the effective ceiling (1,200) is now **lower** than the 1,800 the audit flagged as too small, nowhere near the 4,000 the plan recommended. Long-form reports remain truncation-prone; functionally still a live gap despite a code review showing "fixed."

## G7 — Semantic result caching: STILL OPEN

`crates/vox-research-shim/src/research/orchestrator/pipeline_cache.rs` unchanged in substance since before the audit. `research_cache_key()` (lines 63-72) still builds an FNV1a hash of `(normalized_query, scope, max_sources, verify_claims)` — exact-match, not embedding-similarity. `research_cache_short_circuit` (lines 8-24) does a linear scan for exact key match within TTL. No cosine-similarity threshold, no stored embeddings — P3.3 not started.

## G8 — Async / durable research jobs: STILL OPEN

`crates/vox-orchestrator-mcp/src/memory_tools/handlers_memory.rs:335-421` (`research_start`) dispatches async via `tokio::spawn`, tracks a DB-backed session, with a companion `research_status`/`research_get` poll surface — but this pattern predates the audit (introduced before 2026-06-17) and hasn't changed since. Remains fire-and-forget in-process `tokio::spawn`, not a durable queue: if the orchestrator process restarts mid-run, the spawned task is lost (the session row sits at `"running"` forever). No wiring to `vox-orchestrator-queue`'s durable oplog for research jobs specifically. P4.1 not started.

## G9 — Per-hop span observability: STILL OPEN

`crates/vox-research-events/src/schema_types.rs` has zero `hop`-related types. The only per-hop signal is a plain `tracing::info!(hop = ..., query_count = ..., "starting research hop")` log line at `crates/vox-search/src/research.rs:54-58` — a log statement, not a structured span/metric, predating the audit. No `research.hop.{n}.{metric}` telemetry exists. P4.2 not started.

## Summary

| Gap | Status |
|---|---|
| G1 — LLM-driven CRAG expansion | Partially addressed (primary pipeline only; autonomous loop still heuristic-only) |
| G2 — Cross-encoder reranking | Still open |
| G3 — Novelty/dedup scoring | Partially addressed (same pipeline split as G1) |
| G4 — Free-tier OpenRouter routing | Resolved (floor works; scorer path still excludes free models) |
| G5 — Confidence gate fusion | Resolved (docs now stale/misleading) |
| G6 — Synthesis context budget | Partially addressed; effective ceiling now *lower* (1200) than the flagged-too-small 1800 |
| G7 — Semantic result caching | Still open |
| G8 — Async/durable research jobs | Still open |
| G9 — Per-hop span observability | Still open |

## New findings not in the original audit

1. **Stale "Phase 0a stub" documentation is now actively misleading.** `crates/vox-research-shim/src/research/mod.rs:4-8` still declares the whole module "currently in Phase 0a stub state: types are real, behavior returns empty/default values" — false today; claim extraction, verification, gate scoring, and synthesis are all real LLM-backed implementations. Worse, `crates/vox-research-shim/src/research/verifier.rs:99-100` explicitly says `verify_claims_with_config` "**PHASE_0a_STUB**: returns `Vec::new()`" directly above ~90 lines of real cascade-based NLI verification logic (lines 114-193) with JSON parsing, evidence spans, and abstain-threshold handling. A future engineer grepping for `PHASE_0a_STUB` — as every research agent in this program was instructed to do — gets misdirected into thinking core verification doesn't exist. **This is a near-zero-effort, high-value fix**: update the doc comments in `mod.rs` and `verifier.rs` to reflect actual state.
2. **The free-tier fix caught a real bug the original audit's own plan would have shipped**: P1.1 proposed dispatching `openrouter/free` literally as a model id; commit `309c9eea98` shows this was tried and reverted because the real OpenRouter API rejects the virtual router id when dispatched directly.
3. **G1 and G3's fixes landed in different, non-overlapping code paths** — LLM-driven expansion went into the `vox-research-shim` orchestrator pipeline (`web_gather.rs`), novelty scoring went into `vox-search`'s standalone `run_multi_hop_web_research` used by the orchestrator's autonomous task-dispatch loop. Each fix is invisible to the other pipeline: the shim pipeline still has no novelty gating, the autonomous loop still has no LLM query expansion. Not flagged in the original audit — a real integration gap worth closing explicitly, and a strong candidate for early implementation-plan work since both fixes already exist, they just need to be shared.
4. **No new TODO/FIXME/`unimplemented!`/`todo!` markers** appeared in `vox-research-shim` or `vox-search` since the audit — general code hygiene in this area has not regressed.
5. **`crates/vox-publisher/src/scientia_novelty_assess.rs`** and its `NoveltyEvidenceBundle` schema (`vox-research-events/src/schema_types.rs:149-225`) implement a *separate* prior-art/literature novelty pipeline (OpenAlex/Crossref/SemanticScholar) for SCIENTIA publication gating — unrelated to the deep-research G3 gap but easy to confuse given the shared "novelty" name. See [deep-research-trust-novelty-scoring-landscape-2026-08-01.md](deep-research-trust-novelty-scoring-landscape-2026-08-01.md) for how this schema should actually get populated.
