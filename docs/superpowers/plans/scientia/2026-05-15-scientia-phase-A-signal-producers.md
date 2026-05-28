# SCIENTIA Phase A — Self-Observation Signal Producers

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Status:** detailed (promoted from outline 2026-05-15 after code-surface exploration).

**Goal:** Produce `FindingCandidateProposed` research-events automatically from the user's own development activity — commit graph, benchmark CI history, and Socrates telemetry — and persist them as `scientia_finding_candidates` rows so downstream discovery surfaces have signal sources beyond the Provider Atlas flow.

**Architecture:** New L2 crate `vox-scientia-producers`. Each producer is a small *deterministic* detector (no LLM in the producer path) consuming existing observation surfaces and emitting `ResearchEvent::FindingCandidateProposed` via the existing `ResearchEventEmitter` trait. Persistence: a new `scientia_finding_candidates` table mirroring `scientia_discoveries`. Producers compose via `ProducerRegistry::run_all()` with dedup on a (`candidate_class`, signal-fingerprint) key. LLM-grounded T2/T3 promotion happens downstream in the existing claim extractor and worthiness gates — out of scope here.

**Tech Stack:** Rust 2024; existing `vox-research-events` (`ResearchEventEmitter`, `ResearchEvent::FindingCandidateProposed`); existing `vox-db` schema and store-ops conventions (mirroring `scientia_discoveries`); existing `vox-git` (gix-backed; **do NOT use `git2` or shell-out** per workspace convention); existing `ExecTimeRecord` from `vox-db-types`. No new external deps.

