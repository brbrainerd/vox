//! First-class conflict resolution — inspired by Jujutsu's `Merge<T>` model.
//!
//! Instead of hard-blocking when two agents touch the same file, we record
//! both sides as a conflict and let resolution happen later (or automatically).
//! Conflicts live as first-class objects, not file-level markers.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::snapshot::SnapshotId;
use crate::types::AgentId;

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// Unique conflict identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConflictId(pub u64);

impl fmt::Display for ConflictId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "C-{:06}", self.0)
    }
}

/// Thread-safe generator for [`ConflictId`]s.
#[derive(Debug)]
pub struct ConflictIdGenerator(AtomicU64);

impl ConflictIdGenerator {
    /// Create a new generator.
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    /// Produce the next unique [`ConflictId`].
    pub fn next(&self) -> ConflictId {
        ConflictId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for ConflictIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Conflict types
// ---------------------------------------------------------------------------

/// One side of a conflict — an agent's version of a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictSide {
    /// The agent that produced this version.
    pub agent_id: AgentId,
    /// Snapshot containing this version.
    pub snapshot_id: SnapshotId,
    /// When this side was recorded.
    pub timestamp_ms: u64,
}

/// How a conflict was (or should be) resolved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Accept the left (first) side entirely.
    TakeLeft,
    /// Accept the right (second) side entirely.
    TakeRight,
    /// A specific agent should resolve manually.
    DeferToAgent(AgentId),
    /// Manual resolution with the provided content hash.
    Manual { resolved_snapshot: SnapshotId },
}

/// A file conflict between two or more agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConflict {
    /// Unique conflict identifier.
    pub id: ConflictId,
    /// The contested file path.
    pub path: PathBuf,
    /// The base (common ancestor) snapshot ID.
    pub base_snapshot: Option<SnapshotId>,
    /// Each agent's version of the file.
    pub sides: Vec<ConflictSide>,
    /// Whether the conflict has been resolved.
    pub resolved: bool,
    /// Resolution strategy (if resolved).
    pub resolution: Option<ConflictResolution>,
    /// Unix timestamp when the conflict was first detected.
    pub created_ms: u64,
}

// ---------------------------------------------------------------------------
// ConflictManager
// ---------------------------------------------------------------------------

/// Tracks and manages file-level conflicts between agents.
#[derive(Debug)]
pub struct ConflictManager {
    id_gen: ConflictIdGenerator,
    /// Active and resolved conflicts keyed by conflict ID.
    conflicts: HashMap<ConflictId, FileConflict>,
    /// Index: path → active conflict IDs.
    path_index: HashMap<PathBuf, Vec<ConflictId>>,
}

impl ConflictManager {
    /// Create a new, empty manager.
    pub fn new() -> Self {
        Self {
            id_gen: ConflictIdGenerator::new(),
            conflicts: HashMap::new(),
            path_index: HashMap::new(),
        }
    }

    /// Record a new conflict on a file between two agent snapshots.
    pub fn record_conflict(
        &mut self,
        path: impl Into<PathBuf>,
        base_snapshot: Option<SnapshotId>,
        sides: Vec<(AgentId, SnapshotId)>,
    ) -> ConflictId {
        let id = self.id_gen.next();
        let path = path.into();
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let sides = sides
            .into_iter()
            .map(|(agent_id, snapshot_id)| ConflictSide {
                agent_id,
                snapshot_id,
                timestamp_ms,
            })
            .collect();

        let conflict = FileConflict {
            id,
            path: path.clone(),
            base_snapshot,
            sides,
            resolved: false,
            resolution: None,
            created_ms: timestamp_ms,
        };

        self.conflicts.insert(id, conflict);
        self.path_index.entry(path).or_default().push(id);

        id
    }

    /// Resolve a conflict with the given strategy.
    pub fn resolve(&mut self, conflict_id: ConflictId, resolution: ConflictResolution) -> bool {
        if let Some(conflict) = self.conflicts.get_mut(&conflict_id) {
            if conflict.resolved {
                return false; // already resolved
            }
            conflict.resolved = true;
            conflict.resolution = Some(resolution);
            true
        } else {
            false
        }
    }

    /// Get a conflict by ID.
    pub fn get(&self, id: ConflictId) -> Option<&FileConflict> {
        self.conflicts.get(&id)
    }

    /// List all active (unresolved) conflicts.
    pub fn active_conflicts(&self) -> Vec<&FileConflict> {
        self.conflicts.values().filter(|c| !c.resolved).collect()
    }

    /// List active conflicts for a specific file.
    pub fn conflicts_for_path(&self, path: &Path) -> Vec<&FileConflict> {
        self.path_index
            .get(path)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.conflicts.get(id))
                    .filter(|c| !c.resolved)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if a file has any active conflicts.
    pub fn has_conflict(&self, path: &Path) -> bool {
        !self.conflicts_for_path(path).is_empty()
    }

    /// Count of active (unresolved) conflicts.
    pub fn active_count(&self) -> usize {
        self.conflicts.values().filter(|c| !c.resolved).count()
    }

    /// Total conflicts (including resolved).
    pub fn total_count(&self) -> usize {
        self.conflicts.len()
    }
}

impl Default for ConflictManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_id_display() {
        assert_eq!(ConflictId(3).to_string(), "C-000003");
    }

    #[test]
    fn record_and_resolve_conflict() {
        let mut mgr = ConflictManager::new();

        let id = mgr.record_conflict(
            "src/lib.rs",
            Some(SnapshotId(1)),
            vec![(AgentId(1), SnapshotId(2)), (AgentId(2), SnapshotId(3))],
        );

        assert_eq!(mgr.active_count(), 1);
        assert!(mgr.has_conflict(Path::new("src/lib.rs")));

        let conflict = mgr.get(id).expect("conflict exists");
        assert_eq!(conflict.sides.len(), 2);
        assert!(!conflict.resolved);

        // Resolve
        assert!(mgr.resolve(id, ConflictResolution::TakeLeft));
        assert_eq!(mgr.active_count(), 0);
        assert!(!mgr.has_conflict(Path::new("src/lib.rs")));

        // Can't resolve twice
        assert!(!mgr.resolve(id, ConflictResolution::TakeRight));
    }

    #[test]
    fn multiple_conflicts_on_same_path() {
        let mut mgr = ConflictManager::new();
        mgr.record_conflict(
            "foo.rs",
            None,
            vec![(AgentId(1), SnapshotId(1)), (AgentId(2), SnapshotId(2))],
        );
        mgr.record_conflict(
            "foo.rs",
            None,
            vec![(AgentId(3), SnapshotId(3)), (AgentId(4), SnapshotId(4))],
        );

        assert_eq!(mgr.conflicts_for_path(Path::new("foo.rs")).len(), 2);
        assert_eq!(mgr.active_count(), 2);
    }

    #[test]
    fn defer_to_agent_resolution() {
        let mut mgr = ConflictManager::new();
        let id = mgr.record_conflict("bar.rs", None, vec![(AgentId(1), SnapshotId(1))]);

        mgr.resolve(id, ConflictResolution::DeferToAgent(AgentId(5)));
        let conflict = mgr.get(id).expect("exists");
        assert!(conflict.resolved);
        assert_eq!(
            conflict.resolution,
            Some(ConflictResolution::DeferToAgent(AgentId(5)))
        );
    }
}
