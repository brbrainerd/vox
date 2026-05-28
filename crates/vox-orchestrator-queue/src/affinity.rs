//! File-level affinity: which agent may write which paths.
//!
//! [`crate::affinity::FileAffinityMap`] enforces single-writer ownership and records pattern
//! experience so routing can prefer agents that have touched similar files.
//!
//! **Affinity is a hint, lock is hard.** Callers wishing to write must additionally hold a
//! `WorkingTreeWrite` capability minted via `vox-orchestrator-cap-mint` and the lock-leader must
//! have granted the lease (Phase 0 / P3-T5).
use std::sync::Arc;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::sync_lock;
use vox_orchestrator_types::AgentId;

// ---------------------------------------------------------------------------
// Vector-clock affinity types (P3-T4)
// ---------------------------------------------------------------------------

/// Opaque identity of a daemon node (UUIDv4 bytes in network order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DaemonId(pub [u8; 16]);

/// Lamport logical clock value. Higher beats lower; ties broken by `DaemonId` bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Lamport(pub u64);

/// Value stored in the vector-clock affinity map (`inner_v`).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AffinityValue {
    pub daemon: DaemonId,
    pub agent: AgentId,
    pub lamport: Lamport,
    /// Wall-clock ms when this entry was last written.
    pub assigned_at_ms: u64,
}

/// Hold-down window: within 60 s of first assignment, higher-lamport remote asserts are ignored
/// to give the local daemon time to propagate its claim.
const HOLD_DOWN_MS: u64 = 60_000;

/// Thread-safe map tracking which agent "owns" each file path.
///
/// The single-writer principle: at most one agent holds write affinity
/// for any given file. This prevents race conditions and lost updates.
///
/// **Affinity is a hint, lock is hard.** See module-level docs.
#[derive(Clone)]
pub struct FileAffinityMap {
    inner: Arc<std::sync::RwLock<HashMap<PathBuf, AgentId>>>,
    /// Tracks "experience" — agent_id -> { pattern -> count }
    experience: Arc<std::sync::RwLock<HashMap<AgentId, HashMap<String, u32>>>>,
    /// Vector-clock affinity map for gossip-replicated LWW resolution (P3-T4).
    inner_v: Arc<std::sync::RwLock<HashMap<PathBuf, AffinityValue>>>,
}

impl FileAffinityMap {
    /// Create a new, empty affinity map.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(std::sync::RwLock::new(HashMap::new())),
            experience: Arc::new(std::sync::RwLock::new(HashMap::new())),
            inner_v: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    // ---------------------------------------------------------------------------
    // Vector-clock affinity (P3-T4)
    // ---------------------------------------------------------------------------

    /// Assign file affinity with Lamport LWW semantics and 60 s hold-down.
    ///
    /// Conflict resolution:
    /// 1. If the current owner is a different daemon and the entry is less than
    ///    `HOLD_DOWN_MS` old, the new claim is **ignored** (local stability wins).
    /// 2. After the hold-down, the entry with the higher `lamport` wins;
    ///    equal lamports break ties by daemon-id bytes (deterministic total order).
    ///
    /// **Affinity is a hint, lock is hard.** See module-level docs.
    pub fn assign_v(
        &self,
        file: &Path,
        daemon: DaemonId,
        agent: AgentId,
        lamport: Lamport,
        now_ms: u64,
    ) {
        let mut g = sync_lock::rw_write(&*self.inner_v);
        let new = AffinityValue {
            daemon,
            agent,
            lamport,
            assigned_at_ms: now_ms,
        };
        match g.get(file) {
            None => {
                g.insert(file.to_path_buf(), new);
            }
            Some(cur) => {
                let in_holddown = cur.assigned_at_ms.saturating_add(HOLD_DOWN_MS) > now_ms
                    && cur.daemon != daemon;
                if in_holddown {
                    return;
                }
                if new.lamport > cur.lamport
                    || (new.lamport == cur.lamport && new.daemon.0 > cur.daemon.0)
                {
                    g.insert(file.to_path_buf(), new);
                }
            }
        }
    }

