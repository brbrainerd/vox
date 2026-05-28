---
title: "Scientia × Mesh/Model-Routing Integration Research (2026)"
description: "Fundamental limitations of the Vox-Scientia publication pipeline and a concrete proposal to close the scientia ↔ mesh/orchestrator feedback loop so that provider and model behaviour become first-class, publishable scientific artifacts."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Identifies the canonical seam between the scientia publication SSOT (ADR-011), the model-orchestration SSOT audit, and telemetry-trust, and specifies the contracts that make model/provider observations a publishable finding class. Authoritative for the next-gen routing feedback loop."
sourced_at: "2026-04-23"
vox_relevance:
  - "vox-scientia-core / vox-publisher: novelty, worthiness, distribution"
  - "vox-orchestrator: ModelRegistry, ScoringWeights, task_routing"
  - "vox-ml-cli / vox-mesh-types: federation, TaskSpec, MeshDirectoryEntry"
  - "vox-db: model_scoreboard, llm_interactions, llm_feedback"
  - "vox-secrets: provider-secret envelope"
  - "vox-socrates-policy: hallucination / contradiction gate"
---

# Scientia × Mesh / Model-Routing Integration Research (2026)

## Status

Proposal / research. Not yet an ADR. Normative once accepted as a supplement to:

- [ADR-011: Scientia Publication Manifest SSOT](../adr/011-scientia-publication-ssot.md)
- [Model Orchestration SSOT Audit (2026-04-20)](model-orchestration-ssot-audit-2026.md)
- [Telemetry Trust (SSoT)](telemetry-trust-ssot.md)
- [Next-Gen Orchestrator Research (2026-04-23)](nextgen-orchestrator-research-2026.md)

Supersedes nothing. Extends the Scientia candidate taxonomy and defines the feedback channel from observed provider behaviour into the routing layer.

---

## Executive Summary

Vox-Scientia today is a **deterministic, contract-driven research-publication pipeline**: crawl feeds → deduplicate → score novelty against federated prior art (OpenAlex / Crossref / Semantic Scholar) → evaluate a multi-weight worthiness gate → compile per-channel distribution → post to Twitter/X, Bluesky, Mastodon, Discord, LinkedIn, Reddit, Hacker News, ResearchGate, GitHub, RSS, YouTube, Zenodo, OpenReview, and stage arXiv for operator submission. Secrets are channelled through vox-secrets. Approvals are digest-bound. The pipeline is unusually disciplined for its category.

Its **fundamental limitation** is that it is a publication pipeline about the *world* and nearly silent about its own *substrate*. Vox runs a mesh (`vox-ml-cli` + `vox-mesh-types`) and an orchestrator (`vox-orchestrator`) that choose among providers, endpoints, and local models every second of every day, and the observations that fall out of those choices — "Gemini 2.0 plans to a depth of three tool calls reliably but hallucinates on four," "this OpenRouter endpoint drifts 300 ms after 23:00 UTC," "Claude 3.5 Haiku's long-context recall on 80k-token Vox source trees holds at 0.86" — never become first-class **discovery signals**, never enter the novelty ledger, never pass through the worthiness gate, and never get published. They also never flow back into the **router scoring function** beyond the existing coarse success-rate / cost / p50-latency rollup.

The asymmetry is stark:

- **Outrospection** (what the world publishes) is federated, structured, and pipelined.
- **Introspection** (what Vox learns about its own substrate and the models it runs) is rolled up into three scalars in `model_scoreboard` and nothing else is persisted in a publishable shape.

This memo proposes three changes that, together, close the loop without displacing the existing SSOT:

1. Introduce **provider / model observations as a first-class `DiscoverySignalFamily`** and add two candidate classes to the finding ledger (`ModelCapabilityAtlas`, `ProviderReliabilityAtlas`).
2. Add a **persistent learned-profile overlay** (`model_profile_learning`) that the router reads during `resolve_model_with_registry_fallbacks` and that Scientia writes during rollup.
3. Add a **new publication output**, the **Vox Provider Atlas** — a periodically-regenerated, digest-bound manifest distributed to arXiv / Zenodo / OpenReview / social channels describing the strengths, weaknesses, and subjective characters of the providers and models the Vox mesh has observed, with full evidence provenance.

All three hook into existing seams. None require breaking the ADR-011 publication manifest, the telemetry trust policy, or the orchestration SSOT audit's FIX items.

---

## Part 1 — What Vox-Scientia Actually Is Today

A concrete, file-referenced map of the current pipeline so the limitations section can be precise rather than impressionistic.

### 1.1 Stage map

