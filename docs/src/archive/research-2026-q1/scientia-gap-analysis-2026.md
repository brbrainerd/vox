---
title: "Vox Scientia Gap Analysis (April 2026)"
description: "Comprehensive audit of gaps, bugs, and structural limitations in the Vox Scientia automatic publication model — inbound discovery, outbound pipeline, RAG loop, SSOT convergence, and autonomy boundaries."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Synthesizes architecture gaps and solutions across the full Scientia research lifecycle."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox Scientia Gap Analysis (April 2026)

> [!IMPORTANT]
> This document is a research artifact written to `docs/src/architecture/scientia-gap-analysis-2026.md` per the project's AGENTS.md policy. It identifies **45 concrete problems** across all stages of the Scientia lifecycle with proposed solutions and a recommended execution wave order.

---

## Dimension 1 — Inbound Research Discovery

### Problem 1: The "inbound" pipeline exists only in a research doc

**Status:** `scientia-external-discovery-research-2026.md` describes a Collector → Evaluator → Synthesizer multi-agent inbound stack, but **no crate, no schema, no CLI command, and no DB table** has been created for it.

**Impact:** Scientia is entirely outbound. It can package discoveries but cannot autonomously surface new ones from external literature. Without the inbound stack, "making discoveries externally" requires fully manual effort.

**Solution:** Implement the inbound pipeline in three slices:
1. Add `crates/vox-scientia-ingest/` as a new crate with `InboundItem`, `FeedSource`, and `IngestSession` structs.
2. Add `scientia_external_intelligence` DB table under `publish_cloud`.
3. Expose `vox scientia ingest-feeds` CLI and `vox_scientia_ingest_feeds` MCP tool.

**Owner crates:** `vox-scientia-ingest` (new), `vox-db`, `vox-cli`, `vox-mcp` | **Severity: Critical** | **Effort: Large**

archived_date: 2026-04-18
---

### Problem 2: No RSS/Atom feed parsing crate is wired

**Status:** The research doc recommends `feed-rs`, but there is no `Cargo.toml` dependency and no source code consuming feeds.

**Solution:**
- Add `feed-rs = "1.3"` dependency.
- Implement `FeedCrawler::crawl_all(sources: &[FeedSource]) -> Vec<InboundItem>`.
- Persist source registry in `scientia_feed_sources` table keyed by URL + `last_crawled_at_ms`.

**Severity: High** | **Effort: Small**

---

### Problem 3: No Reddit/HN inbound read path exists (only outbound)

**Status:** `vox-publisher/src/adapters/reddit.rs` handles outbound submission. The research doc proposes inverting this for read-only monitoring, but no implementation exists.

**Solution:**
- Add `RedditInboundClient` behind `scientia-inbound-reddit` feature flag.
- Use existing `refresh_access_token` machinery (read-only scope).
- Gate on `VOX_SCIENTIA_REDDIT_INBOUND=1` via Clavis.

**Severity: Medium** | **Effort: Medium**

archived_date: 2026-04-18
---

### Problem 4: No Socrates inbound policy profile — only outbound preflight profiles

**Status:** `PreflightProfile` variants (`DoubleBlind`, `MetadataComplete`, `ArxivAssist`) evaluate **outgoing** manifests. The research doc specifies a `NewsInbound` profile that doesn't exist in `publication_preflight.rs`.

**Impact:** Any inbound external article would bypass the quality gate entirely. Noise and "slop" would enter the discovery corpus unchecked.

**Solution:**
- Add `PreflightProfile::NewsInbound` variant checking: `requires_code_repo_link`, `requires_reproducible_benchmark`, `maximum_opinion_ratio`.
- Apply `ComplexityJudge` from `vox-socrates-policy` on inbound article text.
- High-contradiction items go to `Quarantine` state in `scientia_external_intelligence.status`.

**Owner:** `vox-publisher`, `vox-socrates-policy` | **Severity: Critical** | **Effort: Medium**

---

### Problem 5: No semantic deduplication before inbound insert

**Status:** `memory_hybrid.rs` does BM25 + vector retrieval, but there is no pre-insert duplicate-detection call for the inbound pipeline. The research doc specifies a `similarity > 0.9` guard that is unimplemented.

**Impact:** The same arXiv preprint reported by multiple sources will be inserted three times, bloating the corpus with redundant signal.

**Solution:**
- Add `IngestDeduplicator::is_duplicate(embedding: &[f32], threshold: f64) -> bool` querying the SQLite embeddings table before insert.
- On duplicate, append the source URL to the existing document's `provenance_json`.
- Threshold pinned in `scientia_heuristics.rs` (not a magic constant).

**Severity: Medium** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 6: No `scientia_external_intelligence` DB table or migration

**Status:** The research doc identifies this table but it does not exist in `publish_cloud.rs`.

