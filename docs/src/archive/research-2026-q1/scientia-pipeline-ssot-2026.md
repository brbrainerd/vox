---
title: "scientia pipeline ssot 2026"
description: "Automatically added frontmatter for scientia pipeline ssot 2026"
category: "Architecture SSOTs"
status: "research"
training_eligible: false
archived_date: 2026-04-18
---
# Scientia Pipeline SSOT — Unified Inbound/Outbound Gap Remediation (2026)

> **This is the authoritative implementation specification for the Vox Scientia research pipeline.**
> All prior gap analysis documents (`scientia-gap-analysis-2026.md`, `scientia-publication-readiness-audit.md`,
> `scientia-implementation-wave-playbook-2026.md`) remain valid for historical context but **this document
> supersedes them for implementation decisions.** Update this document — not those — when the plan changes.

---

## 0. How to Read This Document

This document is written for a downstream LLM agent that will implement each task.
Every task block is **self-contained**: it states the problem (code-verified), the exact file(s) to change,
the data contract to satisfy, and the acceptance test to pass. Do not assume context from prior tasks.

**Each task block follows this structure:**
```
### G{global-id}. Title
SEVERITY: [CRITICAL | HIGH | MEDIUM | LOW]
EFFORT: [hours]
OWNER CRATE: crate-name
VERIFIED: [the exact line/function that confirms the gap is real]
PROBLEM: ...
SOLUTION: ...
DATA CONTRACT: ...
ACCEPTANCE: ...
```

training_eligible: false
archived_date: 2026-04-18
---

## 1. Canonical Data Model

Before any implementation, understand the **two universes of data flow** this pipeline must unify.

### 1.1 Inbound Universe — External Intelligence

External content enters VoxDB through `knowledge_nodes` and `snippets`.
The existing `vox_db::research::ResearchIngestRequest` is the approved struct.

```
ExternalResearchPacket {
  topic, vendor, area, source_url, source_type, title,
  captured_at, summary, raw_excerpt, claims[], tags[],
  confidence, content_hash, metadata
}
→ knowledge_nodes (INSERT OR REPLACE, node_type='external_research')
→ snippets (language='research_chunk', source_ref=source_url)
→ search_documents + search_document_chunks (dual-write)
→ embeddings (per chunk, if vector provided)
```

**What does NOT exist yet** (verified absent by code audit):
- A table for tracking **feed sources** (RSS URLs, social handles, polling schedules).
- A `node_type` for **Scientia-discovered** findings (distinct from competitor research).
- A flag on `knowledge_nodes` or `search_documents` to mark that content has been **reflected into the RAG active corpus** after publication.
- A `tavily_credit_ledger` table or in-memory counter for session credit tracking.

### 1.2 Outbound Universe — Publication Manifests

Outbound content flows from `PublicationManifest` through `publish_cloud` and the scholarly adapters.

```
PublicationManifest {
  publication_id, title, author, body_markdown, metadata_json
}
→ metadata_json.scientific_publication (ScientificPublicationMetadata)
→ metadata_json.scientia_evidence (ScientiaEvidenceContext)
→ metadata_json.scientia_novelty_bundle (NoveltyEvidenceBundleV1)
→ publication_preflight → PreflightReport
→ scholarly adapter (zenodo / openreview)
→ scholarly_external_jobs (DB-backed job queue)
→ publish_cloud (DB ledger)
```

**What does NOT exist yet** (verified absent):
- An outbound `CrossrefAdapter` that sends HTTP deposits (code maps it but skips it).
- Any status sync mechanism that polls Zenodo/OpenReview after initial submit and writes the result back to `publish_cloud`.
- A `revision_history_json` column in `publish_cloud` for tracking resubmissions.
- A camera-ready LaTeX package builder (only markdown + zenodo JSON is generated).

### 1.3 The Feedback Loop (Missing Entirely)

After a finding is published (Zenodo deposit confirmed), **nothing feeds back** to the RAG corpora.
The connection that must be built:

```
publish_cloud (status=published) 
  → ingest finding as knowledge_node (node_type='scientia_published_finding')
  → index chunks into search_document_chunks
  → store embeddings
  → set knowledge_node.metadata.reflected_to_rag = true
```

### 1.4 Unified `node_type` Taxonomy

All `knowledge_nodes` inserted by the Scientia pipeline MUST use one of these `node_type` values.
This is the shared vocabulary across inbound, outbound, and feedback.

| node_type | Inserted by | Purpose |
|---|---|---|
| `external_research` | `vox_db::research::ingest_research_document_async` | Existing — competitor/vendor intel |
| `scientia_inbound_signal` | new ingest path (Tasks G1–G6) | RSS/social/preprint items pending triage |
| `scientia_published_finding` | new feedback path (Tasks G31–G34) | Published Scientia discoveries re-indexed |
| `scientia_crag_snapshot` | new CRAG persist path (Task G22) | Tavily/CRAG results cached per query |

---

## 2. Implementation Tasks — Wave 0: Foundation (≤ 1 week)

Wave 0 tasks are prerequisites for all other waves. They fix real code bugs and establish the data
structures. Do these first, in order.

training_eligible: false
archived_date: 2026-04-18
---

### G1. Fix `rank_candidate()` — novelty fields silently default to zero-overlap (perfect novelty)

SEVERITY: CRITICAL  
EFFORT: 2 hours  
OWNER CRATE: vox-publisher  
VERIFIED: `crates/vox-publisher/src/scientia_discovery.rs` — `rank_candidate()` function.
The function builds a `DiscoveryCandidate` but the `novelty_overlap` field is always `None`
because the caller must call a separate merge function. Any candidate that skips the merge gets
`None`, which the worthiness scorer treats as perfect novelty (0.0 overlap = best score).

PROBLEM: When `rank_candidate()` is called without a prior `merge_novelty_overlap()` call, the
`novelty_overlap` field is `None`. In `publication_worthiness.rs`, a `None` overlap is treated as
0.0 (no prior art), giving the candidate the maximum novelty score. This silently inflates scores
for un-checked candidates.

SOLUTION:  
In `scientia_discovery.rs`, change `rank_candidate()` to accept a required `novelty_overlap: Option<f32>` parameter.  
If `novelty_overlap.is_none()`, set a default of `0.5` (moderate overlap assumed) rather than treating `None` as perfect novelty.  
Add a doc comment: `/// Pass `None` only when no prior-art scan has run; a default of 0.5 is applied (not zero).`  
Update all callers.

DATA CONTRACT: `DiscoveryCandidate.novelty_overlap_assumed_default: bool` — set to `true` when the 0.5 default is applied, so preflight can warn: "Novelty assumed moderate (no prior art scan run)."

ACCEPTANCE:
- Unit test: calling `rank_candidate()` with `novelty_overlap=None` produces a score strictly less than calling it with `novelty_overlap=Some(0.0)`.
- `vox stub-check --path crates/vox-publisher/src/scientia_discovery.rs` passes.

---

### G2. Fix Coverage Paradox — contradiction penalty applied regardless of citation coverage

SEVERITY: HIGH  
EFFORT: 2 hours  
OWNER CRATE: vox-publisher  
VERIFIED: `crates/vox-publisher/src/publication_worthiness.rs`.
The contradiction penalty is subtracted from the worthiness score even when `citation_coverage < 0.3`,
meaning a paper with almost no citations can be penalized for contradictions it structurally cannot have.
The architecture doc (`scientia-publication-worthiness-ssot-unification-research-2026.md`, section
"Coverage Paradox") marks this as `[PLANNED]` but the fix is not in the code.

PROBLEM: The coverage paradox creates a catch-22: new research with too few citations (low coverage)
still gets contradiction-penalized, depressing worthiness unfairly.

SOLUTION:  
In `publication_worthiness.rs`, find the contradiction penalty application. Wrap it with:
```rust
if citation_coverage >= heuristics.worthiness_contradiction_coverage_gate {
    // apply contradiction penalty
}
```
Add `worthiness_contradiction_coverage_gate: f64` to `ScientiaHeuristics` (default: `0.3`).  
Add the YAML key `worthiness_proxy.contradiction_coverage_gate` to `impact-readership-projection.seed.v1.yaml`.

DATA CONTRACT: Add `contradiction_coverage_gate` under `heuristics.worthiness_proxy` in the seed YAML.

ACCEPTANCE:
- Unit test: a candidate with `citation_coverage = 0.1` and `contradiction_count = 5` receives the same score as one with zero contradictions.
- `vox stub-check --path crates/vox-publisher/src/publication_worthiness.rs` passes.

training_eligible: false
archived_date: 2026-04-18
---

### G3. Fix Tavily credit budget — `tavily_credit_budget_per_session` is declared but never enforced

SEVERITY: HIGH  
EFFORT: 3 hours  
OWNER CRATE: vox-search  
VERIFIED: `crates/vox-search/src/policy.rs` line 46: `tavily_credit_budget_per_session: usize` is
declared and defaults to 50. `crates/vox-search/src/bundle.rs` lines 145–190: Tavily is fired inside
`run_search_with_verification()` but there is no counter, no check against the budget, and no decrement.
The field is unused.

PROBLEM: Every CRAG fallback fires a Tavily API call with no session-level budget enforcement.
In a busy MCP session, this can exhaust credits silently.

