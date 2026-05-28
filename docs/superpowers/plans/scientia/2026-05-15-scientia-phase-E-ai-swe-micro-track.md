# SCIENTIA Phase E — AI/SWE Micro-Publication Track (Non-Atlas)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Status:** outline.

**Goal:** Stand up a publication track for individual `algorithmic_improvement`, `reproducibility_infra`, `telemetry_trust`, and `policy_governance` findings that does **not** route through the quarterly Provider Atlas — with per-class venue mappings, per-class artifact-format profiles, and per-class reply-window defaults.

**Architecture:** Mostly configuration + per-class profile data. Three new artifacts: (1) a per-class entry in the venue catalog mapping each `FindingCandidateClass` to a ranked venue list; (2) a per-class artifact-format profile under `contracts/scientia/route-profile-requirements.v1.yaml` declaring section-weights, figure norms, and reproducibility-package shape; (3) a per-class default reply-window length and negative-result quota. The journal-fit recommender (Finalization Phase 8) is extended to read these mappings. No new crates.

**Tech Stack:** YAML config; existing `vox-research-events::FindingCandidateClass`; existing journal-fit recommender; existing venue catalog.

**Strategic context:** [Gap-map §2 Gap E](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-e--aiswe-micro-publication-track-non-atlas); [Finalization Plan Phase 8](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#phase-8--scholarly-automation--venue-strategy-3-wk--complete-2026-05-09).

**Out of scope:**
- New scholarly adapters (existing arXiv/OSF/Crossref/Zenodo/OpenReview cover this).
- IMRaD scaffolder per-class variants — Phase C provides a single template; per-class section-weights are Phase E config that the scaffolder consumes.
- Bespoke venue submission formats requiring new adapters.

---

## File inventory

| Action | Path | Responsibility |
|---|---|---|
| Modify | `contracts/scientia/venue-catalog.v1.yaml` | Add per-class default venue mapping |
| Modify | `contracts/scientia/route-profile-requirements.v1.yaml` | Per-class artifact-format profile |
| Create | `contracts/scientia/finding-class-defaults.v1.yaml` | Per-class reply-window, negative-result quota, critic-allowed |
| Create | `contracts/scientia/finding-class-defaults.schema.json` | Schema for above |
| Modify | `crates/vox-publisher/src/scholarly/recommend.rs` (or current home of journal-fit recommender) | Read per-class defaults |
| Modify | `crates/vox-publisher/src/atlas_gate.rs` (or current home of `AtlasSubmissionGate`) | Make Atlas-specific quotas not apply to non-Atlas tracks |
| Modify | `crates/vox-manuscript-scaffold/src/section_tree.rs` (Phase C) | Consume per-class section-weights |
| Modify | `docs/src/reference/scientia-publication-playbook.md` | Per-class submission walkthroughs |

LoC budget: mostly YAML; ~400 LoC of code changes; ~200 LoC tests.

---

## Tasks (headings only)

### Task E1: Per-class venue mapping

For each `FindingCandidateClass` value:

| Class | Primary venues | Secondary | Notes |
|---|---|---|---|
| `algorithmic_improvement` | ICSE, FSE, OOPSLA, PLDI | TOPLAS, TSE | code-artifact required; ACM badges |
| `reproducibility_infra` | REP, MSR, ICSE-SEIP | EMSE | replay-pack required (Phase B output is the input) |
| `telemetry_trust` | MLSys, SOSP workshops, USENIX-ATC | Distill-style web-native | longitudinal data; right-of-reply per provider |
| `policy_governance` | AIES, FAccT | CHI workshops | mandatory ethics statement; CRediT roles strict |
| `ModelCapabilityAtlas` | IMC, MLSys (existing Atlas track) | — | unchanged; this is the existing path |
| `ProviderReliabilityAtlas` | IMC (existing Atlas track) | — | unchanged |

### Task E2: Per-class artifact-format profile
Section weights (Intro:Methods:Results:Discussion proportions), figure norms, expected reproducibility-pack contents, abstract length, page limit hints.

### Task E3: Per-class defaults file
`finding-class-defaults.v1.yaml`:
```yaml
algorithmic_improvement:
  reply_window_days: 7      # SWE venues use shorter windows
  negative_result_quota: 0  # micro-track does not enforce 3:1
  critic_allowed: true      # per-venue further refined in catalog
reproducibility_infra:
  reply_window_days: 7
  negative_result_quota: 0
  critic_allowed: true
telemetry_trust:
  reply_window_days: 14     # longer; provider implications
  negative_result_quota: 0
  critic_allowed: false     # provider claims need human approver
policy_governance:
  reply_window_days: 14
  negative_result_quota: 0
  critic_allowed: false
ModelCapabilityAtlas:
  reply_window_days: 14     # unchanged
  negative_result_quota: 3  # unchanged Atlas behavior
  critic_allowed: false
ProviderReliabilityAtlas:
  reply_window_days: 14
  negative_result_quota: 3
  critic_allowed: false
```

### Task E4: AtlasSubmissionGate scope-narrowing
Refactor `AtlasSubmissionGate` so its Atlas-specific quota only applies when `candidate_class ∈ {ModelCapabilityAtlas, ProviderReliabilityAtlas}`. Other classes pass through their per-class defaults.

### Task E5: Journal-fit recommender extension
Recommender input: candidate class + worthiness signals. Output: top-3 venues from the per-class mapping, ranked by signal-fit.

### Task E6: Tests
- `algorithmic_improvement` candidate routes to ICSE/FSE/OOPSLA/PLDI ranked list.
- Atlas quota does NOT apply to `algorithmic_improvement`.
- `telemetry_trust` candidate has `critic_allowed: false` so solo-critic path (Phase D) refuses for this class.

### Task E7: Documentation
- Playbook walkthroughs per class.
- SSOT handbook entry summarizing the per-class behavior.

---

## Acceptance criteria

1. `cargo test` green across all touched crates.
2. Journal-fit recommender returns correct per-class venue rankings.
3. Atlas-specific gates don't apply to non-Atlas candidate classes.
4. Per-class config is YAML-driven; adding a class doesn't require code changes beyond enum extension.
5. Solo-critic (Phase D) compatibility surfaces correctly for the classes where it's allowed.

---

## Open questions

- **OQ-E1.** Negative-result quota inheritance. Should the micro-track have its own quota (e.g. 5:1 instead of 3:1) or zero? Recommendation in defaults file: zero — the rationale being that micro-publications are higher-volume, lower-stakes, and the publication-bias defense is venue-mediated (the venues already accept null results).
- **OQ-E2.** Multi-class candidates. Can one finding be both `algorithmic_improvement` and `reproducibility_infra`? Recommendation: primary + secondary class fields; routing follows primary.
- **OQ-E3.** Critic-allowed default. The defaults above are guesses. Worth a separate ethics review before shipping.
- **OQ-E4.** Per-venue style transforms. Each venue has its own paper template. Phase E config or per-venue separate config? Recommendation: per-venue separate config under `contracts/scientia/venues/<venue>/style-profile.yaml`; out of scope for Phase E proper.

---

## Dependencies

- **Upstream:** Phase C (IMRaD scaffolder) — soft dep; Phase E config is consumed by Phase C if both ship. Phase 8 (venue catalog, recommender) — ✅.
- **Downstream:** None hard.

---

## Cross-references

- Gap: [gap-map §2 Gap E](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-e--aiswe-micro-publication-track-non-atlas)
- Venue catalog: [`contracts/scientia/venue-catalog.v1.yaml`](../../../../contracts/scientia/venue-catalog.v1.yaml)
- Route profiles: [`contracts/scientia/route-profile-requirements.v1.yaml`](../../../../contracts/scientia/route-profile-requirements.v1.yaml)
- Candidate classes: `crates/vox-research-events/src/schema_types.rs`