| Stage | Owner | Entry point |
|---|---|---|
| Ingest (RSS, arXiv feeds) | `vox-scientia-ingest` | `rss_crawler.rs::FeedCrawler::crawl_all` |
| Deduplication (embedding, optional) | `vox-scientia-ingest` | `deduplicator.rs::IngestDeduplicator::is_duplicate` |
| Discovery ranking | `vox-publisher` | `scientia_discovery.rs` → `DiscoveryCandidateRank`, `intake_gate_allows` |
| Novelty (federated prior art) | `vox-publisher` | `scientia_prior_art.rs` → `NoveltyEvidenceBundleV1` |
| Worthiness gate | `vox-publisher` | `publication_worthiness.rs::PublicationWorthinessContract` |
| Distribution compile | `vox-publisher` | `distribution_compile.rs::compile_for_publish` (SHA3-256 derivation) |
| Manifest | `vox-publisher` | `publication.rs::PublicationManifest` |
| Social adapters | `vox-publisher/adapters/` | twitter, bluesky, mastodon, discord, linkedin, reddit, hn, rg, github, rss |
| Scholarly adapters | `vox-publisher/scholarly/` | zenodo, openreview; `submission/arxiv.rs` = operator handoff |
| Publish gate | `vox-publisher` | `gate.rs::publish_gate_inputs_for_orchestrator` |
| Secrets | `vox-secrets` | `resolve_secret(SecretId::*)` — Vault / Infisical / env |

### 1.2 Novelty is a static heuristic blend

From `crates/vox-publisher/src/scientia_heuristics.rs`:

```text
novelty_blend_lexical:   0.55
novelty_blend_semantic:  0.45
novelty_moderate_threshold: 0.45
novelty_high_threshold:  0.75
rank_novelty_overlap_penalty_max: 12
```

These are loaded from `contracts/scientia/impact-readership-projection.seed.v1.yaml` at startup and do not change in response to publishing outcomes. The novelty score is a blended Jaccard-plus-embedding-overlap against three fixed external indexes. There is no learning, no calibration against "did we regret publishing this later," no per-topic adjustment.

### 1.3 Worthiness is multivariate but also static

From `crates/vox-publisher/src/publication_worthiness.rs`:

```text
epistemic        0.30
reproducibility  0.25
novelty          0.20
reliability      0.15
metadata_policy  0.10
publish_score_min          0.85
claim_evidence_coverage    0.90
artifact_replayability_min 0.85
8 hard red-lines enabled
```

Decision is `Publish | AskForEvidence | AbstainDoNotPublish`. Hard red-lines include fabricated citations, claim-evidence mismatch, undisclosed AI, unresolved contradictions. These are correct. They are also fixed weights that no feedback ever touches.

### 1.4 Introspection vs outrospection surface

**Introspection signals** already exist but are thin. From `scientia_evidence/signals.rs` and `scientia_finding_ledger.rs`:

```text
DiscoverySignalFamily =
  | EvalGate
  | BenchmarkPair
  | TelemetryAggregate     ← closest to "Vox observes itself"
  | TrustRollup
  | MensScorecard          ← already mesh-aware
  | ReproducibilityArtifact
  | Documentation
  | LinkedCorpus
  | OperatorAttestation
  | FindingCandidateSignal
  | Unspecified
```

```text
FindingCandidateClass =
  | AlgorithmicImprovement
  | ReproducibilityInfra
  | PolicyGovernance
  | TelemetryTrust         ← the only fully-introspective class today
  | Other
```

`MensScorecard` exists. It is not consumed by the router. It is not exported as a publishable artifact.

**Outrospection** is federated, strong, and working: `PriorArtSource::{Openalex, Crossref, SemanticScholar, Manual, Other}`, per-source query traces, HTTP fingerprints, recency buckets.

### 1.5 What the mesh and orchestrator currently capture

From `vox-orchestrator`:

- `ModelRegistry::models : HashMap<String, ModelSpec>` with rich `ModelCapabilities` (context, JSON, vision, native-tools, p50 latency, uptime, rate limits, moderation flag).
- `ModelRegistry::scoreboard : HashMap<String, ModelScore>` with `success_rate`, `quality_score`, `p50_latency_ms`, `cost_per_success_usd`.
- `ModelRegistry::penalty_map` for abstention penalties (FIX-12).
- `ScoringWeights` — 20+ static tunables governing `auto_score_model`.
- `AgentTrustScore` — Kalman filter with `(measurement_noise=0.1, process_noise=0.005)` and UCB1 exploration.
- `task_and_flags_to_profile` → `RoutingProfile { Vision, Research, StrictJson, VoxComposer, Planning, RustLangdev, General }`.
- `task_strengths(TaskCategory) -> Vec<StrengthTag>`.

From `vox-ml-cli` / `vox-mesh-types`:

