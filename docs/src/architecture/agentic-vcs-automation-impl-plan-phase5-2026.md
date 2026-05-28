---
title: "Agentic VCS Automation — Phase 5 Implementation Plan (2026-05-09)"
description: "Step-by-step TDD plan that swaps the GitExec backend for hot-path operations from tokio::process::Command to gix (libgit2-equivalent in pure Rust), keeps shell-out for low-frequency or compatibility-sensitive commands, and evaluates jj-lib for change-id tracking on the write side. The GitExec interface does not change. Includes a feature-flag rollout, a benchmark harness, and a migration plan."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 5 is the optional optimisation pass that takes the central GitExec wrapper and substitutes a more efficient implementation for hot-path ops (status, log, diff, rev-parse) without changing the surface. Concrete benchmark methodology, exact feature-flag plumbing, exact migration order. Future agents executing this plan should not need to invent code."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-orchestrator-mcp/src/git_exec.rs: gains a backend abstraction"
  - "Cargo.toml: adds gix workspace dep with conservative feature set"
  - "vox-orchestrator-mcp: micro-benchmark crate evaluating gix vs git-shellout per op"
---

# Agentic VCS Automation — Phase 5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
>
> **Companion docs:** [Phase 1 plan](agentic-vcs-automation-impl-plan-phase1-2026.md), [research](agentic-version-control-automation-research-2026.md). Read research §"Open question 2: gix vs jj-lib for the underlying Rust impl" before starting.

**Goal:** Make the orchestrator's high-frequency git operations (status, log, diff, rev-parse) faster and more deterministic by routing them through `gix` (pure-Rust git library) inside `GitExec`, while keeping shell-out for everything else. The `GitExec::run` and `GitExec::run_unchecked` signatures do not change. After Phase 5, an agent dashboard render that today triggers ~6 process spawns per refresh runs entirely in-process.

**Architecture:** Introduce a `GitBackend` trait inside `git_exec.rs` with two implementations: `GitBackendShell` (the existing `tokio::process::Command` path) and `GitBackendGix` (uses `gix` for supported subcommands). `GitExec` dispatches per-subcommand: subcommands on a known-fast list go through gix; others go through shell. The dispatch is a static match table — no runtime configuration. A workspace feature flag `vox-orchestrator-mcp/gix-backend` (default-on) lets us disable the gix path globally if a regression appears in production.

`jj-lib` is **evaluated but not adopted in Phase 5**. The benchmark crate measures the same set of ops against jj-lib for context, and the data informs whether a future Phase 5.5 makes sense. The decision criterion is documented in the research doc §"Net read for Vox": jj-lib is preferred only where its op-log + change-id model adds operational safety beyond what `vox-orchestrator-queue` already provides.

