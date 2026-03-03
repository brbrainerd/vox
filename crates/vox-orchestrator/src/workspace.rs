//! Agent workspaces — inspired by Jujutsu's multi-workspace model.
//!
//! Each agent gets a lightweight virtual workspace (a diff-overlay on top of
//! the shared base) so multiple agents can edit the same codebase in parallel
//! without stepping on each other.  Changes are merged back atomically.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::snapshot::SnapshotId;
use crate::types::AgentId;

// ---------------------------------------------------------------------------
// Workspace entry
// ---------------------------------------------------------------------------

/// A single file entry in the workspace overlay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceEntry {
    /// File was modified — stores the content hash of the new version.
    Modified { content_hash: String },
    /// File was deleted in this workspace.
    Deleted,
    /// File was created in this workspace.
    Created { content_hash: String },
}

// ---------------------------------------------------------------------------
// Change tracking (Feature 5)
// ---------------------------------------------------------------------------

/// Stable identifier for a logical unit of work that survives rebases
/// and amendments — inspired by Jujutsu's Change IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangeId(pub u64);

impl fmt::Display for ChangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CH-{:06}", self.0)
    }
}

/// Status of a logical change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeStatus {
    /// Work is in progress.
    InProgress,
    /// Ready for review / merge.
    Ready,
    /// Merged into main.
    Merged,
    /// Abandoned.
    Abandoned,
}

/// A logical change — groups related snapshots across edits and agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    /// Stable identifier.
    pub id: ChangeId,
    /// Human-readable description.
    pub description: String,
    /// Agent that currently owns this change.
    pub agent_id: AgentId,
    /// Ordered list of snapshots comprising this change.
    pub snapshots: Vec<SnapshotId>,
    /// When the change was created (unix ms).
    pub created_ms: u64,
    /// Current status.
    pub status: ChangeStatus,
}

// ---------------------------------------------------------------------------
// Agent workspace
// ---------------------------------------------------------------------------

/// A per-agent virtual workspace overlaying the shared repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWorkspace {
    /// Agent this workspace belongs to.
    pub agent_id: AgentId,
    /// The base snapshot this workspace was forked from.
    pub base_snapshot: SnapshotId,
    /// Overlay of file changes on top of the base.
    pub overlay: HashMap<PathBuf, WorkspaceEntry>,
    /// When the workspace was created (unix ms).
    pub created_ms: u64,
    /// Active change being tracked in this workspace.
    pub active_change: Option<ChangeId>,
}

impl AgentWorkspace {
    /// Number of files modified in the overlay.
    pub fn modified_count(&self) -> usize {
        self.overlay.len()
    }

    /// Check if a file has been modified in this workspace.
    pub fn has_modification(&self, path: &Path) -> bool {
        self.overlay.contains_key(path)
    }

    /// Record a file modification in the workspace overlay.
    pub fn record_modification(&mut self, path: impl Into<PathBuf>, content_hash: String) {
        self.overlay
            .insert(path.into(), WorkspaceEntry::Modified { content_hash });
    }

    /// Record a file creation.
    pub fn record_creation(&mut self, path: impl Into<PathBuf>, content_hash: String) {
        self.overlay
            .insert(path.into(), WorkspaceEntry::Created { content_hash });
    }

    /// Record a file deletion.
    pub fn record_deletion(&mut self, path: impl Into<PathBuf>) {
        self.overlay.insert(path.into(), WorkspaceEntry::Deleted);
    }

    /// List all paths modified in this workspace.
    pub fn modified_paths(&self) -> Vec<&PathBuf> {
        self.overlay.keys().collect()
    }
}

// ---------------------------------------------------------------------------
// WorkspaceManager
// ---------------------------------------------------------------------------

/// Manages per-agent workspaces and change tracking.
#[derive(Debug)]
pub struct WorkspaceManager {
    /// Active workspaces keyed by agent ID.
    workspaces: HashMap<AgentId, AgentWorkspace>,
    /// All tracked changes keyed by change ID.
    changes: HashMap<ChangeId, Change>,
    /// Change ID counter.
    next_change_id: u64,
}

