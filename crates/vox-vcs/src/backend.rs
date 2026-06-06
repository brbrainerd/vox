//! The `VcsBackend` trait and runtime backend selection.

use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum VcsError {
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("backend unavailable: {0}")]
    Unavailable(String),
}

/// Async VCS backend. Methods are `async fn` so backends can drive async engines
/// (e.g. jj-lib's async APIs) by awaiting directly — no internal `block_on`, so
/// they are safe to call from within the orchestrator's tokio runtime.
///
/// `async_trait` boxes the returned futures, which keeps the trait dyn-object-safe
/// (`Box<dyn VcsBackend>` / `Arc<RwLock<dyn VcsBackend>>`).
///
/// ## Why `?Send`
///
/// jj-lib 0.42's async futures are **`!Send`**: `Transaction`, `MutableRepo`
/// (interior `RefCell`/`OnceCell` via `DirtyCell<View>`), `dyn LockedWorkingCopy`,
/// `dyn OpHeadsStoreLock`, and `dyn MutableIndex` are not `Send`/`Sync`. A default
/// (Send) `async_trait` therefore fails to compile for [`JjBackend`]. We use
/// `?Send` futures. The backend stays object-safe and the type itself remains
/// `Send` (the workspace lives behind a `tokio::sync::Mutex`), but the per-call
/// futures must be polled on the thread that created them — see [`JjBackend`] and
/// the module docs for the actor-vs-`?Send` tradeoff.
#[async_trait(?Send)]
pub trait VcsBackend: Send {
    async fn snapshot(
        &mut self,
        label: Option<&str>,
        paths: Vec<PathBuf>,
    ) -> Result<ChangeId, VcsError>;
    async fn changes(&self) -> Result<Vec<Change>, VcsError>;
    async fn diff(&self, a: Option<ChangeId>, b: Option<ChangeId>) -> Result<Diff, VcsError>;
    async fn undo(&mut self) -> Result<ChangeId, VcsError>;
    async fn conflicts(&self) -> Result<Vec<Conflict>, VcsError>;
    async fn resolve(&mut self, path: &Path, strategy: ResolveStrategy) -> Result<(), VcsError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcsBackendKind {
    Jj,
    Cas,
}

/// Choose a backend for `repo_root`. The jj engine does not exist yet, so this
/// always returns [`VcsBackendKind::Cas`]; a later phase makes it prefer `Jj`.
pub fn detect(_repo_root: &Path) -> VcsBackendKind {
    VcsBackendKind::Cas
}