**Tech stack:** Rust 2021. New deps: `gix = "0.69"` (latest stable as of mid-2026; check at implementation time) with the `max-control` feature off (we don't need the env-var resolution that adds startup cost). Optional dev-dep: `criterion` for the benchmark harness.

**Out of scope for Phase 5:**
- Replacing shell-out for write ops (commit, branch, push). Shell-out is correct for these — `gix` write support is partial and migrating now adds risk for ops that already work fine.
- Adopting `jj-lib` as a runtime backend (deferred indefinitely; Phase 5 only benchmarks).
- Feature parity with all `git` subcommands (the migration list is closed: status, log, diff, rev-parse, ls-files, show-ref).

---

## Verification setup

- `cargo test -p vox-orchestrator-mcp --lib git_exec` — backend trait + dispatch tests.
- `cargo bench -p vox-vcs-bench` — benchmark crate (added in Task 4).
- `cargo run -p vox-arch-check` — must remain green; the new backend module stays inside `vox-orchestrator-mcp`.
- `cargo build -p vox-orchestrator-mcp --no-default-features --features ""` — must compile with the gix backend disabled (sanity check the feature flag).

The plan produces 6 commits.

---

## Pre-flight: confirm gix's feature surface fits

Before starting, run a discovery pass:

```
cargo doc --open -p gix
```

Look for: `gix::status::index_worktree`, `gix::diff::Cache`, `gix::revision::Spec::from_bstr`, `gix::reference::iter`. These are the four core APIs Task 2 uses. If any of them have moved or been renamed in the latest gix version, **stop**, update Task 2's API references, and continue. The shape of the migration is invariant; the API names drift.

---

## Task 1: Introduce the GitBackend trait

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/git_exec.rs`
- Test: same file

The trait is private to the crate. `GitExec` holds a `Box<dyn GitBackend + Send + Sync>` and dispatches; the public surface (`run` / `run_unchecked`) is unchanged.

- [ ] **Step 1: Tests for the trait shape and dispatch**

```rust
#[tokio::test]
async fn git_exec_uses_shell_backend_by_default() {
    let exec = GitExec::new(std::env::current_dir().unwrap());
    // The shell backend uses tokio::process::Command; we don't actually run
    // git here — we just check the backend identity through a debug helper.
    assert_eq!(exec.backend_kind(), GitBackendKind::Shell);
}

#[tokio::test]
async fn git_exec_with_gix_backend_dispatches_status_to_gix() {
    let exec = GitExec::new_with_backend(
        std::env::current_dir().unwrap(),
        Box::new(GitBackendGix::new()),
    );
    assert_eq!(exec.backend_kind(), GitBackendKind::Gix);
    // Don't test actual gix call here — that's Task 2.
}
```

- [ ] **Step 2: Implement the trait + dispatch**

```rust
// inside git_exec.rs
#[async_trait::async_trait]
pub(crate) trait GitBackend: Send + Sync {
    fn kind(&self) -> GitBackendKind;
    async fn run(&self, cwd: &Path, args: &[&str]) -> Result<GitOutput, GitExecError>;
    async fn run_unchecked(&self, cwd: &Path, args: &[&str]) -> Result<GitOutput, GitExecError>;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GitBackendKind { Shell, Gix }

pub(crate) struct GitBackendShell;

#[async_trait::async_trait]
impl GitBackend for GitBackendShell {
    fn kind(&self) -> GitBackendKind { GitBackendKind::Shell }
    async fn run(&self, cwd: &Path, args: &[&str]) -> Result<GitOutput, GitExecError> {
        if let Some(reason) = is_banned(args) {
            tracing::warn!(target: "vox.vcs.exec", banned = %reason, ?args, "rejected");
            return Err(GitExecError::Banned(reason));
        }
        self.run_unchecked(cwd, args).await
    }
    async fn run_unchecked(&self, cwd: &Path, args: &[&str]) -> Result<GitOutput, GitExecError> {
        // (existing shell-out body, unchanged)
        unimplemented!("move existing body here")
    }
}

pub struct GitExec {
    cwd: PathBuf,
    backend: Box<dyn GitBackend>,
}

impl GitExec {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            backend: Box::new(GitBackendShell),
        }
    }

    pub(crate) fn new_with_backend(cwd: impl Into<PathBuf>, backend: Box<dyn GitBackend>) -> Self {
        Self { cwd: cwd.into(), backend }
    }

    pub(crate) fn backend_kind(&self) -> GitBackendKind { self.backend.kind() }

    pub fn cwd(&self) -> &Path { &self.cwd }

    pub async fn run(&self, args: &[&str]) -> Result<GitOutput, GitExecError> {
        self.backend.run(&self.cwd, args).await
    }

    pub(crate) async fn run_unchecked(&self, args: &[&str]) -> Result<GitOutput, GitExecError> {
        self.backend.run_unchecked(&self.cwd, args).await
    }
}
```

Move the existing body of the old `GitExec::run` into `GitBackendShell::run_unchecked`. The signature change is internal — public callers still see `GitExec::run(args)`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p vox-orchestrator-mcp --lib git_exec`
Expected: PASS — old tests + 2 new tests.
Run: `cargo test -p vox-orchestrator-mcp --lib`
Expected: PASS — full suite.

