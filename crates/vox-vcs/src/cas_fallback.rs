//! Self-contained in-memory backend. Independent of `vox-orchestrator`'s
//! `SnapshotStore` to avoid a dependency cycle (vox-orchestrator -> vox-vcs).

use crate::backend::{VcsBackend, VcsError};
use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
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

impl VcsBackend for CasFallback {
    fn snapshot(&mut self, label: Option<&str>, paths: Vec<PathBuf>) -> Result<ChangeId, VcsError> {
        self.next_id += 1;
        let id = ChangeId(self.next_id);
        self.changes.push(Change {
            id,
            label: label.map(str::to_owned),
            changed_paths: paths,
        });
        Ok(id)
    }
    fn changes(&self) -> Result<Vec<Change>, VcsError> {
        Ok(self.changes.clone())
    }
    fn diff(&self, _a: Option<ChangeId>, _b: Option<ChangeId>) -> Result<Diff, VcsError> {
        // CasFallback has no content store, so it cannot compute a real two-change
        // diff; callers receive an empty diff (the jj backend computes real diffs).
        Ok(Diff::default())
    }
    // NOTE: `next_id` is intentionally not decremented on undo — change ids are a
    // monotone counter, so a snapshot after an undo gets a fresh id (gaps are by design).
    fn undo(&mut self) -> Result<ChangeId, VcsError> {
        self.changes
            .pop()
            .map(|c| c.id)
            .ok_or(VcsError::NothingToUndo)
    }
    fn conflicts(&self) -> Result<Vec<Conflict>, VcsError> {
        Ok(Vec::new())
    }
    fn resolve(&mut self, _path: &Path, _strategy: ResolveStrategy) -> Result<(), VcsError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::VcsBackend;
    use std::path::PathBuf;
    #[test]
    fn snapshot_then_changes_roundtrips() {
        let mut b = CasFallback::new();
        let id = b
            .snapshot(Some("first"), vec![PathBuf::from("a.rs")])
            .unwrap();
        assert_eq!(id.0, 1);
        let changes = b.changes().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].label.as_deref(), Some("first"));
    }
    #[test]
    fn undo_drops_the_last_change() {
        let mut b = CasFallback::new();
        b.snapshot(None, vec![]).unwrap();
        b.snapshot(None, vec![]).unwrap();
        b.undo().unwrap();
        assert_eq!(b.changes().unwrap().len(), 1);
    }

    #[test]
    fn undo_on_empty_is_nothing_to_undo() {
        let mut b = CasFallback::new();
        assert!(matches!(b.undo(), Err(VcsError::NothingToUndo)));
    }

    #[test]
    fn multi_path_snapshot_preserves_paths_and_label_in_order() {
        // Data in -> data out parity: the exact paths and label written by
        // snapshot() must come back verbatim (and in order) from changes().
        let mut b = CasFallback::new();
        let paths = vec![
            PathBuf::from("src/a.rs"),
            PathBuf::from("src/b.rs"),
            PathBuf::from("docs/c.md"),
        ];
        b.snapshot(Some("multi"), paths.clone()).unwrap();
        let changes = b.changes().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].label.as_deref(), Some("multi"));
        assert_eq!(changes[0].changed_paths, paths);
    }

    #[test]
    fn ids_are_monotone_with_gaps_after_undo() {
        // Documented contract: next_id is NOT decremented on undo, so a snapshot
        // taken after an undo gets a fresh id (gaps by design). This guards the
        // id-allocation invariant the jj backend must also honor.
        let mut b = CasFallback::new();
        assert_eq!(b.snapshot(None, vec![]).unwrap().0, 1);
        assert_eq!(b.snapshot(None, vec![]).unwrap().0, 2);
        assert_eq!(b.undo().unwrap().0, 2); // pops id 2
        // The next snapshot must NOT reuse id 2 — it advances to 3.
        assert_eq!(b.snapshot(None, vec![]).unwrap().0, 3);
        let ids: Vec<u64> = b.changes().unwrap().iter().map(|c| c.id.0).collect();
        assert_eq!(
            ids,
            vec![1, 3],
            "ids are monotone with a gap where 2 was undone"
        );
    }

    #[test]
    fn diff_conflicts_resolve_are_empty_in_cas_fallback() {
        // CasFallback has no content store: diff is empty, there are never any
        // conflicts, and resolve is a successful no-op. These are the documented
        // P0 contracts the real jj backend will override — pinning them prevents
        // silent drift (e.g. resolve() starting to error).
        let mut b = CasFallback::new();
        b.snapshot(Some("x"), vec![PathBuf::from("a.rs")]).unwrap();
        b.snapshot(Some("y"), vec![PathBuf::from("a.rs")]).unwrap();
        assert!(
            b.diff(Some(ChangeId(1)), Some(ChangeId(2)))
                .unwrap()
                .changed_paths
                .is_empty()
        );
        assert!(b.conflicts().unwrap().is_empty());
        assert!(
            b.resolve(Path::new("a.rs"), ResolveStrategy::TakeLeft)
                .is_ok()
        );
    }
}
