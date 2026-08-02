---
title: "Trust, Novelty & Quality-Scoring Landscape for Deep Research (2026-08-01)"
description: "Confirms current stub state of vox-research-shim's gate.rs/verifier.rs and vox-research-events' NoveltyEvidenceBundle/WorthinessSignalsV2 against source, then surveys source-credibility scoring, novelty/dedup detection, claim-verification/hallucination-detection, and bad-research flagging techniques, mapping each to a concrete non-stub replacement and data source."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative gap analysis and technique survey for making Vox's confidence gate, claim verifier, and novelty scoring non-stub; direct input to the deep-research enhancement implementation plan."
---

# Trust, Novelty & Quality-Scoring Landscape for Deep Research

**Date:** 2026-08-01
**Scope:** Confirms current code state of Vox's trust/novelty/quality-scoring stubs, then surveys 2026 techniques for source-credibility scoring, novelty/dedup detection, claim verification, and bad-research flagging, mapping each to Vox's schema. Companion to [deep-research-fundamentals-2026-08-01.md](deep-research-fundamentals-2026-08-01.md) and [deep-research-competitive-landscape-2026-08-01.md](deep-research-competitive-landscape-2026-08-01.md).

---

## 1. Confirmed current-state findings from source

**`crates/vox-research-shim/src/research/gate.rs`** — the module doc-comment is stale relative to the code. It says: "Confidence gate + routing-tier selector. Phase 0a STUB — produces a flat score derived purely from citation count; no claim-level scoring." But `score_with_config` actually already fuses four terms:

```rust
let score = citation_score * 0.35
    + claim_support_score * 0.30
    + diversity_score * 0.20
    + 1.0_f32 * 0.15; // retrieval_score always 1.0 here (guarded above)
```

So it's not *purely* citation count — `claim_support_score` (supported claims / total claims) and `diversity_score` (distinct domains / min_domains) are already wired in. What's real about the "stub" framing: every term is a naive count-based ratio (no trust weighting per source, no contradiction penalty, no retraction awareness), `retrieval_score` is hardcoded to `1.0`, and `routing_tier_for` has its own explicit stub comment: "PHASE_0a_STUB: exact-zero check is valid only while `score_with_config` produces an integer-derived float... Phase 2 multi-signal fusion may produce non-zero scores with no retrieval hits; replace with `input.no_retrieval_hits` passed through the call chain."

**`verifier.rs`** is further along than commonly described, but conditionally: behind the `runtime` feature, `verify_claims_with_config` runs a real per-claim LLM cascade (`chat_with_cascade`) with a JSON-schema-constrained prompt, parsing `{"verdict", "confidence", "supporting_indices", "contradicting_indices"}` into `ClaimVerdict`. Without the `runtime` feature (the actual "stub" path), it maps every claim to `Unverified`. Its doc comment flags the taxonomy gap explicitly: "the SCIENTIA plan... specifies the canonical SciFact labels: `Support`, `Contradict`, `NotEnoughInfo`, `Abstain`. The variants here (`Supported`, `Contradicted`, `Contested`, `Unverified`) match the pre-existing consumer... Phase 1's `vox-claim-extractor` integration is the right point to reconcile to the SciFact taxonomy." It also names its intended replacement: "Phase 1 wires this to `vox-claim-extractor`'s MiniCheck-backed verifier" — a specific NLI-model target already chosen, not just "some LLM."

**`schema_types.rs`** confirms the schema is fully fleshed out and unused-in-anger: `NoveltyEvidenceBundle` has `normalized_hits: Vec<NormalizedHit>` with `lexical_score`, `semantic_score`, `cited_by_count`, an `OverlapSummary` with `max_lexical_score`/`max_semantic_score`/`recency_bucket`, and a `NoveltySource` enum listing `Openalex | Crossref | SemanticScholar | Manual | Other` — the schema already anticipates exactly the APIs this research recommends. `WorthinessSignalsV2` similarly has `hard_gate`/`soft_gate`/`diagnostic` vectors of `WorthinessSignalItem{id, passed, score, reason_code, details}` plus a `WorthinessProfile` enum (`Journal | Preprint | Repository | Social`) — a ready-made hook for peer-review-status gating.

The "URL-level dedup only" claim is confirmed by source: `vox-search/src/research.rs` documents itself as "deduping URLs and expanding queries via `CragRouter`," and `vox-scientia/src/producers/dedup.rs` collapses `FindingCandidateProposed` events purely on an exact `finding_id` string match (a deterministic content fingerprint) — neither does embedding or lexical-similarity comparison. `ResearchHit` in `types.rs` also carries a `trust_score: f32` field that in the verifier's own tests is hardcoded to `1.0` for every hit — a second unused wire point.