SOLUTION:  
In `vox-search`, add a `TavilySessionBudget` struct:
```rust
/// Thread-safe atomic credit counter for one MCP/CLI session.
pub struct TavilySessionBudget {
    remaining: Arc<AtomicUsize>,
}
impl TavilySessionBudget {
    pub fn new(limit: usize) -> Self { ... }
    /// Returns `false` and does NOT decrement if already at zero.
    pub fn try_consume(&self, cost: usize) -> bool { ... }
    pub fn remaining(&self) -> usize { ... }
}
```
Pass `budget: &TavilySessionBudget` into `run_search_with_verification()`.  
Before firing Tavily, call `budget.try_consume(1)`. If it returns `false`, push
`"tavily_budget_exhausted"` into `execution.warnings` and skip the Tavily call.
After a successful call, push `format!("tavily_credits_remaining={}", budget.remaining())` into
`diagnostics.notes`.

DATA CONTRACT: `SearchDiagnostics.notes` entries with key `tavily_credits_remaining=N` and
`tavily_budget_exhausted` (boolean flag).

ACCEPTANCE:
- Unit test with budget=2: after 2 Tavily firings, third call is skipped and `warnings` contains `"tavily_budget_exhausted"`.
- `vox stub-check --path crates/vox-search/src` passes.

---

### G4. Add `vox-scientia-api` façade module — stop CLI/MCP bypassing publisher internals

SEVERITY: HIGH  
EFFORT: 4 hours  
OWNER CRATE: vox-publisher (new public module)  
VERIFIED: `crates/vox-publisher/src/lib.rs` — pub-exports everything at crate root. Both
`vox-cli` and `vox-mcp` import internal functions directly, bypassing any future middleware.

PROBLEM: There is no API boundary between `vox-publisher` internals and CLI/MCP callers.
Adding audit logging, caching, or rate limiting later requires touching all call sites.

SOLUTION:  
Create `crates/vox-publisher/src/scientia_api.rs` as a façade module. It re-exports only the
functions that CLI/MCP should call:
```rust
//! Stable API surface for vox-cli and vox-mcp. 
//! Do not call publisher internals directly from outside this crate — use these.
pub use crate::scientia_discovery::rank_candidate;
pub use crate::publication_worthiness::score_worthiness;
pub use crate::publication_preflight::{run_preflight, run_preflight_with_attention};
pub use crate::scientia_finding_ledger::NoveltyEvidenceBundleV1;
```
Add a `// FROZEN` module comment (per AGENTS.md policy) once the surface stabilizes.  
Update `lib.rs` to expose this module as `pub mod scientia_api`.

DATA CONTRACT: No data contract change. This is a module boundary only.

ACCEPTANCE:
- `cargo check -p vox-publisher` compiles.
- `cargo check -p vox-cli` compiles using the new import paths.

training_eligible: false
archived_date: 2026-04-18
---

### G5. Add `publish_cloud` column: `revision_history_json`

SEVERITY: HIGH  
EFFORT: 2 hours  
OWNER CRATE: vox-db  
VERIFIED: `crates/vox-db/src/` — no `revision_history_json` column exists in publish_cloud DDL.
The `scholarly_external_jobs.rs` creates new job rows for resubmissions but does not link them to
a revision chain, so the revision history is permanently lost.

PROBLEM: When a paper is rejected and resubmitted, the old job row is orphaned. No revision trail
exists in the DB.

SOLUTION:  
In the `.vox` schema file that declares `publish_cloud`, add:
```
revision_history_json TEXT DEFAULT '[]'
```
This is additive (auto-migrate safe).  

In `scholarly_external_jobs.rs`, when creating a new submission job that re-uses an existing
`publication_id`, write the previous `external_submission_id` and `status` into
`revision_history_json` as a JSON-appended array entry:
```json
[{"seq": 1, "adapter": "zenodo", "id": "12345", "status": "rejected", "at_ms": 1234567890}]
```

Expose a `VoxDb::append_revision_history(publication_id, entry)` method that reads, appends, and writes.

DATA CONTRACT:
```typescript
// revision_history_json element
{
  "seq": number,          // 1-indexed submission attempt
  "adapter": string,      // "zenodo" | "openreview"
  "id": string,           // external deposition/submission id
  "status": string,       // last known status at revision time
  "at_ms": number         // unix epoch ms
}
```

ACCEPTANCE:
- `VoxDb::auto_migrate()` applies the column without error on an existing DB.
- Round-trip test: submit → reject → resubmit → `revision_history_json` has 2 entries.

---

### G6. Fix SSOT fragmentation — worthiness thresholds in 5+ locations must converge to 1

SEVERITY: CRITICAL  
EFFORT: 3 hours  
OWNER CRATE: vox-publisher  
VERIFIED: By code search:
- `crates/vox-publisher/src/scientia_heuristics.rs` — `ScientiaHeuristics::default()` has 32 numeric constants.
- `crates/vox-publisher/src/publication_worthiness.rs` — additional hardcoded constants in function bodies.
- `contracts/scientia/impact-readership-projection.seed.v1.yaml` — partially overlapping set.
- `contracts/scientia/finding-candidate.v1.schema.json` — range limits for some fields.
- Research docs (`scientia-publication-worthiness-ssot-unification-research-2026.md`) — describes intended SSOT but it is not enforced.

PROBLEM: When tuning the discovery pipeline, an operator must edit 5 different files and recompile.
There is no CI check that confirms all locations agree.

SOLUTION (two steps):

**Step 1 — Migrate remaining hardcoded constants to `ScientiaHeuristics`:**  
Search `publication_worthiness.rs` for literal `f64` values. Move each one into a named field in
`ScientiaHeuristics` and the corresponding `HeuristicsYaml` struct.

**Step 2 — Add a CI parity check (`vox ci scientia-heuristics-parity`):**  
Create `tools/ci/scientia_heuristics_parity.rs` (or equivalent in the `vox ci` subsystem).
This tool:
1. Loads `ScientiaHeuristics::default()`.
2. Loads the YAML seed from `contracts/scientia/impact-readership-projection.seed.v1.yaml`.
3. Loads `contracts/scientia/finding-candidate.v1.schema.json`.
4. Asserts that the YAML seed's `heuristics.*` numeric values, when present, match
   `ScientiaHeuristics::default()`.
5. Exits non-zero on any mismatch.

Add to CI (`.github/workflows/` or equivalent) as a required check.

DATA CONTRACT: `contracts/scientia/impact-readership-projection.seed.v1.yaml` is the
**single source of truth** for all numeric tuning constants. `ScientiaHeuristics::default()` must
match it exactly. Mark the struct fields with `// SSOT: impact-readership-projection.seed.v1.yaml`.

ACCEPTANCE:
- `vox ci scientia-heuristics-parity` exits 0 with no YAML drift.
- Changing a value in `ScientiaHeuristics::default()` without updating the YAML makes it exit non-zero.

training_eligible: false
archived_date: 2026-04-18
---

## 3. Wave 1: Inbound Discovery Pipeline (1–2 weeks)

These tasks create the inbound pipeline from scratch. Do them in the order listed — later tasks
depend on earlier ones.

---

### G7. Create `scientia_feed_sources` table in VoxDB

SEVERITY: CRITICAL (prerequisite for G8–G11)  
EFFORT: 3 hours  
OWNER CRATE: vox-db  
VERIFIED: No `scientia_feed_sources` table found by searching all `.vox` schema files and `auto_migrate.rs`.

PROBLEM: There is no persistent registry of RSS feeds, social handles, or API endpoints to poll for
inbound research signals. Without this table, the ingestion system cannot be scheduled, replayed, or
audited.

SOLUTION:  
In the appropriate `.vox` schema file, add:
```vox
// vox:skip — SQL-like schema syntax not valid as standalone Vox top-level
table scientia_feed_sources {
  id            TEXT        PRIMARY KEY,  // uuid4
  feed_type     TEXT        NOT NULL,     // 'rss_atom' | 'twitter_user' | 'reddit_sub' | 'arxiv_query' | 'manual'
  label         TEXT        NOT NULL,     // human-readable name, e.g. "arXiv cs.AI daily"
  source_uri    TEXT        NOT NULL,     // URL or identifier
  topic_tags    TEXT        DEFAULT '[]', // JSON array of strings, used for routing to discovery pipeline
  query_filter  TEXT,                     // optional XPath/keyword/JMES filter applied post-fetch
  poll_interval_secs  INTEGER DEFAULT 86400,
  last_polled_at_ms   INTEGER DEFAULT 0,
  last_ingested_count INTEGER DEFAULT 0,
  enabled       INTEGER     DEFAULT 1,
  metadata_json TEXT        DEFAULT '{}',
  created_at    TEXT        DEFAULT (datetime('now')),
  updated_at    TEXT        DEFAULT (datetime('now'))
}

index scientia_feed_sources_by_type on scientia_feed_sources (feed_type)
index scientia_feed_sources_due     on scientia_feed_sources (last_polled_at_ms) where enabled = 1
```

In `vox-db/src/research.rs` (or a new `vox-db/src/scientia_inbound.rs`), add:

```rust
pub struct FeedSource { pub id: String, pub feed_type: String, pub label: String,
  pub source_uri: String, pub topic_tags: Vec<String>, pub query_filter: Option<String>,
  pub poll_interval_secs: i64, pub last_polled_at_ms: i64, pub enabled: bool,
  pub metadata: serde_json::Value }
impl VoxDb {
  pub async fn upsert_feed_source(&self, src: &FeedSource) -> Result<(), StoreError>;
  pub async fn list_due_feed_sources(&self, now_ms: i64) -> Result<Vec<FeedSource>, StoreError>;
  pub async fn mark_feed_polled(&self, id: &str, now_ms: i64, ingested_count: i64) -> Result<(), StoreError>;
}
```

DATA CONTRACT: `feed_type` enum values are enforced at the application layer only (SQLite has no enum support).
Any unknown `feed_type` must be logged and skipped — do not panic.