**Strategic context:** [Gap-map §2 Gap A](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-a--self-observation-candidate-producers); [Finalization Plan §6.1](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#61-new-crates-only-what-cannot-live-elsewhere).

**Out of scope (deferred):**
- LLM-based candidate filtering (lives downstream in `vox-claim-extractor`).
- Provider Atlas observation flow (already complete — Finalization Phase 6).
- Publication-time decisions (worthiness rubric already gates this).
- Scout CLI surface (Phase F).
- MENS training-run producer (deferred to Phase A.2 — the MENS event surface in `vox-actor-runtime/mens.rs` does not yet publish typed completion events on `vox-research-events`; resolve open question OQ-A1 first).

---

## File inventory

| Action | Path | Responsibility |
|---|---|---|
| Create | `crates/vox-scientia-producers/Cargo.toml` | L2 crate manifest |
| Create | `crates/vox-scientia-producers/src/lib.rs` | Public API: `Producer` trait, `ProducerRegistry`, `ProducerContext` |
| Create | `crates/vox-scientia-producers/src/dedup.rs` | Signal-fingerprint dedup |
| Create | `crates/vox-scientia-producers/src/heuristics.rs` | MDL-proxy LoC delta, p95 delta z-score |
| Create | `crates/vox-scientia-producers/src/commit_graph.rs` | `algorithmic_improvement` + `reproducibility_infra` from commit graph |
| Create | `crates/vox-scientia-producers/src/bench_history.rs` | `algorithmic_improvement` from `ExecTimeRecord` rows |
| Create | `crates/vox-scientia-producers/src/socrates_telemetry.rs` | `telemetry_trust` from Socrates surface |
| Create | `crates/vox-scientia-producers/tests/registry_smoke.rs` | Composition smoke test |
| Create | `crates/vox-scientia-producers/tests/commit_graph_synthetic.rs` | Synthetic git repo round-trip |
| Modify | `crates/vox-db/src/schema/domains/scientia.rs` | Add `scientia_finding_candidates` table |
| Modify | `crates/vox-db/src/store/mod.rs` | Wire `ops_finding_candidates` module |
| Create | `crates/vox-db/src/store/ops_finding_candidates.rs` | Insert/list/get row operations |
| Modify | `Cargo.toml` (workspace root) | Add `vox-scientia-producers` to `[workspace.dependencies]` |
| Modify | `docs/src/architecture/layers.toml` | Register `vox-scientia-producers = { layer = 2 }` |
| Modify | `docs/src/architecture/where-things-live.md` | Add row: "Scientia signal producers" → crate path |

LoC budget: ≤1500 LoC implementation + ≤500 LoC tests. Per-module ≤400 LoC.

---

## Pre-flight verification

- [ ] **Step P1: Verify exploration findings still hold**

```bash
grep -n "FindingCandidateProposed" crates/vox-research-events/src/events.rs
```
Expected: a line around 92 showing the variant. If absent, **stop** and re-survey — the plan assumes this exists.

```bash
grep -n "scientia_finding_candidates" crates/vox-db/src/schema/domains/scientia.rs
```
Expected: empty (table doesn't yet exist).

```bash
grep -n "scientia_discoveries" crates/vox-db/src/schema/domains/scientia.rs | head -3
```
Expected: at least one hit around line 4 — this is our mirror pattern.

```bash
ls crates/vox-git/Cargo.toml
```
Expected: file exists — we use this, not `git2`.

- [ ] **Step P2: Read the mirror pattern**

Open `crates/vox-db/src/schema/domains/scientia.rs` lines 4–25 (the `scientia_discoveries` table DDL). The new `scientia_finding_candidates` table follows the same shape.

Open `crates/vox-research-events/src/events.rs` lines 88–100. Note the exact field names of `FindingCandidateProposed`: `finding_id`, `claim_ids`, `worthiness_score`, `session_id`. Producers will emit with these names.

Open `crates/vox-git/src/lib.rs` to find the public commit-walk API. Record the function signature you'll use in Task 4.

---

## Task 1: DB migration — `scientia_finding_candidates` table

**Files:**
- Modify: `crates/vox-db/src/schema/domains/scientia.rs` (append after the `scientia_discoveries` block, around line 25)

Per [SSOT §5.5](../../../src/architecture/mesh-and-language-distribution-ssot-2026.md): bump `BASELINE_VERSION` in `manifest.rs` only. **No date-stamped or numeric SQL files.**

- [ ] **Step 1.1: Write the failing test (table doesn't exist)**

Add to `crates/vox-db/tests/scientia_finding_candidates_schema.rs`:

```rust
use vox_db::Codex;

#[tokio::test]
async fn finding_candidates_table_present_after_init() {
    let codex = Codex::in_memory().await.unwrap();
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='scientia_finding_candidates'"
    )
    .fetch_one(codex.pool())
    .await
    .unwrap();
    assert_eq!(row, 1, "scientia_finding_candidates table must exist");
}
```

- [ ] **Step 1.2: Run test to verify it fails**

```bash
cargo test -p vox-db scientia_finding_candidates_schema -- --nocapture
```
Expected: FAIL — `assert_eq!(row, 1)` left = 0.

- [ ] **Step 1.3: Add DDL**

Append to `crates/vox-db/src/schema/domains/scientia.rs` after the `scientia_discoveries` table block (around line 25):

```rust
pub const SCIENTIA_FINDING_CANDIDATES_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS scientia_finding_candidates (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    candidate_id                TEXT NOT NULL UNIQUE,
    candidate_class             TEXT NOT NULL CHECK (candidate_class IN (
        'algorithmic_improvement',
        'reproducibility_infra',
        'policy_governance',
        'telemetry_trust',
        'other'
    )),
    publication_id              TEXT,
    title_hint                  TEXT,
    internal_signals_json       TEXT NOT NULL,
    novelty_evidence_bundle_id  TEXT,
    worthiness_decision_ref     TEXT,
    confidence_json             TEXT,
    repository_id               TEXT,
    producer_name               TEXT NOT NULL,
    signal_fingerprint          TEXT NOT NULL,
    created_at_ms               INTEGER NOT NULL,
    updated_at_ms               INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scientia_finding_candidates_class
    ON scientia_finding_candidates(candidate_class);
CREATE INDEX IF NOT EXISTS idx_scientia_finding_candidates_repo
    ON scientia_finding_candidates(repository_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_scientia_finding_candidates_fingerprint
    ON scientia_finding_candidates(producer_name, signal_fingerprint);
"#;
```

Then register this DDL in whatever the existing pattern is for the file — match how `SCIENTIA_DISCOVERIES_DDL` is registered. Read those few lines first; the registration is typically in a `domain_ddl()` function or similar at the bottom of the module.

- [ ] **Step 1.4: Bump `BASELINE_VERSION`**

In `crates/vox-db/src/manifest.rs`, increment the `BASELINE_VERSION` constant by 1. Do NOT add a migration SQL file.

- [ ] **Step 1.5: Run test to verify pass**

```bash
cargo test -p vox-db scientia_finding_candidates_schema -- --nocapture
```
Expected: PASS.

- [ ] **Step 1.6: Verify no regression**

```bash
cargo test -p vox-db
```
Expected: full vox-db suite green.

- [ ] **Step 1.7: Commit**

```bash
git add crates/vox-db/src/schema/domains/scientia.rs crates/vox-db/src/manifest.rs crates/vox-db/tests/scientia_finding_candidates_schema.rs
git commit -m "feat(vox-db): add scientia_finding_candidates table for Phase A producers"
```

---

## Task 2: Store ops — insert / list / get

**Files:**
- Create: `crates/vox-db/src/store/ops_finding_candidates.rs`
- Modify: `crates/vox-db/src/store/mod.rs` (add `pub mod ops_finding_candidates;`)

- [ ] **Step 2.1: Write the failing test**

Add to `crates/vox-db/tests/finding_candidate_ops.rs`:

```rust
use vox_db::store::ops_finding_candidates::{insert_candidate, list_candidates, FindingCandidateRow};
use vox_db::Codex;

#[tokio::test]
async fn insert_then_list_round_trip() {
    let codex = Codex::in_memory().await.unwrap();
    let row = FindingCandidateRow {
        candidate_id: "test-001".into(),
        candidate_class: "algorithmic_improvement".into(),
        publication_id: None,
        title_hint: Some("perf improvement".into()),
        internal_signals_json: r#"[{"code":"p95_delta","summary":"latency drop","strength":"strong","family":"benchmark_pair","provenance":{"origin":"phase_a_test"}}]"#.into(),
        novelty_evidence_bundle_id: None,
        worthiness_decision_ref: None,
        confidence_json: Some(r#"{"signal_strength":0.81}"#.into()),
        repository_id: Some("vox".into()),
        producer_name: "test_producer".into(),
        signal_fingerprint: "test-fp-001".into(),
        created_at_ms: 1747000000000,
        updated_at_ms: 1747000000000,
    };
    insert_candidate(codex.pool(), &row).await.unwrap();
    let listed = list_candidates(codex.pool(), None).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].candidate_id, "test-001");
}

#[tokio::test]
async fn duplicate_fingerprint_rejected() {
    let codex = Codex::in_memory().await.unwrap();
    let row = FindingCandidateRow { /* same as above, candidate_id "a" */ };
    insert_candidate(codex.pool(), &row).await.unwrap();
    let row2 = FindingCandidateRow { /* same fingerprint, candidate_id "b" */ };
    let res = insert_candidate(codex.pool(), &row2).await;
    assert!(res.is_err(), "duplicate (producer, fingerprint) must be rejected");
}
```

- [ ] **Step 2.2: Run test to verify it fails (module missing)**

```bash
cargo test -p vox-db finding_candidate_ops 2>&1 | head -20
```
Expected: compile error — `ops_finding_candidates` module not found.

- [ ] **Step 2.3: Implement module**

Create `crates/vox-db/src/store/ops_finding_candidates.rs`:

```rust
//! CRUD over the scientia_finding_candidates table.

use sqlx::{Pool, Sqlite};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FindingCandidateRow {
    pub candidate_id: String,
    pub candidate_class: String,
    pub publication_id: Option<String>,
    pub title_hint: Option<String>,
    pub internal_signals_json: String,
    pub novelty_evidence_bundle_id: Option<String>,
    pub worthiness_decision_ref: Option<String>,
    pub confidence_json: Option<String>,
    pub repository_id: Option<String>,
    pub producer_name: String,
    pub signal_fingerprint: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub async fn insert_candidate(
    pool: &Pool<Sqlite>,
    row: &FindingCandidateRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO scientia_finding_candidates (
            candidate_id, candidate_class, publication_id, title_hint,
            internal_signals_json, novelty_evidence_bundle_id,
            worthiness_decision_ref, confidence_json, repository_id,
            producer_name, signal_fingerprint, created_at_ms, updated_at_ms
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&row.candidate_id)
    .bind(&row.candidate_class)
    .bind(&row.publication_id)
    .bind(&row.title_hint)
    .bind(&row.internal_signals_json)
    .bind(&row.novelty_evidence_bundle_id)
    .bind(&row.worthiness_decision_ref)
    .bind(&row.confidence_json)
    .bind(&row.repository_id)
    .bind(&row.producer_name)
    .bind(&row.signal_fingerprint)
    .bind(row.created_at_ms)
    .bind(row.updated_at_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_candidates(
    pool: &Pool<Sqlite>,
    candidate_class: Option<&str>,
) -> Result<Vec<FindingCandidateRow>, sqlx::Error> {
    let rows = if let Some(class) = candidate_class {
        sqlx::query_as::<_, FindingCandidateRow>(
            "SELECT candidate_id, candidate_class, publication_id, title_hint,
                    internal_signals_json, novelty_evidence_bundle_id,
                    worthiness_decision_ref, confidence_json, repository_id,
                    producer_name, signal_fingerprint, created_at_ms, updated_at_ms
             FROM scientia_finding_candidates WHERE candidate_class = ?
             ORDER BY created_at_ms DESC"
        )
        .bind(class)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, FindingCandidateRow>(
            "SELECT candidate_id, candidate_class, publication_id, title_hint,
                    internal_signals_json, novelty_evidence_bundle_id,
                    worthiness_decision_ref, confidence_json, repository_id,
                    producer_name, signal_fingerprint, created_at_ms, updated_at_ms
             FROM scientia_finding_candidates
             ORDER BY created_at_ms DESC"
        )
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

pub async fn get_candidate(
    pool: &Pool<Sqlite>,
    candidate_id: &str,
) -> Result<Option<FindingCandidateRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, FindingCandidateRow>(
        "SELECT candidate_id, candidate_class, publication_id, title_hint,
                internal_signals_json, novelty_evidence_bundle_id,
                worthiness_decision_ref, confidence_json, repository_id,
                producer_name, signal_fingerprint, created_at_ms, updated_at_ms
         FROM scientia_finding_candidates WHERE candidate_id = ?"
    )
    .bind(candidate_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
```

In `crates/vox-db/src/store/mod.rs` add:
```rust
pub mod ops_finding_candidates;
```

- [ ] **Step 2.4: Run tests to verify pass**

```bash
cargo test -p vox-db finding_candidate_ops
```
Expected: PASS (both round_trip and duplicate_fingerprint_rejected).

- [ ] **Step 2.5: Commit**

```bash
git add crates/vox-db/src/store/ops_finding_candidates.rs crates/vox-db/src/store/mod.rs crates/vox-db/tests/finding_candidate_ops.rs
git commit -m "feat(vox-db): finding-candidate store ops (insert/list/get) with fingerprint uniqueness"
```

---

## Task 3: Crate scaffold — `vox-scientia-producers`

**Files:**
- Create: `crates/vox-scientia-producers/Cargo.toml`
- Create: `crates/vox-scientia-producers/src/lib.rs`
- Modify: `Cargo.toml` (root) `[workspace.members]` AND `[workspace.dependencies]`
- Modify: `docs/src/architecture/layers.toml`

- [ ] **Step 3.1: Scaffold the crate**

Create `crates/vox-scientia-producers/Cargo.toml`:

```toml
[package]
name = "vox-scientia-producers"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false

[dependencies]
vox-research-events = { workspace = true }
vox-db = { workspace = true }
vox-db-types = { workspace = true }
vox-git = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sha3 = { workspace = true }
tokio = { workspace = true, features = ["sync"] }
async-trait = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

(Adjust to match the exact dep style used elsewhere — review one neighbor `Cargo.toml` like `crates/vox-prereg/Cargo.toml` for the workspace-dep pattern.)

- [ ] **Step 3.2: Stub lib.rs**

Create `crates/vox-scientia-producers/src/lib.rs`:

```rust
//! SCIENTIA Phase A — self-observation signal producers.
//!
//! Producers turn the user's own development activity into
//! `FindingCandidate` rows via deterministic detectors (no LLM in the
//! producer path). LLM-grounded T2/T3 promotion happens downstream.

pub mod dedup;
pub mod heuristics;
pub mod commit_graph;
pub mod bench_history;
pub mod socrates_telemetry;

mod producer;
mod registry;

pub use producer::{Producer, ProducerContext};
pub use registry::ProducerRegistry;
```

- [ ] **Step 3.3: Register in workspace**

In root `Cargo.toml`:
- Add `"crates/vox-scientia-producers"` to `[workspace.members]` (match alphabetical insertion).
- In `[workspace.dependencies]` add:
```toml
vox-scientia-producers = { path = "crates/vox-scientia-producers" }
```

- [ ] **Step 3.4: Register in layers.toml**

In `docs/src/architecture/layers.toml`, add in the L2 block (match style of `vox-eval = { layer = 2 }`):
```toml
vox-scientia-producers  = { layer = 2 }
```

- [ ] **Step 3.5: Verify it compiles**

```bash
cargo check -p vox-scientia-producers
```
Expected: errors about missing `producer` and `registry` modules (we declared them in lib.rs but didn't create the files). That's intentional — Task 4 creates them.

- [ ] **Step 3.6: Commit (red baseline, scaffold only)**

```bash
git add crates/vox-scientia-producers Cargo.toml docs/src/architecture/layers.toml
git commit -m "scaffold(vox-scientia-producers): empty L2 crate registered for Phase A"
```

---

## Task 4: `Producer` trait + `ProducerRegistry`

**Files:**
- Create: `crates/vox-scientia-producers/src/producer.rs`
- Create: `crates/vox-scientia-producers/src/registry.rs`

- [ ] **Step 4.1: Write the failing test**

Create `crates/vox-scientia-producers/tests/registry_smoke.rs`:

```rust
use async_trait::async_trait;
use vox_research_events::ResearchEvent;
use vox_scientia_producers::{Producer, ProducerContext, ProducerRegistry};

struct OneShotProducer;

#[async_trait]
impl Producer for OneShotProducer {
    fn name(&self) -> &'static str { "one_shot" }
    async fn observe(&self, _ctx: &ProducerContext) -> Vec<ResearchEvent> {
        vec![ResearchEvent::FindingCandidateProposed {
            finding_id: "one-shot-001".into(),
            claim_ids: vec![],
            worthiness_score: 0.5,
            session_id: "phase-a-smoke".into(),
        }]
    }
}

#[tokio::test]
async fn registry_runs_all_producers_and_collects_events() {
    let mut reg = ProducerRegistry::new();
    reg.register(Box::new(OneShotProducer));
    let ctx = ProducerContext::for_test();
    let events = reg.run_all(&ctx).await;
    assert_eq!(events.len(), 1);
    match &events[0] {
        ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
            assert_eq!(finding_id, "one-shot-001");
        }
        _ => panic!("expected FindingCandidateProposed"),
    }
}
```

- [ ] **Step 4.2: Run to verify it fails**

```bash
cargo test -p vox-scientia-producers registry_smoke 2>&1 | head -20
```
Expected: compile errors (modules missing).

- [ ] **Step 4.3: Implement producer.rs**

Create `crates/vox-scientia-producers/src/producer.rs`:

```rust
use async_trait::async_trait;
use vox_research_events::ResearchEvent;

/// Context passed to every producer on each invocation. Carries the
/// repository root and tunables (commit window, days window) plus a
/// monotonic clock for deterministic tests.
pub struct ProducerContext {
    pub repo_root: std::path::PathBuf,
    pub commit_window: usize,
    pub days_window: u32,
    pub now_ms: i64,
    pub session_id: String,
}

impl ProducerContext {
    pub fn for_test() -> Self {
        Self {
            repo_root: std::env::temp_dir(),
            commit_window: 10,
            days_window: 30,
            now_ms: 1_747_000_000_000,
            session_id: "test-session".into(),
        }
    }
}

/// A deterministic detector that turns observed activity into research events.
#[async_trait]
pub trait Producer: Send + Sync {
    fn name(&self) -> &'static str;
    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent>;
}
```

- [ ] **Step 4.4: Implement registry.rs**

Create `crates/vox-scientia-producers/src/registry.rs`:

```rust
use vox_research_events::ResearchEvent;

use crate::producer::{Producer, ProducerContext};

pub struct ProducerRegistry {
    producers: Vec<Box<dyn Producer>>,
}

impl ProducerRegistry {
    pub fn new() -> Self {
        Self { producers: Vec::new() }
    }

    pub fn register(&mut self, p: Box<dyn Producer>) {
        self.producers.push(p);
    }

    pub async fn run_all(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        let mut out = Vec::new();
        for p in &self.producers {
            let mut events = p.observe(ctx).await;
            out.append(&mut events);
        }
        crate::dedup::dedup_finding_candidates(out)
    }
}

impl Default for ProducerRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        // Registration of the four standard producers added in Task 9.
        reg
    }
}
```

- [ ] **Step 4.5: Stub `dedup::dedup_finding_candidates`**

In `crates/vox-scientia-producers/src/dedup.rs`:

```rust
use vox_research_events::ResearchEvent;

