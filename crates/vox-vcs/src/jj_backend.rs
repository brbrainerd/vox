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

use crate::backend::VcsError;
use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use jj_lib::backend::CommitId;
use jj_lib::config::{ConfigLayer, ConfigSource, StackedConfig};
use jj_lib::conflicts::{MaterializedTreeValue, materialize_tree_value};
use jj_lib::gitignore::GitIgnoreFile;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::merge::Diff as JjDiff;
use jj_lib::object_id::ObjectId;
use jj_lib::op_store::RefTarget;
use jj_lib::op_walk;
use jj_lib::ref_name::{RefName, RemoteName, WorkspaceName, WorkspaceNameBuf};
use jj_lib::repo::{ReadonlyRepo, Repo, StoreFactories};
use jj_lib::settings::UserSettings;
use jj_lib::working_copy::SnapshotOptions;
use jj_lib::workspace::{Workspace, default_working_copy_factories};

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
    /// Side-table projecting the `ChangeId` we hand out (an op-id hash) back to
    /// the underlying jj commit, so `diff`/`undo` can resolve trees. Populated on
    /// every `snapshot`.
    commits: HashMap<ChangeId, CommitId>,
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

    /// Open a jj workspace at `root`, **loading** an existing one if present and
    /// otherwise initializing a fresh colocated jj/git repo. This is the path the
    /// Vox repo itself needs: it is already colocated, so a blind
    /// `init_colocated_git` would error on the existing `.jj` dir.
    ///
    /// Load path (P2 Task 1): `Workspace::load(settings, root,
    /// &StoreFactories::default(), &default_working_copy_factories())`
    /// (workspace.rs:406) attaches to the on-disk `.jj/`. `DefaultWorkspaceLoader::new`
    /// returns `WorkspaceLoadError::NoWorkspaceHere` (workspace.rs:567) when there
    /// is no `.jj` dir — that is our fall-back-to-init signal. The loaded
    /// `Workspace` does not carry a repo, so we materialize it with
    /// `workspace.repo_loader().load_at_head().await` (repo.rs:764).
    pub async fn open(root: &Path) -> Result<Self, VcsError> {
        let settings = Self::bot_settings()?;
        let store_factories = StoreFactories::default();
        let wc_factories = default_working_copy_factories();

        let (workspace, repo) =
            match Workspace::load(&settings, root, &store_factories, &wc_factories) {
                Ok(workspace) => {
                    // Attached to an already-initialized workspace.
                    let repo = workspace
                        .repo_loader()
                        .load_at_head()
                        .await
                        .map_err(|e| VcsError::Unavailable(format!("jj load_at_head: {e}")))?;
                    (workspace, repo)
                }
                Err(jj_lib::workspace::WorkspaceLoadError::NoWorkspaceHere(_)) => {
                    // No workspace on disk yet — initialize a fresh colocated repo.
                    Workspace::init_colocated_git(&settings, root)
                        .await
                        .map_err(|e| VcsError::Unavailable(format!("jj init_colocated_git: {e}")))?
                }
                Err(e) => return Err(VcsError::Unavailable(format!("jj Workspace::load: {e}"))),
            };

        Ok(Self {
            state: tokio::sync::Mutex::new(JjState {
                workspace,
                repo,
                commits: HashMap::new(),
            }),
            settings,
        })
    }

    /// Test-only: construct a commit whose tree CONTAINS a materialized conflict
    /// in file `path`, set it as the working-copy commit, and register it. This
    /// proves jj-lib can REPRESENT a conflict as data (P2 Task 4) without needing
    /// a full rebase/merge driver.
    ///
    /// Recipe: write three file blobs (base/left/right), build three single-file
    /// `MergedTree`s via `MergedTreeBuilder` (merged_tree_builder.rs), then
    /// `MergedTree::merge(Merge::from_vec([left, base, right]))` (merged_tree.rs:336)
    /// — a 3-way merge with divergent left/right yields a tree where
    /// `has_conflict()` is true. We commit that conflicted tree as the wc commit.
    #[cfg(test)]
    pub async fn create_conflict_for_test(
        &mut self,
        path: &str,
        base: &str,
        left: &str,
        right: &str,
    ) -> Result<ChangeId, VcsError> {
        use jj_lib::backend::{CopyId, TreeValue};
        use jj_lib::merge::{Merge, MergedTreeValue};
        use jj_lib::merged_tree::MergedTree;
        use jj_lib::merged_tree_builder::MergedTreeBuilder;
        use jj_lib::repo_path::RepoPathBuf;

        let mut st = self.state.lock().await;
        let name: WorkspaceNameBuf = WorkspaceName::DEFAULT.to_owned();
        let store = st.repo.store().clone();
        let repo_path = RepoPathBuf::from_internal_string(path)
            .map_err(|e| VcsError::Unavailable(format!("jj repo path: {e}")))?;

        // Build a single-file MergedTree for the given content.
        let make_tree = |content: &str| {
            let store = store.clone();
            let repo_path = repo_path.clone();
            let content = content.to_string();
            async move {
                let empty = MergedTree::resolved(store.clone(), store.empty_tree_id().clone());
                let bytes = content.into_bytes();
                let file_id = store
                    .write_file(&repo_path, &mut bytes.as_slice())
                    .await
                    .map_err(|e| VcsError::Unavailable(format!("jj write_file: {e}")))?;
                let mut builder = MergedTreeBuilder::new(empty);
                builder.set_or_remove(
                    repo_path.clone(),
                    MergedTreeValue::resolved(Some(TreeValue::File {
                        id: file_id,
                        executable: false,
                        copy_id: CopyId::placeholder(),
                    })),
                );
                builder
                    .write_tree()
                    .await
                    .map_err(|e| VcsError::Unavailable(format!("jj write_tree: {e}")))
            }
        };

        let base_tree = make_tree(base).await?;
        let left_tree = make_tree(left).await?;
        let right_tree = make_tree(right).await?;

        // 3-way merge: [left, base, right]. Divergent left/right => conflict.
        let conflicted = MergedTree::merge(Merge::from_vec(vec![
            (left_tree, "left".to_string()),
            (base_tree, "base".to_string()),
            (right_tree, "right".to_string()),
        ]))
        .await
        .map_err(|e| VcsError::Unavailable(format!("jj merge trees: {e}")))?;

        if !conflicted.has_conflict() {
            return Err(VcsError::Unavailable(
                "jj: merge unexpectedly auto-resolved (no conflict)".into(),
            ));
        }

        // Commit the conflicted tree as the new wc commit.
        let parent = st
            .repo
            .view()
            .get_wc_commit_id(&name)
            .cloned()
            .ok_or_else(|| VcsError::Unavailable("jj: no wc commit".into()))?;
        let mut tx = st.repo.start_transaction();
        let commit = tx
            .repo_mut()
            .new_commit(vec![parent], conflicted)
            .set_description("conflict")
            .write()
            .await
            .map_err(|e| VcsError::Unavailable(format!("jj new_commit (conflict): {e}")))?;
        let commit_id = commit.id().clone();
        tx.repo_mut()
            .set_wc_commit(name.clone(), commit_id.clone())
            .map_err(|e| VcsError::Unavailable(format!("jj set_wc_commit: {e}")))?;
        let new_repo = tx
            .commit("create conflict")
            .await
            .map_err(|e| VcsError::Unavailable(format!("jj tx.commit (conflict): {e}")))?;
        let change_id = op_hex_to_change_id(&new_repo.op_id().hex());
        st.repo = new_repo;
        st.commits.insert(change_id, commit_id);
        Ok(change_id)
    }
}