- [ ] **Step 4: Commit**

```
git add crates/vox-orchestrator-mcp/src/git_exec.rs crates/vox-orchestrator-mcp/Cargo.toml
git commit -m "refactor(git_exec): introduce GitBackend trait; existing shell path becomes GitBackendShell"
```

---

## Task 2: Implement GitBackendGix for status / log / diff / rev-parse / ls-files / show-ref

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/git_exec/backend_gix.rs`
- Modify: `crates/vox-orchestrator-mcp/src/git_exec.rs` — declare the submodule
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml` — add gix dep behind a feature flag

The backend is selective. Only the six listed subcommands have native gix paths; everything else falls back to shell-out. The dispatch is at the start of `GitBackendGix::run_unchecked`:

```rust
match args.first() {
    Some(&"status") => self.gix_status(cwd, &args[1..]).await,
    Some(&"log")    => self.gix_log(cwd, &args[1..]).await,
    Some(&"diff")   => self.gix_diff(cwd, &args[1..]).await,
    Some(&"rev-parse") => self.gix_rev_parse(cwd, &args[1..]).await,
    Some(&"ls-files")  => self.gix_ls_files(cwd, &args[1..]).await,
    Some(&"show-ref")  => self.gix_show_ref(cwd, &args[1..]).await,
    _ => GitBackendShell.run_unchecked(cwd, args).await,  // fall back
}
```

This means the gix backend always falls back to shell for unsupported subcommands — full coverage with selective acceleration.

- [ ] **Step 1: Add gix to Cargo.toml**

In root `Cargo.toml`:

```toml
gix = { version = "0.69", default-features = false, features = ["index", "revision", "tree-editor"] }
async-trait = "0.1"
```

In `crates/vox-orchestrator-mcp/Cargo.toml`:

```toml
[features]
default = ["gix-backend", "news-publish", "toestub-gate", "json-schema"]
gix-backend = ["dep:gix"]
# ... existing features ...

[dependencies]
gix = { workspace = true, optional = true }
async-trait = { workspace = true }
```

- [ ] **Step 2: Tests for each gix path**

Each test runs against a temporary git repo created via `gix::init`. Pattern for `status`:

```rust
#[tokio::test]
async fn gix_status_lists_modified_file() {
    let tmp = tempfile::tempdir().unwrap();
    init_repo_with_one_modified_file(tmp.path()).await;
    let backend = GitBackendGix::new();
    let out = backend.run_unchecked(tmp.path(), &["status", "--porcelain"]).await.unwrap();
    assert!(out.stdout.contains("M ") || out.stdout.contains(" M"));
}
```

Repeat the same pattern for `log`, `diff`, `rev-parse HEAD`, `ls-files`, `show-ref`. Each test sets up a fixture repo and asserts gix produces output consistent with what shell-git would produce for the same args.

A consistency test should run the same args against both backends and assert byte-identical output for at least one canonical input per subcommand:

```rust
#[tokio::test]
async fn gix_and_shell_produce_byte_identical_rev_parse_head() {
    let tmp = tempfile::tempdir().unwrap();
    init_repo_with_one_commit(tmp.path()).await;
    let shell = GitBackendShell.run_unchecked(tmp.path(), &["rev-parse", "HEAD"]).await.unwrap();
    let gix   = GitBackendGix::new().run_unchecked(tmp.path(), &["rev-parse", "HEAD"]).await.unwrap();
    assert_eq!(shell.stdout.trim(), gix.stdout.trim());
}
```

- [ ] **Step 3: Implementation skeleton**

