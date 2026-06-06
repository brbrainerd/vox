//! Pure conflict-recording core for workspace merge-back. Kept free of
//! `Orchestrator`/`WorkspaceManager` so it is unit-testable in isolation.

use crate::conflicts::{ConflictId, ConflictManager};
use crate::snapshot::SnapshotId;
use crate::types::AgentId;
use std::path::PathBuf;

/// Record one conflict per overlapping path between the merging agent and each
/// other active workspace. `others` is `(agent, its_base_snapshot, overlap_paths)`.
pub fn record_overlap_conflicts(
    cm: &mut ConflictManager,
    merging: (AgentId, SnapshotId),
    others: &[(AgentId, SnapshotId, Vec<PathBuf>)],
) -> Vec<ConflictId> {
    let (merging_agent, merging_snap) = merging;
    let mut ids = Vec::new();
    for (other_agent, other_snap, paths) in others {
        for path in paths {
            let id = cm.record_conflict(
                path.clone(),
                Some(merging_snap),
                vec![(merging_agent, merging_snap), (*other_agent, *other_snap)],
            );
            ids.push(id);
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::record_overlap_conflicts;
    use crate::conflicts::ConflictManager;
    use crate::snapshot::SnapshotId;
    use crate::types::AgentId;
    use std::path::PathBuf;

    #[test]
    fn one_conflict_per_overlapping_path() {
        let mut cm = ConflictManager::new();
        let others = vec![(
            AgentId(2),
            SnapshotId(20),
            vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")],
        )];
        let ids = record_overlap_conflicts(&mut cm, (AgentId(1), SnapshotId(10)), &others);
        assert_eq!(ids.len(), 2);
        assert_eq!(cm.active_conflicts().len(), 2);
    }

    #[test]
    fn no_overlap_records_nothing() {
        let mut cm = ConflictManager::new();
        let ids = record_overlap_conflicts(&mut cm, (AgentId(1), SnapshotId(10)), &[]);
        assert!(ids.is_empty());
        assert!(cm.active_conflicts().is_empty());
    }
}