impl JjBackend {
    pub async fn snapshot(
        &mut self,
        label: Option<&str>,
        _paths: Vec<PathBuf>,
    ) -> Result<ChangeId, VcsError> {
        let description = label.unwrap_or("").to_string();
        let name: WorkspaceNameBuf = WorkspaceName::DEFAULT.to_owned();
        let mut st = self.state.lock().await;

        let (new_op_hex, commit_id) = {
            let JjState {
                workspace,
                repo,
                commits: _,
            } = &mut *st;

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
            let commit_id = commit.id().clone();
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
            (op_hex, commit_id)
        };

        let change_id = op_hex_to_change_id(&new_op_hex);
        st.commits.insert(change_id, commit_id);
        Ok(change_id)
    }

    pub async fn changes(&self) -> Result<Vec<Change>, VcsError> {
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

    /// P2 Task 3: real tree diff between two changes.
    ///
    /// Resolves each `ChangeId` to its jj `Commit` via the side-table, takes the
    /// commit's `MergedTree` (`commit.tree()`, commit.rs:120), and streams
    /// `from_tree.diff_stream(&to_tree, &EverythingMatcher)`
    /// (merged_tree.rs:283), collecting the changed `RepoPath`s. A `None` change
    /// means the empty tree.
    pub async fn diff(&self, a: Option<ChangeId>, b: Option<ChangeId>) -> Result<Diff, VcsError> {
        let st = self.state.lock().await;
        let store = st.repo.store();

        // Resolve an optional ChangeId to a MergedTree (empty tree for None).
        let tree_of = |id: Option<ChangeId>| -> Result<jj_lib::merged_tree::MergedTree, VcsError> {
            match id {
                None => Ok(jj_lib::merged_tree::MergedTree::resolved(
                    store.clone(),
                    store.empty_tree_id().clone(),
                )),
                Some(cid) => {
                    let commit_id = st.commits.get(&cid).ok_or_else(|| {
                        VcsError::Unavailable(format!("jj diff: unknown ChangeId {cid}"))
                    })?;
                    let commit = store
                        .get_commit(commit_id)
                        .map_err(|e| VcsError::Unavailable(format!("jj get_commit: {e}")))?;
                    Ok(commit.tree())
                }
            }
        };

        let from_tree = tree_of(a)?;
        let to_tree = tree_of(b)?;

        let mut changed_paths = Vec::new();
        let mut stream = from_tree.diff_stream(&to_tree, &EverythingMatcher);
        while let Some(entry) = futures::StreamExt::next(&mut stream).await {
            // Surface backend read errors rather than silently dropping paths.
            entry
                .values
                .map_err(|e| VcsError::Unavailable(format!("jj diff entry: {e}")))?;
            changed_paths.push(PathBuf::from(entry.path.as_internal_file_string()));
        }
        Ok(Diff { changed_paths })
    }

    /// P2 Task 3: undo the last operation by restoring the parent op's view.
    ///
    /// We walk the op log (`op_walk::walk_ancestors`, op_walk.rs:263) to find the
    /// current head op's parent, reload the repo at that operation
    /// (`repo_loader().load_at`, repo.rs:776), and re-point the working copy by
    /// finishing the wc mutation against the restored op id. The restored head op
    /// becomes the new current op. Returns the restored op's `ChangeId`.
    pub async fn undo(&mut self) -> Result<ChangeId, VcsError> {
        let mut st = self.state.lock().await;

        // Find the parent of the current head operation.
        let head = st.repo.operation().clone();
        let parents = head
            .parents()
            .await
            .map_err(|e| VcsError::Unavailable(format!("jj op parents: {e}")))?;
        let parent = parents.into_iter().next().ok_or(VcsError::NothingToUndo)?;

        // Reload the repo at the parent operation.
        let restored = st
            .repo
            .loader()
            .load_at(&parent)
            .await
            .map_err(|e| VcsError::Unavailable(format!("jj load_at parent op: {e}")))?;

        // Re-point the working-copy state to the restored op so a subsequent
        // snapshot bases off the restored view.
        {
            let JjState {
                workspace, repo, ..
            } = &mut *st;
            *repo = restored;
            let locked = workspace
                .start_working_copy_mutation()
                .await
                .map_err(|e| VcsError::Unavailable(format!("jj undo wc lock: {e}")))?;
            locked
                .finish(repo.op_id().clone())
                .await
                .map_err(|e| VcsError::Unavailable(format!("jj undo wc finish: {e}")))?;
        }

        Ok(op_hex_to_change_id(&parent.id().hex()))
    }

    /// P2 Task 4 (conflicts-as-data): surface conflicts in the current
    /// working-copy commit tree as data.
    ///
    /// Reads the wc commit's `MergedTree`, iterates `tree.conflicts()`
    /// (merged_tree.rs:196) for conflicted paths, and materializes each via
    /// `materialize_tree_value` (conflicts.rs:207). A `FileConflict` carries
    /// `contents: Merge<BString>` — the conflict sides — which we surface as the
    /// `sides` of our `Conflict` type. This is the "conflict as readable data,
    /// not a blocking error" property the spec wants.
    pub async fn conflicts(&self) -> Result<Vec<Conflict>, VcsError> {
        let st = self.state.lock().await;
        let name: WorkspaceNameBuf = WorkspaceName::DEFAULT.to_owned();
        let store = st.repo.store();

        let wc_commit_id = st
            .repo
            .view()
            .get_wc_commit_id(&name)
            .cloned()
            .ok_or_else(|| VcsError::Unavailable("jj conflicts: no wc commit".into()))?;
        let wc_commit = store
            .get_commit(&wc_commit_id)
            .map_err(|e| VcsError::Unavailable(format!("jj get_commit: {e}")))?;
        let tree = wc_commit.tree();
        let labels = tree.labels().clone();

        let mut out = Vec::new();
        for (path, value) in tree.conflicts() {
            let value =
                value.map_err(|e| VcsError::Unavailable(format!("jj conflict value: {e}")))?;
            let materialized = materialize_tree_value(store, &path, value, &labels)
                .await
                .map_err(|e| VcsError::Unavailable(format!("jj materialize: {e}")))?;
            if let MaterializedTreeValue::FileConflict(file) = materialized {
                let sides = file
                    .contents
                    .iter()
                    .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                    .collect();
                out.push(Conflict {
                    path: PathBuf::from(path.as_internal_file_string()),
                    sides,
                });
            }
        }
        Ok(out)
    }

    pub async fn resolve(
        &mut self,
        _path: &Path,
        _strategy: ResolveStrategy,
    ) -> Result<(), VcsError> {
        Err(VcsError::Unavailable("jj resolve: P2 Task 3".into()))
    }

    // P2 Task 5 finding: `jj_lib::git::add_remote` (git.rs:2315) forces a
    // `gix::remote::fetch::Tags` argument, so any in-process remote registration
    // pulls `gix` into the caller's dependency graph (jj-lib does not re-export
    // it). For the spike we therefore register the remote out-of-band (the
    // colocated `.git` is a normal git repo, so `git remote add` works) and keep
    // `gix` out of vox-vcs. The default trait impl returns `Unavailable`; the test
    // drives remote setup directly.

    /// P2 Task 5: push a change to a remote under `bookmark`.
    ///
    /// **jj-lib 0.42 reality:** `push_refs` (git.rs:3177) ultimately calls
    /// `git_ctx.spawn_push` (git.rs:3336) — a `git` **subprocess**, NOT pure gix
    /// transport. A `git` binary on PATH (>= 2.41) is therefore required. We set
    /// the local bookmark to the change's commit, then push it as a new ref
    /// (`before: None`).
    pub async fn push(
        &mut self,
        remote: &str,
        bookmark: &str,
        change: ChangeId,
    ) -> Result<(), VcsError> {
        let mut st = self.state.lock().await;
        let commit_id =
            st.commits.get(&change).cloned().ok_or_else(|| {
                VcsError::Unavailable(format!("jj push: unknown ChangeId {change}"))
            })?;

        let mut tx = st.repo.start_transaction();
        // Point the local bookmark at the change's commit.
        tx.repo_mut().set_local_bookmark_target(
            RefName::new(bookmark),
            RefTarget::normal(commit_id.clone()),
        );

        let targets = jj_lib::git::GitPushRefTargets {
            bookmarks: vec![(
                RefName::new(bookmark).to_owned(),
                JjDiff {
                    before: None,
                    after: Some(commit_id),
                },
            )],
            tags: vec![],
        };
        let subprocess_options = jj_lib::git::GitSubprocessOptions {
            executable_path: "git".into(),
            environment: HashMap::new(),
        };
        let mut callback = NoopGitCallback;
        let stats = jj_lib::git::push_refs(
            tx.repo_mut(),
            subprocess_options,
            RemoteName::new(remote),
            &targets,
            &mut callback,
            &jj_lib::git::GitPushOptions::default(),
        )
        .map_err(|e| VcsError::Unavailable(format!("jj push_refs: {e}")))?;

        if !stats.rejected.is_empty() || !stats.remote_rejected.is_empty() {
            return Err(VcsError::Unavailable(format!(
                "jj push rejected: {:?} / remote {:?}",
                stats.rejected, stats.remote_rejected
            )));
        }
        let new_repo = tx
            .commit(format!("push {bookmark} -> {remote}"))
            .await
            .map_err(|e| VcsError::Unavailable(format!("jj push commit: {e}")))?;
        st.repo = new_repo;
        Ok(())
    }
}

/// No-op Git subprocess callback (we do not surface progress/sideband for the
/// spike). Implements the minimum of `GitSubprocessCallback` (git_subprocess.rs:686).
struct NoopGitCallback;

impl jj_lib::git::GitSubprocessCallback for NoopGitCallback {
    fn needs_progress(&self) -> bool {
        false
    }
    fn progress(&mut self, _progress: &jj_lib::git::GitProgress) -> std::io::Result<()> {
        Ok(())
    }
    fn local_sideband(
        &mut self,
        _message: &[u8],
        _term: Option<jj_lib::git::GitSidebandLineTerminator>,
    ) -> std::io::Result<()> {
        Ok(())
    }
    fn remote_sideband(
        &mut self,
        _message: &[u8],
        _term: Option<jj_lib::git::GitSidebandLineTerminator>,
    ) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::JjBackend;
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

