//! The `VcsBackend` trait and runtime backend selection.

use crate::cas_fallback::CasFallback;
use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

// `jj_actor` is only compiled under the "jj" feature; this suppresses the
// unused-import lint on the cfg-gated use below.
#[cfg(feature = "jj")]
use crate::jj_actor::JjActor;

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
/// ## Send-safe design
///
/// jj-lib 0.42's async futures are **`!Send`**. `VcsBackend` is `#[async_trait]`
/// (Send futures) so handles can cross `tokio::spawn`. The `JjBackend` engine
/// (which is `!Send`) lives on a dedicated OS thread behind `JjActorHandle`,
/// which satisfies this `Send` contract.
#[async_trait]
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

    /// Register a remote named `name` pointing at `url`. Backends without a
    /// remote concept (CAS) return [`VcsError::Unavailable`].
    async fn add_remote(&mut self, _name: &str, _url: &str) -> Result<(), VcsError> {
        Err(VcsError::Unavailable("backend has no remotes".into()))
    }

    /// Create (or move) a named branch/bookmark at the current change. Backends
    /// without a branch concept (CAS) return [`VcsError::Unavailable`].
    async fn create_branch(&mut self, _name: &str) -> Result<(), VcsError> {
        Err(VcsError::Unavailable("backend has no branches".into()))
    }

    /// Push `change` to `remote` under bookmark/branch `bookmark`.
    async fn push(
        &mut self,
        _remote: &str,
        _bookmark: &str,
        _change: ChangeId,
    ) -> Result<(), VcsError> {
        Err(VcsError::Unavailable("backend cannot push".into()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcsBackendKind {
    Jj,
    Cas,
}

/// Detect which VCS kind is present at `repo_root`.
///
/// Returns `Jj` when a `.jj` directory exists **and** the `jj` feature is
/// enabled; otherwise returns `Cas`.
pub fn detect(repo_root: &Path) -> VcsBackendKind {
    #[cfg(feature = "jj")]
    if repo_root.join(".jj").exists() {
        return VcsBackendKind::Jj;
    }
    #[cfg(not(feature = "jj"))]
    let _ = repo_root; // suppress unused-variable lint
    VcsBackendKind::Cas
}

/// Construct a boxed [`VcsBackend`] for `root`.
///
/// With the `jj` feature (default): if the directory contains a `.jj`
/// workspace, a [`crate::jj_actor::JjActorHandle`] is spawned (the actor owns
/// the `!Send` jj engine on a dedicated OS thread). If spawning fails, or
/// there is no jj workspace, [`CasFallback`] is returned.
///
/// Without the `jj` feature: always returns [`CasFallback`].
pub async fn boxed_for(root: &Path) -> Box<dyn VcsBackend> {
    #[cfg(feature = "jj")]
    if detect(root) == VcsBackendKind::Jj
        && let Ok(handle) = JjActor::spawn(root.to_path_buf())
    {
        return Box::new(handle);
    }
    #[cfg(not(feature = "jj"))]
    let _ = root; // suppress unused-variable lint
    Box::new(CasFallback::new())
}

#[cfg(test)]
mod semcov_wave4_tests {
    #![allow(unused_imports)]
    use super::*;
    use std::path::PathBuf;

    /// boxed_for() on a plain directory (no .jj) must return a working CasFallback.
    #[tokio::test]
    async fn boxed_for_no_jj_dir_returns_cas_fallback() {
        let dir = tempfile::tempdir().unwrap();
        // No .jj directory => must fall back to CasFallback regardless of feature flags.
        let mut backend = crate::backend::boxed_for(dir.path()).await;
        // The returned backend must be functional: snapshot + changes round-trip.
        let id = backend
            .snapshot(Some("probe"), vec![PathBuf::from("probe.rs")])
            .await
            .expect("CasFallback snapshot must succeed");
        let changes = backend.changes().await.expect("changes must succeed");
        assert!(
            changes.iter().any(|c| c.id == id),
            "snapshot id must appear in change list, got {:?}",
            changes
        );
    }

    /// boxed_for() fallback: undo on the returned backend must work (no NothingToUndo
    /// after a snapshot).
    #[tokio::test]
    async fn boxed_for_fallback_undo_removes_last_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut backend = crate::backend::boxed_for(dir.path()).await;
        backend.snapshot(Some("x"), vec![]).await.expect("snapshot");
        backend
            .undo()
            .await
            .expect("undo after snapshot must succeed");
        let changes = backend.changes().await.expect("changes");
        assert!(changes.is_empty(), "all changes must be gone after undo");
    }
}
