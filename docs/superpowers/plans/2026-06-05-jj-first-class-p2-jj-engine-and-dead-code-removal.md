# P2 — JjBackend (jj-lib 0.42 engine) + Dead-Code Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make in-process **jj-lib 0.42** the real default VCS engine behind the `vox-vcs` `VcsBackend`
trait, replace the subprocess/feature-gated jj usage, verify pure-`gix` fetch/push, and TDD-delete the
hand-rolled jj-shaped code jj-lib obviates — with no external `jj` binary required.

**Architecture:** jj-lib's API is intentionally **unstable and under-documented**, and its workspace
init/commit calls are **async** while `VcsBackend` is sync. So this plan is **spike-first**: Task 1
stands up a *real, green* `JjBackend::open + snapshot + changes` against a temp colocated repo,
discovering the exact construction (`UserSettings`/`StoreFactories`/`Signer`) and establishing the
**async↔sync bridge** (a `JjBackend`-owned `tokio` runtime that `block_on`s jj-lib's async calls).
Every later task reuses that verified foundation. All `jj_lib::` calls stay confined to
`crates/vox-vcs/src/jj_backend.rs` (the `jj-lib-confined` arch-check exemption from P0). Deletions are
each gated by a green parity test (the P0/P1 audit already proved `ContentMerge`/`OperationDag` dead).

**Tech Stack:** Rust, `jj-lib 0.42` (pure-`gix`, no libgit2, no `jj` binary), `tokio`, `tempfile`,
`vox-arch-check`.

**Source spec:** [`2026-06-05-jj-first-class-vcs-design.md`](../specs/2026-06-05-jj-first-class-vcs-design.md) §3, §4 (deletion ledger), §8 (P2).
**Depends on:** P0 (the `vox-vcs` crate + jj-lib 0.42 dep). Independent of P1.

**VERIFIED jj-lib 0.42.0 anchors** (read against the installed crate source on 2026-06-06, not
docs.rs guesses — file refs are within `jj-lib-0.42.0/src/`). The **async/sync split is the load-bearing
fact for the bridge design below**:

| Call | Exact signature | async? | src ref |
|---|---|---|---|
| init (colocated) | `Workspace::init_colocated_git(&UserSettings, &Path) -> Result<(Workspace, Arc<ReadonlyRepo>), WorkspaceInitError>` | **async** | `workspace.rs:223` |
| init (no git) | `Workspace::init_simple(&UserSettings, &Path) -> Result<(Workspace, Arc<ReadonlyRepo>), _>` | **async** | `workspace.rs:194` |
| load existing | `Workspace::load(&UserSettings, &Path, &StoreFactories, &WorkingCopyFactories) -> Result<Workspace, WorkspaceLoadError>` | **sync** | `workspace.rs:406` |
| settings | `UserSettings::from_config(StackedConfig) -> Result<Self, ConfigGetError>` (needs `user.name`/`user.email`; `signing.key` optional) | sync | `settings.rs:135` |
| config | `StackedConfig::with_defaults()` / `::empty()` | sync | `config.rs:654/660` |
| txn start | `Arc<ReadonlyRepo>::start_transaction(&self) -> Transaction` | sync | `repo.rs:333` |
| txn mut | `Transaction::repo_mut(&mut self) -> &mut MutableRepo` | sync | `transaction.rs:93` |
| txn commit | `Transaction::commit(self, impl Into<String>) -> Result<Arc<ReadonlyRepo>, _>` | **async** | `transaction.rs:120` |
| wc snapshot | `TreeState::snapshot(&mut self, &SnapshotOptions) -> Result<(bool, SnapshotStats), _>` | **async** | `local_working_copy.rs:1278` |
| signer | `Signer::new(None, vec![]) -> Signer` (no-op; signing disabled) | sync | `signing.rs:205` |
| op heads | `op_walk::get_current_head_ops(&RepoLoader) -> Result<Vec<Operation>, _>` | **async** | `op_walk.rs:212` |
| revset eval | `RevsetExpression::evaluate(Arc<Self>, &dyn Repo) -> Result<Box<dyn Revset>, _>` | sync | `revset.rs:680` |
| git backend | pure **gix 0.84** (NOT libgit2/git2) | — | `Cargo.toml:97` |

**`SnapshotOptions` is non-trivial to construct** (`working_copy.rs:211`): it needs
`base_ignores: Arc<GitIgnoreFile>` (use `GitIgnoreFile::empty()`), a `start_tracking_matcher` and
`force_tracking_matcher` (`&dyn Matcher` — use `EverythingMatcher`), `progress: None`, and a
`max_new_file_size`. Pin the exact ctors in the Task-1 spike.

> **⚠ CRITICAL BRIDGE CORRECTION (supersedes the original "JjBackend owns a tokio runtime + `block_on`"
> design).** init/commit/snapshot/op-walk are **async**, and `JjBackend` is wired into the **orchestrator,
> which already runs inside a tokio runtime** (Task 5). Calling `Runtime::block_on` (or
> `Handle::block_on`) from *within* a tokio worker thread **panics** ("Cannot start a runtime from within
> a runtime" / "can call blocking only when running on the multi-threaded runtime"). The fix is the
> **offload-thread bridge**: `JjBackend` owns a dedicated OS thread that hosts its *own* current-thread
> runtime; sync `VcsBackend` methods send a boxed closure to that thread over an `mpsc` channel and wait
> on a `oneshot` reply. This is correct from **both** sync CLI callers (Task 6) and async orchestrator
> callers (Task 5) — the `block_on` only ever runs on the non-tokio worker thread. See the Task-1 struct
> below. (jj-lib also refuses to read config from `$HOME`/env by design, so `UserSettings` is built from
> an explicit in-memory `StackedConfig` carrying a fixed bot identity.)

---

## File Structure

| File | Responsibility |
|---|---|
| Modify `crates/vox-vcs/Cargo.toml` | Add non-optional `jj-lib` + `tokio` (rt) deps; re-add `tempfile` dev-dep |
| Create `crates/vox-vcs/src/jj_backend.rs` | `JjBackend` — the ONLY place `jj_lib::` is called; owns the async↔sync runtime bridge |
| Modify `crates/vox-vcs/src/backend.rs` | `detect()` → prefer `Jj` when a repo is present/initable |
| Modify `crates/vox-vcs/src/lib.rs` | `pub mod jj_backend;` + re-export `JjBackend` |
| Modify `crates/vox-orchestrator/src/workspace.rs` | Replace feature-gated `JjBridge` calls with `VcsBackend` |
| Modify `crates/vox-cli/src/commands/vcs.rs` | Route `vox vcs` through `vox-vcs` (drop `jj` subprocess) |
| Delete from `crates/vox-orchestrator/src/jj_backend.rs` | `ContentMerge`, `OperationDag` (+ `lib.rs` re-exports) |
| Modify `crates/vox-orchestrator/Cargo.toml`, `crates/vox-git/Cargo.toml` | Remove `jj-backend` feature + optional jj-lib |
| Delete `crates/vox-git/src/sync.rs` fetch/push stubs | Real fetch/push lives in `vox-vcs` |
| Modify `docs/src/architecture/layers.toml` | Add `Command::new("jj")` forbidden pattern |

---

### Task 1: SPIKE — `JjBackend::open` + `snapshot` + `changes`, green against a temp colocated repo

**This task de-risks the entire phase.** Its deliverable is a *working* `JjBackend` for two methods,
plus a documented async↔sync bridge and the verified construction objects. Expect iteration — the
TDD red→green loop is the API-discovery mechanism for jj-lib's unstable surface.

**Files:** Modify `crates/vox-vcs/Cargo.toml`; Create `crates/vox-vcs/src/jj_backend.rs`; Modify `crates/vox-vcs/src/lib.rs`.

- [ ] **Step 1: Add deps**

In `crates/vox-vcs/Cargo.toml`:
```toml
[dependencies]
# ... existing serde, thiserror, workspace-hack ...
jj-lib = { workspace = true }                 # now =0.42.0, non-optional — vox-vcs is the confinement crate
tokio  = { workspace = true, features = ["rt", "rt-multi-thread"] }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Write the failing integration test**

Create `crates/vox-vcs/src/jj_backend.rs` with the test first:
```rust
#[cfg(test)]
mod tests {
    use super::JjBackend;
    use crate::backend::VcsBackend;
    use std::path::PathBuf;

    #[test]
    fn open_snapshot_and_list_changes() {
        let dir = tempfile::tempdir().unwrap();
        // Create one file so there is something to snapshot.
        std::fs::write(dir.path().join("hello.txt"), b"hi").unwrap();

        let mut be = JjBackend::open(dir.path()).expect("init colocated jj repo");
        let id = be.snapshot(Some("first"), vec![PathBuf::from("hello.txt")]).unwrap();
        let changes = be.changes().unwrap();
        assert!(changes.iter().any(|c| c.id == id), "snapshot must appear in the change/op log");
    }
}
```

- [ ] **Step 3: Run to confirm it fails**

Run: `cargo test -p vox-vcs jj_backend`
Expected: FAIL — `JjBackend` undefined.

- [ ] **Step 4: Implement `JjBackend::open` + `snapshot` + `changes`**

Prepend to `jj_backend.rs`. The structure below uses the confirmed jj-lib 0.42 entry points; **resolve
the exact construction of `UserSettings`, `Signer`, and the op-log read by compiling against jj-lib
0.42 and iterating to green** (these are the bits the spike exists to pin down). Keep ALL `jj_lib::`
usage in this file.

```rust
//! `JjBackend` — the in-process jj-lib 0.42 engine. THE ONLY place `jj_lib::` is called.
//!
//! jj-lib's workspace init/commit calls are async; `VcsBackend` is sync, so this type owns a
//! current-thread tokio runtime and `block_on`s jj-lib's futures. jj-lib also "cannot read config
//! from the home dir / env" by design, so we construct `UserSettings` from an explicit in-memory
//! config (a fixed bot identity for agent commits).

use crate::backend::{VcsBackend, VcsError};
use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// OFFLOAD-THREAD BRIDGE (corrected design). All jj-lib state lives on one dedicated worker
// thread that hosts its own current-thread runtime; the public type is a sync handle that ships
// closures to it. This is safe to call from inside the orchestrator's tokio runtime (Task 5)
// because the `block_on` runs on the worker thread, never on a tokio worker.
type Job = Box<dyn FnOnce(&mut JjState) + Send>;

/// jj-lib state — NEVER crosses the worker-thread boundary (Workspace is not Sync-friendly).
struct JjState {
    rt: tokio::runtime::Runtime, // current-thread; only this thread ever calls block_on
    workspace: jj_lib::workspace::Workspace,
    repo: Arc<jj_lib::repo::ReadonlyRepo>,
}

pub struct JjBackend {
    tx: std::sync::mpsc::Sender<Job>,
    _worker: std::thread::JoinHandle<()>,
}

impl JjBackend {
    /// Open (init colocated if absent) a jj workspace at `root`, on a dedicated worker thread.
    pub fn open(root: &Path) -> Result<Self, VcsError> {
        let root = root.to_path_buf();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), VcsError>>();
        let (job_tx, job_rx) = std::sync::mpsc::channel::<Job>();
        let worker = std::thread::Builder::new()
            .name("jj-backend".into())
            .spawn(move || {
                // Build the per-thread runtime + workspace; report readiness, then serve jobs.
                let init = (|| -> Result<JjState, VcsError> {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| VcsError::Unavailable(e.to_string()))?;
                    let settings = bot_settings()?; // sync: UserSettings::from_config(StackedConfig)
                    // load() is sync; if the workspace doesn't exist yet, init_colocated_git is async.
                    let (workspace, repo) = rt
                        .block_on(jj_lib::workspace::Workspace::init_colocated_git(&settings, &root))
                        .map_err(|e| VcsError::Unavailable(format!("jj init: {e}")))?;
                    Ok(JjState { rt, workspace, repo })
                })();
                match init {
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                    Ok(mut state) => {
                        let _ = ready_tx.send(Ok(()));
                        while let Ok(job) = job_rx.recv() {
                            job(&mut state); // each method's closure runs here, may block_on safely
                        }
                    }
                }
            })
            .map_err(|e| VcsError::Unavailable(e.to_string()))?;
        ready_rx
            .recv()
            .map_err(|e| VcsError::Unavailable(e.to_string()))??;
        Ok(Self { tx: job_tx, _worker: worker })
    }

    /// Run `f` on the worker thread and return its result (the sync↔async seam every method uses).
    fn call<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut JjState) -> Result<R, VcsError> + Send + 'static,
    ) -> Result<R, VcsError> {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        self.tx
            .send(Box::new(move |st| {
                let _ = reply_tx.send(f(st));
            }))
            .map_err(|_| VcsError::Unavailable("jj worker thread gone".into()))?;
        reply_rx
            .recv()
            .map_err(|_| VcsError::Unavailable("jj worker thread dropped reply".into()))?
    }
}

