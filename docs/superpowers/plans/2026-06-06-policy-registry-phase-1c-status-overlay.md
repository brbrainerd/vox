# Policy Registry — Phase 1c: Per-Branch Run-Status Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every policy a per-branch run status (`pass | fail | warn | unknown`) for the current branch and any other active worktree, so the GUI (Plan 1d) can color group counts and badge the nav. Multi-branch (worktrees) is first-class: one status file per branch under `.vox/policy-status/`. Grey `unknown` ("not run") is the honest default — **we never color a rule green without a real passing result.**

**Architecture:** A net-new `PolicyRunReport` type lives in `vox-config` next to `policy::registry` (`policy::status`), with a reader (`load_status` / multi-branch reader) — the lightweight runtime side that the GUI daemon and CLI read. `vox-cli` owns the writer (`.vox/policy-status/<sanitized-branch>.json`, MERGE-by-id so successive gate runs accumulate) and three capture seams:

1. **Per-GATE capture** — a thin dispatch *wrapper* in `crates/vox-cli/src/commands/ci/run_body.rs` records each `ci-gate`/`audit-check`/`crl-gate`'s registry id + pass/fail + duration **without** invasive per-gate edits (wrap the single `match cmd`, capture `Ok`/`Err` + the gate's id).
2. **Per-FINDING capture (code-audit)** — a `--json` path that maps the existing serde-ready `vox_code_audit::Finding` set into per-rule results (any finding for a rule → that rule failed/warned; a rule that ran with no finding → pass).
3. **Per-FINDING capture (arch)** — a `--json` flag on `vox-arch-check` emitting per-rule results from its internal `Report`.
4. **effort-audit** already writes `findings.jsonl` — reuse, do not rebuild.

`vox policy status [--branch b]…` joins the catalog (`load_policy_registry`) with the report(s); rules with no result show `unknown`.

**Tech Stack:** Rust, `serde` + `serde_json` (status files are JSON, not YAML — already a dep of both crates), `clap`. The current git branch comes via `vox_git::read_only(repo, &["rev-parse", "--abbrev-ref", "HEAD"])` (read-only allowlist already permits `rev-parse`, [`read_cmd.rs:15`](../../../crates/vox-git/src/read_cmd.rs)); worktree enumeration shells `git worktree list --porcelain` directly in `vox-cli` (writer side; `vox-cli` already spawns `git`). Tests via `cargo test -p <crate>`. Format with `cargo fmt -p <crate>` (never `--all` on Windows).

**Determinism:** `ran_at` is an ISO-8601 **string passed in by the caller**, never generated with `*::now()` inside the pure builder/merge functions. The CLI dispatch layer stamps it once per run; the merge/serialize functions are pure and table-testable.

**Scope note:** This is Phase 1c of the initiative in
[`docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md`](../specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md).
Read its **§4.5** and **§10 addendum point 3** first — they pin the verified granularity (per-GATE for ci/audit/crl via a dispatch wrapper; per-FINDING for code-audit/arch via `--json`; effort-audit already emits `findings.jsonl`; `.vox/policy-status/` is net-new; grey `unknown` is the honest default). Plan 1a (model + loader + `vox policy` CLI) has **landed** ([`crates/vox-config/src/policy/registry.rs`](../../../crates/vox-config/src/policy/registry.rs), the two `CiCmd::PolicyRegistry*` arms at [`run_body.rs:62`](../../../crates/vox-cli/src/commands/ci/run_body.rs)).

---

## Verified facts (file:line)

- **Dispatch point for `vox ci <gate>`:** [`crates/vox-cli/src/commands/ci/run_body.rs:55`](../../../crates/vox-cli/src/commands/ci/run_body.rs) — `pub async fn run(cmd: CiCmd) -> Result<()>`; `let root = repo_root();` (line 56), single `match cmd { … }` (lines 60–601), every arm returns `anyhow::Result<()>`. The wrapper records `id + Ok/Err + duration` around this match.
- **code-audit `Finding`:** [`crates/vox-code-audit/src/rules.rs:135`](../../../crates/vox-code-audit/src/rules.rs) — `#[derive(Serialize, Deserialize)]`, fields `rule_id: String`, `severity: Severity`, `file: PathBuf`, `line: usize`. `OutputFormat::{Terminal,Json,LlmJson,Markdown}` exists ([`report.rs:39`](../../../crates/vox-code-audit/src/report.rs)) and `format_json` already serializes `&[Finding]` ([`report.rs:213`](../../../crates/vox-code-audit/src/report.rs)). The engine is invoked from [`crates/vox-cli/src/commands/diagnostics/stub_check/mod.rs`](../../../crates/vox-cli/src/commands/diagnostics/stub_check/mod.rs) (`all_rules`, `ToestubEngine`, `OutputFormat` imports at lines 10–14). **There is NO `--json` flag wired onto the audit/check command** — this plan adds the policy-status emission there (it does not need a new user-facing flag; it emits as a side effect under an env/opt — see Task 5).
- **`vox-arch-check` `Report`:** [`crates/vox-arch-check/src/main.rs:234`](../../../crates/vox-arch-check/src/main.rs) — internal `struct Report` with per-rule `*_warns` vecs + `strict_*` bools; `fn main` reads `--warn-only` only ([line 216](../../../crates/vox-arch-check/src/main.rs)); `print_summary` is text/`eprintln!`-only ([line 311](../../../crates/vox-arch-check/src/main.rs)). Plan adds a `--json` flag emitting per-rule results.
- **effort-audit `findings.jsonl`:** already written by `JsonlWriter` ([`crates/vox-effort-audit/src/output/jsonl.rs:1`](../../../crates/vox-effort-audit/src/output/jsonl.rs)); the CLI prints `findings: <out_dir>/findings.jsonl` ([`crates/vox-cli/src/commands/audit_effort.rs:235`](../../../crates/vox-cli/src/commands/audit_effort.rs)). **Reuse, do not rebuild.**
- **Branch identity:** current branch via `vox_git::read_only(repo, &["rev-parse", "--abbrev-ref", "HEAD"])` ([`read_cmd.rs:11–19`, `47`](../../../crates/vox-git/src/read_cmd.rs); `pub use read_cmd::read_only` at [`lib.rs:35`](../../../crates/vox-git/src/lib.rs)). Worktrees: `git worktree list --porcelain` (not on the read-only allowlist → shell directly in `vox-cli`). Repo root: `vox_repository::resolve_repo_root_for_ci()` (used throughout `vox-cli`, e.g. [`command_catalog.rs:97`](../../../crates/vox-cli/src/command_catalog.rs)); `repo_root()` already bound in `run_body.rs`.

---

## File Structure

