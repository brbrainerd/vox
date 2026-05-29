# Effort Audit Core (S1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a new `vox-effort-audit` crate plus `vox audit effort` CLI subcommand that walks git history, calls a model-agnostic LLM judge per commit, optionally substitutes measured token cost from Claude Code transcripts, and emits a ranked `findings.jsonl` + `report.md` + `manifest.json`.

**Architecture:** New L2 crate (`crates/vox-effort-audit/`, `max_loc = 4000`) with a thin CLI shim in `vox-cli`. All LLM I/O goes through `vox_actor_runtime::llm`; model selection adds a new `CodeEffortJudge` task category to `contracts/orchestration/model-routing.v1.yaml`. Hybrid cost signal is partial in S1 (Claude Code transcripts only); billing exports and broader telemetry are S3. Output JSONL `schema_version = "1.0"` is the stable contract S2–S4 consume.

**Tech Stack:** Rust 2024, `gix 0.70` (workspace-pinned), `serde`/`serde_json`, `tokio`, `futures` (`FuturesUnordered`), `insta` for snapshot tests, `tempfile` for fixture repos, `vox-actor-runtime` for LLM, `vox-secrets` for keys, `vox-telemetry` for events, `vox-config` for timeouts.

**Spec:** [`docs/superpowers/specs/2026-05-28-effort-audit-core-design.md`](../specs/2026-05-28-effort-audit-core-design.md).

**Branch / worktree:** `spec/effort-audit-core` in `.worktrees/effort-audit-spec/`.

**Pre-flight verification (do this once before Task 1):**
```bash
git -C .worktrees/effort-audit-spec status                # should be clean
git -C .worktrees/effort-audit-spec log --oneline main..HEAD   # 3 commits (spec, AGENTS rule, coverage-fix)
cargo --version && rustc --version                        # 2024 edition support
```
If anything is off, stop and resolve before proceeding.

---

## Phase A — Foundations (no LLM)

### Task A1: Scaffold the crate

**Files:**
- Create: `crates/vox-effort-audit/Cargo.toml`
- Create: `crates/vox-effort-audit/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add `crates/vox-effort-audit` to `members`

- [ ] **Step 1: Write failing workspace-build test**

Run: `cargo check -p vox-effort-audit`
Expected: FAIL with `package ID specification vox-effort-audit did not match any packages`.

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name        = "vox-effort-audit"
version.workspace     = true
edition.workspace     = true
license.workspace     = true
description = "AI-judged audit of git commit history, ranking commits by estimated agent token spend"

[dependencies]
vox-actor-runtime = { path = "../vox-actor-runtime" }
vox-secrets       = { path = "../vox-secrets" }
vox-telemetry     = { path = "../vox-telemetry" }
vox-config        = { path = "../vox-config" }
gix               = { workspace = true }
serde             = { workspace = true, features = ["derive"] }
serde_json        = { workspace = true }
chrono            = { workspace = true }
uuid              = { workspace = true, features = ["v7"] }
tracing           = { workspace = true }
tokio             = { workspace = true, features = ["macros", "rt-multi-thread", "sync"] }
futures           = { workspace = true }
thiserror         = { workspace = true }
sha2              = { workspace = true }
regex             = { workspace = true }

[dev-dependencies]
insta             = { workspace = true, features = ["json", "yaml"] }
tempfile          = { workspace = true }
tokio             = { workspace = true, features = ["macros", "rt-multi-thread", "test-util"] }
```

If any of the listed workspace deps are missing from the root `Cargo.toml`'s `[workspace.dependencies]`, add them with the version pinned by an existing consumer. Run `cargo tree -p vox-actor-runtime` to look up real versions; do not invent.

- [ ] **Step 3: Create lib.rs**

```rust
//! AI-judged audit of git commit history.
//!
//! See `docs/superpowers/specs/2026-05-28-effort-audit-core-design.md`.

pub mod config;
pub mod range;
pub mod walk;
pub mod shape;
pub mod judge;
pub mod hybrid;
pub mod output;
pub mod pipeline;

pub use pipeline::run;
```

(Each `mod` line forward-declares a module that subsequent tasks create. Compilation will fail until the modules exist — that is intentional and resolved in A4–F18.)

- [ ] **Step 4: Add to workspace members**

Edit root `Cargo.toml`, locate `[workspace] members = [...]`, insert `"crates/vox-effort-audit",` in alphabetical position.

- [ ] **Step 5: Re-run the check**

Run: `cargo check -p vox-effort-audit`
Expected: FAIL with `unresolved module` for each submodule. That confirms the crate is recognized; the unresolved modules are filled in by later tasks. The crate compiles green at the end of Task A4 once `config.rs` lands and we add `#[allow(dead_code)]` placeholders for the rest in A1.5.

- [ ] **Step 5b: Temporary stub modules**

To keep `cargo check` green between tasks, create each declared module as a one-line stub that A2+ overwrites:

```bash
for m in config range walk shape judge hybrid output pipeline; do
  case "$m" in
    judge|hybrid|output) mkdir -p crates/vox-effort-audit/src/$m && \
      printf "//! Stub; see plan.\n" > crates/vox-effort-audit/src/$m/mod.rs ;;
    *) printf "//! Stub; see plan.\n" > crates/vox-effort-audit/src/$m.rs ;;
  esac
done
```

These stubs contain no `pub fn`, so the TDD guard does not require a test. They are deleted-and-replaced by each later task, not extended — per `feedback_no_stubs.md`.

- [ ] **Step 6: Run check again**

Run: `cargo check -p vox-effort-audit`
Expected: PASS, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): scaffold L2 crate (A1)

