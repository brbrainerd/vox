//! The real jj-lib 0.42 VCS engine. **All** `jj_lib::` calls in the workspace
//! are confined to this file (enforced by `vox-arch-check`'s `jj-lib-confined`).
//!
//! ## Verified jj-lib 0.42 construction (spike, P2 Task 1)
//!
//! Read directly from the installed crate source at
//! `~/.cargo/registry/src/*/jj-lib-0.42.0/src/{workspace,repo,transaction,
//! settings,config,working_copy,op_walk,object_id}.rs`. Source of truth, not memory.
//!
//! ### `UserSettings` (jj cannot read $HOME/env config; we supply an explicit one)
//! ```text
//! let mut config = StackedConfig::with_defaults();            // config.rs:660-ish
//! config.add_layer(ConfigLayer::parse(ConfigSource::User,     // config.rs:341
//!     "user.name = '...'\nuser.email = '...'")?);
//! let settings = UserSettings::from_config(config)?;          // settings.rs:135
//! ```
//!
//! ### Colocated init (ASYNC — must be driven by a runtime)
//! ```text
//! let (workspace, repo) =
//!     Workspace::init_colocated_git(&settings, workspace_root).await?;  // workspace.rs:223
//! ```
//! Returns `(Workspace, Arc<ReadonlyRepo>)`. `git` is a default feature.
//!
//! ### Snapshot the working copy + commit it as a change
//! The full sequence discovered (workspace.rs / working_copy.rs / repo.rs /
//! transaction.rs / commit_builder.rs):
//! 1. `let mut locked = workspace.start_working_copy_mutation().await?;`  (workspace.rs:442)
//! 2. Build `SnapshotOptions` (working_copy.rs:211): `base_ignores =
//!    GitIgnoreFile::empty()` (gitignore.rs:44), `start_tracking_matcher` &
//!    `force_tracking_matcher = &EverythingMatcher` (matchers.rs:120),
//!    `progress = None`, `max_new_file_size = u64::MAX`.
//! 3. `let (new_tree, _stats) = locked.locked_wc().snapshot(&options).await?;` (working_copy.rs:117)
//! 4. `let parent = repo.view().get_wc_commit_id(name).cloned();` (view.rs:58) —
//!    the workspace's current wc commit (its parent for the new change).
//! 5. `let mut tx = repo.start_transaction();`                          (repo.rs:333)
//! 6. `let commit = tx.repo_mut().new_commit(vec![parent], new_tree)`   (repo.rs:965)
//!    `.set_description(label).write().await?;`                         (commit_builder.rs:116/163)
//! 7. `tx.repo_mut().set_wc_commit(name, commit.id().clone())?;`        (repo.rs:1493)
//! 8. `let repo = tx.commit(description).await?;` -> new `Arc<ReadonlyRepo>`  (transaction.rs:124)
//! 9. `locked.finish(repo.op_id().clone()).await?;`                     (workspace.rs:491)
//!
//! ### Reading the op/change log
//! `op_walk::walk_ancestors(&[repo.operation().clone()])` (op_walk.rs:263) returns
//! a `Stream<Item = OpStoreResult<Operation>>`; collect with
//! `futures::TryStreamExt::try_collect`. Each `Operation` exposes
//! `.id() -> &OperationId` (operation.rs:97) and
//! `.metadata().description` (operation.rs:122 -> op_store.rs:420).
//!
//! ### Mapping jj ids -> our `ChangeId(u64)`
//! `OperationId` is an opaque byte id with `.hex()` (object_id.rs / op_store.rs:47).
//! We hash the hex string with `DefaultHasher` to a stable `u64`. The same op id
//! that `snapshot()` returns therefore reappears in `changes()`, satisfying the
//! contract that a snapshot's `ChangeId` is present in the change log.

use crate::backend::{VcsBackend, VcsError};
use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use async_trait::async_trait;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use jj_lib::config::{ConfigLayer, ConfigSource, StackedConfig};
use jj_lib::gitignore::GitIgnoreFile;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::object_id::ObjectId;
use jj_lib::op_walk;
use jj_lib::ref_name::{WorkspaceName, WorkspaceNameBuf};
use jj_lib::repo::ReadonlyRepo;
use jj_lib::settings::UserSettings;
use jj_lib::working_copy::SnapshotOptions;
use jj_lib::workspace::Workspace;