```rust
// crates/vox-orchestrator-mcp/src/git_exec/backend_gix.rs
//! gix-backed implementations of the GitExec API for hot-path subcommands.
//! Falls through to GitBackendShell for any subcommand without a gix
//! path, so callers see no behavioural difference — only latency.

use std::path::Path;

use crate::git_exec::{GitBackend, GitBackendKind, GitBackendShell, GitExecError, GitOutput};

pub struct GitBackendGix;

impl GitBackendGix {
    pub fn new() -> Self { Self }

    async fn gix_rev_parse(&self, cwd: &Path, args: &[&str]) -> Result<GitOutput, GitExecError> {
        let repo = gix::open(cwd).map_err(|e| GitExecError::Spawn(std::io::Error::new(
            std::io::ErrorKind::Other, format!("gix open: {e}")
        )))?;
        let target = args.first().copied().unwrap_or("HEAD");
        let oid = repo
            .rev_parse_single(target)
            .map_err(|e| GitExecError::NonZero {
                code: 128,
                stdout: String::new(),
                stderr: format!("gix rev-parse: {e}"),
            })?
            .detach();
        Ok(GitOutput {
            stdout: format!("{}\n", oid.to_hex_with_len(40)),
            stderr: String::new(),
            exit_code: 0,
        })
    }

    async fn gix_status(&self, cwd: &Path, _args: &[&str]) -> Result<GitOutput, GitExecError> {
        // Implement using gix::status::index_worktree to produce a
        // --porcelain-style output. See gix docs for the exact iterator
        // shape; collect entries into "XY path" lines.
        unimplemented!()
    }

    async fn gix_log(&self, _cwd: &Path, _args: &[&str]) -> Result<GitOutput, GitExecError> { unimplemented!() }
    async fn gix_diff(&self, _cwd: &Path, _args: &[&str]) -> Result<GitOutput, GitExecError> { unimplemented!() }
    async fn gix_ls_files(&self, _cwd: &Path, _args: &[&str]) -> Result<GitOutput, GitExecError> { unimplemented!() }
    async fn gix_show_ref(&self, _cwd: &Path, _args: &[&str]) -> Result<GitOutput, GitExecError> { unimplemented!() }
}

#[async_trait::async_trait]
impl GitBackend for GitBackendGix {
    fn kind(&self) -> GitBackendKind { GitBackendKind::Gix }

    async fn run(&self, cwd: &Path, args: &[&str]) -> Result<GitOutput, GitExecError> {
        if let Some(reason) = crate::git_exec::is_banned(args) {
            tracing::warn!(target: "vox.vcs.exec", banned = %reason, ?args, "rejected");
            return Err(GitExecError::Banned(reason));
        }
        self.run_unchecked(cwd, args).await
    }

    async fn run_unchecked(&self, cwd: &Path, args: &[&str]) -> Result<GitOutput, GitExecError> {
        match args.first() {
            Some(&"rev-parse") => self.gix_rev_parse(cwd, &args[1..]).await,
            Some(&"status")    => self.gix_status(cwd, &args[1..]).await,
            Some(&"log")       => self.gix_log(cwd, &args[1..]).await,
            Some(&"diff")      => self.gix_diff(cwd, &args[1..]).await,
            Some(&"ls-files")  => self.gix_ls_files(cwd, &args[1..]).await,
            Some(&"show-ref")  => self.gix_show_ref(cwd, &args[1..]).await,
            _ => GitBackendShell.run_unchecked(cwd, args).await,
        }
    }
}
```

Implement the six `gix_*` methods. Each is mechanical: open the repo, query gix's API, format output to match `git --porcelain` (or whatever the args specify). The tests from Step 2 are the canonical correctness oracle; if shell-git and gix disagree, the bug is in the gix path or in our format adapter, not in shell-git.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp --lib git_exec --features gix-backend`
Expected: PASS — all tests, including the byte-identity check on rev-parse.

Also run with the feature OFF:
```
cargo build -p vox-orchestrator-mcp --no-default-features --features news-publish,toestub-gate,json-schema
```
Expected: PASS — the gix backend module is hidden behind `#[cfg(feature = "gix-backend")]` and the crate compiles without it.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-mcp/src/git_exec/backend_gix.rs crates/vox-orchestrator-mcp/src/git_exec.rs crates/vox-orchestrator-mcp/Cargo.toml Cargo.toml
git commit -m "feat(git_exec): add gix-backed paths for status/log/diff/rev-parse/ls-files/show-ref"
```

---

## Task 3: Make GitExec::new auto-pick the gix backend when feature is on

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/git_exec.rs`