/// First-pass dedup: identity (returns input unchanged).
/// Task 5 implements real (producer, signal-fingerprint) dedup.
pub fn dedup_finding_candidates(events: Vec<ResearchEvent>) -> Vec<ResearchEvent> {
    events
}
```

Add `pub mod dedup;` and stub bodies for `heuristics`, `commit_graph`, `bench_history`, `socrates_telemetry` so the crate compiles:

```rust
// src/heuristics.rs
//! Scoring heuristics (Task 5 fleshes these out).

// src/commit_graph.rs
//! Commit-graph producer (Task 6 fleshes this out).

// src/bench_history.rs
//! Benchmark-history producer (Task 7 fleshes this out).

// src/socrates_telemetry.rs
//! Socrates-telemetry producer (Task 8 fleshes this out).
```

- [ ] **Step 4.6: Run smoke test to verify pass**

```bash
cargo test -p vox-scientia-producers registry_smoke
```
Expected: PASS.

- [ ] **Step 4.7: Commit**

```bash
git add crates/vox-scientia-producers
git commit -m "feat(vox-scientia-producers): Producer trait + ProducerRegistry skeleton"
```

---

## Task 5: Real dedup on (producer-name, signal-fingerprint)

**Files:**
- Modify: `crates/vox-scientia-producers/src/dedup.rs`

- [ ] **Step 5.1: Write the failing test**

Append to `crates/vox-scientia-producers/tests/registry_smoke.rs`:

```rust
#[tokio::test]
async fn dedup_collapses_duplicate_finding_ids() {
    use vox_scientia_producers::dedup::dedup_finding_candidates;
    let events = vec![
        ResearchEvent::FindingCandidateProposed { finding_id: "x".into(), claim_ids: vec![], worthiness_score: 0.5, session_id: "s".into() },
        ResearchEvent::FindingCandidateProposed { finding_id: "x".into(), claim_ids: vec![], worthiness_score: 0.7, session_id: "s".into() },
        ResearchEvent::FindingCandidateProposed { finding_id: "y".into(), claim_ids: vec![], worthiness_score: 0.4, session_id: "s".into() },
    ];
    let out = dedup_finding_candidates(events);
    assert_eq!(out.len(), 2, "duplicate finding_id 'x' must collapse to one");
}
```

- [ ] **Step 5.2: Run to verify it fails**

```bash
cargo test -p vox-scientia-producers dedup_collapses
```
Expected: FAIL (current dedup is identity).

- [ ] **Step 5.3: Implement real dedup**

Replace `dedup.rs`:

```rust
use std::collections::HashSet;

