---
title: "SCIENTIA Micro-Publication, SSOT Unification & Discovery-Surfacing Design (2026)"
description: "Design for a claim-centric single source of truth across micro-publication (nanopub), long-form scholarly publication, and social syndication; with a human-gated discovery-review workflow (no auto-publishing), spec-compliant nanopublication identity/signing, and concrete fixes to novelty and claim detection so new discoveries surface accurately. Grounded in a 2026-06-05 code audit and the nanopublication / Trusty-URI specs."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Establishes the target architecture for the micro-publication and syndication legs of SCIENTIA: a human-gated, claim-centric SSOT; spec-compliant nanopub identity/signing with an offline compatibility-verification path; and the accuracy fixes for novelty/claim detection. Informs the next round of phase plans."
---

# SCIENTIA Micro-Publication, SSOT Unification & Discovery-Surfacing Design (2026)

> **Companions:** [SCIENTIA SSOT handbook](../reference/scientia-ssot-handbook.md) ·
> [Publication worthiness rules](../reference/scientia-publication-worthiness-rules.md) ·
> [Self-Publication Gap Map (2026)](./scientia-self-publication-gap-map-2026.md) ·
> [GUI ↔ Scientia coverage audit (2026)](./vox-gui-scientia-coverage-audit-2026.md) ·
> [Where Things Live](./where-things-live.md).
>
> **Revision note (2026-06-05, v2).** This revision incorporates four locked
> decisions and one major new requirement (human-gated review, no
> auto-publishing) — see §1. The plan is reordered so **spec-compliant
> nanopublication identity is P1**, and **human review is the spine of the whole
> pipeline**, not an add-on. §12 critiques v1 and explains why v2 is higher
> quality.

## 0. Scope

Make it easy to maintain **one source of truth** across **micro-publication
(nanopub)**, **regular scientific publication**, and **social media (Twitter)** —
with the hardest focus on micro-publication *quality* and on *surfacing new
discoveries accurately*. The audit (§2) found the micro-publication and social
legs are largely unreachable stubs with duplicate implementations; this design
fixes that and adds the human-in-the-loop surfacing workflow the user requires.

## 1. Locked decisions (2026-06-05) and the new requirement

