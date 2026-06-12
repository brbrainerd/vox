# P2 — Human-Gated Discovery Review Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Stacking:** This phase **depends on P1** (`docs/superpowers/plans/scientia/2026-06-05-p1-spec-compliant-nanopub-identity.md`, PR #144). It modifies P1's `nanopub_build` to require an `ApprovalToken`. Execute it **stacked on the P1 branch** (or after P1 merges to main). Confirm the exact P1 signatures against the merged code before writing each task's code — they are stable but this plan was authored pre-merge.

**Goal:** No SCIENTIA artifact (nanopub today; manuscript/social later) can be emitted without an explicit, audited human approval. Surface verified claims for review; record per-claim decisions; gate emission behind a type-level `ApprovalToken`.

**Architecture:** A new append-only `scientia_review_decisions` audit table records per-claim human decisions (approve/reject/defer/edit) bound to the claim's content digest. The review service is the **only** minter of `ApprovalToken` (a non-constructible-elsewhere type). `nanopub_build` (P1) is changed to require an `ApprovalToken`, so emission is impossible without a persisted approval. Reuses the existing `scientia_discoveries.human_gate_status` vocabulary (`pending|approved|rejected`) and digest-binding discipline.

**Tech Stack:** Rust; `vox-db` (libSQL, no CHECK constraints); existing `vox-scientia` claim/nanopub modules; `vox-cli` handlers.

**Design source:** [scientia-micropublication-ssot-and-surfacing-design-2026.md](../../../src/architecture/scientia-micropublication-ssot-and-surfacing-design-2026.md) §5.1, §9 (four-layer no-auto-publish guarantee).

---

## Context the engineer needs (verified facts)

