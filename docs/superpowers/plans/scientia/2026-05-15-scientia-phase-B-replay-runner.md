# SCIENTIA Phase B — Replay Runner (Measured `artifact_replayability`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Status:** detailed (promoted from outline 2026-05-15 after code-surface exploration).

**Goal:** Replace the operator-asserted `artifact_replayability` field in the worthiness rubric with a *measured* score produced by re-executing the manifest's declared `entry_point` in a sandbox and verifying its output hash against the deposited one.

**Architecture:** Two changes:

1. **Extend the existing RO-Crate metadata** (which lives in `crates/vox-scientia/src/ro_crate/`, not a separate `vox-ro-crate` crate as the Finalization Plan §6.1 implied — see "Architectural decision" below) with an optional `MainEntity` carrying `entry_point`, `expected_output_paths`, `env_pin`, `timeout_seconds`, and `resource_budget`. RO-Crates without a `mainEntity` are not replay-eligible.
2. **Create a new L2 crate `vox-replay-runner`** exposing `replay_manifest(codex, publication_id) -> ReplayReport`. It: (a) materializes the RO-Crate into a fresh git worktree using existing `superpowers:using-git-worktrees` machinery, (b) reads `mainEntity`, (c) executes in a sandboxed child via `tokio::process::Command` with `kill_on_drop=true` and the declared timeout, (d) computes `vox_crypto::compliance_hash` of declared output paths, (e) compares to manifest deposited hashes, (f) writes a measured score back to worthiness signals as a new entry `artifact_replayability_measured` (binary 0.0 / 1.0; partial-match scoring is a follow-up).

**Tech Stack:** Rust 2024; `tokio::process::Command`; `vox-crypto::compliance_hash` (SHA3-256 facade at `crates/vox-crypto/src/facades.rs:30`); existing `WorthinessInputs` at `crates/vox-publisher/src/publication_worthiness.rs:68`; existing worktree machinery. No new external deps.