    /// Look up the current vector-clock affinity owner for a file, if any.
    ///
    /// **Affinity is a hint, lock is hard.** See module-level docs.
    pub fn lookup_v(&self, file: &Path) -> Option<AffinityValue> {
        sync_lock::rw_read(&*self.inner_v).get(file).copied()
    }

    /// Record that an agent successfully worked on a file (dynamic learning).
    pub fn record_experience(&self, agent: AgentId, file: &Path) {
        let mut exp = sync_lock::rw_write(&*self.experience);
        let agent_exp = exp.entry(agent).or_default();

        // Increment for extension
        if let Some(ext) = file.extension().and_then(|e| e.to_str()) {
            let key = format!("ext:.{}", ext);
            *agent_exp.entry(key).or_insert(0) += 1;
        }

        // Increment for parent directory
        if let Some(parent) = file.parent().and_then(|p| p.to_str()) {
            let key = format!("dir:{}", parent);
            *agent_exp.entry(key).or_insert(0) += 1;
        }
    }

    /// Recommend the best agent for a file based on learned experience.
    pub fn best_agent_for(&self, file: &Path) -> Option<AgentId> {
        let exp = sync_lock::rw_read(&*self.experience);
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!("ext:.{}", e));
        let dir = file
            .parent()
            .and_then(|p| p.to_str())
            .map(|p| format!("dir:{}", p));

        let mut best_agent = None;
        let mut max_score = 0;

        for (agent, patterns) in exp.iter() {
            let mut score = 0;
            if let Some(ref e) = ext {
                score += patterns.get(e).cloned().unwrap_or(0);
            }
            if let Some(ref d) = dir {
                score += patterns.get(d).cloned().unwrap_or(0);
            }

            if score > max_score {
                max_score = score;
                best_agent = Some(*agent);
            }
        }

