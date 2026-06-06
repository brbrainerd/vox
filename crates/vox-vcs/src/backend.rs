//! The `VcsBackend` trait and runtime backend selection.

use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum VcsError {
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("backend unavailable: {0}")]
    Unavailable(String),
}

pub trait VcsBackend: Send + Sync {
    fn snapshot(&mut self, label: Option<&str>, paths: Vec<PathBuf>) -> Result<ChangeId, VcsError>;
    fn changes(&self) -> Result<Vec<Change>, VcsError>;
    fn diff(&self, a: Option<ChangeId>, b: Option<ChangeId>) -> Result<Diff, VcsError>;
    fn undo(&mut self) -> Result<ChangeId, VcsError>;
    fn conflicts(&self) -> Result<Vec<Conflict>, VcsError>;
    fn resolve(&mut self, path: &Path, strategy: ResolveStrategy) -> Result<(), VcsError>;
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