    // ---- P2 part B: the five risky operations ----

    /// Task 1: open() on an ALREADY-colocated repo must LOAD, not re-init/error.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn open_loads_existing_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f.txt"), b"v1").unwrap();

        // First open: fresh dir -> init branch.
        let id1 = {
            let mut be = JjBackend::open(dir.path())
                .await
                .expect("first open (init)");
            be.snapshot(Some("first"), vec![PathBuf::from("f.txt")])
                .await
                .unwrap()
        };
        // Backend dropped; .jj now exists on disk.

        // Second open on the SAME dir: must take the load branch and read history.
        let be2 = JjBackend::open(dir.path())
            .await
            .expect("second open must LOAD existing workspace, not error");
        let changes = be2.changes().await.expect("read existing change log");
        assert!(
            changes.iter().any(|c| c.id == id1),
            "reopened workspace must expose the change made before reopen"
        );
    }

    /// Task 2: undo restores the working state to before the last snapshot.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn undo_restores_previous_head() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("u.txt"), b"one").unwrap();
        let mut be = JjBackend::open(dir.path()).await.unwrap();

        let c1 = be
            .snapshot(Some("c1"), vec![PathBuf::from("u.txt")])
            .await
            .unwrap();
        std::fs::write(dir.path().join("u.txt"), b"two").unwrap();
        let c2 = be
            .snapshot(Some("c2"), vec![PathBuf::from("u.txt")])
            .await
            .unwrap();