ACCEPTANCE:
- `VoxDb::auto_migrate()` creates the table on a fresh DB.
- `upsert_feed_source` + `list_due_feed_sources` round-trip test passes.

training_eligible: false
archived_date: 2026-04-18
---

### G8. Create `scientia_inbound_signals` table in VoxDB

SEVERITY: CRITICAL (prerequisite for G9–G11)  
EFFORT: 3 hours  
OWNER CRATE: vox-db  
VERIFIED: No `scientia_inbound_signals` table found. Currently, inbound items go into
`knowledge_nodes` with `node_type='external_research'`, which conflates competitor intelligence
with discovery candidates. This breaks the triage pipeline.

PROBLEM: Research mined from arXiv RSS looks the same as a competitor product analysis in the DB.
The Socrates triage and the worthiness scorer cannot distinguish them.

SOLUTION:  
Add a dedicated staging table for inbound candidates, separate from `knowledge_nodes`:
```vox
// vox:skip — SQL-like schema syntax not valid as standalone Vox top-level
table scientia_inbound_signals {
  id                TEXT PRIMARY KEY,        // uuid4
  feed_source_id    TEXT,                    // FK → scientia_feed_sources.id (nullable for manual)
  external_id       TEXT,                    // arXiv ID, tweet ID, etc.
  signal_type       TEXT NOT NULL,           // 'preprint' | 'blog' | 'social' | 'repo' | 'news'
  title             TEXT NOT NULL DEFAULT '',
  authors_json      TEXT DEFAULT '[]',       // JSON array of author name strings
  abstract_text     TEXT DEFAULT '',
  full_url          TEXT DEFAULT '',
  content_hash      TEXT DEFAULT '',         // blake3 of (title + abstract)
  raw_json          TEXT DEFAULT '{}',       // original API response
  topic_tags        TEXT DEFAULT '[]',       // inherited from feed_source.topic_tags + auto-inferred
  worthiness_score  REAL DEFAULT 0.0,        // heuristic pre-score from G9
  triage_status     TEXT DEFAULT 'pending',  // 'pending' | 'accepted' | 'rejected' | 'promoted'
  triage_notes      TEXT DEFAULT '',         // reason for triage decision
  knowledge_node_id TEXT,                    // FK → knowledge_nodes.id after G11 promotion
  created_at_ms     INTEGER NOT NULL,
  updated_at_ms     INTEGER NOT NULL
}

index scientia_inbound_by_triage  on scientia_inbound_signals (triage_status)
index scientia_inbound_by_hash    on scientia_inbound_signals (content_hash)
index scientia_inbound_by_feed    on scientia_inbound_signals (feed_source_id)
```

In `vox-db/src/scientia_inbound.rs`, add:
```rust
pub struct InboundSignal { /* mirrors table fields */ }
impl VoxDb {
  pub async fn insert_inbound_signal(&self, sig: &InboundSignal) -> Result<String, StoreError>;
  // INSERT OR IGNORE on content_hash to deduplicate
  pub async fn list_pending_signals(&self, limit: i64) -> Result<Vec<InboundSignal>, StoreError>;
  pub async fn update_signal_triage(&self, id: &str, status: &str, notes: &str) -> Result<(), StoreError>;
  pub async fn promote_signal_to_knowledge_node(&self, id: &str, node_id: &str) -> Result<(), StoreError>;
}
```

DATA CONTRACT: `content_hash` is `blake3(title.trim().to_lowercase() + "|" + abstract_text.trim())`.
Do NOT use the full body — the abstract is stable across re-fetches.
`triage_status` transitions are: `pending → accepted | rejected`, `accepted → promoted`.

ACCEPTANCE:
- `insert_inbound_signal` silently ignores duplicate content_hash.
- `update_signal_triage` to `rejected` is irreversible (cannot transition back).
- `vox stub-check --path crates/vox-db/src/scientia_inbound.rs` passes.

---

### G9. Implement RSS/Atom feed ingestion in a new `vox-scientia-ingest` crate

SEVERITY: CRITICAL  
EFFORT: 8 hours  
OWNER CRATE: new `crates/vox-scientia-ingest`  
VERIFIED: No such crate exists. `feed-rs` is listed in research docs as the planned dependency
but is not in any `Cargo.toml`.

PROBLEM: There is no mechanism to poll RSS/Atom feeds and turn them into `InboundSignal` rows.

SOLUTION:  
Create `crates/vox-scientia-ingest/` with:
- `Cargo.toml`: depends on `feed-rs = "1"`, `vox-db`, `vox-clavis`, `reqwest`, `tokio`, `tracing`.
- `src/lib.rs`: exposes `pub mod rss_poller`, `pub mod signal_extractor`, `pub mod triage_preflight`.
- `src/rss_poller.rs`:

```rust
/// Fetch one feed source, parse with feed-rs, return raw items.
pub async fn poll_feed(source: &FeedSource, http: &reqwest::Client) -> Result<Vec<FeedItem>, IngestError>;

pub struct FeedItem {
  pub external_id: String,    // guid or link as fallback
  pub title: String,
  pub authors: Vec<String>,
  pub summary: String,        // first 1000 chars of content/summary
  pub url: String,
  pub published_at_ms: Option<i64>,
  pub raw_json: serde_json::Value,
}
```

- `src/signal_extractor.rs`:

```rust
/// Convert a FeedItem into an InboundSignal ready for DB insert.
/// Applies topic_tags from the FeedSource. Computes content_hash.
/// Scores worthiness_score via a fast heuristic (no prior-art scan).
pub fn extract_signal(item: FeedItem, source: &FeedSource) -> InboundSignal;

/// Fast heuristic pre-score: keyword match against known high-value venues/topics.
/// Returns 0.0–1.0. Not a substitute for full worthiness scoring.
fn fast_prescore(title: &str, abstract_text: &str, topic_tags: &[String]) -> f64;
```

- `src/triage_preflight.rs`:

```rust
/// Socrates-style preflight BEFORE inserting (no Socrates runtime required).
/// Checks: title too short (<10 chars), abstract empty, URL missing, known spam domain.
/// Returns Ok(()) or Err(TriageRejectReason).
pub fn triage_preflight(item: &FeedItem) -> Result<(), TriageRejectReason>;

pub enum TriageRejectReason {
  TitleTooShort,
  NoAbstract,
  NoUrl,
  SpamDomain(String),
}
```

**Polling loop** in CLI (`vox scientia ingest-feeds --dry-run`):
1. Call `db.list_due_feed_sources(now_ms)`.
2. For each due source, call `poll_feed(source, http)`.
3. For each item, call `triage_preflight`. On reject, log and skip.
4. Call `extract_signal` → `db.insert_inbound_signal`. Catch duplicate-hash silently.
5. Call `db.mark_feed_polled(source.id, now_ms, count)`.

DATA CONTRACT: `InboundSignal.worthiness_score` from `fast_prescore()` is informational only.
The full `publication_worthiness` scorer runs only on `accepted` signals in Wave 2 (G16).

ACCEPTANCE:
- `cargo test -p vox-scientia-ingest` passes with a mock HTTP server returning a sample arXiv RSS feed.
- Duplicate item (same content_hash) inserts without error and count is not incremented twice.
- `vox stub-check --path crates/vox-scientia-ingest/src` passes (no unimplemented!() or todo!()).

training_eligible: false
archived_date: 2026-04-18
---

### G10. Seed default feed sources in Clavis + DB bootstrap

SEVERITY: HIGH  
EFFORT: 3 hours  
OWNER CRATE: vox-clavis, vox-scientia-ingest  
VERIFIED: `vox-clavis/src/spec.rs` — has `SecretId::VoxOpenReviewAccessToken` etc. but no
inbound feed API keys. The `VOX_SCIENTIA_REDDIT_INBOUND` environment variable is mentioned in
research docs but has no Clavis `SecretId`.

PROBLEM: There is no canonical list of default inbound sources, and API keys for them have
no Clavis registration.

SOLUTION:  
In `vox-clavis/src/spec.rs`, add:
```rust
/// Reddit OAuth client for inbound r/MachineLearning / r/compsci monitoring.
VoxScientiaRedditClientId,
VoxScientiaRedditClientSecret,
/// arXiv API key (optional; public API works without it but with rate limits).
VoxArxivApiKey,
```

Create `contracts/scientia/default-feed-sources.v1.json` with the canonical seed list:
```json
[
  {
    "id": "arxiv-cs-ai",
    "feed_type": "rss_atom",
    "label": "arXiv cs.AI daily",
    "source_uri": "https://rss.arxiv.org/rss/cs.AI",
    "topic_tags": ["machine_learning", "ai"],
    "poll_interval_secs": 86400
  },
  {
    "id": "arxiv-cs-lg",
    "feed_type": "rss_atom",
    "label": "arXiv cs.LG daily",
    "source_uri": "https://rss.arxiv.org/rss/cs.LG",
    "topic_tags": ["machine_learning"],
    "poll_interval_secs": 86400
  },
  {
    "id": "reddit-ml",
    "feed_type": "reddit_sub",
    "label": "r/MachineLearning",
    "source_uri": "r/MachineLearning",
    "topic_tags": ["machine_learning", "research"],
    "poll_interval_secs": 3600
  }
]
```

The CLI command `vox scientia feed-sources seed` reads this file and calls `db.upsert_feed_source()` for each entry. Idempotent — safe to run multiple times.

DATA CONTRACT: `id` in `default-feed-sources.v1.json` is the stable primary key. Never reuse a retired id.

