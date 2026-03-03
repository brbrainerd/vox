use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::compaction::CompactionConfig;
use crate::memory::MemoryConfig;
use crate::scope::ScopeEnforcement;
use crate::session::SessionConfig;
use crate::types::TaskPriority;

/// Strategy for handling queue overflow when max tasks is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverflowStrategy {
    /// Block the request until space is available.
    Block,
    /// Drop the lowest-priority task to make room.
    DropLowest,
    /// Spawn a new agent to handle overflow.
    SpawnNewAgent,
}

/// Preference for balancing model quality vs operational cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostPreference {
    /// Prioritize model performance/quality over cost.
    Performance,
    /// Prioritize lower cost models even if quality is slightly reduced.
    Economy,
}

/// User-governable scaling profile: when to scale up and how aggressively to scale down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScalingProfile {
    /// Scale up only when load is high; retire idle agents quickly.
    Conservative,
    /// Default balance of scale-up threshold and retirement time.
    #[default]
    Balanced,
    /// Scale up earlier; keep idle agents longer.
    Aggressive,
}

impl ScalingProfile {
    /// Multiplier for scaling_threshold (higher = scale up later).
    pub fn threshold_multiplier(self) -> f64 {
        match self {
            ScalingProfile::Conservative => 1.5,
            ScalingProfile::Balanced => 1.0,
            ScalingProfile::Aggressive => 0.7,
        }
    }

    /// Multiplier for idle_retirement_ms (higher = retire later).
    pub fn retirement_multiplier(self) -> f64 {
        match self {
            ScalingProfile::Conservative => 0.6,
            ScalingProfile::Balanced => 1.0,
            ScalingProfile::Aggressive => 1.5,
        }
    }
}

