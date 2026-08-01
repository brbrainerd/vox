---
title: "Deep Research Enhancement — Trust/Novelty Core & Multi-Provider Routing Design"
description: "Design spec for the P0/P1 priorities from the 2026-08-01 deep-research synthesis: unify the split CRAG/novelty pipelines, fix stale stub docs, populate trust/novelty/worthiness scoring, add a post-hoc citation audit, and fix multi-provider model routing so configured GUI keys actually get used for research."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative design spec for the deep-research trust/novelty core and multi-provider routing fix; direct input to the program's implementation plan."
---

# Deep Research Enhancement — Trust/Novelty Core & Multi-Provider Routing Design

**Date:** 2026-08-01
**Inputs:** [deep-research-synthesis-and-priorities-2026-08-01.md](../../src/architecture/deep-research-synthesis-and-priorities-2026-08-01.md) and its four supporting research docs, plus [deep-research-verification-2026-08-01.md](../../src/architecture/deep-research-verification-2026-08-01.md). This spec covers only the P0/P1 items from that synthesis (items 1–10); P2–P4 are explicitly out of scope here — see "Deferred" below.

---

## Problem statement

Two independent research passes (a code audit and a competitive-landscape study) converged on the same conclusion from opposite directions:

- **From the outside in:** every major deep-research competitor (Google, Anthropic, OpenAI, Perplexity) has saturated the "breadth and iteration" axis — all claim hundreds of sources and iterative refinement. The one gap none has convincingly closed is **verifiable trust infrastructure**: auditable citation-to-claim mapping, honest confidence signaling, and novelty/quality scoring a user can actually check. Citation hallucination rates of 15–57% are documented across every reviewed product.
- **From the inside out:** Vox's own schema already anticipates exactly this — `NoveltyEvidenceBundle` and `WorthinessSignalsV2` are fully-typed, tested structures with *nothing populating them*, and the confidence gate (`gate.rs`) already does real multi-signal fusion but with no trust weighting per source.

Separately, a code audit found that Vox's multi-provider LLM infrastructure is a **split-brain**: key-presence-aware provider gating (`key_guard.rs`, `decide()`) and a full 9-provider GUI settings surface already exist and work correctly for the chat path, but the research pipeline calls an older selection path that never checks key presence, and the actual LLM egress cascade (`cascade_for_research_stage`) only has two lanes — local Ollama and OpenRouter — with no dispatch to the other 7 provider keys a user can already configure through the GUI today.