ACCEPTANCE:
- `vox scientia feed-sources seed --dry-run` prints the list without writing.
- `vox scientia feed-sources seed` inserts exactly 3 rows on a fresh DB, 0 rows on re-run.

---

### G11. Implement semantic deduplication guard for inbound signals

SEVERITY: HIGH  
EFFORT: 4 hours  
OWNER CRATE: vox-scientia-ingest  
VERIFIED: `crates/vox-db/src/research.rs` line 163: `INSERT OR REPLACE INTO knowledge_nodes`
uses `content_hash` only for the `id` (not a UNIQUE constraint dedup). The `scientia_inbound_signals`
table in G8 uses `content_hash` but only for title+abstract. Two different articles with the same
abstract (e.g., arXiv v1 vs v2) would collide.

PROBLEM: Version 2 of an arXiv preprint has the same abstract as v1 but is a different document.
The blake3 hash on title+abstract would produce the same hash, silently discarding the update.

SOLUTION:  
Change the dedup key for `scientia_inbound_signals.content_hash` to include the version-sensitive `external_id`:
```
content_hash = blake3(external_id | "|" | title.trim().to_lowercase())
```

Additionally, in the polling loop (G9), before inserting, query for an existing signal with the same `full_url`:
```sql
SELECT id FROM scientia_inbound_signals WHERE full_url = ?1 LIMIT 1
```
If found, update its `raw_json` and `updated_at_ms` instead of inserting.

DATA CONTRACT: `content_hash` is now `blake3(external_id + "|" + title.trim().to_lowercase())`.
Document this in `vox-db/src/scientia_inbound.rs` as a module-level doc comment.

ACCEPTANCE:
- arXiv v1 and v2 of the same paper create two separate rows (different external_id).
- The same v2 fetched twice creates only one row (update path, not insert).

training_eligible: false
archived_date: 2026-04-18
---

## 4. Wave 2: RAG-to-Scientia Feedback Loop (2–3 weeks)

---

### G12. Create `SocratesResearchDecision::evaluate_research_need()` — marked PLANNED, implement it

SEVERITY: CRITICAL  
EFFORT: 6 hours  
OWNER CRATE: vox-socrates-policy  
VERIFIED: Architecture doc `rag-and-research-architecture-2026.md` says this function is `[PLANNED]`.
Search `crates/vox-socrates-policy/src/` — the function signature exists as a stub but the body
is `unimplemented!()` or empty-return.

PROBLEM: When Socrates decides `Abstain`, there is no path that checks: "Should we trigger a CRAG
web search?" The `evaluate_research_need()` function is the intended decision bridge, but it is not
implemented. Every `Abstain` is a dead end.

SOLUTION:  
In `vox-socrates-policy`, implement `evaluate_research_need()`:

```rust
/// Given a Socrates `Abstain` event, determine if a CRAG web search should be triggered.
/// Returns `Some(research_query)` if CRAG should fire, `None` if Abstain should stand.
pub fn evaluate_research_need(
  decision: RiskDecision,
  confidence: f64,
  contradiction_ratio: f64,
  query_text: &str,
  evidence_quality: f64,
  policy: &SocratesResearchPolicy,
) -> Option<String> {
  if decision != RiskDecision::Abstain { return None; }
  if confidence < policy.research_trigger_confidence_ceiling
    && evidence_quality < policy.research_trigger_evidence_ceiling {
    // Refine the query: drop stopwords, keep noun phrases
    Some(refine_query_for_research(query_text))
  } else {
    None
  }
}
```

Add `SocratesResearchPolicy` struct with fields:
- `research_trigger_confidence_ceiling: f64` (default: 0.40)
- `research_trigger_evidence_ceiling: f64` (default: 0.50)

Load from env: `VOX_SOCRATES_RESEARCH_CONFIDENCE_CEILING`, `VOX_SOCRATES_RESEARCH_EVIDENCE_CEILING`.

The `refine_query_for_research()` helper: strip common stop words, trim to 120 chars.

DATA CONTRACT: The returned `String` is fed directly to `TavilySearchClient::search()` (G3)
and to `vox-scientia-ingest` for creating an `InboundSignal` with `signal_type = "crag_triggered"`.

