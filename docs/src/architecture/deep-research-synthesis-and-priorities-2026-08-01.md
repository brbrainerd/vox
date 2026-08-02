---
title: "Deep Research Enhancement Program — Synthesis & Priorities (2026-08-01)"
description: "Synthesizes the four 2026-08-01 deep-research research docs (fundamentals, competitive landscape, trust/novelty scoring, multi-provider/skills/publication) plus the verification pass into one impact-ranked gap list, the recommended differentiation strategy, and the handoff point to an implementation plan."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative synthesis and prioritization for the deep-research enhancement program; the direct input to its implementation plan."
---

# Deep Research Enhancement Program — Synthesis & Priorities

**Date:** 2026-08-01
**Inputs:** [deep-research-verification-2026-08-01.md](deep-research-verification-2026-08-01.md), [deep-research-fundamentals-2026-08-01.md](deep-research-fundamentals-2026-08-01.md), [deep-research-competitive-landscape-2026-08-01.md](deep-research-competitive-landscape-2026-08-01.md), [deep-research-trust-novelty-scoring-landscape-2026-08-01.md](deep-research-trust-novelty-scoring-landscape-2026-08-01.md), [deep-research-model-agnostic-multi-provider-and-skills-publication-2026-08-01.md](deep-research-model-agnostic-multi-provider-and-skills-publication-2026-08-01.md). Supersedes the priority ordering (not the technical content) of [deep-research-capabilities-audit-2026-06-17.md](deep-research-capabilities-audit-2026-06-17.md).

---

## Strategic framing

The competitive research is unambiguous: **every major deep-research product (Google, Anthropic, OpenAI, Perplexity, You.com/Grok) already claims breadth, iteration, and speed — that axis is saturated.** The one gap none of them has convincingly closed is **verifiable trust infrastructure**: citation-to-claim auditability, reproducible run traces, honest confidence signaling, and novelty/quality scoring users can actually check. Vox's own pipeline independently corroborates this from the inside — `gate.rs` is a real multi-signal fusion but has no trust weighting; `NoveltyEvidenceBundle`/`WorthinessSignalsV2` are fully-specified schemas with nothing populating them; citation grounding exists structurally but has no post-hoc verification pass. **This convergence — what the market lacks and what Vox's own schema already anticipates but hasn't wired — is the strategic bet: differentiate on trust/novelty/verification rigor, not on out-executing breadth or speed.**

## Prioritized gap list (impact × effort)