Empty module skeleton; subsequent tasks fill in each module per plan
docs/superpowers/plans/2026-05-28-effort-audit-core.md."
```

---

### Task A2: Architectural registration

**Files:**
- Modify: `docs/src/architecture/layers.toml` — add `[crates.vox-effort-audit]`
- Modify: `docs/src/architecture/where-things-live.md` — add row

- [ ] **Step 1: Add layers.toml row**

Append to the `[crates.*]` section, alphabetical position:

```toml
[crates.vox-effort-audit]
layer            = 2
kind             = "library"
max_loc          = 4000
staleness_exempt = false
```

- [ ] **Step 2: Add where-things-live row**

Insert into the table (alphabetical by concept) a row:

```markdown
| AI-effort auditing of git history | `crates/vox-effort-audit/` | Walks commits, calls model-agnostic judge facade, emits findings JSONL + report. CLI: `vox audit effort`. |
```

(Match the exact 3-column shape the existing table uses; copy from a nearby row if the columns drift.)

- [ ] **Step 3: Run vox-arch-check**

Run: `cargo run -q -p vox-arch-check`
Expected: PASS. `vox-effort-audit` shows in the layer-2 report. No `where_things_live` mismatch.

- [ ] **Step 4: Commit**

```bash
git add docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register vox-effort-audit in layers + WTL (A2)"
```

---

### Task A3: Add `CodeEffortJudge` task category

**Files:**
- Modify: `contracts/orchestration/model-routing.v1.yaml` — append `CodeEffortJudge` to `task_categories`

- [ ] **Step 1: Write the failing parity test**

Inspect: `crates/vox-orchestrator-types/tests/routing_and_ids_smoke.rs` — find the test that asserts `TaskCategory::from_str("CodeEffortJudge").is_ok()` if one exists; if not, add this test inline at the bottom of that file:

```rust
#[test]
fn code_effort_judge_category_present() {
    use vox_orchestrator_types::TaskCategory;
    use std::str::FromStr;
    assert!(matches!(
        TaskCategory::from_str("CodeEffortJudge").unwrap(),
        TaskCategory::CodeEffortJudge
    ));
}
```

Run: `cargo test -p vox-orchestrator-types code_effort_judge_category_present`
Expected: FAIL — variant not present.

- [ ] **Step 2: Add the category to the SSOT YAML**

In `contracts/orchestration/model-routing.v1.yaml` under `task_categories:`, append `  - CodeEffortJudge` after `Visus` (preserve alphabetical drift only if the existing list is alphabetical — it is not; appending is correct).

- [ ] **Step 3: Re-run the test**

Run: `cargo test -p vox-orchestrator-types code_effort_judge_category_present`
Expected: PASS. The build.rs in `vox-orchestrator-types/build.rs` reads the YAML and re-emits the enum.

If the YAML schema (`contracts/orchestration/model-routing.v1.schema.json`) enforces a closed-set enum, also append `"CodeEffortJudge"` to its `task_categories.items.enum` array.

- [ ] **Step 4: Re-emit any generated catalogs**

Run: `cargo run -q -p vox-cli -- ci command-sync` (in case any docs reference the routing list).
Run: `cargo run -q -p vox-cli -- ci generate-plugin-catalog-docs` if a plugin catalog references task categories — check first with `grep -l "Visus" docs/src/reference/*.generated.md`; only run if a match exists.

- [ ] **Step 5: Commit**

```bash
git add contracts/orchestration crates/vox-orchestrator-types/tests \
        docs/src/reference/  # only if regenerated
git commit -m "feat(routing): add CodeEffortJudge task category (A3)"
```

---

### Task A4: Add `EFFORT_AUDIT_JUDGE_TIMEOUT` to timeouts SSOT

**Files:**
- Modify: `crates/vox-config/src/timeouts.rs`
- Test: `crates/vox-config/src/timeouts.rs` (tests live in-file per house style)

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-config/src/timeouts.rs` after the existing test block:

```rust
#[test]
fn effort_audit_judge_timeout_is_60s() {
    assert_eq!(EFFORT_AUDIT_JUDGE_TIMEOUT, std::time::Duration::from_secs(60));
}
```

Run: `cargo test -p vox-config effort_audit_judge_timeout_is_60s`
Expected: FAIL — `EFFORT_AUDIT_JUDGE_TIMEOUT` undefined.

- [ ] **Step 2: Add the constant**

In `crates/vox-config/src/timeouts.rs`, in the same group as other long-running operation timeouts, add:

```rust
/// Per-commit LLM judge timeout for `vox audit effort`. See
/// `docs/superpowers/specs/2026-05-28-effort-audit-core-design.md` §4.4.
pub const EFFORT_AUDIT_JUDGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
```

- [ ] **Step 3: Run the test**

Run: `cargo test -p vox-config effort_audit_judge_timeout_is_60s`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-config/src/timeouts.rs
git commit -m "feat(vox-config): add EFFORT_AUDIT_JUDGE_TIMEOUT (A4)"
```

---

### Task A5: Implement `config.rs` (TOML + CLI merge)

**Files:**
- Replace stub: `crates/vox-effort-audit/src/config.rs`
- Test: same file, inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test**

Replace `config.rs` stub with:

```rust
//! Configuration for `vox audit effort`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffortAuditConfig {
    #[serde(default = "default_since")]
    pub default_since: String,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    #[serde(default = "default_max_diff_bytes")]
    pub max_diff_bytes: usize,
    #[serde(default = "default_true")]
    pub with_transcripts: bool,
    #[serde(default = "default_transcript_dir")]
    pub transcript_dir: PathBuf,
    #[serde(default = "default_report_top_n")]
    pub report_top_n: usize,
    #[serde(default)]
    pub judge: JudgeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct JudgeConfig {
    pub model_preference: Option<String>,
    #[serde(default = "default_max_total_tokens")]
    pub max_total_tokens: u64,
    #[serde(default = "default_max_dollar_cost")]
    pub max_dollar_cost: f64,
    #[serde(default = "default_schema_retry_limit")]
    pub schema_retry_limit: u32,
}

fn default_since() -> String { "30 days ago".into() }
fn default_max_concurrent() -> usize { 4 }
fn default_max_diff_bytes() -> usize { 200 * 1024 }
fn default_true() -> bool { true }
fn default_transcript_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude/projects")
}
fn default_report_top_n() -> usize { 20 }
fn default_max_total_tokens() -> u64 { 5_000_000 }
fn default_max_dollar_cost() -> f64 { 5.00 }
fn default_schema_retry_limit() -> u32 { 1 }

impl Default for EffortAuditConfig {
    fn default() -> Self {
        Self {
            default_since: default_since(),
            max_concurrent: default_max_concurrent(),
            max_diff_bytes: default_max_diff_bytes(),
            with_transcripts: default_true(),
            transcript_dir: default_transcript_dir(),
            report_top_n: default_report_top_n(),
            judge: JudgeConfig {
                model_preference: None,
                max_total_tokens: default_max_total_tokens(),
                max_dollar_cost: default_max_dollar_cost(),
                schema_retry_limit: default_schema_retry_limit(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = EffortAuditConfig::default();
        assert_eq!(c.default_since, "30 days ago");
        assert_eq!(c.max_concurrent, 4);
        assert_eq!(c.max_diff_bytes, 200 * 1024);
        assert!(c.with_transcripts);
        assert_eq!(c.report_top_n, 20);
        assert_eq!(c.judge.max_total_tokens, 5_000_000);
        assert!((c.judge.max_dollar_cost - 5.00).abs() < f64::EPSILON);
    }

    #[test]
    fn partial_toml_inherits_defaults() {
        let t: EffortAuditConfig = toml::from_str(r#"
            default_since = "7 days ago"
            [judge]
            model_preference = "mens-r6.2"
        "#).unwrap();
        assert_eq!(t.default_since, "7 days ago");
        assert_eq!(t.judge.model_preference.as_deref(), Some("mens-r6.2"));
        assert_eq!(t.max_concurrent, 4);  // default
    }
}
```

Add `toml` and `dirs` to `[dev-dependencies]` and `[dependencies]` respectively (look up workspace versions first).

- [ ] **Step 2: Run tests**

Run: `cargo test -p vox-effort-audit config::tests`
Expected: PASS (both tests).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): config + TOML schema (A5)"
```

---

### Task A6: Implement `range.rs`

**Files:**
- Replace stub: `crates/vox-effort-audit/src/range.rs`

- [ ] **Step 1: Write the failing test**

Append to a fresh `range.rs`:

```rust
//! Resolution of `--since`/`--until` into a concrete `CommitRange`.

use chrono::{DateTime, Duration, Utc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum CommitRange {
    /// Inclusive `since_ref..until_ref` git ref pair.
    Refs { since: String, until: String },
    /// "commits with commit_ts >= now - duration, walking from until_ref".
    SinceDuration { duration: Duration, until: String },
}

#[derive(Debug, Error)]
pub enum RangeError {
    #[error("invalid duration string: {0}")]
    InvalidDuration(String),
}

/// Parses a duration string of the form `<n>{d|h|w}` or "<n> days ago" / "<n> hours ago".
pub fn parse_duration(s: &str) -> Result<Duration, RangeError> {
    let s = s.trim();
    if let Some(rest) = s.strip_suffix(" days ago").or_else(|| s.strip_suffix("d")) {
        return rest.trim().parse::<i64>()
            .map(Duration::days)
            .map_err(|_| RangeError::InvalidDuration(s.into()));
    }
    if let Some(rest) = s.strip_suffix(" hours ago").or_else(|| s.strip_suffix("h")) {
        return rest.trim().parse::<i64>()
            .map(Duration::hours)
            .map_err(|_| RangeError::InvalidDuration(s.into()));
    }
    if let Some(rest) = s.strip_suffix(" weeks ago").or_else(|| s.strip_suffix("w")) {
        return rest.trim().parse::<i64>()
            .map(Duration::weeks)
            .map_err(|_| RangeError::InvalidDuration(s.into()));
    }
    Err(RangeError::InvalidDuration(s.into()))
}

/// Resolves CLI args + config default into a `CommitRange`.
///
/// `since_arg` and `until_arg` are the raw `--since`/`--until` strings.
/// If neither parses as a duration, both are treated as git refs.
pub fn resolve(
    since_arg: Option<&str>,
    until_arg: Option<&str>,
    default_since: &str,
) -> Result<CommitRange, RangeError> {
    let until = until_arg.unwrap_or("HEAD").to_string();
    let since_raw = since_arg.unwrap_or(default_since);

    match parse_duration(since_raw) {
        Ok(d) => Ok(CommitRange::SinceDuration { duration: d, until }),
        Err(_) => Ok(CommitRange::Refs { since: since_raw.into(), until }),
    }
}

/// For `SinceDuration`, the wall-clock cutoff at run time.
pub fn duration_cutoff(now: DateTime<Utc>, d: Duration) -> DateTime<Utc> {
    now - d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_days_suffix_forms() {
        assert_eq!(parse_duration("30d").unwrap(), Duration::days(30));
        assert_eq!(parse_duration("30 days ago").unwrap(), Duration::days(30));
    }

    #[test]
    fn duration_default_when_no_args() {
        let r = resolve(None, None, "30 days ago").unwrap();
        assert!(matches!(r, CommitRange::SinceDuration { .. }));
    }

    #[test]
    fn ref_when_since_is_sha_or_branch() {
        let r = resolve(Some("v0.5.0"), Some("HEAD"), "30 days ago").unwrap();
        assert!(matches!(r, CommitRange::Refs { ref since, .. } if since == "v0.5.0"));
    }

    #[test]
    fn head_caret_is_ref_not_duration() {
        // HEAD~30 is git-native "30 commits back", not a duration. Must be treated as ref.
        let r = resolve(Some("HEAD~30"), None, "30 days ago").unwrap();
        assert!(matches!(r, CommitRange::Refs { .. }));
    }
}
```

Run: `cargo test -p vox-effort-audit range::tests`
Expected: PASS.

- [ ] **Step 2: Commit**

```bash
git add crates/vox-effort-audit/src/range.rs
git commit -m "feat(vox-effort-audit): commit-range resolution (A6)"
```

---

### Task A7: Implement `walk.rs` (gix iterator)

**Files:**
- Replace stub: `crates/vox-effort-audit/src/walk.rs`
- Create: `crates/vox-effort-audit/tests/fixtures/repos/.gitkeep`

- [ ] **Step 1: Test fixture**

Write a tiny helper script `crates/vox-effort-audit/tests/support/mod.rs`:

```rust
use std::path::PathBuf;
use std::process::Command;

pub fn make_smoke_repo() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let run = |args: &[&str]| {
        let s = Command::new("git").current_dir(&path).args(args).status().unwrap();
        assert!(s.success(), "git {:?}", args);
    };
    run(&["init", "--quiet", "-b", "main"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "Test"]);
    run(&["config", "commit.gpgsign", "false"]);
    for i in 0..5 {
        std::fs::write(path.join(format!("f{i}.txt")), format!("hello {i}\n")).unwrap();
        run(&["add", "."]);
        run(&["commit", "--quiet", "-m", &format!("commit {i}")]);
    }
    (dir, path)
}
```

- [ ] **Step 2: Write the failing test**

Replace `walk.rs` with:

```rust
//! gix-backed commit walker.

use crate::range::CommitRange;
use chrono::{DateTime, Utc};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct CommitRecord {
    pub sha: String,
    pub parent_sha: Option<String>,
    pub commit_ts: DateTime<Utc>,
    pub message: String,
    pub author_email_sha256: String,
    pub files: Vec<FileChange>,
    pub additions: u64,
    pub deletions: u64,
    pub unified_diff_text: String,
    pub diff_truncated: bool,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub additions: u64,
    pub deletions: u64,
}

#[derive(Debug, Error)]
pub enum WalkError {
    #[error("git open failed: {0}")]
    Open(String),
    #[error("git walk failed: {0}")]
    Walk(String),
}

/// Iterates commits in the range, newest-first.
pub fn iter_commits(
    repo_path: &Path,
    range: &CommitRange,
    max_diff_bytes: usize,
) -> Result<Vec<CommitRecord>, WalkError> {
    // Implementation hint: open with gix::open, resolve revs via
    // gix::Repository::rev_parse_single, walk with .rev_walk(...).
    // For SinceDuration, filter by commit time post-walk.
    // Hash author email with sha2::Sha256.
    // Compute diff via the gix tree-diff API; if total bytes > max_diff_bytes,
    // set diff_truncated=true and replace unified_diff_text with the file list.
    todo!("see plan A7")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::range::CommitRange;

    #[test]
    fn iter_walks_smoke_repo() {
        let (_g, path) = crate::tests_support::make_smoke_repo();
        let range = CommitRange::Refs { since: "HEAD~5".into(), until: "HEAD".into() };
        let v = iter_commits(&path, &range, 64 * 1024).unwrap();
        assert_eq!(v.len(), 5);
        assert!(v.iter().all(|c| !c.author_email_sha256.is_empty()));
        // Newest-first
        assert!(v[0].commit_ts >= v[4].commit_ts);
    }

    #[test]
    fn diff_truncation_kicks_in() {
        let (_g, path) = crate::tests_support::make_smoke_repo();
        let range = CommitRange::Refs { since: "HEAD~5".into(), until: "HEAD".into() };
        let v = iter_commits(&path, &range, 1).unwrap();
        assert!(v.iter().any(|c| c.diff_truncated));
    }
}

// Bridge to tests/support/mod.rs
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod tests_support;
```

Run: `cargo test -p vox-effort-audit walk::tests`
Expected: FAIL with `todo!`.

- [ ] **Step 3: Implement `iter_commits` against gix**

Replace the `todo!()` with a real implementation. Key gix calls (validate against `gix 0.70` docs as you write):

```rust
let repo = gix::open(repo_path).map_err(|e| WalkError::Open(e.to_string()))?;

let (since_id, until_id) = match range {
    CommitRange::Refs { since, until } => (
        repo.rev_parse_single(since.as_str()).map_err(|e| WalkError::Walk(e.to_string()))?.detach(),
        repo.rev_parse_single(until.as_str()).map_err(|e| WalkError::Walk(e.to_string()))?.detach(),
    ),
    CommitRange::SinceDuration { duration, until } => {
        let until_id = repo.rev_parse_single(until.as_str()).map_err(|e| WalkError::Walk(e.to_string()))?.detach();
        let cutoff = chrono::Utc::now() - *duration;
        // Walk all from until, filter by time below.
        (until_id, until_id)
    }
};

let walk = repo.rev_walk([until_id]).sorting(gix::revision::walk::Sorting::ByCommitTimeNewestFirst);
// Stop at since for Refs; for SinceDuration filter by commit_ts.

// For each commit:
//   - sha = commit.id().to_hex().to_string()
//   - parent_sha = commit.parent_ids().next().map(|p| p.to_hex().to_string())
//   - author = commit.author()?; email_sha256 = sha2::Sha256::digest(author.email)
//   - message = commit.message()?.summary().to_string() + body
//   - files / additions / deletions via repo.diff_tree_to_tree(parent_tree, this_tree, ...)
//   - unified_diff via gix's text-diff pretty printer
//   - if diff bytes > max_diff_bytes, truncate
```

Run: `cargo test -p vox-effort-audit walk::tests`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): gix-backed commit walker (A7)"
```

---

### Task A8: Implement `shape.rs` (heuristic features)

**Files:**
- Replace stub: `crates/vox-effort-audit/src/shape.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Local heuristic shape features computed without LLM.

use crate::walk::CommitRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShapeFeatures {
    pub additions: u64,
    pub deletions: u64,
    pub files_changed: u64,
    pub file_extension_histogram: HashMap<String, u32>,
    pub mechanical_sweep_score: f32,
    pub is_lockfile_only: bool,
    pub is_generated_only: bool,
    pub is_doc_only: bool,
    pub commit_kind_from_message: CommitKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitKind { Feat, Fix, Chore, Refactor, Docs, Test, Style, Ci, Other }

pub fn features(rec: &CommitRecord) -> ShapeFeatures { todo!("see plan A8") }

#[cfg(test)]
mod tests {
    // Inline fixtures: build CommitRecord directly. No git needed.
    use super::*;
    use crate::walk::{CommitRecord, FileChange};
    use chrono::TimeZone;

    fn rec(msg: &str, files: Vec<(&str, u64, u64)>, diff: &str) -> CommitRecord {
        CommitRecord {
            sha: "0".into(),
            parent_sha: None,
            commit_ts: chrono::Utc.timestamp_opt(0, 0).unwrap(),
            message: msg.into(),
            author_email_sha256: "x".into(),
            additions: files.iter().map(|(_, a, _)| *a).sum(),
            deletions: files.iter().map(|(_, _, d)| *d).sum(),
            files: files.iter().map(|(p, a, d)| FileChange {
                path: (*p).into(), additions: *a, deletions: *d,
            }).collect(),
            unified_diff_text: diff.into(),
            diff_truncated: false,
        }
    }

    #[test]
    fn lockfile_only_detection() {
        let r = rec("chore: bump deps", vec![("Cargo.lock", 4, 4)], "");
        let f = features(&r);
        assert!(f.is_lockfile_only);
        assert!(!f.is_doc_only);
    }

    #[test]
    fn doc_only_detection() {
        let r = rec("docs: fix typo", vec![("README.md", 1, 1), ("docs/x.md", 2, 0)], "");
        assert!(features(&r).is_doc_only);
    }

    #[test]
    fn commit_kind_from_conventional() {
        assert_eq!(features(&rec("fix(foo): bar", vec![], "")).commit_kind_from_message, CommitKind::Fix);
        assert_eq!(features(&rec("refactor!: drop", vec![], "")).commit_kind_from_message, CommitKind::Refactor);
        assert_eq!(features(&rec("random text", vec![], "")).commit_kind_from_message, CommitKind::Other);
    }

    #[test]
    fn mechanical_sweep_score_high_on_repetition() {
        let big = "-    pub const T: u64 = 30;\n+    pub const T: u64 = vox_config::T;\n".repeat(50);
        let r = rec("refactor: sweep", vec![], &big);
        let s = features(&r).mechanical_sweep_score;
        assert!(s > 0.7, "score was {s}, expected > 0.7");
    }

    #[test]
    fn mechanical_sweep_score_low_on_varied() {
        let varied = "+ fn alpha() {}\n+ fn beta(x: i32) {}\n+ struct Gamma;\n+ impl Gamma { fn delta(&self) {} }\n";
        let s = features(&rec("feat: misc", vec![], varied)).mechanical_sweep_score;
        assert!(s < 0.3, "score was {s}, expected < 0.3");
    }
}
```

Run: `cargo test -p vox-effort-audit shape::tests`
Expected: FAIL.

- [ ] **Step 2: Implement `features`**

Replace the `todo!()`. Sketch:

```rust
pub fn features(rec: &CommitRecord) -> ShapeFeatures {
    let kind = parse_commit_kind(&rec.message);
    let mut hist = HashMap::new();
    for f in &rec.files {
        if let Some(ext) = std::path::Path::new(&f.path).extension().and_then(|s| s.to_str()) {
            *hist.entry(ext.to_string()).or_insert(0) += 1;
        }
    }
    let lockfiles = ["Cargo.lock", "pnpm-lock.yaml", "package-lock.json", "uv.lock"];
    let is_lockfile_only = !rec.files.is_empty() && rec.files.iter().all(|f| {
        lockfiles.iter().any(|l| f.path.ends_with(l))
    });
    let is_doc_only = !rec.files.is_empty() && rec.files.iter().all(|f| {
        f.path.starts_with("docs/") || f.path.ends_with(".md")
    });
    let is_generated_only = !rec.files.is_empty() && rec.files.iter().all(|f| {
        f.path.contains(".generated.") || /* TODO: header check */ false
    });
    let mechanical_sweep_score = compute_repetition(&rec.unified_diff_text);

    ShapeFeatures {
        additions: rec.additions, deletions: rec.deletions,
        files_changed: rec.files.len() as u64,
        file_extension_histogram: hist,
        mechanical_sweep_score,
        is_lockfile_only, is_generated_only, is_doc_only,
        commit_kind_from_message: kind,
    }
}