use vox_research_events::ResearchEvent;

/// Dedup `FindingCandidateProposed` events on `finding_id`.
/// On collision, the *first* event wins; later events with the same
/// finding_id are dropped. Non-finding events pass through unchanged.
pub fn dedup_finding_candidates(events: Vec<ResearchEvent>) -> Vec<ResearchEvent> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        match &ev {
            ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
                if seen.insert(finding_id.clone()) {
                    out.push(ev);
                }
            }
            _ => out.push(ev),
        }
    }
    out
}
```

- [ ] **Step 5.4: Verify pass**

```bash
cargo test -p vox-scientia-producers
```
Expected: PASS.

- [ ] **Step 5.5: Commit**

```bash
git add crates/vox-scientia-producers/src/dedup.rs crates/vox-scientia-producers/tests/registry_smoke.rs
git commit -m "feat(vox-scientia-producers): dedup FindingCandidateProposed by finding_id"
```

---

## Task 6: Commit-graph producer

**Files:**
- Modify: `crates/vox-scientia-producers/src/commit_graph.rs`
- Modify: `crates/vox-scientia-producers/src/heuristics.rs`

**Detector logic:** Walk the last N commits via `vox-git`. For each commit, compute (`+lines`, `-lines`). Emit `algorithmic_improvement` candidates when a single commit shows `-lines ≥ 50 AND +lines ≤ 10` (refactor / compression). Emit `reproducibility_infra` candidates when a commit's diff touches `tests/` AND adds ≥ 100 lines of test code. Real perf detection lives in Task 7.

- [ ] **Step 6.1: Write the synthetic-repo integration test**

Create `crates/vox-scientia-producers/tests/commit_graph_synthetic.rs`:

```rust
use std::process::Command;
use tempfile::tempdir;
use vox_scientia_producers::commit_graph::CommitGraphProducer;
use vox_scientia_producers::{Producer, ProducerContext};
use vox_research_events::ResearchEvent;