- `TaskKind { TextInfer, ImageGen, SpeechTranscribe, TrainQLoRA, Embed, VoxScript }`.
- `TaskSpec { kind, model_id, min_vram_mb, priority, timeout_secs, payload_b64, required_labels }`.
- `MeshDirectoryEntry { scope_id, control_url, task_kinds, queue_depth, priorities, Ed25519 signature }`.

From `vox-db`:

- `llm_interactions` + `llm_feedback` → `rollup_model_scoreboard()` aggregates success_rate, p50 / p99 latency, cost_per_success, quality_score over sliding windows (per model × task_category × strength_tag).
- `model_pricing_catalog` for observed cost blending.

This is a lot of raw material. Almost none of it reaches Scientia.

---

## Part 2 — Fundamental Limitations

### 2.1 Static novelty in a moving field

The `0.55 / 0.45` lexical/semantic blend is defensible as a cold-start prior. It is indefensible as a terminal posterior. Nothing in the pipeline updates the blend based on observed publish regret, adversarial prior-art discovery after the fact, or shifts in the distribution of arXiv / Crossref corpora. **Novelty is currently a belief, not a calibrated estimator.**

### 2.2 Outrospection is passive and narrow

Ingest is RSS-shaped. Prior art is three academic indexes. The pipeline does not actively probe — it has no equivalent of a "call this endpoint with a known prompt and observe the response" harness that feeds the same evidence types. Outrospection stops at "what did someone else publish that might collide." The richer outrospective question — "how does the world of model providers actually behave when Vox queries it" — is unmeasured by Scientia even though the mesh measures it implicitly on every call.

### 2.3 Introspection has a category but no content

`FindingCandidateClass::TelemetryTrust` exists. There is no parallel class for model or provider behaviour. `DiscoverySignalFamily::MensScorecard` exists and is already used by `infer_candidate_class`, but its output has nowhere to land except `Other`. The schema welcomes introspection; the plumbing doesn't deliver it.

### 2.4 No loop back into routing

The most consequential gap. Scientia already computes things that would materially improve routing decisions:

- Claim-evidence coverage per model output.
- Socrates contradiction ratios per generation.
- Reproducibility of produced artifacts.
- Worthiness / confidence decomposition.

None of these reach `ScoringWeights`, `ModelRegistry::scoreboard`, or the Kalman `AgentTrustScore`. The router's definition of "this model is good" is `success_rate × (1 / latency) × (1 / cost)`. That is not the same thing as "this model is *trustworthy for this class of task*."

### 2.5 The scoreboard is too coarse to be subjective

`ModelScore { success_rate, quality_score, p50_latency_ms, cost_per_success_usd }` cannot distinguish "this model plans in three tool-calls but degrades past five" from "this model plans to any depth but hallucinates file paths" from "this model refuses politely at 2 000 input tokens of our code style." These are the observations that would matter if Vox wanted to guide the field. The current scoreboard flattens them to a single scalar per model × task × window.

### 2.6 Operator attestations are the wrong shape for model claims

Operator attestations are binary (`human_meaningful_advance`, `human_ai_disclosure_complete`) and apply to findings, not to providers. There is no structured place for the human operator to say "DeepSeek is noticeably better at Rust trait bounds than on Python typing" with evidence-provenance attached. Yet this is exactly the kind of expert prior that a calibrated routing system needs.

### 2.7 Telemetry trust is zero-surprise, which is correct — but nothing exports it

Per `telemetry-trust-ssot.md`: telemetry is local-first, no PII in default payloads, remote upload is opt-in. Scientia publications are also local-first and digest-bound. The two policies are aligned. But there is no defined path from a local telemetry rollup to a redaction-safe, publication-worthy atlas. The trust policy is compatible with what we want; it just has no consumer on the Scientia side.

### 2.8 Publications are about *findings*, not *instruments*

Scientific communities publish both results and instrument characterizations (telescope throughput curves, MRI pulse sequences, CPU benchmark suites). Vox-Scientia can publish findings. It cannot publish instrument characterizations — and in the AI field, the instruments *are* the models and their providers. This is the output the ecosystem lacks most acutely and that Vox is best positioned to produce because it already satisfies the digest-binding, claim-evidence, and reproducibility rails that a credible publication needs.

### 2.9 Known, contained limitations not central to this proposal

For completeness, the exploration also surfaced:

- Venue profile `required_checks` in `publication_worthiness.rs` are advisory-only; no runtime enforcement.
- YouTube and Reddit adapters are feature-gated (`scientia-youtube`, `scientia-reddit`); completeness varies per build.
- arXiv is operator-handoff, not a direct submission.
- `VOX_SCHOLARLY_JOB_LOCK_OWNER` implies distributed job coordination with no documented protocol.

These are tractable; this memo does not address them.

---

## Part 3 — The Missing Loop

Drawn as prose because the structure is small:

> Every routed call in `vox-orchestrator` produces an outcome row in `llm_interactions`. The rollup collapses these into `model_scoreboard`. Scientia never reads this table. Every Scientia evaluation produces a worthiness decision, contradiction ratios, reproducibility artifacts, and a confidence decomposition. The router never reads these. Both sides keep journals. Neither side reads the other's journal.

The proposed loop is three edges, not one:

- **Edge A (router → scientia):** telemetry rollups + per-call outcomes become `DiscoverySignalFamily::MensScorecard` and a new `ProviderObservation` family, feeding a new finding candidate class.
- **Edge B (scientia → router):** a learned-profile overlay persists Scientia-classified model / provider traits, which the router consults at selection time, adjusting `ScoringWeights` or filtering candidates.
- **Edge C (scientia → world):** a new publication output (the Vox Provider Atlas) distributes the learned profiles with full provenance through the existing adapter stack.

---

## Part 4 — Proposed Architecture

### 4.1 First-class provider observations

Extend `DiscoverySignalFamily` (`crates/vox-publisher/src/scientia_evidence/signals.rs`) with:

- `ProviderObservation` — an observation about a specific provider / endpoint / model arising from a Vox call (latency, refusal, tool-call malformation, context-truncation behaviour, JSON-mode violation, cost deviation, quota shape). Provenance must include `model_id`, `provider_id`, `endpoint`, `task_category`, `strength_tag`, Vox commit SHA, and the vox-secrets secret fingerprint (not the secret).
- `ModelCapabilityEvidence` — a claim about a model's capability at a specific task/strength, supported by evidence rows (benchmark pair id, Socrates run id, telemetry aggregate id). This is the aggregated, de-noised shape, not a raw call.

Both feed `infer_candidate_class` (`scientia_finding_ledger.rs::infer_candidate_class`, currently at line ~154).

### 4.2 New finding candidate classes

Extend `FindingCandidateClass` with:

- `ModelCapabilityAtlas` — a finding whose primary subject is a model's strengths and weaknesses across a mapped task/strength space.
- `ProviderReliabilityAtlas` — a finding whose primary subject is a provider's or endpoint's reliability, quota dynamics, or governance behaviour (moderation, refusal patterns) over time.

Both are eligible for the worthiness gate. Both use the existing `NoveltyEvidenceBundleV1` shape — "novelty" for a provider atlas means "what has not already been published by existing benchmark consortia (HELM, OpenLLM Leaderboard, Artificial Analysis)" — which is a natural extension of the prior-art federation and does not require schema changes.

### 4.3 Learned-profile overlay (`model_profile_learning`)

A new Codex / libSQL table owned by `vox-db`:

```text
model_profile_learning
  model_id              TEXT     -- FK-ish into ModelRegistry
  provider_id           TEXT
  endpoint_id           TEXT NULL
  task_category         TEXT
  strength_tag          TEXT
  trait_key             TEXT     -- e.g. "plans_3_tool_calls_reliably"
  trait_value           TEXT     -- JSON-encoded scalar or enum
  support_level         TEXT     -- strong | supporting | informational
  evidence_bundle_id    TEXT     -- joins scientia novelty / finding ledger
  kalman_mean           REAL     -- optional; mirrors AgentTrustScore shape
  kalman_variance       REAL     -- optional
  sample_n              INTEGER
  window_days           INTEGER
  observed_at_ms        INTEGER
  stale_after_ms        INTEGER
  scientia_commit_sha   TEXT     -- provenance of the classifier that wrote this
```

Key design points:

- **Row-level provenance.** Every row carries an `evidence_bundle_id` so the router can explain a routing change ("you picked Claude because Scientia bundle `nb:sha3:…` raised its planning profile").
- **Subjective traits encoded as structured enums.** Not a free-text LLM output. The classifier (Scientia) produces a constrained enum set defined in `contracts/scientia/provider-atlas.schema.v1.yaml` (new).
- **Time-bounded.** `stale_after_ms` lets the router prefer recent observations without permanent drift.
- **Kalman fields are optional.** They make it a drop-in for the existing `AgentTrustScore` pattern (`attention/routing.rs:52–94`) if we want per-skill filtered beliefs.

### 4.4 Router integration seams

From the exploration, the cleanest seams are in `vox-orchestrator`:

- `ModelRegistry::inject_scoreboard()` and `inject_pricing_catalog()` — already the canonical way to hydrate dynamic data into the registry. Add `inject_learned_profiles()` alongside them. No trait change; pure addition.
- `ScoringWeights` — already 20+ fields. Add weights for learned-trait bonuses and penalties (`learned_strength_bonus`, `learned_weakness_penalty`, `learned_context_accuracy_bonus`, `learned_tool_call_reliability_bonus`, `provider_reliability_bonus`).
- `auto_score_model` / `resolve_model_with_registry_fallbacks` (`dei_shim/selection/resolve.rs:80`) — extend scoring to consult learned profiles with the new weights.
- `record_penalty` — extend signature with `LearningContext` so abstention / refusal recordings carry task / strength tags and feed back into Scientia's `ProviderObservation` family rather than being a local-only penalty.
- `AgentTrustScore` — reuse for per-(model, task, strength) skill vectors, not only for agent-level trust.