fn parse_commit_kind(msg: &str) -> CommitKind {
    let first = msg.split('\n').next().unwrap_or("");
    let prefix = first.split(|c: char| c == ':' || c == '(' || c == '!').next().unwrap_or("");
    match prefix.trim().to_lowercase().as_str() {
        "feat" => CommitKind::Feat,
        "fix" => CommitKind::Fix,
        "chore" => CommitKind::Chore,
        "refactor" => CommitKind::Refactor,
        "docs" => CommitKind::Docs,
        "test" => CommitKind::Test,
        "style" => CommitKind::Style,
        "ci" => CommitKind::Ci,
        _ => CommitKind::Other,
    }
}

fn compute_repetition(diff: &str) -> f32 {
    // Cheap proxy: count duplicate lines (excluding "+++"/"---" headers) /
    // total non-header lines. Robust enough for "same edit 50 times".
    let mut counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    let mut total = 0u32;
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
            continue;
        }
        if line.starts_with('+') || line.starts_with('-') {
            *counts.entry(line).or_insert(0) += 1;
            total += 1;
        }
    }
    if total == 0 { return 0.0; }
    let max_dup = counts.values().copied().max().unwrap_or(1);
    (max_dup as f32 - 1.0) / total as f32 * (counts.len() as f32 / total.max(1) as f32).recip().min(2.0)
}
```

Run: `cargo test -p vox-effort-audit shape::tests`
Expected: PASS (5 tests). If `mechanical_sweep_score` thresholds fail, tune the formula until both the "high" and "low" tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): heuristic shape features (A8)"
```

---

## Phase B — Judge stack

### Task B1: `judge/schema.rs` — finding schema + enums

**Files:**
- Replace stub: `crates/vox-effort-audit/src/judge/mod.rs` (just `pub mod schema;` for now)
- Create: `crates/vox-effort-audit/src/judge/schema.rs`

- [ ] **Step 1: Write the failing test**

`schema.rs`:

```rust
//! Public finding schema (stable contract for S2–S4).

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum WasteCategory {
    MechanicalSweep,
    MissingProjectConvention,
    LinterGap,
    LowLeverageDebugging,
    ExploratoryDeadEnd,
    LegitFeatureWork,
    LegitBugfix,
    LegitDocs,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum RemediationKind {
    ScriptAutomation,
    AgentsMdRule,
    LinterRule,
    CorpusNegativeExample,
    NoneNeeded,
    Unknown,
}

/// What the judge actually outputs (the inner `finding` object on JSONL rows).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JudgeFinding {
    pub waste_score: u8,                          // 0..=10 inclusive
    pub waste_category: WasteCategory,
    pub suggested_remediation_kind: RemediationKind,
    pub rationale_one_line: String,
    #[serde(default)]
    pub evidence_pointers: Vec<String>,
}

/// JSON Schema string for use with `LlmConfig.response_format`.
pub fn judge_finding_json_schema() -> serde_json::Value {
    serde_json::json!({
      "type": "object",
      "properties": {
        "waste_score": { "type": "integer", "minimum": 0, "maximum": 10 },
        "waste_category": { "type": "string", "enum": [
          "MechanicalSweep","MissingProjectConvention","LinterGap","LowLeverageDebugging",
          "ExploratoryDeadEnd","LegitFeatureWork","LegitBugfix","LegitDocs","Other"
        ]},
        "suggested_remediation_kind": { "type": "string", "enum": [
          "ScriptAutomation","AgentsMdRule","LinterRule","CorpusNegativeExample","NoneNeeded","Unknown"
        ]},
        "rationale_one_line": { "type": "string", "maxLength": 240 },
        "evidence_pointers": { "type": "array", "items": { "type": "string" } }
      },
      "required": ["waste_score","waste_category","suggested_remediation_kind","rationale_one_line"],
      "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_judge_finding() {
        let f = JudgeFinding {
            waste_score: 8, waste_category: WasteCategory::MechanicalSweep,
            suggested_remediation_kind: RemediationKind::ScriptAutomation,
            rationale_one_line: "same edit ×50".into(),
            evidence_pointers: vec!["crates/x:42".into()],
        };
        let j = serde_json::to_string(&f).unwrap();
        assert!(j.contains("\"MechanicalSweep\""));
        let back: JudgeFinding = serde_json::from_str(&j).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn schema_lists_all_enum_variants() {
        let s = judge_finding_json_schema();
        let cats = s["properties"]["waste_category"]["enum"].as_array().unwrap();
        assert_eq!(cats.len(), 9);
        let rems = s["properties"]["suggested_remediation_kind"]["enum"].as_array().unwrap();
        assert_eq!(rems.len(), 6);
    }
}
```

Run: `cargo test -p vox-effort-audit judge::schema::tests`
Expected: PASS (2 tests).

- [ ] **Step 2: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): JudgeFinding schema + JSON Schema (B1)"
```

---

### Task B2: `judge/prompt.rs`

**Files:**
- Create: `crates/vox-effort-audit/src/judge/prompt.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Prompt construction for the per-commit judge.

use crate::shape::ShapeFeatures;
use crate::walk::CommitRecord;
use vox_actor_runtime::llm::LlmChatMessage;