This spec designs the fix for both: closing the trust/novelty gap (the strategic differentiator) and closing the multi-provider routing gap (makes the GUI's existing key-entry surface actually functional for research).

## Scope

**In scope (P0 + P1 from the synthesis doc):**
1. Fix stale `PHASE_0a_STUB` doc comments in `mod.rs`/`verifier.rs`
2. Unify the G1/G3 pipeline split (share LLM CRAG expansion + novelty scoring between the two research loops)
3. Populate `ResearchHit.trust_score` (Crossref retraction + OpenAlex reputation)
4. Add an NLI first pass to `verifier.rs`, reconcile `Verdict` to the SciFact taxonomy
5. Populate `NoveltyEvidenceBundle` (OpenAlex/Crossref/Semantic Scholar + MinHash-LSH + embedding cosine)
6. Populate `WorthinessSignalsV2` hard/soft/diagnostic gates
7. Add a post-hoc citation audit pass
8. Route research stage selection through key-presence-aware selection
9. Make `cascade_for_research_stage` provider-aware (beyond local+OpenRouter)
10. Per-stage free-tier-aware selection once #8/#9 land

**Explicitly deferred (see synthesis doc for P2–P4):** cross-encoder reranking, synthesis token budget bump, semantic result caching, durable async jobs, per-hop observability, Reflexion-style self-revision, internal eval harness, deep-research-as-skill packaging, SCIENTIA auto-publication handoff, GUI settings-search indexing. None of these block P0/P1, and several (skill packaging, auto-publication) are explicitly gated on P1 landing first.

## Architecture

### 1. Doc-comment correction (near-zero risk, do first)

Update `crates/vox-research-shim/src/research/mod.rs`'s module doc and `verifier.rs`'s `PHASE_0a_STUB` comment to describe actual current behavior (LLM-cascade-backed claim verification, not `Vec::new()`). Pure documentation change, no behavior change, no tests needed beyond confirming doctest/doc-lint still passes.

### 2. Pipeline unification

Two independent research loops exist: `vox-research-shim`'s orchestrator (`web_gather.rs`, used by `vox research run` / MCP tools) and `vox-search`'s standalone `run_multi_hop_web_research` (used by the orchestrator's autonomous task-dispatch loop). Each has one of the two fixes the other lacks:
- `web_gather.rs` has LLM-driven CRAG expansion (`try_llm_query_expansion`) but no novelty gating.
- `research.rs::run_multi_hop_web_research` has novelty gating (`NoveltyScorer`) but only heuristic CRAG expansion.

Design: extract both `try_llm_query_expansion` and the `NoveltyScorer` integration into shared helpers callable from both loops (or, if the two loops' call shapes diverge too much for a clean shared function, duplicate the ~20-30 lines each direction rather than force an abstraction — this is a judgment call for the implementer given actual code shape, not a mandate to unify the loops themselves). The two loops remain architecturally separate; only these two capabilities need to stop being loop-exclusive.

### 3. Trust scoring

New `TrustScorer` (likely in `crates/vox-search` alongside `novelty.rs`, or a new small module): given a `ResearchHit`, resolve its URL to a DOI where possible (existing URL parsing in the hit pipeline), then:
- Query Crossref (`api.crossref.org/works/{DOI}`, free, no auth) for retraction/correction status via `update-to`/`relation` fields — hard-zero the hit's trust score on retraction.
- Query OpenAlex (free REST API) for venue `type` and author h-index — soft-weight non-retracted academic hits.
- Fall back to a small hardcoded domain-reputation table for non-resolving/non-academic web sources (this table starts minimal — a handful of clearly-high-trust domains like major standards bodies — and is explicitly not meant to be comprehensive; it's a fallback floor, not the primary signal).

Both API calls are async HTTP, cacheable by DOI/URL (short TTL is fine — retraction status changes rarely). Wire the result into `ResearchHit.trust_score` (currently hardcoded to `1.0`), then update `gate.rs::score_with_config` so `citation_score` sums `trust_score` instead of raw citation count, and `diversity_score` becomes an entropy measure over (domain, trust-tier) rather than raw distinct-domain count.

### 4. Verifier NLI first pass

Add an in-process ONNX NLI model (MiniCheck-class, e.g. `fastembed`/`ort`-backed `DeBERTa-v3-large-mnli-fever-anli-ling-wanli` or similar) as a cheap first pass in `verifier.rs::verify_claims_with_config`, ahead of the existing LLM cascade: run NLI on each (claim, evidence) pair, accept high-confidence (>0.99) verdicts directly, escalate the 0.90–0.99 band and below to the existing LLM cascade (this threshold split matches the published accuracy cliff — pure NLI drops from 98% to ~68% accuracy below 0.99 confidence). This should reduce LLM-cascade calls by roughly the fraction of claims NLI resolves confidently, while adding a second, independent verification signal.

Reconcile the existing `Verdict` enum (`Supported | Contradicted | Contested | Unverified`) to the SciFact taxonomy (`Support | Contradict | NotEnoughInfo | Abstain`) the code's own doc comment already flags as owed — this is a naming/mapping change, verify all call sites that pattern-match on `Verdict` before renaming.

### 5. Novelty evidence population

For each `FindingCandidateV1`, query OpenAlex/Crossref/Semantic Scholar (matching the existing `NoveltySource` enum variants) for prior-art works matching the candidate's claim text. Two-stage scoring:
- **Stage 1 (cheap):** MinHash-LSH shingle comparison against retrieved abstracts → `lexical_score`.
- **Stage 2 (semantic):** embedding cosine similarity (via `fastembed` for in-process ONNX embedding, indexed with `hnswlib-rs` against a persistent local corpus of prior `FindingCandidateV1` records) → `semantic_score`.

Populate `NormalizedHit.lexical_score`/`semantic_score`, `OverlapSummary.max_lexical_score`/`max_semantic_score`/`recency_bucket` on `NoveltyEvidenceBundle`. This same embedding infrastructure (stage 2) should also replace the current URL-string dedup in `web_gather.rs` and the finding_id-exact-match dedup in `vox-scientia::producers::dedup`, since both currently miss "same finding, restated differently."

### 6. Worthiness signal population

Populate `WorthinessSignalsV2`'s three buckets:
- **Hard gate** (`hg-retraction`): reuses the Crossref lookup from #3 — any claim resting on a retracted source hard-fails regardless of `gate.rs`'s score.
- **Soft gate** (`sg-peer-review`): derived from OpenAlex `venue.type`, populates `WorthinessProfile` (Journal/Preprint/Repository/Social), down-weights preprint-only evidence without blocking it.
- **Diagnostic** (`diag-numeric-recheck`): for claims where `Claim.is_numeric` is true and a p-value/test-statistic triple is extractable from the evidence snippet, run a statcheck-style recomputation and surface a `WorthinessActionItem` for human follow-up rather than a hard block — false-positive rate on this technique is real, so it stays advisory.

### 7. Post-hoc citation audit

After synthesis produces a report with `[N]`-style citation markers, a new audit pass: for each marker, re-fetch the cited source (or use the already-fetched evidence text if still in context) and run an NLI/entailment check confirming the *sentence the marker is attached to* is actually supported by the cited passage — not just that the citation number exists and the URL resolves. Surface a per-claim verification badge (Verified / Unverified / Source-mismatch) in the final report. This directly targets the failure mode the competitive research found universal across every reviewed product (citing a real, resolving source that doesn't actually say what's claimed).

### 8–9. Multi-provider routing fix

Two changes, either sufficient alone to make progress, both needed for full effect:
- **Selection-side:** route `crates/vox-research-shim/src/research/model_select.rs::resolve_stage()` through `vox_orchestrator::models::decide()` instead of the bare `select_with_default_registry()`/`select()` cascade — or, if `decide()`'s call shape doesn't fit the research-stage call site cleanly, port `key_guard::available_inference_providers()` into `select_via_scorer`'s filter closure so a resolved model is never for a provider with no configured key.
- **Egress-side:** extend `crates/vox-actor-runtime/src/llm/cascade.rs::cascade_for_research_stage()` beyond its current two lanes (local + OpenRouter) to dispatch through whichever provider the resolved model actually belongs to, using the `ModelRouteBackend`/`ChatRouteBackend` plumbing the chat path (`model_resolution.rs`) already has — this is reuse, not new infrastructure.

Also fix `registry.rs`'s blanket `CostPreference::Performance ⇒ exclude free models` rule, which currently makes the research intent structurally unable to select a free-tier model regardless of what the user has configured.

### 10. Per-stage free-tier-aware selection

Once #8/#9 land: wire `QualityLevel` (currently dead/unused plumbing in `model_select.rs`) to actually express a per-stage cost preference, so cheap/fast stages (planning, claim extraction) can route to Groq/Cerebras free tiers and quality-sensitive stages (synthesis, judge) route to a stronger free/cheap tier (Gemini Flash, Mistral Experiment) rather than forcing every stage through one lane. This is a config/preference change built on top of #8/#9, not new selection infrastructure.

## Data flow (trust/novelty path)

```
ResearchHit (from retrieval)
  → TrustScorer (Crossref retraction + OpenAlex reputation) → ResearchHit.trust_score
  → gate.rs::score_with_config (trust-weighted citation_score, entropy diversity_score)

Claim (from claim extraction)
  → verifier.rs NLI first pass → escalate low-confidence → LLM cascade → Verdict (SciFact taxonomy)
  → gate.rs claim_support_score

FindingCandidateV1 (pre-publication)
  → prior-art query (OpenAlex/Crossref/SemanticScholar) → MinHash-LSH → embedding cosine
  → NoveltyEvidenceBundle populated
  → WorthinessSignalsV2 hard/soft/diagnostic gates (reusing TrustScorer's retraction/venue data)
  → only findings clearing hard+soft gates are eligible for SCIENTIA publication promotion (future work, not this spec)

Synthesized report (post-synthesis)
  → post-hoc citation audit (re-verify each [N] marker against its cited passage)
  → per-claim verification badge in final output
```

## Testing strategy

Each numbered item gets its own unit/integration tests following existing patterns in the touched files (`gate.rs`, `verifier.rs`, `novelty.rs` all have existing test modules to extend, not replace). Specific attention:
- Trust scorer and novelty prior-art queries hit external APIs (Crossref, OpenAlex, Semantic Scholar) — these need mockable HTTP clients (check for an existing pattern in `vox-search`'s `tavily_research.rs` or `web_dispatcher.rs`) so tests don't require live network access.
- The NLI model integration (#4, #5) needs a small fixture-based test confirming the ONNX model loads and produces sane scores on a known claim/evidence pair, plus explicit tests for the confidence-band escalation logic (>0.99 direct, 0.90–0.99 escalated).
- The multi-provider routing fix (#8/#9) should extend the existing `key_guard.rs`/`decide()` test pattern (keyless-provider-rejected, key-present-selected) into the research-stage call path specifically, and add a cascade-level test confirming a configured non-OpenRouter key actually produces a dispatchable candidate.
- Doc-comment fix (#1) needs no test beyond existing doc-lint/doctest gates passing.

## Deferred (explicitly out of scope for this spec)

See the synthesis doc's P2–P4 for the full list and rationale. Two dependency notes worth restating here: **deep-research-as-skill packaging** and **SCIENTIA auto-publication handoff** are not just lower-priority than this spec's items — they are actively gated on this spec landing first, since packaging a still-stub-heavy pipeline as a more-discoverable skill, or auto-publishing findings before the worthiness gates are real, would both be net-negative moves.