1. **Trusty URI / nanopub: full spec compliance — and it is priority #1.**
   Implement the real nanopublication Trusty-URI (`RA` module, RDF
   normalization) and **RSA** signing per spec, validated against the official
   conformance vectors. This becomes **Phase P1**. **Resolved (round 2):** use the
   maintained Rust **`nanopub` crate** as a dependency (decision #B approved) — it
   provides RSA keygen/signing + Trusty-URI normalization + the validator, so we do
   **not** add RSA to `vox-crypto` (which is Ed25519-only). Signing keys are
   **per-user, never shared** (see §4.1 and decision #A below).
2. **Novelty consolidation: best of both, leaning on vox-scientia's verdict
   model.** Use vox-publisher's real federated retrieval as the *evidence
   producer*; promote vox-scientia's typed verdict ladder (+ conflict/chrono) as
   the *decision layer*; unify the two divergent bundle types; add real
   embeddings.
3. **Do not publish nanopubs yet — but prove full compatibility now.** Build
   spec-compliant, signed artifacts and verify them **offline** against the
   nanopub conformance test-suite and the reference validator, with an *optional,
   default-off* path to the public **test server** ("test balloon"). The
   production network path stays disabled.
4. **(New requirement) Human-gated discovery review — no auto-publishing.** The
   pipeline must **surface** new claims/discoveries to the user for review, and
   the user decides whether to nano-publish. Producers, claim extraction, and
   novelty all run to *produce evidence*; **emission of any artifact requires an
   explicit human approval.** This must be first-class in the **GUI**.

## 2. Current-state map (audit, 2026-06-05)

Legend: **Real** = works end-to-end · **Stub** = placeholder on the path ·
**Orphaned** = code + tests exist but no CLI/MCP caller.

| Capability | Status | Evidence |
|---|---|---|
| Long-form scholarly path (scaffold → LaTeX → arXiv bundle → adapters → dual-approval → replay) | **Real** | [`scientia_phase_handlers.rs`](../../../crates/vox-cli/src/commands/scientia_phase_handlers.rs) |
| Claim extraction | **Real (reachable), low-signal default** | `vox scientia publication-extract-claims` → [`pipeline.rs`](../../../crates/vox-scientia/src/claim_extractor/pipeline.rs); mock verifier unless `VOX_MINICHECK_ENDPOINT` |
| Prior-art retrieval (vox-publisher) | **Real federated fetch, fake "semantic"** | [`scientia_prior_art.rs`](../../../crates/vox-publisher/src/scientia_prior_art.rs): OpenAlex/Crossref/S2, recency, citations, traces — but `semantic_proxy(lexical)=lexical` |
| Novelty verdict (vox-scientia `AtomicNoveltyScorer`) | **Orphaned, single-signal, SPECTER2 stub** | [`novelty.rs`](../../../crates/vox-scientia/src/inspect_bridge/novelty.rs) |
| Micro-publication (nanopub) | **Orphaned + publish stub + non-spec identity + Ed25519** | [`nanopub/`](../../../crates/vox-scientia/src/nanopub/), [`network.rs:15`](../../../crates/vox-scientia/src/nanopub/network.rs:15) |
| Short-form social (Scientia) | **Orphaned truncation stub; excludes Twitter; UTF-8 panic** | [`publication_format.rs`](../../../crates/vox-research-events/src/publication_format.rs) |
| Social syndication (vox-publisher) | **Real adapters incl. Twitter/X** | [`adapters/twitter.rs`](../../../crates/vox-publisher/src/adapters/twitter.rs) — driven by RSS/news, not claims |
| `EvidenceConflict` / `ChronoFilter` | **Built, unwired** | [`conflict.rs`](../../../crates/vox-scientia/src/inspect_bridge/conflict.rs), [`chronofact.rs`](../../../crates/vox-scientia/src/inspect_bridge/chronofact.rs) |
| GUI surface registry | **Real, contract-backed, CI-gated** | [`gui_surface_registry.rs`](../../../crates/vox-cli/src/commands/ci/gui_surface_registry.rs), `contracts/gui/surface-registry.v1.schema.json`, `crates/vox-gui/ui/src/components/surfaces/` |

**Seven drift seams nothing keeps in sync:** (1) two Trusty-URI impls that
disagree — [`preregistration/trusty_uri.rs`](../../../crates/vox-orchestrator/src/preregistration/trusty_uri.rs)
(`RA`+base64url(SHA-256(canonical JSON))) vs nanopub
[`trig.rs:57`](../../../crates/vox-nanopub/src/trig.rs:57) (`RA`+**hex**),
**neither hashing normalized RDF**; (2) two novelty scorers; (3) **two
`NoveltyEvidenceBundle` types** — `vox-research-events` (read by the scorer) vs
`NoveltyEvidenceBundleV1` in [`scientia_finding_ledger.rs`](../../../crates/vox-publisher/src/scientia_finding_ledger.rs)
(produced by the fetch); (4) two social systems that never call each other;
(5) claim text copied not referenced (`ResultsRow.claim_text`); (6) Ed25519
signing incompatible with the nanopub network's RSA expectation; (7) doc drift
([`lib.rs:12`](../../../crates/vox-scientia/src/lib.rs:12), handbook, `layers.toml`).

## 3. The target model — a human-gated, claim-centric SSOT

**Principle:** the **verified `AtomicClaim`, identified by one spec-compliant
Trusty URI, is the single source of truth.** All three publication forms are
*projections* of it. **Nothing leaves the system without a human decision.**

```
 producers ─┐
 CI/bench ──┤   ┌───────────────┐   ┌──────────────────────────┐   ┌──────────────┐
 telemetry ─┼──►│ claim extract │──►│ Verified AtomicClaim      │──►│  REVIEW      │
 commits ───┘   │  + novelty    │   │ tuple · verifiability ·   │   │  QUEUE       │
                │  + conflict   │   │ confidence · verdict ·    │   │ (human gate) │
                └───────────────┘   │ novelty verdict+priorart ·│   └──────┬───────┘
                                    │ ONE Trusty URI            │          │
                                    └──────────────────────────┘   approve │ reject/edit/defer
                                                                            ▼
                                        ┌───────────────────────────────────────────────┐
                                        │ projections (only after approval)              │
                                        │  • nanopub_for(claim)   → signed RA nanopub     │
                                        │  • results_row_for(claim) → IMRaD row (by URI)  │
                                        │  • syndicate(claim, ch) → Twitter/Bluesky (URI) │
                                        └───────────────────────────────────────────────┘
```

SSOT changes: **one Trusty-URI algorithm** (§4); **one `NoveltyEvidenceBundle`**
(§6); **reference, don't copy** (`ResultsRow` stores `claim_id`+`trusty_uri`); a
single **projection module** (`nanopub_for` / `results_row_for` / `syndicate`)
that all read the same verified-claim struct.

### 3.1 Infrastructure we reuse (do not rebuild)

The audit found most of what this design needs already exists — the work is
*assembly + a few targeted additions*, not greenfield:

| Need | Reuse this | Add only this |
|---|---|---|
| Nanopub identity, RSA signing, Trusty URI, validator | Rust **`nanopub`** crate (decision #B) | Clavis key custody + ORCID binding |
| Semantic embeddings | **`vox_actor_runtime::llm::llm_embed`** — the *same* multi-vendor facade [`vox-search` already uses](../../../crates/vox-search/src/embeddings.rs) (OpenRouter/OpenAI/HF/auto), with vectors in [`vox-db`](../../../crates/vox-db/src/store/ops_memory.rs) + Qdrant ANN ([`vector_qdrant.rs`](../../../crates/vox-search/src/vector_qdrant.rs)) | An optional **local Candle backend** (benefits search too) |
| Prior-art retrieval | `vox-publisher` [`fetch_prior_art_federated`](../../../crates/vox-publisher/src/scientia_prior_art.rs) (OpenAlex/Crossref/S2) | Real `semantic_score` from the facade above |
| Social posting | `vox-publisher` adapters (Twitter/Bluesky/…) + `social_retry`/`topic_packs` | The `syndicate()` seam (§7) |
| ORCID auth | `vox-publisher` [`orcid_oauth.rs`](../../../crates/vox-publisher/src/scholarly/orcid_oauth.rs) PKCE state machine | HTTP wrapper + per-user token storage |
| GUI surfacing | contract-backed, CI-gated surface registry | The `DiscoveryReview` surface (§5.2) |
| Critic suggestion | `vox-scientia` [`critic_gate`](../../../crates/vox-scientia/src/critic_gate/) (different-model-family) | Advisory-only wiring into review |

## 4. Pillar 1 (P1) — spec-compliant micro-publication identity & signing

Per the nanopublication / Trusty-URI specs (see §13 references), a network-valid
nanopub requires:

- **Trusty URI, module `RA`** (multi-graph). The 45-char artifact code is
  base64 over the **normalized RDF** of all four graphs, with the self-URI
  replaced by a placeholder during hashing. RDF normalization (statement sorting
  + blank-node handling) is the load-bearing, error-prone part; the spec itself
  notes it is "not fully deterministic," which is exactly why we validate against
  shared conformance vectors rather than trusting our own output.
- **RSA signing** in the pubinfo graph: `npx:hasAlgorithm "RSA"`,
  `npx:hasPublicKey`, `npx:hasSignature`, `npx:hasSignatureTarget`, plus
  `dct:created` and `dct:creator` (ORCID). The signature target is the normalized
  content *including* the public key; the Trusty URI is computed **last**,
  covering the signature.

**Design decisions:**
- **Depend on the maintained Rust `nanopub` crate — do not re-roll normalization
  or RSA.** (Decision #B approved.) Re-implementing RDF normalization + RSA
  signing to be byte-compatible with the network is high-risk (the spec itself
  notes normalization is "not fully deterministic"); the crate already passes the
  community conformance suite and owns RSA keygen/sign/verify. `vox-crypto` stays
  Ed25519-only; **the nanopub RSA identity lives in the `nanopub` crate's key
  format**, with only *custody* (encrypted storage) delegated to Clavis. This
  retires drift seam (6) without adding `rsa` to `vox-crypto`.

### 4.1 Per-user keys, never a shared Vox key (decision #A)

A nanopub signature is an **attribution claim** bound to an ORCID — "this person
asserts this." A single shared Vox key is therefore the wrong model on every axis:

- **Attribution collapse** — every user's discoveries would be signed as one
  identity; no one could prove *they* made a finding, and the network would credit
  the wrong author.
- **Non-repudiation / custody risk** — one leaked private key compromises *all*
  users' signatures with no per-user revocation; you can never un-sign.
- **Accountability** — retractions, COPE workflows, and the right-of-reply all
  assume a real responsible author, not a collective bot key.

**Design: per-user RSA nanopub identity.**
- Each user gets their **own** RSA keypair (generated locally by the `nanopub`
  crate), **stored encrypted in their Clavis vault** (account/profile-scoped via
  [`vox_vault.rs`](../../../crates/vox-secrets/src/backend/vox_vault.rs)); the
  private key never leaves the machine.
- Bound to the **user's own ORCID** via the existing PKCE state machine
  ([`orcid_oauth.rs`](../../../crates/vox-publisher/src/scholarly/orcid_oauth.rs))
  once an HTTP wrapper + per-user token storage are added.
- **Gap to close:** today Clavis is *account*-scoped, not *per-human-user*, and
  there is no `user_identities` table. P1 adds `user_identities(user_id, orcid_id,
  nanopub_key_ref, …)` and per-user key storage (this also unblocks the
  [Ludus identity work](./ludus-identity-github-integration-research-2026.md)).
- **Vox-the-project** MAY hold a *separate, distinct* identity for
  project-authored artifacts (e.g. the automated Provider Atlas) — that is one
  more identity, **not** a key shared across users.
- **Retire the hand-rolled hex "trusty URI"** ([`trig.rs:57`](../../../crates/vox-nanopub/src/trig.rs:57))
  and the base64url prereg variant in favor of the one spec-compliant module;
  the preregistration `trusty_uri.rs` becomes a caller of the shared module.
- **Enrich the assertion/provenance graphs** with the `SciClaimTuple`,
  `VerifiabilityClass`, support confidence, the **novelty verdict + prior-art
  set**, and verification provenance — so a nanopub is the *richest* per-claim
  artifact, not a bare text triple. Embed the signature into pubinfo (today it is
  only a struct field; the `NanopubGraphs` "signature embedded here" comment is a
  lie the build path never honors).

**Compatibility verification — "test balloons" without publishing (decision #3):**
1. **Conformance vectors in CI.** Run our emitted nanopubs through the official
   [`nanopub-testsuite`](https://github.com/Nanopublication/nanopub-testsuite)
   raw-RDF cases; a `vox ci nanopub-conformance` gate fails the build on any
   trusty-URI or signature mismatch.
2. **Offline validator.** The `nanopub` crate validates a signed artifact
   (trusty URI + signature + structure) with no network — wire it as an
   acceptance test on every emitted artifact.
3. **Round-trip.** Serialize → parse → re-derive trusty URI → assert stable.
4. **Optional test server, default-off.** A `--to-test-server` flag (gated by an
   env allow + an explicit human approval) may POST to the public **test**
   instance (`use_test_server`), which is periodically wiped and is not the real
   network. **Production publishing stays disabled** behind a separate,
   unimplemented-by-design switch (§11 Open decision #C is already answered: not
   yet).

**Acceptance (P1):** an emitted nanopub passes the conformance suite + the
reference validator; the Trusty URI is stable and RDF-normalized; RSA signature
verifies; **no production network call exists in the codebase.**

## 5. Pillar 2 (P2 + P3) — human-gated discovery review (the spine)

This is the new requirement and the heart of "make it easier + surface
accurately, but never auto-publish."

### 5.1 Review state machine (P2, backend)
A surfaced item carries verified claims + their evidence and moves through:

```
surfaced ──► under_review ──┬──► approved  ──► emitted(local/staged) ──► [published: OFF]
                            ├──► rejected(reason)
                            ├──► edited ──► (re-extract + re-verify) ──► surfaced
                            └──► deferred/snoozed
```

- **Emission is gated.** `nanopub_for(claim)` / `syndicate(claim, …)` refuse to
  run unless an `approved` decision row bound to the claim's content digest
  exists. This is a **hard structural guard**, not a convention: the projection
  functions take an `ApprovalToken` that only the review service can mint.
- **Audit trail.** Every decision records who/when/why and the model
  fingerprints present in the artifact (feeds AI-disclosure). Reuses the digest-
  binding discipline already enforced for scholarly approvals.
- **Optional `AuditedLLMCritic` as a *suggestion*, never an auto-approve.** The
  existing critic gate ([`critic_gate/`](../../../crates/vox-scientia/src/critic_gate/),
  with different-model-family enforcement) may *advise* the human, but for
  micro-pub the **human is the gate**.
- **Idempotence + supersession.** Re-emission after an edit supersedes the prior
  artifact via the nanopub retraction/version mechanism (the orchestrator already
  has [`preregistration/retraction.rs`](../../../crates/vox-orchestrator/src/preregistration/retraction.rs)).

Data model: a `scientia_review_queue` row (claim_id, publication_id, digest,
state, surfaced_at, evidence_ref) + an append-only `scientia_review_decisions`
audit table. REST `/api/v2/scientia/review`; WS topic `scientia.review.changed`
(matches the route conventions in the mesh SSOT §5.6).

### 5.2 Discovery Review GUI surface (P3, "especially in the GUI")
A new **`DiscoveryReview`** surface under
`crates/vox-gui/ui/src/components/surfaces/`, **registered in the contract-backed
surface registry** (`contracts/gui/surface-registry.v1.schema.json`) so the
CI self-surfacing gate ([`gui_surface_registry.rs`](../../../crates/vox-cli/src/commands/ci/gui_surface_registry.rs))
guarantees it exists and is reachable from the Sidebar.

**Review granularity (decision #E — recommended resolution): finding-level card
with per-claim drill-down.** Users think in terms of "this discovery" (a
*finding* = a cluster of atomic claims), not 20 fragments — so the card is the
finding. But nanopubs are per-atomic-claim and approval must be per-claim (you may
accept 3 of 5), so the card **expands** to its atomic claims, each with its own
approve/reject toggle. Default action "approve all promotable claims" with
per-claim opt-out; emission produces one nanopub per *approved* claim. This keeps
the human UX coarse and glanceable while matching nanopub's per-assertion
granularity. (Queue schema therefore keys on both `finding_id` and `claim_id`.)

Each **review card** makes the decision glanceable and one-click:
- The claim text + structured tuple (variable → relation → variable).
- Verifiability class, confidence, and verdict (Supported / Contested /
  **Contradicted** / Abstain).
- **Novelty verdict** (Novel / PossiblyNovel / NotNovel / **InsufficientEvidence**)
  with the **closest prior-art hits** (title, year, citations, link) so the user
  sees *why* it is or isn't novel — and an explicit "retrieval failed / no
  sources answered" banner when evidence is insufficient (so a user never
  approves a *false* novelty).
- Any `EvidenceConflict` (supporting vs. contradicting hits side by side).
- Provenance: which producer surfaced it (commit / bench delta / telemetry) and
  the worthiness-score breakdown.
- Actions: **Approve → emit nanopub (local/staged)** · **Edit** (re-verifies) ·
  **Reject (reason)** · **Defer** · **Promote to manuscript** · **Queue for
  social (post-approval)**.

A **surfacing inbox** + opt-in OS notification on strong candidates (the gap
map's `scout --watch` idea) routes to *review*, never to publish.

**Acceptance (P2+P3):** no artifact can be emitted without an approval row; the
surface-registry CI gate is green; a surfaced discovery can be reviewed and
decided end-to-end in the GUI; rejected/insufficient-evidence claims never emit.

## 6. Pillar 3 (P4) — surfacing accuracy: novelty "best of both" + claim fixes

### 6.1 Novelty — unify and upgrade (decision #2)
- **One bundle type.** Merge `NoveltyEvidenceBundle` (vox-research-events) and
  `NoveltyEvidenceBundleV1` (vox-publisher) into a single canonical type; the
  scorer and the fetch then speak the same shape (retires drift seam 3).
- **vox-publisher = evidence producer.** Keep `fetch_prior_art_federated`
  (OpenAlex/Crossref/Semantic Scholar, recency buckets, citation counts, query
  traces, dedup) — it is genuinely good.
- **vox-scientia = decision layer (promoted).** Promote the typed verdict ladder
  as the single scorer vox-publisher calls, but upgrade it from single-scalar to
  **multi-signal**: semantic + lexical agreement, closest-hit citation count,
  recency, and the *number* of near-hits (near many works ⇒ less novel).
- **Real semantic similarity — via the shared embedding facade (decision #D).**
  Both impls fake "semantic" as a lexical proxy (`semantic_proxy(lexical)=lexical`;
  SPECTER2 stub). Wire a real embedding model through the **same seam vox-search
  already uses** — `vox_actor_runtime::llm::llm_embed` — so search and novelty
  share one embedding path (no bespoke model). That facade already supports
  **OpenRouter** (use free embedding models where available) and `auto` provider
  resolution. Because the facade is HTTP-only today, the **single shared addition**
  is a **local embedding backend** (Candle / `fastembed`, no Docker) so novelty
  works offline and free — and vox-search inherits it. **SPECTER2** specifically is
  feasible locally (SciBERT/bert-base, ~440 MB, Candle/ONNX, no Docker), with one
  caveat: it is an *adapter* model (base + proximity adapter), so the local backend
  must load the proximity adapter or fall back to a plain scientific
  sentence-transformer. This is the single biggest accuracy lever.
- **`InsufficientEvidence` verdict.** Drive it from `query_traces`: if **no
  source returned a successful HTTP status**, the verdict is
  `InsufficientEvidence`, never `Novel`. This fixes the worst false positive —
  today `empty_novelty_bundle` emits `max_semantic = Some(0.0)`
  ([`scientia_prior_art.rs:149`](../../../crates/vox-publisher/src/scientia_prior_art.rs:149))
  which the scorer reads as Novel.
- **Wire `ChronoFilter` + `EvidenceConflict`.** Filter future-dated "prior art"
  before scoring; fold contradiction into the verdict (contradicted-by-prior-art
  surfaces as *contested*, not *novel*).

### 6.2 Claim detection — correctness first
- **Make `Contradicted` reachable.** Give MiniCheck an NLI-style
  entail/neutral/**contradict** signal; today
  [`pipeline.rs:91`](../../../crates/vox-scientia/src/claim_extractor/pipeline.rs:91)
  never emits it, so `ExtractedClaimsSummary.refuted` is structurally always 0
  and `contradiction_penalty` can never fire.
- **Real default verifier with honest abstain.** The mock word-overlap never
  abstains (floor 0.5 > τ 0.3); point at a real default and calibrate ABSTAIN.
- **Robust sentence splitter** that doesn't break numeric claims ("12.5ms" →
  "12.").

**Acceptance (P4):** empty/failed retrieval → `InsufficientEvidence` (never
Novel); a contradicted claim → `refuted > 0`; novelty uses ≥3 signals including
real embeddings; numeric claims split intact.

## 7. Pillar 4 (P5) — one `syndicate()` SSOT seam (manuscript + Twitter)

A single projection seam bridges the canonical claim to outputs, **all gated on
approval (§5)**:
- `results_row_for(claim)` returns a manuscript row that **references the Trusty
  URI** (no copied text) — retires drift seam 5.
- `syndicate(claim, channel)` adapts the claim via **constrained-grammar
  generation** (not truncation) carrying its Trusty URI, and hands it to the
  **existing** `vox-publisher` adapters. Add `Twitter` to the platform enum; fix
  the UTF-8 byte-slice panic
  ([`publication_format.rs:68`](../../../crates/vox-research-events/src/publication_format.rs:68)).
- Social posting requires an `approved` decision (no auto-post), reusing
  `social_retry` / `topic_packs` for backoff and per-channel floors.

**Acceptance (P5):** one approved claim's Trusty URI is identical across nanopub,
manuscript row, and social post; it posts to Twitter + Bluesky carrying that URI;
the SSOT CI gate (below) is green.

## 8. Pillar 5 (P6) — evaluation, consolidation, doc-drift

- **Surfacing quality harness.** A golden set of known discoveries and known
  non-discoveries with measured **precision/recall** for the surfacing decision,
  run in CI (ties into the behavioral-output-verification initiative). "Higher
  quality" is only real if it is measured.
- **SSOT drift gate.** A CI check asserting a claim's Trusty URI is byte-identical
  across nanopub / manuscript / social projections, alongside the existing
  ssot-drift and gui-surface gates.
- **Consolidation + doc-drift.** One Trusty-URI module, one novelty scorer, one
  bundle type; fix [`lib.rs:12`](../../../crates/vox-scientia/src/lib.rs:12), the
  SSOT handbook §3 map, and `layers.toml`; add new surfaces to
  [`where-things-live.md`](./where-things-live.md); regenerate `SUMMARY.md` via
  the generator (never hand-edit).

## 9. The no-auto-publish guarantee (defense in depth)

Because "do not publish automatically" is a hard requirement, it is enforced at
**four** independent layers:
1. **Type-level:** projection/emit functions require an `ApprovalToken` only the
   review service mints from a persisted human decision.
2. **Data-level:** emission refuses without an `approved` row bound to the
   content digest.
3. **Config-level:** the production network publisher is a separate switch that is
   **unimplemented by design** in this plan (test server only, default-off).
4. **CI-level:** a guard test asserts no production nanopub-network POST exists on
   any code path reachable without an approval token.

## 10. Proposed phase plan (reordered)

| Phase | Title | Gated outcome |
|---|---|---|
| **P1** | Spec-compliant nanopub via `nanopub` crate: **per-user** RSA identity (Clavis custody + `user_identities` table + ORCID bind), **offline** validation + conformance suite (no network) | Emitted artifact passes `nanopub-testsuite` + reference validator; per-user RSA sig verifies; zero production network calls |
| **P2** | Human-gated review **backend**: state machine, approval tokens, audit trail, REST/WS | No emission without an approval row; full audit trail; idempotent supersession |
| **P3** | Discovery Review **GUI** surface (registry-gated) + surfacing inbox | A discovery is reviewed and decided end-to-end in the GUI; emission requires a click |
| **P4** | Surfacing accuracy: unify bundle, promote scorer, `InsufficientEvidence`, real embeddings, wire chrono/conflict; reachable `Contradicted` + real verifier + splitter | Empty retrieval ≠ Novel; contradicted ⇒ refuted>0; ≥3 novelty signals |
| **P5** | One `syndicate()` SSOT seam: reference-not-copy manuscript rows; bridge to vox-publisher adapters; add Twitter; fix panic | One claim's URI identical across all three forms; approved claim posts to Twitter/Bluesky |
| **P6** | Evaluation harness + SSOT/conformance CI gates + consolidation + doc-drift | Measured surfacing precision/recall; drift gates green |
| **(deferred)** | **Live nanopub-network publishing** | Out of scope by decision #3 — behind human approval + an explicit future switch |

Dependency order: **P1 → P2 → P3** form the credentialed, human-gated spine and
should land in sequence. **P4** can proceed in parallel with P2/P3 (it improves
the evidence the review surface shows). **P5** depends on P1 (URI) + P2 (gate).
**P6** is continuous. Detailed TDD-step plans get promoted per-phase under
`docs/superpowers/plans/scientia/` when next-to-execute.

## 11. Decisions — resolved and remaining

**Resolved (2026-06-05, round 2):**
- **#A — Keys: per-user, never shared.** Per-user RSA nanopub identity, generated
  locally, custody in Clavis, bound to the user's own ORCID (§4.1). Vox-project
  artifacts use a separate distinct identity.
- **#B — Use the Rust `nanopub` crate** as a dependency (RSA + Trusty URI +
  validator); `vox-crypto` stays Ed25519-only.
- **#D — Share vox-search's embedding seam** (`llm_embed`), add one local Candle
  backend that benefits both; OpenRouter free models where available; SPECTER2
  local is feasible (adapter caveat, §6.1).
- **#E — Finding-level review card with per-claim drill-down** (§5.2).

**Remaining sub-questions (lower-stakes; can default):**
- **#A2 — `user_identities` table now or interim account-scoping?** P1 needs
  per-user key custody, but the per-human user model doesn't exist yet (Clavis is
  account-scoped). Build the `user_identities` table in P1 (cleaner), or ship P1
  with account-scoped keys and add the table in P2? *Rec: build it in P1 — it
  unblocks ORCID binding and the Ludus identity work.*
- **#D2 — Default embedding provider order.** Prefer local Candle (offline, free,
  private) by default and fall back to OpenRouter free models, or the reverse?
  *Rec: local-first, OpenRouter fallback — aligns with free-by-default.*
- **#D3 — SPECTER2 vs. a general scientific sentence-transformer** for the local
  backend (SPECTER2's adapter loading is more work; a plain MiniLM/BGE-small is
  simpler and lighter). *Rec: ship with a light general model, add SPECTER2 as an
  opt-in once the adapter path is proven.*

## 12. Critique: why v2 is higher quality than v1

- **v1 buried identity inside "P1: emit nanopubs."** But a *correct, spec-valid
  Trusty URI* is the foundation everything else references — v2 makes
  spec-compliant identity the standalone first phase, with conformance vectors as
  the acceptance bar (not just "embed a signature").
- **v1's "emit one nanopub per promotable claim via one command" was effectively
  auto-publish.** v2 makes **human review the spine**, with a four-layer
  no-auto-publish guarantee, and treats the GUI review surface as a first-class
  phase rather than a dashboard afterthought.
- **v1 under-specified "best of both" for novelty.** v2 names the concrete split
  (vox-publisher = retrieval, vox-scientia = verdict), surfaces the **two
  divergent bundle types** as a unification target, and identifies that *both*
  impls fake semantic similarity — making a real embedding model the explicit
  accuracy lever.
- **v1 had no way to measure "better."** v2 adds a precision/recall surfacing
  harness and an SSOT drift CI gate, so quality is verifiable, not asserted.
- **v1 ignored compatibility risk for "don't publish yet."** v2 turns that into a
  concrete offline-compatibility strategy (conformance suite + validator +
  round-trip + optional default-off test server), so we can *prove* network
  readiness without touching the production network.
- **v1 missed the RSA-vs-Ed25519 incompatibility.** v2 surfaces it as a hard
  blocker to compliance and plans the RSA nanopub identity.

## 13. References (external — nanopublication / Trusty-URI spec)

- Kuhn & Dumontier, *Trusty URIs: Verifiable, Immutable, and Permanent Digital
  Artifacts for Linked Data* — [arXiv:1401.5775](https://arxiv.org/pdf/1401.5775).
- Kuhn et al., *Making Digital Artifacts on the Web Verifiable and Reliable* —
  [arXiv:1507.01697](https://arxiv.org/pdf/1507.01697).
- [Nanopublication Guidelines (working draft)](https://nanopub.net/guidelines/working_draft/).
- [`Nanopublication/nanopub-testsuite`](https://github.com/Nanopublication/nanopub-testsuite) — cross-language conformance vectors.
- Rust `nanopub` crate — [docs.rs/nanopub](https://docs.rs/nanopub/latest/nanopub/) · [nanopub-rs toolkit](https://vemonet.github.io/nanopub-rs/).
- [Nanopub Registry](https://github.com/knowledgepixels/nanopub-registry) (production lookup/publish; test instances available).
- nanopub-py publishing & test server — [publish guide](https://nanopublication.github.io/nanopub-py/publishing/publish-nanopublications/).

## 14. Cross-references (internal)

- [SCIENTIA SSOT handbook](../reference/scientia-ssot-handbook.md) — needs §3 SSOT-map update (§8).
- [Publication worthiness rules](../reference/scientia-publication-worthiness-rules.md) — `contradiction_penalty` depends on P4.
- [GUI ↔ Scientia coverage audit (2026)](./vox-gui-scientia-coverage-audit-2026.md) — the surface-registry + self-surfacing gate P3 builds on.
- [Self-Publication Gap Map (2026)](./scientia-self-publication-gap-map-2026.md) — the built Phases A–H this design extends.
- [Where Things Live](./where-things-live.md) — add rows for the new surfaces in the implementing PR.
