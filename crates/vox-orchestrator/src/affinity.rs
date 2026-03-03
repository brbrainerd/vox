use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::types::AgentId;

/// Thread-safe map tracking which agent "owns" each file path.
///
/// The single-writer principle: at most one agent holds write affinity
/// for any given file. This prevents race conditions and lost updates.
#[derive(Clone)]
pub struct FileAffinityMap {
    inner: Arc<RwLock<HashMap<PathBuf, AgentId>>>,
    /// Tracks "experience" — agent_id -> { pattern -> count }
    experience: Arc<RwLock<HashMap<AgentId, HashMap<String, u32>>>>,
}

impl FileAffinityMap {
    /// Create a new, empty affinity map.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            experience: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record that an agent successfully worked on a file (dynamic learning).
    pub fn record_experience(&self, agent: AgentId, file: &Path) {
        let mut exp = self.experience.write().expect("experience lock poisoned");
        let agent_exp = exp.entry(agent).or_insert_with(HashMap::new);

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
        let exp = self.experience.read().expect("experience lock poisoned");
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
        self.inner
            .write()
            .expect("affinity map lock poisoned")
            .insert(file.to_path_buf(), agent);
    }

    /// Look up which agent owns a file, if any.
    pub fn lookup(&self, file: &Path) -> Option<AgentId> {
        self.inner
            .read()
            .expect("affinity map lock poisoned")
            .get(file)
            .copied()
    }

    /// Release ownership of a single file.
    pub fn release(&self, file: &Path) {
        self.inner
            .write()
            .expect("affinity map lock poisoned")
            .remove(file);
    }

    /// Release all files owned by the given agent.
    pub fn release_all(&self, agent: AgentId) {
        self.inner
            .write()
            .expect("affinity map lock poisoned")
            .retain(|_, v| *v != agent);
    }

    /// Atomically look up the owner of a file, or assign it if unowned.
    /// Returns the actual owner (which may differ from `agent` if already claimed).
    pub fn owner_or_assign(&self, file: &Path, agent: AgentId) -> AgentId {
        let mut map = self.inner.write().expect("affinity map lock poisoned");
        *map.entry(file.to_path_buf()).or_insert(agent)
    }

    /// List all files owned by a given agent.
    pub fn files_for_agent(&self, agent: AgentId) -> Vec<PathBuf> {
        self.inner
            .read()
            .expect("affinity map lock poisoned")
            .iter()
            .filter(|(_, v)| **v == agent)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Return a map of agent → number of files owned (for load balancing).
    pub fn agent_load(&self) -> HashMap<AgentId, usize> {
        let map = self.inner.read().expect("affinity map lock poisoned");
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
        manifest: &[crate::types::FileAffinity],
        requesting_agent: AgentId,
    ) -> Vec<(PathBuf, AgentId)> {
        let map = self.inner.read().expect("affinity map lock poisoned");
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
        self.inner.read().expect("affinity map lock poisoned").len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return the entire affinity map as a JSON object (Path -> AgentId).
    pub fn as_json(&self) -> serde_json::Value {
        let map = self.inner.read().expect("affinity map lock poisoned");
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
        use crate::types::FileAffinity;
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
        use crate::types::FileAffinity;
        let map = FileAffinityMap::new();
        map.assign(Path::new("mine.rs"), AgentId(1));

        let manifest = vec![FileAffinity::write("mine.rs")];
        let conflicts = map.conflicts(&manifest, AgentId(1));
        assert!(
            conflicts.is_empty(),
            "agent should not conflict with itself"
        );
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
