---
title: "Deep Research Implementation Divergence Audit (2026-08-01)"
description: "Diffs what Stage 1's synthesis doc and two implementation plans promised against what actually shipped. Corrects an earlier claim that the post-hoc citation audit pass was silently dropped — `audit_citations()`/`CitationAuditResult` exists and is wired into `ResearchMetadata`, but as a lighter-weight evidence-span-overlap check rather than the spec's true post-hoc re-fetch. Finds the NLI-first-pass verifier design was substituted with LLM self-consistency resampling, NoveltyEvidenceBundle remains unpopulated despite dedup work landing, and self_consistency is computed but still unconsumed by the confidence gate. Identifies the dominant implementation-time failure pattern: under-specified integration surface (wire X into the real call site), not incorrect algorithms."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative divergence audit between the deep-research Stage 1 plans and shipped code; the pattern finding (under-specified integration surface) should directly shape how the Stage 2 implementation plan is written."
---

# Deep Research Implementation Divergence Audit

**Date:** 2026-08-01 (Stage 2)
**Scope:** Diffs the Stage 1 synthesis doc's P0-P4 gap list and both implementation plans (trust/novelty-core, multi-provider-routing) against the actual current code, not the plan docs' own claims of completion. Companion to [deep-research-synthesis-and-priorities-2026-08-01.md](deep-research-synthesis-and-priorities-2026-08-01.md).

---

## 1. Confirmed-done P0/P1 items

- **P0-1/P0-2** (stale stub comments, unified CRAG/novelty pipelines) — landed and verified in code.
- **P1-3** (trust_score population + gate weighting) — genuinely wired: `TrustScorer` (Crossref + OpenAlex) feeds `gate.rs`'s `trust_weighted_citation_score`, capped at 1.0 per hit with a regression test proving the cap holds.
- **P1-4, partial** (self-consistency resampling) — landed with correct Contested-verdict write-back and nonzero resampling temperature, but **diverged from the original design** (see §2).
- **P1-6** (WorthinessSignalsV2 populated) — `hard_gate_retraction_signal`/`soft_gate_peer_review_signal` implemented and tested.
- **P1-8/9** (multi-provider routing) — `primary_candidate_for_intent()` is real, key-gated, and wired into claim extraction, planning, verification, and stage dispatch.
- **P1-10** (QualityLevel/free-tier) — wired into per-stage axes, opt-in per stage.

## 2. Partially-done or diverged items

- **P1-4 NLI pass — a real divergence, not just partial completion.** The design spec called for a genuinely independent NLI model (ONNX MiniCheck/DeBERTa) as a first pass ahead of the LLM cascade — an architecturally independent signal cutting ~40% of LLM calls. What shipped is LLM self-consistency resampling instead: still fully LLM-mediated, no independent cross-check, no LLM-call reduction. This was a reasonable, disclosed substitution at the time (no ONNX/embedding crate existed in the workspace), but it means P1-4's original value proposition — an independent verification signal — is not actually delivered. `Verdict`'s reconciliation to the SciFact taxonomy also remains unconfirmed.
- **P1-5 (two-stage MinHash-LSH → embedding-cosine novelty dedup)** — only the lexical-similarity stage landed (replacing exact-`finding_id` matching with shingle-based Jaccard similarity). No MinHash-LSH, no embedding-cosine stage, and no evidence `NoveltyEvidenceBundle` itself (the OpenAlex/Crossref/Semantic Scholar prior-art query schema) is populated anywhere — the schema this whole item was meant to feed remains empty. This needs a direct follow-up grep to confirm definitively.
- **Self-consistency in gate scoring is genuinely still unconsumed.** `pipeline.rs`'s "not wired yet" comment for weighting `supported_claim_count` by `self_consistency` is accurate as of current code, not stale. `ClaimVerdict.self_consistency` is computed and stored but the confidence gate doesn't read it. Separately, `orchestrator_policy.rs` has a **different**, pre-existing `self_consistency` field (weight 0.20, `contradiction_hints`-derived) — a naming collision between the new field and an existing one that neither plan flagged as a risk, and that a future implementer could easily conflate.

## 3. Still-untouched P2-P4 items (confirmed against current code)