**Strategic context:** [Gap-map §2 Gap B](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-b--replay-runner-that-measures-artifact_replayability); [Finalization Plan §3.4 (symbolic verifiers)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#34-ground-truth-verifier--symbolic-where-possible-minicheck-where-not).

**Architectural decision (resolve in Pre-flight before Task 1):** The Finalization Plan §6.1 said `vox-ro-crate` would be a separate L2 crate, but the implementation landed under `crates/vox-scientia/src/ro_crate/`. Two options:

- **B-arch-1 (recommended, plan assumes this):** Extend the existing in-crate location with `mainEntity`. Smaller diff; no crate-split risk; Finalization-Plan §6.1 documentation gets a footnote noting the consolidation.
- **B-arch-2:** Finally split `vox-ro-crate` out as a separate L2 crate as §6.1 specified, *then* add `mainEntity`. Bigger diff; cleaner long-term boundaries; risks dragging in adjacent refactors.

This plan executes B-arch-1. If the user prefers B-arch-2, the crate-split is a prerequisite sub-phase (separate plan).

**Out of scope:**
- GPU-bound replay (deferred; CPU-only — see OQ-B1).
- Non-Vox manifest formats.
- Distributed replay across the mesh (Mesh Phase 5/6).
- LLM-judged partial-replay scoring.
- Container-based sandboxing (plain subprocess + working-dir + timeout in Phase B; container adapter is a follow-up).

---

## File inventory

| Action | Path | Responsibility |
|---|---|---|
| Create | `crates/vox-replay-runner/Cargo.toml` | L2 crate manifest |
| Create | `crates/vox-replay-runner/src/lib.rs` | Public API: `replay_manifest`, `ReplayReport`, `ReplayError` |
| Create | `crates/vox-replay-runner/src/sandbox.rs` | Subprocess execution with timeout + `kill_on_drop` + stdout/stderr capture |
| Create | `crates/vox-replay-runner/src/contract.rs` | `MainEntity` parsing from the manifest JSON |
| Create | `crates/vox-replay-runner/src/hash_compare.rs` | SHA3 of output paths; compare to manifest |
| Create | `crates/vox-replay-runner/src/report.rs` | `ReplayReport` shape |
| Create | `crates/vox-replay-runner/tests/integration.rs` | Trivial-RO-Crate replay round-trip |
| Modify | `crates/vox-scientia/src/ro_crate/metadata.rs` | Add optional `main_entity: Option<MainEntity>` field |
| Modify | `crates/vox-scientia/src/ro_crate/mod.rs` | Re-export `MainEntity` |
| Modify | `crates/vox-publisher/src/publication_worthiness.rs` | Prefer measured value when present; add `artifact_replayability_measured` to `WorthinessInputs` |
| Modify | `crates/vox-publisher/src/publication_preflight/worthiness_extraction.rs` | Read measured field from manifest if present |
| Modify | `contracts/scientia/worthiness-signals.v2.schema.json` | Document the new signal id `artifact-replayability-measured` |
| Modify | `crates/vox-cli/src/commands/db/publication.rs` | Add `publication-replay` subcommand |
| Modify | `contracts/cli/command-registry.yaml` | Register CLI command |
| Modify | `crates/vox-orchestrator/src/mcp_tools/tools/scientia_tools.rs` | Add MCP tool |
| Modify | `contracts/mcp/tool-registry.canonical.yaml` | Register MCP tool |
| Modify | `docs/src/reference/scientia-publication-worthiness-rules.md` | Document measured vs declared semantics |
| Modify | `docs/src/architecture/where-things-live.md` | Add row: "Replay runner" → `crates/vox-replay-runner/` |
| Modify | `docs/src/architecture/layers.toml` | Register at L2 |
| Modify | `Cargo.toml` (root) | Add to `[workspace.members]` + `[workspace.dependencies]` |

LoC budget: ~1000 LoC + ~300 tests.

---

## Pre-flight verification

- [ ] **Step P1: Confirm RO-Crate is in `vox-scientia`, not a separate crate**

```bash
ls crates/ | grep -E "vox-ro-crate|vox-scientia"
cat crates/vox-scientia/src/ro_crate/mod.rs
```

Expected: no `crates/vox-ro-crate/`; `crates/vox-scientia/src/ro_crate/mod.rs` exists and exposes `RoCrateMetadata`.

If this isn't true (e.g., someone has since split `vox-ro-crate` out), **stop** and re-survey before continuing.

- [ ] **Step P2: Confirm `compliance_hash` and worthiness inputs**

```bash
grep -n "pub fn compliance_hash" crates/vox-crypto/src/facades.rs
grep -n "pub.*artifact_replayability" crates/vox-publisher/src/publication_worthiness.rs
```

Expected: matches around `facades.rs:30` and `publication_worthiness.rs:76`.

- [ ] **Step P3: Confirm `kill_on_drop` API**

```bash
grep -n "tokio::process::Command\|kill_on_drop" crates/vox-actor-runtime/src/
```

Note the idiomatic pattern in the codebase for spawning a subprocess with a timeout.

---

## Task 1: Extend `RoCrateMetadata` with optional `MainEntity`

**Files:**
- Modify: `crates/vox-scientia/src/ro_crate/metadata.rs`

- [ ] **Step 1.1: Write the failing test**

Add to `crates/vox-scientia/tests/ro_crate_main_entity.rs`:

```rust
use vox_scientia::ro_crate::{RoCrateMetadata, MainEntity};

#[test]
fn metadata_serializes_with_main_entity() {
    let meta = RoCrateMetadata {
        name: "demo".into(),
        description: "demo".into(),
        doi: None,
        author_orcid: vec![],
        author_ror: vec![],
        license_spdx: "MIT".into(),
        published_at: 0,
        keywords: vec![],
        main_entity: Some(MainEntity {
            entry_point: "run.sh".into(),
            expected_output_paths: vec!["out.txt".into()],
            expected_output_hashes_hex: vec!["abcd".into()],
            env_pin: "lockfile-sha:deadbeef".into(),
            timeout_seconds: 60,
            max_stdout_bytes: 1_000_000,
            max_stderr_bytes: 1_000_000,
        }),
    };
    let json = vox_scientia::ro_crate::build_ro_crate_json(&meta);
    let s = serde_json::to_string(&json).unwrap();
    assert!(s.contains("\"mainEntity\""), "JSON-LD must include mainEntity node");
    assert!(s.contains("\"run.sh\""));
}
```

- [ ] **Step 1.2: Run to verify it fails**

```bash
cargo test -p vox-scientia ro_crate_main_entity 2>&1 | head -20
```
Expected: compile error — `MainEntity` not defined.

- [ ] **Step 1.3: Add `MainEntity` and field**

In `crates/vox-scientia/src/ro_crate/metadata.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MainEntity {
    pub entry_point: String,
    pub expected_output_paths: Vec<String>,
    pub expected_output_hashes_hex: Vec<String>,
    pub env_pin: String,
    pub timeout_seconds: u32,
    pub max_stdout_bytes: u64,
    pub max_stderr_bytes: u64,
}
```

Add to `RoCrateMetadata`:
```rust
pub main_entity: Option<MainEntity>,
```

In `build_ro_crate_json`, when `main_entity.is_some()`, append to `@graph` a node:
```jsonc
{
  "@id": "./",
  "@type": "Dataset",
  "mainEntity": {
    "@id": "#/mainEntity",
    "@type": "SoftwareSourceCode",
    "vox:entryPoint": entry_point,
    "vox:expectedOutputs": [
      {"path": <p1>, "hashHex": <h1>}, ...
    ],
    "vox:envPin": env_pin,
    "vox:timeoutSeconds": timeout_seconds,
    "vox:resourceBudget": {"maxStdoutBytes": ..., "maxStderrBytes": ...}
  }
}
```

(Use `vox:` as the local JSON-LD prefix; declare `"vox": "https://vox-lang.org/ro-crate/v1#"` in `@context`. Verify the exact `@context` shape used by `build_ro_crate_json` and extend rather than replace.)

- [ ] **Step 1.4: Run test to verify pass**

- [ ] **Step 1.5: Re-export from `ro_crate/mod.rs`**

```rust
pub use metadata::{RoCrateMetadata, MainEntity, build_ro_crate_json};
```

- [ ] **Step 1.6: Commit**

```bash
git add crates/vox-scientia/src/ro_crate crates/vox-scientia/tests/ro_crate_main_entity.rs
git commit -m "feat(vox-scientia): RoCrateMetadata.main_entity for replay contract"
```

---

## Task 2: Scaffold `vox-replay-runner` crate

**Files:**
- Create: `crates/vox-replay-runner/Cargo.toml`
- Create: `crates/vox-replay-runner/src/lib.rs`
- Modify: workspace `Cargo.toml` and `layers.toml`

- [ ] **Step 2.1: Cargo.toml**

```toml
[package]
name = "vox-replay-runner"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false

[dependencies]
vox-scientia = { workspace = true }
vox-crypto = { workspace = true }
vox-db = { workspace = true }
tokio = { workspace = true, features = ["process", "time", "io-util", "macros", "rt-multi-thread"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror = { workspace = true }
hex = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 2.2: Stub lib.rs**

```rust
//! SCIENTIA Phase B — replay runner.
//!
//! Re-executes the `mainEntity` of a publication manifest's RO-Crate in
//! a sandboxed subprocess and writes a measured replayability score back
//! to the worthiness signals.

pub mod sandbox;
pub mod contract;
pub mod hash_compare;
pub mod report;

pub use report::{ReplayReport, ReplayOutcome, ReplayError};

pub async fn replay_manifest(
    codex: vox_db::Codex,
    publication_id: &str,
) -> Result<ReplayReport, ReplayError> {
    // Task 5 fills this in.
    todo!()
}
```

- [ ] **Step 2.3: Register in workspace + layers.toml**

Workspace `Cargo.toml`: add `"crates/vox-replay-runner"` to members; add `vox-replay-runner = { path = "crates/vox-replay-runner" }` to dependencies.

`docs/src/architecture/layers.toml`: add `vox-replay-runner = { layer = 2 }`.

- [ ] **Step 2.4: Verify it compiles**

```bash
cargo check -p vox-replay-runner
```

- [ ] **Step 2.5: Commit**

```bash
git add crates/vox-replay-runner Cargo.toml docs/src/architecture/layers.toml
git commit -m "scaffold(vox-replay-runner): empty L2 crate"
```

---

## Task 3: Sandbox subprocess execution

**Files:**
- Create: `crates/vox-replay-runner/src/sandbox.rs`

**Contract:** Given a working directory, a command, and a timeout, spawn the child with `kill_on_drop(true)`, race against the timeout, and return captured stdout/stderr/exit/wall-time/peak-rss.

- [ ] **Step 3.1: Write the failing test**

In `crates/vox-replay-runner/tests/sandbox_smoke.rs`:

```rust
use std::time::Duration;
use tempfile::tempdir;
use vox_replay_runner::sandbox::{run_in_sandbox, SandboxOutcome};

#[tokio::test]
async fn echo_exits_zero() {
    let dir = tempdir().unwrap();
    let out = run_in_sandbox(
        dir.path(),
        if cfg!(windows) { "cmd" } else { "sh" },
        if cfg!(windows) { &["/C", "echo ok"] } else { &["-c", "echo ok"] },
        Duration::from_secs(5),
    ).await.unwrap();
    assert_eq!(out.exit_code, Some(0));
    assert!(out.stdout.contains("ok"));
}

#[tokio::test]
async fn timeout_kills_child() {
    let dir = tempdir().unwrap();
    let out = run_in_sandbox(
        dir.path(),
        if cfg!(windows) { "ping" } else { "sleep" },
        if cfg!(windows) { &["-n", "60", "127.0.0.1"] } else { &["30"] },
        Duration::from_millis(500),
    ).await.unwrap();
    assert!(matches!(out, SandboxOutcome { timed_out: true, .. }));
}
```

- [ ] **Step 3.2: Implement `sandbox.rs`**

```rust
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug)]
pub struct SandboxOutcome {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub wall_ms: u64,
}