fn run(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git").args(args).current_dir(dir).status().unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

#[tokio::test]
async fn commit_graph_detects_compression_commit() {
    let dir = tempdir().unwrap();
    let p = dir.path();
    run(p, &["init", "-q"]);
    run(p, &["config", "user.email", "test@test"]);
    run(p, &["config", "user.name", "Test"]);

    // Seed file with many lines
    std::fs::write(p.join("big.txt"), "x\n".repeat(200)).unwrap();
    run(p, &["add", "."]);
    run(p, &["commit", "-q", "-m", "seed"]);

    // Compression commit: large delete, small add
    std::fs::write(p.join("big.txt"), "x\n".repeat(50)).unwrap();
    run(p, &["add", "."]);
    run(p, &["commit", "-q", "-m", "compress big.txt"]);

    let producer = CommitGraphProducer::new();
    let ctx = ProducerContext {
        repo_root: p.to_path_buf(),
        commit_window: 10,
        days_window: 30,
        now_ms: 1_747_000_000_000,
        session_id: "test".into(),
    };
    let events = producer.observe(&ctx).await;

    assert!(
        events.iter().any(|e| matches!(e,
            ResearchEvent::FindingCandidateProposed { finding_id, .. } if finding_id.contains("compress")
        )),
        "expected a compression-class candidate, got {:?}", events
    );
}
```

- [ ] **Step 6.2: Run to verify it fails**

```bash
cargo test -p vox-scientia-producers commit_graph_synthetic 2>&1 | head -30
```
Expected: compile error (`CommitGraphProducer` doesn't exist).

- [ ] **Step 6.3: Implement the producer**

Replace `crates/vox-scientia-producers/src/commit_graph.rs`:

```rust
//! Commit-graph signal producer.
//!
//! Walks the last `ctx.commit_window` commits via `vox-git` and emits
//! candidates for compression / refactor patterns (large delete with
//! small add) and reproducibility-infra patterns (test additions).

use async_trait::async_trait;
use sha3::{Digest, Sha3_256};
use vox_research_events::ResearchEvent;

use crate::producer::{Producer, ProducerContext};

pub struct CommitGraphProducer {
    pub compression_min_deleted: usize,
    pub compression_max_added: usize,
}

impl CommitGraphProducer {
    pub fn new() -> Self {
        Self { compression_min_deleted: 50, compression_max_added: 10 }
    }
}

#[async_trait]
impl Producer for CommitGraphProducer {
    fn name(&self) -> &'static str { "commit_graph" }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        let commits = match vox_git::list_recent_commits(&ctx.repo_root, ctx.commit_window) {
            Ok(c) => c,
            Err(_) => return Vec::new(),  // missing/invalid repo: silent skip
        };
        let mut out = Vec::new();
        for c in commits {
            if c.lines_deleted >= self.compression_min_deleted
                && c.lines_added <= self.compression_max_added
            {
                let mut hash = Sha3_256::new();
                hash.update(b"commit_graph:compress:");
                hash.update(c.sha.as_bytes());
                let fp = hex_short(&hash.finalize());
                let finding_id = format!("algimp-{}-compress-{}", date_slug(ctx.now_ms), &fp);
                out.push(ResearchEvent::FindingCandidateProposed {
                    finding_id,
                    claim_ids: vec![],
                    worthiness_score: heuristic_compression_score(&c),
                    session_id: ctx.session_id.clone(),
                });
            }
        }
        out
    }
}

