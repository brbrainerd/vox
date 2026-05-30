# Effort Route (S2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a new `vox-effort-route` crate plus `vox audit effort-route` CLI subcommand that reads S1's `findings.jsonl`, groups findings (deterministic enum-bucket + conditional embedding sub-cluster), re-judges each cluster with adversarial verification through the model-agnostic facade, and emits ranked `recommendations.jsonl` + `recommendations.md` + staging-dir `.proposed` draft enforcement artifacts.

**Architecture:** New L2 crate consuming S1's versioned file contract (`schema_version="1.0"`). Drafted fixes take whatever enforcement form the repo uses (`ArtifactForm` enum), with Vox gated behind model capability passed in by the CLI. All LLM/embedding I/O goes through `vox_actor_runtime::llm` (`infer_with_retry`, `llm_embed`); MENS first-class. Output JSONL `schema_version="1.0"` is the contract S4 consumes.

**Tech Stack:** Rust 2024, `serde`/`serde_json`, `tokio`, `futures` (`FuturesUnordered` + `Semaphore`), `gix 0.70` (re-read diffs), `insta` (snapshots), `tempfile` (fixtures), `vox-effort-audit` (shared types), `vox-actor-runtime` (LLM + embed), `vox-config`, `vox-telemetry`. **No `vox-search` dep** (uses `llm_embed` directly).

**Spec:** [`docs/superpowers/specs/2026-05-30-effort-route-design.md`](../specs/2026-05-30-effort-route-design.md).