impl WorkspaceManager {
    /// Create a new workspace manager.
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            changes: HashMap::new(),
            next_change_id: 1,
        }
    }

    /// Create a new workspace for an agent, forked from the given base snapshot.
    pub fn create_workspace(
        &mut self,
        agent_id: AgentId,
        base_snapshot: SnapshotId,
    ) -> &AgentWorkspace {
        let ws = AgentWorkspace {
            agent_id,
            base_snapshot,
            overlay: HashMap::new(),
            created_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            active_change: None,
        };
        self.workspaces.insert(agent_id, ws);
        self.workspaces.get(&agent_id).expect("just inserted")
    }

    /// Get an agent's workspace.
    pub fn get_workspace(&self, agent_id: AgentId) -> Option<&AgentWorkspace> {
        self.workspaces.get(&agent_id)
    }

    /// Get a mutable reference to an agent's workspace.
    pub fn get_workspace_mut(&mut self, agent_id: AgentId) -> Option<&mut AgentWorkspace> {
        self.workspaces.get_mut(&agent_id)
    }

    /// Destroy an agent's workspace (e.g. after successful merge).
    pub fn destroy_workspace(&mut self, agent_id: AgentId) -> Option<AgentWorkspace> {
        self.workspaces.remove(&agent_id)
    }

    /// Check if an agent has an active workspace.
    pub fn has_workspace(&self, agent_id: AgentId) -> bool {
        self.workspaces.contains_key(&agent_id)
    }

    /// List all active workspaces.
    pub fn list_workspaces(&self) -> Vec<&AgentWorkspace> {
        self.workspaces.values().collect()
    }

    /// Get conflicting paths between two agent workspaces.
    pub fn overlapping_paths(&self, agent_a: AgentId, agent_b: AgentId) -> Vec<PathBuf> {
        match (self.workspaces.get(&agent_a), self.workspaces.get(&agent_b)) {
            (Some(ws_a), Some(ws_b)) => ws_a
                .overlay
                .keys()
                .filter(|p| ws_b.overlay.contains_key(*p))
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Change tracking (Feature 5)
    // -----------------------------------------------------------------------

    /// Create a new logical change.
    pub fn create_change(&mut self, agent_id: AgentId, description: impl Into<String>) -> ChangeId {
        let id = ChangeId(self.next_change_id);
        self.next_change_id += 1;

        let change = Change {
            id,
            description: description.into(),
            agent_id,
            snapshots: Vec::new(),
            created_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            status: ChangeStatus::InProgress,
        };

        self.changes.insert(id, change);

        // Link to active workspace if one exists.
        if let Some(ws) = self.workspaces.get_mut(&agent_id) {
            ws.active_change = Some(id);
        }

        id
    }

    /// Add a snapshot to a change.
    pub fn add_snapshot_to_change(&mut self, change_id: ChangeId, snapshot_id: SnapshotId) {
        if let Some(change) = self.changes.get_mut(&change_id) {
            change.snapshots.push(snapshot_id);
        }
    }

    /// Update a change's status.
    pub fn update_change_status(&mut self, change_id: ChangeId, status: ChangeStatus) {
        if let Some(change) = self.changes.get_mut(&change_id) {
            change.status = status;
        }
    }

    /// Get a change by ID.
    pub fn get_change(&self, change_id: ChangeId) -> Option<&Change> {
        self.changes.get(&change_id)
    }

    /// List changes, optionally filtered by agent.
    pub fn list_changes(&self, agent_id: Option<AgentId>, limit: usize) -> Vec<&Change> {
        let mut changes: Vec<_> = self
            .changes
            .values()
            .filter(|c| agent_id.is_none_or(|a| c.agent_id == a))
            .collect();
        changes.sort_by(|a, b| b.created_ms.cmp(&a.created_ms));
        changes.truncate(limit);
        changes
    }
}

impl Default for WorkspaceManager {
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
    fn change_id_display() {
        assert_eq!(ChangeId(42).to_string(), "CH-000042");
    }

    #[test]
    fn create_and_modify_workspace() {
        let mut mgr = WorkspaceManager::new();
        mgr.create_workspace(AgentId(1), SnapshotId(1));

        assert!(mgr.has_workspace(AgentId(1)));
        assert!(!mgr.has_workspace(AgentId(2)));

        let ws = mgr.get_workspace_mut(AgentId(1)).expect("exists");
        ws.record_modification("src/lib.rs", "hash123".into());
        ws.record_creation("src/new.rs", "hash456".into());
        ws.record_deletion("src/old.rs");

        assert_eq!(ws.modified_count(), 3);
        assert!(ws.has_modification(Path::new("src/lib.rs")));
    }

    #[test]
    fn overlapping_paths_detected() {
        let mut mgr = WorkspaceManager::new();
        mgr.create_workspace(AgentId(1), SnapshotId(1));
        mgr.create_workspace(AgentId(2), SnapshotId(1));

        mgr.get_workspace_mut(AgentId(1))
            .expect("exists")
            .record_modification("shared.rs", "a".into());
        mgr.get_workspace_mut(AgentId(2))
            .expect("exists")
            .record_modification("shared.rs", "b".into());

        let overlaps = mgr.overlapping_paths(AgentId(1), AgentId(2));
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0], PathBuf::from("shared.rs"));
    }

    #[test]
    fn destroy_workspace() {
        let mut mgr = WorkspaceManager::new();
        mgr.create_workspace(AgentId(1), SnapshotId(1));
        assert!(mgr.has_workspace(AgentId(1)));

        mgr.destroy_workspace(AgentId(1));
        assert!(!mgr.has_workspace(AgentId(1)));
    }

    #[test]
    fn change_lifecycle() {
        let mut mgr = WorkspaceManager::new();
        mgr.create_workspace(AgentId(1), SnapshotId(1));

        let change_id = mgr.create_change(AgentId(1), "Fix parser bug");
        mgr.add_snapshot_to_change(change_id, SnapshotId(2));
        mgr.add_snapshot_to_change(change_id, SnapshotId(3));

        let change = mgr.get_change(change_id).expect("exists");
        assert_eq!(change.snapshots.len(), 2);
        assert_eq!(change.status, ChangeStatus::InProgress);

        mgr.update_change_status(change_id, ChangeStatus::Merged);
        let change = mgr.get_change(change_id).expect("exists");
        assert_eq!(change.status, ChangeStatus::Merged);

        // Workspace should have active_change set
        let ws = mgr.get_workspace(AgentId(1)).expect("exists");
        assert_eq!(ws.active_change, Some(change_id));
    }

    #[test]
    fn list_changes_filters_by_agent() {
        let mut mgr = WorkspaceManager::new();
        mgr.create_change(AgentId(1), "change A");
        mgr.create_change(AgentId(2), "change B");
        mgr.create_change(AgentId(1), "change C");

        assert_eq!(mgr.list_changes(Some(AgentId(1)), 10).len(), 2);
        assert_eq!(mgr.list_changes(Some(AgentId(2)), 10).len(), 1);
        assert_eq!(mgr.list_changes(None, 10).len(), 3);
    }
}