pub fn build_messages(rec: &CommitRecord, shape: &ShapeFeatures) -> Vec<LlmChatMessage> {
    let system = include_str!("prompt_system.md");
    let user = format!(
"COMMIT_SHA: {sha}
COMMIT_MESSAGE:
{msg}

SHAPE_FEATURES (locally computed, trust as ground truth):
- additions: {add}
- deletions: {del}
- files_changed: {fc}
- commit_kind_from_message: {kind:?}
- mechanical_sweep_score: {ms:.2}
- is_lockfile_only: {ll}
- is_generated_only: {gen}
- is_doc_only: {doc}

UNIFIED_DIFF (possibly truncated; see `[TRUNCATED]` marker):
```
{diff}
```

Return a single JSON object matching the schema. Be concise.",
        sha = rec.sha,
        msg = rec.message,
        add = rec.additions,
        del = rec.deletions,
        fc = rec.files.len(),
        kind = shape.commit_kind_from_message,
        ms = shape.mechanical_sweep_score,
        ll = shape.is_lockfile_only,
        gen = shape.is_generated_only,
        doc = shape.is_doc_only,
        diff = if rec.diff_truncated {
            format!("[TRUNCATED — only file list shown]\n{}",
                rec.files.iter().map(|f| format!("- {} (+{}/-{})", f.path, f.additions, f.deletions)).collect::<Vec<_>>().join("\n"))
        } else {
            rec.unified_diff_text.clone()
        },
    );
    vec![
        LlmChatMessage { role: "system".into(), content: system.into() },
        LlmChatMessage { role: "user".into(), content: user },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::CommitKind;
    use std::collections::HashMap;

    fn fake_rec() -> CommitRecord {
        crate::walk::CommitRecord {
            sha: "abc123".into(), parent_sha: None,
            commit_ts: chrono::Utc::now(), message: "refactor: foo".into(),
            author_email_sha256: "z".into(), files: vec![], additions: 10, deletions: 5,
            unified_diff_text: "diff body".into(), diff_truncated: false,
        }
    }
    fn fake_shape() -> ShapeFeatures {
        ShapeFeatures {
            additions: 10, deletions: 5, files_changed: 2,
            file_extension_histogram: HashMap::new(),
            mechanical_sweep_score: 0.85, is_lockfile_only: false,
            is_generated_only: false, is_doc_only: false,
            commit_kind_from_message: CommitKind::Refactor,
        }
    }

    #[test]
    fn includes_shape_features_in_user_prompt() {
        let m = build_messages(&fake_rec(), &fake_shape());
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].role, "system");
        assert!(m[1].content.contains("mechanical_sweep_score: 0.85"));
        assert!(m[1].content.contains("abc123"));
    }

    #[test]
    fn truncation_marker_when_truncated() {
        let mut r = fake_rec();
        r.diff_truncated = true;
        let m = build_messages(&r, &fake_shape());
        assert!(m[1].content.contains("[TRUNCATED"));
    }
}
```

- [ ] **Step 2: Author the system prompt**

Create `crates/vox-effort-audit/src/judge/prompt_system.md` (verbatim — this IS the prompt; do not abbreviate):

```markdown
You are an auditor of AI-agent token spend on a software project. For each
git commit shown, output a single JSON object scoring how much token spend
this commit likely represents and tagging the cheapest structural fix.

Calibration anchors (from MSR 2026 "When AI Code Doesn't Stick"):
- 22% of agent reverts are overengineering → MechanicalSweep / LowLeverageDebugging
- 22% functional bugs → LegitBugfix or ExploratoryDeadEnd
- 18% quality issues → LinterGap or MissingProjectConvention
- 12% dep churn → MechanicalSweep

Scoring rules:
- waste_score 0–2: legitimate, focused work (typed feature, fixed bug, real refactor)
- waste_score 3–5: useful but bloated, or near-mechanical with a few real edits
- waste_score 6–8: mostly mechanical sweep that should have been scripted, or
  long debugging trace that a missing convention would have prevented
- waste_score 9–10: pure repetition, generated-file edit-by-hand, dead-end branch

Choose suggested_remediation_kind by what would have PREVENTED this commit:
- ScriptAutomation: a small `vox run scripts/*.vox` would have done this in one commit
- AgentsMdRule: a one-paragraph rule in AGENTS.md would have made the agent skip this
- LinterRule: a vox-code-audit / clippy detector would catch this at write time
- CorpusNegativeExample: a MENS fine-tuning corpus entry showing "don't do X" would help
- NoneNeeded: legitimate, already optimal
- Unknown: cannot judge from this diff alone

Rationale rules:
- One line, ≤240 chars, plain English, no markdown
- Reference specific signals you used (file count, repetition, message prefix)
- If shape features indicate lockfile-only or generated-only, weight heavily
- If diff is [TRUNCATED], say so and base judgment on file list + shape

NEVER:
- Mention authors, emails, or "blame"
- Output anything but the JSON object
- Speculate beyond what the diff shows
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vox-effort-audit judge::prompt::tests`
Expected: PASS (2 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): judge prompt + system message (B2)"
```

---

### Task B3: `judge/parse.rs` — response parse + retry

**Files:**
- Create: `crates/vox-effort-audit/src/judge/parse.rs`

- [ ] **Step 1: Write failing test**

```rust
//! Robust parse of judge JSON response with one schema-error retry.

use crate::judge::schema::{JudgeFinding, judge_finding_json_schema};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("json parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("schema validation failed: {0}")]
    Schema(String),
}

/// Parse a judge response into a JudgeFinding. Strips common LLM artifacts
/// (leading ```json fences) before parsing.
pub fn parse(raw: &str) -> Result<JudgeFinding, ParseError> {
    let cleaned = strip_fence(raw);
    let v: serde_json::Value = serde_json::from_str(cleaned)?;
    validate_against_schema(&v)?;
    let f: JudgeFinding = serde_json::from_value(v)?;
    Ok(f)
}

/// Builds a corrective user message for a retry round, given the validator error.
pub fn retry_message(err: &ParseError) -> String {
    format!(
        "Your last response failed validation: {err}. \
         Re-emit ONLY the JSON object matching the schema. No prose, no fences."
    )
}

fn strip_fence(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix("```json").or_else(|| s.strip_prefix("```")).unwrap_or(s);
    s.strip_suffix("```").unwrap_or(s).trim()
}