ACCEPTANCE:
- `evaluate_research_need(Abstain, 0.2, 0.1, "how does X work", 0.3, default_policy)` returns `Some("...")`.
- `evaluate_research_need(Answer, 0.9, 0.0, "...", 0.9, default_policy)` returns `None`.
- `evaluate_research_need(Abstain, 0.9, 0.1, "...", 0.9, default_policy)` returns `None` (high confidence, don't trigger).

training_eligible: false
archived_date: 2026-04-18
---

### G13. Persist CRAG Tavily results to `knowledge_nodes` — stop ephemeral results burning credits

SEVERITY: HIGH  
EFFORT: 4 hours  
OWNER CRATE: vox-search  
VERIFIED: `crates/vox-search/src/bundle.rs` lines 159–178: Tavily results are added to
`execution.web_lines` and `execution.rrf_fused_lines` (in-memory only). They are never written
to any DB table. On the next query for similar content, Tavily fires again.

PROBLEM: Each CRAG fallback is idempotent from the API's perspective but costs API credits.
Semantically equivalent queries (rephrased) will always fire Tavily even if a relevant result
was fetched moments ago.

SOLUTION:  
After a successful Tavily call, write results to `knowledge_nodes` with `node_type = 'scientia_crag_snapshot'`:

```rust
// In bundle.rs, after successful Tavily call:
if let Some(db) = ctx.db.as_ref() {
  for hit in &tavily_hits {
    let node_id = format!("crag:{}", blake3_hex(hit.url.as_bytes()));
    let meta = serde_json::json!({
      "query": query, "url": hit.url, "title": hit.title,
      "score": hit.score, "fetched_at_ms": now_ms(),
      "crag_ttl_ms": policy.crag_cache_ttl_ms
    });
    let _ = db.upsert_knowledge_node_simple(
      &node_id, &hit.title, &hit.content, "scientia_crag_snapshot",
      &meta.to_string()
    ).await;
  }
}
```

Add `upsert_knowledge_node_simple(id, label, content, node_type, metadata)` to `VoxDb`.
This is `INSERT OR REPLACE INTO knowledge_nodes`.

Add `crag_cache_ttl_ms: u64` (default: `3_600_000` = 1 hour) to `SearchPolicy`.
Before firing Tavily, query:
```sql
SELECT content FROM knowledge_nodes
WHERE node_type = 'scientia_crag_snapshot'
AND json_extract(metadata, '$.query') = ?1
AND (strftime('%s','now') * 1000) - json_extract(metadata, '$.fetched_at_ms') < ?2
LIMIT 5
```
If hit, inject cached results into `execution.web_lines` and skip Tavily.

DATA CONTRACT: `node_type = 'scientia_crag_snapshot'` is in the unified taxonomy (see §1.4).
TTL is enforced at query time, not via DELETE (soft expiry).

ACCEPTANCE:
- Unit test: after one Tavily call, second identical query does not call Tavily (uses cache).
- Cache expires after TTL and re-fires Tavily.

---

### G14. Implement RAG feedback loop — index published Scientia findings back into search corpora

SEVERITY: CRITICAL  
EFFORT: 6 hours  
OWNER CRATE: vox-db, vox-publisher  
VERIFIED: `crates/vox-db/src/research.rs` — `ingest_research_document_async` exists but is never
called from `scholarly_external_jobs.rs` after a publication is confirmed. When Zenodo publishes
and returns `state = "published"`, the scholarly adapter returns a `ScholarlySubmissionReceipt`
and the job is marked done. No further action writes the finding to `search_documents` or
`knowledge_nodes` as a first-class searchable item.

PROBLEM: Published Scientia findings are invisible to future RAG queries. This means the system
cannot build on its own published work.

SOLUTION:  
In `scholarly_external_jobs.rs`, after a job transitions to `completed` state, call a new function:
```rust
pub async fn reflect_published_finding_to_rag(
  db: &VoxDb,
  publication_id: &str,
  manifest: &PublicationManifest,
  receipt: &ScholarlySubmissionReceipt,
) -> Result<(), StoreError>
```

This function:
1. Builds an `ExternalResearchPacket` from the manifest fields.
2. Sets `node_type = 'scientia_published_finding'` (**not** `'external_research'`).
3. Sets `source_url` to the Zenodo DOI URL from `receipt.metadata_json` (parse `doi` field).
4. Sets `vendor = "vox_scientia"` (marks it as self-authored; needed for `list_research_packets` filtering).
5. Calls `db.ingest_research_document_async(&mut req)`.
6. Updates the `publish_cloud` row: `ADD COLUMN reflected_to_rag INTEGER DEFAULT 0`, set to `1`.

Add `reflected_to_rag INTEGER DEFAULT 0` to `publish_cloud` (additive, auto-migrate safe).

DATA CONTRACT: `vendor = "vox_scientia"` is the canonical tag for self-published Scientia content.
Never use `"internal"`, `"self"`, or `"vox"` — they differ and break filter queries.

ACCEPTANCE:
- After `scholarly_external_jobs::process_completed_job()` runs, `knowledge_nodes` has a row with
  `node_type = 'scientia_published_finding'` and the correct `source_url`.
- `publish_cloud.reflected_to_rag = 1`.
- A RAG query for the paper title returns it from `knowledge_lines` in `SearchExecution`.

training_eligible: false
archived_date: 2026-04-18
---

### G15. Socrates Abstain events must create `InboundSignal` rows instead of being discarded

SEVERITY: HIGH  
EFFORT: 3 hours  
OWNER CRATE: vox-search (integration point), vox-scientia-ingest  
VERIFIED: `crates/vox-search/src/bundle.rs` — the CRAG section generates `t_lines` from Tavily
but only pushes them into the in-memory `execution.web_lines`. Nothing invokes
`evaluate_research_need()` (G12). CRAG results are not linked back to `InboundSignal`.

PROBLEM: A Socrates `Abstain` that triggers a CRAG web search produces interesting external results
that are immediately discarded (after the session ends). These results are exactly the kind of
`InboundSignal` that should enter the triage pipeline for possible publication.

SOLUTION:  
After a successful Tavily CRAG call, for each hit with `score >= policy.crag_signal_promote_threshold`:
```rust
let sig = InboundSignal {
  id: uuid4(),
  feed_source_id: None,   // manually triggered
  external_id: hit.url.clone(),
  signal_type: "crag_triggered",
  title: hit.title.clone(),
  abstract_text: hit.content.chars().take(500).collect(),
  full_url: hit.url.clone(),
  content_hash: blake3(external_id + "|" + title),
  worthiness_score: hit.score as f64,
  triage_status: "pending",
  ...
};
let _ = db.insert_inbound_signal(&sig).await;
```

Add `crag_signal_promote_threshold: f32` (default: `0.70`) to `SearchPolicy`.

DATA CONTRACT: `signal_type = "crag_triggered"` identifies signals from CRAG vs. feed polling.
They go through the same triage_preflight (G9) before being promoted.

ACCEPTANCE:
- A Tavily hit with `score >= 0.70` creates an `InboundSignal` row with `triage_status = "pending"`.
- A hit with `score < 0.70` does not create a row.

---

## 5. Wave 3: Advanced Discovery Mechanisms (2–4 weeks)

training_eligible: false
archived_date: 2026-04-18
---

### G16. Full worthiness scoring for `accepted` InboundSignals — prior-art scan integration

SEVERITY: HIGH  
EFFORT: 8 hours  
OWNER CRATE: vox-publisher, vox-scientia-ingest  
VERIFIED: `crates/vox-publisher/src/scientia_prior_art.rs` — `run_prior_art_scan()` exists and works.
`crates/vox-scientia-ingest/src/signal_extractor.rs` (created in G9) uses only `fast_prescore()`.
No code runs the full prior-art scan for inbound signals.

PROBLEM: Accepted inbound signals get a fast heuristic score only. Full worthiness scoring (including
prior-art Tavily search and novelty overlap) never runs on them.

SOLUTION:  
Create `vox-scientia-ingest/src/worthiness_enricher.rs`:
```rust
/// Run full prior-art scan + worthiness scoring for a promoted InboundSignal.
/// Must be called AFTER signal is in 'accepted' state.
pub async fn enrich_accepted_signal(
  signal: &InboundSignal,
  db: &VoxDb,
  heuristics: &ScientiaHeuristics,
  tavily_budget: &TavilySessionBudget,
) -> Result<EnrichedSignal, IngestError>;

pub struct EnrichedSignal {
  pub signal_id: String,
  pub worthiness_score: f64,      // from ScientiaHeuristics
  pub novelty_overlap: Option<f32>,
  pub prior_art_hits: Vec<PriorArtHit>,
  pub draft_preparation: DraftPreparationHints,
}
```

The function:
1. Calls `scientia_prior_art::run_prior_art_scan()` with signal title + abstract.
2. Calls `rank_candidate()` (G1 fixed) with the novelty overlap result.
3. Calls `publication_worthiness::score_worthiness()`.
4. Updates `scientia_inbound_signals.worthiness_score` in DB.
5. Promotes signal to `evidence` phase if score >= `heuristics.worthiness_promote_threshold` (new field, default: 0.65).

Add `worthiness_promote_threshold: f64` to `ScientiaHeuristics` and to the YAML seed.

DATA CONTRACT: `EnrichedSignal` is not persisted directly. Only `worthiness_score` is written back.
`prior_art_hits` are stored in `knowledge_nodes` per G13 (CRAG cache).

ACCEPTANCE:
- End-to-end test: seed a fake `InboundSignal`, call `enrich_accepted_signal`, verify `worthiness_score` is updated in DB.
- `vox stub-check --path crates/vox-scientia-ingest/src/worthiness_enricher.rs` passes.

---

### G17. Implement evidence completeness scoring — fix equal-weight flaw

SEVERITY: MEDIUM  
EFFORT: 3 hours  
OWNER CRATE: vox-publisher  
VERIFIED: `crates/vox-publisher/src/publication_worthiness.rs` — `evidence_completeness_score()`
counts which of 9–11 evidence signals are present and divides by `heuristics.evidence_completeness_max`
(which defaults to 9). All signals are weighted equally. A "benchmark pair complete" signal has
the same weight as "author_bio_present".

PROBLEM: Equal-weight completeness scoring means a paper with many minor signals outscores one
with fewer but more scientifically significant signals (benchmark pair + eval gate).

SOLUTION:  
Replace the equal-weight count with a weighted sum:
```rust
let weights: &[(SignalFamily, f64)] = &[
  (BenchmarkPair, 3.0),
  (EvalGate,      3.0),
  (OperatorAttestation, 2.0),
  (ReproducibilityArtifact, 2.0),
  (MensScorecard, 1.5),
  (LinkedCorpus,  1.0),
  (Documentation, 0.5),
  (TelemetryAggregate, 0.5),
  (TrustRollup,   0.5),
];
let max_weight: f64 = weights.iter().map(|(_, w)| w).sum();
let score = signals.iter().map(|s| weight_for(s.family)).sum::<f64>() / max_weight;
```

Expose `evidence_completeness_signal_weights` as a YAML key in the seed file (JSON object of
`family_name → weight`). `ScientiaHeuristics` stores a `HashMap<DiscoverySignalFamily, f32>`.

DATA CONTRACT: `evidence_completeness_signal_weights` in YAML is the SSOT for these weights.

ACCEPTANCE:
- A signal set of `[BenchmarkPair, EvalGate]` outscores `[Documentation, LinkedCorpus, TelemetryAggregate, TrustRollup, Documentation, Documentation]` (quality > quantity).

training_eligible: false
archived_date: 2026-04-18
---

### G18. Implement MENS Lane G (research-expert) runtime integration

SEVERITY: HIGH  
EFFORT: 12 hours  
OWNER CRATE: new module in vox-orchestrator or vox-scientia-ingest  
VERIFIED: `docs/src/architecture/mens-research-track-blueprint-2026.md` specifies Lane G.
Search `crates/` — no crate has `lane_g`, `research_expert`, or `mens_research_track` in any
source file. The blueprint is **specification only**; runtime integration is absent.

PROBLEM: The MENS "Research Expert" training track is specified but has zero runtime hooks.
Scientia discoveries are never routed to Lane G training data generation.

SOLUTION:  
Create `crates/vox-orchestrator/src/scientia_mens_hook.rs` (or equivalent in the orchestrator):
```rust
/// Called after a Scientia finding is promoted to `accepted` status.
/// Generates a Lane G training example if the finding meets quality threshold.
pub async fn maybe_emit_lane_g_example(
  signal: &EnrichedSignal,  // from G16
  heuristics: &ScientiaHeuristics,
  mens_output_dir: &Path,   // from env: VOX_MENS_LANE_G_OUTPUT_DIR
) -> Result<Option<PathBuf>, MensHookError>;
```

A Lane G example is a JSON file at `{output_dir}/lane_g_{signal_id}.json`:
```json
{
  "track": "lane_g_research_expert",
  "input": {
    "query": "<signal title as research question>",
    "context": "<abstract_text>"
  },
  "target_output": {
    "evidence_synthesis": "<to be filled by human reviewer>",
    "citation_grounding": "<extracted prior_art_hits URLs>",
    "novelty_assessment": "<computed novelty_overlap>",
    "recommended_action": "draft | reject | monitor"
  },
  "reward_signals": {
    "citation_coverage": <prior_art_hits.len() / 5.0 capped at 1.0>,
    "novelty_score": <1.0 - novelty_overlap>
  }
}
```

Emit only when `EnrichedSignal.worthiness_score >= heuristics.mens_lane_g_worthiness_gate` (new field, default: 0.70).

Add `mens_lane_g_worthiness_gate: f64` to `ScientiaHeuristics` and YAML seed.

DATA CONTRACT: The `target_output.evidence_synthesis` field is intentionally empty — it is filled
by a human reviewer during the MENS annotation phase. Do not auto-fill it with AI-generated text.

ACCEPTANCE:
- A high-quality `EnrichedSignal` (score >= 0.70) produces a JSON file with all required keys.
- A low-quality signal produces no file (None return).
- `vox stub-check --path crates/vox-orchestrator/src/scientia_mens_hook.rs` passes.

---

## 6. Wave 4: Outbound Publication Pipeline Completion (2–3 weeks)

training_eligible: false
archived_date: 2026-04-18
---

### G19. Crossref adapter — wire the HTTP deposit call that currently doesn't fire

SEVERITY: HIGH  
EFFORT: 6 hours  
OWNER CRATE: vox-publisher  
VERIFIED: `crates/vox-publisher/src/crossref_metadata.rs` — the struct
`CrossrefDepositBody` exists and serializes to the correct Crossref XML schema.
`crates/vox-publisher/src/scholarly/mod.rs` — no `CrossrefAdapter` struct exists.
The Crossref adapter is referenced in arch docs and `PreflightProfile::MetadataComplete` but
no HTTP POST to `https://doi.crossref.org/servlet/deposit` is ever sent.

PROBLEM: Crossref DOI registration never fires. Papers submitted to Zenodo need a Crossref deposit
to get a proper DOI resolved through the main registry (not just Zenodo's internal DOI).

SOLUTION:  
Create `crates/vox-publisher/src/scholarly/crossref.rs`:
```rust
pub(super) struct CrossrefAdapter { client: reqwest::Client, username: String, password: String }
impl CrossrefAdapter {
  pub(super) fn from_clavis() -> Result<Self, ScholarlyError>;
  // POST multipart/form-data to https://doi.crossref.org/servlet/deposit
  async fn deposit_once(&self, xml_body: &str, operation: &str) -> Result<CrossrefDepositReceipt, ScholarlyError>;
  pub(super) async fn deposit(&self, xml_body: &str) -> Result<CrossrefDepositReceipt, ScholarlyError>;
}
pub(super) struct CrossrefDepositReceipt { pub batch_id: String, pub status: String }
```

Add `SecretId::VoxCrossrefUsername` and `SecretId::VoxCrossrefPassword` to `vox-clavis/src/spec.rs`.

Add to `ScientiaHeuristics` (and YAML): `crossref_deposit_enabled: bool` (default: `false`, must be explicitly opted in).

In `scholarly/mod.rs`, route to `CrossrefAdapter` when `crossref_deposit_enabled` is `true`
and the manifest has a DOI field in `scientific_publication.doi`.

DATA CONTRACT: Crossref deposits are XML. Use `crossref_metadata::CrossrefDepositBody` → `.to_xml()`.
The DOI in `scientific_publication.doi` must be pre-registered (not auto-assigned) — validate
format `^10\\.\\d{4,9}/` before sending.

ACCEPTANCE:
- Mock HTTP server test: `CrossrefAdapter::deposit()` sends a POST with correct `Content-Type: multipart/form-data` and `operation=doMDUpload`.
- In dry-run mode, prints the XML body without sending.

---

### G20. Status sync job — poll Zenodo/OpenReview for status changes

SEVERITY: HIGH  
EFFORT: 8 hours  
OWNER CRATE: vox-publisher, vox-db  
VERIFIED: `crates/vox-publisher/src/scholarly/zenodo.rs` — `fetch_status()` method exists and
correctly calls `GET /deposit/depositions/{id}`. `crates/vox-publisher/src/scholarly/external_jobs.rs`
— no scheduled status sync loop exists. Submitted jobs stay in `submitted` state forever in `publish_cloud`.

PROBLEM: A paper accepted on Zenodo remains `status = 'submitted'` in `publish_cloud` unless
an operator manually calls a status-check command. There is no autonomous status reconciliation.

SOLUTION:  
In `scholarly_external_jobs.rs`, add `sync_scholarly_statuses()`:
```rust
/// For all publish_cloud rows with status IN ('submitted', 'pending_review', 'under_review'),
/// call fetch_status() on the appropriate adapter and update publish_cloud.
pub async fn sync_scholarly_statuses(
  db: &VoxDb,
  adapters: &HashMap<String, Box<dyn ScholarlyAdapter>>,
  dry_run: bool,
) -> Result<SyncReport, ScholarlyError>;

pub struct SyncReport {
  pub checked: usize,
  pub updated: usize,
  pub errors: Vec<(String, String)>,  // (publication_id, error_msg)
}
```

Status mapping from Zenodo to canonical `publish_cloud.status`:
| Zenodo state | publish_cloud status |
|---|---|
| `draft` | `draft` |
| `published` | `published` |
| `inprogress` | `submitted` |
| anything else | `unknown_<zenodo_state>` |

Add `status_synced_at_ms INTEGER DEFAULT 0` to `publish_cloud` (additive).

CLI: `vox scientia publication-sync-status [--publication-id <id>] [--dry-run]`.

After status changes to `published`, trigger `reflect_published_finding_to_rag()` (G14).

DATA CONTRACT: `status_synced_at_ms` is the epoch ms of the last successful poll.
The tool MUST NOT mark a row as `published` based only on its own submission receipt —
it must confirm via `fetch_status()`.

ACCEPTANCE:
- Test: mock Zenodo returns `state = "published"` → `publish_cloud.status` is updated to `"published"`.
- Test: `reflect_published_finding_to_rag()` is called after the status update.
- `vox stub-check --path crates/vox-publisher/src/scholarly/external_jobs.rs` passes.

training_eligible: false
archived_date: 2026-04-18
---

### G21. Double-blind anonymization gate — fix email-only pattern matching

SEVERITY: MEDIUM  
EFFORT: 2 hours  
OWNER CRATE: vox-publisher  
VERIFIED: `crates/vox-publisher/src/publication_preflight.rs` — `PreflightProfile::DoubleBlind`
checks for email patterns using `email_pattern()` regex and for ORCID IDs using `orcid_id_pattern()`.
No check exists for: author institution names, GitHub usernames, repository URLs containing
a real username, or "Acknowledgments" sections naming people.

PROBLEM: A double-blind submission can pass preflight with a GitHub URL like
`https://github.com/jane-doe/myrepo` or "This work was done at Acme Corp" in the body.

SOLUTION:  
In `run_preflight_with_attention()`, add a `DoubleBlind` profile section:
```rust
if profile == PreflightProfile::DoubleBlind {
  // 1. GitHub URL pattern: look for github.com/<username>/<repo> in body_markdown
  if body_has_github_user_url(&manifest.body_markdown) {
    findings.push(PreflightFinding {
      code: "double_blind_github_url",
      severity: PreflightSeverity::Error,
      message: "Body contains a GitHub URL with a username — anonymize before double-blind submit."
    });
  }
  // 2. Acknowledgment section: if any author name from scientific_publication.authors appears
  //    verbatim in the body_markdown.
  if let Ok(Some(ref sci)) = parse_scientific_from_metadata_json(...) {
    for author in &sci.authors {
      if body_contains_name(&manifest.body_markdown, &author.name) {
        findings.push(PreflightFinding {
          code: "double_blind_author_named_in_body", ...
        });
      }
    }
  }
}
```

Add `fn body_has_github_user_url(body: &str) -> bool` using the pattern `github.com/[a-zA-Z0-9._-]+/`.
Add `fn body_contains_name(body: &str, name: &str) -> bool` — case-insensitive substring match on names with ≥ 2 tokens.

DATA CONTRACT: These are `Error` severity in `DoubleBlind` profile, `Warning` in `Default`.

ACCEPTANCE:
- Body containing `"see github.com/alice/myrepo"` → `DoubleBlind` preflight returns `ok=false`.
- Body containing the primary author's name → `DoubleBlind` preflight returns `ok=false`.

---

### G22. Authors array model fix — `manifest.author` (string) vs `scientific_publication.authors[]` (array)

SEVERITY: HIGH  
EFFORT: 3 hours  
OWNER CRATE: vox-publisher  
VERIFIED: `crates/vox-publisher/src/publication.rs` — `PublicationManifest.author` is a `String`.
`crates/vox-publisher/src/scientific_metadata.rs` — `ScientificPublicationMetadata.authors` is
`Vec<ScientificAuthor>`. `crates/vox-publisher/src/publication_preflight.rs` lines 735–746:
there is an existing check `author_primary_mismatch` that compares `manifest.author` to
`scientific_publication.authors[0].name`. But Zenodo, Crossref, and OpenReview all need the
full authors array, not just the primary author string.

PROBLEM: Multi-author papers submitted to Zenodo or Crossref include only the primary author
(from `manifest.author`). Co-authors are silently dropped.

SOLUTION:  
This is NOT a breaking change to `PublicationManifest`. Instead:
1. In `zenodo_metadata.rs`, change `zenodo_deposition_create_body()` to:
   a. Parse `scientific_publication.authors[]` from `manifest.metadata_json`.
   b. If the array has ≥1 entry, use the full array for `metadata.creators`.
   c. Fall back to `manifest.author` only if the array is empty.

2. Add a new preflight check `scientific_authors_recommended`:
```rust
if sci.authors.is_empty() && profile != PreflightProfile::Default {
  findings.push(PreflightFinding {
    code: "scientific_authors_recommended",
    severity: PreflightSeverity::Warning,
    message: "scientific_publication.authors is empty; multi-author papers need the full array for venue submission."
  });
}
```

DATA CONTRACT: `ScientificAuthor.name` is `"First Last"` format. `ScientificAuthor.orcid` is
optional. `ScientificAuthor.affiliation` is optional. Zenodo maps:
`{ "name": "Last, First", "affiliation": "...", "orcid": "..." }`.
The name conversion `"First Last" → "Last, First"` is done at serialization time in `zenodo_metadata.rs`.

ACCEPTANCE:
- A manifest with 3 authors in `scientific_publication.authors` → Zenodo request JSON has 3 `creators`.
- A manifest with empty `scientific_publication.authors` → Zenodo request uses `manifest.author` as single creator.
- New preflight warning fires when authors array is empty and profile != Default.

training_eligible: false
archived_date: 2026-04-18
---

## 7. Wave 5: SSOT Hardening and CI Enforcement (1–2 weeks)

---

### G23. Rename/unify shadow SSOT — `voxgiantia-publication-architecture.md` may conflict

SEVERITY: MEDIUM  
EFFORT: 2 hours  
OWNER CRATE: docs  
VERIFIED: `grep -r "voxgiantia" docs/` — if the file exists, it is a shadow document not linked
from `research-index.md`. If it does not exist, this task is already resolved.

PROBLEM: A shadow SSOT with a misspelled name could contain divergent architecture decisions
that later implementers treat as canonical.

SOLUTION:  
Run `Get-ChildItem -Recurse docs/ | Where-Object { $_.Name -match "voxgiantia" }`.
If found: rename the file to the correct spelling, add a deprecation header:
```markdown
<!-- DEPRECATED: This document was renamed. See scientia-pipeline-ssot-2026.md. -->
```
If not found: close this task as resolved.

ACCEPTANCE:
- `rg "voxgiantia" docs/` returns 0 matches (no shadow doc remains).

training_eligible: false
archived_date: 2026-04-18
---

### G24. Add CI check: `vox ci scientia-heuristics-parity` (part of G6, expanded here)

SEVERITY: HIGH  
EFFORT: 4 hours  
OWNER CRATE: vox-ci or scripts  
VERIFIED: See G6 for code evidence. This task expands G6's Step 2 into a full specification.

Full parity check specification:
1. Load `contracts/scientia/impact-readership-projection.seed.v1.yaml`.
2. Load `contracts/scientia/finding-candidate.v1.schema.json`.
3. Compile `ScientiaHeuristics::default()` in a test binary.
4. For each numeric field in the YAML `heuristics.*` section:
   - Extract the value.
   - Find the matching field in `ScientiaHeuristics`.
   - Assert equality within 1e-9 tolerance for floats, exact for integers.
5. For each range in the JSON Schema (e.g., `minimum`, `maximum` on novelty thresholds):
   - Assert that `ScientiaHeuristics::default()` values fall within the declared range.
6. Exit 0 on all pass, exit 1 on first failure with a clear message:
   `PARITY FAIL: heuristics.novelty_overlap.high_threshold yaml=0.75 code=0.80`

The check runs as `cargo test -p vox-ci scientia_heuristics_parity_check --features parity_tests`.

ACCEPTANCE:
- Changing `novelty_high_threshold` in `ScientiaHeuristics::default()` from `0.75` to `0.80`
  without updating YAML causes the test to fail.

---

### G25. God Object split — extract `vox-scientia-core` from `vox-publisher`

SEVERITY: HIGH (long-term maintainability blocker)  
EFFORT: 16 hours  
OWNER CRATE: new `crates/vox-scientia-core`  
VERIFIED: `crates/vox-publisher/src/` — 28 files, ~40KB of source. Files prefixed `scientia_*`
are logically a separate subsystem but are not in a separate crate. This violates the God Object
Limit (500 lines or 12 methods per struct/class) and the Sprawl Limit (20 files per directory).
Current count: 28 files including non-scientia publisher logic.

PROBLEM: Any change to Scientia logic requires recompiling all of `vox-publisher`, including the
social syndication adapters. The crate has >20 files, exceeding the sprawl limit.

SOLUTION:  
Extract `crates/vox-scientia-core/` with:
```
src/
  lib.rs
  discovery.rs          (from scientia_discovery.rs)
  evidence.rs           (from scientia_evidence.rs)
  finding_ledger.rs     (from scientia_finding_ledger.rs)
  heuristics.rs         (from scientia_heuristics.rs)
  prior_art.rs          (from scientia_prior_art.rs)
  worthiness.rs         (from scientia_worthiness_enrich.rs + publication_worthiness.rs)
  contracts.rs          (from scientia_contracts.rs)
```

`vox-publisher` becomes a thin layer that `use vox_scientia_core::*` for the Scientia path.

**Move order** (to avoid circular imports):
1. Move `scientia_heuristics.rs` first (no publisher dependencies).
2. Move `scientia_contracts.rs`.
3. Move `scientia_evidence.rs` and `scientia_finding_ledger.rs` (depends on heuristics + contracts).
4. Move `scientia_discovery.rs` (depends on all above).
5. Update `vox-publisher/src/lib.rs` to re-export via `pub use vox_scientia_core::*`.

DATA CONTRACT: `vox-scientia-core` must NOT depend on `vox-publisher` (no circular imports).
It may depend on: `vox-db`, `vox-clavis`, `vox-bounded-fs`, `serde`, `serde_json`.

ACCEPTANCE:
- `cargo check -p vox-scientia-core` compiles independently.
- `cargo check -p vox-publisher` still compiles with the re-exports.
- `crates/vox-publisher/src/` has ≤ 20 files after the move.

training_eligible: false
archived_date: 2026-04-18
---

## 8. Wave 6: Quality, Evaluation, and Autonomy (2–4 weeks)

---

### G26. Implement golden test set for search recall

SEVERITY: HIGH  
EFFORT: 8 hours  
OWNER CRATE: vox-search, tests/  
VERIFIED: `crates/vox-search/src/evaluation.rs` exists but is 1789 bytes — it defines structs
but no test fixtures. `crates/vox-db/src/research_eval_runs.rs` (implied by `research.rs` — see
`record_research_eval_run()`) exists. No golden query set exists in `contracts/` or `tests/`.

PROBLEM: There is no way to verify that a change to `SearchPolicy` or `run_search_with_verification()`
has not degraded recall quality. Every tuning change is a leap of faith.

SOLUTION:  
Create `contracts/scientia/search-golden-set.v1.json`:
```json
{
  "version": 1,
  "queries": [
    {
      "id": "q001",
      "query": "what is the Socrates confidence gate threshold",
      "expected_corpus": "knowledge",
      "expected_code_refs": ["vox_socrates_policy"],
      "min_recall_at_5": 0.8
    }
  ]
}
```

Create `tests/scientia_search_recall_test.rs` (integration test, feature-gated on `local`):
```rust
#[test]
fn golden_set_recall_above_threshold() {
  let db = VoxDb::connect(DbConfig::Memory).unwrap();
  // Seed DB with golden documents
  // Run each query
  // Assert recall_at_5 >= min_recall_at_5
}
```

The test runner calls `db.record_research_eval_run()` to persist results for trend tracking.

DATA CONTRACT: `contracts/scientia/search-golden-set.v1.json` is the SSOT for the golden set.
Add queries incrementally; never remove existing queries without a deprecation period.

ACCEPTANCE:
- `cargo test --test scientia_search_recall_test --features local` passes on a seeded in-memory DB.
- A deliberately broken `SearchPolicy` (e.g., `tavily_enabled = false`, all corpora emptied) causes at least one golden query to fail.

training_eligible: false
archived_date: 2026-04-18
---

### G27. Implement RAGAS-style faithfulness metric for Scientia evidence

SEVERITY: MEDIUM  
EFFORT: 10 hours  
OWNER CRATE: vox-db, new `vox-scientia-eval`  
VERIFIED: `crates/vox-db/src/research_metrics_contract.rs` has `METRIC_TYPE_MEMORY_HYBRID_FUSION`
and `METRIC_TYPE_SOCRATES_SURFACE` but no faithfulness metric type. `crates/vox-db/src/rag_evidence.rs`
exists (9148 bytes) and defines `RagEvidenceRow` but does not compute a faithfulness score.

PROBLEM: There is no automated measure of whether a Scientia draft's claims are grounded in the
evidence attached to its `ScientiaEvidenceContext`. A claim in the body could contradict the
benchmark data without any detector catching it.

SOLUTION:  
Create `METRIC_TYPE_SCIENTIA_FAITHFULNESS: &str = "scientia_faithfulness"` in
`research_metrics_contract.rs`.

Create `crates/vox-scientia-eval/src/faithfulness.rs`:
```rust
/// Compute a faithfulness score: what fraction of checkable claims in the body
/// are grounded in the attached DiscoverySignals and prior-art hits?
/// 
/// Algorithm:
/// 1. Extract factual claims from body_markdown (sentences containing numbers,
///    percentages, or comparison language: "outperforms", "achieves", "beats").
/// 2. For each claim, check if any DiscoverySignal.summary or PriorArtHit.abstract
///    contains a supporting substring (simple BM25-style keyword overlap, not LLM).
/// 3. faithfulness = grounded_claims / total_claims (clamped to [0, 1]).
pub fn score_faithfulness(
  body_markdown: &str,
  signals: &[DiscoverySignal],
  prior_art_hits: &[PriorArtHit],
) -> FaithfulnessReport;

pub struct FaithfulnessReport {
  pub score: f64,
  pub total_claims: usize,
  pub grounded_claims: usize,
  pub ungrounded_claim_snippets: Vec<String>,
}
```

Write faithful score to `research_metrics` via `append_research_metric(...)`.

DATA CONTRACT: This metric is **assistive only** — it never blocks submission. Add it to
`PreflightReport.worthiness` as an optional field: `faithfulness_score: Option<f64>`.

ACCEPTANCE:
- A body with 5 numeric claims all backed by signals scores 1.0.
- A body with 5 numeric claims, 0 backed, scores 0.0.
- `vox stub-check --path crates/vox-scientia-eval/src/faithfulness.rs` passes.

---

### G28. arXiv format preflight — validate submission bundle layout

SEVERITY: HIGH  
EFFORT: 5 hours  
OWNER CRATE: vox-publisher  
VERIFIED: `crates/vox-publisher/src/publication_preflight.rs` — `PreflightProfile::ArxivAssist`
exists in the enum (line 21) but the `run_preflight_with_attention()` function has no
`ArxivAssist`-specific checks. The profile is accepted as input but ignored in logic.

PROBLEM: Selecting the `ArxivAssist` profile currently gives the same checks as `Default`.
An operator generating an arXiv submission bundle gets no feedback on whether it is compliant.

SOLUTION:  
Add an `ArxivAssist` section to the preflight logic:
```rust
if profile == PreflightProfile::ArxivAssist {
  // 1. Abstract presence (arXiv requires explicit abstract, not inferred from body)
  let has_abstract = parse_scientific_from_metadata_json(manifest.metadata_json.as_deref())
    .ok().flatten()
    .and_then(|s| s.abstract_text)
    .is_some_and(|a| !a.trim().is_empty());
  if !has_abstract {
    findings.push(error("arxiv_abstract_required", "arXiv submissions require an explicit abstract in scientific_publication.abstract_text"));
  }
  
  // 2. Primary category (required by arXiv)
  let has_category = parse_scientific_from_metadata_json(...)
    .ok().flatten()
    .and_then(|s| s.arxiv_primary_category)
    .is_some_and(|c| !c.trim().is_empty());
  if !has_category {
    findings.push(warning("arxiv_category_recommended", "Set scientific_publication.arxiv_primary_category (e.g. cs.AI)"));
  }
  
  // 3. Staging directory existence (VOX_ARXIV_STAGING_DIR)
  let staging_exists = std::env::var("VOX_ARXIV_STAGING_DIR")
    .ok()
    .is_some_and(|d| std::path::Path::new(&d).is_dir());
  if !staging_exists {
    findings.push(warning("arxiv_staging_dir_missing", "Set VOX_ARXIV_STAGING_DIR to the latex package root for arXiv assist"));
  }
}
```

Add `arxiv_primary_category: Option<String>` to `ScientificPublicationMetadata`.
Add `abstract_text: Option<String>` to `ScientificPublicationMetadata` (if not already present — verify).

DATA CONTRACT: `arxiv_primary_category` must be a valid arXiv category string (e.g., `"cs.AI"`, `"stat.ML"`).
Validate format: `^[a-z]+\.[A-Z]{1,4}$` and emit a warning if it doesn't match.

ACCEPTANCE:
- `run_preflight(manifest_with_no_abstract, ArxivAssist)` → `ok=false`, contains `"arxiv_abstract_required"`.
- `run_preflight(manifest_with_abstract_and_category, ArxivAssist)` → no errors from the arxiv-specific checks.

training_eligible: false
archived_date: 2026-04-18
---

## 9. Unified Environment Variable Registry

All environment variables used by the Scientia pipeline. This is the canonical list.
Do not introduce new `std::env::var()` calls for Scientia logic without adding them here.

| Variable | Crate | Default | Purpose |
|---|---|---|---|
| `VOX_SEARCH_TAVILY_ENABLED` | vox-search | `false` | Enable CRAG Tavily fallback |
| `VOX_SEARCH_TAVILY_DEPTH` | vox-search | `basic` | `basic` or `advanced` |
| `VOX_SEARCH_TAVILY_MAX_RESULTS` | vox-search | `5` | Max Tavily results per call |
| `VOX_SEARCH_TAVILY_ON_EMPTY` | vox-search | `true` | Auto-fire on empty local corpora |
| `VOX_SEARCH_TAVILY_ON_WEAK` | vox-search | `false` | Auto-fire on weak evidence quality |
| `VOX_SEARCH_TAVILY_BUDGET` | vox-search | `50` | Max Tavily calls per session |
| `VOX_SEARCH_CRAG_CACHE_TTL_MS` | vox-search | `3600000` | TTL for cached CRAG results in DB |
| `VOX_SEARCH_CRAG_SIGNAL_PROMOTE_THRESHOLD` | vox-search | `0.70` | Min Tavily score to create InboundSignal |
| `VOX_SOCRATES_RESEARCH_CONFIDENCE_CEILING` | vox-socrates-policy | `0.40` | Max confidence for CRAG trigger |
| `VOX_SOCRATES_RESEARCH_EVIDENCE_CEILING` | vox-socrates-policy | `0.50` | Max evidence quality for CRAG trigger |
| `VOX_SCIENTIA_INGEST_POLL_INTERVAL_SECS` | vox-scientia-ingest | `86400` | Default poll interval for feed sources |
| `VOX_MENS_LANE_G_OUTPUT_DIR` | vox-orchestrator | *(unset)* | Directory for Lane G training examples |
| `VOX_ZENODO_HTTP_MAX_ATTEMPTS` | vox-publisher/scholarly | `3` | Zenodo HTTP retry limit |
| `VOX_ZENODO_STAGING_DIR` | vox-publisher/scholarly | *(unset)* | Root of zenodo staging export |
| `VOX_ZENODO_REQUIRE_METADATA_PARITY` | vox-publisher/scholarly | `false` | Enforce title parity check |
| `VOX_ZENODO_VERIFY_STAGING_CHECKSUMS` | vox-publisher/scholarly | `false` | Verify sha3-256 on upload |
| `VOX_ZENODO_DRAFT_ONLY` | vox-publisher/scholarly | `false` | Never publish (stay as draft) |
| `VOX_SCHOLARLY_ADAPTER` | vox-publisher/scholarly | *(unset)* | Override default adapter selection |
| `VOX_SCHOLARLY_DISABLE_ZENODO` | vox-publisher/scholarly | `false` | Disable Zenodo adapter |
| `VOX_ARXIV_STAGING_DIR` | vox-publisher/preflight | *(unset)* | Root of arXiv staging directory |
| `VOX_SCHOLARLY_ENABLE_CROSSREF` | vox-publisher/scholarly | `false` | Enable Crossref deposit |

---

## 10. Clavis Secret Registry

All secrets consumed by the Scientia pipeline. Add to `vox-clavis/src/spec.rs` if missing.

| SecretId | Env alias (fallback) | Purpose |
|---|---|---|
| `TavilyApiKey` | `TAVILY_API_KEY` | CRAG web search |
| `VoxZenodoAccessToken` | `ZENODO_ACCESS_TOKEN` | Zenodo deposit |
| `VoxOpenReviewAccessToken` | `VOX_OPENREVIEW_ACCESS_TOKEN` | OpenReview submit |
| `VoxOpenReviewEmail` | `VOX_OPENREVIEW_EMAIL` | OpenReview login |
| `VoxOpenReviewPassword` | `VOX_OPENREVIEW_PASSWORD` | OpenReview login |
| `VoxCrossrefUsername` [NEW] | `VOX_CROSSREF_USERNAME` | Crossref deposit (G19) |
| `VoxCrossrefPassword` [NEW] | `VOX_CROSSREF_PASSWORD` | Crossref deposit (G19) |
| `VoxScientiaRedditClientId` [NEW] | `VOX_SCIENTIA_REDDIT_CLIENT_ID` | Reddit inbound (G10) |
| `VoxScientiaRedditClientSecret` [NEW] | `VOX_SCIENTIA_REDDIT_CLIENT_SECRET` | Reddit inbound (G10) |
| `VoxArxivApiKey` [NEW] | `VOX_ARXIV_API_KEY` | arXiv inbound (G10, optional) |

After adding any new `SecretId`, run: `vox ci secret-env-guard` and `vox ci clavis-parity`.

training_eligible: false
archived_date: 2026-04-18
---

## 11. DB Schema Additive Changes Summary

All changes are `ADD COLUMN` or `CREATE TABLE` — safe for `VoxDb::auto_migrate()`.

| Table | Change | Task |
|---|---|---|
| (new) `scientia_feed_sources` | CREATE TABLE | G7 |
| (new) `scientia_inbound_signals` | CREATE TABLE | G8 |
| `publish_cloud` | ADD COLUMN `revision_history_json TEXT DEFAULT '[]'` | G5 |
| `publish_cloud` | ADD COLUMN `reflected_to_rag INTEGER DEFAULT 0` | G14 |
| `publish_cloud` | ADD COLUMN `status_synced_at_ms INTEGER DEFAULT 0` | G20 |
| `knowledge_nodes` | No schema change — new `node_type` values only | G13, G14, G15 |

---

## 12. Task Execution Order (For LLM Implementation Agent)

Execute tasks in this exact order. Each group can proceed in parallel within the group,
but the group boundary is a hard dependency.

**Group A — Must complete first (no prerequisites):**
- G1, G2, G3, G6 (independent bug fixes)

**Group B — Requires Group A:**
- G4 (requires G1), G5 (no dependency but write last to avoid schema noise)

**Group C — New DB tables (no code dependencies):**
- G7, G8 (CREATE TABLE tasks — can run immediately after DB is accessible)

**Group D — Inbound pipeline (requires Group C and Group A):**
- G9 (requires G7, G8), G10 (requires G9), G11 (requires G9)

**Group E — Feedback loop (requires Group A and Group D):**
- G12 (requires G3), G13 (requires G3), G14 (requires G8, G13), G15 (requires G12, G13)

**Group F — Advanced features (requires Group E):**
- G16 (requires G9, G15), G17, G18 (requires G16)

**Group G — Outbound hardening (requires Group A):**
- G19 (requires G6), G20 (requires G5, G19), G21, G22

**Group H — SSOT and CI (requires Group A):**
- G23, G24 (requires G6), G25 (requires all Group A+B)

**Group I — Quality and evaluation (no hard dependencies, can run in parallel with F+G):**
- G26, G27, G28

training_eligible: false
archived_date: 2026-04-18
---

## 13. Verification Ritual

Before marking any task complete, run in order:

1. `vox stub-check --path <changed-dir>` — must return 0 TOESTUB violations.
2. `cargo check -p <changed-crate>` — must compile.
3. `cargo test -p <changed-crate>` — all unit tests must pass.
4. `vox ci scientia-heuristics-parity` (after any G6 work) — must exit 0.
5. `vox ci scientia-novelty-ledger-contracts` — must exit 0.
6. For DB schema changes: `vox db auto-migrate --dry-run` — must report only `CREATE TABLE` or `ADD COLUMN` actions (no DROP).