The default constructor selects the gix backend when the feature is compiled in; otherwise shell. This is the rollout switch.

- [ ] **Step 1: Implementation**

```rust
impl GitExec {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        #[cfg(feature = "gix-backend")]
        let backend: Box<dyn GitBackend> = Box::new(crate::git_exec::backend_gix::GitBackendGix::new());

        #[cfg(not(feature = "gix-backend"))]
        let backend: Box<dyn GitBackend> = Box::new(GitBackendShell);

        Self { cwd: cwd.into(), backend }
    }

    /// Test-only constructor that forces shell backend even when gix is enabled.
    /// Used by tests that compare backends and by anyone debugging a gix
    /// regression.
    pub fn new_shell(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            backend: Box::new(GitBackendShell),
        }
    }
}
```

- [ ] **Step 2: Update existing tests that need a deterministic backend**

Any test that asserts on shell-specific behavior (e.g. `git` not on PATH error format) should switch from `GitExec::new` to `GitExec::new_shell`. Run the test suite and update as needed.

- [ ] **Step 3: Run tests**

Expected: PASS, including the `cargo test -p vox-orchestrator-mcp --lib` full suite. Any test that fails because it implicitly relied on shell-out semantics now needs to declare its backend choice.

- [ ] **Step 4: Commit**

```
git add crates/vox-orchestrator-mcp/src/git_exec.rs
git commit -m "feat(git_exec): default to gix backend when feature compiled in; new_shell escape hatch for tests"
```

---

## Task 4: Benchmark crate

**Files:**
- Create: `crates/vox-vcs-bench/Cargo.toml`
- Create: `crates/vox-vcs-bench/benches/git_ops.rs`
- Modify: root `Cargo.toml` — add to workspace members

The benchmark proves the speedup is real. It runs each of the six migrated ops against:
1. `GitBackendShell`
2. `GitBackendGix`
3. (Optional, evaluative-only) `jj-lib` equivalents for `log` and `status` — for Phase 5.5 decision data.

Fixtures: a small (~100-commit) repo and a medium (~10k-commit) repo, both generated by a setup script. The bench reports ms/op and counts allocations via `dhat` if available.

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "vox-vcs-bench"
version.workspace = true
edition.workspace = true
publish = false

[[bench]]
name = "git_ops"
harness = false

[dependencies]
vox-orchestrator-mcp = { workspace = true, features = ["gix-backend"] }
gix.workspace = true