use futures::TryStreamExt;

/// A bot identity for op/commit metadata. jj-lib refuses to read config from
/// `$HOME`/env in-process, so we pin an explicit author.
const BOT_NAME: &str = "Vox VCS";
const BOT_EMAIL: &str = "vcs@vox.invalid";

/// Mutable jj engine state. `Workspace` owns a `Box<dyn WorkingCopy>` that is
/// `Send` but not `Sync`, so it lives behind a [`tokio::sync::Mutex`] — whose
/// guard is `Send` and may be held across `.await` (unlike `std::sync::Mutex`) —
/// to make [`JjBackend`] satisfy the `VcsBackend: Send + Sync` bound.
struct JjState {
    workspace: Workspace,
    repo: Arc<ReadonlyRepo>,
}

/// In-process jj engine. Awaits jj-lib's async APIs directly from the async
/// [`VcsBackend`] trait — **no internal tokio runtime**, so it never panics with
/// "Cannot start a runtime from within a runtime" when called from the
/// orchestrator's async context.
pub struct JjBackend {
    state: tokio::sync::Mutex<JjState>,
    #[allow(dead_code)]
    settings: UserSettings,
}

/// Stable `OperationId` (hex) -> `ChangeId(u64)` projection.
fn op_hex_to_change_id(hex: &str) -> ChangeId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    hex.hash(&mut h);
    ChangeId(h.finish())
}

impl JjBackend {
    /// Construct in-memory [`UserSettings`] with a fixed bot identity.
    fn bot_settings() -> Result<UserSettings, VcsError> {
        let mut config = StackedConfig::with_defaults();
        let layer = ConfigLayer::parse(
            ConfigSource::User,
            &format!("user.name = {BOT_NAME:?}\nuser.email = {BOT_EMAIL:?}\n"),
        )
        .map_err(|e| VcsError::Unavailable(format!("jj config parse: {e}")))?;
        config.add_layer(layer);
        UserSettings::from_config(config)
            .map_err(|e| VcsError::Unavailable(format!("jj UserSettings: {e}")))
    }

    /// Initialize a colocated jj/git repo at `root` and load its working copy.
    /// Awaits jj-lib's async init directly — call from within a tokio runtime.
    pub async fn open(root: &Path) -> Result<Self, VcsError> {
        let settings = Self::bot_settings()?;
        let (workspace, repo) = Workspace::init_colocated_git(&settings, root)
            .await
            .map_err(|e| VcsError::Unavailable(format!("jj init_colocated_git: {e}")))?;
        Ok(Self {
            state: tokio::sync::Mutex::new(JjState { workspace, repo }),
            settings,
        })
    }
}

#[async_trait(?Send)]
impl VcsBackend for JjBackend {
    async fn snapshot(
        &mut self,
        label: Option<&str>,
        _paths: Vec<PathBuf>,
    ) -> Result<ChangeId, VcsError> {
        let description = label.unwrap_or("").to_string();
        let name: WorkspaceNameBuf = WorkspaceName::DEFAULT.to_owned();
        let mut st = self.state.lock().await;

        let new_op_hex = {
            let JjState { workspace, repo } = &mut *st;

            // 1. Lock + snapshot the working copy into a tree, then release the
            //    lock against the current op id (a no-op state write).
            let new_tree = {
                let mut locked = workspace
                    .start_working_copy_mutation()
                    .await
                    .map_err(|e| VcsError::Unavailable(format!("jj wc lock: {e}")))?;
                let options = SnapshotOptions {
                    base_ignores: GitIgnoreFile::empty(),
                    progress: None,
                    start_tracking_matcher: &EverythingMatcher,
                    force_tracking_matcher: &EverythingMatcher,
                    max_new_file_size: u64::MAX,
                };
                let (tree, _stats) = locked
                    .locked_wc()
                    .snapshot(&options)
                    .await
                    .map_err(|e| VcsError::Unavailable(format!("jj wc snapshot: {e}")))?;
                locked
                    .finish(repo.op_id().clone())
                    .await
                    .map_err(|e| VcsError::Unavailable(format!("jj wc finish: {e}")))?;
                tree
            };

            // 2. Parent of the new change = current wc commit.
            let parent = repo
                .view()
                .get_wc_commit_id(&name)
                .cloned()
                .ok_or_else(|| VcsError::Unavailable("jj: no working-copy commit".into()))?;

            // 3. Commit the snapshot tree as a new change and publish the op.
            let mut tx = repo.start_transaction();
            let commit = tx
                .repo_mut()
                .new_commit(vec![parent], new_tree)
                .set_description(description.clone())
                .write()
                .await
                .map_err(|e| VcsError::Unavailable(format!("jj new_commit: {e}")))?;
            tx.repo_mut()
                .set_wc_commit(name.clone(), commit.id().clone())
                .map_err(|e| VcsError::Unavailable(format!("jj set_wc_commit: {e}")))?;
            let new_repo = tx
                .commit(description.clone())
                .await
                .map_err(|e| VcsError::Unavailable(format!("jj tx.commit: {e}")))?;

            // 4. Re-finish the wc state against the published op id.
            let op_hex = new_repo.op_id().hex();
            {
                let locked = workspace
                    .start_working_copy_mutation()
                    .await
                    .map_err(|e| VcsError::Unavailable(format!("jj wc relock: {e}")))?;
                locked
                    .finish(new_repo.op_id().clone())
                    .await
                    .map_err(|e| VcsError::Unavailable(format!("jj wc refinish: {e}")))?;
            }
            *repo = new_repo;
            op_hex
        };

        Ok(op_hex_to_change_id(&new_op_hex))
    }