All ten synthesis-doc P2-P4 items confirmed still untouched: cross-encoder reranking, `synthesis_max_tokens` bump (still hardcoded `1200`), semantic result caching (still FNV1a exact-match), durable async jobs (only fire-and-forget `tokio::spawn` found), per-hop observability, Reflexion-style self-revision, an internal eval harness, deep-research SKILL.md packaging (a same-named file exists but predates this program — June commits — don't double-credit it), SCIENTIA auto-publication handoff (confirmed zero callers, per `worthiness.rs`'s own module doc), and GUI settings-search indexing for API keys.

## 4. Newly-discovered gaps not captured in any prior doc

- **Correction (found during Stage 2 GUI work): the post-hoc citation audit pass (P1-7) was NOT dropped.** An earlier version of this audit claimed `audit_citations()` did not appear in any of the ~40 implementation commits. That claim was wrong: `audit_citations()` in `vox-research-shim`'s `pipeline.rs` (~line 731-770) exists, runs as part of the pipeline, and produces a `CitationAuditResult` (`checked_citations`, `supported_citations`, `unsupported_citation_indices`, `precision`, `supports`) that is wired into `ResearchMetadata.citation_audit` (~line 600). It is, however, a **lighter-weight design** than the original spec: it checks each citation's snippet for quote-overlap against the verifier's already-fetched evidence spans, rather than performing a true post-hoc re-fetch of each cited URL to independently re-verify the quote against the live page. So the signal exists and is consumed (the GUI's Research view now surfaces `citation_precision` from it), but it inherits any staleness/inaccuracy already present in the evidence spans captured earlier in the pipeline, which a true re-fetch would not.
- **`worthiness.rs` is confirmed standalone by its own module doc** ("no production callers yet... deliberate future work"). This means P1-6 ("WorthinessSignalsV2 populated") should not be read as "trust/novelty gating is live in the actual SCIENTIA publication pipeline" — the building blocks exist, tested, but gate nothing yet. Consistent with the synthesis doc's own P4-19 framing, but worth restating plainly since "P1-6 done" is an easy claim to over-read.
- **`NoveltyEvidenceBundle` remains unpopulated** despite the dedup mechanics improving — the schema this item was meant to feed shows no construction sites anywhere near the shipped dedup work.

## 5. Pattern synthesis: what needed correcting during implementation

Reading through the commit history, roughly a third of all commits in the trust/novelty-core and multi-provider-routing plans' execution window are "code review fix" / "code review follow-up" commits. A clear pattern emerges: **the first implementation pass consistently under-scoped "wire X into the real call site" tasks** — not incorrect algorithms, but incomplete integration surface:

- TrustScorer landed at one `ResearchHit` construction site first; a separate follow-up commit was needed to find and wire the second, dominant construction site.
- Key-gating landed on one selection function first; a follow-up had to extend it to sibling functions (`select_via_scorer`, `select_via_premium_alias`) doing the equivalent job.
- Two separate "offline fail-closed contract" gaps surfaced only after initial code shipped — a cross-cutting invariant not enforced uniformly on the first pass.
- The Contested-verdict-not-written-back bug and the hardcoded-temperature-defeats-resampling bug both surfaced only under adversarial code review, not the happy path the plan anticipated.
- A cross-crate bridging gap (`TrustScorer::venue_type` needed to reach `vox-scientia`'s worthiness gates) wasn't anticipated by either plan's task boundaries.
- DOI extraction from hit URLs was needed retroactively — the retraction-check logic shipped initially assuming DOIs would already be present, without accounting for the actual shape of real search-hit URLs.

**The dominant failure mode is under-specified integration surface, not wrong logic.** Nearly every fix-commit is "the logic was applied to only one of N equivalent call sites/structs/functions" or "a cross-cutting invariant was enforced in the new code path but not audited across pre-existing sibling paths needing the same treatment." The synthesis and plan docs were strong on *what* signal to add, weaker on *where all it needs to land*. (P1-7, citation audit, is not an example of this pattern — see the §4 correction above: it landed, just in a lighter-weight form than originally spec'd.)

**Implication for Stage 2's implementation plan:** every task that says "wire X into Y" must include an explicit, grep-verified inventory of every call site/construction site X needs to reach, established during plan-writing (not discovered during code review). Cross-cutting invariants (fail-closed behavior, key-gating, verdict/consistency coherence) should get their own audit task checking every code path that needs the same treatment, rather than being scoped implicitly inside a single feature task.