**Create:**
- `crates/vox-config/src/policy/status.rs` — `PolicyRunReport` + `PolicyResult` + `RunStatus` + `Hit` + `sanitize_branch` + `load_status` + `load_status_for_branches`.
- `crates/vox-cli/src/commands/policy/status_writer.rs` — branch resolution, MERGE-by-id writer, per-gate/per-finding capture builders.
- `crates/vox-arch-check/src/json_report.rs` — `--json` per-rule projection of `Report`.

**Modify:**
- `crates/vox-config/src/policy/mod.rs` — add `pub mod status;`.
- `crates/vox-config/src/lib.rs` — re-export status types.
- `crates/vox-cli/src/commands/ci/run_body.rs` — wrap the `match cmd` to capture per-gate status.
- `crates/vox-cli/src/commands/ci/cmd_enums.rs` — map each capturable `CiCmd` variant to its registry id (a `gate_policy_id()` helper).
- `crates/vox-cli/src/commands/diagnostics/stub_check/mod.rs` — emit per-rule code-audit results into the status store.
- `crates/vox-arch-check/src/main.rs` — `--json` flag → write per-rule results.
- `crates/vox-cli/src/commands/policy/mod.rs` — add `PolicyCmd::Status { branch: Vec<String> }` + render.
- `docs/src/architecture/where-things-live.md` — extend the policy-catalog row with the status store.

---

## Task 1: `PolicyRunReport` model + reader in `vox-config`

**Files:**
- Create: `crates/vox-config/src/policy/status.rs`
- Modify: `crates/vox-config/src/policy/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-config/src/policy/status.rs` with only the tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_report() {
        let json = r#"{
          "branch": "main",
          "commit": "abc123",
          "ran_at": "2026-06-06T00:00:00Z",
          "results": [
            { "id": "code-audit/stub/todo", "status": "fail", "duration_ms": 12,
              "hits": [{ "file": "src/a.rs", "line": 7, "note": "todo!()" }] },
            { "id": "ci/manifest", "status": "pass", "duration_ms": 40, "hits": [] }
          ]
        }"#;
        let r: PolicyRunReport = serde_json::from_str(json).unwrap();
        assert_eq!(r.branch, "main");
        assert_eq!(r.results.len(), 2);
        assert_eq!(r.results[0].status, RunStatus::Fail);
        assert_eq!(r.results[0].hits[0].line, 7);
        assert_eq!(r.results[1].status, RunStatus::Pass);
    }

    #[test]
    fn sanitize_branch_is_filesystem_safe() {
        assert_eq!(sanitize_branch("main"), "main");
        assert_eq!(sanitize_branch("feature/foo"), "feature-foo");
        assert_eq!(sanitize_branch("cc/bot/amazing-x"), "cc-bot-amazing-x");
        assert_eq!(sanitize_branch("a b\\c"), "a-b-c");
    }

    #[test]
    fn unknown_is_the_default_variant_for_missing_results() {
        // RunStatus::default() is the honest "not run" grey.
        assert_eq!(RunStatus::default(), RunStatus::Unknown);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-config policy::status::tests`
Expected: FAIL — `cannot find type PolicyRunReport`.

- [ ] **Step 3: Write the model + reader**

Prepend to `crates/vox-config/src/policy/status.rs` (above tests):

```rust
//! Per-branch policy run-status overlay (Phase 1c).
//!
//! One report per branch at `.vox/policy-status/<sanitized-branch>.json`, so
//! multiple worktrees/branches coexist. See spec §4.5 / §10 addendum point 3.
//!
//! Honesty contract: a result is recorded ONLY when a gate/rule actually ran.
//! Rules with no result are surfaced as `unknown` ("not run", grey) by the
//! catalog-join in `vox policy status` — never faked green.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One full run report for a single branch. Accumulates across gate runs (the
/// writer in `vox-cli` MERGES results by `id`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyRunReport {
    /// The (unsanitized) git branch this report describes.
    pub branch: String,
    /// Short commit the run observed (provenance; may be `"unknown"`).
    pub commit: String,
    /// ISO-8601 timestamp, STAMPED BY THE CALLER (never `now()` in pure code).
    pub ran_at: String,
    /// One entry per policy id that has actually run.
    #[serde(default)]
    pub results: Vec<PolicyResult>,
}

/// The recorded outcome for one policy id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyResult {
    /// Registry id (matches `PolicyEntry::id`), e.g. `code-audit/stub/todo`.
    pub id: String,
    pub status: RunStatus,
    /// Per-finding locations (empty for per-gate pass/fail).
    #[serde(default)]
    pub hits: Vec<Hit>,
    /// Wall-clock of the run that produced this result.
    #[serde(default)]
    pub duration_ms: u64,
}

/// A single finding location within a per-rule result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub file: String,
    pub line: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Pass,
    Fail,
    Warn,
    /// Honest default: the gate/rule has not produced a result on this branch.
    #[default]
    Unknown,
}

/// Directory (relative to repo root) holding per-branch status files.
pub const STATUS_DIR_REL: &str = ".vox/policy-status";

/// Sanitize a branch name into a single filesystem-safe path segment.
/// Any non `[A-Za-z0-9._-]` run collapses to a single `-`.
pub fn sanitize_branch(branch: &str) -> String {
    let mut out = String::with_capacity(branch.len());
    let mut prev_dash = false;
    for ch in branch.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Absolute path to the status file for a (sanitized) branch.
pub fn status_path(repo_root: &Path, branch: &str) -> PathBuf {
    repo_root
        .join(STATUS_DIR_REL)
        .join(format!("{}.json", sanitize_branch(branch)))
}

/// Error returned when a status file cannot be read/parsed.
#[derive(Debug)]
pub enum PolicyStatusError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for PolicyStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyStatusError::Io(e) => write!(f, "reading policy status: {e}"),
            PolicyStatusError::Parse(e) => write!(f, "parsing policy status: {e}"),
        }
    }
}
impl std::error::Error for PolicyStatusError {}

/// Load the status report for one branch. `Ok(None)` if no run has happened
/// (file absent) — the honest "nothing ran yet" state.
pub fn load_status(
    repo_root: &Path,
    branch: &str,
) -> Result<Option<PolicyRunReport>, PolicyStatusError> {
    let path = status_path(repo_root, branch);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(PolicyStatusError::Parse),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PolicyStatusError::Io(e)),
    }
}

/// Load reports for several branches at once (multi-worktree selector).
/// Each entry is `(requested_branch, Option<report>)`, preserving input order.
pub fn load_status_for_branches(
    repo_root: &Path,
    branches: &[String],
) -> Result<Vec<(String, Option<PolicyRunReport>)>, PolicyStatusError> {
    branches
        .iter()
        .map(|b| load_status(repo_root, b).map(|r| (b.clone(), r)))
        .collect()
}
```

Add to `crates/vox-config/src/policy/mod.rs`:

```rust
pub mod status;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-config policy::status::tests`
Expected: PASS (all three).

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-config
git add crates/vox-config/src/policy/status.rs crates/vox-config/src/policy/mod.rs
git commit -m "feat(vox-config): per-branch policy run-status model + reader"
```