        best_agent
    }

    /// Assign a file to an agent. Overwrites any previous assignment.
    pub fn assign(&self, file: &Path, agent: AgentId) {
        sync_lock::rw_write(&*self.inner).insert(file.to_path_buf(), agent);
    }

    /// Look up which agent owns a file, if any.
    pub fn lookup(&self, file: &Path) -> Option<AgentId> {
        sync_lock::rw_read(&*self.inner).get(file).copied()
    }

    /// Release ownership of a single file.
    pub fn release(&self, file: &Path) {
        sync_lock::rw_write(&*self.inner).remove(file);
    }

    /// Release all files owned by the given agent.
    pub fn release_all(&self, agent: AgentId) {
        sync_lock::rw_write(&*self.inner).retain(|_, v| *v != agent);
    }

    /// Atomically look up the owner of a file, or assign it if unowned.
    /// Returns the actual owner (which may differ from `agent` if already claimed).
    pub fn owner_or_assign(&self, file: &Path, agent: AgentId) -> AgentId {
        let mut map = sync_lock::rw_write(&*self.inner);
        *map.entry(file.to_path_buf()).or_insert(agent)
    }

    /// List all files owned by a given agent.
    pub fn files_for_agent(&self, agent: AgentId) -> Vec<PathBuf> {
        sync_lock::rw_read(&*self.inner)
            .iter()
            .filter(|(_, v)| **v == agent)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Return a map of agent → number of files owned (for load balancing).
    pub fn agent_load(&self) -> HashMap<AgentId, usize> {
        let map = sync_lock::rw_read(&*self.inner);
        let mut load: HashMap<AgentId, usize> = HashMap::new();
        for agent in map.values() {
            *load.entry(*agent).or_insert(0) += 1;
        }
        load
    }

    /// Check which files in a manifest are already owned by other agents.
    /// Returns `(file_path, current_owner)` pairs for each conflict.
    pub fn conflicts(
        &self,
        manifest: &[vox_orchestrator_types::FileAffinity],
        requesting_agent: AgentId,
    ) -> Vec<(PathBuf, AgentId)> {
        let map = sync_lock::rw_read(&*self.inner);
        manifest
            .iter()
            .filter_map(|fa| {
                map.get(&fa.path).and_then(|owner| {
                    if *owner != requesting_agent {
                        Some((fa.path.clone(), *owner))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Total number of file assignments.
    pub fn len(&self) -> usize {
        sync_lock::rw_read(&*self.inner).len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return the entire affinity map as a JSON object (Path -> AgentId).
    pub fn as_json(&self) -> serde_json::Value {
        let map = sync_lock::rw_read(&*self.inner);
        let mut obj = serde_json::Map::new();
        for (path, agent) in map.iter() {
            obj.insert(
                path.display().to_string(),
                serde_json::json!(agent.0.to_string()),
            );
        }
        serde_json::Value::Object(obj)
    }
}

impl Default for FileAffinityMap {
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
    fn assign_and_lookup() {
        let map = FileAffinityMap::new();
        let agent = AgentId(1);
        map.assign(Path::new("src/lib.rs"), agent);
        assert_eq!(map.lookup(Path::new("src/lib.rs")), Some(agent));
        assert_eq!(map.lookup(Path::new("src/main.rs")), None);
    }

    #[test]
    fn release_single() {
        let map = FileAffinityMap::new();
        map.assign(Path::new("a.rs"), AgentId(1));
        map.assign(Path::new("b.rs"), AgentId(1));
        map.release(Path::new("a.rs"));
        assert_eq!(map.lookup(Path::new("a.rs")), None);
        assert_eq!(map.lookup(Path::new("b.rs")), Some(AgentId(1)));
    }

    #[test]
    fn release_all_for_agent() {
        let map = FileAffinityMap::new();
        map.assign(Path::new("a.rs"), AgentId(1));
        map.assign(Path::new("b.rs"), AgentId(1));
        map.assign(Path::new("c.rs"), AgentId(2));
        map.release_all(AgentId(1));
        assert_eq!(map.lookup(Path::new("a.rs")), None);
        assert_eq!(map.lookup(Path::new("b.rs")), None);
        assert_eq!(map.lookup(Path::new("c.rs")), Some(AgentId(2)));
    }

    #[test]
    fn owner_or_assign_existing() {
        let map = FileAffinityMap::new();
        map.assign(Path::new("x.rs"), AgentId(1));
        // Agent 2 tries to claim x.rs — should get back agent 1
        let owner = map.owner_or_assign(Path::new("x.rs"), AgentId(2));
        assert_eq!(owner, AgentId(1));
    }

    #[test]
    fn owner_or_assign_new() {
        let map = FileAffinityMap::new();
        let owner = map.owner_or_assign(Path::new("new.rs"), AgentId(5));
        assert_eq!(owner, AgentId(5));
    }

    #[test]
    fn files_for_agent_filter() {
        let map = FileAffinityMap::new();
        map.assign(Path::new("a.rs"), AgentId(1));
        map.assign(Path::new("b.rs"), AgentId(2));
        map.assign(Path::new("c.rs"), AgentId(1));
        let mut files = map.files_for_agent(AgentId(1));
        files.sort();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn agent_load_counts() {
        let map = FileAffinityMap::new();
        map.assign(Path::new("a.rs"), AgentId(1));
        map.assign(Path::new("b.rs"), AgentId(1));
        map.assign(Path::new("c.rs"), AgentId(2));
        let load = map.agent_load();
        assert_eq!(load[&AgentId(1)], 2);
        assert_eq!(load[&AgentId(2)], 1);
    }

    #[test]
    fn conflicts_detection() {
        use vox_orchestrator_types::FileAffinity;
        let map = FileAffinityMap::new();
        map.assign(Path::new("owned.rs"), AgentId(1));

        let manifest = vec![
            FileAffinity::write("owned.rs"),
            FileAffinity::write("free.rs"),
        ];
        let conflicts = map.conflicts(&manifest, AgentId(2));
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].1, AgentId(1));
    }

    #[test]
    fn conflicts_no_self_conflict() {
        use vox_orchestrator_types::FileAffinity;
        let map = FileAffinityMap::new();
        map.assign(Path::new("mine.rs"), AgentId(1));

        let manifest = vec![FileAffinity::write("mine.rs")];
        let conflicts = map.conflicts(&manifest, AgentId(1));
        assert!(
            conflicts.is_empty(),
            "agent should not conflict with itself"
        );
    }

    // ---------------------------------------------------------------------------
    // Vector-clock affinity tests (P3-T4)
    // ---------------------------------------------------------------------------

    #[test]
    fn lww_with_holddown_keeps_local_for_60s_then_yields_to_higher_lamport() {
        let aff = FileAffinityMap::new();
        let local = DaemonId([1u8; 16]);
        let remote = DaemonId([2u8; 16]);
        let t0 = 1_700_000_000_000u64;

        // Local claims at t0.
        aff.assign_v(Path::new("a.rs"), local, AgentId(1), Lamport(100), t0);
        assert_eq!(aff.lookup_v(Path::new("a.rs")).unwrap().daemon, local);

        // Remote asserts higher lamport within 60s — ignored (hold-down).
        aff.assign_v(
            Path::new("a.rs"),
            remote,
            AgentId(7),
            Lamport(200),
            t0 + 1_000,
        );
        assert_eq!(aff.lookup_v(Path::new("a.rs")).unwrap().daemon, local);

        // After 60 s, higher lamport wins.
        aff.assign_v(
            Path::new("a.rs"),
            remote,
            AgentId(7),
            Lamport(200),
            t0 + 60_001,
        );
        assert_eq!(aff.lookup_v(Path::new("a.rs")).unwrap().daemon, remote);
    }

    #[test]
    fn lww_same_daemon_always_updates() {
        let aff = FileAffinityMap::new();
        let d = DaemonId([5u8; 16]);
        let t0 = 1_000_000u64;
        aff.assign_v(Path::new("b.rs"), d, AgentId(1), Lamport(10), t0);
        // Same daemon: hold-down doesn't apply.
        aff.assign_v(Path::new("b.rs"), d, AgentId(1), Lamport(20), t0 + 100);
        assert_eq!(
            aff.lookup_v(Path::new("b.rs")).unwrap().lamport,
            Lamport(20)
        );
    }

    #[test]
    fn lww_lower_lamport_does_not_overwrite_after_holddown() {
        let aff = FileAffinityMap::new();
        let d1 = DaemonId([1u8; 16]);
        let d2 = DaemonId([2u8; 16]);
        let t0 = 1_000_000u64;
        aff.assign_v(Path::new("c.rs"), d1, AgentId(1), Lamport(50), t0);
        // After hold-down, lower lamport does NOT win.
        aff.assign_v(Path::new("c.rs"), d2, AgentId(2), Lamport(40), t0 + 60_001);
        assert_eq!(aff.lookup_v(Path::new("c.rs")).unwrap().daemon, d1);
    }

    #[test]
    fn lww_equal_lamport_tiebreaks_by_daemon_id() {
        let aff = FileAffinityMap::new();
        let d1 = DaemonId([1u8; 16]);
        let d2 = DaemonId([2u8; 16]); // higher bytes → wins
        let t0 = 1_000_000u64;
        aff.assign_v(Path::new("d.rs"), d1, AgentId(1), Lamport(10), t0);
        aff.assign_v(Path::new("d.rs"), d2, AgentId(2), Lamport(10), t0 + 60_001);
        assert_eq!(aff.lookup_v(Path::new("d.rs")).unwrap().daemon, d2);
    }

    #[test]
    fn lookup_v_returns_none_for_unknown_file() {
        let aff = FileAffinityMap::new();
        assert!(aff.lookup_v(Path::new("ghost.rs")).is_none());
    }

    #[test]
    fn dynamic_experience_learning() {
        let map = FileAffinityMap::new();
        let agent1 = AgentId(1);
        let agent2 = AgentId(2);

        map.record_experience(agent1, Path::new("src/parser.rs"));
        map.record_experience(agent1, Path::new("src/lexer.rs"));
        map.record_experience(agent2, Path::new("docs/readme.md"));

        // agent1 should be best for a new .rs file in src
        assert_eq!(map.best_agent_for(Path::new("src/new.rs")), Some(agent1));
        // agent2 should be best for a .md file
        assert_eq!(map.best_agent_for(Path::new("other/info.md")), Some(agent2));
    }
}