**Solution:** Add additive migration:
```sql
CREATE TABLE IF NOT EXISTS scientia_external_intelligence (
  id TEXT PRIMARY KEY,
  source_url TEXT NOT NULL,
  source_kind TEXT NOT NULL,  -- 'rss', 'reddit', 'hn', 'arxiv'
  title TEXT NOT NULL,
  abstract_text TEXT,
  embedding_id TEXT,
  provenance_json TEXT DEFAULT '[]',
  ingest_status TEXT NOT NULL DEFAULT 'pending',
  preflight_score REAL,
  ingested_at_ms INTEGER NOT NULL,
  reviewed_at_ms INTEGER
);
```

**Owner:** `vox-db` | **Severity: Critical** | **Effort: Small**

---

### Problem 7: Inbound Scholarly Digest has no synthesis loop contract

**Status:** The research doc specifies a Collector → Evaluator → Synthesizer multi-agent flow, but the Synthesizer has no design contract in code or contracts directory.

**Solution:**
- Add `contracts/scientia/scholarly-digest.v1.schema.json` specifying the digest output structure (cluster, delta summary, impact assessment).
- Add `vox scientia digest-generate` CLI to drive the A2A multi-agent synthesis flow.
- Use `Tier 1` (local model) for initial categorization; escalate `ComplexityBand::Complex` to `Tier 2`.

**Severity: High** | **Effort: Medium**

archived_date: 2026-04-18
---

### Problem 8: No persistent registry of external intelligence sources

**Status:** Feed URLs have no registry table. Sources would be hardcoded or passed per-invocation.

**Solution:**
- Add `scientia_feed_sources` table: `(id, url, source_kind, crawl_interval_ms, enabled, last_crawled_at_ms, last_error)`.
- Add `vox scientia feed-source-add` / `feed-source-list` / `feed-source-disable` commands.

**Severity: Medium** | **Effort: Small**

---

## Dimension 2 — RAG-to-Scientia Feedback Loop

### Problem 9: Scientia publications never re-enter the search corpora

**Status:** After a successful publication, the manifest and evidence pack are stored in `publish_cloud` tables but are **never indexed into `vox-search` corpora**.

**Impact:** The system cannot search its own published discoveries. This is a fundamental closed-loop failure.

**Solution:**
- Add `PostPublishIndexer` step in `postPublishAudit`.
- On `publication_status = 'published'`, embed manifest title + abstract + evidence metadata into `DocumentChunks` corpus with `source_kind = 'scientia_publication'`.
- Tag chunk with manifest digest for retrieval attribution.

**Owner:** `vox-publisher`, `vox-search` | **Severity: Critical** | **Effort: Medium**

archived_date: 2026-04-18
---

### Problem 10: Evidence packs are not linked into the knowledge graph

**Status:** `metadata_json.scientia_evidence` is stored per-manifest but never inserted into the `KnowledgeGraph` SQLite tables.

**Impact:** Multi-hop queries like "what findings relate to our GRPO reward shaping work?" cannot traverse from publication to its evidence chain.

**Solution:**
- Add `EvidencePackKGIndexer` inserting typed nodes and edges:
  - Node: `Publication(id, title, pub_date)`
  - Node: `BenchmarkRun(run_id, result_summary)`
  - Edge: `has_evidence(publication_id → benchmark_run_id)`
  - Edge: `cites_doc(publication_id → doc_path)`

**Severity: Medium** | **Effort: Medium**

---

### Problem 11: Socrates Abstain events are not persisted for analysis or training

**Status:** The RAG SSOT §8 explicitly identifies "Hallucination events → Not persisted" as a gap.

**Impact:** We cannot detect patterns in what Scientia fails to answer. `min_training_pair_confidence = 0.75` floor is defined but high-confidence Abstain events are lost.

**Solution:**
- Add `socrates_abstain_events` Arca table: `(id, query_hash, confidence, contradiction_ratio, risk_decision, suggested_query, timestamp)`.
- Persist on every `Abstain` outcome from the research path.
- Include abstain rate and top abstain queries in `vox telemetry search-quality-report`.

**Owner:** `vox-db`, `vox-socrates-policy` | **Severity: High** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 12: CRAG loop fires and fetches web evidence that is never persisted

**Status:** The CRAG loop in `bundle.rs` fetches Tavily results and re-runs RRF fusion. However, there is no mechanism to persist the corrected retrieval result.

**Impact:** The same low-quality query will trigger Tavily again on the next execution — burning credits and adding latency — because the new evidence was never stored.

**Solution:**
- After CRAG correction (evidence_quality improved above threshold), store Tavily-retrieved content into `DocumentChunks` corpus with `source_kind = 'crag_web_result'` and a 7-day TTL.

**Severity: High** | **Effort: Small**

---

### Problem 13: No awareness of in-progress Scientia findings in the RAG pipeline

**Status:** When an agent query matches a topic that Scientia has already identified as a `StrongCandidate` discovery, the RAG pipeline has no way to surface this.