/// Configuration for the orchestrator system.
///
/// Can be loaded from the `[orchestrator]` section in `Vox.toml`,
/// overridden by `VOX_ORCHESTRATOR_*` environment variables,
/// or constructed programmatically.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestratorConfig {
    /// Whether the orchestrator is enabled (default: true).
    pub enabled: bool,
    /// Maximum number of concurrent agents (default: 8).
    pub max_agents: usize,
    /// Default priority for new tasks (default: Normal).
    pub default_priority: TaskPriority,
    /// How to handle queue overflow (default: SpawnNewAgent).
    pub queue_overflow_strategy: OverflowStrategy,
    /// Lock timeout in milliseconds (default: 30000).
    pub lock_timeout_ms: u64,
    /// Bulletin board broadcast channel capacity (default: 256).
    pub bulletin_capacity: usize,
    /// Whether to fall back to a single agent when routing is ambiguous (default: true).
    pub fallback_to_single_agent: bool,
    /// Whether to run TOESTUB validation after each completed task (default: true).
    pub toestub_gate: bool,
    /// Maximum number of times a task can be re-routed due to validation failures (default: 3).
    pub max_debug_iterations: u8,
    /// Log level for orchestrator events (default: "info").
    pub log_level: String,

    // ── Phase 1: New fields ──────────────────────────────────
    /// Heartbeat check interval in milliseconds (default: 5000).
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_ms: u64,
    /// Threshold in milliseconds before an agent is considered stale (default: 60000).
    #[serde(default = "default_stale_threshold")]
    pub stale_threshold_ms: u64,
    /// Whether auto-continuation is enabled (default: true).
    #[serde(default = "default_true")]
    pub auto_continue_enabled: bool,
    /// Cooldown between auto-continuations per agent in ms (default: 30000).
    #[serde(default = "default_continuation_cooldown")]
    pub continuation_cooldown_ms: u64,
    /// Maximum auto-continuations before requiring manual intervention (default: 5).
    #[serde(default = "default_max_auto_continuations")]
    pub max_auto_continuations: u32,
    /// How strictly to enforce agent scope boundaries (default: Warn).
    #[serde(default)]
    pub scope_enforcement: ScopeEnforcement,
    /// Event bus capacity (default: 1024).
    #[serde(default = "default_event_capacity")]
    pub event_bus_capacity: usize,

    // ── Phase 12: Scaling & Cost ─────────────────────────────
    /// Minimum number of concurrent agents (default: 1).
    #[serde(default = "default_min_agents")]
    pub min_agents: usize,
    /// Number of queued tasks per agent to trigger scaling (default: 5).
    #[serde(default = "default_scaling_threshold")]
    pub scaling_threshold: usize,
    /// Time an idle dynamic agent lives before retirement in ms (default: 300000 / 5min).
    #[serde(default = "default_idle_retirement")]
    pub idle_retirement_ms: u64,
    /// Whether dynamic scaling is enabled (default: false).
    #[serde(default = "default_false")]
    pub scaling_enabled: bool,
    /// Preference for cost vs performance (default: Performance).
    #[serde(default = "default_cost_preference")]
    pub cost_preference: CostPreference,
    /// Number of ticks to look back for predictive scaling (default: 5).
    #[serde(default = "default_lookback_ticks")]
    pub scaling_lookback_ticks: usize,
    /// Weight of system resource usage in load calculation (0.0 to 1.0, default: 0.3).
    #[serde(default = "default_resource_weight")]
    pub resource_weight: f64,
    /// Baseline multiplier for CPU usage in the load calculation (default: 0.7).
    #[serde(default = "default_cpu_multiplier")]
    pub resource_cpu_multiplier: f64,
    /// Baseline multiplier for Memory usage in the load calculation (default: 0.3).
    #[serde(default = "default_mem_multiplier")]
    pub resource_mem_multiplier: f64,
    /// Exponent to apply to the final resource factor, allowing exponential scaling (default: 1.0).
    #[serde(default = "default_resource_exponent")]
    pub resource_exponent: f64,
    /// User-governable scaling profile (conservative / balanced / aggressive).
    #[serde(default)]
    pub scaling_profile: ScalingProfile,
    /// Max number of agents to spawn in one scaling tick (default: 1).
    #[serde(default = "default_max_spawn_per_tick")]
    pub max_spawn_per_tick: usize,
    /// Cooldown in ms between scale-up actions (default: 5000).
    #[serde(default = "default_scaling_cooldown_ms")]
    pub scaling_cooldown_ms: u64,
    /// Number of Urgent tasks on a single agent that triggers an automatic rebalance (default: 3).
    /// Set to 0 to disable urgent auto-rebalance.
    #[serde(default = "default_urgent_rebalance_threshold")]
    pub urgent_rebalance_threshold: usize,

    // ── OpenClaw-Inspired Features ───────────────────────────────────────
    /// Configuration for the context compaction engine.
    #[serde(default)]
    pub compaction: CompactionConfig,
    /// Configuration for the persistent memory system.
    #[serde(default)]
    pub memory: MemoryConfig,
    /// Configuration for the session lifecycle manager.
    #[serde(default)]
    pub session: SessionConfig,
}