fn validate_against_schema(v: &serde_json::Value) -> Result<(), ParseError> {
    // Cheap structural check (we trust serde_json::from_value below for full enum check).
    // Only enforce the constraints serde would not catch.
    let score = v.get("waste_score").and_then(|s| s.as_u64())
        .ok_or_else(|| ParseError::Schema("missing waste_score".into()))?;
    if score > 10 {
        return Err(ParseError::Schema(format!("waste_score {score} > 10")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_finding() {
        let raw = r#"{"waste_score":4,"waste_category":"LegitBugfix","suggested_remediation_kind":"NoneNeeded","rationale_one_line":"ok"}"#;
        let f = parse(raw).unwrap();
        assert_eq!(f.waste_score, 4);
    }

    #[test]
    fn strips_fence() {
        let raw = "```json\n{\"waste_score\":1,\"waste_category\":\"LegitDocs\",\"suggested_remediation_kind\":\"NoneNeeded\",\"rationale_one_line\":\"x\"}\n```";
        assert!(parse(raw).is_ok());
    }

    #[test]
    fn rejects_score_above_10() {
        let raw = r#"{"waste_score":11,"waste_category":"Other","suggested_remediation_kind":"Unknown","rationale_one_line":"x"}"#;
        assert!(matches!(parse(raw), Err(ParseError::Schema(_))));
    }

    #[test]
    fn retry_message_mentions_error() {
        let e = ParseError::Schema("waste_score 11 > 10".into());
        let m = retry_message(&e);
        assert!(m.contains("11"));
    }
}
```

Run: `cargo test -p vox-effort-audit judge::parse::tests`
Expected: PASS (4 tests).

- [ ] **Step 2: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): judge response parse + retry message (B3)"
```

---

### Task B4: `judge/mod.rs` — Judge trait + real + Mock

**Files:**
- Modify: `crates/vox-effort-audit/src/judge/mod.rs`

- [ ] **Step 1: Write failing test**

```rust
//! Per-commit judge pipeline.

pub mod prompt;
pub mod parse;
pub mod schema;

use crate::config::JudgeConfig;
use crate::shape::ShapeFeatures;
use crate::walk::CommitRecord;
use crate::judge::schema::JudgeFinding;
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct JudgeOutcome {
    pub finding: Option<JudgeFinding>,
    pub model_id: String,
    pub latency_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub status: JudgeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JudgeStatus {
    Judged,
    Failed(String),
    Skipped(SkipReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason { BudgetExhausted, DryRun }

#[async_trait]
pub trait Judge: Send + Sync {
    async fn judge_one(&self, rec: &CommitRecord, shape: &ShapeFeatures) -> JudgeOutcome;
    fn model_id(&self) -> &str;
}

/// Deterministic in-memory judge for tests.
pub struct MockJudge {
    pub fixed_score: u8,
    pub model: String,
}

#[async_trait]
impl Judge for MockJudge {
    async fn judge_one(&self, rec: &CommitRecord, shape: &ShapeFeatures) -> JudgeOutcome {
        use schema::*;
        let kind = if shape.mechanical_sweep_score > 0.7 {
            RemediationKind::ScriptAutomation
        } else if shape.is_doc_only {
            RemediationKind::NoneNeeded
        } else {
            RemediationKind::Unknown
        };
        JudgeOutcome {
            finding: Some(JudgeFinding {
                waste_score: self.fixed_score,
                waste_category: if shape.is_doc_only { WasteCategory::LegitDocs } else { WasteCategory::Other },
                suggested_remediation_kind: kind,
                rationale_one_line: format!("mock judgement of {}", &rec.sha[..7.min(rec.sha.len())]),
                evidence_pointers: vec![],
            }),
            model_id: self.model.clone(),
            latency_ms: 0,
            input_tokens: 0,
            output_tokens: 0,
            status: JudgeStatus::Judged,
        }
    }
    fn model_id(&self) -> &str { &self.model }
}

/// Real judge wired through vox-actor-runtime::llm with the chosen model.
pub struct LlmJudge {
    pub config: JudgeConfig,
    pub resolved_model: String,
    pub timeout: Duration,
}

#[async_trait]
impl Judge for LlmJudge {
    async fn judge_one(&self, rec: &CommitRecord, shape: &ShapeFeatures) -> JudgeOutcome {
        // Build messages, call vox_actor_runtime::llm::infer_with_retry, parse, retry once on schema fail.
        todo!("see plan B4 step 2")
    }
    fn model_id(&self) -> &str { &self.resolved_model }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::{ShapeFeatures, CommitKind};
    use std::collections::HashMap;

    fn fake() -> (CommitRecord, ShapeFeatures) {
        let r = crate::walk::CommitRecord {
            sha: "deadbeef".into(), parent_sha: None,
            commit_ts: chrono::Utc::now(), message: "refactor: mass sweep".into(),
            author_email_sha256: "z".into(), files: vec![], additions: 100, deletions: 100,
            unified_diff_text: "".into(), diff_truncated: false,
        };
        let s = ShapeFeatures {
            additions: 100, deletions: 100, files_changed: 50,
            file_extension_histogram: HashMap::new(),
            mechanical_sweep_score: 0.9, is_lockfile_only: false,
            is_generated_only: false, is_doc_only: false,
            commit_kind_from_message: CommitKind::Refactor,
        };
        (r, s)
    }

    #[tokio::test]
    async fn mock_judge_routes_high_sweep_to_script() {
        let j = MockJudge { fixed_score: 8, model: "mock".into() };
        let (r, s) = fake();
        let out = j.judge_one(&r, &s).await;
        assert_eq!(out.status, JudgeStatus::Judged);
        assert_eq!(out.finding.unwrap().suggested_remediation_kind, schema::RemediationKind::ScriptAutomation);
    }
}
```

Add `async-trait` to `[dependencies]` (workspace version).

Run: `cargo test -p vox-effort-audit judge::tests`
Expected: PASS (the MockJudge test). LlmJudge has `todo!()` but is not exercised yet.

- [ ] **Step 2: Implement `LlmJudge::judge_one` against the facade**

Replace the `todo!()` body:

```rust
let started = std::time::Instant::now();
let mut messages = prompt::build_messages(rec, shape);
let llm_config = vox_actor_runtime::llm::LlmConfig {
    provider: "auto".into(),
    model: self.resolved_model.clone(),
    cost_per_1k: None,
    base_url: None,
    api_key: None,
    temperature: Some(0.0),
    top_p: None,
    max_tokens: Some(512),
    response_format: Some(schema::judge_finding_json_schema()),
    timeout_ms: Some(self.timeout.as_millis() as u64),
    telemetry_session_id: None,
    telemetry_user_id: None,
    telemetry_task_category: Some("CodeEffortJudge".into()),
    telemetry_strength_tag: None,
    telemetry_trace_id: None,
    telemetry_attempt_number: Some(1),
    telemetry_skip_interaction: false,
};

let mut attempts = 0u32;
loop {
    attempts += 1;
    let resp = match vox_actor_runtime::llm::infer_with_retry(messages.clone(), llm_config.clone()).await {
        Ok(r) => r,
        Err(e) => {
            return JudgeOutcome {
                finding: None,
                model_id: self.resolved_model.clone(),
                latency_ms: started.elapsed().as_millis() as u64,
                input_tokens: 0, output_tokens: 0,
                status: JudgeStatus::Failed(format!("llm error: {e}")),
            };
        }
    };
    match parse::parse(&resp.text) {
        Ok(finding) => {
            return JudgeOutcome {
                finding: Some(finding),
                model_id: self.resolved_model.clone(),
                latency_ms: started.elapsed().as_millis() as u64,
                input_tokens: resp.usage.input_tokens,
                output_tokens: resp.usage.output_tokens,
                status: JudgeStatus::Judged,
            };
        }
        Err(e) if attempts <= self.config.schema_retry_limit => {
            messages.push(vox_actor_runtime::llm::LlmChatMessage {
                role: "user".into(),
                content: parse::retry_message(&e),
            });
            continue;
        }
        Err(e) => {
            return JudgeOutcome {
                finding: None,
                model_id: self.resolved_model.clone(),
                latency_ms: started.elapsed().as_millis() as u64,
                input_tokens: 0, output_tokens: 0,
                status: JudgeStatus::Failed(format!("parse error after {attempts} attempts: {e}")),
            };
        }
    }
}
```

The exact field names on `LlmResponse` / `LlmUsage` may differ — check `crates/vox-actor-runtime/src/llm/types.rs` and adjust. Do NOT bypass the facade.

- [ ] **Step 3: Run lint guard**

Run: `cargo run -q -p vox-cli -- ci code-audit --filter llm_provider_call`
Expected: zero findings against `crates/vox-effort-audit/`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-effort-audit Cargo.toml
git commit -m "feat(vox-effort-audit): Judge trait + MockJudge + LlmJudge (B4)"
```

---

## Phase C — Hybrid cost signal

### Task C1: `hybrid/transcripts.rs`

**Files:**
- Replace stub: `crates/vox-effort-audit/src/hybrid/mod.rs`
- Create: `crates/vox-effort-audit/src/hybrid/transcripts.rs`
- Create: `crates/vox-effort-audit/tests/fixtures/transcripts/sample.jsonl`

- [ ] **Step 1: Fixture transcript**

Write `tests/fixtures/transcripts/sample.jsonl` with 5 lines, each a JSON object roughly shaped like a Claude Code transcript message:

```json
{"ts":"2026-05-28T14:00:00Z","cwd":"/c/Users/Owner/vox","session_id":"S1","role":"user","usage":{"input_tokens":100,"output_tokens":0}}
{"ts":"2026-05-28T14:00:30Z","cwd":"/c/Users/Owner/vox","session_id":"S1","role":"assistant","usage":{"input_tokens":0,"output_tokens":400}}
{"ts":"2026-05-28T14:05:00Z","cwd":"/c/Users/Owner/other","session_id":"S2","role":"user","usage":{"input_tokens":50,"output_tokens":0}}
{"ts":"2026-05-28T14:30:00Z","cwd":"/c/Users/Owner/vox","session_id":"S3","role":"user","usage":{"input_tokens":1000,"output_tokens":0}}
{"ts":"2026-05-28T14:30:30Z","cwd":"/c/Users/Owner/vox","session_id":"S3","role":"assistant","usage":{"input_tokens":0,"output_tokens":2000}}
```

- [ ] **Step 2: Write failing test**

`hybrid/mod.rs`:

```rust
//! Hybrid cost signal: measured tokens where available, LLM estimate elsewhere.

pub mod transcripts;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MeasuredCost {
    Measured { input_tokens: u64, output_tokens: u64, source: String, session_id: String },
    Estimated { input_tokens: u64, output_tokens: u64 },
    Ambiguous,
    Unavailable,
}
```

`hybrid/transcripts.rs`:

```rust
//! Claude Code transcript correlation.

use super::MeasuredCost;
use chrono::{DateTime, Duration, Utc};
use std::path::Path;

/// Sum tokens in transcripts whose `cwd` matches `repo_root` and whose `ts`
/// falls in `[commit_ts - window, commit_ts + window]`. Returns Measured if
/// exactly one session matches, Ambiguous if more than one, Unavailable if none.
pub fn resolve_for_commit(
    transcript_dir: &Path,
    repo_root: &Path,
    commit_ts: DateTime<Utc>,
    window: Duration,
) -> MeasuredCost { todo!("see plan C1 step 3") }

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixture_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/transcripts")
    }

    #[test]
    fn measured_when_single_session_matches() {
        let ts = chrono::Utc.with_ymd_and_hms(2026, 5, 28, 14, 0, 15).unwrap();
        let m = resolve_for_commit(
            &fixture_dir(),
            std::path::Path::new("/c/Users/Owner/vox"),
            ts,
            Duration::minutes(2),
        );
        match m {
            MeasuredCost::Measured { input_tokens, output_tokens, session_id, .. } => {
                assert_eq!(input_tokens, 100);
                assert_eq!(output_tokens, 400);
                assert_eq!(session_id, "S1");
            }
            other => panic!("expected Measured, got {other:?}"),
        }
    }

    #[test]
    fn unavailable_when_no_window_match() {
        let ts = chrono::Utc.with_ymd_and_hms(2026, 5, 28, 16, 0, 0).unwrap();
        let m = resolve_for_commit(&fixture_dir(), std::path::Path::new("/c/Users/Owner/vox"), ts, Duration::minutes(2));
        assert_eq!(m, MeasuredCost::Unavailable);
    }

    #[test]
    fn unavailable_when_cwd_mismatch() {
        let ts = chrono::Utc.with_ymd_and_hms(2026, 5, 28, 14, 5, 5).unwrap();
        let m = resolve_for_commit(&fixture_dir(), std::path::Path::new("/c/Users/Owner/vox"), ts, Duration::minutes(2));
        assert_eq!(m, MeasuredCost::Unavailable);
    }
}
```

Run: `cargo test -p vox-effort-audit hybrid::`
Expected: FAIL (todo).

- [ ] **Step 3: Implement `resolve_for_commit`**

Walk `transcript_dir/**/*.jsonl`, parse each line as `serde_json::Value`, filter by `cwd == repo_root` AND `ts ∈ window`. Group by `session_id`. If >1 session group: Ambiguous. If 1 session: sum its `usage.input_tokens` + `usage.output_tokens`. If 0: Unavailable.

Run: `cargo test -p vox-effort-audit hybrid::`
Expected: PASS (3 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): Claude Code transcript correlation (C1)"
```

---

## Phase D — Output

### Task D1: `output/jsonl.rs` — streaming writer

**Files:**
- Replace stub: `crates/vox-effort-audit/src/output/mod.rs`
- Create: `crates/vox-effort-audit/src/output/jsonl.rs`

- [ ] **Step 1: Write failing test**

`output/mod.rs`:

```rust
//! Output writers: JSONL, markdown, manifest.

pub mod jsonl;
pub mod manifest;
pub mod markdown;

use serde::{Deserialize, Serialize};

/// The top-level shape of one line in `findings.jsonl`. Stable schema_version="1.0".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingRow {
    pub schema_version: String,
    pub commit_sha: String,
    pub parent_sha: Option<String>,
    pub commit_ts: chrono::DateTime<chrono::Utc>,
    pub author_email_sha256: String,
    pub branch_hint: String,
    pub message_first_line: String,
    pub shape: crate::shape::ShapeFeatures,
    pub cost: crate::hybrid::MeasuredCost,
    pub judge: JudgeMeta,
    pub finding: Option<crate::judge::schema::JudgeFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeMeta {
    pub model_id: String,
    pub latency_ms: u64,
    pub judge_input_tokens: u64,
    pub judge_output_tokens: u64,
    pub outcome: String, // "Judged" | "Failed" | "Skipped"
}
```

`output/jsonl.rs`:

```rust
use super::FindingRow;
use std::io::Write;
use std::path::Path;

/// Append-only JSONL writer. Each `append` flushes so partial progress is visible.
pub struct JsonlWriter {
    file: std::fs::File,
}

impl JsonlWriter {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { file })
    }
    pub fn append(&mut self, row: &FindingRow) -> std::io::Result<()> {
        let line = serde_json::to_string(row)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(self.file, "{line}")?;
        self.file.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hybrid::MeasuredCost;
    use crate::shape::{CommitKind, ShapeFeatures};
    use std::collections::HashMap;

    fn row() -> FindingRow {
        FindingRow {
            schema_version: "1.0".into(),
            commit_sha: "abc".into(), parent_sha: None,
            commit_ts: chrono::Utc::now(),
            author_email_sha256: "z".into(), branch_hint: "main".into(),
            message_first_line: "m".into(),
            shape: ShapeFeatures {
                additions: 0, deletions: 0, files_changed: 0,
                file_extension_histogram: HashMap::new(),
                mechanical_sweep_score: 0.0, is_lockfile_only: false,
                is_generated_only: false, is_doc_only: false,
                commit_kind_from_message: CommitKind::Other,
            },
            cost: MeasuredCost::Unavailable,
            judge: super::super::JudgeMeta {
                model_id: "mock".into(), latency_ms: 0,
                judge_input_tokens: 0, judge_output_tokens: 0,
                outcome: "Judged".into(),
            },
            finding: None,
        }
    }

    #[test]
    fn append_writes_one_line_per_row() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut w = JsonlWriter::create(tmp.path()).unwrap();
        w.append(&row()).unwrap();
        w.append(&row()).unwrap();
        let body = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(body.lines().count(), 2);
        assert!(body.lines().all(|l| l.contains("\"schema_version\":\"1.0\"")));
    }
}
```

Run: `cargo test -p vox-effort-audit output::jsonl::tests`
Expected: PASS.

- [ ] **Step 2: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): JSONL streaming writer (D1)"
```

---

### Task D2: `output/manifest.rs`

**Files:**
- Create: `crates/vox-effort-audit/src/output/manifest.rs`

- [ ] **Step 1: Write failing test**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: String,
    pub run_id: String,
    pub run_started: chrono::DateTime<chrono::Utc>,
    pub run_completed: chrono::DateTime<chrono::Utc>,
    pub vox_version: String,
    pub effort_audit_crate_version: String,
    pub range: RangeManifest,
    pub commits_in_range: u64,
    pub commits_judged: u64,
    pub commits_skipped: u64,
    pub judge_model_id_resolved: String,
    pub judge_total_input_tokens: u64,
    pub judge_total_output_tokens: u64,
    pub judge_total_estimated_usd: f64,
    pub hybrid_coverage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeManifest {
    pub since: String,
    pub until: String,
    pub resolved_since_sha: Option<String>,
    pub resolved_until_sha: Option<String>,
}

pub fn write(path: &std::path::Path, m: &Manifest) -> std::io::Result<()> {
    if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
    let j = serde_json::to_string_pretty(m)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, j)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trips() {
        let m = Manifest {
            schema_version: "1.0".into(), run_id: "01HW7".into(),
            run_started: chrono::Utc::now(), run_completed: chrono::Utc::now(),
            vox_version: "0.6.0".into(), effort_audit_crate_version: "0.1.0".into(),
            range: RangeManifest { since: "30 days ago".into(), until: "HEAD".into(),
                resolved_since_sha: None, resolved_until_sha: None },
            commits_in_range: 10, commits_judged: 10, commits_skipped: 0,
            judge_model_id_resolved: "mock".into(),
            judge_total_input_tokens: 100, judge_total_output_tokens: 50,
            judge_total_estimated_usd: 0.05, hybrid_coverage_percent: 30.0,
        };
        let tmp = tempfile::NamedTempFile::new().unwrap();
        write(tmp.path(), &m).unwrap();
        let back: Manifest = serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();
        assert_eq!(back.run_id, "01HW7");
    }
}
```