**Solution:**
- Add `FindingsDraftCorpus` as a new optional `SearchCorpus` variant backed by `publication_manifests` where `status = 'draft' AND discovery_tier = 'strong_candidate'`.
- Activate when `SearchIntent::Research` and query relevance exceeds threshold.
- Gate with `VOX_SEARCH_FINDINGS_DRAFT=1`.

**Severity: Medium** | **Effort: Medium**

archived_date: 2026-04-18
---

## Dimension 3 — Internal Scientific Discovery Mechanisms

### Problem 14: Discovery ranking constants are hardcoded in Rust

**Status:** `scientia_discovery.rs` calls `ScientiaHeuristics::default()` with embedded numeric constants. The impact-readership research doc explicitly identifies this as architectural debt.

**Impact:** Tuning discovery sensitivity requires a code change and recompile.

**Solution:**
- Load heuristics from `contracts/scientia/scientia-discovery-heuristics.v1.yaml`.
- Implement `ScientiaHeuristics::from_yaml(path: &Path) -> Result<Self>`.

**Owner:** `vox-publisher`, `vox-scientia-core` | **Severity: High** | **Effort: Small**

---

### Problem 15: Signal catalog (`discovery_signals`) has no formal schema contract

**Status:** Signal codes like `eval_gate_passed`, `human_advance_attested` are string literals without a machine-checkable registry.

**Impact:** A typo in a signal code silently produces an `Informational` signal instead of `Strong`.

**Solution:**
- Add `contracts/scientia/discovery-signal-codes.v1.yaml` enumerating all valid codes with their strength level.
- Add `vox ci scientia-signal-codes` CI check.
- Consider `SignalCode` enum generated from the YAML at build time.

**Severity: Medium** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 16: No multi-hop hypothesis chain generation

**Status:** `scientia_prior_art.rs` checks overlap and `scientia_finding_ledger.rs` scores novelty, but there is **no mechanism to chain multiple findings into a composite hypothesis**.

**Solution:**
- Design `HypothesisChainBuilder` in `vox-scientia-core`:
  1. Fetch `StrongCandidate` manifests.
  2. Query KnowledgeGraph for shared evidence nodes.
  3. Use MENS Lane G or Tier 2 model to propose hypothesis chains.
  4. Return `HypothesisCandidate` structs with attribution map.
- Add `vox scientia hypothesis-scan` CLI.
- Gate as `human_approval_required = true` per automation boundary matrix.

**Severity: High** | **Effort: Large**

---

### Problem 17: No experimental design scaffolding

**Status:** Once a hypothesis is identified, there is no tooling to scaffold a research experiment (define metrics, set baseline run, configure eval gate).

**Solution:**
- Add `vox scientia experiment-scaffold --hypothesis-id <id>` which:
  1. Creates a draft manifest pre-filled with the hypothesis.
  2. Emits a `scientia_evidence` template with placeholder eval gate and benchmark block.
  3. Generates a checklist of evidence needed to reach `AutoDraftEligible`.
- All generated content marked `machine_suggested = true`.

**Severity: Medium** | **Effort: Medium**

archived_date: 2026-04-18
---

### Problem 18: `prior_art_max_lexical_overlap` and `prior_art_max_semantic_overlap` are always `None`

**Status:** In `scientia_discovery.rs` lines 289-291, both overlap fields are hardcoded to `None` in `rank_candidate()`. They are only populated by a separately-called `merge_novelty_overlap_into_rank()`.

**Impact:** Any ranking performed without the explicit merge call returns `None` for novelty overlap, making the rank appear to have perfect novelty when it may not.

**Solution:**
- Rename `rank_candidate()` → `rank_candidate_without_novelty()`.
- Add `rank_candidate_with_novelty(…, novelty_bundle: Option<&NoveltyEvidenceBundleV1>)` that internally merges.
- Update all callers (CLI, MCP, scan paths).

**Owner:** `vox-publisher` | **Severity: High** | **Effort: Small**

---

### Problem 19: `evidence_completeness_score` counts 11 binary signals with equal weight

**Status:** All 11 evidence signals contribute 1 point each. `human_meaningful_advance = true` weighs the same as `!doc_section_hints.is_empty()`.

**Impact:** Completeness scores are misleading. The `submission_readiness_score` KPI is contaminated.

**Solution:**
- Load per-signal weights from the heuristics YAML (Problem 14).
- `human_meaningful_advance` and `eval_gate_passed` should weigh 3×; doc hints 1×.

**Severity: Medium** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 20: No contamination risk detection for internal eval corpora

**Status:** The worthiness unification research doc identifies `contamination_risk_flag` as a candidate signal. No implementation exists.

**Impact:** An internal benchmark may be inflated due to training data overlapping with the eval set — a form of benchmark leakage that Scientia has no detector for.

**Solution:**
- Add `ContaminationRiskAssessor::assess(eval_corpus_id, training_corpus_ids) -> ContaminationRisk` in `vox-scientia-core`.
- Use n-gram overlap as a first-pass detector.
- Emit `contamination_risk_flag` in `worthiness_signals.v2` with `soft_gate` classification.