Existing tier-2 trust (Kalman + UCB) and tier-3 mesh federation (signed `MeshDirectoryEntry`) do not need changes for this proposal.

### 4.5 Active probing harness (outrospection of the model ecosystem)

A new sub-crate, **`vox-scientia-probe`** (or a module inside `vox-scientia-ingest` if we want to avoid a crate split early):

- Canonical probe prompts stored under `contracts/scientia/provider-probes/` (tool-call chain, long-context recall, strict-JSON, refusal patterns, planning-depth, Rust trait bounds, Vox-source comprehension).
- Runs each probe across eligible providers / endpoints on a schedule (`scripts/scientia-probe.vox`, per [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md)).
- Emits a `ProviderObservation` signal per (probe, provider, model, endpoint, timestamp).
- Digest-binds the probe set (prompt corpus + tooling) so the atlas can cite a specific probe-suite version.

This is where we earn the right to claim calibration: the router's behaviour is not the only thing being observed; it is being actively exercised against known instruments.

### 4.6 The Vox Provider Atlas (new publication output)

A new `PublicationManifest` variant — not a schema break, a new content shape with its own topic pack:

- **Cadence:** quarterly candidate, monthly draft, weekly internal dashboard snapshot.
- **Content:** a digest-bound report with per-provider, per-model, per-task-strength summaries; claim-evidence table; reproducibility manifest pointing to probe-suite versions, probe results, and the Scientia worthiness decision.
- **Channels:**
  - *Scholarly:* Zenodo (authoritative artifact), arXiv (operator handoff today; CI-Submit once ADR) — category `cs.LG` with cross-list `cs.SE` where appropriate. OpenReview for discussion.
  - *Social:* Twitter/X thread (capability snapshot), Mastodon (long), Bluesky (medium), Reddit (r/LocalLLaMA-style with responsible moderation), Hacker News (Show HN for each quarterly), YouTube community post with an accompanying explainer video (once YouTube adapter is promoted from feature-gate).
  - *RSS:* canonical atlas feed for automated subscribers.
- **Tone:** first-person-plural, mechanistic, evidence-forward. No marketing affect. "We observed" + bundle id + numbers.

This is the "help guide the field of AI forward" deliverable, constructed on top of rails that already enforce claim-evidence coverage, no-fabricated-citations, and digest-bound approvals.

### 4.7 The other direction — Scientia improves Vox itself

The same overlay enables:

- **Auto-calibration of worthiness weights.** After N published findings, a Scientia job correlates `publish_score` with downstream `citation_count`, `peer_mention_count`, and operator overrides. Adjusts `ScientiaHeuristics` weights with a bounded learning rate, proposes a diff against `contracts/scientia/impact-readership-projection.seed.v1.yaml`, and opens a reviewable PR. Never self-merges.
- **Commit-history novelty.** A scheduled `.vox` script walks the last N commits (via `vox-git`) and emits `DiscoverySignalFamily::TelemetryAggregate` plus `ProviderObservation` where commits reference routing changes, model upgrades, or new probe results. This produces "Vox release-notes as science" with real novelty scoring.
- **Self-awareness gate for routing changes.** Any change to `ScoringWeights` defaults passes through the worthiness gate before merge, because it is now a publishable claim about routing behaviour.

---

## Part 5 — Executable Proposal (trait sketches, schema stubs)

These signatures are Rust sketches that match existing file conventions and can be dropped into the named files. They are deliberately minimal — the point is to show the seam, not to pre-own the detail.

### 5.1 Extend `DiscoverySignalFamily`

File: `crates/vox-publisher/src/scientia_evidence/signals.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySignalFamily {
    // existing variants retained
    EvalGate,
    BenchmarkPair,
    TelemetryAggregate,
    TrustRollup,
    MensScorecard,
    ReproducibilityArtifact,
    Documentation,
    LinkedCorpus,
    OperatorAttestation,
    FindingCandidateSignal,
    Unspecified,

    // new
    ProviderObservation,
    ModelCapabilityEvidence,
}
```

### 5.2 Extend `FindingCandidateClass`

File: `crates/vox-publisher/src/scientia_finding_ledger.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FindingCandidateClass {
    AlgorithmicImprovement,
    ReproducibilityInfra,
    PolicyGovernance,
    TelemetryTrust,

    // new
    ModelCapabilityAtlas,
    ProviderReliabilityAtlas,

    #[default]
    Other,
}
```