fn default_heartbeat_interval() -> u64 {
    5_000
}
fn default_stale_threshold() -> u64 {
    60_000
}
fn default_true() -> bool {
    true
}
fn default_continuation_cooldown() -> u64 {
    30_000
}
fn default_max_auto_continuations() -> u32 {
    5
}
fn default_event_capacity() -> usize {
    1024
}
fn default_min_agents() -> usize {
    1
}
fn default_scaling_threshold() -> usize {
    5
}
fn default_idle_retirement() -> u64 {
    300_000
}
fn default_false() -> bool {
    false
}
fn default_cost_preference() -> CostPreference {
    CostPreference::Performance
}
fn default_lookback_ticks() -> usize {
    5
}
fn default_resource_weight() -> f64 {
    0.3
}
fn default_cpu_multiplier() -> f64 {
    0.7
}
fn default_mem_multiplier() -> f64 {
    0.3
}
fn default_resource_exponent() -> f64 {
    1.0
}
fn default_max_spawn_per_tick() -> usize {
    1
}
fn default_scaling_cooldown_ms() -> u64 {
    5_000
}
fn default_urgent_rebalance_threshold() -> usize {
    3
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_agents: 8,
            default_priority: TaskPriority::Normal,
            queue_overflow_strategy: OverflowStrategy::SpawnNewAgent,
            lock_timeout_ms: 30_000,
            bulletin_capacity: 256,
            fallback_to_single_agent: true,
            toestub_gate: true,
            max_debug_iterations: 3,
            log_level: "info".to_string(),
            heartbeat_interval_ms: default_heartbeat_interval(),
            stale_threshold_ms: default_stale_threshold(),
            auto_continue_enabled: default_true(),
            continuation_cooldown_ms: default_continuation_cooldown(),
            max_auto_continuations: default_max_auto_continuations(),
            scope_enforcement: ScopeEnforcement::default(),
            event_bus_capacity: default_event_capacity(),
            min_agents: default_min_agents(),
            scaling_threshold: default_scaling_threshold(),
            idle_retirement_ms: default_idle_retirement(),
            scaling_enabled: default_false(),
            cost_preference: default_cost_preference(),
            scaling_lookback_ticks: default_lookback_ticks(),
            resource_weight: default_resource_weight(),
            resource_cpu_multiplier: default_cpu_multiplier(),
            resource_mem_multiplier: default_mem_multiplier(),
            resource_exponent: default_resource_exponent(),
            scaling_profile: ScalingProfile::default(),
            max_spawn_per_tick: default_max_spawn_per_tick(),
            scaling_cooldown_ms: default_scaling_cooldown_ms(),
            urgent_rebalance_threshold: default_urgent_rebalance_threshold(),
            compaction: CompactionConfig::default(),
            memory: MemoryConfig::default(),
            session: SessionConfig::default(),
        }
    }
}