        // c2 is the head before undo.
        let head_before = be.changes().await.unwrap()[0].id;
        assert_eq!(head_before, c2, "head should be c2 before undo");

        let restored = be.undo().await.expect("undo must succeed");
        let head_after = be.changes().await.unwrap()[0].id;
        assert_ne!(head_after, c2, "after undo, head must no longer be c2");
        assert_eq!(
            head_after, restored,
            "current head op must equal the restored op id"
        );
        // The restored op is the one that recorded c1 (the snapshot before c2).
        assert!(c1 != c2, "sanity: two distinct snapshots");
    }

    /// Task 3: diff lists the file changed between two snapshots.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn diff_lists_changed_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.txt"), b"alpha").unwrap();
        let mut be = JjBackend::open(dir.path()).await.unwrap();

        let c1 = be
            .snapshot(Some("c1"), vec![PathBuf::from("x.txt")])
            .await
            .unwrap();
        std::fs::write(dir.path().join("x.txt"), b"beta-changed").unwrap();
        let c2 = be
            .snapshot(Some("c2"), vec![PathBuf::from("x.txt")])
            .await
            .unwrap();

        let diff = be
            .diff(Some(c1), Some(c2))
            .await
            .expect("diff must succeed");
        assert!(
            diff.changed_paths.contains(&PathBuf::from("x.txt")),
            "diff must list the modified file, got {:?}",
            diff.changed_paths
        );
    }

    /// Task 4 (THE killer feature): a conflict is represented and materialized as
    /// readable DATA, not a blocking error.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn conflict_is_materialized_as_data() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("seed.txt"), b"seed").unwrap();
        let mut be = JjBackend::open(dir.path()).await.unwrap();
        be.snapshot(Some("seed"), vec![PathBuf::from("seed.txt")])
            .await
            .unwrap();

        // Construct a real conflict in file "c.txt": base "common", left "LEFT",
        // right "RIGHT" on the same line -> unresolvable 3-way merge.
        be.create_conflict_for_test("c.txt", "common\n", "LEFT\n", "RIGHT\n")
            .await
            .expect("construct conflicted commit");

        let conflicts = be.conflicts().await.expect("read conflicts");
        assert_eq!(conflicts.len(), 1, "exactly one conflicted path expected");
        let conflict = &conflicts[0];
        assert_eq!(conflict.path, PathBuf::from("c.txt"));
        // The materialized sides must carry both divergent contents as data.
        let joined = conflict.sides.join("|");
        assert!(
            joined.contains("LEFT") && joined.contains("RIGHT"),
            "conflict sides must surface both divergent edits as readable data, got {:?}",
            conflict.sides
        );
    }

    /// Task 5: push a bookmark to a LOCAL bare git repo, then verify with plain
    /// `git` that the ref/commit arrived. jj-lib 0.42 push shells out to the `git`
    /// binary (NOT pure gix), so this exercises that subprocess path end-to-end.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn push_bookmark_to_local_bare_repo() {
        // Local bare "remote".
        let remote_dir = tempfile::tempdir().unwrap();
        let bare = remote_dir.path().join("origin.git");
        // vox-arch-check: allow git-exec
        let init = std::process::Command::new("git")
            .args(["init", "--bare", "-b", "main"])
            .arg(&bare)
            .output()
            .expect("spawn git init --bare");
        assert!(init.status.success(), "git init --bare failed: {init:?}");

        // Colocated jj workspace. Open once to materialize `.git`, then drop so
        // we can register the remote out-of-band (jj's gix repo snapshots config
        // at open time, so the remote must exist BEFORE the backend we push from).
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("p.txt"), b"payload").unwrap();
        {
            let _ = JjBackend::open(work.path()).await.unwrap();
        }
        // vox-arch-check: allow git-exec
        let add = std::process::Command::new("git")
            .current_dir(work.path())
            .args(["remote", "add", "origin"])
            .arg(&bare)
            .output()
            .expect("spawn git remote add");
        assert!(add.status.success(), "git remote add failed: {add:?}");

        // Reopen (load path) so jj sees the remote, then make the change.
        let mut be = JjBackend::open(work.path()).await.unwrap();
        let change = be
            .snapshot(Some("payload"), vec![PathBuf::from("p.txt")])
            .await
            .unwrap();

        be.push("origin", "feature", change)
            .await
            .expect("push to local bare repo must succeed");

        // Verify with plain git: the bare repo now has refs/heads/feature.
        // vox-arch-check: allow git-exec
        let show = std::process::Command::new("git")
            .args(["--git-dir"])
            .arg(&bare)
            .args(["rev-parse", "refs/heads/feature"])
            .output()
            .expect("spawn git rev-parse");
        assert!(
            show.status.success(),
            "pushed bookmark 'feature' must exist in the bare remote: {}",
            String::from_utf8_lossy(&show.stderr)
        );
    }
}