Run: `cargo test -p vox-effort-audit output::manifest::tests`
Expected: PASS.

- [ ] **Step 2: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): run manifest writer (D2)"
```

---

### Task D3: `output/markdown.rs` — report renderer with snapshot tests

**Files:**
- Create: `crates/vox-effort-audit/src/output/markdown.rs`
- Create: `crates/vox-effort-audit/src/output/snapshots/` (insta will populate)

- [ ] **Step 1: Write failing snapshot test**

```rust
use super::{FindingRow, JudgeMeta};
use crate::judge::schema::{JudgeFinding, RemediationKind, WasteCategory};

pub fn render(rows: &[FindingRow], top_n: usize) -> String {
    let mut s = String::new();
    s.push_str("# Effort Audit Report\n\n");

    // 1. Run summary
    let total = rows.len();
    let judged = rows.iter().filter(|r| r.judge.outcome == "Judged").count();
    s.push_str(&format!("- Commits judged: {judged} / {total}\n\n"));

    // 2. Top-N
    s.push_str("## Top commits by waste_score\n\n");
    let mut ranked: Vec<&FindingRow> = rows.iter()
        .filter(|r| r.finding.is_some()).collect();
    ranked.sort_by_key(|r| std::cmp::Reverse(r.finding.as_ref().map(|f| f.waste_score).unwrap_or(0)));
    for r in ranked.iter().take(top_n) {
        let f = r.finding.as_ref().unwrap();
        s.push_str(&format!(
            "- **[{}]** `{}` — {} ({:?})\n  - {}\n",
            f.waste_score,
            &r.commit_sha[..r.commit_sha.len().min(8)],
            r.message_first_line,
            f.suggested_remediation_kind,
            f.rationale_one_line,
        ));
    }

    // 3. Waste-category breakdown
    s.push_str("\n## Waste categories\n\n| Category | Count |\n|---|---:|\n");
    let mut cat_counts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    for r in rows.iter().filter_map(|r| r.finding.as_ref()) {
        *cat_counts.entry(format!("{:?}", r.waste_category)).or_insert(0) += 1;
    }
    for (k, v) in &cat_counts { s.push_str(&format!("| {k} | {v} |\n")); }

    // 4. Remediation kinds
    s.push_str("\n## Remediation kinds (preview for S2)\n\n| Kind | Count |\n|---|---:|\n");
    let mut rem_counts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    for r in rows.iter().filter_map(|r| r.finding.as_ref()) {
        *rem_counts.entry(format!("{:?}", r.suggested_remediation_kind)).or_insert(0) += 1;
    }
    for (k, v) in &rem_counts { s.push_str(&format!("| {k} | {v} |\n")); }

    // 5. Methodology
    s.push_str("\n## Methodology\n\nJudge model resolved via vox-orchestrator::models registry for the CodeEffortJudge task class. Hybrid signal: measured tokens from Claude Code transcripts when correlatable; LLM estimate otherwise.\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hybrid::MeasuredCost;
    use crate::shape::{CommitKind, ShapeFeatures};
    use std::collections::HashMap;

    fn synth(sha: &str, msg: &str, score: u8, cat: WasteCategory, rem: RemediationKind) -> FindingRow {
        FindingRow {
            schema_version: "1.0".into(),
            commit_sha: sha.into(), parent_sha: None,
            // Fixed timestamp so snapshot is deterministic.
            commit_ts: chrono::DateTime::parse_from_rfc3339("2026-05-28T12:00:00Z").unwrap().to_utc(),
            author_email_sha256: "0".repeat(64),
            branch_hint: "main".into(), message_first_line: msg.into(),
            shape: ShapeFeatures {
                additions: 10, deletions: 5, files_changed: 1,
                file_extension_histogram: HashMap::new(),
                mechanical_sweep_score: 0.0, is_lockfile_only: false,
                is_generated_only: false, is_doc_only: false,
                commit_kind_from_message: CommitKind::Other,
            },
            cost: MeasuredCost::Unavailable,
            judge: JudgeMeta { model_id: "mock".into(), latency_ms: 0,
                judge_input_tokens: 0, judge_output_tokens: 0, outcome: "Judged".into() },
            finding: Some(JudgeFinding {
                waste_score: score, waste_category: cat,
                suggested_remediation_kind: rem,
                rationale_one_line: format!("rationale for {sha}"),
                evidence_pointers: vec![],
            }),
        }
    }

    #[test]
    fn report_snapshot() {
        let rows = vec![
            synth("aaaaaaaa", "refactor: mass sweep", 9, WasteCategory::MechanicalSweep, RemediationKind::ScriptAutomation),
            synth("bbbbbbbb", "fix: real bug", 3, WasteCategory::LegitBugfix, RemediationKind::NoneNeeded),
            synth("cccccccc", "docs: typo", 1, WasteCategory::LegitDocs, RemediationKind::NoneNeeded),
        ];
        insta::assert_snapshot!(render(&rows, 20));
    }

    #[test]
    fn does_not_emit_author_email() {
        let rows = vec![synth("aaa", "x", 1, WasteCategory::Other, RemediationKind::Unknown)];
        let out = render(&rows, 20);
        assert!(!out.contains("@"));
        assert!(!out.contains("author"));
        assert!(!out.contains("0000000000")); // hash should not leak
    }
}
```

Run: `cargo test -p vox-effort-audit output::markdown::tests`
Expected: snapshot test creates `.snap.new` on first run; review and accept with `cargo insta accept -p vox-effort-audit`. `does_not_emit_author_email` passes immediately.

- [ ] **Step 2: Accept the snapshot after manual review**

Run: `cargo insta review -p vox-effort-audit`
Visually verify the rendered report does NOT contain author info and matches §6.2 of the spec.

- [ ] **Step 3: Commit (include the snapshot)**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): markdown report renderer + snapshot (D3)"
```

---

## Phase E — Pipeline + telemetry

### Task E1: Add telemetry event types

**Files:**
- Modify: `crates/vox-telemetry/src/events/mod.rs` (or wherever event types live — check)

- [ ] **Step 1: Locate event registry**

Run: `grep -rln "OrchSubagentDispatchEvent\|FixtureModelIntentResolvedEvent" crates/vox-telemetry/src | head -3`
Read the file. Add new event structs in the same style; match the serde patterns used.

- [ ] **Step 2: Write failing test**

In the test module of the events file:

```rust
#[test]
fn audit_effort_event_round_trips() {
    let e = AuditEffortRunStartedEvent {
        run_id: "01HW7".into(),
        range: "30 days ago..HEAD".into(),
        judge_model_id: "mock".into(),
    };
    let j = serde_json::to_string(&e).unwrap();
    let _: AuditEffortRunStartedEvent = serde_json::from_str(&j).unwrap();
    assert!(j.contains("01HW7"));
}
```

Run: `cargo test -p vox-telemetry audit_effort_event_round_trips`
Expected: FAIL.

- [ ] **Step 3: Add the event types**

Add four event structs (`AuditEffortRunStartedEvent`, `AuditEffortCommitJudgedEvent`, `AuditEffortRunCompletedEvent`, `AuditEffortRunFailedEvent`) following the existing pattern (`#[derive(Serialize, Deserialize, ...)]`). Re-export from `lib.rs` if the crate re-exports event types.

Run: `cargo test -p vox-telemetry audit_effort_event_round_trips`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-telemetry
git commit -m "feat(vox-telemetry): add audit.effort.* event types (E1)"
```

---

### Task E2: `pipeline.rs` — composition + budget tracking

**Files:**
- Replace stub: `crates/vox-effort-audit/src/pipeline.rs`

- [ ] **Step 1: Write failing integration test**

Create `crates/vox-effort-audit/tests/e2e_smoke.rs`:

```rust
//! End-to-end: run the pipeline against a fixture git repo with MockJudge,
//! assert on the emitted files.

mod support;

use std::path::PathBuf;
use vox_effort_audit::config::EffortAuditConfig;
use vox_effort_audit::judge::MockJudge;

#[tokio::test]
async fn smoke_run_produces_outputs() {
    let (_g, repo_path) = support::make_smoke_repo();
    let out_dir = tempfile::tempdir().unwrap();
    let cfg = EffortAuditConfig::default();
    let judge = Box::new(MockJudge { fixed_score: 5, model: "mock".into() });

    let summary = vox_effort_audit::run(
        &repo_path,
        out_dir.path(),
        cfg,
        judge,
        None, // no transcript dir override
    ).await.unwrap();

    assert!(out_dir.path().join("findings.jsonl").exists());
    assert!(out_dir.path().join("report.md").exists());
    assert!(out_dir.path().join("manifest.json").exists());
    assert_eq!(summary.commits_judged, 5);
}
```

Run: `cargo test -p vox-effort-audit --test e2e_smoke`
Expected: FAIL with `cannot find function 'run'` (pipeline.rs is still a stub).

- [ ] **Step 2: Implement `run`**

`pipeline.rs`:

```rust
//! Top-level `run` entry point: composes range → walk → shape → hybrid → judge → emit.

use crate::config::EffortAuditConfig;
use crate::judge::{Judge, JudgeStatus};
use crate::output::{FindingRow, JudgeMeta, manifest::{Manifest, RangeManifest}};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub run_id: String,
    pub commits_in_range: u64,
    pub commits_judged: u64,
    pub commits_skipped: u64,
}