pub async fn run_in_sandbox(
    cwd: &Path,
    program: &str,
    args: &[&str],
    timeout_dur: Duration,
) -> std::io::Result<SandboxOutcome> {
    let start = std::time::Instant::now();
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd).kill_on_drop(true);
    let child = cmd.output();
    let result = timeout(timeout_dur, child).await;
    let elapsed = start.elapsed().as_millis() as u64;
    match result {
        Ok(Ok(out)) => Ok(SandboxOutcome {
            exit_code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            timed_out: false,
            wall_ms: elapsed,
        }),
        Ok(Err(e)) => Err(e),
        Err(_elapsed) => Ok(SandboxOutcome {
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: true,
            wall_ms: elapsed,
        }),
    }
}
```

- [ ] **Step 3.3: Verify pass**

```bash
cargo test -p vox-replay-runner sandbox
```

- [ ] **Step 3.4: Commit**

```bash
git commit -am "feat(vox-replay-runner): sandbox subprocess runner with timeout + kill_on_drop"
```

---

## Task 4: Hash-comparator + `MainEntity` loader

**Files:**
- Create: `crates/vox-replay-runner/src/hash_compare.rs`
- Create: `crates/vox-replay-runner/src/contract.rs`

- [ ] **Step 4.1: Write tests**

In `tests/hash_compare_smoke.rs`:

```rust
use std::fs;
use tempfile::tempdir;
use vox_replay_runner::hash_compare::compare_output_hashes;

