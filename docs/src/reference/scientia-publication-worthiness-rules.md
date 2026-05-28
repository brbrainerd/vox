---
title: "SCIENTIA publication worthiness rules"
description: "Rules and metrics for classifying findings as publishable, evidence-incomplete, or non-publishable."
category: "Language Reference"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "TechArticle"
---

## SCIENTIA publication worthiness rules

This document is the policy/rubric SSOT for deciding whether a finding should be prepared for publication.

Use with:

- `docs/src/architecture/scientia-publication-automation-ssot.md`
- `docs/src/reference/socrates-protocol.md`

## Decision outputs

- `Publish`: finding is sufficiently novel, reproducible, policy-compliant, and evidence-backed.
- `AskForEvidence`: promising but incomplete; requires targeted additional evidence.
- `Abstain/DoNotPublish`: fails hard red lines or has unacceptable integrity/policy risk.

## Hard red lines (automatic `Abstain/DoNotPublish`)

1. Fabricated or unresolved citations used as evidence.
2. Evidence-claim mismatch for core claims (claim not traceable to data/artifact).
3. Undisclosed AI-generated substantive content in venues requiring disclosure.
4. AI listed as author/contributor where prohibited by policy.
5. Disallowed AI-generated figures/images for target venue.
6. Unverifiable benchmark deltas (missing baseline/candidate pair or missing benchmark manifest).
7. Missing reproducibility essentials (cannot replay key result path).
8. Serious contradiction in Socrates gating unresolved at submission time.

## What should not be generated