---

## Task 2: Re-export status types + multi-branch reader test

**Files:**
- Modify: `crates/vox-config/src/lib.rs`
- Test: `crates/vox-config/src/policy/status.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `status.rs`:

```rust
#[test]
fn multi_branch_reader_isolates_per_branch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(STATUS_DIR_REL)).unwrap();
    std::fs::write(
        status_path(root, "main"),
        r#"{"branch":"main","commit":"a","ran_at":"t","results":[{"id":"ci/manifest","status":"pass","hits":[],"duration_ms":1}]}"#,
    )
    .unwrap();
    // "feature/x" sanitizes to "feature-x"; only one file exists.
    let got = load_status_for_branches(
        root,
        &["main".to_string(), "feature/x".to_string()],
    )
    .unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].0, "main");
    assert!(got[0].1.is_some(), "main report present");
    assert_eq!(got[0].1.as_ref().unwrap().results[0].status, RunStatus::Pass);
    assert_eq!(got[1].0, "feature/x");
    assert!(got[1].1.is_none(), "absent branch → None (not run)");
}
```

(`tempfile` is already a dev-dependency of `vox-config`.)

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p vox-config policy::status::tests::multi_branch_reader_isolates_per_branch`
Expected: PASS (reader already exists from Task 1).

- [ ] **Step 3: Add public re-exports**

In `crates/vox-config/src/lib.rs`, after the `pub use policy::registry::{…};` block added by Plan 1a, add:

```rust
pub use policy::status::{
    load_status, load_status_for_branches, sanitize_branch, status_path, Hit, PolicyResult,
    PolicyRunReport, PolicyStatusError, RunStatus, STATUS_DIR_REL,
};
```

- [ ] **Step 4: Verify the crate builds**

Run: `cargo build -p vox-config`
Expected: success, no unused-import warnings.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-config
git add crates/vox-config/src/lib.rs crates/vox-config/src/policy/status.rs
git commit -m "feat(vox-config): re-export status API + multi-branch reader test"
```

---

## Task 3: MERGE-by-id status writer + branch resolution in `vox-cli`

**Files:**
- Create: `crates/vox-cli/src/commands/policy/status_writer.rs`
- Modify: `crates/vox-cli/src/commands/policy/mod.rs` (add `pub mod status_writer;`)

> **Pre-check:** confirm `vox-cli` depends on `vox-git` and `vox-repository`
> (`grep -n 'vox-git\|vox-repository' crates/vox-cli/Cargo.toml`). Both are used
> elsewhere in `vox-cli`; if missing, add under `[dependencies]` with `{ workspace = true }`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/src/commands/policy/status_writer.rs`:

```rust
//! Writer for the per-branch policy run-status store (Phase 1c).
//!
//! MERGES results by id so successive gate runs accumulate into one report.
//! Timestamps are passed in by the caller (determinism: no `now()` here).

use std::path::Path;
use vox_config::{PolicyResult, PolicyRunReport, RunStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use vox_config::{load_status, status_path};

    fn res(id: &str, status: RunStatus) -> PolicyResult {
        PolicyResult { id: id.into(), status, hits: vec![], duration_ms: 1 }
    }

    #[test]
    fn merge_replaces_by_id_and_keeps_others() {
        let prior = PolicyRunReport {
            branch: "main".into(),
            commit: "a".into(),
            ran_at: "t0".into(),
            results: vec![res("ci/manifest", RunStatus::Pass), res("arch/fan_in", RunStatus::Warn)],
        };
        let merged = merge_results(
            prior,
            vec![res("ci/manifest", RunStatus::Fail), res("code-audit/stub/todo", RunStatus::Pass)],
            "main",
            "b",
            "t1",
        );
        // ci/manifest replaced; arch/fan_in untouched; stub/todo added.
        assert_eq!(merged.commit, "b");
        assert_eq!(merged.ran_at, "t1");
        let by = |id: &str| merged.results.iter().find(|r| r.id == id).map(|r| r.status);
        assert_eq!(by("ci/manifest"), Some(RunStatus::Fail));
        assert_eq!(by("arch/fan_in"), Some(RunStatus::Warn));
        assert_eq!(by("code-audit/stub/todo"), Some(RunStatus::Pass));
        assert_eq!(merged.results.len(), 3);
    }

    #[test]
    fn write_then_read_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        write_results(
            dir.path(),
            "feature/x",
            "deadbee",
            "2026-06-06T00:00:00Z",
            vec![res("ci/manifest", RunStatus::Pass)],
        )
        .unwrap();
        // Sanitized filename was used.
        assert!(status_path(dir.path(), "feature/x").exists());
        let r = load_status(dir.path(), "feature/x").unwrap().unwrap();
        assert_eq!(r.branch, "feature/x");
        assert_eq!(r.results[0].id, "ci/manifest");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli commands::policy::status_writer::tests`
Expected: FAIL — `cannot find function merge_results` / module not declared.

- [ ] **Step 3: Write the writer**

Add above the tests module in `status_writer.rs`:

```rust
/// Pure MERGE: replace any prior result with the same id, append the rest,
/// and stamp fresh provenance. `ran_at`/`commit` are passed in (no `now()`).
pub fn merge_results(
    mut report: PolicyRunReport,
    fresh: Vec<PolicyResult>,
    branch: &str,
    commit: &str,
    ran_at: &str,
) -> PolicyRunReport {
    for new in fresh {
        if let Some(slot) = report.results.iter_mut().find(|r| r.id == new.id) {
            *slot = new;
        } else {
            report.results.push(new);
        }
    }
    report.results.sort_by(|a, b| a.id.cmp(&b.id));
    report.branch = branch.to_string();
    report.commit = commit.to_string();
    report.ran_at = ran_at.to_string();
    report
}

/// Read-or-default the report for `branch`, MERGE `fresh`, write it back.
/// Atomic-ish: write to a temp file then rename.
pub fn write_results(
    repo_root: &Path,
    branch: &str,
    commit: &str,
    ran_at: &str,
    fresh: Vec<PolicyResult>,
) -> std::io::Result<()> {
    let prior = vox_config::load_status(repo_root, branch)
        .ok()
        .flatten()
        .unwrap_or_else(|| PolicyRunReport {
            branch: branch.to_string(),
            commit: commit.to_string(),
            ran_at: ran_at.to_string(),
            results: Vec::new(),
        });
    let merged = merge_results(prior, fresh, branch, commit, ran_at);
    let path = vox_config::status_path(repo_root, branch);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(&merged)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Current branch via the read-only git bridge (`rev-parse` is allowlisted).
/// Falls back to `"DETACHED"` when not on a branch / git unavailable.
pub fn current_branch(repo_root: &Path) -> String {
    vox_git::read_only(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "HEAD")
        .unwrap_or_else(|_| "DETACHED".to_string())
}

/// Short HEAD commit for provenance (best-effort).
pub fn head_commit(repo_root: &Path) -> String {
    vox_git::read_only(repo_root, &["rev-parse", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Enumerate active worktrees' branches via `git worktree list --porcelain`.
/// Not on the read-only allowlist, so shelled directly (writer side).
pub fn worktree_branches(repo_root: &Path) -> Vec<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["worktree", "list", "--porcelain"])
        .output();
    let Ok(out) = out else {
        return vec![current_branch(repo_root)];
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut branches: Vec<String> = text
        .lines()
        .filter_map(|l| l.strip_prefix("branch refs/heads/"))
        .map(|b| b.to_string())
        .collect();
    if branches.is_empty() {
        branches.push(current_branch(repo_root));
    }
    branches.sort();
    branches.dedup();
    branches
}
```