pub async fn run(
    repo_path: &Path,
    out_dir: &Path,
    cfg: EffortAuditConfig,
    judge: Box<dyn Judge>,
    transcript_dir_override: Option<PathBuf>,
) -> anyhow::Result<RunSummary> {
    let run_id = uuid::Uuid::now_v7().to_string();
    let started = chrono::Utc::now();

    // 1. range → walk
    let range = crate::range::resolve(None, None, &cfg.default_since)?;
    let commits = crate::walk::iter_commits(repo_path, &range, cfg.max_diff_bytes)?;
    let total = commits.len() as u64;

    // 2. emit setup
    std::fs::create_dir_all(out_dir)?;
    let mut writer = crate::output::jsonl::JsonlWriter::create(&out_dir.join("findings.jsonl"))?;
    let transcript_dir = transcript_dir_override.unwrap_or(cfg.transcript_dir.clone());

    let mut judged = 0u64;
    let mut skipped = 0u64;
    let mut total_in = 0u64;
    let mut total_out = 0u64;
    let mut measured_count = 0u64;
    let mut tokens_spent = 0u64;

    let mut rows_for_report: Vec<FindingRow> = Vec::with_capacity(commits.len() as usize);

    // 3. per-commit pipeline (sequential first pass; concurrency added in E3 if needed)
    for rec in &commits {
        let shape = crate::shape::features(rec);
        let cost = if cfg.with_transcripts {
            crate::hybrid::transcripts::resolve_for_commit(
                &transcript_dir, repo_path, rec.commit_ts,
                chrono::Duration::minutes(10),
            )
        } else {
            crate::hybrid::MeasuredCost::Unavailable
        };
        if matches!(cost, crate::hybrid::MeasuredCost::Measured { .. }) {
            measured_count += 1;
        }

        // Budget check
        if tokens_spent >= cfg.judge.max_total_tokens {
            let row = build_row(rec, &shape, &cost, &JudgeMeta {
                model_id: judge.model_id().into(), latency_ms: 0,
                judge_input_tokens: 0, judge_output_tokens: 0,
                outcome: "Skipped".into(),
            }, None);
            writer.append(&row)?;
            rows_for_report.push(row);
            skipped += 1;
            continue;
        }

        let outcome = judge.judge_one(rec, &shape).await;
        total_in += outcome.input_tokens;
        total_out += outcome.output_tokens;
        tokens_spent += outcome.input_tokens + outcome.output_tokens;

        let meta = JudgeMeta {
            model_id: outcome.model_id.clone(),
            latency_ms: outcome.latency_ms,
            judge_input_tokens: outcome.input_tokens,
            judge_output_tokens: outcome.output_tokens,
            outcome: match outcome.status {
                JudgeStatus::Judged => "Judged".into(),
                JudgeStatus::Failed(_) => "Failed".into(),
                JudgeStatus::Skipped(_) => "Skipped".into(),
            },
        };
        let row = build_row(rec, &shape, &cost, &meta, outcome.finding);
        writer.append(&row)?;
        rows_for_report.push(row);
        match outcome.status {
            JudgeStatus::Judged => judged += 1,
            _ => skipped += 1,
        }
    }

    // 4. report + manifest
    std::fs::write(out_dir.join("report.md"),
        crate::output::markdown::render(&rows_for_report, cfg.report_top_n))?;

    let manifest = Manifest {
        schema_version: "1.0".into(),
        run_id: run_id.clone(),
        run_started: started,
        run_completed: chrono::Utc::now(),
        vox_version: env!("CARGO_PKG_VERSION").into(),
        effort_audit_crate_version: env!("CARGO_PKG_VERSION").into(),
        range: RangeManifest {
            since: cfg.default_since.clone(),
            until: "HEAD".into(),
            resolved_since_sha: commits.last().map(|c| c.sha.clone()),
            resolved_until_sha: commits.first().map(|c| c.sha.clone()),
        },
        commits_in_range: total,
        commits_judged: judged,
        commits_skipped: skipped,
        judge_model_id_resolved: judge.model_id().into(),
        judge_total_input_tokens: total_in,
        judge_total_output_tokens: total_out,
        judge_total_estimated_usd: 0.0,  // S1: leave 0; cost computation lands with S3
        hybrid_coverage_percent: if total > 0 { (measured_count as f64 / total as f64) * 100.0 } else { 0.0 },
    };
    crate::output::manifest::write(&out_dir.join("manifest.json"), &manifest)?;

    Ok(RunSummary { run_id, commits_in_range: total, commits_judged: judged, commits_skipped: skipped })
}

fn build_row(
    rec: &crate::walk::CommitRecord,
    shape: &crate::shape::ShapeFeatures,
    cost: &crate::hybrid::MeasuredCost,
    judge: &JudgeMeta,
    finding: Option<crate::judge::schema::JudgeFinding>,
) -> FindingRow {
    FindingRow {
        schema_version: "1.0".into(),
        commit_sha: rec.sha.clone(),
        parent_sha: rec.parent_sha.clone(),
        commit_ts: rec.commit_ts,
        author_email_sha256: rec.author_email_sha256.clone(),
        branch_hint: "main".into(),
        message_first_line: rec.message.lines().next().unwrap_or("").to_string(),
        shape: shape.clone(),
        cost: cost.clone(),
        judge: judge.clone(),
        finding,
    }
}
```

Add `anyhow` to `[dependencies]`. Adjust trait imports as needed (e.g. `JudgeMeta` needs `Clone`).

- [ ] **Step 3: Run the integration test**

Run: `cargo test -p vox-effort-audit --test e2e_smoke`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): pipeline composition + e2e smoke (E2)"
```

---

### Task E3: Bounded concurrency

**Files:**
- Modify: `crates/vox-effort-audit/src/pipeline.rs`

- [ ] **Step 1: Write failing performance test**

Append to `e2e_smoke.rs`:

```rust
#[tokio::test]
async fn concurrent_judge_completes_under_budget() {
    let (_g, repo_path) = support::make_smoke_repo();
    let out_dir = tempfile::tempdir().unwrap();
    let mut cfg = EffortAuditConfig::default();
    cfg.max_concurrent = 4;
    // Mock judge that sleeps 100ms per call. 5 commits sequentially = 500ms.
    // With max_concurrent=4 it should be <250ms.
    struct SlowMock;
    #[async_trait::async_trait]
    impl vox_effort_audit::judge::Judge for SlowMock {
        async fn judge_one(&self, rec: &vox_effort_audit::walk::CommitRecord, shape: &vox_effort_audit::shape::ShapeFeatures) -> vox_effort_audit::judge::JudgeOutcome {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            vox_effort_audit::judge::MockJudge { fixed_score: 3, model: "slow".into() }.judge_one(rec, shape).await
        }
        fn model_id(&self) -> &str { "slow" }
    }
    let started = std::time::Instant::now();
    let _ = vox_effort_audit::run(&repo_path, out_dir.path(), cfg, Box::new(SlowMock), None).await.unwrap();
    let elapsed = started.elapsed();
    assert!(elapsed < std::time::Duration::from_millis(400), "elapsed: {elapsed:?}");
}
```

Run: `cargo test -p vox-effort-audit --test e2e_smoke concurrent_judge_completes_under_budget`
Expected: FAIL (sequential implementation takes ~500ms).

- [ ] **Step 2: Switch the judge loop to bounded concurrency**

In `pipeline.rs`, replace the sequential `for rec in &commits` loop with a `FuturesUnordered` driven by a `tokio::sync::Semaphore::new(cfg.max_concurrent)`. Preserve ordering for the report by sorting `rows_for_report` by `commit_ts` descending after the join. Keep streaming `findings.jsonl` writes in arrival order (NOT timestamp order); the report's ranking sorts by `waste_score` anyway.

- [ ] **Step 3: Run all tests**

Run: `cargo test -p vox-effort-audit`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-effort-audit
git commit -m "feat(vox-effort-audit): bounded-concurrency judge fan-out (E3)"
```

---

## Phase F — CLI + finishing

### Task F1: `vox audit effort` CLI subcommand

**Files:**
- Create: `crates/vox-cli/src/commands/audit/mod.rs` (if `audit` is brand new)
- Create: `crates/vox-cli/src/commands/audit/effort.rs`
- Modify: `crates/vox-cli/src/commands/mod.rs` to wire `audit`
- Modify: `crates/vox-cli/src/cli.rs` (or wherever the clap subcommand enum lives — grep first)

- [ ] **Step 1: Locate the CLI subcommand registry**

Run: `grep -rln "Subcommand" crates/vox-cli/src/cli* | head -3`
Find the enum the existing subcommands live in. Confirm whether an `Audit { ... }` variant already exists.

- [ ] **Step 2: Write failing CLI integration test**

Create `crates/vox-cli/tests/audit_effort_cli.rs`:

```rust
#[test]
fn audit_effort_help_includes_since_flag() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["audit", "effort", "--help"])
        .output().unwrap();
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("--since"), "--since missing from help:\n{s}");
    assert!(s.contains("--limit"));
}
```

Run: `cargo test -p vox-cli --test audit_effort_cli`
Expected: FAIL (subcommand doesn't exist).

- [ ] **Step 3: Add the subcommand**

Pattern (adapt to actual clap structure):

```rust
// crates/vox-cli/src/commands/audit/effort.rs
use clap::Args;

#[derive(Args, Debug)]
pub struct EffortArgs {
    /// Range start (git ref or duration like "30 days ago" / "7d")
    #[arg(long)]
    pub since: Option<String>,
    /// Range end (git ref). Default: HEAD.
    #[arg(long)]
    pub until: Option<String>,
    /// Override the judge model (skips registry selection)
    #[arg(long)]
    pub model: Option<String>,
    /// Cap number of commits judged (for CI smoke runs)
    #[arg(long)]
    pub limit: Option<usize>,
    /// Disable transcript correlation
    #[arg(long, default_value_t = false)]
    pub no_transcripts: bool,
    /// Output directory (default: target/audit/effort/<run-id>/)
    #[arg(long)]
    pub out_dir: Option<std::path::PathBuf>,
}