Never auto-generate without explicit human authorship/verification {

- novelty/significance assertions in the final narrative,
- claims of causal mechanism unsupported by evidence,
- safety/ethics conclusions without explicit reviewed rationale,
- references/citations not machine-verified and human-confirmed,
- figures that imply measured outcomes unless traceably generated from stored artifacts.

## What should be automated

Should be fully automated where possible:

- artifact hashing, manifest/digest updates, provenance tracking,
- metadata normalization and completeness checks,
- policy/profile validation for target venue,
- benchmark evidence pack assembly,
- package scaffolding and static checks,
- adapter payload generation and status polling,
- discrepancy detection (citation validity, claim-evidence linkage, contradiction flags).

## Scientific-worthiness metrics

All metrics are normalized in `[0, 1]` unless stated.

### A. Epistemic rigor

- `claim_evidence_coverage`: proportion of publishable claims with direct evidence links.
- `contradiction_penalty`: derived from Socrates contradiction ratio.
- `abstain_trigger_rate`: frequency of unresolved high-risk claims.

### B. Reproducibility

- `artifact_replayability`: can independent runner reproduce declared primary metrics.
- `config_completeness`: presence of benchmark config, run config, seeds, environment.
- `before_after_pair_integrity`: baseline/candidate comparability completeness.

### C. Novelty and compression (information-theoretic)

- `mdl_gain_proxy`: improvement in explanatory compression relative to baseline model/report.
- `delta_signal_to_noise`: effect size adjusted by variability/instability.
- `non_redundancy_score`: overlap penalty against prior internal findings.

### D. Reliability and operational validity

- `eval_gate_pass_rate`: pass fraction across required gates.
- `run_stability`: repeated-run variance and failure consistency.
- `pipeline_integrity`: no broken ledger/provenance transitions.

### E. Metadata and policy completeness

- `metadata_completeness`: required publication metadata present for target route.
- `ai_disclosure_compliance`: policy-compliant AI usage disclosures present.
- `submission_profile_compatibility`: package/profile fits target venue constraints.

## Threshold policy (default profile)

Hard requirements:

- No hard red-line violation.
- `claim_evidence_coverage >= 0.90`
- `artifact_replayability >= 0.85`
- `before_after_pair_integrity >= 0.90`
- `metadata_completeness >= 0.90`
- `ai_disclosure_compliance = 1.0`

Decision rubric:

- `Publish`:
  - all hard requirements pass, and
  - aggregate score >= `0.85`, and
  - `mdl_gain_proxy` or `delta_signal_to_noise` indicates meaningful advance.
- `AskForEvidence`:
  - no hard red-line violation, but one or more soft thresholds fail.
- `Abstain/DoNotPublish`:
  - any hard red-line violation, or repeated unresolved contradiction, or aggregate score < `0.65`.

## Aggregate score definition

Recommended weighted aggregate:

`worthiness_score = 0.30 * epistemic + 0.25 * reproducibility + 0.20 * novelty + 0.15 * reliability + 0.10 * metadata_policy`

Weights may be profile-specific by venue, but all changes must be versioned and documented.

## Venue profile overlays

### `tmlr_double_blind`

- Require anonymization checks and broader-impact declaration when risk is non-trivial.
- Enforce stricter contradiction handling on factual claims.

### `jmlr_camera_ready`

- Require camera-ready source package compileability and formatting checks.
- Strong reproducibility artifact expectations for experiment-heavy papers.

### `jair_camera_ready`

- Require JAIR template conformance and final source archive readiness.

### `arxiv_direct`

- Require arXiv format/moderation profile checks (machine readability, references, code/data link resolvability).

### `zenodo_archive`

- Require complete deposition metadata and immutable artifact manifest.

## Required evidence pack fields

Each publication candidate must carry:

- finding ID and repository context,
- baseline/candidate run IDs,
- benchmark manifest reference,
- metric deltas with uncertainty/stability context,
- artifact hashes and environment snapshot,
- citation verification report,
- policy gate and preflight report,
- human accountability declaration.

## Human accountability rule

Automation prepares and validates. Humans remain accountable for:

- scientific interpretation and claims,
- ethical framing and broader-impact statements,
- final sign-off on submission materials.

## Governance and drift

- This ruleset is versioned SSOT for publication-worthiness decisions.
- Any threshold or red-line change requires:
  - rationale,
  - expected impact,
  - backward-compatibility note for ongoing publication candidates.

## Machine-readable contract

Canonical contract artifacts for this rubric:

- `contracts/scientia/publication-worthiness.schema.json`
- `contracts/scientia/publication-worthiness.default.yaml`

CI and runtime surfaces:

- `vox ci scientia-worthiness-contract` — schema + invariant check (also nested in `vox ci ssot-drift`).
- `vox scientia publication-worthiness-evaluate --metrics-json <path>` (and `vox db publication-worthiness-evaluate`) — print evaluation JSON from contract + metrics file.
- MCP `vox_scientia_worthiness_evaluate` — same evaluation using repo root + JSON `metrics` (no DB).
- `vox scientia publication-preflight --with-worthiness` / MCP `vox_scientia_publication_preflight` with `with_worthiness: true` — attaches a `worthiness` block. When VoxDb has `socrates_surface` rows for `metadata_json.repository_id` (or MCP server repo id), a live rollup is merged into `metadata_json.scientia_evidence.socrates_aggregate` before scoring. Embed optional `scientia_evidence` (eval-gate, benchmark pair, human attestations) under `metadata_json` for decisions closer to human review (see `crates/vox-publisher/src/scientia_evidence.rs`).

## Social distribution policy overlays

When `metadata_json.scientia_distribution` is present:

- Reddit publish intent requires OAuth-backed identity, explicit User-Agent compliance, and `submit`-scope compatibility checks before live mode.
- Hacker News publish intent must remain `manual_assist` unless the official API surface changes to support write operations.
- YouTube publish intent must enforce privacy-safe defaults (`private`) unless project verification/compliance audit is complete.
- Cross-channel derivations (e.g. YouTube -> Reddit/HN summaries) must preserve claim-evidence alignment and reuse manifest digest context.
- `distribution_policy.channel_policy.<channel>.worthiness_floor` MAY set stricter per-channel thresholds than the global publish floor.
- `distribution_policy.channel_policy.<channel>.topic_filters` SHOULD prevent blanket posting and constrain fan-out to relevant topic tags.
- Topic-to-channel baseline packs are versioned in `contracts/scientia/distribution.topic-packs.yaml`.

## External policy URL appendix

- COPE AI authorship and tooling position: [https://publicationethics.org/cope-position-statements/ai-author](https://publicationethics.org/cope-position-statements/ai-author)
- ICMJE recommendations (AI tools and authorship context): [https://www.icmje.org/recommendations/](https://www.icmje.org/recommendations/)
- Nature Portfolio policy on AI: [https://www.nature.com/nature-portfolio/editorial-policies/ai](https://www.nature.com/nature-portfolio/editorial-policies/ai)
- Elsevier policy for AI-assisted writing: [https://www.nature.com/nature-portfolio/editorial-policies/ai](https://www.nature.com/nature-portfolio/editorial-policies/ai)
- TMLR venue policy context: [https://openreview.net/group?id=TMLR](https://openreview.net/group?id=TMLR)