- **The human-gate vocabulary already exists:** `scientia_discoveries.human_gate_status` (`pending|approved|rejected`) + `human_gate_reason` in `crates/vox-db/src/schema/domains/scientia.rs`. Reuse the same status strings.
- **Granularity (design decision #E): finding-level review, per-claim approval.** A discovery (`scientia_discoveries`, finding-level) expands to its atomic claims (`scientia_claims`); approval is **per claim**. So decisions key on `claim_id` (+ the finding/`publication_id` for grouping).
- **Digest binding:** approvals must bind to the claim's content so an edit invalidates the approval. Use the claim's `claim_id` (FNV-1a of text) + the publication `content_sha3_256` as the bound digest (mirror P1's `nanopub_key_ref`/digest pattern).
- **Table/ops pattern:** rows live inline in `crates/vox-db/src/store/ops_*.rs`, re-exported via `store/mod.rs`; baseline bump in `schema/manifest.rs` (now 71 → 72) **plus** the two companion artifacts (`contracts/db/baseline-version-policy.yaml` digest + `LEGACY_EXPORT_TABLES` in `codex_legacy.rs`) — see the P1 plan's "Baseline companion artifacts" note.
- **P1 emit site to gate:** `crates/vox-cli/src/commands/scientia_nanopub.rs::nanopub_build`.

## File Structure

- **Modify** `crates/vox-db/src/schema/domains/scientia.rs` — add `scientia_review_decisions` table.
- **Modify** `crates/vox-db/src/schema/manifest.rs` — `BASELINE_VERSION` 71 → 72 (+ companions).
- **Create** `crates/vox-db/src/store/ops_review.rs` — `ReviewDecisionRow`, `record_review_decision`, `latest_decision_for_claim`. (As-built: `list_claims_awaiting_review` instead landed in `crates/vox-db/src/research_pipeline.rs`, directly after `list_publication_claims`, because it is a `scientia_claims ⋈ scientia_claim_verdicts ⋈ scientia_review_decisions` join that reuses the `ScientiaClaimWithVerdict` row type and the verdict correlated-subquery idiom from that module — cohesion with the primary table's module won over grouping by review-domain.)
- **Create** `crates/vox-scientia/src/review/mod.rs` — the `ApprovalToken` type (opaque; only the review service constructs it) + the pure state-machine transitions.
- **Modify** `crates/vox-cli/src/commands/scientia_nanopub.rs` — `nanopub_build` requires `ApprovalToken`; add `publication-claim-review` handler.
- **Modify** `crates/vox-cli-core/src/scientia.rs` + dispatcher — wire `publication-claim-review`.

## Task 1: `scientia_review_decisions` table + ops

**Files:** `schema/domains/scientia.rs`, `schema/manifest.rs`, `store/ops_review.rs`, companions; test mirrors `ops_user_identity_tests.rs`.

- [ ] **Step 1 — failing test:** open `VoxDb::connect(DbConfig::Memory)`, `record_review_decision` (claim_id, publication_id, digest, decision="approved", actor="alice", reason=None), then `latest_decision_for_claim(claim_id)` returns it; a later `rejected` supersedes (latest wins by `decided_at_ms`).
- [ ] **Step 2:** run; verify FAIL.
- [ ] **Step 3 — DDL** (append to `SCHEMA_SCIENTIA`):
```sql
-- Append-only per-claim human review decisions (design §5.1). Latest by decided_at_ms wins.
CREATE TABLE IF NOT EXISTS scientia_review_decisions (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    claim_id          INTEGER NOT NULL,
    publication_id    TEXT,
    bound_digest      TEXT    NOT NULL,         -- publication content_sha3_256 at decision time
    decision          TEXT    NOT NULL,         -- approved|rejected|deferred|edited (validated in Rust)
    actor             TEXT    NOT NULL,         -- human user_id (local_user_id())
    reason            TEXT,
    model_fingerprints_json TEXT,               -- artifact-side model fps present (for AI disclosure)
    decided_at_ms     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scientia_review_decisions_claim
    ON scientia_review_decisions(claim_id, decided_at_ms);
```
- [ ] **Step 4:** bump `BASELINE_VERSION` 71 → 72 (comment: review decisions) + update `baseline-version-policy.yaml` (run the failing `baseline_policy_matches_compiled_schema` test, set the reported digest) + add `scientia_review_decisions` to `LEGACY_EXPORT_TABLES`.
- [ ] **Step 5:** `ReviewDecisionRow` + ops in `ops_review.rs` (mirror `ops_user_identity.rs`); validate the `decision` enum in Rust (no CHECK).
- [ ] **Step 6:** run until PASS (`cargo test -p vox-db review_decisions` + `--lib` 0 failures).
- [ ] **Step 7:** `cargo fmt -p vox-db`; commit.

## Task 2: `ApprovalToken` + review state machine (pure)

**Files:** `crates/vox-scientia/src/review/mod.rs`; wire `pub mod review;` in lib.rs; inline tests.

- [ ] **Step 1 — failing test:** `ApprovalToken` cannot be constructed outside `review` (compile-fence: private field); `mint_from_decision(&ReviewDecisionLike)` returns `Some(token)` only when `decision == "approved"`, `None` otherwise; the token exposes `claim_id()` + `bound_digest()`.
- [ ] **Step 2:** run; verify FAIL.
- [ ] **Step 3:** implement `pub struct ApprovalToken { claim_id: u64, bound_digest: String }` with a **private** constructor; `pub fn mint_from_decision(...) -> Option<ApprovalToken>`; a pure `next_state(current, action) -> ReviewState` for `surfaced→under_review→{approved,rejected,deferred,edited}` (edited → re-surface). Pure unit tests for every transition incl. invalid ones.
- [ ] **Step 4:** PASS; `cargo fmt -p vox-scientia`; commit.

## Task 3: gate `nanopub_build` behind `ApprovalToken`

**Files:** `crates/vox-cli/src/commands/scientia_nanopub.rs`; tests.

- [ ] **Step 1 — failing test:** `nanopub_build` now takes an `ApprovalToken`; calling the build path for a claim with **no approved decision** errors ("claim not approved for nanopublication — run publication-claim-review --decision approve") and persists **no** `scientia_nanopubs` row; with an approved decision (token minted) it succeeds and persists `local`.
- [ ] **Step 2:** run; verify FAIL.
- [ ] **Step 3:** change `nanopub_build(db, publication_id, claim_id, orcid)` → require an `ApprovalToken` argument whose `claim_id`/`bound_digest` match the manifest's current digest; the CLI arm first loads the latest decision, mints a token via `review::mint_from_decision`, and refuses if absent or digest-mismatched (a stale approval after an edit must not emit). Keep the P1 guard test (no publish symbols).
- [ ] **Step 4:** PASS; commit. *(This is the four-layer §9 guarantee's type + data layers made real.)*

## Task 4: `publication-claim-review` CLI

**Files:** `scientia_nanopub.rs`, `vox-cli-core/src/scientia.rs`, dispatcher, catalog baseline.

- [ ] **Step 1 — failing test:** `vox scientia publication-claim-review --publication-id X --claim-id N --decision approve|reject|defer [--reason R]` records a decision row (actor = `local_user_id()`, bound_digest = manifest digest) and prints it as JSON; mirror the `publication-extract-claims` wiring + update `command_catalog_paths_baseline.txt`.
- [ ] **Step 2–4:** implement the handler (load manifest for digest, `record_review_decision`), wire the Clap variant + dispatcher arm, run until PASS, commit.

## Task 5: surfacing read model

**Files:** `ops_review.rs`, `scientia_nanopub.rs` (or a small `publication-review-queue` handler); tests.

- [ ] **Step 1 — failing test:** `list_claims_awaiting_review(publication_id)` returns claims that have an extracted verdict but **no terminal decision** (no `approved`/`rejected` latest), grouped for a finding-level card; a `vox scientia publication-review-queue --publication-id X` prints them as JSON.
- [ ] **Step 2–4:** implement the query (join `scientia_claims` ⋈ latest `scientia_review_decisions`), the handler, run until PASS, commit.

## Acceptance (P2 done when all true)

- No `scientia_nanopubs` row can be created without a matching **approved** `scientia_review_decisions` row whose `bound_digest` equals the manifest's current digest (verified by Task 3 tests).
- `ApprovalToken` is non-constructible outside `vox_scientia::review` (compile-fence).
- A claim can be surfaced → reviewed → approved/rejected/deferred via `publication-claim-review`, with a full append-only audit trail.
- An **edit** (digest change) invalidates a prior approval (stale token rejected).
- `cargo test -p vox-db -p vox-scientia -p vox-cli` green; `vox ci ssot-drift` green (run from **source**, per the binary-freshness note); `vox-arch-check` exit 0.

## Deferred to P2b / P3

- REST `/api/v2/scientia/review` + WS `scientia.review.changed` (P2b — wire into the existing dashboard REST/WS surface).
- The `DiscoveryReview` GUI surface (P3) consumes this backend.
- The same `ApprovalToken` gate is applied to `syndicate()` in P5.

## Subsequent tracks (each its own plan + PR)

- **P3** DiscoveryReview GUI surface (registry-gated; consumes P2's read model + decision API).
- **P4** Surfacing accuracy: unify the two `NoveltyEvidenceBundle` types; promote the vox-scientia scorer; `InsufficientEvidence` driven by `query_traces`; **SPECTER2 via the shared `vox_actor_runtime::llm::llm_embed` facade** (+ local Candle backend); reachable `Contradicted` verdict; robust sentence splitter.
- **P5** One `syndicate(claim, channel)` SSOT seam (reference-not-copy manuscript rows; bridge to vox-publisher adapters; add Twitter; fix the UTF-8 truncation panic); all gated on `ApprovalToken`.
- **P6** Surfacing precision/recall eval harness + SSOT/conformance CI gates + consolidation + doc-drift.
