# P2 — JjBackend (jj-lib 0.42 engine) + Dead-Code Removal Implementation Plan

> **✅ STATUS: IMPLEMENTED & MERGED (2026-06-06).** This plan is now historical. The jj engine
> shipped in `crates/vox-vcs/src/jj_backend.rs` + `jj_actor.rs` (commits `a85e40d1bd` spike →
> `ddb13d732e` jj-actor → `5f6f29264c` orchestrator wiring → `0209c9c7fa` reconcile). **The shipped
> design differs from the sketch below in one key way:** rather than the sync-trait + in-place
> `block_on` shown here, `VcsBackend` is an **`#[async_trait]`** and the `!Send` jj engine lives behind
> a dedicated-OS-thread **`jj_actor`** (a `Send + Sync` `JjActorHandle`). That actor *is* the
> "offload-thread bridge" this plan's scope-correction recommended — it just wraps an async trait
> instead of a sync one. Tasks 1–5, 7, 8 are DONE; **Task 6 (route the `vox vcs` CLI in-process) is
> intentionally deferred** behind a documented `layers.toml` `raw-jj-exec` exemption, pending jj-lib
> exposing blame/rebase in-process. Verified by 20 green `vox-vcs` tests (7 jj-lib surface
> characterizations + 4 jj-actor tests). Next phases are P4 (isolation policy + GUI) and P5
> (`@versioned` decorators) — see the sibling `2026-06-06-jj-first-class-p4-*` / `p5-*` plans.

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

> **✅ SPIKE (Task 1) DONE & VERIFIED (2026-06-06, branch `cc_bdesktop2/jj-p2-spike`, commit `45d0e2e656`).**
> The whole phase is validated: a real in-process `JjBackend` does colocated init + working-copy
> snapshot + change-as-commit + op-log read, green in ~1.9 min cold build, no `jj` binary, clippy clean,
> arch-check exit 0. **Two findings every later task must honor:**
> 1. **jj-lib needs `features = ["git"]`** in `vox-vcs/Cargo.toml` (the root pins `default-features = false`, so `init_colocated_git` is absent without it).
> 2. **`Workspace` is `Send` but `!Sync`** (owns `Box<dyn WorkingCopy>`); `VcsBackend: Send + Sync`, so `JjBackend` wraps the `Workspace`+`Arc<ReadonlyRepo>` in a `std::sync::Mutex` and owns a current-thread tokio runtime that `block_on`s every jj future.
>
> **Verified construction (reuse in Tasks 2-4):** `UserSettings::from_config(StackedConfig::with_defaults() + ConfigLayer::parse(ConfigSource::User, "user.name=…\nuser.email=…"))`; snapshot = `workspace.start_working_copy_mutation()` → `locked_wc().snapshot(SnapshotOptions{ base_ignores: GitIgnoreFile::empty(), matchers: &EverythingMatcher, max_new_file_size: u64::MAX, .. })` → `start_transaction()` → `tx.repo_mut().new_commit(vec![parent], tree).set_description(..).write().await` → `set_wc_commit` → `tx.commit(desc).await` → re-finish wc lock against the new op id; op-log = `op_walk::walk_ancestors(&[repo.operation().clone()])` collected via `futures::TryStreamExt::try_collect`. `WorkspaceName`/`WorkspaceNameBuf` live in `jj_lib::ref_name`; `id.hex()` needs `jj_lib::object_id::ObjectId` in scope.
> **Open identity choice (Task 3):** the spike's `ChangeId` hashes the jj **operation id**. If P2 needs identity stable across commit rewrites, switch the hash source to the commit's jj **`ChangeId`** (reachable from the commit builder).