**Severity: Medium** | **Effort: Medium**

---

### Problem 21: MENS Lane G (`research-expert`) is not integrated into Scientia evidence flow

**Status:** `mens-research-track-blueprint-2026.md` gives Lane G a spec. The blueprint says "when `research_model_enabled` is true, the orchestrator delegates to this adapter." But:
- `research_model_enabled` is not a field in any config or runtime struct.
- No gate in `scientia_evidence.rs` or the orchestrator dispatches to Lane G.

**Solution:**
- Add `research_model_enabled: bool` to `VoxPopuliConfig` (or `SocratesTaskContext`).
- When `research_model_enabled && complexity >= Complex`, dispatch synthesis to Lane G endpoint.
- Add `MENS_LANE_G_ENDPOINT` env var resolved via Clavis.

**Owner:** `vox-orchestrator`, `vox-scientia-core` | **Severity: High** | **Effort: Medium**

archived_date: 2026-04-18
---

## Dimension 4 — Outbound Publication Pipeline

### Problem 22: LaTeX/journal template engine is absent from `submission/mod.rs`

**Status:** The readiness audit (§Phase 1 "Remaining") explicitly lists: "LaTeX/camera-ready package builder, figure/filename validators, template compliance against JMLR/TMLR/JAIR style packs" as still missing.

**Solution:**
- Add `TemplateProfile` enum: `Jmlr`, `Tmlr`, `Jair`, `Arxiv`, `Generic`.
- Implement `SubmissionPackageBuilder::build_with_template(profile)`:
  1. Validate source directory against profile requirements.
  2. Check figure formats (PDF preferred for JMLR, etc.).
  3. Generate `manifest.json` with SHA-256 digests.
  4. Create deterministic `.zip` archive.

**Owner:** `vox-publisher` | **Severity: High** | **Effort: Large**

---

### Problem 23: arXiv format preflight profile is missing

**Status:** The readiness audit explicitly states `arxiv_format_profile` is "missing."

**Solution:**
- Add `PreflightProfile::ArxivFormat` checking:
  - No filenames with spaces or non-ASCII characters.
  - Root LaTeX file present.
  - All `\includegraphics` targets resolvable.
  - No disallowed extensions in root.
- Wire into `publication-preflight --profile arxiv_format`.

**Severity: Medium** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 24: Crossref adapter is documented but not wired

**Status:** `crossref_metadata.rs` exists (transform is drafted). But no adapter in `scholarly/` actually submits to Crossref.

**Solution:**
- Implement `CrossrefAdapter` in `scholarly/crossref.rs`.
- Use existing `crossref_metadata.rs` for payload construction.
- Gate behind `VOX_SCHOLARLY_ENABLE_CROSSREF=1` and `CROSSREF_API_KEY` via Clavis.
- Add `vox scientia crossref-deposit` CLI (dry-run by default).

**Severity: High** | **Effort: Medium**

---

### Problem 25: `CITATION.cff` generation is incomplete / not wired to CLI

**Status:** `citation_cff.rs` exists (5.4KB) but the readiness audit lists this as "Missing machine-readable citation assets."

**Solution:**
- Audit `citation_cff.rs` against CFF 1.2.0 spec.
- Wire `vox scientia generate-citation-cff --output CITATION.cff` as a CLI command.
- Include `CITATION.cff` in `SubmissionPackageBuilder` output for Zenodo profile.

**Severity: Medium** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 26: Zenodo adapter only generates metadata JSON — no HTTP deposit

**Status:** The readiness audit says "Zenodo → partial (metadata done, upload/deposit not done)."

**Solution:**
- Add `ZenodoDepositClient` in `scholarly/zenodo.rs` using the Zenodo REST API.
- Implement: deposition creation → file upload → publish workflow.
- `ZENODO_ACCESS_TOKEN` via Clavis.
- Add `--sandbox` mode for pre-production validation.

**Owner:** `vox-publisher` | **Severity: High** | **Effort: Medium**

---

### Problem 27: No automatic submission status synchronization

**Status:** `publication-scholarly-remote-status-sync-batch` requires manual invocation. No scheduler calls it.

**Impact:** Submission status drift: an accepted paper may show as "submitted" indefinitely.

**Solution:**
- Add a scheduled worker that calls `publication-scholarly-remote-status-sync-batch` for all non-terminal submissions.
- Add `milestone_events` table: `(publication_id, milestone, recorded_at_ms, external_id)` with values `submitted | under_review | accepted | published | rejected`.

**Owner:** `vox-db`, `vox-publisher` | **Severity: High** | **Effort: Medium**

archived_date: 2026-04-18
---

### Problem 28: Author / co-author model mismatch (single `author` string vs `authors[]` array)

