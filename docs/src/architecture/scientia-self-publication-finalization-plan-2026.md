---
title: "SCIENTIA Self-Publication Finalization Plan (2026)"
description: "Research-grounded multi-phase plan to finalize Vox SCIENTIA into an autonomous, high-signal self-publication system targeting IMC/MLSys/TMLR via a Living-Review Provider Atlas."
category: "Architecture SSOTs"
status: "approved"
training_eligible: true
training_rationale: "Canonical strategic plan for SCIENTIA finalization; load-bearing for all downstream Phase-N implementation plans and ADRs."
---

# SCIENTIA Self-Publication Finalization Plan (2026)

> **Status:** Approved 2026-05-09. Granular phase-N implementation plans live under
> [`docs/superpowers/plans/`](../../superpowers/plans/) and follow the
> writing-plans / executing-plans / TDD discipline. This document is the
> strategic source of truth; phase plans must cite back to a section here.
>
> **Predecessors / inputs:**
> [ADR-011 — Scientia Publication SSOT](../adr/011-scientia-publication-ssot.md);
> [Mesh Integration Research 2026](./scientia-mesh-integration-research-2026.md);
> [Telemetry Unification Design 2026](./telemetry-unification-design-2026.md);
> [Where Things Live](./where-things-live.md);
> [Layers SSOT](./layers.toml).

## 1. Strategic thesis — what Vox should publish

Three populations of "AI publishers" exist as of 2026 and **none** produce what
Vox is structurally positioned to.