### P0 — Near-zero effort, do immediately
1. **Fix stale `PHASE_0a_STUB` doc comments** in `crates/vox-research-shim/src/research/mod.rs` and `verifier.rs` — they falsely claim stub/empty behavior over code that is fully real, actively misdirecting anyone (human or agent) who greps for stub markers. *(Verification doc, New Finding #1)*
2. **Unify the G1/G3 pipeline split** — LLM-driven CRAG expansion (`web_gather.rs`) and novelty scoring (`vox-search::research.rs`) each landed in only one of two independent research loops (the primary `vox-research-shim` pipeline vs. the autonomous-agent `run_multi_hop_web_research` loop). Both fixes already exist; share them so neither loop is missing a capability the other already has. *(Verification doc, New Finding #3)*

### P1 — High impact, medium effort — the trust/novelty core (directly answers the user's original ask)
3. **Populate `ResearchHit.trust_score`** via Crossref retraction lookup (hard-zero) + OpenAlex venue-type/author-h-index (soft weight); feed it into `gate.rs::score_with_config` so `citation_score` becomes trust-weighted, not a flat count. *(Trust/novelty doc §2)*
4. **Add an NLI first pass to `verifier.rs`** (ONNX MiniCheck/DeBERTa-mnli-fever-anli, in-process via `fastembed`/`ort`) ahead of the existing LLM cascade, escalating only low-confidence claims — cuts LLM calls ~40% and adds a signal independent of the current single-shot JSON path. Reconcile `Verdict` to the SciFact taxonomy the code already says is owed. *(Trust/novelty doc §4)*
5. **Populate `NoveltyEvidenceBundle`** via OpenAlex/Crossref/Semantic Scholar prior-art queries + a two-stage MinHash-LSH → embedding-cosine (`fastembed`/`hnswlib-rs`) pipeline, replacing the URL-only and finding_id-only dedup at both existing dedup sites. This is Vox's own most conspicuous schema-without-implementation gap. *(Trust/novelty doc §3)*
6. **Populate `WorthinessSignalsV2`** hard/soft/diagnostic gates (retraction hard-gate, peer-review-status soft-gate, statcheck-style numeric-claim diagnostic) — the schema is already the right shape (reason-coded, auditable), just unfed. *(Trust/novelty doc §5)*
7. **Add a post-hoc citation audit pass**: after synthesis, re-fetch every cited source and confirm the quoted claim actually appears in it, surfacing a per-claim verification badge. This is the single most-requested-by-implication fix from the competitive research — no reviewed competitor does this rigorously at open-web scale. *(Competitive-landscape doc, "What would beat all of these" #1; Fundamentals doc §7)*

### P1 — High impact, medium effort — multi-provider routing (unlocks the GUI keys already collectable)
8. **Route research stage selection through `decide()`** (or port `key_guard::available_inference_providers()` into the scorer's filter) so a resolved model always has a configured key. *(Multi-provider doc, Part A2)*
9. **Make `cascade_for_research_stage` provider-aware** beyond its current two lanes (local + OpenRouter) — dispatch to whichever of the 9 already-collectable provider keys actually owns the resolved model, using the `ModelRouteBackend`/`ChatRouteBackend` plumbing the chat path already has. *(Multi-provider doc, Part A2)* This is the prerequisite for #10.
10. **Per-stage free-tier-aware selection**: once #8/#9 land, route cheap/fast stages (planning, claim extraction) to Groq/Cerebras free tiers and stronger stages (synthesis, judge) to Gemini Flash/Mistral Experiment rather than a rate-starved single lane. Also fix the scorer's blanket `Performance ⇒ exclude free models` rule so `QualityLevel` (currently dead plumbing) can actually express a preference. *(Multi-provider doc, Part A3, Part B)*

### P2 — Well-scoped, moderate effort
11. **Cross-encoder reranking** — no implementation exists; published benchmarks show ~13pp recall@10 gain from adding this over RRF-only fusion. *(Fundamentals doc §6, Verification doc G2)*
12. **Bump/parameterize `synthesis_max_tokens`** — currently defaults to 1200, actually *lower* than the 1800 the original audit flagged as too small. Cheap fix, direct quality impact on report length. *(Verification doc G6)*
13. **Semantic result caching** — replace the exact-match FNV1a cache key with embedding-similarity lookup. *(Verification doc G7)*

### P3 — Longer-horizon / infrastructure
14. **Durable async research jobs** — current `tokio::spawn` fire-and-forget loses in-flight runs on process restart; needs `vox-orchestrator-queue` oplog wiring. *(Verification doc G8)*
15. **Per-hop span observability** — structured telemetry, not just log lines. *(Verification doc G9)*
16. **Reflexion-style iterative self-revision** — current self-check (`run_self_verification`) is single-pass CoVE-style, not iterate-until-satisfied. *(Fundamentals doc §3)*
17. **Internal evaluation harness** — a small BrowseComp/DeepResearch-Bench-style rubric set for the *composed* pipeline; currently nothing validates end-to-end report quality, only unit-level correctness. *(Fundamentals doc §9)*

### P4 — Deferred until P1 lands (explicitly gated, not just deprioritized)
18. **Deep research as an ad-hoc `SKILL.md`** — packaging a still-stub-heavy pipeline as a more-discoverable skill just exposes the stubs to more callers; do after the P1 trust/novelty work closes. *(Multi-provider doc, Part C)*
19. **SCIENTIA/VoxGiantia auto-publication handoff** — should trigger only on findings that clear the (currently unpopulated) `WorthinessSignalsV2` gates from #6; auto-publishing before that lands would be strictly worse than the current human-gated status quo. *(Multi-provider doc, Part D)*
20. **GUI settings-search indexing for API keys** — small, independent fix (`federatedSearchIndex.ts` doesn't index the Keys & Secrets page); not blocking, can land anytime. *(Multi-provider doc, Part A1)*

## Recommended next step

This synthesis, together with the four research docs and the verification pass, is the complete input for an implementation plan. Per the brainstorming skill's own terminal-state rule, the next step is **not** further research — it's translating items #1–#10 (P0/P1) into a concrete implementation plan via the writing-plans skill, since those ten items are where impact and evidence both concentrate: they're simultaneously what the user asked to prioritize (trust/novelty/bad-research surfacing, multi-provider intelligence) and what the competitive research independently confirms is the open market gap.