**Status:** The readiness audit §Lifecycle stage 2 flags: digest and CLI use a single `author` string; full co-author list lives in a JSON block. Mismatches if they disagree.

**Solution:**
- Add preflight check: if `scientific_publication.authors[]` present, derive `display_author` from `authors[0]`, warn on disagreement.
- Soft-deprecate the manifest `author` field.
- Update `manifest_completion_report` to check `authors[].orcid` completeness separately.

**Severity: Medium** | **Effort: Small**

---

### Problem 29: Revision lifecycle has no external venue revision ID mapping

**Status:** When digest changes, there is no way to know what revision number it corresponds to at the external venue (e.g., TMLR `v2`, OpenReview R2).

**Solution:**
- Add `scholarly_revision_map` table per `scholarly-external-schema-plan.md`.
- Capture external revision ID on each adapter submit response.
- `publication-status` should show unified timeline: `v1(digest=abc) → submitted → R1 → v2(digest=xyz) → R2 → accepted`.

**Severity: Medium** | **Effort: Medium**

archived_date: 2026-04-18
---

### Problem 30: Double-blind anonymization gate is partial (email heuristic only)

**Status:** The readiness audit (§Lifecycle stage 3) states: "email heuristic present, broader anonymization missing" for `double_blind` profile.

**Solution:**
- Extend `publication_preflight.rs` double-blind checks to scan:
  - `abstract_text` field for name/institution patterns (heuristic regex).
  - Generated filenames and LaTeX comments for author metadata.
  - Acknowledgements section stub.
- Add `AnonymizationScanResult { risk_level: High | Medium | Low }`.
- `High` → hard fail; `Medium` → warning in `next_actions`.

**Severity: Medium** | **Effort: Small**

---

### Problem 31: HN submission has no structured handoff payload

**Status:** The social execution board template exists but `hn_assist` in `destination_transform_previews()` (scientia_discovery.rs:470) just concatenates a string.

**Solution:**
- Add `HnHandoffPayload { title: String, url: String, comment: String }` to `syndication_outcome.rs`.
- Generate structured JSON during `destination_transform_previews()`.
- Add CI check that `title` respects the 80-char HN limit.

**Severity: Low** | **Effort: Small**

archived_date: 2026-04-18
---

## Dimension 5 — SSOT Convergence and Structural Problems

### Problem 32: Worthiness scoring exists in 5 competing locations with no CI parity check

**Status:** Numerics appear in `publication_worthiness.rs`, `publication-worthiness.default.yaml`, `worthiness-signals.v2.schema.json`, `scientia_heuristics.rs`, and `scientia_finding_ledger.rs`.

**Impact:** Updating a threshold requires touching 2-4 files. Silent inconsistency risk is high.

**Solution:**
- Declare `publication-worthiness.default.yaml` as the **single source of numeric truth**.
- `ScientiaHeuristics::from_default_yaml()` loads and validates against the JSON schema at startup.
- Add `vox ci scientia-worthiness-parity` cross-checking YAML values against unit test constants.
- All Rust constants reference the loaded struct, not magic numbers.

**Owner:** `vox-publisher`, contracts | **Severity: High** | **Effort: Medium**

---

### Problem 33: The 232-task wave backlog has no CI tracking or CLI surface

**Status:** `implementation-wave-backlog.v1.yaml` exists but there is no `vox ci scientia-wave-progress` and no CLI to query wave completion.

**Solution:**
- Add `vox scientia wave-status` CLI that reads the YAML and checks which expected artifacts exist on disk.
- Emit completion percentage per wave.
- Add as informational step in `vox ci ssot-drift`.

**Severity: Medium** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 34: `vox-publisher` is still the God Object the package-family split was meant to dissolve

**Status:** `vox-publisher/src/` has 28 source files; `lib.rs` alone is 40KB. `vox-scientia-core` does not exist as a crate. AGENTS.md limits to 500 lines / 12 methods.

**Solution:**
- Execute the Split Wave: move `scientia_evidence.rs`, `scientia_heuristics.rs`, `scientia_discovery.rs`, `scientia_contracts.rs` to `vox-scientia-core`.
- Wire `vox-publisher` as a re-export shim.
- Track in a `scientia-split-migration-ledger.md`.

**Severity: Medium** | **Effort: Large**

---

### Problem 35: Research Index does not link the RAG SSOT as the canonical retrieval reference

**Status:** `rag-and-research-architecture-2026.md` is the current-state SSOT for retrieval. `research-index.md` mentions it tangentially but does not surface it as the canonical SSOT.

**Solution:**
- Add "**Retrieval and RAG Architecture (Current)**" section to `research-index.md` linking to the RAG SSOT.
- Also cross-link from `scientia-publication-automation-ssot.md` source anchors.

**Severity: Low** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 36: `contracts/index.yaml` likely does not register all 27 scientia contracts

**Status:** The impact-readership research doc mandates contract registration in `contracts/index.yaml`. No evidence all 27 `contracts/scientia/` files are registered.