#[test]
fn matching_hash_passes() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("out.txt"), "hello").unwrap();
    let want_hex = hex::encode(vox_crypto::compliance_hash(b"hello"));
    let outcome = compare_output_hashes(dir.path(), &["out.txt".into()], &[want_hex]).unwrap();
    assert!(outcome.all_match);
}

#[test]
fn mismatching_hash_fails() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("out.txt"), "hello").unwrap();
    let outcome = compare_output_hashes(dir.path(), &["out.txt".into()], &["deadbeef".into()]).unwrap();
    assert!(!outcome.all_match);
    assert_eq!(outcome.mismatches.len(), 1);
}
```

- [ ] **Step 4.2: Implement `hash_compare.rs`**

```rust
use std::path::Path;

pub struct HashCompareOutcome {
    pub all_match: bool,
    pub mismatches: Vec<HashMismatch>,
}
pub struct HashMismatch {
    pub path: String,
    pub expected_hex: String,
    pub actual_hex: String,
}

pub fn compare_output_hashes(
    cwd: &Path,
    paths: &[String],
    expected_hex: &[String],
) -> std::io::Result<HashCompareOutcome> {
    assert_eq!(paths.len(), expected_hex.len(), "paths/hashes len mismatch");
    let mut mismatches = Vec::new();
    for (p, want) in paths.iter().zip(expected_hex.iter()) {
        let bytes = std::fs::read(cwd.join(p))?;
        let got = hex::encode(vox_crypto::compliance_hash(&bytes));
        if &got != want {
            mismatches.push(HashMismatch {
                path: p.clone(),
                expected_hex: want.clone(),
                actual_hex: got,
            });
        }
    }
    Ok(HashCompareOutcome { all_match: mismatches.is_empty(), mismatches })
}
```

- [ ] **Step 4.3: Implement `contract.rs`**

```rust
use vox_scientia::ro_crate::MainEntity;