// Deterministic bot UserSettings (no home/env reads). Build a StackedConfig with user.name/email.
// VERIFIED: UserSettings::from_config(StackedConfig) (settings.rs:135); StackedConfig::with_defaults()
// (config.rs:660) then layer in the bot identity. Pin the exact config-insert call in the spike.
fn bot_settings() -> Result<jj_lib::settings::UserSettings, VcsError> {
    todo!("spike: StackedConfig::with_defaults() + insert user.name/user.email → UserSettings::from_config; NO todo! may remain at commit")
}

impl VcsBackend for JjBackend {
    fn snapshot(&mut self, label: Option<&str>, _paths: Vec<PathBuf>) -> Result<ChangeId, VcsError> {
        let label = label.map(str::to_owned);
        // Closure runs ON THE WORKER THREAD, so block_on is legal here.
        self.call(move |st| {
            // 1) async snapshot the working copy (TreeState::snapshot, SnapshotOptions with
            //    GitIgnoreFile::empty() + EverythingMatcher), 2) sync start_transaction +
            //    repo_mut, 3) async tx.commit(label). Map the new commit/change id → ChangeId.
            //    Use st.rt.block_on(...) for the async steps. Stable id mapping pinned in-spike.
            let _ = (&st.workspace, &st.repo, &label);
            todo!("resolve in spike")
        })
    }
    fn changes(&self) -> Result<Vec<Change>, VcsError> {
        self.call(|st| {
            // st.rt.block_on(op_walk::get_current_head_ops(st.repo.loader())) → project ops →
            // Vec<Change> newest→oldest. (changes() is &self in the trait, so `call` takes &self.)
            let _ = &st.repo;
            todo!("resolve in spike")
        })
    }
    fn diff(&self, _a: Option<ChangeId>, _b: Option<ChangeId>) -> Result<Diff, VcsError> {
        Ok(Diff::default()) // implemented in Task 3
    }
    fn undo(&mut self) -> Result<ChangeId, VcsError> { Err(VcsError::Unavailable("undo: Task 3".into())) }
    fn conflicts(&self) -> Result<Vec<Conflict>, VcsError> { Ok(Vec::new()) } // Task 3
    fn resolve(&mut self, _p: &Path, _s: ResolveStrategy) -> Result<(), VcsError> {
        Err(VcsError::Unavailable("resolve: Task 3".into()))
    }
}
```

> **Note on `&self` vs `&mut self`:** `changes()`/`conflicts()`/`diff()` take `&self` in the trait, so
> `call` is defined on `&self` (it only needs the `Sender`, which is `Clone`/shareable). The worker owns
> the single `&mut JjState`, so there is no aliasing problem even though jj-lib mutates internally.

**Discipline:** the `todo!()`s above are *spike markers for this task only* — Task 1 is NOT done until
`bot_settings`, `snapshot`, and `changes` are real and the test is green. No `todo!()` may remain in a
committed file (the repo's no-stub policy). If a method genuinely belongs to a later task, return
`Err(VcsError::Unavailable("... : Task N"))` as shown for `undo`/`resolve` (an honest "not yet",
not a stub of fake behavior).

- [ ] **Step 5: Register the module**

In `crates/vox-vcs/src/lib.rs`: add `pub mod jj_backend;` and `pub use jj_backend::JjBackend;`.

- [ ] **Step 6: Iterate to green**

Run: `cargo test -p vox-vcs jj_backend` (jj-lib is a large dep — first build is SLOW; use a long
timeout). Iterate on `bot_settings`/`snapshot`/`changes` until PASS. At the top of `jj_backend.rs`,
add a short comment block recording the verified construction (UserSettings ctor, op-log read call,
change-id mapping) so Tasks 2-4 reuse it.

- [ ] **Step 7: Format + commit**

```bash
cargo fmt -p vox-vcs
git add crates/vox-vcs/Cargo.toml crates/vox-vcs/src/jj_backend.rs crates/vox-vcs/src/lib.rs
git commit -m "feat(vox-vcs): JjBackend spike — colocated init + snapshot + change log (jj-lib 0.42)"
```

**If the spike cannot be made green** (e.g. an init factory is not constructible without more of
jj-lib's plumbing than is reasonable), STOP and report BLOCKED with the specific jj-lib API obstacle —
do not fabricate. The fallback is to narrow `JjBackend` to read-only ops first, or to escalate the
async/construction obstacle to the human.

---

### Task 2: `JjBackend` — backend selection (`detect`) + orchestrator/CLI binding

**Files:** Modify `crates/vox-vcs/src/backend.rs`; Modify `crates/vox-vcs/src/jj_backend.rs`.

- [ ] **Step 1: Failing test for `detect`**

Add to `backend.rs` tests:
```rust
#[test]
fn detect_prefers_jj_when_initable() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(detect(dir.path()), VcsBackendKind::Jj);
}
```
(Add `tempfile` to dev-deps if not already present from Task 1.)

- [ ] **Step 2: Run → FAIL** (`detect` still hard-returns `Cas`). Run: `cargo test -p vox-vcs detect_prefers_jj`.

- [ ] **Step 3: Implement detection**

Replace `detect` in `backend.rs`:
```rust
/// Prefer the jj engine when `repo_root` is a writable dir we can host a jj workspace in;
/// otherwise fall back to the in-memory CAS engine.
pub fn detect(repo_root: &Path) -> VcsBackendKind {
    if crate::jj_backend::JjBackend::is_supported(repo_root) {
        VcsBackendKind::Jj
    } else {
        VcsBackendKind::Cas
    }
}
```
Add a cheap `JjBackend::is_supported(root: &Path) -> bool` (e.g. `root.is_dir()` and a `.jj`/`.git`
present-or-creatable check — confirm against jj-lib; keep it side-effect-free, do NOT init here).

- [ ] **Step 4: Run → PASS.** Run: `cargo test -p vox-vcs`.

- [ ] **Step 5: Provide a constructor for callers**

Add `pub fn boxed_for(root: &Path) -> Box<dyn VcsBackend>` to `backend.rs` returning `JjBackend::open`
boxed when `detect == Jj` (falling back to `CasFallback` on open error), else `CasFallback`. Unit-test
that it returns a usable backend for a temp dir.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-vcs
git add crates/vox-vcs/src/backend.rs crates/vox-vcs/src/jj_backend.rs
git commit -m "feat(vox-vcs): runtime backend selection (detect prefers Jj) + boxed_for"
```