**Solution:**
- Audit `contracts/index.yaml` against `contracts/scientia/` directory listing.
- Add missing registrations.
- Add CI check that enforces `contracts/scientia/` ⊆ `contracts/index.yaml`.

**Severity: Medium** | **Effort: Small**

---

### Problem 37: `voxgiantia-publication-architecture.md` may be a shadow SSOT

**Status:** This 6.7KB doc is not referenced in the main SSOT's source anchors. It is unclear if it is superseded or covers a distinct scope.

**Solution:**
- Audit the doc for overlap with `scientia-publication-automation-ssot.md`.
- If superseded: add deprecation header + link to current SSOT.
- If distinct: add to SSOT source anchors with a scope label.

**Severity: Low** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 38: Syndication security docs are architecturally isolated from Scientia

**Status:** `news_syndication_incident_patterns.md` and `news_syndication_security.md` are not linked from the Scientia SSOT or the inbound discovery research doc.

**Solution:**
- Add links from `scientia-external-discovery-research-2026.md` to both syndication docs in a "Security constraints" section.
- Ensure `NewsInbound` preflight (Problem 4) incorporates the threat taxonomy from `news_syndication_security.md`.

**Severity: Low** | **Effort: Small**

---

## Dimension 6 — Quality, Evaluation, and Autonomy Gaps

### Problem 39: No golden test set for search recall

**Status:** The RAG SSOT §8 explicitly identifies "Recall@K golden set → Not built" as a gap.

**Solution:**
- Build 50-100 labelled `(query, expected_doc_ids)` pairs from real orchestrator queries.
- Add `vox ci search-recall-at-k` emitting Recall@5 and MRR metrics.
- Gate on ≤5% relative regression budget per PR.

**Severity: Medium** | **Effort: Medium**

archived_date: 2026-04-18
---

### Problem 40: No RAGAS-style faithfulness metric

**Status:** The RAG SSOT §8 identifies "RAGAS faithfulness → Not implemented" as a gap.

**Solution:**
- Implement lightweight faithfulness check: compare claim-sentences in answers against retrieved passages using existing BM25 lexical overlap logic.
- Run as a periodic background job (not on every completion).
- Persist results to Arca. Flag completions below `min_faithfulness = 0.4` for analysis.

**Severity: Medium** | **Effort: Medium**

---

### Problem 41: Socrates has no `evaluate_research_need()` dispatch path

**Status:** The RAG SSOT §4.4 shows `SocratesResearchDecision` as `[PLANNED]`. The struct is defined in the doc but does not exist in `crates/vox-socrates-policy/src/lib.rs`.

**Impact:** When Socrates returns `Abstain`, the caller has no structured signal about whether to trigger CRAG or simply decline.

**Solution:**
- Implement `evaluate_research_need(confidence, contradiction_ratio, complexity) -> SocratesResearchDecision` in `vox-socrates-policy`.
- Wire into the orchestrator's pre-generation hook.
- Auto-dispatch CRAG when `should_research = true`.

**Owner:** `vox-socrates-policy`, `vox-orchestrator` | **Severity: High** | **Effort: Medium**

archived_date: 2026-04-18
---

### Problem 42: The Coverage Paradox fix is documented but not coded

**Status:** The RAG SSOT §4.3 documents the fix (only apply contradiction penalty when `citation_coverage >= 0.3`) as `[PLANNED]`.

**Impact:** Agents fall into a refusal loop on abstract synthesis queries — the very class most relevant to Scientia research workflows.

**Solution:**
- Add `citation_coverage: Option<f64>` parameter to `classify_risk()`.
- When `citation_coverage < 0.3`, suppress `max_contradiction_ratio_for_answer` penalty.
- Add unit test: `low_coverage_high_contradiction_should_ask_not_abstain`.

**Owner:** `vox-socrates-policy` | **Severity: High** | **Effort: Small**

---

### Problem 43: No Tavily credit budget tracking or doctor warning

**Status:** The RAG SSOT §8 identifies "Tavily credit usage → Not tracked" as a gap.

**Impact:** Aggressive CRAG loops can exhaust the session credit budget silently.

**Solution:**
- Track `tavily_credits_used: u32` in the `SearchPolicy` session context.
- When usage ≥ 80% of budget, emit `SearchRefinementAction::BudgetWarning`.
- Add `vox clavis doctor` check displaying current credit budget.

**Severity: Medium** | **Effort: Small**

archived_date: 2026-04-18
---

### Problem 44: CLI/MCP tools bypass the `vox-scientia-api` package boundary

**Status:** `vox-cli/src/commands/scientia.rs` and `vox-mcp/src/tools/scientia_tools.rs` both directly import from `vox-publisher`, not `vox-scientia-api`.

**Impact:** When `vox-publisher` is eventually split, every CLI/MCP callsite will break.