pub fn load_main_entity_from_manifest(
    codex: &vox_db::Codex,
    publication_id: &str,
) -> Result<Option<MainEntity>, crate::ReplayError> {
    // Step 4.4 maps the read path through the manifest store.
    todo!()
}
```

- [ ] **Step 4.4: Wire `load_main_entity_from_manifest`**

Locate the function in `crates/vox-db/src/store/ops_publication/*` that fetches a manifest by `publication_id`. It returns a row whose `metadata_json` field holds the RO-Crate metadata. Parse `metadata_json` → `RoCrateMetadata` → return `.main_entity`.

If the manifest has no `main_entity`, return `Ok(None)`; the caller surfaces this as `ReplayError::NotReplayEligible`.

- [ ] **Step 4.5: Verify pass**

- [ ] **Step 4.6: Commit**

---

## Task 5: `replay_manifest` orchestration

**Files:**
- Modify: `crates/vox-replay-runner/src/lib.rs`
- Create: `crates/vox-replay-runner/src/report.rs`

- [ ] **Step 5.1: Define `ReplayReport` + `ReplayError`**

```rust
// src/report.rs
use thiserror::Error;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReplayReport {
    pub publication_id: String,
    pub manifest_digest: String,
    pub outcome: ReplayOutcome,
    pub wall_ms: u64,
    pub measured_score: f64,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum ReplayOutcome {
    Pass,
    HashMismatch,
    NonZeroExit(i32),
    TimedOut,
    NotReplayEligible,
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("manifest not found: {0}")]
    ManifestNotFound(String),
    #[error("manifest not replay-eligible (no mainEntity)")]
    NotReplayEligible,
    #[error("worktree materialization failed: {0}")]
    Worktree(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("db: {0}")]
    Db(String),
}
```

- [ ] **Step 5.2: Write the integration test**

In `tests/integration.rs`:

```rust
use vox_replay_runner::{replay_manifest, ReplayOutcome};

#[tokio::test]
async fn trivial_main_entity_replays_to_pass() {
    // 1. Spin up a Codex with a manifest whose RO-Crate mainEntity declares
    //    entry_point that creates out.txt with content "ok".
    // 2. Call replay_manifest.
    // 3. Assert outcome = Pass, measured_score = 1.0.
}

#[tokio::test]
async fn mutated_expected_hash_replays_to_hash_mismatch() {
    // Same setup with a wrong expected hash.
    // Assert outcome = HashMismatch, measured_score = 0.0.
}
```

- [ ] **Step 5.3: Implement `replay_manifest`**

In `lib.rs`:

```rust
pub async fn replay_manifest(
    codex: vox_db::Codex,
    publication_id: &str,
) -> Result<ReplayReport, ReplayError> {
    let main_entity = contract::load_main_entity_from_manifest(&codex, publication_id)?
        .ok_or(ReplayError::NotReplayEligible)?;

    let worktree_dir = tempfile::tempdir()
        .map_err(|e| ReplayError::Worktree(e.to_string()))?;

    // Materialize: write each artifact in the manifest's RO-Crate into worktree_dir.
    // (Step 5.4 maps the artifact-fetching API.)
    materialize_artifacts(&codex, publication_id, worktree_dir.path()).await?;

    let timeout_dur = std::time::Duration::from_secs(main_entity.timeout_seconds as u64);
    let entry = &main_entity.entry_point;

    let outcome = sandbox::run_in_sandbox(
        worktree_dir.path(),
        if cfg!(windows) { "cmd" } else { "sh" },
        if cfg!(windows) { &["/C", entry] } else { &["-c", entry] },
        timeout_dur,
    ).await?;

    let report = if outcome.timed_out {
        report::ReplayReport {
            publication_id: publication_id.into(),
            manifest_digest: "".into(), // Step 5.5: fetch from manifest
            outcome: ReplayOutcome::TimedOut,
            wall_ms: outcome.wall_ms,
            measured_score: 0.0,
            diagnostics: vec!["sandbox timeout".into()],
        }
    } else if let Some(code) = outcome.exit_code {
        if code != 0 {
            report::ReplayReport {
                publication_id: publication_id.into(),
                manifest_digest: "".into(),
                outcome: ReplayOutcome::NonZeroExit(code),
                wall_ms: outcome.wall_ms,
                measured_score: 0.0,
                diagnostics: vec![format!("exit {}", code)],
            }
        } else {
            let cmp = hash_compare::compare_output_hashes(
                worktree_dir.path(),
                &main_entity.expected_output_paths,
                &main_entity.expected_output_hashes_hex,
            )?;
            if cmp.all_match {
                report::ReplayReport {
                    publication_id: publication_id.into(),
                    manifest_digest: "".into(),
                    outcome: ReplayOutcome::Pass,
                    wall_ms: outcome.wall_ms,
                    measured_score: 1.0,
                    diagnostics: vec![],
                }
            } else {
                report::ReplayReport {
                    publication_id: publication_id.into(),
                    manifest_digest: "".into(),
                    outcome: ReplayOutcome::HashMismatch,
                    wall_ms: outcome.wall_ms,
                    measured_score: 0.0,
                    diagnostics: cmp.mismatches.iter().map(|m|
                        format!("{}: expected {} got {}", m.path, m.expected_hex, m.actual_hex)
                    ).collect(),
                }
            }
        }
    } else {
        report::ReplayReport { /* signal-killed / no exit code */ ..todo!() }
    };

    persist_replay_report(&codex, &report).await?;
    Ok(report)
}
```

- [ ] **Step 5.4: Implement `materialize_artifacts`**

Trace the existing artifact-storage path (likely in `publication_manifests` or a sibling table that holds artifact bytes / file references). For each entry, write to `worktree_dir.join(path)`. If artifacts are stored as content-addressable blobs, fetch by hash.

- [ ] **Step 5.5: Persist the report**

Write a `publication_status_events` row (or whichever status-event surface exists — exploration suggested this might be under a different name; verify via `grep -rn "status_event\|StatusEvent" crates/vox-db/src/store/`). The status code is `replay_measured`; the JSON payload is the `ReplayReport`.

- [ ] **Step 5.6: Verify pass**

- [ ] **Step 5.7: Commit**

---

## Task 6: Worthiness rubric integration

**Files:**
- Modify: `crates/vox-publisher/src/publication_worthiness.rs`
- Modify: `crates/vox-publisher/src/publication_preflight/worthiness_extraction.rs`
- Modify: `contracts/scientia/worthiness-signals.v2.schema.json`

- [ ] **Step 6.1: Add `artifact_replayability_measured` to `WorthinessInputs`**

```rust
pub struct WorthinessInputs {
    // ... existing fields ...
    pub artifact_replayability: f64,                       // operator-declared (existing)
    pub artifact_replayability_measured: Option<f64>,      // NEW
}
```

- [ ] **Step 6.2: Prefer measured when present**

In `evaluate_worthiness`:

```rust
let effective_replayability = inputs.artifact_replayability_measured
    .unwrap_or(inputs.artifact_replayability);
