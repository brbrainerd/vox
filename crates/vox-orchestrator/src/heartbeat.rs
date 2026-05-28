use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::events::{AgentActivity, AgentEventKind, EventBus};
use crate::types::AgentId;

/// Record a heartbeat for a node/agent pair in the database with circuit breaker protection.
pub async fn persist_heartbeat_with_breaker(
    db: &vox_db::VoxDb,
    node_id: &str,
    agent_id: AgentId,
    activity: crate::events::AgentActivity,
    repository_id: &str,
) -> Result<(), String> {
    db.breaker()
        .call(|| async { persist_heartbeat(db, node_id, agent_id, activity, repository_id).await })
        .await
}

/// Record a heartbeat for a node/agent pair in the database.
pub async fn persist_heartbeat(
    store: &vox_db::VoxDb,
    node_id: &str,
    agent_id: AgentId,
    activity: AgentActivity,
    repository_id: &str,
) -> Result<(), String> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    store
        .upsert_mesh_heartbeat(
            node_id,
            &agent_id.0.to_string(),
            &activity.to_string(),
            now_ms,
            repository_id,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Retrieve all heartbeats from the database that are NOT dead (last seen within threshold).
pub async fn live_nodes_from_db(
    store: &vox_db::VoxDb,
    threshold_ms: u64,
    repository_id: &str,
) -> Result<Vec<(String, AgentId, AgentActivity, u64)>, String> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let min_seen = now_ms.saturating_sub(threshold_ms) as i64;

    let rows = store
        .list_live_nodes(min_seen, repository_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut nodes = Vec::new();
    for row in rows {
        let node_id = row[0].clone();
        let agent_id_str = row[1].clone();
        let activity_str = row[2].clone();
        let last_seen_str = row[3].clone();

        let agent_id = AgentId(agent_id_str.parse().unwrap_or(0));
        let activity = activity_str.parse().unwrap_or(AgentActivity::Idle);
        let last_seen = last_seen_str.parse::<u64>().unwrap_or(0);
        nodes.push((node_id, agent_id, activity, last_seen));
    }
    Ok(nodes)
}

/// Remove heartbeats older than max_age_ms (dead nodes).
pub async fn evict_dead_heartbeats(store: &vox_db::VoxDb, max_age_ms: u64) -> Result<u64, String> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let min_seen = now_ms.saturating_sub(max_age_ms) as i64;

    store
        .evict_dead_heartbeats(min_seen)
        .await
        .map_err(|e| e.to_string())
}

const DEFAULT_STALE_THRESHOLD_MS: u64 = 60_000;

/// Graduated staleness levels.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum StalenessLevel {
    /// Heartbeats arriving within policy.
    Healthy,
    /// First missed window; informational.
    Warn,
    /// Sustained misses; operator should investigate.
    Alert,
    /// Approaching eviction; likely stuck or partitioned.
    Critical,
    /// Treat as dead for routing purposes.
    Dead,
}

impl std::fmt::Display for StalenessLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Warn => write!(f, "warn"),
            Self::Alert => write!(f, "alert"),
            Self::Critical => write!(f, "critical"),
            Self::Dead => write!(f, "dead"),
        }
    }
}

/// Policy for graduated heartbeat response.
#[derive(Debug, Clone)]
pub struct HeartbeatPolicy {
    /// Consecutive stale intervals before emitting `Warn`.
    pub warn_after_misses: u32,
    /// Miss count threshold for `Alert`.
    pub alert_after_misses: u32,
    /// Miss count threshold for `Critical`.
    pub critical_after_misses: u32,
    /// Miss count threshold marking the agent as `Dead`.
    pub dead_after_misses: u32,
}

impl Default for HeartbeatPolicy {
    fn default() -> Self {
        Self {
            warn_after_misses: 1,
            alert_after_misses: 3,
            critical_after_misses: 5,
            dead_after_misses: 10,
        }
    }
}

impl HeartbeatPolicy {
    /// Maps a consecutive miss count to the corresponding [`StalenessLevel`].
    pub fn level_for_misses(&self, misses: u32) -> StalenessLevel {
        if misses >= self.dead_after_misses {
            StalenessLevel::Dead
        } else if misses >= self.critical_after_misses {
            StalenessLevel::Critical
        } else if misses >= self.alert_after_misses {
            StalenessLevel::Alert
        } else if misses >= self.warn_after_misses {
            StalenessLevel::Warn
        } else {
            StalenessLevel::Healthy
        }
    }
}

