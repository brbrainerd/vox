---
title: "SCIENTIA Self-Publication Gap Map (2026)"
description: "Audit of what is still missing in Vox SCIENTIA after Finalization Plan Phases 0–10, mapped to the developer's end-to-end self-publication user journey, with priority and dependencies."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-15"
training_eligible: true
training_rationale: "Identifies the remaining holes between the Finalization Plan and a complete user-facing self-publication workflow; informs the next round of phase plans."
---

# SCIENTIA Self-Publication Gap Map (2026)

> **Companion to:** [SCIENTIA Self-Publication Finalization Plan
> (2026)](./scientia-self-publication-finalization-plan-2026.md). That plan's
> Phases 0–10 are marked complete as of 2026-05-09. This document does **not**
> re-spec those phases — it inventories what is still missing for a developer
> on this codebase to go from `git log` to a published, citable artifact in a
> single coherent flow.
>
> **Per-gap implementation plans:** See [Phase
> index](../../superpowers/plans/scientia/2026-05-15-scientia-self-publication-phase-index.md)
> for outline plans A–H. Each plan is promoted from outline to detailed
> TDD-step fidelity when it becomes next-to-execute, matching the Finalization
> Plan's §12 rhythm.

## 0. Orientation: what the Finalization Plan already provides

To prevent redesign, the following are out of scope for this gap map because
they are built and complete per the Finalization Plan:

