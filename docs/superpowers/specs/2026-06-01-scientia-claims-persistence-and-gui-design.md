# Scientia Claims — Persistence + Read + GUI — Design

**Date:** 2026-06-01
**Status:** Approved (build in full)
**Relates to:** CLI→GUI hybrid spine ([umbrella](2026-06-01-cli-gui-hybrid-spine-design.md)); Scientia T0–T2 claim ladder.

## Problem

The SCIENTIA claim-extraction pipeline is **already real and tested**
(`crates/vox-scientia/src/claim_extractor/`: VeriScore gate → atomic decomposition → span check →
MiniCheck verifier, with a deterministic mock backend usable offline). The DB write primitives
`store_claim` / `store_claim_verdict` / `store_evidence_span` exist in
`crates/vox-db/src/research_pipeline.rs`.

**Gap:** `publication_extract_claims` (in `crates/vox-cli/src/commands/scientia_phase_handlers.rs`)
runs the pipeline but persists only a *summary* into `publication_manifests.metadata_json` — it never
writes the individual claims/verdicts to the `scientia_claims` / `scientia_claim_verdicts` tables. There
are also **no read functions**, so nothing structured exists for a GUI. This builds that out end to end.

## Key facts (verified)

- `ExtractionResult { claims: Vec<AtomicClaim>, verdicts: Vec<ClaimVerdict>, promotable_claim_ids,
  abstained_sentence_count }`. `claims[i]` and `verdicts[i]` are **parallel** (built in one loop), so
  `claims[i].id` is the `claim_id` for `verdicts[i]`.
- `AtomicClaim { id: u64, text, tuple, span: SpanBound, verifiability: VerifiabilityClass,
  verifiability_score }`.
- `ClaimVerdict`: `Supported{confidence}` | `Contradicted{confidence}` | `Contested{confidence}` |
  `Abstain{reason}`.
- `store_claim` is idempotent (`INSERT OR IGNORE`; `claim_id` is a globally-UNIQUE FNV hash of the claim
  text). `store_claim_verdict` is a plain append.
- `scientia_claims` has `session_id` (no `publication_id` column).

## Design

### Decision 1 — publication ↔ claims association

No schema migration. Derive a stable `session_id: i64` from `publication_id` via FNV-1a (same hash family
already used for `claim_id`), in the vox-cli handler. All claims/verdicts for a publication share that
`session_id`. Documented limitation: because `claim_id` is globally unique by text-hash, two publications
sharing an identical claim sentence dedup to the first owner — an acceptable, pre-existing constraint.

### Decision 2 — persist orchestration lives in vox-cli (not vox-db)

vox-db is the data layer and must not depend on vox-scientia (layer rule). So the mapping
`ExtractionResult → store_*` calls lives in the vox-cli handler. For each `i`:
`store_claim(session_id, claims[i].id, claims[i].text, is_numeric, is_recent, is_named_event)` then
`store_claim_verdict(claims[i].id, verdict_str, confidence, model)`.
- `is_numeric = verifiability == Numeric`; `is_named_event = verifiability == EventBased`;
  `is_recent = false` (no source signal).
- verdict mapping: `Supported{c}→("Supported",c)`, `Contested{c}→("Contested",c)`,
  `Contradicted{c}→("Contradicted",c)`, `Abstain{..}→("Abstain",0.0)`.
- `model` = `"mock"` when `VOX_MINICHECK_ENDPOINT` is unset, else the endpoint (matches the existing
  handler's backend detection).
- Idempotency: claims dedup via `INSERT OR IGNORE`; verdicts append, and reads take the **latest** verdict
  per `claim_id` (verdict history is preserved, the view shows the newest).

The existing `metadata_json` summary write is kept (unchanged) — it complements the table persistence.

### Decision 3 — read primitives in vox-db

Two new functions in `research_pipeline.rs` (owner crate), following the existing `query_all` pattern,
returning `Serialize`-derived rows (row-serde-lint compliant):

```rust
pub async fn list_publication_claims(&self, session_id: i64) -> Result<Vec<ScientiaClaimWithVerdict>, StoreError>;
pub async fn scientia_claims_pending_summary(&self) -> Result<ClaimsPendingCounts, StoreError>;
```

- `ScientiaClaimWithVerdict { claim_id, text, is_numeric, verifiability_score: Option<f64>,
  verdict: Option<String>, confidence: Option<f64>, verifier_model: Option<String>, created_at_ms }`.
  Query LEFT JOINs `scientia_claims` to its newest `scientia_claim_verdicts` row (per `claim_id`,
  excluding the synthetic `'Unverified'` span rows) ordered by `created_at_ms`.
- `ClaimsPendingCounts { verifiable, abstained, extraction_running }` (global): `verifiable` = claims with a
  latest verdict `Supported`; `abstained` = latest verdict `Abstain`; `extraction_running` = claims with no
  non-span verdict row yet.

### Decision 4 — CLI read command

New `ScientiaCmd::Claims { publication_id: String }` → handler `publication_claims(&publication_id)`:
derive `session_id`, call `db.list_publication_claims(session_id)`, print the rows as JSON. Read-only.

### Decision 5 — GUI Claims surface

A `claims` decorator surface (registry + sidebar + View union). It:
1. runs `vox scientia publication-discovery-scan` to list publication ids (reuse existing read),
2. lets the operator pick / enter a `publication_id`,
3. runs `vox scientia claims --publication-id <id>` through the shared `execute_command` path,
4. renders each claim with a verdict badge (Supported = emerald, Contested = amber, Abstain = zinc,
   Contradicted = red), its confidence, and verifiability score.

All execution routes through `execute_command` (the runAction seam), so it also earns the universal CLI
reward.

## Build order (each stage verified independently)

- **Stage 1 — vox-db reads.** `list_publication_claims` + `scientia_claims_pending_summary` + the two
  `Serialize` row structs. Integration test: temp `VoxDb`, insert via existing `store_claim` /
  `store_claim_verdict`, assert the join/counts. (vox-db only.)
- **Stage 2 — vox-cli persist + read command.** Extend `publication_extract_claims` to persist; add
  `ScientiaCmd::Claims` + `publication_claims` handler; a pure unit test for the verdict→(str,confidence)
  mapping and the `session_id` derivation.
- **Stage 3 — GUI Claims surface.** New surface using the shared `execute_command` path; `pnpm build`.

## Testability

Fully deterministic end to end: the MiniCheck mock backend (word-overlap) needs no live LLM, so the
extractor + persistence + reads can be exercised offline in tests. Stage 1 uses a temp DB; Stage 2 unit
tests the pure mappings; Stage 3 verifies via `pnpm build`.

## Non-goals

- No schema migration (session_id derivation instead of a `publication_id` column).
- No live MiniCheck/LLM dependency (mock backend is the test path).
- No change to the extraction pipeline itself (it is already real); this wires persistence/read/GUI.
- The phantom `vox-dei-shim` research-orchestrator modules are out of scope (separate Phase 0a effort).