    async fn changes(&self) -> Result<Vec<Change>, VcsError> {
        let st = self.state.lock().await;
        let head = st.repo.operation().clone();
        let ops: Vec<jj_lib::operation::Operation> = op_walk::walk_ancestors(&[head])
            .try_collect()
            .await
            .map_err(|e| VcsError::Unavailable(format!("jj op_walk: {e}")))?;
        Ok(ops
            .into_iter()
            .map(|op| {
                let desc = op.metadata().description.clone();
                Change {
                    id: op_hex_to_change_id(&op.id().hex()),
                    label: if desc.is_empty() { None } else { Some(desc) },
                    changed_paths: Vec::new(),
                }
            })
            .collect())
    }

    async fn diff(&self, _a: Option<ChangeId>, _b: Option<ChangeId>) -> Result<Diff, VcsError> {
        // Real tree-diff plumbing is P2 Task 3. Honest empty diff for now.
        Ok(Diff::default())
    }

    async fn undo(&mut self) -> Result<ChangeId, VcsError> {
        Err(VcsError::Unavailable("jj undo: P2 Task 3".into()))
    }

    async fn conflicts(&self) -> Result<Vec<Conflict>, VcsError> {
        Err(VcsError::Unavailable("jj conflicts: P2 Task 3".into()))
    }

    async fn resolve(&mut self, _path: &Path, _strategy: ResolveStrategy) -> Result<(), VcsError> {
        Err(VcsError::Unavailable("jj resolve: P2 Task 3".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::JjBackend;
    use crate::backend::VcsBackend;
    use std::path::PathBuf;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn open_snapshot_and_list_changes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), b"hi").unwrap();
        let mut be = JjBackend::open(dir.path())
            .await
            .expect("init colocated jj repo");
        let id = be
            .snapshot(Some("first"), vec![PathBuf::from("hello.txt")])
            .await
            .unwrap();
        let changes = be.changes().await.unwrap();
        assert!(
            changes.iter().any(|c| c.id == id),
            "snapshot must appear in the change/op log"
        );
    }

    /// Regression proof: the whole open + snapshot + changes flow runs inside a
    /// real multi-thread tokio runtime. Before the async conversion, `JjBackend`
    /// owned an internal `tokio::runtime::Runtime` and called `block_on`, which
    /// panicked with "Cannot start a runtime from within a runtime" here. This
    /// test passing is the evidence the nested-runtime panic is gone.
    #[tokio::test(flavor = "multi_thread")]
    async fn no_nested_runtime_panic_in_async_context() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"alpha").unwrap();
        let mut be = JjBackend::open(dir.path())
            .await
            .expect("init colocated jj repo inside tokio runtime");
        let id = be
            .snapshot(Some("regression"), vec![PathBuf::from("a.txt")])
            .await
            .expect("snapshot must not panic with nested-runtime error");
        let changes = be.changes().await.expect("changes must not panic");
        assert!(
            changes.iter().any(|c| c.id == id),
            "snapshot id must appear in the op log"
        );
    }
}