**Solution:**
- Create `crates/vox-scientia-api/` as a façade crate.
- Update `vox-cli` and `vox-mcp` `Cargo.toml` to depend on `vox-scientia-api`.
- Add FROZEN marker on `vox-publisher`'s public surface.

**Severity: Medium** | **Effort: Small**

---

### Problem 45: No end-to-end integration test for the Scientia lifecycle

**Status:** Unit tests exist for individual functions. `acceptance_matrix.ps1` exists. But no integration test exercises the full pipeline: prepare → preflight → approve → scholarly-pipeline-run → status → metrics.

**Solution:**
- Add `tests/scientia_lifecycle_test.rs` using `local_ledger` / `echo_ledger` adapters (no external credentials needed).
- Cover: manifest creation → preflight pass → dual approval → external job tick → status assertion.
- Add to `vox ci scientia-novelty-ledger-contracts` or as `vox ci scientia-lifecycle`.

**Severity: Medium** | **Effort: Medium**

archived_date: 2026-04-18
---

## Summary Priority Matrix

| # | Problem | Severity | Effort | Owner Crate |
|---|---|---|---|---|
| 1 | No inbound pipeline crate | **Critical** | Large | `vox-scientia-ingest` (new) |
| 4 | No Socrates inbound profile | **Critical** | Medium | `vox-publisher`, `vox-socrates-policy` |
| 6 | No external intelligence DB table | **Critical** | Small | `vox-db` |
| 9 | Publications never re-enter search corpora | **Critical** | Medium | `vox-publisher`, `vox-search` |
| 18 | Prior art overlaps always `None` in `rank_candidate()` | **High** | Small | `vox-publisher` |
| 11 | Socrates Abstain events not persisted | **High** | Small | `vox-db`, `vox-socrates-policy` |
| 12 | CRAG results not stored back | **High** | Small | `vox-search` |
| 14 | Discovery ranking constants hardcoded in Rust | **High** | Small | `vox-publisher` |
| 16 | No multi-hop hypothesis chain generation | **High** | Large | `vox-scientia-core` |
| 21 | Lane G not integrated into Scientia evidence flow | **High** | Medium | `vox-orchestrator` |
| 22 | LaTeX package builder absent | **High** | Large | `vox-publisher` |
| 24 | Crossref adapter not wired | **High** | Medium | `vox-publisher` |
| 26 | Zenodo adapter metadata-only, no HTTP deposit | **High** | Medium | `vox-publisher` |
| 27 | No automatic submission status sync | **High** | Medium | `vox-db`, `vox-publisher` |
| 32 | Worthiness scoring split across 5 locations | **High** | Medium | `vox-publisher`, contracts |
| 41 | Socrates research dispatch not coded | **High** | Medium | `vox-socrates-policy` |
| 42 | Coverage Paradox fix not coded | **High** | Small | `vox-socrates-policy` |
| 5 | No semantic deduplication inbound | **Medium** | Small | `vox-scientia-ingest` |
| 7 | No Scholarly Digest contract | **Medium** | Medium | contracts, `vox-scientia-core` |
| 10 | Evidence packs not in knowledge graph | **Medium** | Medium | `vox-scientia-core`, `vox-search` |
| 13 | No FindingsDraftCorpus in RAG | **Medium** | Medium | `vox-search` |
| 15 | No signal code registry/CI check | **Medium** | Small | contracts, CI |
| 19 | Evidence completeness uses equal weights | **Medium** | Small | `vox-publisher` |
| 20 | No contamination risk detection | **Medium** | Medium | `vox-scientia-core` |
| 23 | arXiv format preflight missing | **Medium** | Small | `vox-publisher` |
| 25 | CITATION.cff generation incomplete | **Medium** | Small | `vox-publisher` |
| 28 | Author/co-author model mismatch | **Medium** | Small | `vox-publisher`, `vox-db` |
| 29 | No revision lifecycle mapping | **Medium** | Medium | `vox-db`, `vox-publisher` |
| 30 | Double-blind anonymization gate is partial | **Medium** | Small | `vox-publisher` |
| 33 | Wave backlog has no CI tracking | **Medium** | Small | CI, `vox-cli` |
| 34 | `vox-publisher` God Object not split | **Medium** | Large | All Scientia crates |
| 36 | Contract index missing scientia registrations | **Medium** | Small | contracts |
| 39 | No golden test set for search recall | **Medium** | Medium | `vox-search` |
| 40 | No RAGAS-style faithfulness metric | **Medium** | Medium | `vox-search`, `vox-db` |
| 43 | No Tavily credit tracking | **Medium** | Small | `vox-search`, `vox-clavis` |
| 44 | CLI/MCP bypass `vox-scientia-api` boundary | **Medium** | Small | `vox-cli`, `vox-mcp` |
| 45 | No lifecycle integration test | **Medium** | Medium | `vox-db` |
| 2 | No RSS/Atom feed parsing crate | **Medium** | Small | `vox-scientia-ingest` |
| 8 | No feed source registry table | **Medium** | Small | `vox-db` |
| 17 | No experimental design scaffolding | **Medium** | Medium | `vox-scientia-core` |
| 3 | No Reddit/HN inbound read path | **Low** | Medium | `vox-publisher` |
| 31 | HN submission unstructured handoff | **Low** | Small | `vox-publisher` |
| 35 | Research index missing RAG SSOT link | **Low** | Small | docs |
| 37 | Shadow SSOT doc `voxgiantia-publication-architecture.md` | **Low** | Small | docs |
| 38 | Syndication security docs isolated from Scientia | **Low** | Small | docs |