impl OrchestratorConfig {
    /// Load configuration from a TOML file.
    ///
    /// Looks for an `[orchestrator]` section in the given file.
    /// Returns the default config if the section is missing.
    pub fn load_from_toml(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        let table: toml::Table = content.parse().map_err(ConfigError::Parse)?;

        if let Some(section) = table.get("orchestrator") {
            let section_str = toml::to_string(section).map_err(ConfigError::Serialize)?;
            let config: Self = toml::from_str(&section_str).map_err(ConfigError::Parse)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Override configuration values from `VOX_ORCHESTRATOR_*` environment variables.
    /// Logs a warning when an env value fails to parse; invalid values are ignored.
    pub fn merge_env_overrides(&mut self) {
        fn parse_or_warn<T: std::str::FromStr>(key: &str, val: &str, default: T) -> T {
            val.parse().unwrap_or_else(|_| {
                tracing::warn!("{}: invalid value '{}', using default", key, val);
                default
            })
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_ENABLED") {
            self.enabled = parse_or_warn("VOX_ORCHESTRATOR_ENABLED", &val, self.enabled);
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_MAX_AGENTS") {
            self.max_agents = parse_or_warn("VOX_ORCHESTRATOR_MAX_AGENTS", &val, self.max_agents);
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_LOCK_TIMEOUT_MS") {
            self.lock_timeout_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_LOCK_TIMEOUT_MS",
                &val,
                self.lock_timeout_ms,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_TOESTUB_GATE") {
            self.toestub_gate =
                parse_or_warn("VOX_ORCHESTRATOR_TOESTUB_GATE", &val, self.toestub_gate);
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_MAX_DEBUG_ITERATIONS") {
            self.max_debug_iterations = parse_or_warn(
                "VOX_ORCHESTRATOR_MAX_DEBUG_ITERATIONS",
                &val,
                self.max_debug_iterations,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_LOG_LEVEL") {
            self.log_level = val;
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_FALLBACK_SINGLE") {
            self.fallback_to_single_agent = parse_or_warn(
                "VOX_ORCHESTRATOR_FALLBACK_SINGLE",
                &val,
                self.fallback_to_single_agent,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_MIN_AGENTS") {
            self.min_agents = parse_or_warn("VOX_ORCHESTRATOR_MIN_AGENTS", &val, self.min_agents);
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_SCALING_THRESHOLD") {
            self.scaling_threshold = parse_or_warn(
                "VOX_ORCHESTRATOR_SCALING_THRESHOLD",
                &val,
                self.scaling_threshold,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_IDLE_RETIREMENT_MS") {
            self.idle_retirement_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_IDLE_RETIREMENT_MS",
                &val,
                self.idle_retirement_ms,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_SCALING_ENABLED") {
            self.scaling_enabled = parse_or_warn(
                "VOX_ORCHESTRATOR_SCALING_ENABLED",
                &val,
                self.scaling_enabled,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_COST_PREFERENCE") {
            match val.to_lowercase().as_str() {
                "performance" => self.cost_preference = CostPreference::Performance,
                "economy" => self.cost_preference = CostPreference::Economy,
                _ => tracing::warn!("VOX_ORCHESTRATOR_COST_PREFERENCE: invalid value '{}', expected 'performance' or 'economy'", val),
            }
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_SCALING_LOOKBACK") {
            self.scaling_lookback_ticks = parse_or_warn(
                "VOX_ORCHESTRATOR_SCALING_LOOKBACK",
                &val,
                self.scaling_lookback_ticks,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_RESOURCE_WEIGHT") {
            self.resource_weight = parse_or_warn(
                "VOX_ORCHESTRATOR_RESOURCE_WEIGHT",
                &val,
                self.resource_weight,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_RESOURCE_CPU_MULT") {
            self.resource_cpu_multiplier = parse_or_warn(
                "VOX_ORCHESTRATOR_RESOURCE_CPU_MULT",
                &val,
                self.resource_cpu_multiplier,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_RESOURCE_MEM_MULT") {
            self.resource_mem_multiplier = parse_or_warn(
                "VOX_ORCHESTRATOR_RESOURCE_MEM_MULT",
                &val,
                self.resource_mem_multiplier,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_RESOURCE_EXPONENT") {
            self.resource_exponent = parse_or_warn(
                "VOX_ORCHESTRATOR_RESOURCE_EXPONENT",
                &val,
                self.resource_exponent,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_SCALING_PROFILE") {
            match val.to_lowercase().as_str() {
                "conservative" => self.scaling_profile = ScalingProfile::Conservative,
                "balanced" => self.scaling_profile = ScalingProfile::Balanced,
                "aggressive" => self.scaling_profile = ScalingProfile::Aggressive,
                _ => tracing::warn!("VOX_ORCHESTRATOR_SCALING_PROFILE: invalid value '{}', expected conservative|balanced|aggressive", val),
            }
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_MAX_SPAWN_PER_TICK") {
            self.max_spawn_per_tick = parse_or_warn(
                "VOX_ORCHESTRATOR_MAX_SPAWN_PER_TICK",
                &val,
                self.max_spawn_per_tick,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_SCALING_COOLDOWN_MS") {
            self.scaling_cooldown_ms = parse_or_warn(
                "VOX_ORCHESTRATOR_SCALING_COOLDOWN_MS",
                &val,
                self.scaling_cooldown_ms,
            );
        }
        if let Ok(val) = std::env::var("VOX_ORCHESTRATOR_URGENT_REBALANCE_THRESHOLD") {
            self.urgent_rebalance_threshold = parse_or_warn(
                "VOX_ORCHESTRATOR_URGENT_REBALANCE_THRESHOLD",
                &val,
                self.urgent_rebalance_threshold,
            );
        }
    }

    /// Create a config suitable for testing (small limits, fast timeouts).
    pub fn for_testing() -> Self {
        Self {
            max_agents: 4,
            lock_timeout_ms: 1000,
            bulletin_capacity: 16,
            toestub_gate: false,
            ..Default::default()
        }
    }
}

/// A validation error encountered when checking an orchestrator configuration.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigValidationError {
    #[error("max_agents must be >= 1 (got {0})")]
    InvalidMaxAgents(usize),
    #[error("lock_timeout_ms must be >= 100 (got {0})")]
    InvalidLockTimeout(u64),
    #[error("bulletin_capacity must be >= 1 (got {0})")]
    InvalidBulletinCapacity(usize),
    #[error("min_agents ({0}) cannot be greater than max_agents ({1})")]
    InvalidScalingLimits(usize, usize),
}

impl OrchestratorConfig {
    /// Validates the configuration against required invariants.
    pub fn validate(&self) -> Result<(), Vec<ConfigValidationError>> {
        let mut errors = Vec::new();

        if self.max_agents < 1 {
            errors.push(ConfigValidationError::InvalidMaxAgents(self.max_agents));
        }
        if self.lock_timeout_ms < 100 {
            errors.push(ConfigValidationError::InvalidLockTimeout(
                self.lock_timeout_ms,
            ));
        }
        if self.bulletin_capacity < 1 {
            errors.push(ConfigValidationError::InvalidBulletinCapacity(
                self.bulletin_capacity,
            ));
        }
        if self.min_agents > self.max_agents {
            errors.push(ConfigValidationError::InvalidScalingLimits(
                self.min_agents,
                self.max_agents,
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Errors that can occur loading orchestrator configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("I/O error reading config: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = OrchestratorConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.max_agents, 8);
        assert_eq!(cfg.default_priority, TaskPriority::Normal);
        assert_eq!(cfg.queue_overflow_strategy, OverflowStrategy::SpawnNewAgent);
        assert_eq!(cfg.lock_timeout_ms, 30_000);
        assert!(cfg.toestub_gate);
        assert!(cfg.fallback_to_single_agent);
        assert_eq!(cfg.min_agents, 1);
        assert!(!cfg.scaling_enabled);
        assert_eq!(cfg.cost_preference, CostPreference::Performance);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let cfg = OrchestratorConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: OrchestratorConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.max_agents, cfg.max_agents);
        assert_eq!(back.enabled, cfg.enabled);
    }

    #[test]
    fn test_config_values() {
        let cfg = OrchestratorConfig::for_testing();
        assert_eq!(cfg.max_agents, 4);
        assert_eq!(cfg.lock_timeout_ms, 1000);
        assert!(!cfg.toestub_gate);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validation_errors() {
        let cfg = OrchestratorConfig {
            max_agents: 0,
            lock_timeout_ms: 50,
            bulletin_capacity: 0,
            ..Default::default()
        };

        let errs = cfg.validate().unwrap_err();
        // max_agents=0, lock_timeout=50, bulletin_capacity=0, AND min_agents(1) > max_agents(0)
        assert_eq!(errs.len(), 4);
        assert!(errs.contains(&ConfigValidationError::InvalidMaxAgents(0)));
        assert!(errs.contains(&ConfigValidationError::InvalidLockTimeout(50)));
        assert!(errs.contains(&ConfigValidationError::InvalidBulletinCapacity(0)));
        assert!(errs.contains(&ConfigValidationError::InvalidScalingLimits(1, 0)));
    }

    #[test]
    fn missing_toml_section_returns_default() {
        // Write a temp TOML without [orchestrator]
        let dir = std::env::temp_dir().join("vox_orch_test");
        std::fs::create_dir_all(&dir).ok();
        let toml_path = dir.join("no_orch.toml");
        std::fs::write(&toml_path, "[package]\nname = \"test\"\n").ok();

        let cfg = OrchestratorConfig::load_from_toml(&toml_path).expect("should load");
        assert_eq!(cfg.max_agents, 8); // default
    }
}