**Branch / worktree:** `spec/effort-route` in `.worktrees/effort-route-spec/` (based on S1 tip; rebase onto `main` after S1's PR #95 merges).

**Verified S1 surface (import these, do not redefine):**
- `vox_effort_audit::judge::schema::{WasteCategory, RemediationKind, JudgeFinding, SCHEMA_VERSION}` — `RemediationKind` variants: `ScriptAutomation, AgentsMdRule, LinterRule, CorpusNegativeExample, NoneNeeded, Unknown`.
- `vox_effort_audit::output::{FindingRow, JudgeMeta}` — `FindingRow.finding: Option<JudgeFinding>`, `FindingRow.shape: ShapeFeatures`, `FindingRow.cost: MeasuredCost`.
- `vox_effort_audit::hybrid::MeasuredCost` — variants `Measured{input_tokens,output_tokens,source,session_id}`, `Estimated{input_tokens,output_tokens}`, `Ambiguous`, `Unavailable`. **No `estimated_usd` field.**
- `vox_effort_audit::shape::ShapeFeatures` — has `file_extension_histogram: HashMap<String,u32>`.
- Embedding: `vox_actor_runtime::llm::llm_embed(options: &ActivityOptions, text: &str, config: LlmConfig) -> ActivityResult<Result<Vec<f32>, String>>`.
- LLM: `vox_actor_runtime::llm::{infer_with_retry, LlmConfig, LlmChatMessage}` (same as S1 B4 used).

**Pre-flight (once before Task 1):**
```bash
git -C /c/Users/Owner/vox/.worktrees/effort-route-spec status        # clean
git -C /c/Users/Owner/vox/.worktrees/effort-route-spec log --oneline -1   # 8e536c8a08 spec correction
cargo check -p vox-effort-audit                                      # S1 compiles (dependency)
```

---

## Phase A — Foundations (no LLM)

### Task A1: Scaffold the crate

**Files:**
- Create: `crates/vox-effort-route/Cargo.toml`
- Create: `crates/vox-effort-route/src/lib.rs`
- Create: stub modules

- [ ] **Step 1: Verify the crate is not yet recognized**

Run: `cargo check -p vox-effort-route`
Expected: FAIL — `did not match any packages`.

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name        = "vox-effort-route"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Routes effort-audit findings to verified, drafted enforcement artifacts"

[dependencies]
vox-effort-audit  = { path = "../vox-effort-audit" }
vox-actor-runtime = { path = "../vox-actor-runtime" }
vox-config        = { path = "../vox-config" }
vox-telemetry     = { path = "../vox-telemetry" }
gix               = { workspace = true }
serde             = { workspace = true, features = ["derive"] }
serde_json        = { workspace = true }
chrono            = { workspace = true }
uuid              = { workspace = true, features = ["v4"] }
tracing           = { workspace = true }
tokio             = { workspace = true, features = ["macros", "rt-multi-thread", "sync"] }
futures           = { workspace = true }
thiserror         = { workspace = true }
async-trait       = { workspace = true }

[dev-dependencies]
insta             = { workspace = true, features = ["json"] }
tempfile          = { workspace = true }
tokio             = { workspace = true, features = ["macros", "rt-multi-thread", "test-util"] }
```

If any workspace dep is missing from root `[workspace.dependencies]`, look up the real pin from an existing consumer — do not invent.

- [ ] **Step 3: Create lib.rs**

```rust
//! Routes effort-audit findings to verified, drafted enforcement artifacts.
//!
//! See `docs/superpowers/specs/2026-05-30-effort-route-design.md`.

pub mod config;
pub mod load;
pub mod bucket;
pub mod cluster;
pub mod route;
pub mod emit;
pub mod pipeline;

// `pub use pipeline::run;` is added in the pipeline task (E-phase) once
// `pipeline::run` exists; adding it against a stub causes E0432.
```

- [ ] **Step 4: Create transient stub modules**

For each of {config, load, bucket, cluster, pipeline} create `src/<name>.rs` containing only `//! Stub; see plan.\n`. For each of {route, emit} create `src/<name>/mod.rs` containing only `//! Stub; see plan.\n`. No `pub fn` — the TDD guard must not fire.

- [ ] **Step 5: Verify glob inclusion + green check**

Run: `cargo check -p vox-effort-route`
Expected: PASS (workspace `members = ["crates/*",...]` glob auto-includes it; zero warnings attributable to the new crate).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-effort-route Cargo.lock
git commit -m "feat(vox-effort-route): scaffold L2 crate (A1)"
```

---

### Task A2: Architectural registration

**Files:**
- Modify: `docs/src/architecture/layers.toml`
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Add layers.toml row**

In the `[crates]` section, L2 group, alphabetical position (note: this repo uses inline-row style, NOT `[crates.<name>]` table headers — match the existing rows):

```toml
vox-effort-route = { layer = 2, kind = "library", max_loc = 4_000 }
```

- [ ] **Step 2: Add where-things-live row**

Match the existing 2-column table format (Crate | scope). Insert alphabetically:

```markdown
| `vox-effort-route` | Routes effort-audit findings to verified, drafted enforcement artifacts (AGENTS.md rule / lint detector spec / arch rule / CI gate / corpus example / Vox script). CLI: `vox audit effort-route`. |
```

- [ ] **Step 3: Run arch-check**

Run: `cargo run -q -p vox-arch-check`
Expected: PASS for `vox-effort-route` (an unrelated pre-existing `vox-cli-tests` finding may persist — verify it predates this task with `git stash` if unsure).

- [ ] **Step 4: Commit**

```bash
git add docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register vox-effort-route in layers + WTL (A2)"
```

---

### Task A3: `config.rs`

**Files:**
- Replace stub: `crates/vox-effort-route/src/config.rs`

- [ ] **Step 1: Write the failing test + impl**

```rust
//! Configuration for `vox audit effort-route`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffortRouteConfig {
    #[serde(default = "default_min_waste_score")]
    pub min_waste_score: u8,
    #[serde(default = "default_max_bucket_size")]
    pub max_bucket_size: usize,
    #[serde(default = "default_max_context_commits")]
    pub max_context_commits: usize,
    #[serde(default = "default_staging_dir")]
    pub staging_dir: PathBuf,
    #[serde(default)]
    pub judge: RouteJudgeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteJudgeConfig {
    pub model_preference: Option<String>,
    #[serde(default = "default_max_total_tokens")]
    pub max_total_tokens: u64,
    #[serde(default = "default_max_dollar_cost")]
    pub max_dollar_cost: f64,
    #[serde(default = "default_verify")]
    pub verify: bool,
}

fn default_min_waste_score() -> u8 { 4 }
fn default_max_bucket_size() -> usize { 20 }
fn default_max_context_commits() -> usize { 6 }
fn default_staging_dir() -> PathBuf { PathBuf::from("target/audit/effort-route") }
fn default_max_total_tokens() -> u64 { 5_000_000 }
fn default_max_dollar_cost() -> f64 { 5.00 }
fn default_verify() -> bool { true }

impl Default for RouteJudgeConfig {
    fn default() -> Self {
        Self {
            model_preference: None,
            max_total_tokens: default_max_total_tokens(),
            max_dollar_cost: default_max_dollar_cost(),
            verify: default_verify(),
        }
    }
}

impl Default for EffortRouteConfig {
    fn default() -> Self {
        Self {
            min_waste_score: default_min_waste_score(),
            max_bucket_size: default_max_bucket_size(),
            max_context_commits: default_max_context_commits(),
            staging_dir: default_staging_dir(),
            judge: RouteJudgeConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = EffortRouteConfig::default();
        assert_eq!(c.min_waste_score, 4);
        assert_eq!(c.max_bucket_size, 20);
        assert_eq!(c.max_context_commits, 6);
        assert!(c.judge.verify);
        assert_eq!(c.judge.max_total_tokens, 5_000_000);
    }

    #[test]
    fn partial_toml_inherits_defaults() {
        let c: EffortRouteConfig = toml::from_str(r#"
            min_waste_score = 6
            [judge]
            verify = false
        "#).unwrap();
        assert_eq!(c.min_waste_score, 6);
        assert!(!c.judge.verify);
        assert_eq!(c.max_bucket_size, 20);
    }
}
```

Add `toml` to `[dev-dependencies]` (check workspace pin first).

- [ ] **Step 2: Run tests**

Run: `cargo test -p vox-effort-route config::tests`
Expected: PASS (2 tests).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-effort-route
git commit -m "feat(vox-effort-route): config + TOML schema (A3)"
```

---

### Task A4: `load.rs`

**Files:**
- Replace stub: `crates/vox-effort-route/src/load.rs`
- Create: `crates/vox-effort-route/tests/fixtures/findings.jsonl`

- [ ] **Step 1: Create fixture**

Write `tests/fixtures/findings.jsonl` with 4 lines covering the cases (use real `FindingRow` JSON shape — match S1's serialization exactly; generate one by reading `vox_effort_audit::output::FindingRow` fields):
- A `MechanicalSweep`/`ScriptAutomation` finding with `waste_score: 8`, `primary_crate`-derivable evidence pointer `crates/vox-config/src/timeouts.rs:8`
- A `LegitBugfix` finding with `waste_score: 3` (below default threshold → filtered)
- A row with `finding: null` (Skipped commit → filtered)
- A `LinterGap`/`LinterRule` finding with `waste_score: 7`

Each line MUST include `"schema_version":"1.0"` and all required `FindingRow` fields. Author hash is a 64-char hex placeholder.

- [ ] **Step 2: Write failing test + impl**

```rust
//! Load + validate + filter S1's findings.jsonl.

use vox_effort_audit::output::FindingRow;
use vox_effort_audit::judge::schema::SCHEMA_VERSION;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("read failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("json parse failed at line {line}: {source}")]
    Parse { line: usize, source: serde_json::Error },
    #[error("schema_version mismatch: found {found:?}, expected {expected:?}")]
    SchemaMismatch { found: String, expected: String },
}

/// A finding that survived filtering (guaranteed `finding.is_some()` and score >= threshold).
#[derive(Debug, Clone)]
pub struct LoadedFinding {
    pub row: FindingRow,
}

/// Parse, schema-validate, and filter findings.jsonl.
pub fn read(path: &Path, min_waste_score: u8) -> Result<Vec<LoadedFinding>, LoadError> {
    let body = std::fs::read_to_string(path)?;
    let mut out = Vec::new();
    for (i, line) in body.lines().enumerate() {
        if line.trim().is_empty() { continue; }
        let row: FindingRow = serde_json::from_str(line)
            .map_err(|source| LoadError::Parse { line: i + 1, source })?;
        if row.schema_version != SCHEMA_VERSION {
            return Err(LoadError::SchemaMismatch {
                found: row.schema_version.clone(),
                expected: SCHEMA_VERSION.to_string(),
            });
        }
        match &row.finding {
            Some(f) if f.waste_score >= min_waste_score => out.push(LoadedFinding { row }),
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/findings.jsonl")
    }

    #[test]
    fn filters_null_and_low_score() {
        let v = read(&fixture(), 4).unwrap();
        // 4 fixture rows; 1 null-finding + 1 low-score dropped → 2 remain.
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|f| f.row.finding.as_ref().unwrap().waste_score >= 4));
    }

    #[test]
    fn schema_mismatch_aborts() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"{"schema_version":"9.9","commit_sha":"x","parent_sha":null,"commit_ts":"2026-05-28T00:00:00Z","author_email_sha256":"0","branch_hint":"main","message_first_line":"m","shape":{"additions":0,"deletions":0,"files_changed":0,"file_extension_histogram":{},"mechanical_sweep_score":0.0,"is_lockfile_only":false,"is_generated_only":false,"is_doc_only":false,"commit_kind_from_message":"other"},"cost":{"kind":"Unavailable"},"judge":{"model_id":"m","latency_ms":0,"judge_input_tokens":0,"judge_output_tokens":0,"outcome":"Judged"},"finding":null}"#).unwrap();
        assert!(matches!(read(tmp.path(), 4), Err(LoadError::SchemaMismatch { .. })));
    }
}
```

NOTE: the `shape` JSON in the mismatch test must match `ShapeFeatures`' actual serde shape. Before writing, run `cargo test -p vox-effort-audit shape::` and inspect `ShapeFeatures` serialization, or serialize a default instance to confirm field names (`commit_kind_from_message` serde rename, etc.). Adjust the JSON literal to match.

- [ ] **Step 3: Run tests**

Run: `cargo test -p vox-effort-route load::tests`
Expected: PASS (2 tests). If the fixture JSON shape is wrong, the parse error will tell you which field — fix the fixture, not the loader.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-effort-route
git commit -m "feat(vox-effort-route): load + schema-validate + filter findings (A4)"
```

---

### Task A5: `bucket.rs`

**Files:**
- Replace stub: `crates/vox-effort-route/src/bucket.rs`

- [ ] **Step 1: Write failing test + impl**

```rust
//! Deterministic grouping of findings by the structural fix that prevents them.

use crate::load::LoadedFinding;
use vox_effort_audit::judge::schema::{RemediationKind, WasteCategory};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BucketKey {
    pub waste_category: String,   // Debug-formatted enum (stable, sortable)
    pub remediation_kind: String,
    pub primary_crate: String,
}

#[derive(Debug, Clone)]
pub struct Bucket {
    pub key: BucketKey,
    pub members: Vec<LoadedFinding>,
}

/// Derive the owning crate from a finding's evidence pointers (preferred) or
/// shape histogram (fallback). Returns "<workspace-root>" when no crate path found.
pub fn primary_crate(f: &LoadedFinding) -> String {
    let finding = f.row.finding.as_ref();
    if let Some(finding) = finding {
        for ptr in &finding.evidence_pointers {
            if let Some(c) = crate_from_path(ptr) { return c; }
        }
    }
    // Fallback: nothing usable.
    "<workspace-root>".to_string()
}

fn crate_from_path(path: &str) -> Option<String> {
    // "crates/<name>/..." → "<name>"
    let path = path.split(':').next().unwrap_or(path); // strip ":line"
    let mut parts = path.split('/');
    while let Some(p) = parts.next() {
        if p == "crates" {
            if let Some(name) = parts.next() {
                return Some(name.to_string());
            }
        }
    }
    None
}

pub fn group(findings: Vec<LoadedFinding>) -> Vec<Bucket> {
    let mut map: BTreeMap<BucketKey, Vec<LoadedFinding>> = BTreeMap::new();
    for f in findings {
        let finding = f.row.finding.as_ref().expect("filtered to Some");
        let key = BucketKey {
            waste_category: format!("{:?}", finding.waste_category),
            remediation_kind: format!("{:?}", finding.suggested_remediation_kind),
            primary_crate: primary_crate(&f),
        };
        map.entry(key).or_default().push(f);
    }
    map.into_iter().map(|(key, members)| Bucket { key, members }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    // Build LoadedFinding fixtures inline by constructing FindingRow.
    // (Helper mirrors the load fixture; keep it local to this test module.)
    // ... construct 3 findings: 2 with identical (cat,kind,crate), 1 different ...

    #[test]
    fn crate_from_evidence_pointer() {
        assert_eq!(crate_from_path("crates/vox-config/src/timeouts.rs:8"), Some("vox-config".into()));
        assert_eq!(crate_from_path("README.md"), None);
    }

    #[test]
    fn identical_keys_join_one_bucket() {
        // Construct two findings with the same waste_category + remediation_kind
        // + evidence pointer crate, assert group() yields 1 bucket of 2 members.
        // (Full FindingRow construction shown in the load fixture helper.)
    }
}
```

The two `#[test]` stubs marked `...` MUST be filled with real `FindingRow` construction (copy the builder pattern from the load fixture). Do not commit an empty test body.

- [ ] **Step 2: Run + commit**

Run: `cargo test -p vox-effort-route bucket::tests` → PASS.
```bash
git add crates/vox-effort-route
git commit -m "feat(vox-effort-route): deterministic bucketing (A5)"
```

---

### Task A6: `cluster.rs` — conditional embedding sub-cluster

**Files:**
- Replace stub: `crates/vox-effort-route/src/cluster.rs`

- [ ] **Step 1: Write failing test + impl (with Embedder trait + MockEmbedder)**

```rust
//! Conditional embedding sub-cluster: only buckets over the size threshold split.

use crate::bucket::Bucket;
use async_trait::async_trait;

/// Abstraction over embedding so tests can assert call counts and determinism.
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// A sub-cluster of a bucket (or the whole bucket when not split).
#[derive(Debug, Clone)]
pub struct Cluster {
    pub key_suffix: String,        // "" for unsplit, "-0"/"-1"/... for split
    pub bucket: Bucket,
}

/// Split oversized buckets into sub-clusters; pass small buckets through unchanged.
pub async fn maybe_split(
    buckets: Vec<Bucket>,
    max_bucket_size: usize,
    embedder: &dyn Embedder,
) -> Vec<Cluster> {
    let mut out = Vec::new();
    for b in buckets {
        if b.members.len() <= max_bucket_size {
            out.push(Cluster { key_suffix: String::new(), bucket: b });
            continue;
        }
        // Embed each member's rationale; agglomerative cosine cluster.
        // NB: use an `embed_failed` flag, NOT an early `out.push(... b)` inside
        // the loop — moving `b` there makes `split_by_labels(b, ...)` below a
        // use-after-move (E0382). Push the unsplit bucket after the loop instead.
        let mut vectors = Vec::with_capacity(b.members.len());
        let mut embed_failed = false;
        for m in &b.members {
            let text = m.row.finding.as_ref().map(|f| f.rationale_one_line.clone()).unwrap_or_default();
            match embedder.embed(&text).await {
                Ok(v) => vectors.push(v),
                Err(_) => { embed_failed = true; break; } // embedding failure → don't split
            }
        }
        if embed_failed {
            out.push(Cluster { key_suffix: String::new(), bucket: b });
            continue;
        }
        let labels = agglomerative_cosine(&vectors, 0.30); // distance threshold
        out.extend(split_by_labels(b, &labels));
    }
    out
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { return 1.0; }
    1.0 - (dot / (na * nb))
}

/// Single-linkage agglomerative clustering: assign cluster ids by union-find
/// over pairs within `threshold` cosine distance.
fn agglomerative_cosine(vectors: &[Vec<f32>], threshold: f32) -> Vec<usize> {
    let n = vectors.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, x: usize) -> usize {
        if p[x] != x { let r = find(p, p[x]); p[x] = r; }
        p[x]
    }
    for i in 0..n {
        for j in (i + 1)..n {
            if cosine_distance(&vectors[i], &vectors[j]) <= threshold {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj { parent[ri] = rj; }
            }
        }
    }
    // Normalize roots to dense 0..k labels.
    let mut label_of = std::collections::HashMap::new();
    let mut next = 0usize;
    (0..n).map(|i| {
        let r = find(&mut parent, i);
        *label_of.entry(r).or_insert_with(|| { let l = next; next += 1; l })
    }).collect()
}

fn split_by_labels(b: Bucket, labels: &[usize]) -> Vec<Cluster> {
    let mut groups: std::collections::BTreeMap<usize, Vec<_>> = std::collections::BTreeMap::new();
    for (m, &l) in b.members.iter().zip(labels) {
        groups.entry(l).or_default().push(m.clone());
    }
    groups.into_iter().map(|(l, members)| Cluster {
        key_suffix: format!("-{l}"),
        bucket: Bucket { key: b.key.clone(), members },
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingMock { calls: AtomicUsize }
    #[async_trait]
    impl Embedder for CountingMock {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![1.0, 0.0, 0.0])
        }
    }

    #[tokio::test]
    async fn small_bucket_does_not_embed() {
        // Construct a bucket of 3 members (<= 20). Assert 0 embed calls.
        // (Construct Bucket with the FindingRow builder.)
    }

    #[tokio::test]
    async fn cosine_distance_basics() {
        assert!(cosine_distance(&[1.0,0.0], &[1.0,0.0]) < 1e-6);
        assert!((cosine_distance(&[1.0,0.0], &[0.0,1.0]) - 1.0).abs() < 1e-6);
    }
}
```

Fill the `small_bucket_does_not_embed` test body with a real 3-member bucket and assert `mock.calls.load(Ordering::SeqCst) == 0`. Add an `oversized splits` test if time permits (construct 25 members, assert >1 cluster when vectors differ).

- [ ] **Step 2: Run + commit**

Run: `cargo test -p vox-effort-route cluster::tests` → PASS.
```bash
git add crates/vox-effort-route
git commit -m "feat(vox-effort-route): conditional embedding sub-cluster (A6)"
```

---

## Phase B — Routing (re-judge + verify)

### Task B1: `route/mod.rs` — types + ArtifactForm + Router trait + MockRouter

**Files:**
- Replace stub: `crates/vox-effort-route/src/route/mod.rs`

- [ ] **Step 1: Write the types + trait + MockRouter + test**

```rust
//! Cluster re-judge and adversarial verification.

pub mod decide;
pub mod verify;
pub mod prompt;

use crate::cluster::Cluster;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ArtifactForm {
    AgentsMdRule,
    CodeAuditDetector,
    ArchRule,
    CiGate,
    VoxScript,
    CorpusNegativeExample,
    None,
}

impl ArtifactForm {
    /// Staging-file extension for this form (always ends in `.proposed`).
    pub fn staging_extension(self) -> &'static str {
        match self {
            ArtifactForm::AgentsMdRule          => "agents-rule.md.proposed",
            ArtifactForm::CodeAuditDetector     => "detector.md.proposed",
            ArtifactForm::ArchRule              => "arch-rule.toml.proposed",
            ArtifactForm::CiGate                => "ci.yaml.proposed",
            ArtifactForm::VoxScript             => "vox.proposed",
            ArtifactForm::CorpusNegativeExample => "corpus.jsonl.proposed",
            ArtifactForm::None                  => "",
        }
    }
    /// Forms allowed when the authoring model is not Vox-capable.
    pub fn vox_required(self) -> bool { matches!(self, ArtifactForm::VoxScript) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftedArtifact {
    pub form: ArtifactForm,
    pub staging_path: String,
    pub body: String,
    pub form_rationale: String,
    pub authoring_model_vox_capable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationDecision {
    pub cluster_id: String,
    pub member_commit_shas: Vec<String>,
    pub member_count: usize,
    pub total_member_tokens: u64,
    pub artifact_form: ArtifactForm,
    pub confidence: f32,
    pub synthesized_fix_summary: String,
    pub drafted_artifact: Option<DraftedArtifact>,
    pub verified: bool,
    pub refutation_note: String,
}

/// Whether the selected judge model can author Vox source. Passed in by the CLI
/// so this crate need not depend on vox-orchestrator's model registry.
#[derive(Debug, Clone, Copy)]
pub struct ModelVoxCapability(pub bool);

#[async_trait]
pub trait Router: Send + Sync {
    /// Re-judge one cluster into a decision (decide + verify happen inside).
    async fn route(&self, cluster: &Cluster, cluster_id: &str, vox_capable: ModelVoxCapability) -> RemediationDecision;
}

/// Deterministic in-memory router for tests.
pub struct MockRouter { pub confidence: f32 }

#[async_trait]
impl Router for MockRouter {
    async fn route(&self, cluster: &Cluster, cluster_id: &str, vox_capable: ModelVoxCapability) -> RemediationDecision {
        // Pick a form from the bucket's remediation_kind, respecting the vox gate.
        let kind = &cluster.bucket.key.remediation_kind;
        let mut form = match kind.as_str() {
            "ScriptAutomation"      => ArtifactForm::VoxScript,
            "AgentsMdRule"          => ArtifactForm::AgentsMdRule,
            "LinterRule"            => ArtifactForm::CodeAuditDetector,
            "CorpusNegativeExample" => ArtifactForm::CorpusNegativeExample,
            _                       => ArtifactForm::None,
        };
        if form.vox_required() && !vox_capable.0 {
            form = ArtifactForm::CiGate; // fallback when not vox-capable
        }
        let shas: Vec<String> = cluster.bucket.members.iter().map(|m| m.row.commit_sha.clone()).collect();
        let tokens = cluster.bucket.members.iter().map(|m| token_sum(&m.row.cost)).sum();
        let artifact = if matches!(form, ArtifactForm::None) { None } else {
            Some(DraftedArtifact {
                form,
                staging_path: format!("{cluster_id}.{}", form.staging_extension()),
                body: format!("# proposed fix for {} members", shas.len()),
                form_rationale: "mock".into(),
                authoring_model_vox_capable: vox_capable.0,
            })
        };
        RemediationDecision {
            cluster_id: cluster_id.to_string(),
            member_count: shas.len(),
            member_commit_shas: shas,
            total_member_tokens: tokens,
            artifact_form: form,
            confidence: self.confidence,
            synthesized_fix_summary: "mock synthesis".into(),
            drafted_artifact: artifact,
            verified: self.confidence >= 0.5,
            refutation_note: "mock".into(),
        }
    }
}

/// Sum input+output tokens from a MeasuredCost (0 for Unavailable/Ambiguous).
pub fn token_sum(cost: &vox_effort_audit::hybrid::MeasuredCost) -> u64 {
    use vox_effort_audit::hybrid::MeasuredCost::*;
    match cost {
        Measured { input_tokens, output_tokens, .. } => input_tokens + output_tokens,
        Estimated { input_tokens, output_tokens } => input_tokens + output_tokens,
        Ambiguous | Unavailable => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vox_form_falls_back_when_not_capable() {
        assert!(ArtifactForm::VoxScript.vox_required());
        assert!(!ArtifactForm::CiGate.vox_required());
    }

    #[test]
    fn staging_extensions_all_end_in_proposed() {
        for f in [ArtifactForm::AgentsMdRule, ArtifactForm::CodeAuditDetector, ArtifactForm::ArchRule,
                  ArtifactForm::CiGate, ArtifactForm::VoxScript, ArtifactForm::CorpusNegativeExample] {
            assert!(f.staging_extension().ends_with(".proposed"), "{:?}", f);
        }
        assert_eq!(ArtifactForm::None.staging_extension(), "");
    }

    #[tokio::test]
    async fn mock_router_respects_vox_gate() {
        // Build a Cluster whose bucket.key.remediation_kind == "ScriptAutomation".
        // route with ModelVoxCapability(false) → artifact_form == CiGate.
        // route with ModelVoxCapability(true)  → artifact_form == VoxScript.
    }
}
```

Note: B1 declares `pub mod decide; pub mod verify; pub mod prompt;` — create those as one-line stubs in this commit so `cargo check` stays green; B2/B3 fill them.

Fill the `mock_router_respects_vox_gate` test body with a real Cluster.

- [ ] **Step 2: Run + commit**

Run: `cargo test -p vox-effort-route route::` → PASS.
```bash
git add crates/vox-effort-route
git commit -m "feat(vox-effort-route): Router trait + ArtifactForm + MockRouter (B1)"
```

---

### Task B2: `route/prompt.rs` — decide + refute prompts

**Files:**
- Replace stub: `crates/vox-effort-route/src/route/prompt.rs`
- Create: `crates/vox-effort-route/src/route/decide_system.md`
- Create: `crates/vox-effort-route/src/route/refute_system.md`

- [ ] **Step 1: Author the two system prompts (verbatim)**

`decide_system.md`:
```markdown
You are deciding the single cheapest enforceable fix that would have prevented
a cluster of related, token-wasting commits in a software project.

You are given a cluster: a group of commits that share a waste category, a
suggested remediation kind, and a primary crate. For the cluster as a whole
(not commit-by-commit) decide ONE authoritative remediation.

Choose an artifact_form from the ALLOWED set you are given (the set excludes
VoxScript unless the host says the authoring model is Vox-capable):
- AgentsMdRule: a one-paragraph rule in AGENTS.md would make agents skip this
- CodeAuditDetector: a vox-code-audit lint detector would catch this at write time
- ArchRule: a vox-arch-check / layers.toml rule would prevent this structurally
- CiGate: a CI contract entry or a test/example fixture would fail on this
- VoxScript: a small `vox run` script would have done the mechanical work in one commit
- CorpusNegativeExample: a MENS fine-tuning negative example would discourage it
- None: the cluster is legitimate work needing no structural fix

Then DRAFT the actual artifact body in the chosen form. Make it concrete and
correct for its target surface — real YAML for CiGate, a real rule spec for
CodeAuditDetector, a real markdown paragraph for AgentsMdRule, etc. Do NOT
draft Vox source unless VoxScript is in the allowed set.

Return one JSON object: { artifact_form, confidence (0..1), synthesized_fix_summary,
drafted_body, form_rationale }.

NEVER mention authors, emails, or blame. Base your judgment only on the diffs
and rationales shown.
```

`refute_system.md`:
```markdown
You are a skeptical reviewer trying to REFUTE a proposed fix for a cluster of
commits. You are given the cluster and the proposed remediation (form + body).

Ask:
- Would this fix actually have prevented these specific commits? If even one
  member commit would slip through, it is weak.
- Is the drafted artifact well-formed for its target surface (valid YAML, a
  real detector spec, etc.)?
- Is the fix overreaching (would it cause false positives on legitimate work)?

Default to refuted=true if you are uncertain. Return one JSON object:
{ refuted (bool), refutation_note }.
```

- [ ] **Step 2: Write `build_decide_messages` + `build_refute_messages` + tests**

```rust
//! Prompt construction for decide + refute passes.

use crate::cluster::Cluster;
use crate::route::ArtifactForm;
use vox_actor_runtime::llm::LlmChatMessage;

pub fn allowed_forms(vox_capable: bool) -> Vec<ArtifactForm> {
    let mut v = vec![
        ArtifactForm::AgentsMdRule, ArtifactForm::CodeAuditDetector,
        ArtifactForm::ArchRule, ArtifactForm::CiGate,
        ArtifactForm::CorpusNegativeExample, ArtifactForm::None,
    ];
    if vox_capable { v.push(ArtifactForm::VoxScript); }
    v
}

pub fn build_decide_messages(cluster: &Cluster, diffs: &[(String, String)], vox_capable: bool) -> Vec<LlmChatMessage> {
    let system = include_str!("decide_system.md");
    let allowed: Vec<String> = allowed_forms(vox_capable).iter().map(|f| format!("{f:?}")).collect();
    let members: String = cluster.bucket.members.iter().map(|m| {
        let f = m.row.finding.as_ref().unwrap();
        format!("- {} [{}] {}", m.row.commit_sha, f.waste_score, f.rationale_one_line)
    }).collect::<Vec<_>>().join("\n");
    let diff_block: String = diffs.iter().map(|(sha, d)| format!("### {sha}\n```\n{d}\n```")).collect::<Vec<_>>().join("\n");
    let user = format!(
"CLUSTER: {cat} / {kind} / {crate_}
ALLOWED artifact_form values: {allowed:?}

MEMBER COMMITS:
{members}

REPRESENTATIVE DIFFS:
{diff_block}

Decide one remediation and draft its artifact. Return the JSON object.",
        cat = cluster.bucket.key.waste_category,
        kind = cluster.bucket.key.remediation_kind,
        crate_ = cluster.bucket.key.primary_crate,
    );
    vec![
        LlmChatMessage { role: "system".into(), content: system.into() },
        LlmChatMessage { role: "user".into(), content: user },
    ]
}

pub fn build_refute_messages(cluster: &Cluster, form: ArtifactForm, body: &str) -> Vec<LlmChatMessage> {
    let system = include_str!("refute_system.md");
    let user = format!(
"CLUSTER: {cat} / {kind} / {crate_} ({n} commits)
PROPOSED form: {form:?}
PROPOSED body:
```
{body}
```
Try to refute. Return the JSON object.",
        cat = cluster.bucket.key.waste_category,
        kind = cluster.bucket.key.remediation_kind,
        crate_ = cluster.bucket.key.primary_crate,
        n = cluster.bucket.members.len(),
    );
    vec![
        LlmChatMessage { role: "system".into(), content: system.into() },
        LlmChatMessage { role: "user".into(), content: user },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_forms_gate_vox() {
        assert!(!allowed_forms(false).contains(&ArtifactForm::VoxScript));
        assert!(allowed_forms(true).contains(&ArtifactForm::VoxScript));
    }

    #[test]
    fn decide_prompt_includes_allowed_and_members() {
        // Build a 2-member cluster, assert user content contains the crate name
        // and "ALLOWED artifact_form".
    }
}
```

Fill the second test with a real cluster.

- [ ] **Step 3: Run + commit**

Run: `cargo test -p vox-effort-route route::prompt::tests` → PASS.
```bash
git add crates/vox-effort-route
git commit -m "feat(vox-effort-route): decide + refute prompts (B2)"
```

---

### Task B3: `route/decide.rs` + `route/verify.rs` + `LlmRouter`

**Files:**
- Replace stub: `crates/vox-effort-route/src/route/decide.rs`
- Replace stub: `crates/vox-effort-route/src/route/verify.rs`
- Modify: `crates/vox-effort-route/src/route/mod.rs` (add `LlmRouter`)

- [ ] **Step 1: `decide.rs` — parse helpers + JSON schema**

```rust
//! Decide-pass response parsing.

use crate::route::ArtifactForm;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DecideResponse {
    pub artifact_form: ArtifactForm,
    pub confidence: f32,
    pub synthesized_fix_summary: String,
    pub drafted_body: String,
    pub form_rationale: String,
}

pub fn decide_json_schema(vox_capable: bool) -> serde_json::Value {
    let mut forms = vec!["AgentsMdRule","CodeAuditDetector","ArchRule","CiGate","CorpusNegativeExample","None"];
    if vox_capable { forms.push("VoxScript"); }
    serde_json::json!({
      "type":"object",
      "properties":{
        "artifact_form":{"type":"string","enum":forms},
        "confidence":{"type":"number","minimum":0,"maximum":1},
        "synthesized_fix_summary":{"type":"string"},
        "drafted_body":{"type":"string"},
        "form_rationale":{"type":"string"}
      },
      "required":["artifact_form","confidence","synthesized_fix_summary","drafted_body","form_rationale"],
      "additionalProperties":false
    })
}

pub fn parse(raw: &str) -> Result<DecideResponse, String> {
    let cleaned = raw.trim().strip_prefix("```json").or_else(|| raw.trim().strip_prefix("```")).unwrap_or(raw.trim());
    let cleaned = cleaned.strip_suffix("```").unwrap_or(cleaned).trim();
    serde_json::from_str(cleaned).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_decide_response() {
        let raw = r#"{"artifact_form":"CiGate","confidence":0.8,"synthesized_fix_summary":"s","drafted_body":"b","form_rationale":"r"}"#;
        let d = parse(raw).unwrap();
        assert_eq!(d.artifact_form, ArtifactForm::CiGate);
    }
    #[test]
    fn schema_excludes_vox_when_incapable() {
        let s = decide_json_schema(false);
        let arr = s["properties"]["artifact_form"]["enum"].as_array().unwrap();
        assert!(!arr.iter().any(|v| v == "VoxScript"));
    }
}
```

- [ ] **Step 2: `verify.rs` — refute parsing**

```rust
//! Refute-pass response parsing.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RefuteResponse {
    pub refuted: bool,
    pub refutation_note: String,
}

pub fn refute_json_schema() -> serde_json::Value {
    serde_json::json!({
      "type":"object",
      "properties":{
        "refuted":{"type":"boolean"},
        "refutation_note":{"type":"string"}
      },
      "required":["refuted","refutation_note"],
      "additionalProperties":false
    })
}

pub fn parse(raw: &str) -> Result<RefuteResponse, String> {
    let cleaned = raw.trim().strip_prefix("```json").or_else(|| raw.trim().strip_prefix("```")).unwrap_or(raw.trim());
    let cleaned = cleaned.strip_suffix("```").unwrap_or(cleaned).trim();
    serde_json::from_str(cleaned).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_refute() {
        let r = parse(r#"{"refuted":false,"refutation_note":"ok"}"#).unwrap();
        assert!(!r.refuted);
    }
}
```

- [ ] **Step 3: `LlmRouter` in `route/mod.rs`**

Add a `LlmRouter` implementing `Router` that: re-reads up to `max_context_commits` member diffs via `gix` (reuse a helper analogous to S1's `walk.rs` shell-out — or extract a tiny `git show <sha>` call), builds decide messages, calls `vox_actor_runtime::llm::infer_with_retry` with `decide_json_schema`, parses, then (if `verify` enabled) builds refute messages, calls again with `refute_json_schema`, parses, and assembles a `RemediationDecision`. Budget + timeout mirror S1 B4. On decide failure → `verified=false, confidence=0, artifact_form=None`. The `staging_path` is `format!("{cluster_id}.{ext}")` joined under the staging dir by the emit step (B3 only stores the filename; emit prefixes the dir).

The exact `infer_with_retry` / `LlmResponse.usage` field names: reuse exactly what S1's `crates/vox-effort-audit/src/judge/mod.rs` (`LlmJudge`) does — open that file and copy the call shape. Do NOT bypass the facade.

Write one `#[tokio::test]` that constructs an `LlmRouter` with a stubbed facade is NOT possible without network; instead assert the assembly logic by unit-testing a private `assemble_decision(decide, refute, cluster, id)` pure function with fixture inputs.

- [ ] **Step 4: Run + lint guard + commit**

Run: `cargo test -p vox-effort-route route::` → PASS.
Run: `cargo run -q -p vox-code-audit --bin toestub -- --rules vox/llm/direct-provider-call --mode audit --format json --min-severity info crates/vox-effort-route` → `"findings": []`.
```bash
git add crates/vox-effort-route
git commit -m "feat(vox-effort-route): decide + verify parsing + LlmRouter (B3)"
```

---

## Phase C — Emit

### Task C1: `emit/jsonl.rs` + `emit/markdown.rs` + `emit/artifacts.rs`

**Files:**
- Replace stub: `crates/vox-effort-route/src/emit/mod.rs`
- Create: `crates/vox-effort-route/src/emit/{jsonl,markdown,artifacts}.rs`

- [ ] **Step 1: `emit/mod.rs` — RecommendationRow + module decls**

```rust
//! Output writers: recommendations.jsonl, recommendations.md, staging artifacts.

pub mod jsonl;
pub mod markdown;
pub mod artifacts;

use crate::route::RemediationDecision;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "1.0";

/// One row in recommendations.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationRow {
    pub schema_version: String,
    pub decision: RemediationDecision,
}

impl RecommendationRow {
    pub fn new(decision: RemediationDecision) -> Self {
        Self { schema_version: SCHEMA_VERSION.to_string(), decision }
    }
}
```

- [ ] **Step 2: `emit/artifacts.rs` — staging writer with the no-in-tree guard**

```rust
//! Writes draft artifacts to the staging dir. NEVER writes into the build tree.

use crate::route::RemediationDecision;
use std::path::Path;

/// Write one decision's drafted artifact into `staging_root`. Returns the path
/// written, or None if the decision has no artifact (form == None or unverified).
pub fn write_artifact(staging_root: &Path, decision: &RemediationDecision) -> std::io::Result<Option<std::path::PathBuf>> {
    let Some(artifact) = &decision.drafted_artifact else { return Ok(None); };
    if !decision.verified { return Ok(None); }
    // Filename only from staging_path; force it under staging_root; force .proposed.
    let filename = Path::new(&artifact.staging_path).file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad staging_path"))?;
    let dest = staging_root.join("artifacts").join(filename);
    assert!(dest.to_string_lossy().ends_with(".proposed"), "artifact must be .proposed");
    if let Some(parent) = dest.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&dest, &artifact.body)?;
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{ArtifactForm, DraftedArtifact};

    fn decision(form: ArtifactForm, verified: bool) -> RemediationDecision {
        RemediationDecision {
            cluster_id: "c1".into(), member_commit_shas: vec![], member_count: 0,
            total_member_tokens: 0, artifact_form: form, confidence: 0.9,
            synthesized_fix_summary: "s".into(),
            drafted_artifact: if matches!(form, ArtifactForm::None) { None } else {
                Some(DraftedArtifact { form, staging_path: format!("c1.{}", form.staging_extension()),
                    body: "body".into(), form_rationale: "r".into(), authoring_model_vox_capable: false })
            },
            verified, refutation_note: "n".into(),
        }
    }

    #[test]
    fn writes_proposed_file_under_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let p = write_artifact(tmp.path(), &decision(ArtifactForm::CiGate, true)).unwrap().unwrap();
        assert!(p.starts_with(tmp.path()));
        assert!(p.to_string_lossy().ends_with(".proposed"));
        assert!(p.components().any(|c| c.as_os_str() == "artifacts"));
    }

    #[test]
    fn no_write_for_unverified_or_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(write_artifact(tmp.path(), &decision(ArtifactForm::CiGate, false)).unwrap().is_none());
        assert!(write_artifact(tmp.path(), &decision(ArtifactForm::None, true)).unwrap().is_none());
    }
}
```

- [ ] **Step 3: `emit/jsonl.rs` — streaming writer** (mirror S1's `output/jsonl.rs`: `JsonlWriter::create` + `append(&RecommendationRow)` with per-row flush). Add a round-trip test.

- [ ] **Step 4: `emit/markdown.rs` — renderer + snapshot + author-leak guard**

`render(rows: &[RecommendationRow]) -> String` produces: summary (counts, verified vs not), a Top-N table ranked by `total_member_tokens` desc then confidence desc (verified first), a per-`artifact_form` breakdown, and a methodology note. Include:

```rust
#[test]
fn does_not_emit_author_identity() {
    // Build rows, render, assert no '@' and no 64-hex run.
    let out = render(&rows);
    assert!(!out.contains('@'));
    assert!(!out.as_bytes().windows(64).any(|w| w.iter().all(|b| b.is_ascii_hexdigit())));
}

#[test]
fn report_snapshot() {
    // Deterministic fixture rows (fixed cluster ids, no timestamps), insta snapshot.
    insta::assert_snapshot!(render(&fixture_rows()));
}
```

Accept the snapshot with `cargo insta accept -p vox-effort-route` after manual review.

- [ ] **Step 5: Run + commit**

Run: `cargo test -p vox-effort-route emit::` → PASS (accept snapshot first).
```bash
git add crates/vox-effort-route
git commit -m "feat(vox-effort-route): emit jsonl + markdown + staging artifacts (C1)"
```

---

## Phase D — Pipeline + telemetry + CLI

### Task D1: `audit.route.*` telemetry events

**Files:**
- Modify: `crates/vox-telemetry/src/...` (locate the events module the same way S1's E1 did)

- [ ] **Step 1: Locate event registry + write failing round-trip test**

Mirror S1's `audit.effort.*` events (grep `AuditEffortRunStartedEvent` to find the file). Add `AuditRouteRunStartedEvent`, `AuditRouteClusterDecidedEvent`, `AuditRouteRunCompletedEvent`, `AuditRouteRunFailedEvent` following the exact same pattern. Round-trip test.

- [ ] **Step 2: Run + commit**

Run: `cargo test -p vox-telemetry audit_route` → PASS.
```bash
git add crates/vox-telemetry
git commit -m "feat(vox-telemetry): add audit.route.* event types (D1)"
```

---

### Task D2: `pipeline.rs` — composition + budget + concurrency

**Files:**
- Replace stub: `crates/vox-effort-route/src/pipeline.rs`
- Modify: `crates/vox-effort-route/src/lib.rs` (add `pub use pipeline::run;`)

- [ ] **Step 1: Write failing e2e integration test**

Create `crates/vox-effort-route/tests/e2e.rs`:

```rust
//! e2e: run the pipeline against a fixture findings.jsonl with MockRouter + MockEmbedder.

use std::path::PathBuf;
use vox_effort_route::config::EffortRouteConfig;
use vox_effort_route::route::{MockRouter, ModelVoxCapability};

#[tokio::test]
async fn smoke_run_produces_outputs() {
    let findings = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/findings.jsonl");
    let out = tempfile::tempdir().unwrap();
    let mut cfg = EffortRouteConfig::default();
    cfg.staging_dir = out.path().to_path_buf();
    let summary = vox_effort_route::run(
        &findings, out.path(), cfg,
        Box::new(MockRouter { confidence: 0.9 }),
        // MockEmbedder unused for small buckets but required by signature:
        Box::new(vox_effort_route::cluster_test_support::ZeroEmbedder),
        ModelVoxCapability(false),
    ).await.unwrap();

    assert!(out.path().join("recommendations.jsonl").exists());
    assert!(out.path().join("recommendations.md").exists());
    // 2 surviving findings → at least 1 cluster, at least 1 verified recommendation.
    assert!(summary.clusters_routed >= 1);
}
```

(If exposing a `ZeroEmbedder` from the crate is undesirable, define a tiny one inline in the test file implementing `cluster::Embedder`.)

- [ ] **Step 2: Implement `run`**

```rust
//! Top-level run: load → bucket → cluster → route(decide+verify) → emit.

use crate::cluster::Embedder;
use crate::config::EffortRouteConfig;
use crate::emit::RecommendationRow;
use crate::route::{ModelVoxCapability, Router};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RouteSummary {
    pub run_id: String,
    pub findings_loaded: usize,
    pub buckets: usize,
    pub clusters_routed: usize,
    pub verified: usize,
}

pub async fn run(
    findings_path: &Path,
    out_dir: &Path,
    cfg: EffortRouteConfig,
    router: Box<dyn Router>,
    embedder: Box<dyn Embedder>,
    vox_capable: ModelVoxCapability,
) -> anyhow::Result<RouteSummary> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let loaded = crate::load::read(findings_path, cfg.min_waste_score)?;
    let findings_loaded = loaded.len();
    let buckets = crate::bucket::group(loaded);
    let bucket_count = buckets.len();
    let clusters = crate::cluster::maybe_split(buckets, cfg.max_bucket_size, embedder.as_ref()).await;

    std::fs::create_dir_all(out_dir)?;
    let mut writer = crate::emit::jsonl::JsonlWriter::create(&out_dir.join("recommendations.jsonl"))?;
    let mut rows: Vec<RecommendationRow> = Vec::new();
    let mut verified = 0usize;

    for (i, cluster) in clusters.iter().enumerate() {
        let cluster_id = format!("{run_id}-{i}");
        let decision = router.route(cluster, &cluster_id, vox_capable).await;
        if decision.verified { verified += 1; }
        crate::emit::artifacts::write_artifact(out_dir, &decision)?;
        let row = RecommendationRow::new(decision);
        writer.append(&row)?;
        rows.push(row);
    }

    std::fs::write(out_dir.join("recommendations.md"), crate::emit::markdown::render(&rows))?;

    Ok(RouteSummary {
        run_id, findings_loaded, buckets: bucket_count,
        clusters_routed: clusters.len(), verified,
    })
}
```

Add `anyhow` to deps. Add `pub use pipeline::run;` to `lib.rs` now (pipeline::run exists).

Budget tracking: thread `cfg.judge.max_total_tokens` through; the `LlmRouter` accumulates and the pipeline stops routing (emitting `artifact_form=None` rows) once exhausted — for the MockRouter path this is a no-op. Concurrency: route clusters via a `Semaphore(cfg.max_concurrent)`-bounded `FuturesUnordered` if `Router` is `Sync` — but since artifact writes + jsonl appends must be serialized, collect decisions concurrently then emit sequentially. For S2's cluster counts (tens), sequential routing is acceptable; add concurrency only if a timing test shows need. Keep it sequential in D2 to stay simple; note the option.

- [ ] **Step 3: Run + commit**

Run: `cargo test -p vox-effort-route --test e2e` → PASS.
Run: `cargo test -p vox-effort-route` → all pass.
```bash
git add crates/vox-effort-route
git commit -m "feat(vox-effort-route): pipeline composition + e2e (D2)"
```

---

### Task D3: `vox audit effort-route` CLI subcommand

**Files:**
- Create: `crates/vox-cli/src/commands/audit_route.rs`
- Modify: the `audit` subcommand enum (grep where `effort` was wired in S1's F1)
- Modify: `crates/vox-cli/Cargo.toml` (add `vox-effort-route` dep)

- [ ] **Step 1: Write failing CLI help test**

`crates/vox-cli/tests/audit_route_cli.rs`:

```rust
#[test]
fn audit_route_help_includes_findings_flag() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["audit", "effort-route", "--help"]).output().unwrap();
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("--findings"), "help:\n{s}");
}
```

- [ ] **Step 2: Add the subcommand**

Mirror S1's `effort` subcommand exactly. `EffortRouteArgs { findings: PathBuf, out_dir: Option<PathBuf>, model: Option<String> }`. In `run`:
1. Load `[audit.route]` config from `vox.toml` (reuse S1's config-load pattern), merge CLI.
2. Resolve judge model id AND its Vox-capability via the same `vox-orchestrator::models::select` path S1's F1 used (open `crates/vox-cli/src/commands/audit_effort.rs` — or whatever S1 named it — and copy the resolution; for capability, read `ModelCapabilities.writes_vox` if present, else default `false` / config allowlist per spec Q3).
3. Build an `LlmRouter` with the resolved model + a real `LlmEmbedder` (an `Embedder` impl calling `llm_embed`).
4. `out_dir = args.out_dir.unwrap_or(cfg.staging_dir.join(&run_id))`.
5. Call `vox_effort_route::run(...)`, print the path to `recommendations.md`.

Create the `LlmEmbedder` (implements `crate::cluster::Embedder` by calling `vox_actor_runtime::llm::llm_embed` with an embed `LlmConfig`) inside `vox-effort-route` (`src/cluster.rs` or a small `embed.rs`), so the CLI just constructs it.

- [ ] **Step 3: Run + commit**

Run: `cargo test -p vox-cli --test audit_route_cli` → PASS.
Run: `cargo run -q -p vox-cli -- audit effort-route --help` → shows `--findings`.
```bash
git add crates/vox-cli crates/vox-effort-route
git commit -m "feat(vox-cli): vox audit effort-route subcommand (D3)"
```

---

## Phase E — Finishing

### Task E1: AGENTS.md umbrella + coverage floor + README

**Files:**
- Modify: `AGENTS.md` (add `effort-route` to the `vox audit` umbrella list)
- Modify: `.config/coverage-gates.toml` (add `vox-effort-route = 70.0`)
- Create: `crates/vox-effort-route/README.md`

- [ ] **Step 1: AGENTS.md** — add a bullet `- vox audit effort-route — routes audit findings to verified enforcement-artifact proposals (vox-effort-route)` to the umbrella section S1's F2 created.

- [ ] **Step 2: coverage floor** — add `vox-effort-route = 70.0` in `[crates]`, alphabetical. Run `cargo run -q -p vox-cli -- ci coverage-gates --since main`; if below 70, lower to measured value rounded down to nearest 5 with a comment (per the `vox-cli` precedent).

- [ ] **Step 3: README** — CLI examples (`vox audit effort-route --findings target/audit/effort/<run-id>/findings.jsonl`), output layout, the ArtifactForm table, S2-of-4 framing, live-network test note.

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md .config/coverage-gates.toml crates/vox-effort-route/README.md
git commit -m "docs(vox-effort-route): umbrella + coverage floor + README (E1)"
```

---

### Task E2: Acceptance gate + PR

**Files:** none changed; verification only.

- [ ] **Step 1:** `cargo run -q -p vox-cli -- ci pre-push --full` → green.
- [ ] **Step 2:** `cargo run -q -p vox-arch-check` → green for `vox-effort-route`.
- [ ] **Step 3:** `cargo run -q -p vox-code-audit --bin toestub -- --rules vox/llm/direct-provider-call --mode audit --format json crates/vox-effort-route` → `"findings": []`.
- [ ] **Step 4: Manual smoke** — generate a real findings file from S1 (`cargo run -p vox-cli -- audit effort --since "30 days ago" --limit 30`), then `cargo run -p vox-cli -- audit effort-route --findings <that path>`. Verify: `recommendations.md` has no author identity; every staging artifact ends in `.proposed`; at least one verified recommendation whose body is well-formed for its surface.
- [ ] **Step 5: Push + PR**

```bash
git push -u origin spec/effort-route
gh pr create --title "feat(vox-effort-route): route audit findings to verified enforcement artifacts (S2)" --body "$(cat <<'EOF'
## Summary
- New L2 crate vox-effort-route + `vox audit effort-route` CLI
- Consumes S1's findings.jsonl; deterministic bucket + conditional embedding sub-cluster; re-judge + adversarial verify; emits recommendations.jsonl/md + staging-dir .proposed artifacts
- Drafted fixes take whatever enforcement form the repo uses; Vox gated behind model capability
- S2 of 4; S3 (billing cost) + S4 (auto-emit) deferred

Spec: docs/superpowers/specs/2026-05-30-effort-route-design.md
Plan: docs/superpowers/plans/2026-05-30-effort-route.md

## Test plan
- [ ] cargo test -p vox-effort-route passes (unit + e2e)
- [ ] cargo test -p vox-cli --test audit_route_cli passes
- [ ] cargo run -p vox-arch-check clean
- [ ] no llm_provider_call findings
- [ ] manual: recommendations.md author-free; all artifacts .proposed

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

**Spec coverage:** §1 in-scope → A4 (load/filter), A5 (bucket), A6 (cluster), B1–B3 (route+verify+forms), C1 (emit+artifacts), D2 (pipeline), D3 (CLI). §2 arch → A1–A2. §3 pipeline → A4–D2. §4 RemediationDecision → B1. §5 ArtifactForm + Vox gate → B1 (enum), B2 (allowed_forms), B3 (schema gate), D3 (capability resolution). §6 config → A3, D3. §7 errors → A4 (LoadError), B3 (decide/verify fail), D2. §8 testing → every task test-first; e2e D2; snapshot C1; no-in-tree-write guard C1; coverage E1. §9 model-agnostic → B3 (facade), D3 (LlmEmbedder via llm_embed). §10 docs → A2, E1. §11 S4 hooks → C1 (jsonl contract), traits in B1/A6. §12 acceptance → E2. §13 risks → mitigations in A6 (conditional embed), B1/B3 (vox gate), C1 (.proposed guard), A4 (schema abort). §14 open Qs resolved: Q1 hand-rolled agglomerative (A6); Q2 evidence-pointer-first primary_crate (A5); Q3 capability passed from CLI (B1 ModelVoxCapability, D3 resolution).

**Placeholder scan:** test bodies marked `...` in A5/A6/B1/B2 are explicitly flagged "fill with real construction" with the pattern named — not silent TODOs. No "add error handling" hand-waves; every error path has a concrete enum + arm.

**Type consistency:** `RemediationDecision`/`ArtifactForm`/`DraftedArtifact` defined in B1, used unchanged in B3/C1/D2. `Embedder` trait in A6 used in D2/D3. `Router` in B1 used in D2/D3. `RecommendationRow` in C1 used in D2. `token_sum` in B1 matches the real `MeasuredCost` variants (no `estimated_usd`). `LoadedFinding` in A4 used in A5/A6.

**Caveats for the executor:**
- B3's `LlmRouter` must copy the exact `infer_with_retry` call shape from S1's `LlmJudge` (`crates/vox-effort-audit/src/judge/mod.rs`). **VERIFIED real surface (not what an earlier draft of this caveat assumed):** `infer_with_retry(&ActivityOptions, messages, vec![llm_config])` returns `ActivityResult::Ok(Ok((resp, _cfg)))` (plus `Ok(Err(api_err))`, `Failed`, `Cancelled`); `LlmResponse { content: String, prompt_tokens: u32, completion_tokens: u32 }` — there is NO `usage` struct and NO `input_tokens`/`output_tokens` fields. Read `resp.content`. The routing layer derives `total_member_tokens` from each member's existing `MeasuredCost` (not from the LLM response), so the response token fields are not needed in S2 at all. Do not bypass the facade.
- D3's model-capability resolution depends on whether `ModelCapabilities.writes_vox` exists. If absent, use the config-allowlist fallback (spec Q3) and note it in the commit.
- The fixture `findings.jsonl` (A4) must match S1's real `FindingRow` serialization exactly — serialize a default to confirm field names before authoring the fixture.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-30-effort-route.md`. Two execution options:

**1. Subagent-Driven (recommended)** — fresh subagent per task (or phase-batch the mechanical ones as we did for S1), two-stage review between.

**2. Inline Execution** — execute in this session via executing-plans with checkpoints.

**Which approach?**