[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio"] }
tokio = { workspace = true, features = ["rt", "rt-multi-thread", "macros"] }
tempfile.workspace = true
```

Add `crates/vox-vcs-bench` to the workspace `members` list.

- [ ] **Step 2: Benchmark file**

```rust
// crates/vox-vcs-bench/benches/git_ops.rs
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use vox_orchestrator_mcp::git_exec::{GitExec};

fn bench_rev_parse(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let small_repo = setup_small_repo();
    let medium_repo = setup_medium_repo();

    let mut group = c.benchmark_group("rev_parse_HEAD");
    for (label, path) in [("small", &small_repo), ("medium", &medium_repo)] {
        group.bench_with_input(BenchmarkId::new("shell", label), path, |b, p| {
            b.to_async(&rt).iter(|| async {
                let g = GitExec::new_shell(p);
                g.run(&["rev-parse", "HEAD"]).await.unwrap()
            })
        });
        group.bench_with_input(BenchmarkId::new("gix", label), path, |b, p| {
            b.to_async(&rt).iter(|| async {
                let g = GitExec::new(p);
                g.run(&["rev-parse", "HEAD"]).await.unwrap()
            })
        });
    }
    group.finish();
}

// repeat for status, log, diff, ls-files, show-ref

criterion_group!(benches, bench_rev_parse /*, bench_status, bench_log, ... */);
criterion_main!(benches);

fn setup_small_repo() -> std::path::PathBuf { unimplemented!() }
fn setup_medium_repo() -> std::path::PathBuf { unimplemented!() }
```

`setup_*` create or cache fixture repos. The first run takes a few seconds; subsequent runs reuse the cached fixture.

- [ ] **Step 3: Run the bench**

```
cargo bench -p vox-vcs-bench
```

Capture the output. Expected qualitative result for `rev-parse HEAD`: gix ~10–50× faster than shell on small repo (no process spawn cost), ~5–20× on medium. If gix is *slower*, that's a finding — investigate before continuing. Phase 5 only commits after the bench shows a real improvement.

Save the output to `docs/src/architecture/agentic-vcs-phase5-bench-results.md` (new file). The doc serves as the empirical justification for the rollout.

- [ ] **Step 4: Commit**

```
git add crates/vox-vcs-bench/ Cargo.toml docs/src/architecture/agentic-vcs-phase5-bench-results.md
git commit -m "feat(bench): add vox-vcs-bench comparing shell vs gix for rev-parse/status/log/diff/ls-files/show-ref"
```

---

## Task 5: Optional jj-lib evaluation

**Files:**
- Modify: `crates/vox-vcs-bench/benches/git_ops.rs` — add jj-lib bench arms
- Document: `docs/src/architecture/agentic-vcs-phase5-bench-results.md` — append jj-lib section

This task is purely evaluative; nothing in the production code depends on jj-lib after Phase 5. The deliverable is a numbers-and-recommendation entry in the bench-results doc that future Phase 5.5 work can reference.

`jj-lib` is at version 0.27 in the workspace already (per the existing `vox-orchestrator/src/jj_backend.rs`). Add bench arms that use jj-lib for `log` and `status` (the two ops where its op-log + change-id model has the strongest theoretical advantage).

- [ ] **Step 1: Add jj-lib bench arms**

```rust
group.bench_with_input(BenchmarkId::new("jj-lib", label), path, |b, p| {
    b.to_async(&rt).iter(|| async {
        // Open the colocated jj+git repo, list operations
        let workspace = jj_lib::workspace::Workspace::load(p, /* ... */).unwrap();
        // ... call jj-lib's equivalent of `log` ...
    })
});
```

The jj-lib API surface differs significantly from gix; this is exploratory code, not production. Keep it minimal.

- [ ] **Step 2: Run + document**

Run the bench. Document the results inline in `agentic-vcs-phase5-bench-results.md` along with a recommendation:

```
## Recommendation

For Phase 5 (this phase), gix is the right backend for the six hot-path
ops; jj-lib was [faster | slower | comparable] for log+status but adds
operational complexity (colocated mode, change-id reconciliation with
oplog) that does not justify adoption right now.

Reconsider jj-lib in Phase 5.5 if:
- Mesh replication needs change-ids that survive rebase (the strongest
  theoretical jj advantage; today the orchestrator queue's change_id
  field already provides this without jj-lib).
- A specific op shows a 2x+ improvement under jj-lib that we cannot
  match with gix.
```

- [ ] **Step 3: Commit**

```
git add crates/vox-vcs-bench/benches/git_ops.rs docs/src/architecture/agentic-vcs-phase5-bench-results.md
git commit -m "bench: evaluate jj-lib for log+status; document Phase 5.5 reconsider criteria"
```

---

## Task 6: Documentation

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`
- Modify: `docs/src/architecture/git-concurrency-policy.md`
- Modify: `docs/src/architecture/architecture-index.md` (regenerated)

- [ ] **Step 1: where-things-live.md row**

```
| Add a hot-path git op with a native backend | Add a `gix_<op>` method in `crates/vox-orchestrator-mcp/src/git_exec/backend_gix.rs` and a dispatch arm in its `run_unchecked`. Falls back to shell for everything else. |
```

- [ ] **Step 2: git-concurrency-policy.md addition**

Append:

```markdown
## Backend implementation

`GitExec::run` dispatches to one of two backends:

| Backend | When | Notes |
|---|---|---|
| Shell (`tokio::process::Command`) | Fallback for any subcommand without a native path; default when the `gix-backend` feature is off | Bug-for-bug compatible with the system git |
| Gix (pure Rust) | `status`, `log`, `diff`, `rev-parse`, `ls-files`, `show-ref` when the `gix-backend` feature is on | ~10× faster on small repos by avoiding process spawn |

The dispatch is internal; callers see only `GitExec::run(&args)`. To
force the shell backend for debugging, use `GitExec::new_shell(cwd)`
instead of `GitExec::new(cwd)`. To disable the gix backend globally,
build without the `gix-backend` feature.

`jj-lib` is **not** used as a backend in Phase 5. Benchmark data and
the Phase 5.5 reconsider criteria live in `agentic-vcs-phase5-bench-results.md`
once that artifact is generated (not committed yet when this plan was written).
```

- [ ] **Step 3: Regenerate**

```
cargo run -p vox-doc-pipeline
cargo run -p vox-doc-pipeline -- --check
```

- [ ] **Step 4: Commit**

```
git add docs/src/architecture/where-things-live.md docs/src/architecture/git-concurrency-policy.md
git commit -m "docs(vcs): document Phase 5 gix backend and jj-lib evaluation"
git add docs/src/SUMMARY.md docs/src/architecture/architecture-index.md docs/src/feed.xml
git commit -m "chore(docs): regenerate SUMMARY.md / architecture-index.md / feed.xml"
```

---

## Phase 5 acceptance criteria

- [ ] `cargo test -p vox-orchestrator-mcp --lib` passes with `--features gix-backend` (the default after Task 2).
- [ ] `cargo build -p vox-orchestrator-mcp --no-default-features --features news-publish,toestub-gate,json-schema` passes (gix-backend off).
- [ ] `cargo bench -p vox-vcs-bench` produces results showing gix ≥ 5× faster than shell on `rev-parse HEAD` for the small repo. (Other ops should also be faster; flag any that aren't.)
- [ ] `cargo run -p vox-arch-check` passes; no new layer violations.
- [ ] `cargo run -p vox-doc-pipeline -- --check` passes.
- [ ] The byte-identity test between shell and gix passes for at least one canonical input per migrated subcommand.
- [ ] All 6 commits land per the per-task templates.

---

## Notes for the implementing engineer

- **Byte-identity is the test oracle, not "looks similar".** If shell and gix produce different output for the same args, fix gix's adapter until they match. This is the only way to guarantee that downstream parsers (in `git_log` etc.) keep working after the swap. If gix genuinely can't match shell's output for a given arg combination, fall through to shell for that combination — partial migration is fine.
- **Don't migrate write ops in this phase.** `commit`, `branch`, `push`, `pull`, `fetch` stay on shell-out. gix's write paths are improving but still partial; the ROI on migrating them is low because they're not hot-path. A Phase 5.5 may revisit this once gix's write story is stable enough.
- **The benchmark recommendation must be empirical, not aspirational.** If the bench shows gix is actually *slower* for some op (it does happen for tiny repos where startup cost is amortised across many shell calls), document the finding and remove that op from the gix-backend dispatch. Phase 5 ships only what's measurably faster.
- **`async-trait` adds an `Box::pin` per call.** That's typically negligible compared to git's process-spawn cost, but the bench should confirm it's not a regression for very fast ops on tiny repos. If it is, the `GitBackend` trait can be replaced with an `enum GitBackend { Shell, Gix }` and `match` dispatch in `run_unchecked`. Don't do this preemptively — only if the bench shows it.
- **Watch jj-lib's stability story.** As of mid-2026 jj-lib is reaching feature stability but its public API still has churn between releases. The evaluation in Task 5 should pin the exact jj-lib version used; do not silently update it across Phase 5 implementations or the bench numbers become non-comparable.
- **The `gix-backend` feature flag is a real safety net.** If a production regression appears, flip the flag, redeploy. Don't remove the flag in Phase 5; it's the rollback path. Reconsider removing only after 3 months of stable production use.