Update `infer_candidate_class` to route the new signal families: a bundle dominated by `ProviderObservation` → `ProviderReliabilityAtlas`; one dominated by `ModelCapabilityEvidence` (with supporting `BenchmarkPair` / `MensScorecard`) → `ModelCapabilityAtlas`.

### 5.3 New contract: `provider-atlas.schema.v1.yaml`

File: `contracts/scientia/provider-atlas.schema.v1.yaml` (new)

Defines the closed enum set of subjective trait keys (e.g. `plans_tool_calls_depth`, `long_context_recall_80k`, `refuses_with_justification`, `json_mode_compliance`, `cost_drift_per_24h`), their value types, and the minimum evidence each trait requires before it can be published (echoing the `DiscoverySignalStrength` tiers).

Paired JSON schema: `contracts/scientia/provider-atlas.v1.schema.json`.

### 5.4 Extend `ScoringWeights`

File: `crates/vox-orchestrator/src/dei_shim/selection/weights.rs`

```rust
#[derive(Debug, Clone)]
pub struct ScoringWeights {
    // ... existing fields retained verbatim ...

    // new: Scientia-informed learned profiles
    pub learned_strength_bonus: f64,
    pub learned_weakness_penalty: f64,
    pub learned_context_accuracy_bonus: f64,
    pub learned_tool_call_reliability_bonus: f64,
    pub provider_reliability_bonus: f64,
    pub provider_drift_penalty: f64,
}
```

Defaults chosen conservatively so the initial effect of enabling the overlay is near-zero; runbook for tuning lives alongside the rollup job.

### 5.5 New `ModelRegistry` injection seam

File: `crates/vox-orchestrator/src/models/registry.rs`

```rust
impl ModelRegistry {
    /// Hydrate the registry with Scientia-classified learned profiles.
    /// No-op if `profiles` is empty. Rows with `stale_after_ms < now` are ignored.
    pub fn inject_learned_profiles(
        &mut self,
        profiles: Vec<LearnedProfileRow>,
    ) -> usize { /* returns rows applied */ }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnedProfileRow {
    pub model_id: String,
    pub provider_id: String,
    pub endpoint_id: Option<String>,
    pub task_category: TaskCategory,
    pub strength_tag: StrengthTag,
    pub trait_key: ProviderAtlasTraitKey,   // constrained enum from provider-atlas.v1
    pub trait_value: serde_json::Value,
    pub support_level: DiscoverySignalStrength,
    pub evidence_bundle_id: String,
    pub kalman_mean: Option<f64>,
    pub kalman_variance: Option<f64>,
    pub sample_n: u32,
    pub window_days: u16,
    pub observed_at_ms: i64,
    pub stale_after_ms: i64,
    pub scientia_commit_sha: String,
}
```

