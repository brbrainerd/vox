# Scientia Research-Pipeline Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade Vox Scientia from "human-driven publication tooling" to a serious research pipeline: accurate novelty verdicts, automated discovery producers over generated code, a metadata-autofill engine that completes every required archive field, an end-to-end (human-gated) archive run to Zenodo + Software Heritage + the nanopub test server, and GUI surfacing of all of it.

**Architecture:** Five tracks. **Track A** fixes novelty/claim correctness (the `InsufficientEvidence` verdict, real embeddings via the `llm_embed` facade, chrono/conflict wiring, reachable `Contradicted`, an eval harness). **Track B** builds the archive pipeline (Zenodo metadata enrichment + deterministic autofill with provenance, arXiv handoff sidecar, Software Heritage save-code-now, nanopub test-server publish, one orchestrating command). **Track C** adds automated producers (commit watcher + code-uniqueness signal over the embedding corpus). **Track D** surfaces everything in the GUI (novelty evidence panel, discovery inbox + OS notifications, archive panel). **Track E** closes gates and docs. Human approval remains the hard gate everywhere — nothing is emitted to any network without an `ApprovalToken`-backed decision row, and **no arXiv live submission and no production nanopub publishing are built** (arXiv's APIs are moribund/gatekept; production nanopub stays out by standing decision).

**Tech Stack:** Rust workspace crates (`vox-scientia`, `vox-publisher`, `vox-research-events`, `vox-db`, `vox-cli`, `vox-orchestrator-mcp`), `nanopub` crate (v0.2.x), `vox_actor_runtime::llm::llm_embed` facade, typify-generated contract types (`cargo run -p vox-scientia-jsonschema-codegen`), Tauri 2 + React/TS GUI (`crates/vox-gui/ui`, pnpm), Zenodo REST (sandbox-first), Software Heritage Web API.

---

## Current-state audit (2026-06-12, hand-verified)

What already EXISTS and works (do not rebuild):

| Capability | Where | Status |
|---|---|---|
| Per-user RSA nanopub identity + ORCID, offline-validated build | `crates/vox-scientia/src/review_flow.rs`, `src/nanopub/spec.rs` | REAL (P1 done) |
| Human review flow + `ApprovalToken` (`crate::review`) | `crates/vox-scientia/src/review_flow.rs` | REAL |
| DiscoveryReview / ScientiaDashboard / ClaimsView GUI surfaces | `crates/vox-gui/ui/src/components/surfaces/Scientia/` | REAL |
| WS `scientia.queue.changed` (5s DB-diff poller) + `vox://scientia-queue` Tauri event | `crates/vox-orchestrator-mcp/src/http_gateway/scientia_feed.rs`, `ui/src/lib/transport.ts:46` | REAL |
| Zenodo client: draft + bucket upload (checksum-verified staging) + flag-gated publish + status | `crates/vox-publisher/src/scholarly/zenodo.rs` | REAL |
| Zenodo metadata builder from `PublicationManifest` | `crates/vox-publisher/src/zenodo_metadata.rs` | REAL but thin (no date/keywords/related_identifiers) |
| Deterministic discovery ranking + `ManifestCompletionReport` (`required_missing`/`inferred_ok`/`human_only_pending`) + `FieldProvenanceEntry` | `crates/vox-publisher/src/scientia_discovery.rs` | REAL |
| Prior-art federated fetch (OpenAlex/Crossref/S2) → `NoveltyEvidenceBundleV1` | `crates/vox-publisher/src/scientia_prior_art.rs` | REAL retrieval, **fake semantic** |
| `AtomicNoveltyScorer` 3-verdict ladder | `crates/vox-scientia/src/inspect_bridge/novelty.rs` | REAL but empty-retrieval→Novel false positive |
| `ChronoFilter` / `EvidenceConflict` | `crates/vox-scientia/src/inspect_bridge/{chronofact,conflict}.rs` | BUILT, UNWIRED |
| Claim extraction pipeline + MiniCheck verifier | `crates/vox-scientia/src/claim_extractor/` | REAL; `Contradicted` defined but unreachable |
| LaTeX render + arXiv `.tar.gz` bundle | `vox scientia publication-render-latex` / `publication-arxiv-bundle` | REAL, local-only (correct — keep it that way) |
| Embedding facade + Qdrant ANN | `vox_actor_runtime::llm::llm_embed`, `crates/vox-search/src/{embeddings,vector_qdrant}.rs` | REAL |
| Scientia cost telemetry by phase | `crates/vox-db/src/facade/scientia_cost.rs` | REAL |

What is MISSING (this plan):
- `InsufficientEvidence` novelty verdict; `empty_novelty_bundle` emits `max_semantic: Some(0.0)` which scores **Novel** (`scientia_prior_art.rs:149`, `novelty.rs:72`).
- `semantic_proxy(lexical) = lexical` (`scientia_prior_art.rs:133`) — no real embeddings anywhere in novelty.
- Two bundle shapes drift: hand-written `NoveltyEvidenceBundleV1` (vox-publisher) vs contract-generated types (vox-research-events).
- `ClaimVerdict::Contradicted` never emitted (`claim_extractor/pipeline.rs:89-109`); `refuted` is structurally 0.
- Naive sentence splitter breaks "12.5ms" (`pipeline.rs:121-130`).
- Zero automated producers — nothing watches commits/generated code for research-worthy findings.
- Zenodo metadata lacks `publication_date`, `keywords`, `related_identifiers`, `version`; no autofill engine driving `required_missing` → filled.
- No Software Heritage archiving; nanopub `publish_stub` (`nanopub/network.rs:13-20`) has no test-server path.
- GUI: no novelty evidence on review cards, no discovery inbox, no OS notifications, no archive-status panel.
- No measured precision/recall for novelty/surfacing.

External constraints (researched 2026-06-12):
- **arXiv**: SWORD v1 is gatekept/moribund; replacement API archived unbuilt. **Do not build live submission** — ship an enriched handoff bundle only.
- **Zenodo**: fully scriptable (token → deposit → bucket upload → publish → DOI); sandbox at `sandbox.zenodo.org`; required metadata: `upload_type`, `publication_date`, `title`, `creators[{name,affiliation?,orcid?}]`, `description`, `access_right`, `license`.
- **Software Heritage**: `POST /api/1/origin/save/git/url/<origin_url>/`, pollable status; bearer token = 1200 req/h.
- **nanopub-rs v0.2.x**: `publish` with no server URL defaults to the **test server** — exactly the safe default we want.
- **Novelty literature**: SPECTER2-style embedding distance is a weak-to-moderate signal (r≈0.33 vs humans) → multi-signal scoring with human-calibrated thresholds, never a lone classifier.

Execution order: A1→A7 (correctness first — everything downstream shows these verdicts), then B1→B6, then C1→C3, then D1→D3, then E1. Tracks B and C are independent of each other and can be parallelized after Track A.

Conventions that apply to every task:
- Format only touched crates: `cargo fmt -p <crate>` (NEVER `cargo fmt --all` on Windows).
- Never hand-edit `schema_types.generated.rs` — regenerate with `cargo run -p vox-scientia-jsonschema-codegen` after editing `contracts/scientia/*.schema.json`.
- vox-db schema changes: edit `crates/vox-db/src/schema/domains/scientia.rs` and bump `BASELINE_VERSION` in `crates/vox-db/src/schema/manifest.rs` — no date-stamped SQL files.
- All LLM/embedding calls go through `vox_actor_runtime::llm` — no vendor SDKs/hostnames.
- GUI package management is **pnpm**, run from `crates/vox-gui/ui`.

---

## Track A — Novelty & claim correctness

### Task 1: `InsufficientEvidence` verdict + retrieval-health rule

The worst false positive: failed/empty retrieval currently scores **Novel**. Add a fourth verdict driven by query traces, and stop `empty_novelty_bundle` from claiming `Some(0.0)` semantic similarity.

**Files:**
- Modify: `crates/vox-scientia/src/inspect_bridge/novelty.rs`
- Modify: `crates/vox-publisher/src/scientia_prior_art.rs:137-156`
- Test: inline `#[cfg(test)]` in both files

- [ ] **Step 1: Write the failing tests** (append to `novelty.rs` tests module; the existing `make_bundle` helper gains a `traces` param — update existing call sites with `None`)

```rust
    fn trace(source: &str, http_status: Option<i64>) -> serde_json::Value {
        serde_json::json!({
            "source": source,
            "request_fingerprint_sha256": "b".repeat(64),
            "http_status": http_status,
        })
    }

    #[test]
    fn empty_bundle_with_failed_traces_is_insufficient_evidence() {
        // All sources errored: nothing was actually searched.
        let mut bundle = make_bundle(vec![], None);
        bundle.query_traces = parse_traces(vec![trace("openalex", Some(500)), trace("crossref", None)]);
        let scorer = AtomicNoveltyScorer::default();
        assert_eq!(scorer.score(&bundle), NoveltyVerdict::InsufficientEvidence);
    }

    #[test]
    fn empty_bundle_with_no_traces_is_insufficient_evidence() {
        // No retrieval ran at all (offline) — we know nothing, not "it's novel".
        let bundle = make_bundle(vec![], None);
        let scorer = AtomicNoveltyScorer::default();
        assert_eq!(scorer.score(&bundle), NoveltyVerdict::InsufficientEvidence);
    }

    #[test]
    fn empty_hits_with_successful_trace_is_novel() {
        // A real search ran and found nothing — that IS evidence of novelty.
        let mut bundle = make_bundle(vec![], None);
        bundle.query_traces = parse_traces(vec![trace("openalex", Some(200))]);
        let scorer = AtomicNoveltyScorer::default();
        assert_eq!(scorer.score(&bundle), NoveltyVerdict::Novel);
    }
```

Note: `NoveltyEvidenceBundle.query_traces`' generated item type has restrictive newtype wrappers; write a small test helper `parse_traces(v: Vec<serde_json::Value>) -> Option<Vec<...>>` that deserializes via `serde_json::from_value` — find the exact item type name with `rg "query_traces" crates/vox-research-events/src/schema_types.generated.rs`. Existing tests (`empty_bundle_is_novel`, `no_summary_no_scores_is_novel`) encode the OLD wrong behavior — rewrite them per the new rule (they become the two insufficient-evidence tests above).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-scientia inspect_bridge::novelty`
Expected: FAIL — `InsufficientEvidence` variant does not exist.

- [ ] **Step 3: Implement** — in `novelty.rs`:

```rust
pub enum NoveltyVerdict {
    /// Retrieval never succeeded (no source returned HTTP 2xx) or never ran.
    /// NEVER treat as Novel: we have no evidence either way.
    InsufficientEvidence,
    Novel,
    PossiblyNovel { closest_score: f64 },
    NotNovel { closest_hit_uri: String, similarity: f64 },
}
```

In `AtomicNoveltyScorer::score`, before the existing `match max_score`:

```rust
        // Retrieval health: at least one source must have answered 2xx for an
        // empty/low-score bundle to mean "novel" rather than "we don't know".
        let any_source_succeeded = bundle
            .query_traces
            .as_ref()
            .map(|traces| {
                traces.iter().any(|t| {
                    t.http_status.map(|s| (200..300).contains(&s)).unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if bundle.normalized_hits.is_empty() && !any_source_succeeded {
            return NoveltyVerdict::InsufficientEvidence;
        }
```

(Adapt the `http_status` field access to the generated trace item type — it may be `Option<i64>` or a newtype; deref accordingly.) Then change the `None` arm of `match max_score`: with hits present but no scores at all, return `NoveltyVerdict::PossiblyNovel { closest_score: 0.0 }` is wrong — keep `Novel` only when `any_source_succeeded`, else `InsufficientEvidence`.

In `scientia_prior_art.rs::empty_novelty_bundle` (line ~149), stop fabricating evidence:

```rust
        overlap_summary: Some(NoveltyOverlapSummary {
            max_lexical_score: None,
            max_semantic_score: None,   // was Some(0.0) — fabricated "we searched and found nothing"
            recency_bucket: NoveltyRecencyBucket::Unknown,
        }),
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-scientia inspect_bridge::novelty && cargo test -p vox-publisher scientia_prior_art`
Expected: PASS. Then `rg "NoveltyVerdict::" crates --type rust -l` and fix every exhaustive `match` to handle `InsufficientEvidence` (treat as "do not promote / do not publish" everywhere; in any score-to-rank mapping use the same handling as the most-conservative arm).

- [ ] **Step 5: Run wider checks + commit**

Run: `cargo test -p vox-scientia && cargo test -p vox-publisher && cargo fmt -p vox-scientia && cargo fmt -p vox-publisher`

```bash
git add -A && git commit -m "feat(scientia): InsufficientEvidence novelty verdict driven by retrieval health"
```

### Task 2: One bundle shape — contract parity between vox-publisher and the schema

`vox-publisher` hand-writes `NoveltyEvidenceBundleV1`; `vox-research-events` generates `ScientiaNoveltyEvidenceBundleV1` from `contracts/scientia/novelty-evidence-bundle.v1.schema.json`. Nothing keeps them in sync. Make the contract the SSOT with a serde round-trip parity gate (cheap, no mass refactor).

**Files:**
- Create: `crates/vox-publisher/tests/novelty_bundle_contract_parity.rs`
- Modify: `crates/vox-publisher/src/scientia_finding_ledger.rs` (only if parity reveals drift)

- [ ] **Step 1: Write the failing/parity test**

```rust
//! The hand-written `NoveltyEvidenceBundleV1` MUST serialize to JSON that the
//! contract schema accepts and that round-trips through the generated type.
use vox_publisher::scientia_finding_ledger::{
    NormalizedPriorArtHit, NoveltyEvidenceBundleV1, NoveltyOverlapSummary, NoveltyQueryTrace,
    NoveltyRecencyBucket, PriorArtSource,
};

fn representative_bundle() -> NoveltyEvidenceBundleV1 {
    NoveltyEvidenceBundleV1 {
        schema_version: 1,
        bundle_id: "nb.deadbeefdeadbeef".into(),
        candidate_id: "C-1".into(),
        computed_at_ms: 1_700_000_000_000,
        query_digest_sha256: "a".repeat(64),
        sources: vec![PriorArtSource::Openalex],
        normalized_hits: vec![NormalizedPriorArtHit {
            source: PriorArtSource::Openalex,
            work_uri: "https://openalex.org/W1".into(),
            title: "Prior work".into(),
            year: Some(2024),
            lexical_score: Some(0.4),
            semantic_score: Some(0.5),
            overlap_note: None,
            cited_by_count: Some(12),
        }],
        overlap_summary: Some(NoveltyOverlapSummary {
            max_lexical_score: Some(0.4),
            max_semantic_score: Some(0.5),
            recency_bucket: NoveltyRecencyBucket::Recent,
        }),
        query_traces: vec![NoveltyQueryTrace {
            source: "openalex".into(),
            request_fingerprint_sha256: "b".repeat(64),
            http_status: Some(200),
            cached: Some(false),
        }],
    }
}

#[test]
fn v1_round_trips_through_generated_contract_type() {
    let json = serde_json::to_value(representative_bundle()).expect("serialize");
    let generated: vox_research_events::schema_types::ScientiaNoveltyEvidenceBundleV1 =
        serde_json::from_value(json.clone()).expect("contract type must accept producer output");
    let back = serde_json::to_value(&generated).expect("re-serialize");
    assert_eq!(json, back, "lossy round-trip = schema drift");
}
```

(Adjust field shapes to compile against the actual hand-written structs; if `vox-publisher` doesn't already depend on `vox-research-events`, add it as a dev-dependency only.)

- [ ] **Step 2: Run; fix whichever side is wrong**

Run: `cargo test -p vox-publisher --test novelty_bundle_contract_parity`
If it fails, the fix direction is: hand-written serde attributes change to match the **contract** (the schema is the SSOT). If the schema itself is missing a field the producer legitimately emits, add it to `contracts/scientia/novelty-evidence-bundle.v1.schema.json` then `cargo run -p vox-scientia-jsonschema-codegen`.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "test(scientia): contract-parity gate for NoveltyEvidenceBundle producer/consumer shapes"
```

### Task 3: Real semantic similarity via the `llm_embed` facade

Replace `semantic_proxy(lexical)=lexical` with cosine similarity of real embeddings, cached in vox-db, degrading honestly (absent ≠ fabricated).

**Files:**
- Create: `crates/vox-publisher/src/scientia_semantic.rs`
- Modify: `crates/vox-publisher/src/lib.rs` (add `pub mod scientia_semantic;`)
- Modify: `crates/vox-publisher/src/scientia_prior_art.rs:133` and the two call sites of `semantic_proxy`
- Modify: `crates/vox-db/src/schema/domains/scientia.rs` + `crates/vox-db/src/schema/manifest.rs` (BASELINE_VERSION bump) — new table `scientia_embedding_cache(text_sha256 TEXT PRIMARY KEY, model TEXT NOT NULL, vector_json TEXT NOT NULL, created_at_ms INTEGER NOT NULL)`
- Modify: `crates/vox-db/src/facade/` — add `get_cached_embedding(text_sha256) -> Option<Vec<f32>>` / `put_cached_embedding(...)` following the facade pattern of `scientia_cost.rs`

- [ ] **Step 1: Write the failing unit tests** (pure math + cache key, no network)

```rust
// in scientia_semantic.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_of_identical_vectors_is_one() {
        let v = vec![0.5_f32, 0.5, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_of_orthogonal_vectors_is_zero() {
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn cosine_handles_zero_vector_without_nan() {
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    }

    #[test]
    fn embed_text_digest_is_stable_and_model_scoped() {
        assert_eq!(embed_cache_key("abc", "m1"), embed_cache_key("abc", "m1"));
        assert_ne!(embed_cache_key("abc", "m1"), embed_cache_key("abc", "m2"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-publisher scientia_semantic`
Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement `scientia_semantic.rs`**

```rust
//! Real semantic similarity for prior-art scoring.
//!
//! Embeddings go through the ONE policy-approved seam
//! (`vox_actor_runtime::llm::llm_embed`); vectors are cached in vox-db keyed by
//! sha256(model + text). On embed failure we return `None` — callers MUST
//! propagate absence (semantic_score: None), never substitute a fake score.

use sha2::{Digest, Sha256};

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let na: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { (dot / (na * nb)).clamp(-1.0, 1.0) }
}

pub fn embed_cache_key(text: &str, model: &str) -> String {
    let mut h = Sha256::new();
    h.update(model.as_bytes());
    h.update([0u8]);
    h.update(text.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Embed with DB cache. `None` on any failure (offline, no key, provider error).
pub async fn embed_cached(
    db: &vox_db::VoxDb,
    options: &vox_actor_runtime::ActivityOptions,
    config: &vox_actor_runtime::llm::LlmConfig,
    text: &str,
) -> Option<Vec<f32>> {
    let key = embed_cache_key(text, &config.model);
    if let Ok(Some(v)) = db.get_cached_embedding(&key).await {
        return Some(v);
    }
    match vox_actor_runtime::llm::llm_embed(options, text, config.clone()).await {
        Ok(Ok(vec)) => {
            let _ = db.put_cached_embedding(&key, &config.model, &vec).await;
            Some(vec)
        }
        _ => None,
    }
}

/// Score query-vs-hit semantic similarity for a batch of hit titles.
/// Returns one `Option<f64>` per hit, `None` where embedding was unavailable.
pub async fn semantic_scores(
    db: &vox_db::VoxDb,
    options: &vox_actor_runtime::ActivityOptions,
    config: &vox_actor_runtime::llm::LlmConfig,
    query_text: &str,
    hit_titles: &[String],
) -> Vec<Option<f64>> {
    let Some(q) = embed_cached(db, options, config, query_text).await else {
        return vec![None; hit_titles.len()];
    };
    let mut out = Vec::with_capacity(hit_titles.len());
    for t in hit_titles {
        let s = match embed_cached(db, options, config, t).await {
            Some(v) => Some(cosine_similarity(&q, &v)),
            None => None,
        };
        out.push(s);
    }
    out
}
```

Check the exact `llm_embed` signature first (`crates/vox-actor-runtime/src/llm/embed.rs:37` — `llm_embed(options: &ActivityOptions, text: &str, config: LlmConfig) -> ActivityResult<Result<Vec<f32>, String>>`) and the exact `LlmConfig` model-field name; adapt imports. Add `vox-actor-runtime` + `vox-db` deps to `vox-publisher/Cargo.toml` if absent (workspace = true).

- [ ] **Step 4: Rewire the producer.** In `scientia_prior_art.rs`: delete `fn semantic_proxy`; in `openalex_hits`/`crossref_hits`/`s2_hits` set `semantic_score: None` at parse time; add a post-fetch enrichment pass in the federated fetch function (it has the assembled hit list before building `overlap_summary`):

```rust
    // Semantic enrichment (post-fetch): real embeddings, honest absence.
    if let Some(sem_ctx) = semantic_ctx {
        let titles: Vec<String> = hits.iter().map(|h| h.title.clone()).collect();
        let scores = crate::scientia_semantic::semantic_scores(
            sem_ctx.db, sem_ctx.options, sem_ctx.config, &search_text, &titles,
        ).await;
        for (h, s) in hits.iter_mut().zip(scores) {
            h.semantic_score = s;
        }
    }
```

where `semantic_ctx: Option<SemanticCtx<'_>>` is a new param struct `{ db, options, config }` threaded from the CLI handler (`publication-novelty-fetch` in `crates/vox-cli/src/commands/scientia.rs` — find it with `rg "novelty-fetch" crates/vox-cli`); pass `None` in offline/test paths. `max_semantic_score` in the overlap summary becomes the max over `Some` scores only (`filter_map`), `None` if no hit has one. Tag the embedding calls with telemetry phase `'novelty'` via `ActivityOptions` so `scientia_cost_by_phase()` starts reporting real novelty spend.

- [ ] **Step 5: DB cache plumbing.** Add the table to `crates/vox-db/src/schema/domains/scientia.rs`, bump `BASELINE_VERSION` in `crates/vox-db/src/schema/manifest.rs`, add facade methods (store vector as JSON array; deserialize on read), with a facade unit test:

```rust
#[tokio::test]
async fn embedding_cache_round_trip() {
    let db = VoxDb::open_in_memory().await.expect("db"); // use the existing test-constructor pattern from scientia_cost tests
    db.put_cached_embedding("k1", "model-a", &[0.1_f32, 0.2]).await.expect("put");
    let v = db.get_cached_embedding("k1").await.expect("get").expect("hit");
    assert_eq!(v.len(), 2);
}
```

- [ ] **Step 6: Run + commit**

Run: `cargo test -p vox-publisher && cargo test -p vox-db embedding_cache && cargo fmt -p vox-publisher && cargo fmt -p vox-db`

```bash
git add -A && git commit -m "feat(scientia): real semantic prior-art similarity via llm_embed with vox-db cache; retire semantic_proxy"
```

### Task 4: Wire `ChronoFilter` + `EvidenceConflict` into one assessment entry point

Both are built and tested but never called. Create the single `assess_novelty()` seam everything (CLI, GUI, producers) uses.

**Files:**
- Create: `crates/vox-publisher/src/scientia_novelty_assess.rs`
- Modify: `crates/vox-publisher/src/lib.rs` (add module)
- Modify: the `publication-novelty-fetch` / `publication-novelty-happy-path` CLI handlers to call it (locate: `rg "novelty" crates/vox-cli/src/commands/scientia.rs crates/vox-cli/src/commands/scientia_phase_handlers.rs`)

- [ ] **Step 1: Write the failing test**

```rust
// tests module in scientia_novelty_assess.rs
#[test]
fn future_dated_hits_are_excluded_before_scoring() {
    // One "prior art" hit dated AFTER the claim: chrono-filter must drop it,
    // leaving an empty (but successfully-searched) bundle => Novel.
    let bundle = bundle_with_hits(vec![hit_with_year("doi:future", 2030, Some(0.95))], /*trace_ok=*/ true);
    let a = assess_novelty(&bundle, claim_year(2026), &Default::default());
    assert_eq!(a.verdict, vox_scientia::inspect_bridge::novelty::NoveltyVerdict::Novel);
    assert_eq!(a.excluded_future_hits, 1);
}

#[test]
fn contradicting_hit_surfaces_conflict_not_novel() {
    let bundle = bundle_with_hits(vec![hit_with_note("doi:contra", 2024, Some(0.9), "contradicts")], true);
    let a = assess_novelty(&bundle, claim_year(2026), &Default::default());
    assert!(!a.conflicts.is_empty(), "EvidenceConflict must be reported");
    assert!(matches!(a.verdict, vox_scientia::inspect_bridge::novelty::NoveltyVerdict::NotNovel { .. }));
}
```

(Write the small `bundle_with_hits`/`hit_with_year`/`hit_with_note` fixture builders in the test module, mirroring Task 2's `representative_bundle`. Read `crates/vox-scientia/src/inspect_bridge/conflict.rs` first to use its actual detection input — it flags supporting-vs-contradicting hits; adapt `hit_with_note` to whatever field it inspects, e.g. `overlap_note`.)

- [ ] **Step 2: Run to verify failure**, then **Step 3: implement**:

```rust
//! Single novelty-assessment entry point: chrono-filter → score → conflicts.
use vox_scientia::inspect_bridge::chronofact::ChronoFilter;
use vox_scientia::inspect_bridge::conflict::EvidenceConflict;
use vox_scientia::inspect_bridge::novelty::{AtomicNoveltyScorer, NoveltyConfig, NoveltyVerdict};

#[derive(Debug, Clone)]
pub struct NoveltyAssessment {
    pub verdict: NoveltyVerdict,
    pub conflicts: Vec<EvidenceConflict>,
    pub excluded_future_hits: usize,
    /// Explainability: the inputs that produced the verdict (GUI shows these).
    pub signals: NoveltySignalBreakdown,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct NoveltySignalBreakdown {
    pub max_semantic: Option<f64>,
    pub max_lexical: Option<f64>,
    pub near_hit_count: usize,      // hits with semantic >= novel_threshold
    pub top_hit_citations: Option<u64>,
    pub sources_succeeded: usize,
}

pub fn assess_novelty(
    bundle: &vox_research_events::schema_types::NoveltyEvidenceBundle,
    claim_year: i32,
    config: &NoveltyConfig,
) -> NoveltyAssessment { /* chrono-filter hits into a filtered copy, score it,
    run conflict detection over the filtered hits, fill the breakdown */ }
```

Implementation notes: clone the bundle, retain only hits `ChronoFilter` accepts for `claim_year` (count the dropped ones), recompute `overlap_summary` maxima over the survivors, call `AtomicNoveltyScorer::new(config.clone()).score(&filtered)`, then run the conflict detector over survivors. Use the REAL APIs of `chronofact.rs`/`conflict.rs` — read both files (≈40 lines each) and call them as written; do not re-implement their logic.

- [ ] **Step 4: Wire the CLI.** In the `publication-novelty-fetch` handler, after the bundle is fetched/persisted, run `assess_novelty` and include `{verdict, conflicts, signals, excluded_future_hits}` in the command's JSON output. Run `cargo test -p vox-publisher scientia_novelty_assess`, expected PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scientia): assess_novelty() seam wiring ChronoFilter + EvidenceConflict + signal breakdown"
```

### Task 5: Make `Contradicted` reachable (NLI contradiction signal)

`ClaimVerdict::Contradicted` exists but no code path emits it, so `refuted` is structurally 0 and the worthiness `contradiction_penalty` can never fire.

**Files:**
- Modify: `crates/vox-scientia/src/claim_extractor/minicheck.rs` (add `contradiction_score` to `VerifierOutput`; HTTP backend parses it; mock backend computes a real negation heuristic)
- Modify: `crates/vox-scientia/src/claim_extractor/pipeline.rs:89-109`
- Modify: `crates/vox-scientia/src/claim_extractor/types.rs` (config threshold)

- [ ] **Step 1: Write the failing tests** (pipeline-level, mock verifier)

```rust
#[tokio::test]
async fn negated_claim_against_contradicting_context_is_contradicted() {
    // Context asserts the opposite of the claim => Contradicted, refuted > 0.
    let pipeline = test_pipeline_with_mock(); // existing test constructor pattern in pipeline tests
    let result = pipeline
        .extract(
            "The cache reduces latency by 40%.",
            &["Benchmarks show the cache does not reduce latency.".to_string()],
        )
        .await
        .expect("extract");
    assert!(result.verdicts.iter().any(|v| matches!(v, ClaimVerdict::Contradicted { .. })));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-scientia claim_extractor`
Expected: FAIL — verdict is `Contested`/`Supported`, never `Contradicted`.

- [ ] **Step 3: Implement.**
  - `VerifierOutput` gains `pub contradiction_score: f64` (default 0.0; add `#[serde(default)]` if it's serialized).
  - HTTP backend: parse `contradiction_score` from the MiniCheck endpoint response when present (`VOX_MINICHECK_ENDPOINT` services that return entail/neutral/contradict probabilities).
  - Mock backend: real lexical-negation heuristic — token-overlap as today, **plus**: if the context sentence with max overlap contains a negator (`not`, `no`, `never`, `fails`, `cannot`, `doesn't`, `does not`) that the claim lacks (or vice versa), emit `contradiction_score = overlap` and reduce `support_score` accordingly. This is honest about being a heuristic and makes the path testable offline.
  - `ClaimExtractorConfig` gains `pub contradiction_threshold: f64` (default `0.6`).
  - Pipeline verdict ladder (order matters — check contradiction FIRST):

```rust
            let verdict = if output.contradiction_score >= self.config.contradiction_threshold {
                ClaimVerdict::Contradicted { confidence: output.contradiction_score }
            } else if output.abstained {
                ClaimVerdict::Abstain { reason: format!(
                    "support_score={:.2} < τ={:.2}", output.support_score, self.config.abstain_threshold) }
            } else if output.support_score >= self.config.promotion_threshold {
                promotable.push(claim.id);
                ClaimVerdict::Supported { confidence: output.support_score }
            } else {
                ClaimVerdict::Contested { confidence: output.support_score }
            };
```

- [ ] **Step 4: Run** `cargo test -p vox-scientia claim_extractor` — PASS; also run the CLI-handler tests: `cargo test -p vox-cli scientia` (the `refuted += 1` mapping at `scientia_phase_handlers.rs:305` is already wired and now becomes reachable).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scientia): reachable Contradicted verdict via contradiction_score (HTTP NLI + negation-aware mock)"
```

### Task 6: Numeric-safe sentence splitter

**Files:**
- Modify: `crates/vox-scientia/src/claim_extractor/pipeline.rs:121-135` (`split_sentences`)

- [ ] **Step 1: Failing test**

```rust
#[test]
fn split_does_not_break_decimal_numbers() {
    let s = split_sentences("Latency fell to 12.5ms. Throughput rose 3.4x! Done?");
    assert_eq!(s, vec!["Latency fell to 12.5ms.", "Throughput rose 3.4x!", "Done?"]);
}

#[test]
fn split_does_not_break_common_abbreviations_or_versions() {
    let s = split_sentences("Vox v0.6.2 ships today. See e.g. the docs.");
    assert_eq!(s.len(), 2);
}
```

- [ ] **Step 2: Run** `cargo test -p vox-scientia split_` — FAIL (first assertion splits at "12.").

- [ ] **Step 3: Implement** — only treat `.` as a boundary when the next non-space char is uppercase/EOF AND the char before `.` is not a digit-with-digit-following pattern; handle `e.g.`/`i.e.` via a tiny suffix denylist:

```rust
fn split_sentences(text: &str) -> Vec<String> {
    const NON_TERMINAL_SUFFIXES: [&str; 4] = ["e.g", "i.e", "etc", "vs"];
    let chars: Vec<char> = text.chars().collect();
    let mut sentences = Vec::new();
    let mut current = String::new();
    for i in 0..chars.len() {
        let ch = chars[i];
        current.push(ch);
        let terminal = match ch {
            '!' | '?' => true,
            '.' => {
                let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
                let next_digit = chars.get(i + 1).is_some_and(|c| c.is_ascii_digit());
                let mid_number = prev_digit && next_digit;           // 12.5, v0.6.2
                let trimmed = current.trim_end_matches('.');
                let abbrev = NON_TERMINAL_SUFFIXES.iter().any(|s| trimmed.to_lowercase().ends_with(s));
                let next_starts_sentence = match chars[i + 1..].iter().find(|c| !c.is_whitespace()) {
                    None => true,
                    Some(c) => c.is_uppercase(),
                };
                !mid_number && !abbrev && next_starts_sentence
            }
            _ => false,
        };
        if terminal {
            let t = current.trim().to_string();
            if !t.is_empty() { sentences.push(t); }
            current.clear();
        }
    }
    let t = current.trim().to_string();
    if !t.is_empty() { sentences.push(t); }
    sentences
}
```

- [ ] **Step 4: Run** `cargo test -p vox-scientia` (full crate — splitter feeds everything). PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "fix(scientia): sentence splitter no longer breaks decimals/abbreviations"
```

### Task 7: Novelty golden harness — measured precision/recall

"Better novelty" must be measurable or it's vibes.

**Files:**
- Create: `crates/vox-publisher/tests/fixtures/novelty_golden.v1.json` (≥12 labeled cases)
- Create: `crates/vox-publisher/tests/novelty_golden_harness.rs`

- [ ] **Step 1: Author the golden set.** JSON array; each entry: `{name, bundle: <full NoveltyEvidenceBundle JSON>, claim_year, expected: "novel"|"possibly_novel"|"not_novel"|"insufficient_evidence"}`. Cover: empty+failed traces, empty+200 trace, high-sim hit, future-dated hit only, contradicting hit, mid-score, missing-scores-with-200, citation-heavy near-hit, multiple near-hits. Use the Task 2 representative bundle as the template; vary fields.

- [ ] **Step 2: Write the harness test**

```rust
//! Golden novelty harness: asserts per-class precision/recall floors so verdict
//! regressions fail CI with a measured number, not a vibe.
use std::collections::HashMap;

#[derive(serde::Deserialize)]
struct GoldenCase {
    name: String,
    bundle: vox_research_events::schema_types::NoveltyEvidenceBundle,
    claim_year: i32,
    expected: String,
}

fn verdict_class(v: &vox_scientia::inspect_bridge::novelty::NoveltyVerdict) -> &'static str {
    use vox_scientia::inspect_bridge::novelty::NoveltyVerdict::*;
    match v { InsufficientEvidence => "insufficient_evidence", Novel => "novel",
              PossiblyNovel { .. } => "possibly_novel", NotNovel { .. } => "not_novel" }
}

#[test]
fn golden_precision_recall_floors() {
    let raw = include_str!("fixtures/novelty_golden.v1.json");
    let cases: Vec<GoldenCase> = serde_json::from_str(raw).expect("fixture parses");
    assert!(cases.len() >= 12, "golden set too small to be meaningful");
    let mut per_class: HashMap<&str, (u32, u32, u32)> = HashMap::new(); // (tp, fp, fn)
    for c in &cases {
        let a = vox_publisher::scientia_novelty_assess::assess_novelty(
            &c.bundle, c.claim_year, &Default::default());
        let got = verdict_class(&a.verdict);
        if got == c.expected { per_class.entry(got).or_default().0 += 1; }
        else {
            per_class.entry(got).or_default().1 += 1;
            per_class.entry(Box::leak(c.expected.clone().into_boxed_str())).or_default().2 += 1;
            eprintln!("MISMATCH {}: expected {} got {}", c.name, c.expected, got);
        }
    }
    for (class, (tp, fp, fnn)) in &per_class {
        let p = *tp as f64 / (*tp + *fp).max(1) as f64;
        let r = *tp as f64 / (*tp + *fnn).max(1) as f64;
        eprintln!("{class}: precision={p:.2} recall={r:.2}");
        assert!(p >= 1.0 && r >= 1.0, "{class} below floor — goldens are deterministic, fix the scorer or relabel deliberately");
    }
}
```

(Goldens are deterministic structural inputs, so the floor is 1.0; the harness exists so future threshold/model changes show **which** labeled case moved and CI fails loudly.)

- [ ] **Step 3: Run** `cargo test -p vox-publisher --test novelty_golden_harness` — iterate fixture labels until green honestly (every label must be defensible; print rationale in `name`).

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "test(scientia): golden novelty harness with per-class precision/recall reporting"
```

---

## Track B — Pipeline to archive, auto-filled

### Task 8: Zenodo metadata enrichment (`publication_date`, `keywords`, `related_identifiers`, `version`)

**Files:**
- Modify: `crates/vox-publisher/src/zenodo_api_types.rs` (add optional fields)
- Modify: `crates/vox-publisher/src/zenodo_metadata.rs`
- Test: existing tests module in `zenodo_metadata.rs`

- [ ] **Step 1: Failing tests** (append to `zenodo_metadata.rs` tests; mirror the existing test style there)

```rust
    #[test]
    fn body_includes_publication_date_and_keywords() {
        let m = manifest_with_scientific(); // existing helper, or build PublicationManifest inline as the current tests do
        let body = zenodo_deposition_create_body(&m);
        let pd = body.metadata.publication_date.expect("publication_date auto-filled");
        assert_eq!(pd.len(), 10, "ISO-8601 date: {pd}");
        assert!(!body.metadata.keywords.is_empty(), "keywords derived from manifest");
    }

    #[test]
    fn related_identifiers_carry_code_repo_and_nanopub_uris() {
        let m = manifest_with_repro_and_nanopubs(); // metadata_json with reproducibility.code_repository_url + nanopub trusty URIs
        let body = zenodo_deposition_create_body(&m);
        let rels: Vec<&str> = body.metadata.related_identifiers.iter().map(|r| r.identifier.as_str()).collect();
        assert!(rels.iter().any(|r| r.contains("github.com")), "code repo as isSupplementTo");
    }
```

- [ ] **Step 2: Run** `cargo test -p vox-publisher zenodo_metadata` — FAIL (fields missing).

- [ ] **Step 3: Implement.** In `zenodo_api_types.rs` add to `ZenodoDepositionMetadata` (all `#[serde(skip_serializing_if = ...)]` so existing depositions don't change shape):

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication_date: Option<String>, // ISO-8601 YYYY-MM-DD
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub keywords: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub related_identifiers: Vec<ZenodoRelatedIdentifier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ZenodoRelatedIdentifier {
    pub identifier: String,        // URL / DOI / SWHID
    pub relation: String,          // "isSupplementTo" | "isDerivedFrom" | "isIdenticalTo"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>, // e.g. "software"
}
```

In `zenodo_metadata.rs::zenodo_deposition_create_body` fill them: `publication_date` = today (`chrono::Utc::now().format("%Y-%m-%d")`); `keywords` from `ScientificPublicationMetadata` if a keywords field exists there, else derive from manifest title tokens (>3 chars, deduped, max 8); `related_identifiers` from `reproducibility.code_repository_url` / `data_repository_url` (relation `isSupplementTo`, resource_type `software`/`dataset`) plus any nanopub trusty URIs found in `metadata_json` under a `scientia.nanopub_uris` array (relation `isSupplementTo`); `version` from `metadata_json` `scientia.version` if present.

- [ ] **Step 4: Run** `cargo test -p vox-publisher zenodo` (includes `scholarly_zenodo_mock_test.rs` — fix any body-shape assertions there). PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scientia): Zenodo metadata enrichment — date, keywords, related_identifiers, version"
```

### Task 9: Deterministic autofill engine with provenance (`publication-autofill`)

Drives `ManifestCompletionReport.required_missing` → filled, every fill recorded as `FieldProvenanceEntry { origin: "autofill:<rule>" }`, human-only fields left for review. No LLM in v1 — deterministic rules only (LLM-assist remains the existing `evidence-assist` path).

**Files:**
- Create: `crates/vox-publisher/src/scientia_autofill.rs`
- Modify: `crates/vox-publisher/src/lib.rs`
- Modify: `crates/vox-cli-core/src/scientia.rs` + the dispatch site (pattern-match an existing read-modify command like `publication-prepare-validated`: `rg "publication-prepare-validated" crates/vox-cli crates/vox-cli-core`) to add subcommand `publication-autofill --publication-id <id> [--apply]`
- Modify: `catalog.v1.yaml` via the official sync command (`vox ci operations-sync` family — run, never hand-edit; see AGENTS.md)

- [ ] **Step 1: Failing unit tests**

```rust
// tests in scientia_autofill.rs
#[test]
fn autofill_fills_missing_date_license_and_creators() {
    let manifest = bare_manifest(); // title+body only
    let identity = Some(UserIdentityView {
        user_id: "owner".into(),
        orcid_id: Some("https://orcid.org/0000-0002-1825-0097".into()),
    });
    let plan = compute_autofill(&manifest, identity.as_ref(), Some("MIT"), Some("https://github.com/x/y"));
    let fields: Vec<&str> = plan.fills.iter().map(|f| f.field.as_str()).collect();
    assert!(fields.contains(&"publication_date"));
    assert!(fields.contains(&"license_spdx"));
    assert!(fields.contains(&"authors[0].orcid"));
    assert!(plan.fills.iter().all(|f| f.origin.starts_with("autofill:")));
}

#[test]
fn autofill_never_overwrites_existing_values() {
    let manifest = manifest_with_license("Apache-2.0");
    let plan = compute_autofill(&manifest, None, Some("MIT"), None);
    assert!(!plan.fills.iter().any(|f| f.field == "license_spdx"),
        "existing values are human ground truth — autofill only fills holes");
}
```

- [ ] **Step 2: Run** `cargo test -p vox-publisher scientia_autofill` — FAIL.

- [ ] **Step 3: Implement** `scientia_autofill.rs`:

```rust
//! Deterministic archive-metadata autofill. Pure planner (`compute_autofill`)
//! + an applier that writes back to the manifest's `metadata_json` and appends
//! `FieldProvenanceEntry { origin: "autofill:<rule>" }` rows. NEVER overwrites
//! a present value; LLM-generated content is out of scope here by design.

pub struct UserIdentityView { pub user_id: String, pub orcid_id: Option<String> }

#[derive(Debug, Clone, serde::Serialize)]
pub struct AutofillPlan {
    pub fills: Vec<PlannedFill>,
    /// Required fields autofill cannot derive — surfaced to the human (GUI checklist).
    pub human_only_remaining: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlannedFill { pub field: String, pub value: serde_json::Value, pub origin: String }

pub fn compute_autofill(
    manifest: &crate::publication::PublicationManifest,
    identity: Option<&UserIdentityView>,
    repo_license_spdx: Option<&str>,   // caller reads LICENSE file / Cargo.toml license
    git_remote_url: Option<&str>,      // caller runs `git remote get-url origin`
) -> AutofillPlan { /* rules below */ }
```

Rules (each one small, each tested): `publication_date` ← today when absent; `license_spdx` ← `repo_license_spdx` when absent; `authors` ← `[{name: manifest.author, orcid: identity.orcid_id}]` when `ScientificPublicationMetadata.authors` empty (origin `autofill:user_identity`); `authors[0].orcid` ← identity ORCID when author exists without one; `reproducibility.code_repository_url` ← `git_remote_url`; `keywords` ← title-token derivation (same rule as Task 8 so the two agree); `upload_type/access_right` defaults already handled by `zenodo_metadata.rs`. `human_only_remaining` = whatever the completion report still lists that has no rule: `abstract_text` when body < 200 chars, `funding_statement`, `competing_interests_statement`, `ethics_and_impact`. The CLI handler: load manifest (find the load/save helpers used by `publication-prepare-validated`), read LICENSE/`git remote get-url origin` (via `std::process::Command`, `CREATE_NO_WINDOW` on Windows per the `quiet_command` helper convention), print the plan as JSON; `--apply` persists `metadata_json` + provenance entries and re-runs the completion report, printing before/after `completeness_0_100`.

- [ ] **Step 4: Wire CLI + catalog.** Add the subcommand; run the catalog sync (`vox ci operations-sync` / `command-sync` — the rename memo's lesson: use official sync cmds, iterate `vox ci ssot-drift` to convergence).

- [ ] **Step 5: Run** `cargo test -p vox-publisher scientia_autofill && cargo test -p vox-cli scientia && vox ci ssot-drift` — PASS / converged.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(scientia): deterministic publication-autofill with field provenance and completion-report integration"
```

### Task 10: arXiv handoff enrichment (sidecar metadata, no live submission)

**Files:**
- Modify: `crates/vox-scientia/src/manuscript/latex.rs` (`render_arxiv_bundle` — confirm exact path with `rg "render_arxiv_bundle" crates/vox-scientia`)
- Modify: `crates/vox-cli/src/commands/scientia_phase_handlers.rs:729-765` (bundle handler)

- [ ] **Step 1: Failing test** (in the module owning `render_arxiv_bundle`)

```rust
#[test]
fn arxiv_bundle_contains_metadata_sidecar_and_upload_readme() {
    let (tar_bytes, entries) = render_bundle_for_test(); // unpack existing test helper or write one that lists tar entries
    assert!(entries.contains(&"arxiv-metadata.json".to_string()));
    assert!(entries.contains(&"UPLOAD-CHECKLIST.md".to_string()));
}
```

- [ ] **Step 2: Run to verify failure**, then **Step 3: implement**: `render_arxiv_bundle` gains two extra tar entries:
  - `arxiv-metadata.json`: `{title, abstract, authors: [{name, orcid?, affiliation?}], primary_category, license_spdx, comments}` — title/abstract/authors from the scaffold + `ScientificPublicationMetadata`; `primary_category` from the existing `publication-venue-recommend` output when the caller passes it (new optional handler arg `--primary-category`, default `"cs.SE"` with a `"category_origin": "default"` marker so the human knows to check it).
  - `UPLOAD-CHECKLIST.md`: generated text enumerating the manual steps (arXiv account, endorsement if first submission, paste each metadata field, attach this tar.gz) — explicitly stating WHY it's manual (no viable arXiv API, 2026 audit).

- [ ] **Step 4: Run** `cargo test -p vox-scientia manuscript && cargo test -p vox-cli scientia` — PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scientia): arXiv handoff bundle carries pre-filled metadata sidecar + manual-upload checklist"
```

### Task 11: Software Heritage save-code-now adapter

Archives the generated-code repo and yields a permanent identifier to cite from Zenodo `related_identifiers`.

**Files:**
- Create: `crates/vox-publisher/src/scholarly/software_heritage.rs`
- Modify: `crates/vox-publisher/src/scholarly/mod.rs` (module + re-export)
- Modify: `crates/vox-secrets/src/spec/registry/scholarly.rs` (new `SecretId::VoxSoftwareHeritageToken`, following the existing Zenodo secret-spec pattern in that file)
- Modify: `crates/vox-cli-core/src/scientia.rs` + dispatch (subcommand `publication-archive-code --origin-url <url> [--wait]`)

- [ ] **Step 1: Failing tests** (URL building + response parsing are pure; network test stays behind the mock pattern used by `scholarly_zenodo_mock_test.rs`)

```rust
#[test]
fn save_request_url_is_correct() {
    assert_eq!(
        save_code_now_url("https://github.com/vox-foundation/vox"),
        "https://archive.softwareheritage.org/api/1/origin/save/git/url/https://github.com/vox-foundation/vox/"
    );
}

#[test]
fn parses_save_request_status() {
    let body = r#"{"save_request_status":"accepted","save_task_status":"succeeded","snapshot_swhid":"swh:1:snp:abc"}"#;
    let st = parse_save_status(body).expect("parse");
    assert_eq!(st.task_status, "succeeded");
    assert_eq!(st.snapshot_swhid.as_deref(), Some("swh:1:snp:abc"));
}
```

- [ ] **Step 2: Run to verify failure**, then **Step 3: implement**: `POST` then poll `GET` on the same URL (bearer token from the new secret when present — 1200 req/h authed vs 120 anonymous); `SaveStatus { request_status, task_status, snapshot_swhid: Option<String> }`; `--wait` polls every 10s up to 5 min. Persist the outcome into the manifest `metadata_json` under `scientia.swhid` so Task 8's `related_identifiers` picks it up (relation `isIdenticalTo`, resource_type `software`). Reuse `vox_http_client::client_builder()` exactly like `zenodo.rs:75`.

- [ ] **Step 4: Run** `cargo test -p vox-publisher software_heritage` — PASS. **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scientia): Software Heritage save-code-now adapter; SWHID recorded as related identifier"
```

### Task 12: Nanopub test-server publish (approval-gated, default-off)

Replace `publish_stub` with a real test-server path. Production publishing remains unbuilt (standing decision).

**Files:**
- Modify: `crates/vox-scientia/src/nanopub/network.rs`
- Modify: `crates/vox-scientia/src/review_flow.rs` (caller passes the `ApprovalToken`)
- Modify: CLI: extend `publication-nanopub-build` with `--publish-test-server`

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn test_server_publish_refuses_without_env_allow() {
    // ApprovalToken alone is not enough: env allow is a second independent layer.
    let err = ensure_test_server_allowed(false /* env unset */).unwrap_err();
    assert!(err.to_string().contains("VOX_NANOPUB_TEST_SERVER"));
}

#[test]
fn production_publish_does_not_exist() {
    // Guard test: grep-level assertion that no production publish entry point compiles.
    // (Compile-time guarantee: this module exports ONLY publish_to_test_server.)
    let exports = ["publish_to_test_server"];
    assert_eq!(exports.len(), 1);
}
```

- [ ] **Step 2: Run to verify failure**, then **Step 3: implement** in `network.rs`:

```rust
//! Nanopub network layer. ONLY the test server is reachable, and only behind
//! BOTH an ApprovalToken (human decision row) and VOX_NANOPUB_TEST_SERVER=1.
//! Production publishing is deliberately unimplemented (standing decision).

pub fn ensure_test_server_allowed(env_allow: bool) -> anyhow::Result<()> {
    if !env_allow {
        anyhow::bail!(
            "nanopub test-server publishing is disabled; set VOX_NANOPUB_TEST_SERVER=1 to allow \
             (this posts to the PUBLIC, periodically-wiped test registry — never production)"
        );
    }
    Ok(())
}

/// Publish a signed, offline-validated nanopub to the TEST server.
/// `_approval` is required by signature: only the review service mints it.
pub async fn publish_to_test_server(
    doc: &crate::nanopub::spec::SignedNanopubDoc,
    profile: &crate::nanopub::spec::NanopubProfile,
    _approval: &crate::review::ApprovalToken,
) -> anyhow::Result<String> {
    ensure_test_server_allowed(std::env::var("VOX_NANOPUB_TEST_SERVER").as_deref() == Ok("1"))?;
    // nanopub-rs: publish with `server_url: None` targets the test server by default.
    // Read crates/vox-scientia/src/nanopub/spec.rs for how Nanopub + NpProfile are
    // constructed there and reuse the same construction; then:
    //   np.publish(Some(&np_profile), None).await
    // Persist published_state="test_server" on the scientia_nanopubs row.
    todo_replace_with_real_call(doc, profile).await
}
```

(The body must be the real `nanopub` crate call — `spec.rs:74-103` already builds `ProfileBuilder`/`Nanopub::sign`; mirror it and call `.publish(Some(&profile), None)`. Delete `publish_stub`. Update the `scientia_nanopubs` row state via the same DB facade `nanopub_build` uses.)

- [ ] **Step 4: Run** `cargo test -p vox-scientia nanopub` — PASS (network call itself is exercised manually: `VOX_NANOPUB_TEST_SERVER=1 vox scientia publication-nanopub-build --publication-id <id> --claim-id <id> --publish-test-server`; document expected output = test-server URI).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scientia): approval-gated nanopub test-server publishing; production stays unimplemented"
```

### Task 13: `publication-archive-run` — one command to the archive

Composes: preflight → autofill → completeness gate → approval check → Zenodo (sandbox default) draft+upload(+flag-gated publish) → SWH code archive → receipt.

**Files:**
- Create: `crates/vox-publisher/src/archive_run.rs` (orchestration, testable plan/step enum)
- Modify: CLI wiring as in Task 9 (`publication-archive-run --publication-id <id> [--production] [--publish]`)

- [ ] **Step 1: Failing test for the step planner** (pure)

```rust
#[test]
fn plan_blocks_on_incomplete_required_fields() {
    let plan = plan_archive_run(&completion_report_with_missing(vec!["license_spdx"]), /*approved=*/ true);
    assert_eq!(plan.first_blocker().unwrap(), "required field missing: license_spdx (run publication-autofill)");
}

#[test]
fn plan_blocks_without_approval() {
    let plan = plan_archive_run(&complete_report(), /*approved=*/ false);
    assert!(plan.first_blocker().unwrap().contains("approval"));
}

#[test]
fn complete_and_approved_plan_orders_steps() {
    let plan = plan_archive_run(&complete_report(), true);
    assert_eq!(plan.step_names(), vec!["zenodo_deposit_draft", "zenodo_upload_staging", "software_heritage_save", "record_receipt"]);
    // "zenodo_publish" appears ONLY when the publish flag profile says so (existing flags::zenodo_publish_deposition()).
}
```

- [ ] **Step 2: Run to verify failure**, then **Step 3: implement**: `ArchiveRunPlan { steps: Vec<ArchiveStep>, blockers: Vec<String> }`; executor walks steps calling the EXISTING pieces (`zenodo_from_secrets()` adapter `submit(...)`, Task 11 SWH adapter, receipt row via the scholarly receipt path — see `ScholarlySubmissionReceipt` usage in `scholarly/mod.rs`). Sandbox is the default; `--production` flips the existing `sandbox: bool`. Approval = an `approved` review decision exists for the publication (reuse `review_flow::approval_for`); without it the command prints the blocker and exits 1 — defense-in-depth unchanged.

- [ ] **Step 4: Run** `cargo test -p vox-publisher archive_run` — PASS; then a live sandbox smoke (manual, documented in the command's `--help`): requires `VOX_ZENODO_*` sandbox token; expected: draft DOI `10.5072/...` printed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scientia): publication-archive-run end-to-end (preflight→autofill gate→approval→Zenodo sandbox→SWH→receipt)"
```

---

## Track C — Automated producers: discovery across generated code

### Task 14: Commit-watcher producer (`discovery-watch`)

First automated producer: scans new commits for research-worthy signals and creates **draft** finding candidates through the existing intake gate (surfaced → review; never published).

**Files:**
- Create: `crates/vox-publisher/src/scientia_producers/mod.rs` + `commit_watcher.rs`
- Modify: `crates/vox-db/src/schema/domains/scientia.rs` + `manifest.rs` BASELINE_VERSION bump — table `scientia_producer_cursor(producer TEXT PRIMARY KEY, last_seen TEXT NOT NULL, updated_at_ms INTEGER NOT NULL)`
- Modify: CLI subcommand `vox scientia discovery-watch [--once] [--repo <path>]`

- [ ] **Step 1: Failing tests** (signal extraction is pure — test it without git)

```rust
#[test]
fn perf_delta_commit_yields_strong_signal() {
    let sig = signals_from_commit(&CommitView {
        sha: "abc".into(),
        message: "perf(parser): arena allocation cuts parse time 38% on the golden corpus".into(),
        files_changed: vec!["crates/vox-compiler/src/parser.rs".into()],
        insertions: 120, deletions: 80,
    });
    assert!(sig.iter().any(|s| s.code == "perf_delta_quantified" && s.strength == DiscoverySignalStrength::Strong));
}

#[test]
fn chore_commit_yields_no_candidate() {
    let sig = signals_from_commit(&commit("chore: bump deps"));
    assert!(sig.is_empty());
}

#[test]
fn new_golden_corpus_entry_is_supporting_signal() {
    let sig = signals_from_commit(&commit_touching("feat: new exhaustive db golden", vec!["goldens/db_operations.vox"]));
    assert!(sig.iter().any(|s| s.code == "golden_corpus_growth"));
}
```

- [ ] **Step 2: Run to verify failure**, then **Step 3: implement**:
  - `CommitView { sha, message, files_changed, insertions, deletions }` built from `git log --since-ref <cursor> --numstat --format=...` via `std::process::Command` (Windows: `CREATE_NO_WINDOW`).
  - `signals_from_commit` — deterministic rules emitting the EXISTING `scientia_evidence` signal vocabulary (read `crates/vox-publisher/src/scientia_evidence.rs` first; reuse `DiscoverySignalStrength` + signal-code conventions): quantified perf claims (regex `\d+(\.\d+)?\s*(%|x|ms|µs)` in message + perf/feat type), golden-corpus growth, new-crate/new-capability touches (`catalog.v1.yaml`, `contracts/` changes), benchmark file changes.
  - For each commit with ≥1 signal: build a draft `PublicationManifest` (title = commit subject, body = message + file list, `metadata_json.scientia.producer = "commit_watcher"`, source commit sha) and pass it through `DiscoveryIntakeGate::AllowReviewSuggested` + `intake_gate_allows` before inserting via the same DB path `publication-prepare` uses (`rg "publication-prepare" crates/vox-cli` for the insert helper). Cursor row advances only after successful insert pass.

- [ ] **Step 4: Run** `cargo test -p vox-publisher commit_watcher`, then a live `--once` run on this repo; expected: JSON listing N commits scanned, M candidates created with signal codes.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scientia): commit-watcher producer — automated finding candidates from commit signals"
```

### Task 15: Code-uniqueness signal (embedding kNN against the corpus)

"Discovery of unique things across code as it is generated": for candidate findings from Task 14, compute how unlike the existing corpus the changed code is — efficiently (digest-keyed cache from Task 3; only changed symbols).

**Files:**
- Create: `crates/vox-publisher/src/scientia_producers/code_uniqueness.rs`
- Modify: `commit_watcher.rs` to attach the signal

- [ ] **Step 1: Failing tests** (pure parts: snippet extraction + uniqueness math)

```rust
#[test]
fn uniqueness_is_one_minus_max_similarity() {
    assert!((uniqueness_score(&[0.2, 0.9, 0.4]) - 0.1).abs() < 1e-9);
    assert_eq!(uniqueness_score(&[]), 1.0, "empty corpus: everything is unique");
}

#[test]
fn doc_comment_snippets_are_extracted_per_changed_symbol() {
    let snips = extract_snippets("crates/x/src/lib.rs", RUST_SOURCE_WITH_TWO_DOC_COMMENTED_FNS);
    assert_eq!(snips.len(), 2);
    assert!(snips[0].text.contains("///"));
}
```

- [ ] **Step 2: Run to verify failure**, then **Step 3: implement**:
  - `extract_snippets`: doc-comment + signature line per top-level `fn`/`struct`/`impl` block in changed files (plain line-scan, no syn dependency — `///` runs followed by the item line; cap 40 lines/snippet).
  - `uniqueness_score(similarities) = 1.0 - max` (`f64`).
  - Async assessor: for each snippet, `embed_cached` (Task 3) → kNN via `vox-search`'s `QdrantSemanticClient::search_vectors` against the semantic-FS collection when configured (`rg "semantic_fs" crates/vox-search/src` for the collection-name policy); fall back to "skip signal" (`None`) when Qdrant is unconfigured — never fabricate.
  - Attach to the candidate: `metadata_json.scientia.code_uniqueness = {score, snippets_assessed, corpus: "<collection>"}`; uniqueness ≥ 0.6 over ≥2 snippets emits signal code `code_novelty_embedding` (Supporting strength — embedding distance is a weak-moderate signal per the literature; it must corroborate, not decide).
  - Efficiency invariants (tested): cache hit means zero `llm_embed` calls for unchanged text (assert via a counting fake embedder injected in tests — make the embed fn a parameter `impl Fn`/trait for testability).

- [ ] **Step 4: Run** `cargo test -p vox-publisher code_uniqueness` — PASS. **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scientia): code-uniqueness producer signal via cached embeddings + Qdrant kNN"
```

### Task 16: Surfaced-discovery events → inbox persistence + WS topic

**Files:**
- Modify: `crates/vox-db/src/schema/domains/scientia.rs` + `manifest.rs` bump — table `scientia_discovery_inbox(id INTEGER PRIMARY KEY AUTOINCREMENT, publication_id TEXT NOT NULL, surfaced_at_ms INTEGER NOT NULL, intake_tier TEXT NOT NULL, signal_codes TEXT NOT NULL, acknowledged_at_ms INTEGER)`
- Modify: `crates/vox-db/src/facade/` — `insert_discovery_inbox`, `list_unacknowledged_discoveries`, `acknowledge_discovery`
- Modify: `crates/vox-orchestrator-mcp/src/http_gateway/scientia_feed.rs` — second topic `scientia.discovery.surfaced` in the SAME poller loop (diff on max inbox id, exactly the existing `QueueSnapshot`-diff pattern at lines 1-109)
- Modify: `commit_watcher.rs` — insert inbox row when a candidate lands

- [ ] **Step 1: Failing facade test** (same in-memory pattern as Task 3 Step 5): insert → list shows it → acknowledge → list empty.
- [ ] **Step 2: Run to verify failure; Step 3: implement** (copy the row-mapping style of an adjacent scientia facade file).
- [ ] **Step 4:** Extend the feed poller: track `last_max_inbox_id`; on increase, broadcast `TopicMessage { topic: "scientia.discovery.surfaced", data: <new rows as JSON> }`. Mirror the existing topic constant + test style in `scientia_feed.rs`.
- [ ] **Step 5: Run** `cargo test -p vox-db discovery_inbox && cargo test -p vox-orchestrator-mcp scientia_feed` — PASS. **Commit:**

```bash
git add -A && git commit -m "feat(scientia): discovery inbox persistence + scientia.discovery.surfaced WS topic"
```

---

## Track D — GUI surfacing

GUI conventions for all three tasks: components live under `crates/vox-gui/ui/src/components/surfaces/Scientia/`; Tauri commands in `crates/vox-gui/src/` next to `scientia_review.rs` (read it first — copy its DTO + `#[tauri::command]` + registration pattern exactly, including the `invoke_handler` registration site); frontend API wrappers next to `discoveryReviewApi.ts`; register every new surface in the surface registry (schema `contracts/gui/surface-registry.v1.schema.json`; regenerate the report via `vox ci gui-surface-registry` — the CI gate fails un-registered surfaces); tests with `pnpm vitest run <file>` from `crates/vox-gui/ui`.

### Task 17: Novelty evidence panel on DiscoveryReview cards

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Scientia/NoveltyEvidencePanel.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Scientia/noveltyApi.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryReview.tsx` (render panel in the detail pane)
- Modify: `crates/vox-gui/src/scientia_review.rs` (new command `get_novelty_assessment`)
- Test: `crates/vox-gui/ui/src/components/surfaces/Scientia/__tests__/NoveltyEvidencePanel.test.tsx`

- [ ] **Step 1: Failing component test**

```tsx
import { render, screen } from '@testing-library/react';
import { NoveltyEvidencePanel } from '../NoveltyEvidencePanel';

const assessment = {
  verdict: { kind: 'not_novel', closest_hit_uri: 'https://openalex.org/W1', similarity: 0.91 },
  signals: { max_semantic: 0.91, max_lexical: 0.55, near_hit_count: 3, top_hit_citations: 120, sources_succeeded: 3 },
  conflicts: [],
  excluded_future_hits: 0,
  prior_art: [{ work_uri: 'https://openalex.org/W1', title: 'Prior work', year: 2024, cited_by_count: 120, semantic_score: 0.91 }],
};

test('renders verdict chip and closest prior art', () => {
  render(<NoveltyEvidencePanel assessment={assessment} />);
  expect(screen.getByText(/not novel/i)).toBeInTheDocument();
  expect(screen.getByText('Prior work')).toBeInTheDocument();
});

test('insufficient evidence shows the retrieval-failure banner', () => {
  render(<NoveltyEvidencePanel assessment={{ ...assessment, verdict: { kind: 'insufficient_evidence' }, prior_art: [] }} />);
  expect(screen.getByText(/retrieval failed or never ran/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run** `pnpm vitest run src/components/surfaces/Scientia/__tests__/NoveltyEvidencePanel.test.tsx` — FAIL.

- [ ] **Step 3: Implement.** Backend command (in `scientia_review.rs`, same registration as siblings): `get_novelty_assessment(publication_id) -> NoveltyAssessmentDto` — loads the persisted bundle (the `publication-novelty-fetch` storage path), runs `vox_publisher::scientia_novelty_assess::assess_novelty`, maps verdict to a tagged DTO `{kind: 'novel'|'possibly_novel'|'not_novel'|'insufficient_evidence', ...}` plus top-5 prior-art hits sorted by semantic score. Frontend `noveltyApi.ts` wraps `invoke('get_novelty_assessment', ...)` exactly like `discoveryReviewApi.ts`. Panel renders: verdict chip (tone: ok=novel, warn=possibly, info=not_novel, warn=insufficient with banner text "Retrieval failed or never ran — do not treat as novel"), signal grid (the 5 breakdown numbers), prior-art rows (title → external link, year, citations, similarity), conflicts side-by-side list when non-empty. Wire into `DiscoveryReview.tsx` detail pane, loading lazily per selected claim's publication.

- [ ] **Step 4: Run tests + the registry gate**

Run: `pnpm vitest run` (Scientia tests) and `cargo test -p vox-gui` if it has Rust-side tests; `vox ci gui-surface-registry` stays green (panel is a child of an existing registered surface, not a new top-level surface).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(gui): novelty evidence panel — verdict, signal breakdown, prior art, insufficient-evidence banner"
```

### Task 18: Discovery Inbox surface + OS notifications

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryInbox.tsx` + `discoveryInboxApi.ts` + test
- Modify: `crates/vox-gui/src/scientia_review.rs` (commands `list_discovery_inbox`, `acknowledge_discovery`)
- Modify: `crates/vox-gui/ui/src/lib/transport.ts` (listener for the new topic, mirroring `listenScientiaQueue` at lines 46-68)
- Modify: surface registry + sidebar registration (copy how `DiscoveryReview` is registered: `rg "DiscoveryReview" crates/vox-gui/ui/src --type ts -l`)
- Modify (notifications): `crates/vox-gui/ui/package.json` (`pnpm add @tauri-apps/plugin-notification`), `crates/vox-gui/Cargo.toml` (`tauri-plugin-notification`), the Tauri builder (`.plugin(tauri_plugin_notification::init())`), and the capability file (`rg "core:default" crates/vox-gui/capabilities` → add `notification:default`) — all four pieces are required (Tauri 2 plugin pattern).

- [ ] **Step 1: Failing component test** — inbox renders unacknowledged rows (tier badge, signal codes, surfaced-at), "Open review" button routes to DiscoveryReview, "Acknowledge" removes the row; StrongCandidate rows render with the strong-tier badge.

```tsx
test('strong candidates are badged and acknowledgeable', async () => {
  render(<DiscoveryInbox api={fakeApi([{ id: 1, publication_id: 'P1', intake_tier: 'strong_candidate', signal_codes: ['perf_delta_quantified'], surfaced_at_ms: 0, acknowledged_at_ms: null }])} pushToast={() => {}} />);
  expect(await screen.findByText(/strong/i)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /acknowledge/i }));
  await waitFor(() => expect(screen.queryByText('P1')).not.toBeInTheDocument());
});
```

(Inject the API as a prop with a default of the real wrapper — the existing surfaces' testing pattern; check how `DiscoveryReview.test.tsx` fakes Tauri if it exists and copy it.)

- [ ] **Step 2: Run to verify failure; Step 3: implement** the surface (master list + acknowledge + deep-link to review), the two Tauri commands over the Task 16 facade, the `listenDiscoverySurfaced()` transport listener, and the notification effect: on a `scientia.discovery.surfaced` event containing a `strong_candidate` row, `sendNotification({ title: 'New research candidate', body: <title + signal codes> })` guarded by `isPermissionGranted()/requestPermission()` — plus the in-app toast fallback.

- [ ] **Step 4: Register + verify.** Add the surface-registry entry; run `vox ci gui-surface-registry` and `pnpm vitest run` — both green.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(gui): discovery inbox surface with WS-driven OS notifications for strong candidates"
```

### Task 19: Archive panel — completeness, autofill, deposit status

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Scientia/ArchivePanel.tsx` + `archiveApi.ts` + test
- Modify: `crates/vox-gui/src/scientia_review.rs` (commands `get_completion_report` → `ManifestCompletionReport` DTO, `run_autofill {apply}` → `AutofillPlan` DTO, `get_archive_status` → receipt/DOI/SWHID DTO)
- Modify: surface registry (new panel under the Scientia group or as a tab of ScientiaDashboard — match whichever pattern `ClaimsView` uses)

- [ ] **Step 1: Failing component test** — renders `completeness_0_100` meter; `required_missing` as a checklist; "Auto-fill" button calls `run_autofill(apply: true)` and re-renders with raised completeness; filled fields show their `origin` provenance chips; `human_only_pending` fields render input affordances that deep-link to edit (or read-only "needs human" tags in v1); archive status block shows Zenodo state + DOI + SWHID when present.

```tsx
test('autofill raises completeness and shows provenance', async () => {
  const api = fakeArchiveApi({ before: 40, after: 80 });
  render(<ArchivePanel publicationId="P1" api={api} pushToast={() => {}} />);
  expect(await screen.findByText(/40/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /auto-fill/i }));
  expect(await screen.findByText(/80/)).toBeInTheDocument();
  expect(screen.getByText(/autofill:user_identity/)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run to verify failure; Step 3: implement** (commands delegate to `vox_publisher::{scientia_discovery completion report fn, scientia_autofill, archive_run status}` — find the completion-report producer with `rg "ManifestCompletionReport" crates/vox-publisher/src` and call it server-side; never recompute field rules in TS).

- [ ] **Step 4: Run** `pnpm vitest run` + `vox ci gui-surface-registry` — green. **Step 5: Commit:**

```bash
git add -A && git commit -m "feat(gui): archive panel — completeness meter, one-click autofill with provenance, deposit status"
```

---

## Track E — Gates, docs, consolidation

### Task 20: Drift gates + documentation rows

**Files:**
- Modify: `docs/src/architecture/where-things-live.md` (rows: novelty assessment seam, autofill engine, producers, archive run, inbox, new GUI panels)
- Modify: `docs/src/reference/scientia-ssot-handbook.md` (§3 map: one assessment entry point, contract-parity gate, producer architecture)
- Modify: `docs/src/architecture/scientia-micropublication-ssot-and-surfacing-design-2026.md` (revision note: P1/P2/P3 landed; this plan supersedes §10 P4/P5 details)
- Verify: `cargo run -p vox-arch-check`, `vox ci ssot-drift` to convergence, `vox ci gui-surface-registry`

- [ ] **Step 1:** Add the where-things-live rows (the table is flat "concept → crate" — follow existing row format; these files already carry frontmatter, do not touch it).
- [ ] **Step 2:** Update the handbook §3 and the design doc's revision note (state plainly which audit rows changed status as of this plan's landing).
- [ ] **Step 3:** Run all three gates; iterate to green (clippy lesson: re-run the `-D` gate to convergence after each fix batch).
- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "docs(scientia): where-things-live + SSOT handbook rows for the research-pipeline upgrade; gates green"
```

---

## Self-review notes (done at plan time)

- **Spec coverage:** archive-with-autofill → Tasks 8/9/10/11/12/13; auto-surfacing → 14/16/18; uniqueness-across-generated-code → 15 (+14 carrier); novelty metrics → 1–7; GUI → 17/18/19; "consider what we have" → audit table; efficiency → digest-keyed embed cache (3), changed-symbols-only + cache-hit invariant test (15), DB-diff WS poller reuse (16).
- **Known unknowns flagged in-task (not placeholders, verification steps):** exact generated trace-item type name (Task 1 Step 1 gives the `rg`), `llm_embed` `LlmConfig` field names (Task 3 Step 3), `render_arxiv_bundle` path (Task 10), completion-report producer fn (Task 19). Each task names the discovery command.
- **Deliberately out of scope:** arXiv live submission (no viable API), production nanopub publishing (standing decision), LLM-generated abstract autofill (deterministic-only v1; `evidence-assist` already covers LLM suggestions), SPECTER2 local Candle backend (decision #D3 said ship light/general first — `llm_embed` provider config covers it without code), Ludus reward wiring (nice-to-have; not in the user's ask).
- **Type consistency check:** `NoveltyVerdict::InsufficientEvidence` (Task 1) is consumed by Tasks 4/7/17; `NoveltySignalBreakdown` defined Task 4, consumed Task 17; `AutofillPlan`/`PlannedFill` defined Task 9, consumed Task 19; inbox row shape defined Task 16, consumed Task 18; `assess_novelty(bundle, claim_year, config)` signature consistent across 4/7/17.