fn heuristic_compression_score(c: &vox_git::CommitInfo) -> f64 {
    let ratio = c.lines_deleted as f64 / (c.lines_added.max(1) as f64);
    (ratio / 20.0).min(1.0)
}

fn date_slug(now_ms: i64) -> String {
    // YYYY-MM-DD from epoch ms. Use chrono if already a workspace dep; else inline minimal impl.
    use std::time::UNIX_EPOCH;
    let _ = (now_ms, UNIX_EPOCH);
    // Implementer: use whichever date crate the workspace already pulls in (chrono / time).
    "2026-05-15".to_string()  // PLACEHOLDER — wire to actual date crate in Task 6.4
}

fn hex_short(bytes: &[u8]) -> String {
    bytes[..6].iter().map(|b| format!("{:02x}", b)).collect()
}
```

> ⚠️ The `date_slug` placeholder needs Task 6.4 to wire it to whichever date crate the workspace uses. Resolve before commit.

**Important:** the function `vox_git::list_recent_commits` and the struct `vox_git::CommitInfo` (with `sha`, `lines_added`, `lines_deleted` fields) need to exist. Confirm in pre-flight Step P2; if they don't, the first sub-task of Task 6 is to add them to `vox-git`. (If `vox-git` exposes a different API surface, adapt — the contract used here is the minimum needed.)

- [ ] **Step 6.4: Replace `date_slug` placeholder**

Discover via `grep "chrono\|time::OffsetDateTime" Cargo.lock` whether `chrono` or `time` is in tree. Implement `date_slug` using whichever is present. If neither (unlikely), add `time` via `[workspace.dependencies]`.

- [ ] **Step 6.5: Run test to verify pass**

```bash
cargo test -p vox-scientia-producers commit_graph_synthetic
```
Expected: PASS — at least one compression candidate event emitted.

- [ ] **Step 6.6: Commit**

```bash
git add crates/vox-scientia-producers
git commit -m "feat(vox-scientia-producers): commit-graph producer detects compression patterns"
```

---

## Task 7: Benchmark-history producer

**Files:**
- Modify: `crates/vox-scientia-producers/src/bench_history.rs`
- Modify: `crates/vox-scientia-producers/src/heuristics.rs`

**Detector logic:** Query `ExecTimeRecord` rows from `agent_telemetry_flat` (per exploration findings). For each `tool_key`, compute the trailing-30-commit p95 and the prior-30-commit p95. If the delta is `≥ 20%` improvement AND the sample count is `≥ 30` in each window, emit an `algorithmic_improvement` candidate.

- [ ] **Step 7.1: Write the failing test**

In `crates/vox-scientia-producers/tests/bench_history_smoke.rs`:

```rust
use vox_db::store::ops_exec_time::insert_exec_time;  // verify exact module path in vox-db
use vox_db::Codex;
use vox_scientia_producers::bench_history::BenchHistoryProducer;
use vox_scientia_producers::{Producer, ProducerContext};