pub async fn run(args: EffortArgs) -> anyhow::Result<()> {
    // 1. Load config: read vox.toml's [audit.effort] section if present, merge with EffortAuditConfig::default()
    // 2. Resolve judge model:
    //    - if args.model.is_some(): build LlmJudge with that explicit id
    //    - else: call vox-orchestrator::models::select::pick(TaskCategory::CodeEffortJudge) for the resolved id
    // 3. Build out_dir = args.out_dir.unwrap_or_else(|| PathBuf::from("target/audit/effort").join(&run_id))
    // 4. Call vox_effort_audit::run(repo_root, out_dir, cfg, judge, transcript_dir_override).await
    // 5. Print the path to report.md to stdout
    todo!("see plan F1")
}
```

Wire it in `commands/mod.rs` (or `cli.rs`):

```rust
// In the Subcommand enum:
Audit(AuditArgs),

// AuditArgs:
#[derive(Args, Debug)]
pub struct AuditArgs {
    #[command(subcommand)]
    pub command: AuditCommand,
}

#[derive(Subcommand, Debug)]
pub enum AuditCommand {
    Effort(crate::commands::audit::effort::EffortArgs),
}
```

Add `vox-effort-audit = { path = "../vox-effort-audit" }` to `vox-cli/Cargo.toml`.

- [ ] **Step 4: Implement `run`**

Replace the `todo!()`. Key calls:
- Config load: `toml::from_str(&std::fs::read_to_string("vox.toml")?)?.audit.effort.unwrap_or_default()` (or however the workspace loads root config — grep for existing patterns).
- Judge model resolution: prefer existing `vox-orchestrator::models::select` API. If a clean API doesn't yet exist for picking by `TaskCategory`, add a one-line helper there (`pub fn pick_for_category(cat: TaskCategory) -> Option<String>`). Document the addition in the commit message.

- [ ] **Step 5: Re-run the CLI test**

Run: `cargo test -p vox-cli --test audit_effort_cli`
Expected: PASS.

- [ ] **Step 6: Manual smoke**

Run: `cargo run -q -p vox-cli -- audit effort --since HEAD~3 --model mock-fixture`
Expected: prints a path; that path contains `findings.jsonl`, `report.md`, `manifest.json`. (If you don't yet have a real `mock-fixture` model registered, use `--limit 0` to skip judge calls and just verify the scaffolding.)

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli
git commit -m "feat(vox-cli): vox audit effort subcommand (F1)"
```

---

### Task F2: AGENTS.md §10.2 umbrella note

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: Apply edit**

Find the section listing existing audit tooling (if none exists, add it as a subsection of the existing audit-adjacent content). Insert:

```markdown
## `vox audit` Umbrella (SSOT)

The unified `vox audit` umbrella hosts:

- `vox audit code` — `vox-code-audit` source-policy detectors
- `vox audit arch` — `vox-arch-check`
- `vox audit retirement` — `vox ci retirement-audit` (planned per CR-L6)
- `vox audit effort` — AI-judged commit history audit (vox-effort-audit)

New audit subcommands MUST emit findings JSONL with `schema_version` and a per-finding discriminator so downstream tooling can multiplex.
```

- [ ] **Step 2: Commit**

```bash
git add AGENTS.md
git commit -m "docs(agents): add vox audit umbrella SSOT note (F2)"
```

---

### Task F3: Coverage floor entry

**Files:**
- Modify: `.config/coverage-gates.toml`

- [ ] **Step 1: Add the entry**

In `[crates]` table, alphabetical position:

```toml
vox-effort-audit  = 70.0   # New crate; mock-judge tests should cover the bulk. Lower if first measurement is < 70.
```

- [ ] **Step 2: Run coverage gate**

Run: `cargo run -q -p vox-cli -- ci coverage-gates --since main`
Expected: PASS, or fail with the actual measured coverage. If it fails because the threshold is too high, lower to the measured value rounded down to the nearest 5 (per the convention used for `vox-cli`). Commit the lowered value with a note in the comment.

- [ ] **Step 3: Commit**

```bash
git add .config/coverage-gates.toml
git commit -m "chore(coverage-gates): floor for vox-effort-audit (F3)"
```

---

### Task F4: Crate README

**Files:**
- Create: `crates/vox-effort-audit/README.md`

- [ ] **Step 1: Write README**

```markdown
# vox-effort-audit

AI-judged audit of git commit history. Walks commits in a range, calls the
model-agnostic judge facade per commit, optionally substitutes measured token
cost from local Claude Code transcripts, and emits a ranked report.

## CLI

```bash
vox audit effort --since "30 days ago"
vox audit effort --since v0.5.0 --until HEAD --model mens-r6.2
vox audit effort --limit 10 --no-transcripts
```

Outputs land in `target/audit/effort/<run-id>/`:
- `findings.jsonl` — one finding per commit, `schema_version = "1.0"`
- `report.md` — human-readable summary (Top-N + category breakdowns)
- `manifest.json` — run metadata (range, model, cost, coverage)

## Architecture

See `docs/superpowers/specs/2026-05-28-effort-audit-core-design.md`.

This is Slice 1 of 4. Cluster-and-route (S2), measured-cost completion (S3),
and auto-emit (S4) all consume the JSONL schema defined here.

## Live-network testing

Live judge calls cost real tokens. The default test suite uses `MockJudge`.
To run a live-network smoke against the configured judge model:

```bash
cargo test -p vox-effort-audit --features live-judge -- --ignored
```
```

- [ ] **Step 2: Commit**

```bash
git add crates/vox-effort-audit/README.md
git commit -m "docs(vox-effort-audit): crate README (F4)"
```

---

### Task F5: Acceptance gate run

**Files:** none changed; verification only.

- [ ] **Step 1: Full local CI tier**

Run: `cargo run -q -p vox-cli -- ci pre-push --full`
Expected: green. Fix anything red before claiming completion.

- [ ] **Step 2: vox-arch-check standalone**

Run: `cargo run -q -p vox-arch-check`
Expected: green; `vox-effort-audit` shows in L2 with no orphan / staleness / LoC budget warnings.

- [ ] **Step 3: vox-code-audit on the new crate**

Run: `cargo run -q -p vox-cli -- ci code-audit --crate vox-effort-audit`
Expected: zero errors. In particular: `llm_provider_call` detector finds nothing.

- [ ] **Step 4: Manual repo smoke**

Run: `cargo run -q -p vox-cli -- audit effort --since "30 days ago"`
Expected: completes under $1 of judge spend (verify in manifest.json); `report.md` does NOT mention any author, email, or hash; at least one of the recent mass-sweep refactor commits is in the Top-N with `suggested_remediation_kind = ScriptAutomation`.

- [ ] **Step 5: Push branch**

```bash
git push -u origin spec/effort-audit-core
```

- [ ] **Step 6: Open PR**

```bash
gh pr create --title "feat(vox-effort-audit): AI-judged commit-history audit (S1)" --body "$(cat <<'EOF'
## Summary

- Ships `vox-effort-audit` (new L2 crate) + `vox audit effort` CLI subcommand
- Walks git history, calls a model-agnostic LLM judge per commit, emits JSONL + markdown report + manifest
- Hybrid cost signal: measured tokens from Claude Code transcripts when correlatable; LLM estimate otherwise
- Adds `CodeEffortJudge` task category and `Model-Agnostic LLM Boundary` AGENTS.md SSOT
- S1 of a 4-slice plan; cluster-and-route (S2), measured-cost completion (S3), auto-emit (S4) defer

Spec: `docs/superpowers/specs/2026-05-28-effort-audit-core-design.md`
Plan: `docs/superpowers/plans/2026-05-28-effort-audit-core.md`

## Test plan

- [ ] `cargo test -p vox-effort-audit` passes (unit + e2e smoke)
- [ ] `cargo test -p vox-cli --test audit_effort_cli` passes
- [ ] `cargo run -p vox-arch-check` clean (layers + LoC + staleness)
- [ ] `cargo run -p vox-cli -- ci code-audit --crate vox-effort-audit` clean (esp. `llm_provider_call`)
- [ ] `cargo run -p vox-cli -- audit effort --since "30 days ago"` produces report.md with no author info and under $1 judge spend

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review (run before marking the plan ready)

**Spec coverage check:**
- §1 in-scope (CLI, range, walk, shape, hybrid, judge, output, telemetry) → Tasks A5–F1. ✓
- §2 architecture (crate placement, layers.toml, where-things-live, module layout) → A1–A2. ✓
- §3 pipeline data flow → A6–A8, C1, B1–B4, D1–D3, E2–E3. ✓
- §4 model-agnostic judge + selection + cost ceiling + concurrency → B4, E2 (budget), E3 (concurrency), F1 (model resolution). ✓
- §5 heuristic shape features → A8. ✓
- §6 output schema (JSONL row, markdown sections, manifest) → D1–D3. ✓
- §7 config (vox.toml `[audit.effort]`) → A5, F1 (load). ✓
- §8 error handling → covered across A6–E2 (each module's error enum). ✓
- §9 testing strategy → every task starts with a failing test; e2e smoke at E2; snapshot at D3; coverage floor at F3; live-judge gate at F4 README. ✓
- §10 AGENTS.md additions (10.1 already landed; 10.2 at F2; 10.3 at A2). ✓
- §11 hooks for S2–S4 → schema_version stable (D1/B1), trait-based judge/hybrid (B4/C1), JSONL-only contract. ✓
- §12 acceptance criteria → F5 walks through all 8. ✓
- §13 prior art → captured in spec, not a code task. ✓
- §14 risk register → mitigations embedded (budget at E2, scoping at C1, no-authors at D3 test, schema_version, detector clean at F5). ✓
- §15 open questions → Q1 (GitHub hyperlinks) deliberately not implemented in S1 (note in F1); Q2 (large diffs) handled in A7 truncation; Q3 (--dry-run) deferred — add stub in F1 that warns "not yet implemented" if needed, otherwise leave for a follow-up.

**Open question handling.** Q3 `--dry-run` is the one open item not covered. Resolution: skip in S1 (the `--limit 0` workaround works for free), document in F4 README as "planned".

**Placeholder scan:** searched for "TBD" / "implement later" / "fill in". None found in the plan. All `todo!()` are explicitly attached to a "Step 2: Implement X" follow-up that shows the code.

**Type consistency:** `JudgeOutcome` used in B4 and E2 has the same fields. `FindingRow` defined in D1 used by E2 and D3. `MeasuredCost` defined in C1 used in D1 and E2. Enum variant casing: PascalCase serde tags used consistently.

**Caveat for the executor:**
- The exact field names on `vox_actor_runtime::llm::LlmResponse`/`LlmUsage` were not verified against the current source; if they differ from what B4 step 2 assumes, adjust during implementation. Do not bypass the facade to "fix" it.
- The exact `vox-orchestrator::models::select` API for picking by TaskCategory is assumed; verify before implementing F1 step 4 and add a one-line helper if missing.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-28-effort-audit-core.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task (A1, A2, A3, ...) in this worktree, review between tasks, fast iteration with two-stage code review.

**2. Inline Execution** — Execute tasks in this session via `superpowers:executing-plans`, batch execution with checkpoints for review.

**Which approach?**