## 2. Source credibility / trustworthiness scoring

Most 2026 web content on "LLM trust signals" is SEO/GEO marketing noise about getting *your* content cited by ChatGPT — not useful here. The academically grounded thread is citation-graph trust propagation. Foundational work applies PageRank to citation networks to separate *prestige* from *popularity*: "PageRank reflects the prestige of the paper while the citation count reflects its popularity... citation counts are easily distorted by self-citation" ([Measuring academic reputation through citation networks via PageRank, arXiv:1803.09104](https://arxiv.org/abs/1803.09104)). More directly relevant: "Trust-Aware Citation Cartel Ranking in Scholarly Knowledge Graphs" ([arXiv:2607.06528](https://arxiv.org/html/2607.06528v1)) combines LLM-supervised citation-intent typing with SciBERT inference and weighted PageRank to build a "trust-weighted graph" privileging citations encoding real method reuse/evidential support and discounting "ceremonial" citations — essentially a fraud-ring/citation-cartel detector, mapping well onto detecting mutually-reinforcing low-quality sources in a research corpus.

For retraction cross-checking: Crossref acquired the Retraction Watch database in 2023 and now serves it through the ordinary Crossref REST API — querying `api.crossref.org/works/{DOI}` and inspecting `update-to`/`relation` fields surfaces retraction/correction notices, updated daily ([Crossref blog](https://www.crossref.org/blog/retraction-watch-retractions-now-in-the-crossref-api/)). This is a free, no-auth, machine-queryable API, directly usable from Rust with a plain HTTP client.

For author/venue reputation, OpenAlex is the strongest fit: a free REST API over 250M works, 90M authors, ~124,000 venues, with author-level h-index, cited-by-count, works-count, and a venue `type` field distinguishing journal/conference/preprint-repository/institutional-repository ([arXiv:2205.01833](https://arxiv.org/pdf/2205.01833)). Semantic Scholar's API offers similar author h-index/citation data as a fallback. Predatory-venue screening has no single authoritative free API; the standard workflow cross-checks DOAJ, Cabells Predatory Reports, and Retraction Watch's Hijacked-Journal Checker for red flags (guaranteed rapid acceptance, vague APCs, fabricated editorial boards) ([GWU guide](https://guides.himmelfarb.gwu.edu/PredatoryPublishing/RedFlags)).

**Wiring:** `ResearchHit.trust_score` is the natural sink — a `TrustScorer` resolves each hit's URL to a DOI where possible, queries Crossref for retraction status (hard-zero on retraction), queries OpenAlex for venue type and author h-index (soft weight), falls back to a domain-reputation heuristic table for non-academic web sources. `gate.rs::score_with_config` then stops treating citations as flat counts: `citation_score` becomes `sum(trust_score)` instead of `citation_count`, `diversity_score` becomes an entropy measure over (domain, trust-tier) pairs rather than raw distinct-domain count — directly replacing the "flat function of citation count" the doc-comment still describes.

## 3. Novelty / redundancy detection

Two complementary techniques are standard. **MinHash + LSH** operates over token shingles, gives sub-linear approximate near-duplicate detection at scale, with collision probability equal to Jaccard similarity — cheap and effective for mirrored/near-identical scraped text, but "largely lexical-overlap driven... prompts that share templates may be removed even when their semantics differ" ([Milvus blog](https://milvus.io/blog/minhash-lsh-in-milvus-the-secret-weapon-for-fighting-duplicates-in-llm-training-data.md)). **Embedding-based semantic dedup** is the harder, complementary layer — "semantic deduplication... is a separate and harder problem, typically approached using embedding-based similarity methods rather than MinHash," commonly a persistent embedding index queried via FAISS-style ANN with a cosine-similarity threshold ([Zilliz blog](https://zilliz.com/blog/data-deduplication-at-trillion-scale-solve-the-biggest-bottleneck-of-llm-training)).

For a Rust backend specifically: `hnswlib-rs` (HNSW ANN index, L2/Cosine/InnerProduct), `fastembed` (ONNX-backed embedding inference, including Qwen3 embedding models via candle), `embed_anything` (multi-modal ONNX/dense/sparse embedding pipeline), `ruvector-onnx-embeddings` (HNSW + ONNX embeddings combined). These let novelty scoring run fully in-process without a Python sidecar.

**Wiring — the single biggest gap-to-schema mismatch found.** `NoveltyEvidenceBundle.normalized_hits[].lexical_score`/`semantic_score` and `OverlapSummary.max_lexical_score`/`max_semantic_score`/`recency_bucket` are declared and round-trip-tested but nothing populates them from real data. Concretely: for each `FindingCandidateV1`, query OpenAlex/Crossref/Semantic Scholar (the exact `NoveltySource` variants already in the enum) for prior-art works matching the candidate's `title_hint`/claim text, compute `lexical_score` as shingle-Jaccard/BM25 against each hit's abstract and `semantic_score` as cosine similarity between the candidate's embedding (via `fastembed`) and each hit's embedding, indexed with `hnswlib-rs` against a persistent local corpus of prior `FindingCandidateV1` records. A two-stage pipeline — cheap MinHash-LSH first pass to catch near-identical mirrors/republished content, embedding cosine-similarity second pass against the historical corpus for genuine semantic overlap — replaces the current URL-string dedup in `vox-search::research::run_multi_hop_web_research` and the finding_id-exact-match collapse in `vox-scientia::producers::dedup::dedup_finding_candidates`, neither of which catches "this is the same finding restated differently."

## 4. Claim verification / hallucination detection

The dominant automated pipeline decomposes an answer into atomic claims and scores each against source text with an NLI entailment model — "claim decomposition combined with NLI entailment is the core automated pipeline for hallucination detection" ([Michael Brenndoerfer](https://mbrenndoerfer.com/writing/hallucination-detection)). Fine-tuned NLI models such as `MoritzLaurer/DeBERTa-v3-large-mnli-fever-anli-ling-wanli` reach 98.47% accuracy on FEVER and 90.09% on SciFact-Open when confidence exceeds 0.99, but drop sharply (to ~68% on FEVER) in the 0.90–0.95 confidence band — a pure-NLI gate needs an abstain/escalate path, not a single threshold ([SciTePress](https://www.scitepress.org/Papers/2024/129000/129000.pdf)). A hybrid NLI-then-LLM pipeline slightly outperforms pure-LLM baselines on both FEVER (0.9402 vs 0.9333) and SciFact (0.8623 vs 0.8587) while resolving ~40% of claims via NLI alone, cutting expensive LLM calls by ~40%.

Independent of retrieval-grounded verification, **self-consistency methods** (SelfCheckGPT) detect hallucination without external knowledge: sample the model N times at nonzero temperature, measure inter-sample contradiction via BERTScore/QA-consistency/n-gram/NLI/prompt-based scoring, treat high variance as a hallucination signal — reported AUC-PR above 93% at sentence level, black-box (no logits needed) ([SelfCheckGPT overview](https://www.emergentmind.com/topics/self-consistency-based-hallucination-detection)). Most directly on-point for "does the cited source actually say this": "Detecting and Correcting Reference Hallucinations in Commercial LLMs and Deep Research Agents" ([arXiv:2604.03173](https://arxiv.org/html/2604.03173v1)) studies exactly the failure mode where an agent cites a real source that doesn't actually support the claim attributed to it.

**Wiring:** `verifier.rs`'s `runtime`-feature LLM cascade is a reasonable Phase-1 seed but has no non-LLM sanity check and, without `runtime`, degrades to blanket `Unverified`. Two upgrades map directly onto the file's own doc-comment TODOs: (1) add the NLI first pass the comment already promises ("wires this to `vox-claim-extractor`'s MiniCheck-backed verifier") ahead of the LLM cascade — run a small ONNX NLI model (`fastembed`/`ort` in-process) per claim-evidence pair, escalating only low-confidence (0.90–0.99 band) results to the existing LLM cascade, cutting cost and adding a signal independent of the LLM prompt/JSON-parsing path that is the only signal today; (2) reconcile `Verdict` to the SciFact taxonomy (`Support`/`Contradict`/`NotEnoughInfo`/`Abstain`) the doc-comment flags as owed. A SelfCheckGPT-style addition — sample the verifier cascade N times at temperature >0, compute agreement rate — would give `gate.rs`'s `claim_support_score` a second, cheaper-to-trust input distinguishing "the LLM said supported" from "the LLM is consistent when re-asked," which the current single-shot JSON call cannot distinguish.

## 5. "Bad research" / low-quality-research flagging

Automated statistical-inconsistency detection is a mature, narrow technique: **statcheck** independently recomputes the p-value implied by a reported test statistic and degrees of freedom, flagging mismatches large enough to change a significance conclusion ("gross inconsistency"); a 2016 screen of ~30,000 psychology-journal articles found roughly half had *some* reporting inconsistency, about one in eight a gross one ([statcheck PMC](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7540394/)). **GRIM** complements this by checking whether reported means/statistics are mathematically possible given the stated sample size and measurement scale. Both are heuristic red-flag generators, not proof of misconduct — false positives are common, which matters for how hard a gate should treat them.

Predatory-journal and peer-review-status detection has no clean API; the practitioner workflow cross-references DOAJ/Cabells/Retraction-Watch's hijacked-journal checker against manual red flags — realistically a "flag for review" signal, not an automatable hard gate.

**Wiring:** `WorthinessSignalsV2` already models this taxonomy precisely (`hard_gate`/`soft_gate`/`diagnostic` `WorthinessSignalItem{passed, score, reason_code}`) but nothing populates it today. Concretely: a `hard_gate` item (e.g. `hg-retraction`) built from the same Crossref/Retraction-Watch lookup as §2 — any `Supported` claim resting on a retracted source should hard-fail regardless of `gate.rs`'s confidence score; a `soft_gate` item (`sg-peer-review`) derived from OpenAlex `venue.type`, populating `WorthinessProfile` and down-weighting preprint-only evidence without blocking it; a `diagnostic` item for `Claim.is_numeric` claims (the field already exists on `Claim`) that runs a statcheck-style recomputation when a p-value/test-statistic triple is extractable from the evidence snippet, surfacing a `WorthinessActionItem` for follow-up rather than a hard block, given the technique's known false-positive rate.

## Synthesis: what changes where

| Stub | Concrete non-stub replacement | Data source |
|---|---|---|
| `gate.rs` citation/diversity terms | Trust-weighted sum via populated `ResearchHit.trust_score` | Crossref retraction check + OpenAlex venue/author reputation |
| `verifier.rs` LLM-only cascade | NLI first pass, LLM escalation only on low confidence; SelfCheckGPT-style resampling for a consistency signal | ONNX NLI model (MiniCheck/DeBERTa-mnli-fever-anli), existing LLM cascade |
| `NoveltyEvidenceBundle` (unpopulated schema) | Two-stage MinHash-LSH + embedding-cosine novelty scoring against `FindingCandidateV1` corpus | OpenAlex/Crossref/Semantic Scholar prior-art queries + `fastembed`/`hnswlib-rs` |
| URL-only / finding_id-only dedup | Semantic redundancy detection replacing/augmenting both dedup sites | Same embedding stack as above |
| `WorthinessSignalsV2` (unpopulated schema) | Hard-gate retraction check, soft-gate peer-review-status, diagnostic statcheck-style numeric-claim screen | Crossref, OpenAlex, in-house statcheck/GRIM port |

## Sources

- [Trust-Aware Citation Cartel Ranking in Scholarly Knowledge Graphs (arXiv:2607.06528)](https://arxiv.org/html/2607.06528v1)
- [Measuring the academic reputation through citation networks via PageRank (arXiv:1803.09104)](https://arxiv.org/abs/1803.09104)
- [Retraction Watch retractions now in the Crossref API — Crossref blog](https://www.crossref.org/blog/retraction-watch-retractions-now-in-the-crossref-api/)
- [Retraction Watch Database User Guide](https://retractionwatch.com/retraction-watch-database-user-guide/)
- [OpenAlex: A fully-open index of scholarly works, authors, venues, institutions, and concepts (arXiv:2205.01833)](https://arxiv.org/pdf/2205.01833)
- [OpenAlex API: Query 250M Academic Works, Authors, and Institutions](https://anysite.io/blog/openalex-api-launch/)
- [Red Flags — Predatory & Problematic Publishing, GWU Research Guides](https://guides.himmelfarb.gwu.edu/PredatoryPublishing/RedFlags)
- [Retractions and Predatory Journals: A Growing Crisis — Enago](https://www.enago.com/academy/retractions-predatory-journals-crisis/)
- [MinHash LSH in Milvus: The Secret Weapon for Fighting Duplicates in LLM Training Data](https://milvus.io/blog/minhash-lsh-in-milvus-the-secret-weapon-for-fighting-duplicates-in-llm-training-data.md)
- [Data Deduplication at Trillion Scale — Zilliz blog](https://zilliz.com/blog/data-deduplication-at-trillion-scale-solve-the-biggest-bottleneck-of-llm-training)
- [Rust crates for embeddings/vector search — crates.io](https://crates.io/crates/fastembed)
- [Hallucination Detection: NLI, Self-Consistency & Learned Models](https://mbrenndoerfer.com/writing/hallucination-detection)
- [Scientific Claim Verification with Fine-Tuned NLI Models — SciTePress](https://www.scitepress.org/Papers/2024/129000/129000.pdf)
- [Self-Consistency Hallucination Detection (SelfCheckGPT) — Emergent Mind](https://www.emergentmind.com/topics/self-consistency-based-hallucination-detection)
- [Detecting and Correcting Reference Hallucinations in Commercial LLMs and Deep Research Agents (arXiv:2604.03173)](https://arxiv.org/html/2604.03173v1)
- ["statcheck": Automatically detect statistical reporting inconsistencies — PMC](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7540394/)
- [GRIM, SPRITE, statcheck: Fraud Detection — CASRAI](https://casrai.org/guides/statistical-fraud-detection-grim-sprite-statcheck)