---

### Task 3: `JjBackend` — `undo`, `diff`, `conflicts`, `resolve`

**Files:** Modify `crates/vox-vcs/src/jj_backend.rs`.

- [ ] **Step 1: Failing tests** (append to `jj_backend.rs` tests), each against a temp colocated repo:
```rust
#[test]
fn undo_returns_to_previous_change() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), b"1").unwrap();
    let mut be = JjBackend::open(dir.path()).unwrap();
    be.snapshot(Some("c1"), vec![]).unwrap();
    std::fs::write(dir.path().join("a.txt"), b"2").unwrap();
    let _c2 = be.snapshot(Some("c2"), vec![]).unwrap();
    let restored = be.undo().unwrap();
    assert!(be.changes().unwrap().iter().any(|c| c.id == restored));
}
```
(Add `diff`/`conflicts` tests mirroring the same temp-repo setup — assert a two-change diff lists the
changed path, and that a synthesized conflict surfaces via `conflicts()`.)

- [ ] **Step 2: Run → FAIL** (`undo` returns `Unavailable`). Run: `cargo test -p vox-vcs jj_backend`.

- [ ] **Step 3: Implement** the four methods via jj-lib `op_walk` (undo = check out the previous
operation's view), `merge`/`conflicts` (materialize conflict sides → `Conflict`), and a two-tree diff.
Use the runtime bridge + the construction recorded in Task 1. Keep all `jj_lib::` in this file.

- [ ] **Step 4: Run → PASS.** Run: `cargo test -p vox-vcs`.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-vcs
git add crates/vox-vcs/src/jj_backend.rs
git commit -m "feat(vox-vcs): JjBackend undo/diff/conflicts/resolve via jj-lib op log"
```

---

### Task 4: `JjBackend` fetch/push via pure `gix` — gix-maturity de-risk

**This is the spec's top external risk.** Implement and prove remote interop against a throwaway LOCAL
bare repo (no network).

**Files:** Modify `crates/vox-vcs/src/jj_backend.rs`; add `fetch`/`push` to the `VcsBackend` trait in
`backend.rs` (and a no-op/`Unavailable` impl on `CasFallback`).

- [ ] **Step 1: Extend the trait**

In `backend.rs`, add to `VcsBackend`:
```rust
    fn fetch(&mut self, remote: &str) -> Result<(), VcsError>;
    fn push(&mut self, remote: &str, change: ChangeId) -> Result<(), VcsError>;
```
Implement on `CasFallback` as `Err(VcsError::Unavailable("fetch/push require the jj backend".into()))`.

- [ ] **Step 2: Failing integration test** (append to `jj_backend.rs` tests):
```rust
#[test]
fn push_then_fetch_against_local_bare_remote() {
    let work = tempfile::tempdir().unwrap();
    let remote = tempfile::tempdir().unwrap();
    // init a bare git repo at `remote` (via gix or jj-lib), register it as remote "origin"
    // in the colocated workspace, snapshot a change, push it, then fetch into a second clone
    // and assert the change is present. Exact remote-registration call resolved against jj-lib's
    // `git` module.
    std::fs::write(work.path().join("f.txt"), b"x").unwrap();
    let mut be = JjBackend::open(work.path()).unwrap();
    // ... register remote, snapshot, push, fetch, assert ...
    let _ = remote; // placeholder binding until the remote wiring is filled in
}
```

- [ ] **Step 3: Run → FAIL.** Run: `cargo test -p vox-vcs push_then_fetch`.

- [ ] **Step 4: Implement** `fetch`/`push` via `jj_lib::git` (pure gix). Iterate to green.

  **If gix push is not functional in jj-lib 0.42** (the known maturity risk): do NOT fake it. Mark the
  test `#[ignore]` with a comment citing the limitation, return `Err(VcsError::Unavailable("gix push
  unsupported in jj-lib 0.42 — see <issue>"))` from `push`, and report DONE_WITH_CONCERNS so the
  controller can surface the limitation to the human. Fetch is expected to work regardless.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-vcs
git add crates/vox-vcs/src/backend.rs crates/vox-vcs/src/cas_fallback.rs crates/vox-vcs/src/jj_backend.rs
git commit -m "feat(vox-vcs): git fetch/push via jj-lib (pure gix) + local-remote integration test"
```

---

### Task 5: Replace the feature-gated `JjBridge` subprocess in the orchestrator

**Files:** Modify `crates/vox-orchestrator/Cargo.toml` (depend on `vox-vcs`); Modify
`crates/vox-orchestrator/src/workspace.rs` (the `#[cfg(feature = "jj-backend")]` block at ~line 273).

- [ ] **Step 1:** Add `vox-vcs = { workspace = true }` to `crates/vox-orchestrator/Cargo.toml`
`[dependencies]`. Build to confirm the L3→L3 edge is legal (arch-check) and resolves.

- [ ] **Step 2: Failing test** — add a `workspace.rs` test that, given a temp repo, calls the new
merge/abandon hook and asserts it routes through a `VcsBackend` (inject a `CasFallback` or a fake to
assert the call happens without the `jj-backend` feature). Run → FAIL.

- [ ] **Step 3:** Replace the `#[cfg(feature = "jj-backend")]` block in `update_change_status`
(currently calling `JjBridge::flush_snapshot_commit`/`revert_agent_snapshot` subprocesses) with calls
through a `VcsBackend` handle (snapshot on `Merged`, undo on `Abandoned`). Remove the `#[cfg]` gate so
it runs in default builds. Keep it fire-and-forget/best-effort as today. **This call site is inside the
orchestrator's tokio runtime — it is the exact reason the offload-thread bridge (Task 1) is mandatory;
a `block_on`-in-place `JjBackend` would panic here.** The sync `VcsBackend` methods are safe to call
directly from this async context (they only `send`+`recv` on channels); if the best-effort hook must not
block the async task even briefly, wrap the call in `tokio::task::spawn_blocking`. Hold the `JjBackend`
handle (or a `Box<dyn VcsBackend>` from `boxed_for`) on the orchestrator so the worker thread is reused,
not respawned per hook.

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-orchestrator workspace`.

- [ ] **Step 5: Commit** `feat(orchestrator): route workspace VCS hooks through vox-vcs (drop jj subprocess)`.

---

### Task 6: Route the `vox vcs` CLI through `vox-vcs` (drop the `jj` subprocess)

**Files:** Modify `crates/vox-cli/src/commands/vcs.rs` (currently shells out to the `jj` binary).

- [ ] **Step 1:** Add `vox-vcs` dep to `crates/vox-cli/Cargo.toml` (L3→L5 edge is legal).
- [ ] **Step 2: Failing test** for one subcommand (e.g. `vox vcs log` returns changes from the backend
on a temp repo). Run → FAIL.
- [ ] **Step 3:** Reimplement `init`/`status`/`diff`/`log`/`merge` via `vox_vcs::backend::boxed_for(cwd)`
instead of `jj(&[...])` subprocess calls. Preserve the CLI's JSON-output behavior. Remove the `jj()`
subprocess helper. (`annotate`/`heatmap` may stay subprocess-free only if jj-lib exposes blame; if
not, keep them out of scope and note it — do not delete features that lack a jj-lib equivalent.)
- [ ] **Step 4: Run → PASS.** **Step 5: Commit** `refactor(cli): vox vcs runs in-process via vox-vcs (no jj binary)`.

---

### Task 7: TDD-gated deletion of the obviated hand-rolled code

Each deletion is preceded by confirming zero remaining consumers. **Do NOT trust the P0/P1 audit's
"dead" classification on its own** — prior retirement audits in this repo were wrong ~50% of the time
and one nearly deleted 9,670 lines of live integration tests (see `feedback_verify_audit_retirement_claims`).
A LoC/Cargo-graph "dead" verdict is necessary but **not sufficient**; hand-verify across the whole tree,
not just `crates/`.

**Files:** `crates/vox-orchestrator/src/jj_backend.rs`, `crates/vox-orchestrator/src/lib.rs:303`,
`crates/vox-orchestrator/Cargo.toml`, `crates/vox-git/Cargo.toml`, `crates/vox-git/src/sync.rs`.

- [ ] **Step 1 (BROADENED — gate the deletion):** grep the **entire repo**, not just `crates/`, for each
symbol and confirm the only hits are the definition, its own `#[cfg(test)]` block, and the `lib.rs:303`
re-export:
```bash
for sym in ContentMerge OperationDag DagNodeId MergeSide JjBridge; do
  echo "== $sym =="; grep -rn "$sym" crates/ tests/ examples/ contracts/ .github/ docs/ scripts/ 2>/dev/null
done
```
Check in particular: integration-test crates, `examples/**/*.vox` goldens, `contracts/**`, CI workflow
YAML, and any ADR/spec that names them as a contract. **Paste the full output into the task log.** If a
symbol has ANY consumer outside (definition + own tests + the one re-export), STOP and report — that
symbol is NOT dead; remove it from this deletion set and note why. Only symbols with a clean sweep proceed
to Step 2. (The orchestrator build+test in Step 4 is the backstop, but it will NOT catch a golden `.vox`
program or a CI script that references a public symbol — hence the repo-wide grep.)
- [ ] **Step 2:** Delete `ContentMerge`, `OperationDag`, `DagNodeId`, `MergeSide`, `JjBridge` and their
tests from `crates/vox-orchestrator/src/jj_backend.rs`; remove the `pub use jj_backend::{...}` re-export
at `lib.rs:303`. (If `jj_backend.rs` becomes empty, delete the file + its `mod` line.)
- [ ] **Step 3:** Remove the `jj-backend` feature + optional `jj-lib` dep from
`crates/vox-orchestrator/Cargo.toml` and `crates/vox-git/Cargo.toml`. Delete the now-unused
`FetchResult`/`PushResult`/`SyncStatus` fetch/push **stubs** in `crates/vox-git/src/sync.rs` (keep any
types still consumed by CodeRabbit/effort-audit — grep first; vox-git stays the git *reader*).
- [ ] **Step 4: Verify nothing broke.** `cargo build -p vox-orchestrator -p vox-git` + `cargo test -p vox-orchestrator`. Run → PASS.
- [ ] **Step 5: Commit** `refactor: delete jj-shaped code obviated by jj-lib (ContentMerge, OperationDag, JjBridge, sync stubs)`.

---

### Task 8: Ban the `jj` subprocess in arch-check

**Files:** Modify `docs/src/architecture/layers.toml`.

- [ ] **Step 1:** Confirm no `Command::new("jj")` remains: `grep -rn 'Command::new("jj")' crates/` → empty.
- [ ] **Step 2:** Add a `[[forbidden_pattern]]` (mirroring `raw-git-exec`):
```toml
[[forbidden_pattern]]
name             = "raw-jj-exec"
pattern          = 'Command::new\("jj"\)'
file_glob        = "crates/**/*.rs"
exempt_files     = []
allow_annotation = "// vox-arch-check: allow jj-exec"
reason           = "jj is used in-process via jj-lib (vox-vcs::JjBackend); no subprocess jj invocation is allowed."
```
- [ ] **Step 3: Run** `cargo run -p vox-arch-check` → exit 0. **Step 4: Commit** `build(arch-check): forbid raw jj subprocess (jj-lib is in-process now)`.

---

## Self-Review

- **Spec coverage (P2 row):** build `JjBackend` (T1-T4) ✓; replace `JjBridge` + `vox vcs` subprocess +
  `sync.rs` stubs (T5-T7) ✓; remove `jj-backend` feature (T7) ✓; delete `ContentMerge`/`OperationDag`
  (T7) ✓; real fetch/push integration test (T4) ✓; no `Command::new("jj")` remains (T8) ✓; demote
  `SnapshotStore`/`OpLog` — they are simply not used by `JjBackend`; the orchestrator keeps them for
  `CasFallback`/audit (no deletion needed, consistent with the spec's "demote not delete").
- **Unstable-API honesty:** Task 1 is an explicit spike; the `todo!()`s there are call-outs that MUST
  be resolved before commit (no-stub policy), and genuinely-later methods return `Unavailable("Task N")`
  rather than faking behavior. BLOCKED/DONE_WITH_CONCERNS escalation paths are specified for the two
  real risks (construction plumbing, gix push maturity).
- **Layering:** `vox-vcs` (L3) gains `vox-orchestrator`(L3)/`vox-cli`(L5) consumers — legal, and clears
  the P0 `orphan_exempt` (can be removed once T5/T6 land a real consumer; note it in T5).
- **Type consistency:** `JjBackend`, `VcsBackend`, `VcsError`, `ChangeId`, `detect`, `boxed_for`,
  `is_supported` are used consistently; the new `fetch`/`push` trait methods get impls on both backends.

---

## Scope-correction changelog (2026-06-06)

This plan was made implementation-ready after verifying the jj-lib 0.42 API against the installed crate
source (the original draft used unverified docs.rs guesses). Material changes:

1. **Async↔sync bridge redesigned** (the load-bearing fix). The original "JjBackend owns a tokio runtime
   and `block_on`s in-place" design **panics** when invoked from the orchestrator's tokio runtime (Task 5)
   — nested `block_on` is illegal. Replaced with the **offload-thread bridge** (a dedicated worker thread
   hosting its own current-thread runtime; sync methods ship closures over a channel). Verified that
   `init_colocated_git`, `Transaction::commit`, `TreeState::snapshot`, and `op_walk::get_current_head_ops`
   are all **async**, while `Workspace::load`, `UserSettings::from_config`, `start_transaction`, and
   `Signer::new` are sync — so the bridge only blocks on the worker thread.
2. **`SnapshotOptions` construction** documented (needs `GitIgnoreFile::empty()` + `EverythingMatcher` +
   `max_new_file_size`) — previously hand-waved.
3. **Deletion ledger (Task 7) hardened** to a repo-wide grep gate (not just `crates/`), per the repo's
   verify-audit-retirement-claims discipline — a LoC/graph "dead" verdict is necessary-but-insufficient.

## Phases beyond P2 (remaining original-roadmap items — unscoped, listed for completeness)

The research roadmap (`docs/src/architecture/vcs-as-vox-language-feature-jujutsu-2026.md` §6) has two
phases past the merged P0/P1/P3 + this P2 that have **no implementation plan yet**. They are NOT blocked
by anything once P2 lands; each warrants its own spec→plan cycle:

- **Orchestrator isolation policy + GUI** (research §5.1, §5.4): the three orchestrator-chosen isolation
  strategies (N agents on one branch / split branches / worktree-per-agent), a conflict surface, and full
  user control via the Vox GUI + config. *Partly seeded already:* P1 landed overlap-detection + conflict
  recording (`json_vcs_facade` / `merge_conflicts`); what's missing is the **strategy selector** (config +
  `repo.*`/orchestrator API) and the **GUI panel** (wire through the existing `SURFACE_REGISTRY`). Depends
  on P2 only for the real backend; the policy layer is otherwise orchestrator-side.
- **Decorators + auto-snapshot-on-effect** (research §4.3): `@versioned`/`@tracked` decorators that
  auto-checkpoint at effect boundaries (snapshot before a `uses fs`/`uses vcs` mutation). Builds on the
  P3 `Vcs` effect + `repo.*` surface; needs decorator lowering + an interpreter hook that calls
  `repo.snapshot` automatically. Self-contained in `vox-compiler`; no jj-lib dependency.

When P2 is green, the natural completion order is **isolation-policy+GUI** (highest user-visible leverage,
per the research doc's ranking) then **decorators** (ergonomics). Each should follow
`superpowers:writing-plans` to produce its own task-by-task plan before execution.