#[tokio::test]
async fn bench_history_detects_p95_improvement() {
    let codex = Codex::in_memory().await.unwrap();
    // Insert 60 baseline observations at ~500ms then 60 improved at ~100ms
    for i in 0..60 {
        insert_exec_time(codex.pool(), "tool:test", "vox", 500 + (i % 10), 1747000000000 + i).await.unwrap();
    }
    for i in 0..60 {
        insert_exec_time(codex.pool(), "tool:test", "vox", 100 + (i % 10), 1747000010000 + i).await.unwrap();
    }
    let producer = BenchHistoryProducer::new(codex.clone());
    let ctx = ProducerContext::for_test();
    let events = producer.observe(&ctx).await;
    assert!(!events.is_empty(), "expected p95-improvement candidate");
}
```

> ⚠️ The `insert_exec_time` and `ops_exec_time` module names are guesses. Step 7.2 verifies the actual API.

- [ ] **Step 7.2: Verify the actual ExecTimeRecord write API**

```bash
grep -rn "ExecTimeRecord" crates/vox-db/src/ | head -20
grep -rn "fn record_exec_time\|fn insert_exec" crates/vox-db/src/ | head -20
```
Adjust the test imports to match the real API.

- [ ] **Step 7.3: Implement BenchHistoryProducer**

Replace `bench_history.rs`:

```rust
use async_trait::async_trait;
use vox_research_events::ResearchEvent;

use crate::producer::{Producer, ProducerContext};

pub struct BenchHistoryProducer {
    codex: vox_db::Codex,
    pub min_samples_per_window: usize,
    pub min_improvement_fraction: f64,
}

impl BenchHistoryProducer {
    pub fn new(codex: vox_db::Codex) -> Self {
        Self { codex, min_samples_per_window: 30, min_improvement_fraction: 0.20 }
    }
}