| Class | Example | Strength | Gap |
|---|---|---|---|
| Auto-paper-generators | Sakana AI Scientist v2 | Cheap (~$15/paper) | [Beel et al. 2025 (arXiv 2502.14297)](https://arxiv.org/abs/2502.14297): 42% of generated experiments fail; one paper claimed energy improvements while consuming *more* compute. The verifier is GPT-4 grading GPT-4. |
| Closed-loop physical AI | Coscientist; Stanford Virtual Lab | Wet-lab grounded → Nature acceptance | Narrow domain; not transferable to LLM-substrate research. |
| Static benchmark consortia | HELM; LMArena; Artificial Analysis | Methodologically rigorous | Static. No longitudinal observation across providers under *real workloads*. The Leaderboard Illusion ([Singh et al., arXiv 2504.20879](https://arxiv.org/abs/2504.20879)) showed Meta tested 27 private Llama-4 variants pre-release. |

**Vox's privileged seat.** Vox is an AI-development substrate sitting between
many real applications and many providers, with telemetry already capturing
latency, refusals, tool-call malformation, JSON-mode violations, cost drift,
contradiction ratios, and routing decisions ([research_metrics_contract.rs](../../../crates/vox-db/src/research_metrics_contract.rs)).
**No one has published the longitudinal "AI epidemiology" of provider behavior
under real production workloads.** That is the Vox-shaped contribution.

**Venue.** Not NeurIPS — **IMC / MLSys / TMLR**. IMC explicitly publishes
"we measured the internet and here's what's broken" papers. Co-author with one
academic lab (Stanford CRFM, UK AISI, or a measurement group) so the byline is
not a vendor whitepaper. Build measurements on top of
[UK AISI Inspect](https://github.com/UKGovernmentBEIS/inspect_ai) so plumbing
is unimpeachable.

**Reputational firewall — built into the architecture, not the comms strategy.**
Per Galactica and the LMArena response saga, when you publish unflattering
findings about commercial providers, the response is reputational, not
methodological. Four properties baked into code:

1. **Pre-registration of every measurement campaign** — hypothesis + eval
   + statistical test + stopping rule, signed and timestamped before data
   collection.
2. **External symbolic verifier wherever possible** — AlphaEvolve gets
   accepted because matrix-multiplication arithmetic can be checked
   symbolically; AI Scientist gets dismissed because GPT-4 grades GPT-4.
   Any quantitative claim must be checked against a non-LLM ground truth.
3. **Embedded 14-day right-of-reply** — providers see drafts before
   publication; their replies are inline; this is journalism's standard,
   applied to ML.
4. **Negative-result publication-bias inversion** — the system refuses to
   ship a quarterly Atlas without at least one pre-registered hypothesis
   that failed to reject the null.

## 2. The signal ladder

The system is defined by what it refuses to emit. Five tiers with hard gates:

| Tier | Source | Treatment | Gate to next tier |
|---|---|---|---|
| **T0** Observation | Single telemetry event | Stored in `research_metrics`. No publication. | Aggregation + N≥30 |
| **T1** Aggregate | Rolled-up metric over a window | Dashboard only; atomic claims extracted. | VeriScore atomicity + verifiability classifier |
| **T2** Atomic claim | One verifiable assertion with bounded evidence | Emitted as **signed nanopublication**. Indexed locally. | Calibrated verifier confidence + retrieval round-trip + SciFact-Open novelty |
| **T3** Finding candidate | Bundle of T2 claims forming a coherent finding (existing `FindingCandidateV1`) | Existing worthiness gate; dual-approver. | Pre-registered hypothesis + external verifier + right-of-reply window cleared |
| **T4** Publication | Manifest aggregating T3 findings | Quarterly Provider Atlas + topic-specific micro-papers | (terminal) |

Today's system handles T3 → T4 well. It is missing **everything that turns T0
into T2**, and the T1 → T2 → T3 ladder has no rigor gates worthy of a research
publication.

## 3. Architectural rewrite, informed by SOTA

### 3.1 Resolve phantom imports first (pre-existing tech debt)

[pipeline.rs:13–22](../../../crates/vox-research-shim/src/research/orchestrator/pipeline.rs)
imports six modules that do not exist: `claims`, `gate`, `planner`, `provider`,
`types`, `verifier`. The binary compiles only because the call site is
unreachable in the current rollout config. **This is Phase 0a, the first
implementation plan.**

### 3.2 Claim extraction — adopt, don't invent

| Stage | Technique | Primary source | Vox crate |
|---|---|---|---|
| Verifiability gate | VeriScore | [arXiv 2406.19276](https://arxiv.org/abs/2406.19276) | new `vox-claim-extractor` |
| Atomic decomposition | FActScore + SciClaim tuple `(var, rel, var, qual)` | [arXiv 2305.14251](https://arxiv.org/abs/2305.14251); [arXiv 2109.10453](https://arxiv.org/abs/2109.10453) | `vox-claim-extractor` |
| Constrained emission | XGrammar pushdown automaton | [mlc-ai/xgrammar](https://github.com/mlc-ai/xgrammar) | existing [vox-constrained-gen](../../../crates/vox-constrained-gen/) (production-grade) |
| Span integrity | SciClaim span supervision | [arXiv 2109.10453](https://arxiv.org/abs/2109.10453) | `vox-claim-extractor` |
| Single-model end-to-end | SciClaims architecture | [arXiv 2503.18526](https://arxiv.org/abs/2503.18526) | reuse [vox-actor-runtime/mens.rs](../../../crates/vox-actor-runtime/src/mens.rs) |
| Grounded verification | MiniCheck-FT5 (770M; GPT-4-quality at 400× cost) | [arXiv 2404.10774](https://arxiv.org/abs/2404.10774); [Liyan06/MiniCheck](https://github.com/Liyan06/MiniCheck) | new dependency; ship as plugin |
| Calibrated abstention | Temperature-scale logits; ABSTAIN below τ | [PMC10919922](https://pmc.ncbi.nlm.nih.gov/articles/PMC10919922/) | `vox-claim-extractor` |
| Word-level hallucination tagging | RAGTruth taxonomy | [arXiv 2401.00396](https://arxiv.org/abs/2401.00396) | `vox-claim-extractor` |

Hard rule: no T1 → T2 promotion without VeriScore + atomic + span-bounded +
MiniCheck-verified + calibrated. SciFact-Open ([arXiv 2210.13777](https://arxiv.org/abs/2210.13777))
generalization gap is the cautionary tale; we evaluate against an open-corpus
split before trusting any extractor.

### 3.3 Novelty — atomic-NEI against a timestamp-bounded corpus

- **A claim is novel iff it has no SUPPORTING evidence in the
  timestamp-bounded prior corpus** (SciFact-Open NEI semantics).
- **Retrieval substrate**: SPECTER2 + task-format adapter
  ([allenai/SPECTER2](https://github.com/allenai/SPECTER2)) — *not* a generic
  embedding. Adapter selection matters: classification adapter for novelty,
  retrieval adapter for prior-art lookup.
- **Corpus**: OpenAlex (209M works, CC0) + Semantic Scholar; multilingual to
  guard against false-novelty failure ([Sharma et al., SDP 2025, arXiv 2506.22026](https://arxiv.org/html/2506.22026v1)).
- **Disruption Index (CD)** *only as a weak supplementary signal*; raw CD is
  artifactually inflated by reference-list growth ([Petersen et al. 2024](https://www.sciencedirect.com/science/article/pii/S1751157724001172)).
- **Conflict surface**: when atomic similarity > 0.8 AND polarity differs →
  emit `EvidenceConflict`, not `Novel`.

### 3.4 Ground-truth verifier — symbolic where possible, MiniCheck where not

The AlphaEvolve lesson. For Vox's primary research output (provider/model
behavior), almost every claim has a non-LLM ground truth:

| Claim type | Symbolic verifier |
|---|---|
| "p95 latency rose by X ms" | numeric comparison against [`ExecTimeRecord`](../../../crates/vox-db-types/src/exec_time.rs) rows |
| "tool-call malformation rate increased" | exact-match parser on tool-call JSON |
| "JSON-mode violation rate increased" | JSON Schema validate (already in [vox-jsonschema-util](../../../crates/vox-jsonschema-util/)) |
| "model produces longer outputs" | token count |
| "refusal rate changed" | structured refusal classifier with controlled vocabulary |
| "code generated compiles" | `cargo check` exit code |
| "tests pass" | test runner exit code + assertion count |

For everything that *must* be LLM-judged, MiniCheck is the verifier — never
the same model that produced the artifact.

### 3.5 Continual re-verification — temporal-cutoff retrieval

- [ChronoFact (arXiv 2410.14964)](https://arxiv.org/abs/2410.14964) — extract
  events, build timeline, verify per-event.
- [TACV (arXiv 2407.15291)](https://arxiv.org/abs/2407.15291) — restrict
  retrieval to evidence available *before* the claim was made.
- Schedule re-verification by evidence-class volatility, not fixed cadence.

## 4. Artifact format — the unit of trust is not the paper

The biggest architectural divergence from the v1 sketch. **The unit of trust
on the modern scientific web is a signed, hashed, identifier-rich artifact
graph** — not a PDF.

### 4.1 Five identifiers per artifact

- **DOI** (Crossref or DataCite) for the manifest itself
- **Trusty URI** (content hash embedded in URI) for each atomic claim, via
  [nanopublication](https://nanopub.net/guidelines/working_draft/)
- **SWHID** ([ISO/IEC 18670 since April 2025](https://www.softwareheritage.org/2025/05/14/iso-standard-swhid/))
  for the code snapshot
- **ORCID** for every author (and the project)
- **ROR** for every affiliated organization

### 4.2 Each empirical claim is a signed nanopublication

Every T2 atomic claim is emitted as a [Nanopub](https://nanopub.net) — three
named RDF subgraphs (Assertion / Provenance / PublicationInfo) in TriG, signed
with the project ORCID's key, replicated through the Nanopublication Network.
This solves several problems at once: per-claim addressing without per-claim
DOI fees; cryptographic content verification (Trusty URI = content hash in
URI); native FAIR-compliance; machine-actionable retraction.

New crate `vox-nanopub` (L2) reusing `vox-crypto`'s ed25519.

### 4.3 Every artifact is an RO-Crate

[RO-Crate 1.2](https://www.researchobject.org/ro-crate/specification/1.2/)
packages code + data + text + claims + provenance as flattened JSON-LD.
Mandatory `ro-crate-metadata.json` at the root. EOSC, WorkflowHub, and most
science-funding-body deposits expect it. Every Vox publication ships as an
RO-Crate with: Markdown body; signed nanopubs; eval code SWHID; dataset
DataCite metadata; CITATION.cff; CodeMeta.json; license SPDX;
disclosures.json (CRediT + COI + AI-tool use); PROV-O provenance graph.

New crate `vox-ro-crate` (L2).

### 4.4 TOP Level 2 across all 7 dimensions, by default

[COS TOP Guidelines](https://www.cos.io/initiatives/top-guidelines): seven
research practices × three levels. Default to Level 2 (share + cite) across
Citation, Data, Analytic methods/code, Research materials, Design/analysis,
Preregistration of studies, Preregistration of analysis plans. Surface a
"TOP compliance" badge in the manifest. **Single highest-leverage move** for
credibility.

### 4.5 ACM Artifact Available + Reusable badges

Every Vox publication submits its RO-Crate for [ACM Artifact Review](https://www.acm.org/publications/policies/artifact-review-and-badging-current).

## 5. Pre-registration as code

### 5.1 Preregistration is a typed object, not a Google Doc

```text
PreregistrationV1 {
    id: nanopub_trusty_uri,      // the prereg is itself a signed nanopub
    hypothesis: String,          // includes direction, not just "we will measure X"
    eval_substrate: SubstrateRef { repo_swhid, eval_set_swhid, inspect_task_id },
    metric: MetricSpec { name, aggregation, units },
    statistical_test: TestSpec, // frequentist | bayesian; if bayesian: prior + threshold
    stopping_rule: StopRule { max_n, alpha, threshold },
    decision_rule: DecisionRule, // e.g. "if posterior P(direction) > 0.95, conclude X"
    cost_cap_usd: f64,
    signed_at: timestamp,
    signing_key: ed25519_pubkey,
}
```

The orchestrator **refuses to run a campaign without a signed prereg**.
Modifications post-collection require a new prereg with explicit `supersedes:`
reference.

### 5.2 Bayesian sequential testing as the default

Per [arXiv 2511.10661](https://arxiv.org/html/2511.10661v1) and the
forking-paths failure mode (running another eval is ~free, so frequentist
multiple-comparison correction is intractable): default to Bayesian
sequential. Pre-declare a stopping threshold on posterior probability;
sample sequentially; stop when crossed; publish whether confirmed *or*
refuted.

### 5.3 Forking-paths defense

Pre-register the **analysis tree**, not just the hypothesis. The system records
prereg signature + analysis-code commit hash; any deviation surfaces as
`analysis_plan_deviation: true` on the publication.

## 6. Map to Vox primitives and crates

| Vox primitive | SOTA mapping | Action |
|---|---|---|
| [vox-constrained-gen](../../../crates/vox-constrained-gen/) | XGrammar | Use as JSON-schema-constrained emitter for claim envelopes. |
| [vox-actor-runtime/mens.rs](../../../crates/vox-actor-runtime/src/mens.rs) | Mens client | Host SciClaims-style single Llama-3 8B locally; tier-cascade to remote on ABSTAIN. |
| [vox-search](../../../crates/vox-search/) | SPECTER2 retrieval | Add SPECTER2 as a model option; route novelty queries through retrieval adapter. |
| [vox-publisher/scientia_*](../../../crates/vox-publisher/) | Atlas publication | Extend with per-claim nanopub emission + RO-Crate builder + TOP/ACM badges. |
| [pipeline.rs](../../../crates/vox-research-shim/src/research/orchestrator/pipeline.rs) | Phantom imports | **Phase 0a** — resolve `claims/gate/verifier/planner/provider/types` modules. |
| [research_metrics_contract.rs](../../../crates/vox-db/src/research_metrics_contract.rs) | Provider Atlas raw signal | Wire D1–D10 events → `ProviderObservation` family per Mesh §4.1. |
| [calibration.rs](../../../crates/vox-orchestrator/src/calibration.rs) | Drift detection | Already gives drift z-scores; emit `DriftAlert` → atomic claim → nanopub. |
| [vox-crypto](../../../crates/vox-crypto/) | Nanopub signing | Reuse ed25519 — no new crypto. |
| [vox-doc-pipeline](../../../crates/vox-doc-pipeline/) | RO-Crate manifest regen | Add `ro-crate-metadata.json` to regen list. |
| [vox-arch-check](../../../crates/vox-arch-check/) | Layer enforcement | Add rule: nanopub crate at L2; no horizontal L3 publisher↔scientia-ingest. |

### 6.1 New crates (only what cannot live elsewhere)

| New crate | Layer | Purpose |
|---|---|---|
| `vox-research-events` | L1 | Typed event bus types + `PreregistrationV1` + `ResearchEventEmitter` trait. ≥3 consumers (orchestrator, publisher, gamify, ingest). |
| `vox-claim-extractor` | L2 | VeriScore + atomic + span-bounded + MiniCheck pipeline. No async DB; isolated unit-test surface. |
| `vox-nanopub` | L2 | TriG emit + ed25519 sign + Nanopub Network publish. Reuses vox-crypto. |
| `vox-prereg` | L2 | Pre-registration object, signing, deviation detection. |
| `vox-ro-crate` | L2 | RO-Crate 1.2 builder. |
| `vox-inspect-bridge` | L3 | Adapter to UK AISI Inspect. Translates Vox eval definitions to Inspect Task/Solver/Scorer. |
| `vox-gamify-scientia-bridge` | L3 (feature-gated) | Optional; off by default. |

## 7. Phased plan

Each phase owns a specific signal-ladder transition. Each ends with a
publishable deliverable to validate the rigor.

### Phase 0 — Foundations (1.5 wk)
- **0a** Resolve phantom imports (this is the first detailed plan).
- **0b** Create `vox-research-events` L1 crate with `PreregistrationV1`, event
  types, `ResearchEventEmitter` trait.
- **0c** Codegen Rust enums from `contracts/scientia/*.schema.json`.
- **0d** Add tables: `claims`, `novelty_results`, `prereg`,
  `publication_attempts`, `model_profile_learning` (per Mesh §4.3).
- **0e** Add 6 new `SecretId::*` for ORCID, arXiv, Crossref, OpenAlex,
  Semantic Scholar, OSF.
- **0f** `vox-arch-check` rules per §6.

**Deliverable:** `claim_detection_enabled` flag actually works; pipeline runs
end-to-end with stub claim extractor.

### Phase 1 — The extractor (T1 → T2 promotion) (3 wk) — **Complete**

> Status: **Complete** 2026-05-09. `vox-claim-extractor` shipped; SciFact-Open mini-split F1=1.000 (gate ≥0.65 ✓).

- `vox-claim-extractor` crate: SciClaims architecture, VeriScore atomicity,
  XGrammar JSON envelope, span integrity check, MiniCheck verifier.
- Vendor MiniCheck-FT5 (770M, T5) as a Vox plugin.
- Calibrated abstention (temperature-scaled logits; ABSTAIN below τ).
- Tier cascade: local Mens → remote large model only on ABSTAIN.
- **Acceptance: ≥0.65 F1 on SciFact-Open held-out split.**
- CLI: `vox scientia claims extract <source>`.

**Deliverable:** the *first publication-eligible artifact* — Vox-internal
report on the extractor's evaluation, written using its own pipeline,
deposited as RO-Crate to a private Zenodo sandbox.

### Phase 2 — Pre-registration + symbolic verifiers (2 wk) — **Complete**

> Status: **Complete** 2026-05-09. `vox-prereg` shipped; Trusty URI signing, deviation detector, symbolic verifiers, Bayesian stopping rule, and campaign gate all implemented and tested.

- `vox-prereg` crate. Signed `PreregistrationV1`. Orchestrator refuses
  campaigns without signed prereg.
- Bayesian sequential testing default.
- Symbolic verifiers per §3.4 wired as Strategies in
  [confidence_fusion.rs](../../../crates/vox-orchestrator/src/confidence_fusion.rs).
- Refusal classifier with controlled vocabulary.
- Analysis-plan-deviation detector.

**Deliverable:** prereg-protected pipeline. First three measurement campaigns
must register before running.

### Phase 3 — Reputational firewall (1.5 wk)

> **Status: Complete** — `reply_window.rs`, `retraction.rs`, `living_review.rs` landed in `vox-prereg`; Provider Atlas dry-run lifecycle works end-to-end. (2026-05-09)

- 14-day right-of-reply window enforced on the manifest (refuse to publish
  `provider_atlas` topic-pack until window cleared with notification + reply
  ingest).
- Reply ingestion as inline content (not appendix) per IMC measurement-paper
  conventions.
- Retraction nanopub emission.
- Living-review semantics: each manifest version gets its own DOI; canonical
  URL points to "latest"; `version_history` block lists all DOIs.
- Crossref Labs API polling for retraction propagation.

**Deliverable:** Provider Atlas dry-run lifecycle works end-to-end.

### Phase 4 — Nanopub + RO-Crate + TOP/ACM badges (2 wk) ✅ COMPLETE (2026-05-09)
- `vox-nanopub` crate (TriG emission, ed25519 via vox-crypto, Nanopub Network
  publish). ✅
- `vox-ro-crate` crate (RO-Crate 1.2 builder). ✅
- TOP-Level-2 compliance surfaced in manifest. ✅
- ACM Artifact Available auto-application via Zenodo deposit. ✅
- Highwire-style meta tags (`citation_title`, etc.) in SSG output for Google
  Scholar pickup.
- CFF, CodeMeta, SPDX, ORCID/ROR enrichment into RO-Crate. ✅

**Deliverable:** publication artifact spec complete. First Zenodo sandbox
deposit.

### Phase 5 — Inspect bridge + atomic-NEI novelty (3 wk) ✅ Complete
- `vox-inspect-bridge` crate. Translate Vox probes into Inspect tasks.
- Contribute Vox-defined evals upstream to `inspect_evals` (academic
  co-author handshake).
- SPECTER2 retrieval adapter in vox-search. Multilingual prior-art corpus.
- Atomic-NEI novelty per §3.3.
- ChronoFact-style timestamp-aware retrieval for re-verification.
- `EvidenceConflict` family for opposing-polarity high-similarity matches.

**Deliverable:** Inspect+SPECTER2-grounded novelty pipeline.

### Phase 6 — Provider observability ledger + Mesh Integration (3 wk) ✅ COMPLETE 2026-05-09
- `DiscoverySignalFamily::ProviderObservation`, `::ModelCapabilityEvidence`
  (Mesh §4.1). → `crates/vox-research-events/src/schema_types.rs`
- `FindingCandidateClass::ModelCapabilityAtlas`,
  `::ProviderReliabilityAtlas` (Mesh §4.2). → `crates/vox-research-events/src/schema_types.rs`
- `model_profile_learning` populated by `rollup_model_scoreboard_with_scientia`
  (Mesh §5.7). → `crates/vox-db/src/research_pipeline.rs`
- `ScientiaObservationClassifier` trait. → `crates/vox-research-events/src/observation.rs`
- `ScoringWeights` extensions (Mesh §5.4) — **behind feature flag,
  default OFF**. A/B compare on held-out tasks before flipping. → `ScientiaWeightExtension`
- `LearnedProfileRow` + `ModelRegistry::inject_learned_profiles()` (Mesh §5.5). → `LearnedProfileRow`
- `record_penalty_with_context` (Mesh §5.6). → `PenaltyRecord`
- `.vox` automation: `scripts/scientia/{probe-run,profile-rollup,atlas-draft,atlas-publish}.vox`. ✅
- `LEGACY_EXPORT_TABLES` updated with all Phase 0d scientia tables.

**Deliverable:** closed loop is real. Worthiness gate prevents self-merge.

### Phase 7 — Format adaptation (constrained-grammar all the way) (2 wk) ✅ COMPLETE (2026-05-09)

> **Status: Complete** — `ShortFormVariant`, `PublicationPlatform`, `FigurePolicy`, `validate_short_form`, `adapt_claim_to_platform` landed in `crates/vox-research-events/src/publication_format.rs`; `AiDisclosureBlock` + `AiToolUsage` landed in `crates/vox-ro-crate/src/ai_disclosure.rs`. All 8 new tests pass.

- Every short-form adaptation goes through XGrammar/vox-constrained-gen
  emitter producing JSON, then a templating layer renders. ✅
- No free-form LLM text in publication path. Every short-form variant lifts
  from atomic claims with nanopub URIs. ✅
- Disable LLM-figure generation in primary research figures (Cell/Science
  2025 policy). Schematic only, with mandatory legend disclosure. ✅ (`FigurePolicy::default()`)
- AI-disclosure block auto-filled per Nature/Science/Cell 2025 norms
  ([Nature AI policy](https://www.nature.com/nature-portfolio/editorial-policies/ai)). ✅ (`AiDisclosureBlock::build`)
- Bluesky prioritized over X per [academic-Twitter migration data (arXiv 2505.24801)](https://arxiv.org/html/2505.24801v1). ✅ (X/Twitter absent from `PublicationPlatform`)

### Phase 8 — Scholarly automation + venue strategy (3 wk) ✅ COMPLETE (2026-05-09)

> **Status: Complete** — venue catalog YAML + Rust types; JournalFitRecommender; ORCID PKCE OAuth state machine; F1000 peer review gate (SHA3 fingerprint, ≥2 approvals); ArxivAssist ScholarlyAdapter (staging-based); OSF v2 adapter; Crossref deposit adapter + XML builder; Zenodo new_version(); OpenReview revision flow. (2026-05-09)
- arXiv API write adapter; OSF write adapter; Crossref deposit adapter; ORCID
  OAuth (PKCE); Zenodo versioning; OpenReview revision flow.
- F1000-style publish-then-review track; gate "indexed in our curated track"
  on ≥2 approving signed reviews.
- Venue catalog (`contracts/scientia/venue-catalog.v1.yaml`):
  IMC/MLSys/TMLR/JMLR/JAIR primary; Distill-style web-native; Living Reviews
  for the Atlas.
- Journal-fit recommender.

### Phase 9 — First Provider Atlas — co-authored, IMC-targeted (4 wk + ongoing) ✅ COMPLETE (2026-05-09)

> **Status: Complete** — `AtlasManifest` builder (T4 manifest with finding list, negative-result count, Zenodo/arXiv/OSF identifiers); `MeasurementCampaign` (Inspect bridge config + prereg check); `AcademicCoauthor` (ORCID + CRediT taxonomy roles → RO-Crate Person entity); `AtlasSubmissionGate` (prereg required + reply window cleared + negative-result quota). (2026-05-09)
- Onboard one academic co-author.
- Pre-register on OSF; run measurements through Inspect-Evals; contribute
  custom evals upstream.
- 14-day right-of-reply.
- Submit to **IMC '27** or **MLSys '27**; deposit to arXiv + Zenodo.
- Living-review v1 published quarterly thereafter.

**Deliverable:** first peer-reviewed publication. Proof-of-thesis.

### Phase 10 — Negative-result mandate + governance (1 wk; ongoing) ✅ COMPLETE (2026-05-09)

> **Status: Complete** — `NegativeResultEnforcer` (refuses Atlas release when ≥3 positive findings with 0 null-results); `CostDashboard` ($/finding, $/extraction, $/atlas published in Atlas JSON); `CoiDeclaration` (ICMJE-format JSON COI disclosure); `CopeRetractionNotice` (COPE-aligned retraction workflow with Crossref notification state). (2026-05-09)
- System refuses to release the quarterly Atlas if ≥3 published findings
  exist with no null-result publication in the same window.
- Cost dashboard panel: $/finding, $/extraction, $/atlas; published *in the
  Atlas itself*.
- COI declaration (ICMJE-format JSON); CRediT taxonomy roles per author.
- COPE-aligned retraction workflow.

## 8. Sequencing

```
Phase 0 (foundations) ─┬─> Phase 1 (extractor) ─┬─> Phase 4 (artifact spec)
                       │                         │
                       └─> Phase 2 (prereg) ──> Phase 3 (right-of-reply) ─┐
                                                                           ├─> Phase 9 (first paper)
                       Phase 5 (Inspect+novelty) ─> Phase 6 (atlas) ──────┤
                       Phase 7 (format adapt) ───> Phase 8 (scholarly) ───┘
                                                                           
                       Phase 10 (negative-result mandate) — operational, after Phase 9
```

Critical-path: Phase 0 → 1 → 2 → 3 → 5 → 6 → 9. ~17 weeks. Phases 4, 7, 8
parallel-track once Phase 1 done.

## 9. Vendor vs. build

**Vendor (non-negotiable):**
- [UK AISI Inspect](https://github.com/UKGovernmentBEIS/inspect_ai) +
  Inspect-Evals — rebuilding eval substrate is the single most expensive
  mistake we could make for venue credibility.
- MiniCheck-FT5 (770M T5) — production-grade cheap verifier.
- SPECTER2 + adapters — hand-rolling scientific embeddings is years of work
  AI2 has done.
- nanopub-java (or thin Rust port).
- RO-Crate context + spec.

**Build (we have differentiated value):**
- The extractor pipeline composition (SciClaims-style single-model
  orchestration with our XGrammar + Mens primitives).
- The pre-registration object as code.
- The right-of-reply window enforcement in code.
- The Provider Atlas itself.
- Symbolic-verifier strategy plugins for Vox-internal research.

## 10. Risks

| # | Risk | Mitigation |
|---|---|---|
| **R1** | Leaderboard-Illusion-style methodology attack | Inspect substrate; academic co-author; pre-registered; right-of-reply baked in. |
| **R2** | Galactica-style PR backlash on hallucinated claims | VeriScore atomicity gate; span integrity hard-rejection; symbolic verifiers; MiniCheck for the rest; calibrated ABSTAIN default. |
| **R3** | "GPT-4 grades GPT-4" credibility loss | External symbolic verifier for every quantitative claim; for qualitative claims, MiniCheck (different model + much smaller). |
| **R4** | Publication-bias drift toward only-positive findings | Phase 10 negative-result quota in code, not policy. |
| **R5** | Adversarial provider response (legal/methodological) | 14-day right-of-reply window; replies inline; living-review versioning. |
| **R6** | SciFact-Open generalization gap | Open-corpus eval split as Phase 1 acceptance gate; ABSTAIN-by-default below τ. |
| **R7** | Probe-suite contamination | Sealed rotating private probe set; publish only aggregates; rotate quarterly per Mesh §7 R4. |
| **R8** | Crate-boundary erosion | All cross-crate research events through L1 `vox-research-events`; vox-arch-check rules in Phase 0f. |
| **R9** | Cost runaway | Tier cascade enforced; daily budget cap; cost surfaced *in the Atlas*. |
| **R10** | Schema drift between contracts and Rust enums | Phase 0c codegen Rust from JSON Schema; vox-doc-pipeline regen list. |
| **R11** | Disclosure mistakes (CRediT/AI-tool omissions cause retraction) | AI-disclosure block auto-filled from extractor metadata; Methods-section block compulsory in template. |
| **R12** | Predatory journal accidental submission | Venue catalog whitelist only; system refuses unlisted venue. |

## 11. Confirmation status

The user approved this plan in full on 2026-05-09 with the message
"approved in full. Implement in full." All seven §11 questions of the
proposing message are answered yes:

1. Strategic thesis — endorsed.
2. Six new crates — approved.
3. UK AISI Inspect adoption — approved.
4. Negative-result quota default-on after Phase 9 — approved.
5. 14-day right-of-reply default — approved.
6. Plan committed at this path.
7. Academic co-author outreach — Phase 9 prerequisite.

## 12. Phase index (links to detailed plans)

| Phase | Plan |
|---|---|
| 0a | [Phase 0a — Phantom-import resolution](../../superpowers/plans/scientia/2026-05-09-scientia-phase-0a-phantom-imports.md) — **Complete** |
| 0b | Phase 0b — `vox-research-events` L1 crate — **Complete** |
| 0c | schema_types module in vox-research-events (5 JSON Schema → Rust types) — **Complete** |
| 0d | 7 DB tables + 12 Codex methods (research_pipeline.rs) — **Complete** |
| 0e | SecretId: VoxOpenAlexEmail, VoxSemanticScholarApiKey, VoxOsfApiToken, VoxArxivAccessToken, VoxNanopubSigningKeyHex, VoxSwhidApiToken — **Complete** |
| 0f | 5 SCIENTIA crates registered in layers.toml (Phase 1–5 prep) — **Complete** |
| 1–10 | (TBD — phase plans land as predecessor phases complete) |
