//! Agent heartbeat monitor with auto-stale detection and graduated response.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::events::{AgentActivity, AgentEventKind, EventBus};
use crate::types::AgentId;

const DEFAULT_STALE_THRESHOLD_MS: u64 = 60_000;

/// Graduated staleness levels.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum StalenessLevel {
    Healthy,
    Warn,
    Alert,
    Critical,
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
    pub warn_after_misses: u32,
    pub alert_after_misses: u32,
    pub critical_after_misses: u32,
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
    pub last_seen: Instant,
    pub activity: AgentActivity,
    pub stale_emitted: bool,
    pub miss_count: u32,
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
    pub fn new(stale_threshold_ms: u64) -> Self {
        Self {
            agents: HashMap::new(),
            stale_threshold: Duration::from_millis(stale_threshold_ms),
            policy: HeartbeatPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: HeartbeatPolicy) -> Self {
        self.policy = policy;
        self
    }

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

    pub fn unregister(&mut self, agent_id: AgentId) {
        self.agents.remove(&agent_id);
    }

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

    pub fn activity(&self, agent_id: AgentId) -> Option<AgentActivity> {
        self.agents.get(&agent_id).map(|hb| hb.activity)
    }

    pub fn last_seen_ms(&self, agent_id: AgentId) -> Option<u64> {
        self.agents
            .get(&agent_id)
            .map(|hb| Instant::now().duration_since(hb.last_seen).as_millis() as u64)
    }

    pub fn is_stale(&self, agent_id: AgentId) -> bool {
        self.agents
            .get(&agent_id)
            .map(|hb| Instant::now().duration_since(hb.last_seen) > self.stale_threshold)
            .unwrap_or(false)
    }

    pub fn staleness_level(&self, agent_id: AgentId) -> StalenessLevel {
        self.agents
            .get(&agent_id)
            .map(|hb| hb.staleness)
            .unwrap_or(StalenessLevel::Healthy)
    }

    pub fn at_or_above(&self, level: StalenessLevel) -> Vec<AgentId> {
        self.agents
            .iter()
            .filter(|(_, hb)| hb.staleness >= level)
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn all_agents(&self) -> &HashMap<AgentId, AgentHeartbeat> {
        &self.agents
    }

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
        std::thread::sleep(Duration::from_millis(20));
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
        std::thread::sleep(Duration::from_millis(20));
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
        std::thread::sleep(Duration::from_millis(20));
        monitor.check_stale(&bus);
        let warns = monitor.at_or_above(StalenessLevel::Warn);
        assert!(warns.contains(&agent));
    }
}