---

## Recommended Execution Order (7 Waves)

### Wave 0 — Quick Wins (1–3 days each, unblock parity and safety)
- **P18**: Fix `rank_candidate()` always-None novelty overlap
- **P42**: Code the Coverage Paradox fix in `classify_risk()`
- **P43**: Add Tavily credit tracking and doctor warning
- **P15**: Add discovery signal code registry and CI check
- **P19**: Load evidence completeness weights from YAML
- **P44**: Create `vox-scientia-api` façade and update CLI/MCP

### Wave 1 — Foundation Hardening (1–2 weeks)
- **P11**: Persist Socrates Abstain events to Arca
- **P12**: Store CRAG results back into DocumentChunks
- **P14**: Load `ScientiaHeuristics` from YAML contract
- **P28**: Author/co-author model preflight + soft-deprecation
- **P32**: Unify worthiness scoring to YAML source of truth + parity CI
- **P35, P36, P37, P38**: Documentation and contract housekeeping
- **P41**: Implement `evaluate_research_need()` dispatch in Socrates
- **P33**: Add `vox scientia wave-status` CLI

### Wave 2 — Inbound Pipeline (new crate focus)
- **P6**: Add `scientia_external_intelligence` DB table
- **P8**: Add `scientia_feed_sources` DB table and CLI commands
- **P1**: Create `vox-scientia-ingest` crate shell
- **P2**: Wire `feed-rs` for RSS/Atom crawling
- **P4**: Add `PreflightProfile::NewsInbound` in Socrates
- **P5**: Add `IngestDeduplicator` against embeddings table
- **P7**: Add `scholarly-digest.v1.schema.json` + `digest-generate` CLI

### Wave 3 — RAG Feedback Loop
- **P9**: `PostPublishIndexer` — publications back into `DocumentChunks`
- **P10**: `EvidencePackKGIndexer` — evidence chains into KnowledgeGraph
- **P13**: `FindingsDraftCorpus` variant for in-progress findings

### Wave 4 — Discovery Intelligence Upgrade
- **P16**: `HypothesisChainBuilder` with Lane G integration
- **P17**: `experiment-scaffold` CLI
- **P20**: `ContaminationRiskAssessor`
- **P21**: Wire Lane G into the Scientia synthesis path

### Wave 5 — Outbound Publication Completeness
- **P22**: LaTeX/template engine in `SubmissionPackageBuilder`
- **P23**: `PreflightProfile::ArxivFormat`
- **P24**: `CrossrefAdapter` wired
- **P25**: Complete `citation_cff.rs` and wire CLI
- **P26**: `ZenodoDepositClient` HTTP submit
- **P27**: Auto status sync scheduler + `milestone_events` table
- **P29**: `scholarly_revision_map` table
- **P30**: Extended double-blind anonymization scan
- **P31**: Structured `HnHandoffPayload`

### Wave 6 — God Object Split and Structural
- **P34**: Extract `vox-scientia-core` from `vox-publisher`
- **P45**: Lifecycle integration test suite

### Wave 7 — Quality and Evaluation
- **P39**: Golden recall test set + `vox ci search-recall-at-k`
- **P40**: Lightweight RAGAS-style faithfulness metric

archived_date: 2026-04-18
---

## Appendix: Cross-References

| Concern | Primary SSOT | Owner Crate |
|---|---|---|
| Publication pipeline | `scientia-publication-automation-ssot.md` | `vox-publisher` |
| RAG retrieval | `rag-and-research-architecture-2026.md` | `vox-search` |
| Hallucination gate | `vox-socrates-policy/src/lib.rs` | `vox-socrates-policy` |
| Evidence model | `scientia_evidence.rs`, `scientia-evidence-graph.schema.json` | `vox-publisher` |
| Discovery ranking | `scientia_discovery.rs`, `publication-worthiness.default.yaml` | `vox-publisher` |
| Inbound discovery | `scientia-external-discovery-research-2026.md` | `vox-scientia-ingest` (TBD) |
| MENS Lane G | `mens-research-track-blueprint-2026.md` | `vox-orchestrator` |
| Worthiness signals | `worthiness-signals.v2.schema.json` | contracts |
| Impact/readership | `scientia-impact-readership-research-2026.md` | assistive only |
| Automation boundaries | `scientia-publication-worthiness-ssot-unification-research-2026.md` | policy |