> **✅ SPIKE ROUND 2 — DONE & VERIFIED (2026-06-06, branch `cc_bdesktop2/jj-p2-spike`, commits `297ada9e6f` async + `a85e40d1bd` ops; 12 tests green, arch-check 0). Two findings REVISE this plan's design — read before executing:**
>
> **A. jj-lib 0.42 is irreducibly `!Send` → `VcsBackend` is ASYNC and the production engine is a JJ-ACTOR.**
> A `Send` async trait does not compile (jj `Transaction`/`MutableRepo`/working-copy use `RefCell`/`OnceCell`/`Cell`). The spike landed `#[async_trait(?Send)]`, which **fixes the proven nested-runtime panic** (a sync `block_on` engine panics when called from the orchestrator's tokio runtime), but `?Send` futures **cannot cross `tokio::spawn`** — and the orchestrator's jj hook (`workspace.rs` `spawn_supervised_infallible`) spawns. **Production design: a jj-actor** — a dedicated OS thread owns the `Workspace`, receives commands over an `mpsc` channel, replies via `oneshot`, and exposes a clean **`Send` async `VcsBackend`** to the rest of Vox (all `!Send` jj futures confined to the actor thread). Tasks 5/6 (orchestrator/CLI wiring) MUST target the actor's `Send` API, not the `?Send` direct impl.
>
> **B. Remote push/fetch use the `git` BINARY, not gix (corrects spec D1).** jj-lib 0.42 routes `push`/`fetch` through `git_subprocess` (`Command::new(git)`). **All LOCAL ops are fully in-process (no binary).** Remote sync requires **`git` ≥ 2.41 on PATH** (decision: accept for remote only; degrade gracefully with a clear "git required" error when absent). The `jj` binary is still never needed. Two notes: jj caches remote config at workspace-open time (register a remote *before* the backend that pushes, or reopen); in-process `git::add_remote` pulls `gix` in (jj doesn't re-export it) → use the existing `GitExec` `git remote add` instead.
>
> **Operations PROVEN against jj-lib 0.42 (Tasks 2-4 de-risked):** open-existing-repo (`Workspace::load` load-or-init), undo (parent-op restore), diff (`MergedTree::diff_stream`), **conflicts-as-data (`MergedTree::merge` + `has_conflict()` + `tree.conflicts()` + `materialize_tree_value` → both sides readable — the killer feature works)**, push (`git::push_refs` to a local bare repo, end-to-end). A `ChangeId → CommitId` side-table in `JjState` resolves our opaque ids back to jj commits.
>
> **Task-level revisions:** Task 1's sync `block_on` `JjBackend` is SUPERSEDED by the async `?Send` impl on the spike branch; the *next* build step is the **jj-actor wrapper** (a new Task 1.5 — production `Send` API). Task 4 "fetch/push via pure gix" → **via jj-lib's git-subprocess** (git binary, graceful degrade), not gix; the "gix push maturity" risk is MOOT. Task 8's `raw-jj-exec` ban still holds (no subprocess *jj*), but note remote sync legitimately spawns *git* (already covered by the existing `raw-git-exec` exemption mechanism).

**Confirmed jj-lib 0.42 anchors (from docs.rs):**
- `Workspace::init_colocated_git(user_settings: &UserSettings, workspace_root: &Path) -> Result<(Workspace, Arc<ReadonlyRepo>), WorkspaceInitError>` (async)
- `Workspace::load(user_settings: &UserSettings, workspace_path: &Path, store_factories: &StoreFactories, working_copy_factories: &WorkingCopyFactories) -> Result<Workspace, WorkspaceLoadError>`
- `workspace.repo_loader() -> &RepoLoader`, `workspace.working_copy() -> &dyn WorkingCopy`
- Transaction: `repo.start_transaction()` → `tx.repo_mut()` (`MutableRepo`) → `tx.commit(description)` → new `Arc<ReadonlyRepo>`
- Op log / conflicts / git fetch-push: modules `op_store`/`op_walk`, `merge`/`conflicts`, `git`/`git_backend` (exact calls resolved in Task 1's spike).

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

pub struct JjBackend {
    rt: tokio::runtime::Runtime,
    workspace: jj_lib::workspace::Workspace,
    repo: Arc<jj_lib::repo::ReadonlyRepo>,
}

impl JjBackend {
    /// Open (init colocated if needed) a jj workspace at `root`.
    pub fn open(root: &Path) -> Result<Self, VcsError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| VcsError::Unavailable(e.to_string()))?;
        let settings = Self::bot_settings()?;             // resolve UserSettings construction in-spike
        let (workspace, repo) = rt
            .block_on(jj_lib::workspace::Workspace::init_colocated_git(&settings, root))
            .map_err(|e| VcsError::Unavailable(format!("jj init: {e}")))?;
        Ok(Self { rt, workspace, repo })
    }

    // Construct a deterministic bot UserSettings (no home/env reads). EXACT API resolved in-spike.
    fn bot_settings() -> Result<jj_lib::settings::UserSettings, VcsError> {
        // e.g. UserSettings::from_config(StackedConfig with user.name/user.email) — confirm the
        // 0.42 constructor name + config type by compiling; map errors to VcsError::Unavailable.
        todo!("resolve in spike — replace before this task is marked done; NO todo! may remain")
    }
}

impl VcsBackend for JjBackend {
    fn snapshot(&mut self, label: Option<&str>, _paths: Vec<PathBuf>) -> Result<ChangeId, VcsError> {
        // 1) snapshot the working copy into the store, 2) start a transaction, 3) describe/commit.
        // Use workspace.working_copy() + repo.start_transaction()/tx.repo_mut()/tx.commit(label).
        // Map the resulting jj change/commit id into our ChangeId (stable mapping resolved in-spike).
        todo!("resolve in spike")
    }
    fn changes(&self) -> Result<Vec<Change>, VcsError> {
        // Walk the operation log (op_walk/op_store) newest→oldest, project each op into a Change.
        todo!("resolve in spike")
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
it runs in default builds. Keep it fire-and-forget/best-effort as today.

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

Each deletion is preceded by confirming zero remaining consumers (grep) — the P0/P1 audit already
classified `ContentMerge`/`OperationDag` as dead (tests + a `lib.rs` re-export only).

**Files:** `crates/vox-orchestrator/src/jj_backend.rs`, `crates/vox-orchestrator/src/lib.rs:303`,
`crates/vox-orchestrator/Cargo.toml`, `crates/vox-git/Cargo.toml`, `crates/vox-git/src/sync.rs`.

- [ ] **Step 1:** `grep -rn "ContentMerge\|OperationDag\|DagNodeId\|MergeSide\|JjBridge" crates/` — confirm
the only references are the definitions, their own tests, and the `lib.rs:303` re-export. Paste output.
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