#[async_trait]
impl Producer for BenchHistoryProducer {
    fn name(&self) -> &'static str { "bench_history" }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        // Pseudocode:
        //  1. Query distinct tool_keys with ≥ 2*min_samples observations in
        //     the last `days_window` days.
        //  2. For each, split into two equal-sized recent windows.
        //  3. p95 each; compute fractional improvement.
        //  4. Emit candidate when improvement ≥ threshold.
        //
        // Implementer: write the query against the *actual* table name and
        // schema discovered in Step 7.2.
        Vec::new()
    }
}
```

> The pseudocode comment is a deliberate punt — Step 7.4 fills in the real query once Step 7.2 has named the surface. **Do not** leave the empty Vec in the committed code.

- [ ] **Step 7.4: Fill in the query**

Implement against the real `vox-db` surface. Include the p95 computation in `heuristics::p95`:

```rust
// src/heuristics.rs
pub fn p95(samples: &[u64]) -> Option<u64> {
    if samples.is_empty() { return None; }
    let mut s: Vec<u64> = samples.to_vec();
    s.sort_unstable();
    let idx = ((s.len() as f64 * 0.95) as usize).min(s.len() - 1);
    Some(s[idx])
}
```

- [ ] **Step 7.5: Run test to verify pass**

```bash
cargo test -p vox-scientia-producers bench_history
```
Expected: PASS.

- [ ] **Step 7.6: Commit**

```bash
git add crates/vox-scientia-producers crates/vox-scientia-producers/tests
git commit -m "feat(vox-scientia-producers): bench-history producer detects p95 improvements"
```

---

## Task 8: Socrates-telemetry producer

**Files:**
- Modify: `crates/vox-scientia-producers/src/socrates_telemetry.rs`

**Detector logic:** Query the Socrates telemetry surface for contradiction-ratio time series. Emit `telemetry_trust` candidate when the trailing-30-window mean is ≥ 20% lower than the prior-30-window mean AND sample count ≥ 30 in each window.

- [ ] **Step 8.1: Map the Socrates query surface**

```bash
grep -rn "socrates_surface\|contradiction_ratio" crates/vox-db/src/ | head -20
grep -rn "fn fetch_socrates\|fn socrates_" crates/vox-db/src/ | head -20
```

Record the actual API (function name, return type) in the plan.

- [ ] **Step 8.2: Write the failing test**

Mirror Task 7's pattern: insert two windows of synthetic Socrates rows with different contradiction ratios; assert the producer emits a `telemetry_trust` candidate.

- [ ] **Step 8.3: Implement**

Mirror `BenchHistoryProducer` structure with the contradiction-ratio metric instead of `duration_ms`.

- [ ] **Step 8.4: Verify pass**

```bash
cargo test -p vox-scientia-producers socrates_telemetry
```

- [ ] **Step 8.5: Commit**

```bash
git add crates/vox-scientia-producers
git commit -m "feat(vox-scientia-producers): socrates-telemetry producer detects trust improvements"
```

---

## Task 9: Wire registry defaults

**Files:**
- Modify: `crates/vox-scientia-producers/src/registry.rs`

- [ ] **Step 9.1: Write the failing test**

In `tests/registry_smoke.rs` add:

```rust
#[tokio::test]
async fn default_registry_has_three_producers() {
    let reg = ProducerRegistry::default_with_codex(/* test codex */);
    // Should have: commit_graph, bench_history, socrates_telemetry
    assert_eq!(reg.producer_names(), vec!["commit_graph", "bench_history", "socrates_telemetry"]);
}
```

(Add `producer_names()` to the registry surface.)

- [ ] **Step 9.2: Replace `Default` impl**

```rust
impl ProducerRegistry {
    pub fn default_with_codex(codex: vox_db::Codex) -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(commit_graph::CommitGraphProducer::new()));
        reg.register(Box::new(bench_history::BenchHistoryProducer::new(codex.clone())));
        reg.register(Box::new(socrates_telemetry::SocratesTelemetryProducer::new(codex)));
        reg
    }
    pub fn producer_names(&self) -> Vec<&'static str> {
        self.producers.iter().map(|p| p.name()).collect()
    }
}
```

- [ ] **Step 9.3: Verify pass**

- [ ] **Step 9.4: Commit**

```bash
git commit -am "feat(vox-scientia-producers): default registry wires three producers"
```

---

## Task 10: Wire to `publication-discovery-scan`

**Files:**
- Modify: `crates/vox-publisher/src/scientia_evidence/mod.rs` (or wherever `publication-discovery-scan` reads its candidate source)

- [ ] **Step 10.1: Locate the read path**

```bash
grep -rn "publication-discovery-scan\|publication_discovery_scan" crates/vox-cli/src/ crates/vox-publisher/src/
```

- [ ] **Step 10.2: Extend the source list**

Add `scientia_finding_candidates` rows to whatever list the scan currently ranks. Preserve existing scan order; new rows appear at the end. (Re-ranking is downstream worthiness work — out of scope.)

- [ ] **Step 10.3: Test**

Add an integration test that inserts a candidate via `vox-scientia-producers` and verifies `publication-discovery-scan` surfaces it.

- [ ] **Step 10.4: Commit**

---

## Task 11: Docs

- [ ] **Step 11.1: Where things live**

Add to `docs/src/architecture/where-things-live.md` under the SCIENTIA cluster section:

```md
| [`vox-scientia-producers`](../../../crates/vox-scientia-producers/) | SCIENTIA self-observation signal producers: commit-graph, benchmark-history, Socrates-telemetry detectors emitting `FindingCandidateProposed`. |
```

- [ ] **Step 11.2: SSOT handbook entry**

Add a row to `docs/src/reference/scientia-ssot-handbook.md` §3 (SSOT map) noting the producer layer.

- [ ] **Step 11.3: README**

Create `crates/vox-scientia-producers/README.md` with a producer-authoring guide (one paragraph per producer, plus a "writing a new producer" mini-tutorial).

- [ ] **Step 11.4: Commit**

```bash
git commit -am "docs(vox-scientia-producers): SSOT handbook + where-things-live + README"
```

---

## Task 12: Final verification

- [ ] **Step 12.1: Full workspace test**

```bash
cargo test --workspace
```
Expected: green.

- [ ] **Step 12.2: Arch check**

```bash
cargo run -p vox-arch-check
```
Expected: exit code 0; `vox-scientia-producers` shows in the L2 inventory; no fan-in violations.

- [ ] **Step 12.3: Doc pipeline regen**

```bash
cargo run -p vox-doc-pipeline
```
Expected: regenerates any auto-generated docs that mention the new crate; no hand-edits required.

- [ ] **Step 12.4: Final commit**

```bash
git commit --allow-empty -m "phase-A: signal producers complete; ready for Phase F (scout)"
```

---

## Acceptance criteria

1. `cargo test -p vox-scientia-producers` green; all six task-level tests pass.
2. `cargo test --workspace` green.
3. `cargo run -p vox-arch-check` exit 0.
4. The synthetic commit-graph fixture produces exactly one expected candidate event (Task 6); no false positives.
5. `publication-discovery-scan` surfaces at least one producer-emitted candidate when run on a real Vox commit-history window.
6. Zero new external Rust dependencies that aren't already in the workspace (gix via `vox-git` is acceptable).
7. `where-things-live.md` includes the new crate row.

---

## Open questions (none blocking — punted to follow-ups)

- **OQ-A1.** MENS training event surface (deferred to Phase A.2): producer added when `vox-actor-runtime/mens.rs` publishes typed completion events. Not blocking this phase.
- **OQ-A3.** Producer pluggability (third-party detectors via `vox-plugin-host`): start fixed registry; revisit when third detector arrives.
- **OQ-A4.** Default windows (100 commits / 30 days): currently hard-coded; future config under `contracts/scientia/`.
- **OQ-A5.** Cross-producer dedup for the same root cause (commit-graph + bench-history both flag the same merge): collapse into combined `internal_signals[]`. Today: dedup is per-producer fingerprint only.

---

## Dependencies

- **Upstream (all complete):** Finalization Phase 0b (`vox-research-events`); Phase 6 (closed loop); `finding-candidate.v1.schema.json` contract; `vox-git`; `vox-db` core; `ExecTimeRecord`.
- **Downstream:** Phase F (`vox scientia scout`) — consumes the candidate ledger; Phase E (AI/SWE micro-track) — benefits from the broader candidate classes.

---

## Cross-references

- Gap: [gap-map §2 Gap A](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-a--self-observation-candidate-producers)
- Schema: [`contracts/scientia/finding-candidate.v1.schema.json`](../../../../contracts/scientia/finding-candidate.v1.schema.json)
- Schema: [`contracts/scientia/discovery-signal.schema.json`](../../../../contracts/scientia/discovery-signal.schema.json)
- Existing emitter: [`crates/vox-research-events/src/emitter.rs`](../../../../crates/vox-research-events/src/emitter.rs)
- Existing event variant: [`crates/vox-research-events/src/events.rs`](../../../../crates/vox-research-events/src/events.rs) line ~92 (`FindingCandidateProposed`)
- Mirror DB pattern: [`crates/vox-db/src/schema/domains/scientia.rs`](../../../../crates/vox-db/src/schema/domains/scientia.rs) lines 4–25 (`scientia_discoveries`)
- ExecTimeRecord: [`crates/vox-db-types/src/exec_time.rs`](../../../../crates/vox-db-types/src/exec_time.rs)
- Workspace dep convention: root [`Cargo.toml`](../../../../Cargo.toml) `[workspace.dependencies]` block
- Layer registry: [`docs/src/architecture/layers.toml`](../../../src/architecture/layers.toml)