`auto_score_model` consults the overlay through a deliberate, small indirection: `resolve_model_with_registry_fallbacks` receives a `&LearnedProfileLens` (read-only view over the registry's overlay) and passes it to the scorer, which applies the new `ScoringWeights` fields. This keeps the overlay optional and testable.

### 5.6 Extend `record_penalty`

File: `crates/vox-orchestrator/src/models/registry.rs`

```rust
impl ModelRegistry {
    pub fn record_penalty_with_context(
        &mut self,
        model_id: &str,
        task: TaskCategory,
        context: LearningContext,
    );
}

#[derive(Debug, Clone)]
pub struct LearningContext {
    pub strength_tag: StrengthTag,
    pub reason: PenaltyReason, // enum: Abstain, ToolCallMalformation, JsonModeViolation, Timeout, ContextOverflow, ProviderQuota, RefusalWithJustification
    pub endpoint_id: Option<String>,
    pub evidence_digest: Option<String>, // SHA3-256 of the request/response envelope (redacted)
}
```

This is what Scientia later pulls through the `llm_interactions` pre-rollup hook and classifies into `ProviderObservation` signals.

### 5.7 Rollup hook

File: `crates/vox-db/src/store/ops_scientia.rs`

Wrap `rollup_model_scoreboard()` with a Scientia classifier pass:

```rust
pub async fn rollup_model_scoreboard_with_scientia(
    db: &Db,
    window_days: u16,
    classifier: &dyn ScientiaObservationClassifier,
) -> anyhow::Result<RollupReport> {
    // 1. read fresh llm_interactions + llm_feedback
    // 2. classifier.classify(rows) -> Vec<ProviderObservation>
    // 3. upsert into provider_observation_ledger (new table)
    // 4. call rollup_model_scoreboard() as today
    // 5. if enough novel observations accumulated, emit a FindingCandidateV1
    //    of class ModelCapabilityAtlas / ProviderReliabilityAtlas
}
```

`ScientiaObservationClassifier` lives in `vox-scientia-core` so it can be composed without pulling `vox-publisher` into `vox-db`.

### 5.8 Publication-manifest variant (no schema break)

File: `crates/vox-publisher/src/publication.rs`

`PublicationManifest::metadata_json` already carries `topic_pack`, tags, and syndication config. Add a new topic pack `"provider_atlas"` declared in `contracts/scientia/distribution.topic-packs.yaml`. The atlas publication uses:

- Its own channel plan (arXiv operator handoff, Zenodo direct, OpenReview, Twitter/X, Mastodon, Bluesky, HN, RSS).
- A content SHA3 over the atlas body plus the `provider-atlas.v1` snapshot used to render it — so the digest binds both the claims and the instrument definitions.
- A worthiness gate that in addition to the default eight red-lines adds:
  - `provider_observation_minimum_n` — reject atlas entries backed by fewer than N observations.
  - `provider_attribution_complete` — every claim must name the provider, model, endpoint, and probe-suite version.
  - `probe_suite_digest_pinned` — the probe suite's content hash must be referenced in the manifest.

### 5.9 Automation — `.vox` scripts

Per [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md) and [`CLAUDE.md`](../../../CLAUDE.md), the automation is authored as Vox, not Python or shell:

- `scripts/scientia/probe-run.vox` — execute the probe suite against a configurable provider list.
- `scripts/scientia/profile-rollup.vox` — nightly classifier pass; hydrates `model_profile_learning`.
- `scripts/scientia/atlas-draft.vox` — monthly atlas draft build; runs preflight, emits `FindingCandidateV1`.
- `scripts/scientia/atlas-publish.vox` — gated by `VOX_NEWS_PUBLISH_ARMED` and dual-approver digest (matches `gate.rs`).

Each is type-checked by `vox check` and telemetered under `vox.script.*`.

---

## Part 6 — Telemetry Contract Additions

Per `docs/src/reference/telemetry-metric-contract.md` and the trust SSoT, additions must be local-first and redaction-safe.

New `metric_type` prefixes on the existing `research_metrics` table (no schema change):

- `scientia:probe:<probe_id>` — one row per probe-run result.
- `scientia:provider_obs:<provider_id>` — aggregated observation; metric_value carries the Kalman mean for the primary trait; metadata_json holds the full provenance bundle reference.
- `scientia:atlas_draft:<atlas_id>` — one row per draft build with worthiness decision.

All `metadata_json` payloads must remain under the 256 KiB cap (trivial for these). None include the request or response bodies — only bundle references, digests, and summary statistics. Bodies remain in the redaction-gated local evidence store.

Remote upload remains opt-in per ADR-023. The atlas publication itself is the intended public channel; raw telemetry is never implicitly exported.

---

## Part 7 — Risks and Open Questions

**R1 — Calibration lag.** The auto-calibration of worthiness weights is tempting and dangerous. Proposed mitigation: never self-merge; always open a PR. Bounded learning rate (e.g. 1 % per window). Canonical weights remain in-repo and reviewable.

**R2 — Publishing subjective claims about commercial providers.** Atlas content has reputational effect on third parties. Proposed mitigation: (a) claims grounded in probe-suite digests; (b) per-provider right-of-reply surface in the manifest (operator-attestable); (c) refusal to publish when `provider_attribution_complete` fails; (d) explicit version pinning of probe corpus and Vox commit — an atlas is a snapshot, not a verdict.

**R3 — Drift between observed and published capabilities.** Providers update models silently. Proposed mitigation: `stale_after_ms` on learned profiles, atlas cadence shorter than typical silent-update interval, explicit "last-verified-at" on every trait in the atlas body.

**R4 — Probe contamination.** Canonical probes leak into training data and stop measuring what they measured. Proposed mitigation: maintain a rotating private probe set in a sealed envelope; publish only aggregates; periodically rotate.

**R5 — Coverage gaps.** Models Vox rarely routes to will have thin evidence. Atlas should state coverage explicitly and abstain on low-n claims (existing `AbstainDoNotPublish` machinery suffices).

**R6 — Schema growth pressure.** The closed-enum trait list will want to expand. Proposed mitigation: treat `ProviderAtlasTraitKey` as versioned; only additive changes without a version bump; removals or renames require an ADR and an atlas-schema ADR pointer in the next atlas.

**R7 — Kalman filter misuse.** Using the existing `AgentTrustScore` Kalman parameters (`measurement_noise=0.1, process_noise=0.005`) may not suit per-trait skill scoring. Proposed mitigation: per-trait noise parameters in `provider-atlas.schema.v1.yaml`; empirical calibration against held-out observations before enabling weights > 0 in production routing.

**R8 — Interaction with ADR-011 digest binding.** The atlas extends the digest to include the probe-suite hash. This is new surface. Proposed mitigation: explicit ADR amendment once the design stabilizes (not blocking the research memo).

---

## Part 8 — Implementation Phases

Ordered to deliver signal as early as possible without breaking existing invariants. None of these are glue scripts — they are first-class work items authored as `.vox` where scripts apply.

**Phase 0 — Observability only (1–2 weeks).**

- Add the two `DiscoverySignalFamily` variants and two `FindingCandidateClass` variants behind a Cargo feature (`scientia-provider-atlas`). No writes yet.
- Add `provider_observation_ledger` table (migration only, no producers).
- Wire `record_penalty_with_context` alongside the existing `record_penalty`; deprecate the old signature only after parity.

**Phase 1 — Passive introspection (2–3 weeks).**

- Implement `ScientiaObservationClassifier` in `vox-scientia-core` over `llm_interactions`.
- Run `rollup_model_scoreboard_with_scientia` in shadow mode (writes to ledger, does not influence router).
- Build the internal dashboard view in `vox-dashboard`.

**Phase 2 — Active outrospection (2–3 weeks).**

- Land `vox-scientia-probe` module, probe contracts, and the `.vox` schedule script.
- Pin initial probe suite digest. Rotate monthly.

**Phase 3 — Loop closure (2–3 weeks).**

- Implement `model_profile_learning` overlay and `inject_learned_profiles()`.
- Turn on the new `ScoringWeights` fields with conservative defaults (near-zero influence) behind a vox-secrets-flagged feature key.
- A/B compare routing decisions with and without the overlay on a held-out task set.

**Phase 4 — First published atlas (2–4 weeks).**

- Draft the `provider_atlas` topic pack, distribution plan, and worthiness red-line additions.
- Stage an internal-only atlas; run the full preflight and worthiness gate.
- Publish v1 atlas to Zenodo, OpenReview, and the social channels; hand off to arXiv via the existing operator flow. Solicit right-of-reply.

**Phase 5 — Self-improvement feedback (ongoing).**

- Turn on worthiness-weight PR proposer against published-then-regretted findings.
- Turn on commit-history novelty publisher once stable.

---

## Part 9 — Relation to Existing SSOTs

- **ADR-011 Scientia Publication SSOT** — extended, not modified. New topic pack + new candidate class + new red-lines live under the existing manifest shape.
- **Model Orchestration SSOT Audit (FIX backlog)** — this proposal adds seams that several FIX items already anticipate (per-task scoreboard granularity, learned-penalty extension, evidence-backed routing). Where a FIX collides with a seam here, the SSOT audit wins; this memo adjusts.
- **Telemetry Trust (SSoT)** — fully preserved. All new metric types ride `research_metrics` within the existing row contract. Remote upload remains opt-in.
- **ADR-005 Socrates Anti-Hallucination** — reused as-is. Contradiction ratios become first-class evidence in provider observations.
- **ADR-023 Optional Telemetry Remote Upload** — referenced for the atlas publication path; the atlas is a deliberate, operator-approved public artifact, not a telemetry sink change.
- **Next-Gen Orchestrator Research (2026-04-23)** — strong agreement on "the orchestration last mile" being the locus of value; this memo operationalizes one of its implications (evidence-backed routing) along a specific seam.

## Related Files (quick jump)

- `crates/vox-publisher/src/scientia_evidence/signals.rs` — extend `DiscoverySignalFamily`
- `crates/vox-publisher/src/scientia_finding_ledger.rs:14` — extend `FindingCandidateClass`
- `crates/vox-publisher/src/scientia_heuristics.rs` — future home for auto-calibration proposer
- `crates/vox-publisher/src/publication_worthiness.rs:16` — red-line additions for atlas
- `crates/vox-publisher/src/publication.rs:39` — topic pack consumption
- `crates/vox-publisher/src/distribution_compile.rs:84` — digest now includes probe-suite hash
- `crates/vox-orchestrator/src/dei_shim/selection/weights.rs` — extend `ScoringWeights`
- `crates/vox-orchestrator/src/dei_shim/selection/resolve.rs:80` — consult `LearnedProfileLens`
- `crates/vox-orchestrator/src/models/registry.rs` — new `inject_learned_profiles`, `record_penalty_with_context`
- `crates/vox-orchestrator/src/attention/routing.rs:52` — Kalman pattern reused per-trait
- `crates/vox-db/src/store/ops_scientia.rs:105` — `rollup_model_scoreboard_with_scientia`
- `contracts/scientia/provider-atlas.schema.v1.yaml` (new)
- `contracts/scientia/provider-atlas.v1.schema.json` (new)
- `contracts/scientia/distribution.topic-packs.yaml` — add `provider_atlas`
- `scripts/scientia/*.vox` (new, authored per VoxScript-First)

## Changelog

- 2026-04-23 — Initial draft (research status).
