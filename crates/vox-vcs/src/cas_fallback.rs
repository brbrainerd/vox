//! Self-contained in-memory backend. Independent of `vox-orchestrator`'s
//! `SnapshotStore` to avoid a dependency cycle (vox-orchestrator -> vox-vcs).

use crate::backend::{VcsBackend, VcsError};
use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct CasFallback {
    changes: Vec<Change>,
    next_id: u64,
}

impl CasFallback {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            next_id: 0,
        }
    }
}

#[async_trait]
impl VcsBackend for CasFallback {
    async fn snapshot(
        &mut self,
        label: Option<&str>,
        paths: Vec<PathBuf>,
    ) -> Result<ChangeId, VcsError> {
        self.next_id += 1;
        let id = ChangeId(self.next_id);
        self.changes.push(Change {
            id,
            label: label.map(str::to_owned),
            changed_paths: paths,
        });
        Ok(id)
    }
    async fn changes(&self) -> Result<Vec<Change>, VcsError> {
        Ok(self.changes.clone())
    }
    async fn diff(&self, _a: Option<ChangeId>, _b: Option<ChangeId>) -> Result<Diff, VcsError> {
        // CasFallback has no content store, so it cannot compute a real two-change
        // diff; callers receive an empty diff (the jj backend computes real diffs).
        Ok(Diff::default())
    }
    // NOTE: `next_id` is intentionally not decremented on undo — change ids are a
    // monotone counter, so a snapshot after an undo gets a fresh id (gaps are by design).
    async fn undo(&mut self) -> Result<ChangeId, VcsError> {
        self.changes
            .pop()
            .map(|c| c.id)
            .ok_or(VcsError::NothingToUndo)
    }
    async fn conflicts(&self) -> Result<Vec<Conflict>, VcsError> {
        Ok(Vec::new())
    }
    async fn resolve(&mut self, _path: &Path, _strategy: ResolveStrategy) -> Result<(), VcsError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::VcsBackend;
    use std::path::PathBuf;
    #[tokio::test]
    async fn snapshot_then_changes_roundtrips() {
        let mut b = CasFallback::new();
        let id = b
            .snapshot(Some("first"), vec![PathBuf::from("a.rs")])
            .await
            .unwrap();
        assert_eq!(id.0, 1);
        let changes = b.changes().await.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].label.as_deref(), Some("first"));
    }
    #[tokio::test]
    async fn undo_drops_the_last_change() {
        let mut b = CasFallback::new();
        b.snapshot(None, vec![]).await.unwrap();
        b.snapshot(None, vec![]).await.unwrap();
        b.undo().await.unwrap();
        assert_eq!(b.changes().await.unwrap().len(), 1);
    }
}