- Signal ladder T0→T4 with rigor gates ([§2 of the
  plan](./scientia-self-publication-finalization-plan-2026.md#2-the-signal-ladder)).
- Claim extraction: VeriScore + atomic + span integrity + MiniCheck +
  calibrated ABSTAIN (`crates/vox-claim-extractor/`).
- Pre-registration as signed code object with deviation detection
  (`crates/vox-prereg/`).
- Nanopublication emission and RO-Crate 1.2 packaging (`crates/vox-nanopub/`,
  `crates/vox-ro-crate/`).
- Atomic-NEI novelty against time-bounded corpus, SPECTER2 retrieval,
  ChronoFact, EvidenceConflict family (`crates/vox-inspect-bridge/`).
- AISI Inspect adapter; symbolic verifiers wired into
  `confidence_fusion.rs`.
- 14-day right-of-reply window, retraction nanopubs, living-review version
  DOIs, COPE retraction workflow.
- Scholarly adapters: arXiv, OSF, Crossref deposit, Zenodo versioning,
  OpenReview revision, ORCID PKCE OAuth, F1000 publish-then-review.
- AtlasSubmissionGate with negative-result quota; venue catalog whitelist;
  predatory-journal refusal.
- AI-disclosure block auto-fill; CRediT + COI declarations.
- Format adaptation for short-form (Bluesky, schematic-only figures).

The Provider Atlas longitudinal AI-epidemiology paper is the plan's terminal
artifact — but it is **one** publication track, not the user-journey
substrate.

## 1. The end-to-end user journey today (with gaps marked)

A developer working in this codebase wanting to self-publish takes this path.
Steps marked **(gap N)** are not yet served by a single command or surface.

1. **Develop normally.** Commits land, CI runs benchmarks, telemetry flows,
   training jobs finish. ✅ *(observed by existing infra)*
2. **"Is any of this publishable?"** → **(gap A)** No `vox scientia scout`
   exists. Discovery scan operates on already-prepared manifests, not on raw
   repo/CI/telemetry state.
3. **"What candidate did I just produce?"** → **(gap A continued, gap F)**
   The `finding-candidate.v1` ledger row schema exists; the *producers* that
   mine commit graph, benchmark CI history, MENS training runs, and
   Vox-internal telemetry to emit candidate rows are thin.
4. **Pre-register the hypothesis** ✅ *(Phase 2 complete)*.
5. **Run the experiment.** ✅
6. **"Did the result reproduce?"** → **(gap B)** `artifact_replayability` is
   in the worthiness rubric as a `[0,1]` metric, but appears operator-asserted.
   No runner re-executes the RO-Crate in a clean sandbox and writes the
   *measured* score back.
7. **Promote claims through the signal ladder.** ✅
8. **"Draft the paper."** → **(gap C)** Phase 7 covers short-form
   (Bluesky/schematic). No long-form IMRaD scaffolder that fills only the
   safe slots from RO-Crate + verified claims and leaves provenance-bound
   TODOs for the human narrative.
9. **Get dual approval.** ✅ *(if you have a co-author)*. **(gap D)** Solo
   developer path requires either a recruited human or a designed
   audited-LLM-critic-as-second-approver flow under AI-author disclosure.
10. **Decide the venue.** ✅ *(journal-fit recommender exists)*. **(gap E)**
    The recommender's first-class output is the Provider Atlas track. The
    AI/SWE micro-publication track — `algorithmic_improvement`,
    `reproducibility_infra`, `telemetry_trust`, `policy_governance` —
    enumerates candidate classes but does not have a parallel
    publication-format + venue-routing track to match.
11. **Submit.** ✅
12. **Right-of-reply, peer review, revisions, retraction.** ✅
13. **"Where does the published artifact live for readers?"** → **(gap G)**
    Zenodo + arXiv host the deposit; `vox-plugin-publication` syndicates
    outward. A Vox-native landing surface (`findings.vox-lang.org/<trusty-uri>`
    or analogous in the docs site / dashboard) that is the publication's
    canonical home is not yet in evidence.
14. **"What's in my queue?"** → **(gap H)** No dashboard panel surfacing
    candidates awaiting prereg, claims awaiting verification, manifests in
    reply-window, retraction queue.

## 2. Gap inventory

### Gap A — Self-observation candidate producers

**What.** Detectors that turn the user's own repo/CI/telemetry/training-run
activity into `finding-candidate.v1` rows.

**Evidence.** `contracts/scientia/finding-candidate.v1.schema.json` defines
the candidate type with classes `algorithmic_improvement`,
`reproducibility_infra`, `policy_governance`, `telemetry_trust`. The
Finalization Plan's Phase 6 wires `ProviderObservation` /
`ModelCapabilityEvidence` families (about *external* providers Vox calls)
into the candidate flow. The orthogonal flow — Vox observes *itself* — has
the schemas but missing producers. No grep hit for a commit-graph miner,
benchmark-CI-delta detector, or MENS-training novelty detector that
populates the ledger.

**Why it matters.** Without these, the candidate ledger is fed only by the
Provider Atlas path. A developer making a real software-engineering or
algorithmic contribution in this codebase has no automatic path from their
work into a publication candidate.

**Concretely missing.**
- Commit-graph miner: perf-improving merges (use existing `ExecTimeRecord`),
  novel-symbol detectors over recent diffs, MDL-style compression gain over
  unfamiliar regions of the AST.
- Benchmark-CI history detector: regression *and* improvement spikes over a
  preregistered baseline.
- MENS-training-run hook (per the in-flight Mn-T1..T15 plan): emit
  candidates from new SOTA checkpoints, novel attention patterns,
  repeatable training regressions.
- Vox-internal telemetry detector for `reproducibility_infra` and
  `policy_governance` candidates (Socrates surface is collected but not
  routed to the ledger as candidates about *Vox itself*).

**Dependencies.** None upstream; reuses existing `vox-research-events`
emitter and the candidate schema.

**Effort.** Medium. One producer per source; each ~400–800 LoC plus
detector heuristics; producers compose, so it can ship incrementally.

### Gap B — Replay runner that measures `artifact_replayability`

**What.** A runner that, given a `publication_manifest` and its RO-Crate,
re-executes the experiment in a sandboxed environment and writes the
measured replayability score back to the worthiness signals.

**Evidence.** `artifact_replayability` appears in
`crates/vox-publisher/src/publication_worthiness.rs`,
`worthiness_extraction.rs`, and the rubric
(`docs/src/reference/scientia-publication-worthiness-rules.md`), but the
*measurement source* is absent. No `vox-replay-runner` crate; no
`replay_artifact` API surface; no CI hook that re-executes RO-Crate
deposits.

**Why it matters.** The Finalization Plan takes credibility-via-symbolic-
verification very seriously (the AlphaEvolve thesis, §3.4). An asserted
replayability number sits in tension with that posture. A measured score —
even a coarse one ("the manifest's `entry_point` ran to a green exit code
and produced an output hash matching the deposited one") — converts a soft
rubric field into a verifier-backed claim.

**Dependencies.** Needs RO-Crate `entry_point` convention (Phase 4) — ✅.
Sandbox isolation: reuse existing CI runner, or a thin
`vox-sandbox-runner` shim.

**Effort.** Medium-small. Sandbox + replay harness; metric writeback;
worthiness rubric update.

### Gap C — Long-form manuscript scaffolder

**What.** A constrained-grammar emitter that produces an IMRaD markdown
skeleton (Intro / Methods / Results / Discussion) from a `FindingCandidate`
+ its verified claims + its RO-Crate, filling **only the safe slots** and
leaving provenance-bound TODOs for the human narrative.

**Evidence.** Phase 7 covers short-form adaptation via
`crates/vox-research-events/src/publication_format.rs`
(`ShortFormVariant`, `adapt_claim_to_platform`). No long-form analogue;
no grep hit for `manuscript`, `IMRaD`, or paper-scaffolding in
non-archived code.

**Why it matters.** The rubric explicitly forbids auto-generating
novelty/significance assertions, causal mechanism claims, and
measurement-implying figures. Fine. But the *blank-page barrier* for a
solo developer is real, and the safe slots (methods derived from RO-Crate
declarations; results table lifted from verified claim envelopes; citation
stubs from the SPECTER2-verified prior-art set; AI-disclosure block from
`AiDisclosureBlock::build`) are all already provenance-bound. Filling them
mechanically does not violate the red lines.

**Concretely.** A `vox scientia manuscript-draft --publication-id <id>`
command emitting `<id>.imrad.md` with:
- Methods section auto-filled from RO-Crate `mainEntity` + prereg
  `eval_substrate`.
- Results table from claim envelopes (one row per verified atomic claim,
  with Trusty URI link).
- References block from the SPECTER2-retrieved prior-art set used for
  novelty.
- Acknowledgments, ORCID/ROR, AI-disclosure auto-filled.
- Discussion / Intro / Significance sections present as
  `<!-- TODO(narrative): -->` blocks the human writes by hand.

**Dependencies.** Reuses RO-Crate (Phase 4), claim extractor (Phase 1),
prereg (Phase 2), `vox-constrained-gen`.

**Effort.** Medium. Templating + grammar; the verified-only constraint is
the discipline.

### Gap D — Solo-author critic-gate path

**What.** A designed path for a single developer to clear the dual-distinct-
approver gate without requiring a second human, under explicit AI-author
disclosure.

**Evidence.** Dual-approver is hard-required at the digest level in
`crates/vox-db/src/store/ops_publication.rs` and surfaced in the playbook.
`machine-suggestion-block.schema.json` carries
`machine_suggested`/`requires_human_review` flags. AI-disclosure block
exists (`vox-ro-crate/src/ai_disclosure.rs`). Phase 9 of the Finalization
Plan onboards an academic co-author — solo is not the primary mode.

**Why it matters.** A real Vox self-publication user is often solo. The
plan's posture (Galactica risk, GPT-4-grades-GPT-4 critique) requires that
any non-human approver be (a) explicitly disclosed, (b) a different
substrate than what produced the artifact, (c) bound to the content
digest, and (d) auditable.

**Concretely.** A second-approver role of `AuditedLLMCritic` that:
- Submits a signed approval bound to `content_sha3_256` from a key
  registered under an ORCID-distinct identity.
- Is automatically disclosed in `AiDisclosureBlock` as a contributor with
  the CRediT role `Validation` only (never `Investigation` or
  `Writing — original draft`).
- Uses a model architecturally different from any used in the artifact
  pipeline (enforced by recording model fingerprints in both manifest and
  critic approval).
- Refuses to approve if the artifact's worthiness signals include any
  contribution from the same model family — closes the
  GPT-4-grades-GPT-4 hole.
- The published artifact's venue catalog must allow LLM-critic
  approval (a new venue-catalog flag); IMC/MLSys/TMLR would set this
  `false`, F1000-track and Zenodo-only deposits could set it `true`.

**Dependencies.** Venue catalog flag; ORCID-distinct critic identity;
critic model fingerprinting.

**Effort.** Small-medium. Mostly policy plumbing plus the critic identity
machinery.

### Gap E — AI/SWE micro-publication track (non-Atlas)

**What.** A publication track for individual `algorithmic_improvement` /
`reproducibility_infra` / `telemetry_trust` / `policy_governance` findings
that does **not** route through the quarterly Provider Atlas.

**Evidence.** `FindingCandidateClass` enumerates these classes
(`crates/vox-research-events/src/schema_types.rs`); the candidate ledger
accepts them. `venue-catalog.v1.yaml` has IMC/MLSys/TMLR plus Distill-style
and Living Reviews entries — but the journal-fit recommender is geared
toward longitudinal measurement papers.

**Why it matters.** The user's question is broader than provider
measurement — it includes "I built a thing in this codebase and want to
publish about it." Examples already in this repo: `vox-prereg` itself is
publishable (PL/SE methods venue); `vox-arch-check` is publishable
(software-architecture venue); the SSOT migration policy
(`mesh-and-language-distribution-ssot-2026.md` §5.5) is publishable
(empirical software engineering). None of these fit the Atlas mold.

**Concretely.**
- Per-class venue mappings: SWE-PL improvements → ICSE/FSE/OOPSLA/PLDI;
  reproducibility infra → REP/MSR/ICSE-SEIP; telemetry trust → MLSys/SOSP
  workshops; policy governance → AIES/FAccT.
- Per-class artifact-format profile: short systems papers ≠ Atlas
  measurement papers; figure norms differ; reproducibility-package
  expectations differ.
- Per-class default reply-window length (14 days is right for Atlas;
  open-review tracks differ).

**Dependencies.** Gap C makes this much more usable (each class implies a
different IMRaD weight). Otherwise none.

**Effort.** Small (config + venue catalog rows) + medium (per-class
artifact-format profiles).

### Gap F — `vox scientia scout` single-command surface

**What.** One command that, run in a Vox workspace, surveys recent
commits / CI history / telemetry / training-run outputs and prints a
ranked candidate list with proposed `candidate_class`, suggested venue,
and a "promote-to-ledger" affordance.

**Evidence.** Discovery surface today is
`publication-discovery-scan`/`explain`/`refresh-evidence` — all over
already-prepared manifests. No "scan repo state from scratch" command.

**Why it matters.** This is the *front door* for the user journey. Without
it, the developer has to know to prepare a manifest before discovery ever
runs. With it, the system surfaces opportunity proactively.

**Concretely.**
- Subcommand under `vox scientia`.
- Aggregates output from Gap A's producers.
- Prints a table: candidate-id, class, top signals, suggested venue,
  recommended next command (`publication-prepare`, `prereg-draft`, etc.).
- Optional `--watch` mode for daemon-style monitoring; emits OS
  notifications when a strong-strength candidate appears.

**Dependencies.** Gap A is the prerequisite (no producers, nothing to
report).

**Effort.** Small (CLI + table rendering); leverages Gap A.

### Gap G — Vox-native publication reading surface

**What.** A canonical landing page for each published manifest, hosted on
Vox-owned infrastructure, distinct from Zenodo/arXiv deposits.

**Evidence.** `vox-plugin-publication` does RSS/Atom + Reddit/YouTube
syndication. `docs/src/index.mdx` is the docs landing. No `/findings/` or
`/publications/` route surfaces canonical published artifacts. Phase 4
notes Highwire-style meta tags for Google Scholar pickup but doesn't
specify the host page.

**Why it matters.** External deposits (Zenodo, arXiv) own the DOI but not
the reading experience. Distill-style web-native artifacts — first-class
in the Finalization Plan's venue catalog — require an HTML host. Living
Reviews need a stable canonical URL pointing at "latest version" with the
version history table inline.

**Concretely.**
- A `findings/<trusty-uri>` route on the docs SSG or dashboard, rendering
  the RO-Crate's text body + claim table + version history + reply thread
  inline.
- Highwire meta tags for Scholar pickup.
- Embedded nanopub viewer for atomic claims.
- Reply window status badge.

**Dependencies.** RO-Crate (Phase 4) — ✅.

**Effort.** Medium. SSG route + renderer; relies on existing docs
infrastructure.

### Gap H — Discovery dashboard panel

**What.** A dashboard panel surfacing the publication-pipeline queue:
candidates by class, claims awaiting verification, manifests in
reply-window, retraction queue, cost dashboard rollup.

**Evidence.** Phase 6 closed the loop end-to-end but is backend. The
dashboard plan (`docs/superpowers/plans/ci/2026-05-03-vox-dashboard-claude-design-port.md`)
exists but I did not verify a Scientia panel.

**Why it matters.** Operator visibility into the pipeline is currently CLI
JSON. A panel converts the workflow into a glanceable surface and
surfaces stalls (e.g., a candidate stuck in evidence-incomplete for 30
days).

**Dependencies.** Existing tables (`publication_manifests`,
`publication_status_events`, `external_submission_jobs`,
`finding_candidates` ledger).

**Effort.** Small-medium. Mostly UI work over existing data.

## 3. Priority and dependency matrix

| Gap | Severity | Effort | Unlock value | Depends on |
|-----|----------|--------|--------------|------------|
| A — self-observation producers | high | medium | unlocks F, gates E credibility | none |
| B — replay runner | high | medium-small | closes credibility hole in rubric | RO-Crate ✅ |
| C — IMRaD scaffolder | high | medium | removes blank-page barrier | claim extractor ✅, RO-Crate ✅, prereg ✅ |
| D — solo critic-gate | high (for solo users) | small-medium | unlocks solo self-publication | AI-disclosure ✅, venue catalog ✅ |
| E — AI/SWE micro-track | medium | small + medium | broadens publication scope beyond Atlas | C |
| F — `vox scientia scout` | medium | small | front-door UX | A |
| G — reading surface | medium | medium | hosts Distill-style + Living-Review | RO-Crate ✅ |
| H — dashboard panel | low | small-medium | operator UX | none |

### Dependency chains

```
A ──> F                     (scout needs producers)
A ──> E (credibility)       (AI/SWE micropapers need own-repo candidates)
C ──> E (usability)         (per-class scaffolds make track usable)
B ──> rubric credibility    (measured replayability)
D ──> solo workflow         (independent of others)
G, H ──> independent UX wins
```

The high-severity cluster is **A + B + C + D**. A and B are independent and
can ship in parallel; C and D are independent and can ship in parallel; F is
a thin wrapper over A once A exists; E is a config + per-class extension of
C; G and H are UX wins that do not block any other gap.

## 4. Recommended first slice

If this gap-map gets approved, the highest-leverage minimal first slice is:

**Slice 1 — "From commits to a candidate row, with a measured replay."**

Scope:
- Gap A, narrowest cut: one producer for `algorithmic_improvement`
  candidates from commit-graph + benchmark CI history (reuse
  `ExecTimeRecord`).
- Gap B, narrowest cut: replay runner that re-executes the RO-Crate
  `mainEntity` entry point in a fresh worktree and writes the measured
  replayability score (binary pass/fail + output-hash match) back to
  worthiness signals.
- Gap F, thin wrapper: `vox scientia scout` listing candidates from this
  one producer.

Why this slice: it makes the *front of the user journey* (steps 1–7 above)
work end-to-end for the most common case (a developer made a perf
improvement), with measured (not asserted) replayability. Everything
downstream of step 7 is already strong.

Slice 2 candidates: Gap C (manuscript scaffolder) or Gap D (solo critic
gate) — independent, pick by user demand.

## 5. Open questions

1. **Producer pluggability.** Should signal producers be a plugin
   contract (so third parties can add detectors) or an L1 trait with a
   fixed set?
2. **Solo critic identity provenance.** Is an ORCID-distinct identity
   for an LLM critic ethically acceptable in the venues we care about?
   (Atlas: probably not. F1000 / Zenodo-deposit: probably yes. Needs
   per-venue declaration in `venue-catalog.v1.yaml`.)
3. **Replay determinism.** What's the minimal RO-Crate `mainEntity`
   contract to make replay tractable? GPU-bound experiments need a
   different cut than CPU-bound.
4. **Reading surface hosting.** Docs SSG or dashboard? Or both with the
   dashboard as edit surface and SSG as canonical reader?
5. **Negative-result quota for the AI/SWE micro-track.** The Atlas has a
   3-positive : 1-null quota. Does the SWE micro-track inherit this or
   set its own?

## 6. Cross-references

- [SCIENTIA Self-Publication Finalization Plan
  (2026)](./scientia-self-publication-finalization-plan-2026.md) — supersedes
  this for Phases 0–10.
- [SCIENTIA SSOT
  handbook](../reference/scientia-ssot-handbook.md) — lifecycle and status
  vocabulary.
- [Publication worthiness
  rules](../reference/scientia-publication-worthiness-rules.md) — rubric and
  red lines.
- [How-to: Publish Scientia
  findings](../how-to/how-to-scientia-publication.md) — operator flow.
- [Where Things Live](./where-things-live.md) — concept → crate map.
- [Mesh & Language Distribution SSOT
  (2026)](./mesh-and-language-distribution-ssot-2026.md) — §3.5 Hopper track,
  §5.5 migration policy, §5.6 routes.