let passed = effective_replayability >= c.thresholds.artifact_replayability_min;

// When measured value is absent, surface a soft-gate signal:
if inputs.artifact_replayability_measured.is_none() {
    soft_gate.push(WorthinessSignal {
        id: "artifact-replayability-not-measured".into(),
        passed: true,
        score: 0.5,
        reason_code: "replay_not_run".into(),
        details: Some("artifact_replayability is operator-declared; run `vox scientia publication-replay` to measure".into()),
    });
}
```

- [ ] **Step 6.3: Extend `worthiness_extraction.rs`**

Read the most recent `replay_measured` status event for the manifest; if present, set `artifact_replayability_measured`. If absent, leave `None`.

- [ ] **Step 6.4: Schema documentation**

In `worthiness-signals.v2.schema.json`, add a `description` block enumerating the canonical signal ids including `artifact-replayability-measured` and `artifact-replayability-not-measured`.

- [ ] **Step 6.5: Verify worthiness tests pass**

```bash
cargo test -p vox-publisher
```

- [ ] **Step 6.6: Commit**

---

## Task 7: CLI surface — `vox scientia publication-replay`

**Files:**
- Modify: `crates/vox-cli/src/commands/db/publication.rs`
- Modify: `crates/vox-cli/src/db_cli/subcommands.rs`
- Modify: `crates/vox-cli/src/commands/scientia.rs`
- Modify: `contracts/cli/command-registry.yaml`

- [ ] **Step 7.1: Add Clap variant**

In `db_cli/subcommands.rs` (publication section):

```rust
PublicationReplay {
    #[arg(long)] publication_id: String,
    #[arg(long, default_value = "json")] output: OutputFormat,
}
```

- [ ] **Step 7.2: Handler**

In `commands/db/publication.rs`:

```rust
pub async fn publication_replay(publication_id: &str) -> Result<()> {
    let codex = vox_db::VoxDb::connect_default().await?;
    let report = vox_replay_runner::replay_manifest(codex.into(), publication_id).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
```

- [ ] **Step 7.3: Scientia-facade mirror**

In `commands/scientia.rs`, add `PublicationReplay` arm dispatching to the handler.

- [ ] **Step 7.4: Register in command-registry.yaml**

Mirror the `publication-prepare` entry shape:

```yaml
- surface: vox-cli
  path: [scientia, publication-replay]
  status: active
  latin_ns: codex
  product_lane: data
  feature_gate: null
  catalog_group: null
  ref_cli_required: true
  reachability_required: null
  handler_rust: commands::db::publication::publication_replay
```

- [ ] **Step 7.5: Update catalog baseline**

```bash
UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli command_catalog_paths_baseline
```

- [ ] **Step 7.6: Test**

```bash
cargo test -p vox-cli
```

- [ ] **Step 7.7: Commit**

---

## Task 8: MCP tool

**Files:**
- Modify: `crates/vox-orchestrator/src/mcp_tools/tools/scientia_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`
- Modify: `contracts/mcp/tool-registry.canonical.yaml`

Mirror an existing scientia MCP tool (e.g., `vox_scientia_publication_status`). Tool name: `vox_scientia_publication_replay`. Input: `{ publication_id: string }`. Output: `ReplayReport` JSON.

After registry changes: in `vox-vscode`, run `pnpm run compile` (or at minimum `pnpm run generate:mcp-registry && pnpm run check:mcp-parity`).

---

## Task 9: Documentation

- [ ] **Step 9.1: Worthiness rules doc**

In `docs/src/reference/scientia-publication-worthiness-rules.md`, add a section under "Reproducibility":

> `artifact_replayability` is operator-declared by default. To replace it with a measured value, run `vox scientia publication-replay --publication-id <id>`; the worthiness rubric prefers measured over declared when both are present. A new soft-gate signal `artifact-replayability-not-measured` is emitted when no measured value exists.

- [ ] **Step 9.2: Playbook entries**

In `docs/src/reference/scientia-publication-playbook.md`, add stable failure-mode entries for `ReplayOutcome::HashMismatch`, `NonZeroExit`, `TimedOut`, `NotReplayEligible`.

- [ ] **Step 9.3: Where things live**

Add row to `docs/src/architecture/where-things-live.md`:
```md
| [`vox-replay-runner`](../../../crates/vox-replay-runner/) | SCIENTIA Phase B: re-executes a manifest's RO-Crate `mainEntity` in a sandbox; writes measured `artifact_replayability` back to worthiness signals. |
```

- [ ] **Step 9.4: Commit**

---

## Task 10: Final verification

- [ ] **Step 10.1:** `cargo test --workspace`
- [ ] **Step 10.2:** `cargo run -p vox-arch-check`
- [ ] **Step 10.3:** `cargo run -p vox-doc-pipeline`
- [ ] **Step 10.4:** Final commit.

---

## Acceptance criteria

1. `cargo test -p vox-replay-runner` green; all task-level tests pass.
2. `cargo test --workspace` green.
3. `cargo run -p vox-arch-check` exit 0.
4. Trivial-mainEntity fixture: measured score 1.0; mutated-expectation fixture: 0.0.
5. Worthiness rubric prefers measured over declared when both present; emits `artifact-replayability-not-measured` soft-gate signal when measured is absent.
6. `vox scientia publication-replay` CLI works on a manifest with `main_entity` populated; returns JSON report.
7. MCP tool registry parity check passes.

---

## Open questions

- **OQ-B1.** GPU replay — deferred to follow-up. Document the boundary in the README.
- **OQ-B2.** Replay determinism — Phase B requires deterministic `entry_point`s. Document `env_pin` semantics: it's the user's responsibility to fix seeds, runner does not enforce.
- **OQ-B3.** Sandbox tech — plain subprocess + working-dir + timeout. Container adapter (Docker/Podman) is a follow-up behind a feature flag.
- **OQ-B4.** Status-event surface — verify the exact API name during Task 5.5; my exploration suggested `publication_status_events` exists but the writer function name was not found at the expected location.

---

## Dependencies

- **Upstream:** RO-Crate metadata (existing in `vox-scientia`); `vox-crypto::compliance_hash` ✅; manifest storage ✅.
- **Downstream:** None hard.

---

## Cross-references

- Gap: [gap-map §2 Gap B](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-b--replay-runner-that-measures-artifact_replayability)
- Worthiness inputs: [`crates/vox-publisher/src/publication_worthiness.rs`](../../../../crates/vox-publisher/src/publication_worthiness.rs) line ~68
- SHA3 facade: [`crates/vox-crypto/src/facades.rs`](../../../../crates/vox-crypto/src/facades.rs) line ~30
- RO-Crate metadata: [`crates/vox-scientia/src/ro_crate/metadata.rs`](../../../../crates/vox-scientia/src/ro_crate/metadata.rs)
- Architectural-decision context: Finalization Plan §6.1 (note: stated as separate `vox-ro-crate` crate; in reality consolidated under `vox-scientia`).