Add to `crates/vox-cli/src/commands/policy/mod.rs`:

```rust
pub mod status_writer;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-cli commands::policy::status_writer::tests`
Expected: PASS (both).

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/policy/status_writer.rs crates/vox-cli/src/commands/policy/mod.rs
git commit -m "feat(vox-cli): merge-by-id policy status writer + branch resolution"
```

---

## Task 4: Per-GATE capture wrapper in `run_body.rs`

The wrapper records each capturable `ci-gate`/`audit-check`/`crl-gate`'s registry id + pass/fail + duration **without editing each arm**. It wraps the single `match cmd`.

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (add `gate_policy_id`)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs`

- [ ] **Step 1: Write the failing test (the id map)**

Add a tests module to `crates/vox-cli/src/commands/ci/cmd_enums.rs` (or a sibling `cmd_enums_tests.rs` if the file forbids inline tests — match the file's convention):

```rust
#[cfg(test)]
mod policy_id_tests {
    use super::*;

    #[test]
    fn known_gates_map_to_registry_ids() {
        assert_eq!(CiCmd::Manifest.gate_policy_id(), Some("ci/manifest"));
        assert_eq!(
            CiCmd::SsotDrift.gate_policy_id(),
            Some("ci/ssot-drift")
        );
        // Generator/parity gates are not run-status-tracked (they ARE the catalog).
        assert_eq!(CiCmd::PolicyRegistryParity.gate_policy_id(), None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli commands::ci::cmd_enums::policy_id_tests`
Expected: FAIL — `no method gate_policy_id`.

- [ ] **Step 3: Add `gate_policy_id`**

Add an `impl CiCmd` block in `cmd_enums.rs`. Map only the **stable, registry-backed** ci-gate ids generated by Plan 1b (the `ci/<kebab>` namespace). Variants without a registry row (or that ARE the registry machinery) return `None` → not tracked (honest grey), never a faked entry:

```rust
impl CiCmd {
    /// The policy-registry id this gate is recorded under in the per-branch
    /// status store, or `None` if it is not run-status-tracked.
    ///
    /// Ids MUST match the `ci-gate` entries the Plan 1b generator emits from
    /// `contracts/operations/catalog.v1.yaml` (id scheme `ci/<command-name>`).
    /// New gates that should appear in the status overlay add a row here.
    pub fn gate_policy_id(&self) -> Option<&'static str> {
        match self {
            CiCmd::Manifest => Some("ci/manifest"),
            CiCmd::SsotDrift => Some("ci/ssot-drift"),
            CiCmd::SsotAudit => Some("ci/ssot-audit"),
            CiCmd::DeterminismAudit => Some("ci/determinism-audit"),
            CiCmd::CommandCompliance => Some("ci/command-compliance"),
            CiCmd::RepoGuards => Some("ci/repo-guards"),
            CiCmd::LineEndings { .. } => Some("ci/line-endings"),
            CiCmd::FmtCheck => Some("ci/fmt-check"),
            // … one arm per registry-backed gate. The Plan 1b parity gate
            // (extended in Task 8 below) asserts this map ⊆ the ci-gate catalog.
            // The registry machinery itself is intentionally untracked:
            CiCmd::PolicyRegistry { .. } | CiCmd::PolicyRegistryParity => None,
            _ => None,
        }
    }
}
```

> **Verification note:** the exact id string for each gate is whatever Plan 1b's
> generator produces (`ci/<command-name>`). Confirm against the generated
> `contracts/policy/policy-registry.v1.yaml` after 1b lands
> (`grep 'id: ci/' contracts/policy/policy-registry.v1.yaml`). If 1c is
> implemented before 1b's ci-gate domain, seed only the handful above and let
> Task 8's parity test gate the rest as 1b fills in.

- [ ] **Step 4: Wrap the dispatch in `run_body.rs`**

In `crates/vox-cli/src/commands/ci/run_body.rs`, the body is currently
`let result = match cmd { … };` returning `Result<()>`. Refactor `run` to:
(a) capture the gate id and start time **before** the match, (b) run the existing
match into a `result` binding, (c) record status, (d) return `result`.

Replace the `match cmd { … }` so its value is bound, then add the capture. The
existing match arms are unchanged — only the surrounding scaffolding is added:

```rust
pub async fn run(cmd: CiCmd) -> Result<()> {
    let root = repo_root();
    crate::freshness::enforce_for_ci(&root)?;

    // Per-gate status capture (Phase 1c). Only registry-backed gates are tracked;
    // others record nothing (honest grey). Disabled via VOX_NO_POLICY_STATUS=1.
    let gate_id = cmd.gate_policy_id();
    let started = std::time::Instant::now();

    let result: Result<()> = match cmd {
        // … ALL EXISTING ARMS UNCHANGED (lines 61–601) …
    };

    if let Some(id) = gate_id {
        if std::env::var("VOX_NO_POLICY_STATUS").is_err() {
            let status = if result.is_ok() {
                vox_config::RunStatus::Pass
            } else {
                vox_config::RunStatus::Fail
            };
            let duration_ms = started.elapsed().as_millis() as u64;
            let branch = crate::commands::policy::status_writer::current_branch(&root);
            let commit = crate::commands::policy::status_writer::head_commit(&root);
            let ran_at = crate::util::now_iso8601(); // caller-stamped; see note
            let _ = crate::commands::policy::status_writer::write_results(
                &root,
                &branch,
                &commit,
                &ran_at,
                vec![vox_config::PolicyResult {
                    id: id.to_string(),
                    status,
                    hits: vec![],
                    duration_ms,
                }],
            );
        }
    }

    result
}
```

> **`now_iso8601` note:** this is the single caller-stamp seam (keeps the pure
> writer deterministic). If `vox-cli` lacks a helper, add one in
> `crates/vox-cli/src/util.rs` (or the existing time util module — `grep -rn
> "now_iso8601\|to_rfc3339\|chrono::Utc::now" crates/vox-cli/src`): e.g.
> `pub fn now_iso8601() -> String { chrono::Utc::now().to_rfc3339() }`
> (`chrono` is already a `vox-cli` dep — confirm with
> `grep -n '^chrono' crates/vox-cli/Cargo.toml`; if `time` is used instead,
> mirror its rfc3339 formatter). The write is best-effort (`let _ =`): a status
> failure must never fail the gate.

- [ ] **Step 5: Run tests + a smoke gate**

```bash
cargo test -p vox-cli commands::ci::cmd_enums::policy_id_tests
cargo build -p vox-cli
cargo run -p vox-cli -- ci manifest
# verify a status file appeared for the current branch:
cargo run -p vox-cli -- policy status   # (added in Task 7)
```
Expected: the id-map test passes; after `ci manifest`, `.vox/policy-status/<branch>.json` contains a `ci/manifest` result with `status: pass`.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs crates/vox-cli/src/util.rs
git commit -m "feat(vox-cli): per-gate policy status capture wrapper around ci dispatch"
```

---

## Task 5: Per-FINDING capture for `vox-code-audit`

When the code-audit engine runs, map its `Vec<Finding>` (already serde-ready) into per-rule `PolicyResult`s and merge them into the status store. Rule with ≥1 finding → `fail` (error severity) or `warn`; a rule that ran and produced **no** finding → `pass`.

**Files:**
- Modify: `crates/vox-cli/src/commands/policy/status_writer.rs` (pure projection fn)
- Modify: `crates/vox-cli/src/commands/diagnostics/stub_check/mod.rs` (emit after a run)

- [ ] **Step 1: Write the failing test**

Add to `status_writer.rs` tests module:

```rust
#[test]
fn findings_project_to_per_rule_results() {
    use vox_code_audit::rules::{Finding, Severity};
    use std::path::PathBuf;
    let ran_rule_ids = vec![
        "stub/todo".to_string(),
        "stub/unimplemented".to_string(),
        "victory-claim".to_string(),
    ];
    let findings = vec![Finding {
        rule_id: "stub/todo".into(),
        diagnostic_id: None,
        rule_name: "TODO stub".into(),
        severity: Severity::Error,
        file: PathBuf::from("src/a.rs"),
        line: 7,
        column: 0,
        message: "todo!()".into(),
        suggestion: None,
        alternatives: vec![],
        rationale: None,
        context: String::new(),
        confidence: None,
        evidence: None,
    }];
    let results = code_audit_results(&ran_rule_ids, &findings);
    let by = |id: &str| results.iter().find(|r| r.id == id).map(|r| r.status);
    assert_eq!(by("code-audit/stub/todo"), Some(RunStatus::Fail));     // had a finding
    assert_eq!(by("code-audit/stub/unimplemented"), Some(RunStatus::Pass)); // ran, clean
    assert_eq!(by("code-audit/victory-claim"), Some(RunStatus::Pass));
    let hit = &results.iter().find(|r| r.id == "code-audit/stub/todo").unwrap().hits[0];
    assert_eq!(hit.line, 7);
    assert_eq!(hit.file, "src/a.rs");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli commands::policy::status_writer::tests::findings_project_to_per_rule_results`
Expected: FAIL — `cannot find function code_audit_results`.

- [ ] **Step 3: Write the projection**

Add to `status_writer.rs` (above tests). Map `vox_code_audit` rule ids into the
registry's `code-audit/<rule_id>` namespace (matching Plan 1a's generator):

```rust
use vox_config::Hit;

/// Project a code-audit run into per-rule status results.
///
/// `ran_rule_ids` are the rules that were actually evaluated (so a clean rule
/// records `pass`, not `unknown`). `findings` are the issues that fired.
/// A rule with any error/critical finding → `fail`; any warn/info finding but
/// no error → `warn`; a rule that ran with no finding → `pass`.
pub fn code_audit_results(
    ran_rule_ids: &[String],
    findings: &[vox_code_audit::rules::Finding],
) -> Vec<PolicyResult> {
    use std::collections::BTreeMap;
    use vox_code_audit::rules::Severity;

    // Bucket findings by raw rule_id.
    let mut by_rule: BTreeMap<&str, Vec<&vox_code_audit::rules::Finding>> = BTreeMap::new();
    for f in findings {
        by_rule.entry(f.rule_id.as_str()).or_default().push(f);
    }

    ran_rule_ids
        .iter()
        .map(|raw| {
            let id = format!("code-audit/{raw}");
            let hits_for = by_rule.get(raw.as_str());
            let status = match hits_for {
                None => RunStatus::Pass,
                Some(fs) => {
                    if fs.iter().any(|f| matches!(f.severity, Severity::Error | Severity::Critical)) {
                        RunStatus::Fail
                    } else {
                        RunStatus::Warn
                    }
                }
            };
            let hits = hits_for
                .map(|fs| {
                    fs.iter()
                        .map(|f| Hit {
                            file: f.file.display().to_string(),
                            line: f.line as u32,
                            note: f.message.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            PolicyResult { id, status, hits, duration_ms: 0 }
        })
        .collect()
}
```

> **Severity path:** `vox_code_audit::rules::Severity` variants are
> `Info | Warning | Error | Critical` ([rules.rs](../../../crates/vox-code-audit/src/rules.rs);
> the stub_check module imports `Severity` at line 13). Confirm the `Warning`
> spelling (not `Warn`) — adjust the `matches!` accordingly.

- [ ] **Step 4: Emit from the engine run site**

In `crates/vox-cli/src/commands/diagnostics/stub_check/mod.rs`, after the engine
produces `findings` and the set of rules that ran is known (the module already
holds `all_rules(None)` at line 541 and the `findings` vec; identify the function
that owns both), add a best-effort status emission gated by an env opt so it does
not change default CLI output:

```rust
// Phase 1c: record per-rule code-audit status for the current branch.
if std::env::var("VOX_NO_POLICY_STATUS").is_err() {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let ran: Vec<String> = all_rules(None).iter().map(|r| r.id().to_string()).collect();
    let results = crate::commands::policy::status_writer::code_audit_results(&ran, &findings);
    let branch = crate::commands::policy::status_writer::current_branch(&repo_root);
    let commit = crate::commands::policy::status_writer::head_commit(&repo_root);
    let ran_at = crate::util::now_iso8601();
    let _ = crate::commands::policy::status_writer::write_results(
        &repo_root, &branch, &commit, &ran_at, results,
    );
}
```

> **Placement:** put this where a *full-tree* audit completes (so `pass` for clean
> rules is meaningful). If the module supports scoped/partial scans, only emit on
> the full scan path — a partial scan must NOT record `pass` for rules it did not
> fully evaluate (that would fake green). If the run-site rule set is not
> trivially the full `all_rules`, pass the actually-evaluated rule ids instead.
> `r.id()` is the detector trait method used by Plan 1a's generator — confirm with
> `grep -n "fn id" crates/vox-code-audit/src/detectors/mod.rs`.

- [ ] **Step 5: Run tests + manual verification**

```bash
cargo test -p vox-cli commands::policy::status_writer::tests
cargo build -p vox-cli
```
Expected: projection test passes; build clean.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/policy/status_writer.rs crates/vox-cli/src/commands/diagnostics/stub_check/mod.rs
git commit -m "feat(vox-cli): per-rule code-audit status emission into policy store"
```

---

## Task 6: `--json` per-rule output for `vox-arch-check`

`vox-arch-check`'s `Report` is text-only today. Add a `--json` flag projecting it
to per-rule results so the same store can record `arch-rule` status.

**Files:**
- Create: `crates/vox-arch-check/src/json_report.rs`
- Modify: `crates/vox-arch-check/src/main.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-arch-check/src/json_report.rs`:

```rust
//! `--json` projection of the arch-check `Report` into per-rule results
//! (Phase 1c policy-status overlay). Mirrors the `arch-rule` registry ids.

use serde::Serialize;

/// One per-rule outcome, JSON-serialized for the policy-status overlay.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArchRuleResult {
    /// Registry id, e.g. `arch/fan_in` (matches the Plan 1b `arch-rule` ids).
    pub id: String,
    /// `pass` | `fail` | `warn`.
    pub status: String,
    /// Number of findings for this rule.
    pub count: usize,
}

/// Build a status string from (has_findings, is_strict).
pub fn status_str(has_findings: bool, strict: bool) -> &'static str {
    match (has_findings, strict) {
        (false, _) => "pass",
        (true, true) => "fail",
        (true, false) => "warn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_finding_is_fail_else_warn() {
        assert_eq!(status_str(false, true), "pass");
        assert_eq!(status_str(false, false), "pass");
        assert_eq!(status_str(true, true), "fail");
        assert_eq!(status_str(true, false), "warn");
    }

    #[test]
    fn result_serializes() {
        let r = ArchRuleResult { id: "arch/fan_in".into(), status: "warn".into(), count: 2 };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains("\"id\":\"arch/fan_in\""));
        assert!(j.contains("\"status\":\"warn\""));
    }
}
```

- [ ] **Step 2: Run test to verify it fails (module not declared)**

Add `mod json_report;` near the top of `crates/vox-arch-check/src/main.rs`, then:
Run: `cargo test -p vox-arch-check json_report::tests`
Expected: PASS once the module is declared (the fns above are self-contained).

- [ ] **Step 3: Add a `Report::to_rule_results` projector**

Add a method on `Report` in `main.rs` that walks each `*_warns` vec with its
matching `strict_*` flag and the canonical `arch/<rule>` id. One arm per rule
(the 11–16 guards in [`layers.toml [guards]`](../../../docs/src/architecture/layers.toml)):

```rust
impl Report {
    /// Project the report into per-rule results for the policy-status overlay.
    /// Ids match the `arch-rule` registry namespace (`arch/<guard-key>`).
    fn to_rule_results(&self) -> Vec<crate::json_report::ArchRuleResult> {
        use crate::json_report::{status_str, ArchRuleResult};
        let mk = |id: &str, has: bool, strict: bool, count: usize| ArchRuleResult {
            id: format!("arch/{id}"),
            status: status_str(has, strict).to_string(),
            count,
        };
        vec![
            mk("fan_in", !self.fan_in_warns.is_empty(), self.strict_fan_in, self.fan_in_warns.len()),
            mk("loc_budget", !self.loc_warns.is_empty(), self.strict_loc, self.loc_warns.len()),
            mk("orphan", !self.orphan_warns.is_empty(), self.strict_orphan, self.orphan_warns.len()),
            mk("description", !self.description_warns.is_empty(), self.strict_description, self.description_warns.len()),
            mk("where_things_live", !self.where_things_live_warns.is_empty(), self.strict_where_things_live, self.where_things_live_warns.len()),
            mk("wtl_parity", !self.wtl_parity_warns.is_empty(), self.strict_wtl_parity, self.wtl_parity_warns.len()),
            mk("staleness", !self.staleness_warns.is_empty(), self.strict_staleness, self.staleness_warns.len()),
            mk("generated_file_drift", !self.generated_file_drift_warns.is_empty(), self.strict_generated_file_drift, self.generated_file_drift_warns.len()),
            mk("forbidden_deps", !self.forbidden_dep_violations.is_empty(), self.strict_forbidden_deps, self.forbidden_dep_violations.len()),
            mk("loc_delta", !self.loc_delta_warns.is_empty(), self.strict_loc_delta, self.loc_delta_warns.len()),
            // layer ordering uses `inversions` + `strict_layer`:
            mk("layers", !self.inversions.is_empty(), self.strict_layer, self.inversions.len()),
            // docstring uses the split (name, strict) vec; treat any entry as a hit:
            mk("docstring", !self.docstring_warns.is_empty(), self.strict_docstring, self.docstring_warns.len()),
        ]
    }
}
```

> **Field-name verification:** the `*_warns` / `*_violations` / `strict_*` field
> names are read directly from [`main.rs:234–285`](../../../crates/vox-arch-check/src/main.rs)
> — `inversions`, `fan_in_warns`, `loc_warns`, `orphan_warns`, `docstring_warns`,
> `description_warns`, `where_things_live_warns`, `staleness_warns`,
> `generated_file_drift_warns`, `forbidden_dep_violations`,
> `forbidden_pattern_hits`, `wtl_parity_warns`, `loc_delta_warns`,
> `cdylib_dep_warns`, `workspace_dep_warns`, `evidence_findings`. Add an arm for
> any guard with a corresponding `arch-rule` registry id; omit guards Plan 1b did
> not catalog. Keep ids identical to the `arch-rule` entries Plan 1b emits from
> `layers.toml [guards]` keys.

- [ ] **Step 4: Wire the `--json` flag in `main`**

In `fn main` ([line 215](../../../crates/vox-arch-check/src/main.rs)), it currently
reads only `--warn-only`. Add `--json`, and on success print the per-rule results
as a JSON array to stdout (text summary still goes to stderr, preserving existing
behavior):

```rust
let warn_only = std::env::args().any(|a| a == "--warn-only");
let json = std::env::args().any(|a| a == "--json");

match run(warn_only) {
    Ok(report) => {
        if json {
            let results = report.to_rule_results();
            println!("{}", serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".into()));
        } else {
            report.print_summary();
        }
        if report.strict_failed() && !warn_only {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        }
    }
    Err(e) => {
        eprintln!("vox-arch-check: {e:#}");
        ExitCode::FAILURE
    }
}
```

> **Dep check:** `vox-arch-check` needs `serde` (derive) + `serde_json`. Confirm
> with `grep -nE '^serde|^serde_json' crates/vox-arch-check/Cargo.toml`; add
> `serde = { workspace = true, features = ["derive"] }` and
> `serde_json = { workspace = true }` if absent (both are workspace deps already).

- [ ] **Step 5: Run tests + manual check**

```bash
cargo test -p vox-arch-check json_report::tests
cargo run -p vox-arch-check -- --json | head -20
```
Expected: tests pass; `--json` prints a JSON array of `{id, status, count}`; plain
`cargo run -p vox-arch-check` is byte-for-byte unchanged (text on stderr).

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-arch-check
git add crates/vox-arch-check/src/json_report.rs crates/vox-arch-check/src/main.rs crates/vox-arch-check/Cargo.toml
git commit -m "feat(vox-arch-check): --json per-rule output for policy-status overlay"
```

---

## Task 7: `vox policy status [--branch b]…` (catalog ⨝ report)

**Files:**
- Modify: `crates/vox-cli/src/commands/policy/mod.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/vox-cli/src/commands/policy/mod.rs`:

```rust
#[test]
fn join_marks_unrun_rules_unknown() {
    use vox_config::{PolicyResult, PolicyRunReport, RunStatus};
    // Catalog ids: two rules; report only has a result for one.
    let catalog_ids = vec!["code-audit/stub/todo".to_string(), "arch/fan_in".to_string()];
    let report = Some(PolicyRunReport {
        branch: "main".into(),
        commit: "a".into(),
        ran_at: "t".into(),
        results: vec![PolicyResult {
            id: "code-audit/stub/todo".into(),
            status: RunStatus::Pass,
            hits: vec![],
            duration_ms: 3,
        }],
    });
    let joined = join_status(&catalog_ids, report.as_ref());
    let by = |id: &str| joined.iter().find(|(i, _)| i == id).map(|(_, s)| *s);
    assert_eq!(by("code-audit/stub/todo"), Some(RunStatus::Pass));
    assert_eq!(by("arch/fan_in"), Some(RunStatus::Unknown)); // never run → grey
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli commands::policy::tests::join_marks_unrun_rules_unknown`
Expected: FAIL — `cannot find function join_status`.

- [ ] **Step 3: Add the join + the `Status` subcommand**

Add the pure join helper to `crates/vox-cli/src/commands/policy/mod.rs`:

```rust
use vox_config::{PolicyRunReport, RunStatus};

/// Join catalog ids to a branch report; ids with no result are `Unknown`.
pub fn join_status(
    catalog_ids: &[String],
    report: Option<&PolicyRunReport>,
) -> Vec<(String, RunStatus)> {
    catalog_ids
        .iter()
        .map(|id| {
            let status = report
                .and_then(|r| r.results.iter().find(|res| &res.id == id))
                .map(|res| res.status)
                .unwrap_or(RunStatus::Unknown); // honest default
            (id.clone(), status)
        })
        .collect()
}
```

Add the variant to `enum PolicyCmd` (it gained `List/Show/Domains/Groups` in 1a):

```rust
    /// Show per-branch run status joined to the catalog. Repeat --branch for
    /// multiple active branches (worktrees).
    Status {
        /// Branch(es) to report. Defaults to the current branch.
        #[arg(long)]
        branch: Vec<String>,
        #[arg(long)]
        json: bool,
    },
```

Add the match arm in `run` (1a's `run(cmd, repo_root)` already loads `reg`):

```rust
        PolicyCmd::Status { branch, json } => {
            let branches = if branch.is_empty() {
                vec![status_writer::current_branch(repo_root)]
            } else {
                branch
            };
            let catalog_ids: Vec<String> = reg.policies.iter().map(|e| e.id.clone()).collect();
            let reports = vox_config::load_status_for_branches(repo_root, &branches)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            if json {
                // { branch -> [ {id,status} ] }
                let mut obj = serde_json::Map::new();
                for (b, rep) in &reports {
                    let rows: Vec<_> = join_status(&catalog_ids, rep.as_ref())
                        .into_iter()
                        .map(|(id, s)| serde_json::json!({ "id": id, "status": format!("{s:?}").to_lowercase() }))
                        .collect();
                    obj.insert(b.clone(), serde_json::Value::Array(rows));
                }
                println!("{}", serde_json::to_string_pretty(&obj)?);
            } else {
                for (b, rep) in &reports {
                    let when = rep.as_ref().map(|r| r.ran_at.as_str()).unwrap_or("never");
                    println!("# branch {b}  (last run: {when})");
                    for (id, s) in join_status(&catalog_ids, rep.as_ref()) {
                        let mark = match s {
                            RunStatus::Pass => "✓ pass",
                            RunStatus::Fail => "● fail",
                            RunStatus::Warn => "▲ warn",
                            RunStatus::Unknown => "— not run",
                        };
                        println!("  {mark:<10} {id}");
                    }
                }
            }
        }
```

> **`status_writer` reference:** `mod.rs` declared `pub mod status_writer;` in
> Task 3, so `status_writer::current_branch` resolves. The `Status` arm sits
> alongside the existing `List/Show/Domains/Groups` arms.

- [ ] **Step 4: Run tests + end-to-end**

```bash
cargo test -p vox-cli commands::policy::tests
cargo run -p vox-cli -- ci manifest          # produce a result
cargo run -p vox-cli -- policy status         # current branch
cargo run -p vox-cli -- policy status --branch main --branch HEAD --json
```
Expected: join test passes; `policy status` lists every catalog id with `✓/●/▲/—`;
ids with no result show `— not run`; multi-`--branch` prints one block per branch;
`--json` emits `{branch: [{id,status}]}`.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/policy/mod.rs
git commit -m "feat(vox-cli): vox policy status (per-branch catalog join)"
```

---

## Task 8: Where-things-live + parity extension + final gate

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`
- Modify: `crates/vox-cli/src/commands/ci/policy_registry.rs` (parity for the `gate_policy_id` map)

- [ ] **Step 1: Extend the WTL row**

Extend the policy-catalog row added by Plan 1a in
`docs/src/architecture/where-things-live.md` to mention the status overlay:

```markdown
| Per-branch policy run status (overlay) | `.vox/policy-status/<branch>.json` store; model+reader in `vox-config` (`policy::status`); writer + capture seams in `vox-cli` (`commands/policy/status_writer.rs`, ci-dispatch wrapper); `--json` emitters in `vox-code-audit`/`vox-arch-check` |
```

- [ ] **Step 2: Add a parity test for the gate-id map**

So the `gate_policy_id` map cannot drift from the `ci-gate` catalog, add a test
to `crates/vox-cli/src/commands/ci/policy_registry.rs` tests module (extend Plan
1b's domain coverage once `ci-gate` entries exist; until then, assert that every
id the map returns is well-formed `ci/<kebab>`):

```rust
#[test]
fn gate_policy_ids_are_well_formed() {
    // Sample a few known variants; ids must be `ci/<kebab-case>`.
    use crate::commands::ci::cmd_enums::CiCmd;
    for id in [
        CiCmd::Manifest.gate_policy_id(),
        CiCmd::SsotDrift.gate_policy_id(),
    ]
    .into_iter()
    .flatten()
    {
        assert!(id.starts_with("ci/"), "{id} must be ci/-namespaced");
        assert!(id.chars().all(|c| c.is_ascii_lowercase() || c == '/' || c == '-'));
    }
}
```

> **Full parity (when Plan 1b's ci-gate domain has landed):** strengthen this to
> assert every `Some(id)` from `gate_policy_id` is present as a `ci-gate` entry in
> the loaded registry, and (optionally) warn on registry `ci-gate` ids that have no
> `gate_policy_id` mapping (= untracked gates). Keep it a *test*, not a blocking CI
> gate, until the ci-gate catalog is complete (honesty: untracked = grey, not red).

- [ ] **Step 3: Run the full local gate set**

```bash
cargo run -p vox-arch-check
cargo test -p vox-config policy::status
cargo test -p vox-cli commands::policy commands::ci::cmd_enums
cargo test -p vox-arch-check json_report
```
Expected: arch-check green (WTL row satisfies the where-things-live guard); all
status/writer/join/projection tests pass.

- [ ] **Step 4: End-to-end honesty check**

```bash
rm -rf .vox/policy-status
cargo run -p vox-cli -- policy status      # everything "— not run" (grey)
cargo run -p vox-cli -- ci manifest
cargo run -p vox-cli -- ci ssot-drift || true
cargo run -p vox-cli -- policy status      # only ci/manifest + ci/ssot-drift lit
```
Expected: before any run, **every** rule is `— not run`; after two gates, only
those two ids change; nothing is faked green.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-cli
git add docs/src/architecture/where-things-live.md crates/vox-cli/src/commands/ci/policy_registry.rs
git commit -m "docs(arch): WTL row for policy status overlay; gate-id parity test"
```

---

## Self-Review

**Spec coverage (Phase 1c slice, per §4.5 + §10 addendum point 3):**
- `PolicyRunReport` type (`{branch, commit, ran_at, results:[{id,status,hits,duration_ms}]}`), serde → Task 1. ✓
- `load_status` + multi-branch reader in `vox-config` → Tasks 1, 2. ✓
- `.vox/policy-status/<sanitized-branch>.json` store, MERGE-by-id writer → Task 3. ✓
- Multi-branch (worktrees): `sanitize_branch`, `worktree_branches`, per-file isolation, multi-`--branch` → Tasks 1, 3, 7. ✓
- Per-GATE capture via a single dispatch wrapper (no invasive per-gate edits) → Task 4. ✓
- Per-FINDING capture: code-audit (`Finding` → per-rule) → Task 5; arch (`--json`) → Task 6. ✓
- effort-audit `findings.jsonl` reused, not rebuilt → noted (Verified facts); no new code. ✓
- `vox policy status [--branch b]…` joins catalog + report; unrun → `unknown` → Task 7. ✓
- Honesty: `RunStatus::default() == Unknown`; `pass` only on a real clean run; best-effort writes never fail a gate; grey before any run → Tasks 1, 4, 5, 8. ✓
- Determinism: `ran_at` is caller-stamped (`now_iso8601` at the dispatch seam); `merge_results`/`code_audit_results`/`join_status`/`status_str` are pure and table-tested → Tasks 3–7. ✓
- where-things-live + arch-check green → Task 8. ✓

**Placeholder scan:** No "TBD"/hollow functions. The `>` notes (dep pre-checks,
`Severity::Warning` spelling, ci-gate id confirmation against the 1b-generated
catalog, run-site placement for full-vs-partial scans, `now_iso8601` helper) are
verification reminders against real files with concrete fallbacks — not stubs. The
`gate_policy_id` map is intentionally seeded with a verified handful and gated by a
parity test, matching the honest "untracked = grey" contract; Plan 1b fills the rest.

**Type consistency:** `PolicyRunReport` / `PolicyResult` / `RunStatus` / `Hit` are
used identically across Tasks 1–7. `write_results(repo_root, branch, commit,
ran_at, fresh)` and `merge_results(report, fresh, branch, commit, ran_at)`
signatures match all call sites (Tasks 3–5). `code_audit_results(ran_ids,
findings)` and `join_status(catalog_ids, report)` are consistent between their
definition and callers. Registry-id namespaces (`code-audit/<raw>`, `arch/<guard>`,
`ci/<command>`) match Plan 1a's generator and Plan 1b's planned domains.

**Cross-plan dependencies:** depends on Plan 1a (landed: `vox-config` model/loader,
`vox policy` CLI scaffold, the two `CiCmd::PolicyRegistry*` arms). The ci-gate /
arch-rule registry **ids** this plan keys on are produced by Plan 1b; if 1c lands
first, the seeded `gate_policy_id` arms and a handful of `arch/*` ids still work
end-to-end, and the parity test in Task 8 tightens as 1b fills the catalog.

---

## Defers (not in this plan)

- **Plan 1b** — generator + parity for `ci-gate` (from `contracts/operations/catalog.v1.yaml`),
  `arch-rule` (`layers.toml [guards]`), `crl-gate`, `audit-check`. This plan's
  `gate_policy_id` map and `arch/<guard>` ids align with 1b's namespaces.
- **Plan 1d** — GUI Policies surface consuming this overlay: status-colored group
  counts, master-sidebar badge, multi-branch selector (Tauri `policy_status` /
  `list_branches` IPC over the `vox-config` reader), graceful-empty "Needs attention".
- **Per-gate `hits` for ci/audit/crl gates** — Phase 1c records per-gate pass/fail
  only (no line-level hits); per-finding hits exist solely for code-audit/arch.
  effort-audit `findings.jsonl` ingestion into the overlay is deferred (the file is
  produced today; mapping it to per-policy results is a later, additive seam).
- **CR-L / audit-check capture** — once those gates have registry ids (Plan 1b),
  they slot into the same per-gate wrapper via `gate_policy_id` with no new code.