/// Per-agent heartbeat state.
#[derive(Debug, Clone)]
pub struct AgentHeartbeat {
    /// Last successful heartbeat instant.
    pub last_seen: Instant,
    /// Last reported [`AgentActivity`].
    pub activity: AgentActivity,
    /// Whether we already published a stale event for the current episode.
    pub stale_emitted: bool,
    /// Consecutive intervals without a heartbeat.
    pub miss_count: u32,
    /// Latest derived staleness bucket.
    pub staleness: StalenessLevel,
}

/// Tracks agent liveness and detects stale agents with graduated response.
#[derive(Debug)]
pub struct HeartbeatMonitor {
    agents: HashMap<AgentId, AgentHeartbeat>,
    stale_threshold: Duration,
    policy: HeartbeatPolicy,
}

impl HeartbeatMonitor {
    /// Creates a monitor; `stale_threshold_ms` is one missed window before counting a miss.
    pub fn new(stale_threshold_ms: u64) -> Self {
        Self {
            agents: HashMap::new(),
            stale_threshold: Duration::from_millis(stale_threshold_ms),
            policy: HeartbeatPolicy::default(),
        }
    }

    /// Replaces the default [`HeartbeatPolicy`] with custom miss thresholds.
    pub fn with_policy(mut self, policy: HeartbeatPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Starts tracking liveness for a new agent at `Idle` / healthy.
    pub fn register(&mut self, agent_id: AgentId) {
        self.agents.insert(
            agent_id,
            AgentHeartbeat {
                last_seen: Instant::now(),
                activity: AgentActivity::Idle,
                stale_emitted: false,
                miss_count: 0,
                staleness: StalenessLevel::Healthy,
            },
        );
    }

    /// Drops heartbeat state when an agent is torn down.
    pub fn unregister(&mut self, agent_id: AgentId) {
        self.agents.remove(&agent_id);
    }

    /// Records a successful ping and resets miss counters for the agent.
    pub fn heartbeat(&mut self, agent_id: AgentId, activity: AgentActivity) {
        if let Some(hb) = self.agents.get_mut(&agent_id) {
            hb.last_seen = Instant::now();
            hb.activity = activity;
            hb.stale_emitted = false;
            hb.miss_count = 0;
            hb.staleness = StalenessLevel::Healthy;
        }
    }

    /// Check all agents; returns list of (AgentId, StalenessLevel) for newly escalated agents.
    pub fn check_stale(&mut self, event_bus: &EventBus) -> Vec<(AgentId, StalenessLevel)> {
        let now = Instant::now();
        let mut stale = Vec::new();

        for (agent_id, hb) in self.agents.iter_mut() {
            let elapsed = now.duration_since(hb.last_seen);
            if elapsed > self.stale_threshold {
                hb.miss_count += 1;
                let new_level = self.policy.level_for_misses(hb.miss_count);
                let level_changed = new_level != hb.staleness;
                hb.staleness = new_level;

                if level_changed || !hb.stale_emitted {
                    hb.stale_emitted = true;
                    stale.push((*agent_id, new_level));
                    event_bus.emit(AgentEventKind::AgentIdle {
                        agent_id: *agent_id,
                    });
                    tracing::warn!(
                        agent = %agent_id,
                        elapsed_ms = elapsed.as_millis() as u64,
                        misses = hb.miss_count,
                        level = %new_level,
                        "agent heartbeat stale"
                    );
                }
            }
        }

        stale
    }

    /// Returns the last known activity for an agent, if registered.
    pub fn activity(&self, agent_id: AgentId) -> Option<AgentActivity> {
        self.agents.get(&agent_id).map(|hb| hb.activity)
    }

    /// Milliseconds since the last heartbeat for this agent.
    pub fn last_seen_ms(&self, agent_id: AgentId) -> Option<u64> {
        self.agents
            .get(&agent_id)
            .map(|hb| Instant::now().duration_since(hb.last_seen).as_millis() as u64)
    }

    /// Seconds since the last heartbeat for this agent.
    pub fn seconds_since_last_seen(&self, agent_id: AgentId) -> Option<u64> {
        self.last_seen_ms(agent_id).map(|ms| ms / 1000)
    }

    /// Checks if a recheck is warranted based on the gap since the last heartbeat.
    pub fn should_recheck_workspace(&self, agent_id: AgentId, min_gap_secs: u64) -> bool {
        self.seconds_since_last_seen(agent_id)
            .is_none_or(|secs| secs > min_gap_secs)
    }

    /// True if the agent has exceeded the stale interval right now.
    pub fn is_stale(&self, agent_id: AgentId) -> bool {
        self.agents
            .get(&agent_id)
            .map(|hb| Instant::now().duration_since(hb.last_seen) > self.stale_threshold)
            .unwrap_or(false)
    }

    /// Current [`StalenessLevel`] or `Healthy` if unknown.
    pub fn staleness_level(&self, agent_id: AgentId) -> StalenessLevel {
        self.agents
            .get(&agent_id)
            .map(|hb| hb.staleness)
            .unwrap_or(StalenessLevel::Healthy)
    }

    /// Agents whose staleness is at least `level`.
    pub fn at_or_above(&self, level: StalenessLevel) -> Vec<AgentId> {
        self.agents
            .iter()
            .filter(|(_, hb)| hb.staleness >= level)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Full heartbeat table (read-only) for diagnostics.
    pub fn all_agents(&self) -> &HashMap<AgentId, AgentHeartbeat> {
        &self.agents
    }

    /// Number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }
}

impl Default for HeartbeatMonitor {
    fn default() -> Self {
        Self::new(DEFAULT_STALE_THRESHOLD_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_heartbeat() {
        let mut monitor = HeartbeatMonitor::new(100);
        let agent = AgentId(1);
        monitor.register(agent);
        assert_eq!(monitor.agent_count(), 1);
        assert_eq!(monitor.activity(agent), Some(AgentActivity::Idle));
        monitor.heartbeat(agent, AgentActivity::Writing);
        assert_eq!(monitor.activity(agent), Some(AgentActivity::Writing));
    }

    #[test]
    fn stale_detection() {
        let mut monitor = HeartbeatMonitor::new(10);
        let bus = EventBus::new(16);
        let agent = AgentId(1);
        monitor.register(agent);
        let stale = monitor.check_stale(&bus);
        assert!(stale.is_empty());
        std::thread::sleep(vox_config::timeouts::D_20MS);
        let stale = monitor.check_stale(&bus);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].0, agent);
    }

    #[test]
    fn heartbeat_resets_stale() {
        let mut monitor = HeartbeatMonitor::new(10);
        let bus = EventBus::new(16);
        let agent = AgentId(1);
        monitor.register(agent);
        std::thread::sleep(vox_config::timeouts::D_20MS);
        let stale = monitor.check_stale(&bus);
        assert_eq!(stale.len(), 1);
        monitor.heartbeat(agent, AgentActivity::Thinking);
        assert!(!monitor.is_stale(agent));
        assert_eq!(monitor.staleness_level(agent), StalenessLevel::Healthy);
    }

    #[test]
    fn unregister_removes_agent() {
        let mut monitor = HeartbeatMonitor::new(100);
        let agent = AgentId(1);
        monitor.register(agent);
        monitor.unregister(agent);
        assert_eq!(monitor.agent_count(), 0);
        assert_eq!(monitor.activity(agent), None);
    }

    #[test]
    fn heartbeat_policy_levels() {
        let policy = HeartbeatPolicy::default();
        assert_eq!(policy.level_for_misses(0), StalenessLevel::Healthy);
        assert_eq!(policy.level_for_misses(1), StalenessLevel::Warn);
        assert_eq!(policy.level_for_misses(3), StalenessLevel::Alert);
        assert_eq!(policy.level_for_misses(5), StalenessLevel::Critical);
        assert_eq!(policy.level_for_misses(10), StalenessLevel::Dead);
    }

    #[test]
    fn at_or_above_filter() {
        let mut monitor = HeartbeatMonitor::new(10);
        let bus = EventBus::new(16);
        let agent = AgentId(99);
        monitor.register(agent);
        std::thread::sleep(vox_config::timeouts::D_20MS);
        monitor.check_stale(&bus);
        let warns = monitor.at_or_above(StalenessLevel::Warn);
        assert!(warns.contains(&agent));
    }
}
