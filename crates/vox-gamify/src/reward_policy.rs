//! Dynamic reward policy engine.
//!
//! Replaces hardcoded XP/crystal constants scattered through `process_event_rewards`
//! with a single policy lookup. Values are multiplied by the active `GamifyMode`
//! multiplier, capped by anti-grind windows, and adjusted for streak bonuses.
//!
//! ## Design
//! - `BaseReward` defines per-event-type floor values.
//! - `PolicyEngine` applies mode multiplier + streak multiplier + novelty bonus.
//! - Anti-grind: same event repeated > `GRIND_WINDOW` times in `GRIND_SECS` yields 0.
//! - Variety bonus: first occurrence of a new event type in a session gets +50%.

use std::collections::HashMap;

// ── Constants ────────────────────────────────────────────

/// Session-scoped state for anti-grind and variety tracking.
/// Initialize once per process and pass into `PolicyEngine`.
#[derive(Debug, Default, Clone)]
pub struct SessionState {
    /// Count of event types seen this session.
    event_counts: HashMap<String, u32>,
    /// Set of event types seen for the first time (variety bonus tracking).
    seen_types: std::collections::HashSet<String>,
}

impl SessionState {
    /// Record an event occurrence and return the updated count.
    pub fn record(&mut self, event_type: &str) -> u32 {
        let count = self.event_counts.entry(event_type.to_string()).or_insert(0);
        *count += 1;
        let is_novel = self.seen_types.insert(event_type.to_string());
        if is_novel {
            tracing::debug!(
                "variety bonus eligible for first occurrence of '{}'",
                event_type
            );
        }
        *count
    }

    /// Whether this is the first occurrence of an event type this session.
    pub fn has_not_been_seen(&self, event_type: &str) -> bool {
        !self.seen_types.contains(event_type)
    }

    /// How many times this event type has been seen this session.
    pub fn count(&self, event_type: &str) -> u32 {
        self.event_counts.get(event_type).copied().unwrap_or(0)
    }
}

/// How many times the same event may fire in a session before rewards taper out.
pub const GRIND_TAPER_END: u32 = 30;

/// The point at which rewards become completely suppressed.
pub const GRIND_ZERO_THRESHOLD: u32 = 31;

/// Novelty bonus factor applied to the first occurrence of each event type.
pub const NOVELTY_FACTOR: f64 = 1.5;

// ── Base rewards ─────────────────────────────────────────

/// Per-event base XP, crystal, and lumens awards.
#[derive(Debug, Clone)]
pub struct BaseReward {
    /// Base XP awarded before multipliers.
    pub xp: u64,
    /// Base crystal currency awarded before multipliers.
    pub crystals: u64,
    /// Base lumens awarded (usually 0 unless pro-social).
    pub lumens: i64,
    /// Whether this event grants a streak shield.
    pub grant_shield: bool,
}

impl BaseReward {
    const fn new(xp: u64, crystals: u64) -> Self {
        Self {
            xp,
            crystals,
            lumens: 0,
            grant_shield: false,
        }
    }

    const fn with_lumens(xp: u64, crystals: u64, lumens: i64) -> Self {
        Self {
            xp,
            crystals,
            lumens,
            grant_shield: false,
        }
    }

    const fn with_shield(xp: u64, crystals: u64) -> Self {
        Self {
            xp,
            crystals,
            lumens: 0,
            grant_shield: true,
        }
    }
}

/// Look up the base reward for a known event type.
pub fn base_reward(event_type: &str) -> BaseReward {
    match event_type {
        // Task lifecycle
        "task_completed" => BaseReward::new(50, 5),
        "task_started" => BaseReward::new(5, 1),
        // Queued work (orchestrator bus) — policy-first; companion/counters in `process_rewards`.
        "task_submitted" => BaseReward::new(8, 1),
        "task_failed" => BaseReward::new(0, 0),
        "task_doubted" => BaseReward::new(10, 2),
        "task_resolved" => BaseReward::new(20, 4), // Base resolution reward

        // Agent lifecycle
        "agent_spawned" => BaseReward::new(25, 2),
        "agent_retired" => BaseReward::new(10, 0),
        "agent_idle" => BaseReward::new(0, 0),
        "agent_busy" => BaseReward::new(2, 0),

        // Git/VCS (rewarding good engineering practice)
        "snapshot_captured" => BaseReward::new(30, 6),
        "operation_undone" => BaseReward::new(5, 0),
        "operation_redone" => BaseReward::new(5, 0),
        "conflict_resolved" => BaseReward::with_lumens(100, 20, 10),

        // Collaboration
        "plan_handoff" => BaseReward::new(40, 8),
        "agent_handoff_accepted" => BaseReward::new(50, 10),
        "peer_teach_session" => BaseReward::with_lumens(500, 100, 50), // High social value
        "message_sent" => BaseReward::new(1, 0),

        // GitHub / Collaboration
        "pr_merged" => BaseReward::with_lumens(200, 40, 15),
        "code_reviewed" => BaseReward::with_lumens(100, 20, 10),
        "issue_closed" => BaseReward::new(150, 30),
        "helped_peer" => BaseReward::with_lumens(300, 60, 20),
        "bounty_completed" => BaseReward::with_lumens(1000, 200, 50),

        // Code quality signals
        "refactor" => BaseReward::with_lumens(150, 30, 5),
        "bug_fix" => BaseReward::with_lumens(200, 40, 8),
        "test_pass" => BaseReward::new(55, 10),
        "lint_clean" => BaseReward::new(30, 6),
        "doc_added" => BaseReward::new(28, 6),

        // CLI command completions
        "build_completed" => BaseReward::new(25, 5),
        "build_failed" => BaseReward::new(5, 0), // Struggle XP: showing up
        "check_completed" => BaseReward::new(15, 3),
        "check_failed" => BaseReward::new(3, 0), // Struggle XP
        "test_fail" => BaseReward::new(10, 0),   // Struggle XP: at least you ran it
        "fmt_completed" => BaseReward::new(2, 0),
        // Generic CLI command completion (any command not covered by a specific
        // type above), emitted once per invocation by central dispatch.
        "cli_command_completed" => BaseReward::new(8, 1),
        "cli_command_failed" => BaseReward::new(2, 0), // Struggle XP
        // LSP events
        "diagnostics_clean" => BaseReward::new(5, 1),
        "completion_accepted" => BaseReward::new(1, 0),
        "bundle_completed" => BaseReward::new(50, 10),

        // ── Build mastery ─────────────────────────────────
        "build_clean" => BaseReward::new(60, 12),
        "build_failed_then_fixed" => BaseReward::with_lumens(100, 20, 3), // Phoenix bonus
        "phoenix_bonus" => BaseReward::with_lumens(150, 30, 5),           // Fired externally
        "build_clean_streak_3" => BaseReward::with_shield(200, 40),       // 3 cleans today = shield
        "check_clean_first_try" => BaseReward::new(40, 8),
        "test_suite_green" => BaseReward::with_shield(250, 50),
        "test_coverage_improved" => BaseReward::with_lumens(150, 30, 7),
        "toestub_violations_fixed" => BaseReward::with_lumens(300, 60, 12),
        // Clean TOESTUB workspace scan (`vox stub-check` with zero findings).
        "toestub_scan_clean" => BaseReward::new(10, 5),
        // Non-rewarding audit event: structural debt signal for teaching hints.
        "stub_check_debt" => BaseReward::new(0, 0),
        "fmt_applied" => BaseReward::new(2, 0),

        // ── Documentation ──────────────────────────────────
        "doc_coverage_100_pct" => BaseReward::with_lumens(1000, 200, 100),
        "missing_docs_zero" => BaseReward::with_lumens(500, 100, 40),

        // ── AI corpus / feedback ──────────────────────────
        "ai_thumbs_up" => BaseReward::new(20, 4),
        "ai_thumbs_down" => BaseReward::new(15, 3), // Still valuable data
        "ai_example_written" => BaseReward::new(200, 40),
        "ai_example_accepted" => BaseReward::with_lumens(1000, 200, 25),
        "populi_corpus_contributed" => BaseReward::with_lumens(500, 100, 15),
        "populi_inference_run" => BaseReward::new(5, 1),
        "populi_finetune_epoch" => BaseReward::new(2000, 400),
        "mens_flywheel_triggered" => BaseReward::with_lumens(200, 40, 10),

        // ── Vox language features ─────────────────────────
        "vox_example_created" => BaseReward::new(200, 40),
        "vox_example_canonical" => BaseReward::with_lumens(5000, 1000, 500), // Massive achievement
        "migration_applied" => BaseReward::new(100, 20),
        "seed_completed" => BaseReward::new(50, 10),
        "vox_web_page_rendered" => BaseReward::new(20, 4),
        "v0_import_complete" => BaseReward::new(150, 30),
        "lsp_go_to_def_used" => BaseReward::new(1, 0),
        "lsp_completion_accepted" => BaseReward::new(1, 0),
        "openapi_spec_generated" => BaseReward::new(100, 20),
        "scheduled_job_ran" => BaseReward::new(40, 8),
        "turso_query_executed" => BaseReward::new(10, 2),

        // ── MCP / capability registry ─────────────────────
        "mcp_tool_called" => BaseReward::new(15, 3),
        "mcp_tool_registered" => BaseReward::new(300, 60),

        // ── Package manager ───────────────────────────────
        "pkg_published" => BaseReward::new(1500, 300),
        "pkg_installed" => BaseReward::new(20, 4),

        // ── Runtime / actors / workflows ─────────────────
        "workflow_completed" => BaseReward::new(1200, 240),
        "workflow_checkpoint_saved" => BaseReward::new(50, 10),
        "actor_message_sent" => BaseReward::new(5, 1),
        "actor_spawned" => BaseReward::new(60, 12),

        // ── Security ─────────────────────────────────────
        "security_review_passed" => BaseReward::with_lumens(1500, 300, 50),
        "perf_regression_caught" => BaseReward::with_lumens(800, 160, 25),
        "unsafe_removed" => BaseReward::with_lumens(400, 80, 15),

        // ── Social & Events ──────────────────────────────
        "collegium_created" => BaseReward::new(200, 40),
        "collegium_joined" => BaseReward::new(100, 20),
        "arena_joined" => BaseReward::new(50, 10),

        // ── Combo chain bonuses ─────────────────────────
        "virtus_trifecta" => BaseReward::with_lumens(500, 100, 20),
        "exterminatus" => BaseReward::new(200, 200),
        "iron_will_recovery" => BaseReward::with_lumens(300, 60, 10),
        "scribes_fury" => BaseReward::with_lumens(400, 80, 15),
        "review_fix_ship_bonus" => BaseReward::with_lumens(320, 50, 12),

        // Cost / continuation
        "cost_incurred" => BaseReward::new(0, 0),
        "continuation_triggered" => BaseReward::new(10, 2),

        // Safety events
        "scope_violation" => BaseReward::new(0, 0),

        // Default: 0 reward for unknown events
        _ => BaseReward::new(0, 0),
    }
}

/// Small crystal bonus in **Learning** mode only: deterministic per user, day (`quest::current_day_number`), and `event_type`. Zero when the policy awarded no crystals.
#[must_use]
pub fn learning_mode_crystal_jitter(
    user_id: &str,
    event_type: &str,
    base_crystals_after_policy: u64,
) -> u64 {
    learning_mode_crystal_jitter_cfg(
        &crate::economy::EconomyConfig::default(),
        user_id,
        event_type,
        base_crystals_after_policy,
    )
}

/// Config-aware variant of [`learning_mode_crystal_jitter`] sourcing the jitter
/// modulus from the loaded [`crate::economy::EconomyConfig`]. With the default
/// config (modulus 4) this is byte-identical to the legacy function.
#[must_use]
pub fn learning_mode_crystal_jitter_cfg(
    cfg: &crate::economy::EconomyConfig,
    user_id: &str,
    event_type: &str,
    base_crystals_after_policy: u64,
) -> u64 {
    if base_crystals_after_policy == 0 {
        return 0;
    }
    if !matches!(crate::config_gate::mode(), vox_config::GamifyMode::Learning) {
        return 0;
    }
    let modulus = cfg.tuning.learning_jitter_modulus.max(1);
    // FNV-1a inline to guarantee stability across Rust upgrades.
    struct Fnv1a(u64);
    impl std::hash::Hasher for Fnv1a {
        fn finish(&self) -> u64 {
            self.0
        }
        fn write(&mut self, bytes: &[u8]) {
            for &b in bytes {
                self.0 = (self.0 ^ b as u64).wrapping_mul(1099511628211);
            }
        }
    }
    use std::hash::{Hash, Hasher};
    let day = crate::quest::current_day_number();
    let mut h = Fnv1a(14695981039346656037);
    user_id.hash(&mut h);
    day.hash(&mut h);
    event_type.hash(&mut h);
    h.finish() % modulus
}

// ── Trust Tier ────────────────────────────────────────────

/// Multiplier for rewards based on the player's community trust tier.
pub fn trust_tier_multiplier(tier: crate::profile::TrustTier) -> f64 {
    match tier {
        crate::profile::TrustTier::Novice => 0.5,
        crate::profile::TrustTier::Linked => 1.0,
        crate::profile::TrustTier::Proven => 1.2,
        crate::profile::TrustTier::Master => 1.5,
    }
}

// ── Policy engine ─────────────────────────────────────────

/// Calculated reward after policy application.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PolicyReward {
    /// XP awarded after all policy multipliers.
    pub xp: u64,
    /// Crystals awarded after all policy multipliers.
    pub crystals: u64,
    /// Lumens awarded (not affected by multipliers).
    pub lumens: i64,
    /// Whether a streak shield was granted.
    pub grant_shield: bool,
    /// Multiplier actually applied (for diagnostics).
    pub effective_multiplier: f64,
    /// Whether the anti-grind cap zeroed this reward.
    pub grind_capped: bool,
}

/// Apply the full policy stack to an event.
///
/// - `base`: floor values from `base_reward()`
/// - `mode_multiplier`: from active `GamifyMode`
/// - `streak_days`: current user streak (adds up to 50% bonus)
/// - `session`: mutable session state for anti-grind and novelty
pub fn apply_policy(
    base: &BaseReward,
    mode_multiplier: f64,
    streak_days: u32,
    trust_tier: crate::profile::TrustTier,
    event_type: &str,
    session: &mut SessionState,
) -> PolicyReward {
    // Backward-compatible path: source tuning/trust from the in-code defaults
    // (which mirror the constants in this module). Live callers should prefer
    // `apply_policy_cfg` to read from the loaded `EconomyConfig`.
    apply_policy_cfg(
        &crate::economy::EconomyConfig::default(),
        base,
        mode_multiplier,
        streak_days,
        trust_tier,
        event_type,
        session,
    )
}

/// Apply the full policy stack, sourcing all tuning scalars and trust-tier
/// multipliers from a loaded [`crate::economy::EconomyConfig`] (the economy
/// contract) rather than the bare in-code constants.
///
/// When `cfg` is [`EconomyConfig::default`] this is byte-identical to the
/// legacy [`apply_policy`] (the defaults mirror the constants in this module),
/// so the migration is behavior-preserving for the shipped contract.
#[allow(clippy::too_many_arguments)]
pub fn apply_policy_cfg(
    cfg: &crate::economy::EconomyConfig,
    base: &BaseReward,
    mode_multiplier: f64,
    streak_days: u32,
    trust_tier: crate::profile::TrustTier,
    event_type: &str,
    session: &mut SessionState,
) -> PolicyReward {
    let tuning = &cfg.tuning;
    // Record occurrence then apply tiered decay. High-frequency / low-signal events taper faster.
    let count = session.record(event_type);
    let (full_cap, half_cap) = match event_type {
        "mcp_tool_called" | "message_sent" | "actor_message_sent" | "build_completed"
        | "task_submitted" | "lock_acquired" | "lock_released" | "snapshot_captured" => {
            tuning.grind_caps_high_frequency
        }
        _ => tuning.grind_caps_default,
    };
    let grind_multiplier = match count {
        c if c <= full_cap => 1.0,
        c if c <= half_cap => 0.5,
        c if c <= tuning.grind_taper_end => 0.1,
        _ => {
            tracing::debug!(
                "grind cap: event '{}' has fired {} times this session, reward zeroed",
                event_type,
                count
            );
            return PolicyReward {
                xp: 0,
                crystals: 0,
                lumens: 0,
                grant_shield: false,
                effective_multiplier: 0.0,
                grind_capped: true,
            };
        }
    };

    // Streak bonus: +2% per day, max +50% at 25 days (defaults; from config).
    let streak_bonus =
        1.0 + (streak_days.min(tuning.streak_bonus_cap_days) as f64 * tuning.streak_bonus_per_day);

    // Novelty bonus for first occurrence (already recorded above, so check if count == 1)
    let novelty = if count == 1 {
        tuning.novelty_factor
    } else {
        1.0
    };

    // Trust tier multiplier (from config).
    let trust_mult = cfg.trust_tier_multipliers.multiplier(trust_tier);

    let effective_multiplier =
        mode_multiplier * streak_bonus * novelty * grind_multiplier * trust_mult;
    let grind_capped = grind_multiplier < 1.0;
    let xp = (base.xp as f64 * effective_multiplier).round() as u64;
    let crystals = (base.crystals as f64 * effective_multiplier).round() as u64;
    let lumens = (base.lumens as f64 * grind_multiplier).round() as i64;

    tracing::debug!(
        "policy: event='{}' base=({},{},{}) mode={:.2} streak={:.2} novelty={:.2} grind={:.2} trust={:.2} → xp={} crystals={} lumens={}",
        event_type,
        base.xp,
        base.crystals,
        base.lumens,
        mode_multiplier,
        streak_bonus,
        novelty,
        grind_multiplier,
        trust_mult,
        xp,
        crystals,
        lumens
    );

    PolicyReward {
        xp,
        crystals,
        lumens,
        grant_shield: base.grant_shield,
        effective_multiplier,
        grind_capped,
    }
}

// ── Event config overrides ────────────────────────────────

/// Runtime override table: event_type → (xp_override, crystals_override).
/// Populated from the DB via `event_config` table. Takes precedence over `base_reward`.
#[derive(Debug, Default, Clone)]
pub struct EventConfigOverrides {
    /// Map from event type slug to override base reward.
    pub overrides: HashMap<String, BaseReward>,
}

impl EventConfigOverrides {
    /// Insert or replace an override for an event type.
    pub fn set(&mut self, event_type: impl Into<String>, xp: u64, crystals: u64) {
        self.overrides
            .insert(event_type.into(), BaseReward::new(xp, crystals));
    }

    /// Resolve the effective base reward, applying runtime overrides above policy base.
    pub fn resolve(&self, event_type: &str) -> BaseReward {
        self.overrides
            .get(event_type)
            .cloned()
            .unwrap_or_else(|| base_reward(event_type))
    }
}

/// Apply the full policy stack with optional runtime overrides.
pub fn apply_policy_with_overrides(
    overrides: &EventConfigOverrides,
    mode_multiplier: f64,
    streak_days: u32,
    event_type: &str,
    session: &mut SessionState,
) -> PolicyReward {
    let base = overrides.resolve(event_type);
    apply_policy(
        &base,
        mode_multiplier,
        streak_days,
        crate::profile::TrustTier::Linked,
        event_type,
        session,
    )
}

// ── Diagnostics ───────────────────────────────────────────

/// Result of routing an event, containing the policy reward and any level-up context.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RouteResult {
    /// The reward granted for the event (if any).
    pub reward: Option<PolicyReward>,
    /// Whether the user leveled up from this event: (new_level, title).
    pub leveled_up: Option<(u64, String)>,
}

/// Diagnostic snapshot of effective rewards for a given event under current policy.
#[derive(Debug, serde::Serialize)]
pub struct RewardDiagnostic {
    /// Event type slug being diagnosed.
    pub event_type: String,
    /// Base XP from policy table (before overrides).
    pub base_xp: u64,
    /// Base crystals from policy table (before overrides).
    pub base_crystals: u64,
    /// Whether a runtime override was applied.
    pub override_applied: bool,
    /// Active mode multiplier.
    pub mode_multiplier: f64,
    /// Combined multiplier including streak and novelty.
    pub effective_multiplier: f64,
    /// Streak days used for calculation.
    pub streak_days: u32,
    /// Estimated XP output under current policy.
    pub estimated_xp: u64,
    /// Estimated crystals output under current policy.
    pub estimated_crystals: u64,
}

/// Build a diagnostic snapshot without mutating session state.
pub fn diagnostic(
    overrides: &EventConfigOverrides,
    mode_multiplier: f64,
    streak_days: u32,
    event_type: &str,
) -> RewardDiagnostic {
    let base = base_reward(event_type);
    let override_applied = overrides.overrides.contains_key(event_type);
    let resolved = overrides.resolve(event_type);

    let streak_bonus = 1.0 + (streak_days.min(25) as f64 * 0.02);
    // Diagnostic assumes NOVELTY_FACTOR not applied for repeat estimations
    let effective_multiplier = mode_multiplier * streak_bonus;
    let estimated_xp = (resolved.xp as f64 * effective_multiplier).round() as u64;
    let estimated_crystals = (resolved.crystals as f64 * effective_multiplier).round() as u64;

    RewardDiagnostic {
        event_type: event_type.to_string(),
        base_xp: base.xp,
        base_crystals: base.crystals,
        override_applied,
        mode_multiplier,
        effective_multiplier,
        streak_days,
        estimated_xp,
        estimated_crystals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_completed_base() {
        let b = base_reward("task_completed");
        assert_eq!(b.xp, 50);
        assert_eq!(b.crystals, 5);
    }

    #[test]
    fn mode_multiplier_scales_reward() {
        let base = BaseReward::new(10, 2);
        let mut session = SessionState::default();
        let r = apply_policy(
            &base,
            1.5,
            0,
            crate::profile::TrustTier::Linked,
            "task_completed",
            &mut session,
        );
        assert!(r.xp > 10); // novelty + mode
        assert!(!r.grind_capped);
    }

    #[test]
    fn grind_cap_zeros_reward() {
        let base = BaseReward::new(10, 2);
        let mut session = SessionState::default();
        for _ in 0..=GRIND_ZERO_THRESHOLD {
            let _ = apply_policy(
                &base,
                1.0,
                0,
                crate::profile::TrustTier::Linked,
                "task_completed",
                &mut session,
            );
        }
        let r = apply_policy(
            &base,
            1.0,
            0,
            crate::profile::TrustTier::Linked,
            "task_completed",
            &mut session,
        );
        assert!(r.grind_capped);
        assert_eq!(r.xp, 0);
    }

    #[test]
    fn novelty_bonus_only_on_first() {
        let base = BaseReward::new(10, 0);
        let mut session = SessionState::default();
        let first = apply_policy(
            &base,
            1.0,
            0,
            crate::profile::TrustTier::Linked,
            "unique_event",
            &mut session,
        );
        let second = apply_policy(
            &base,
            1.0,
            0,
            crate::profile::TrustTier::Linked,
            "unique_event",
            &mut session,
        );
        assert!(first.xp > second.xp, "first should have novelty bonus");
    }

    #[test]
    fn streak_adds_bonus() {
        let base = BaseReward::new(10, 0);
        let mut s1 = SessionState::default();
        let mut s2 = SessionState::default();
        let no_streak = apply_policy(
            &base,
            1.0,
            0,
            crate::profile::TrustTier::Linked,
            "task_completed",
            &mut s1,
        );
        let with_streak = apply_policy(
            &base,
            1.0,
            25,
            crate::profile::TrustTier::Linked,
            "task_completed",
            &mut s2,
        );
        assert!(with_streak.xp >= no_streak.xp);
    }

    #[test]
    fn bug_fix_rewarded_more_than_task_completed() {
        let bug = base_reward("bug_fix");
        let task = base_reward("task_completed");
        assert!(bug.xp > task.xp, "bug fixes should earn more XP");
    }

    #[test]
    fn lumen_tapering_works() {
        let base = BaseReward::with_lumens(100, 20, 10);
        let mut session = SessionState::default();

        // Firing many times to trigger grind multiplier < 1.0
        // mcp_tool_called has (8, 14) thresholds
        for _ in 0..10 {
            let _ = apply_policy(
                &base,
                1.0,
                0,
                crate::profile::TrustTier::Linked,
                "mcp_tool_called",
                &mut session,
            );
        }

        let r = apply_policy(
            &base,
            1.0,
            0,
            crate::profile::TrustTier::Linked,
            "mcp_tool_called",
            &mut session,
        );
        assert!(r.grind_capped);
        assert!(r.lumens < 10, "Lumens should be tapered down from 10");
    }

    #[test]
    fn resolve_returns_override_when_present() {
        let mut o = EventConfigOverrides::default();
        o.set("task_completed", 999, 42);
        let r = o.resolve("task_completed");
        assert_eq!(r.xp, 999);
        assert_eq!(r.crystals, 42);
    }

    #[test]
    fn apply_policy_cfg_default_matches_legacy_apply_policy() {
        // Behavior preservation: the config-aware path with the DEFAULT economy
        // config produces byte-identical rewards to the legacy `apply_policy`.
        let cfg = crate::economy::EconomyConfig::default();
        let base = base_reward("bug_fix");
        for streak in [0u32, 7, 25, 40] {
            for ev in ["bug_fix", "mcp_tool_called", "task_completed"] {
                let mut s_legacy = SessionState::default();
                let mut s_cfg = SessionState::default();
                let legacy = apply_policy(
                    &base,
                    1.5,
                    streak,
                    crate::profile::TrustTier::Proven,
                    ev,
                    &mut s_legacy,
                );
                let via_cfg = apply_policy_cfg(
                    &cfg,
                    &base,
                    1.5,
                    streak,
                    crate::profile::TrustTier::Proven,
                    ev,
                    &mut s_cfg,
                );
                assert_eq!(
                    (
                        legacy.xp,
                        legacy.crystals,
                        legacy.lumens,
                        legacy.grant_shield
                    ),
                    (
                        via_cfg.xp,
                        via_cfg.crystals,
                        via_cfg.lumens,
                        via_cfg.grant_shield
                    ),
                    "default-config path diverged for event '{ev}' streak {streak}"
                );
            }
        }
    }

    #[test]
    fn apply_policy_cfg_sources_tuning_from_config() {
        // LIVE-path proof: an override in the EconomyConfig flows through
        // apply_policy_cfg to a changed reward. Here we bump the novelty factor
        // and trust multiplier, so the first-occurrence reward must increase.
        let mut cfg = crate::economy::EconomyConfig::default();
        let base = base_reward("task_completed"); // (50, 5)

        let mut s_base = SessionState::default();
        let baseline = apply_policy_cfg(
            &cfg,
            &base,
            1.0,
            0,
            crate::profile::TrustTier::Linked,
            "task_completed",
            &mut s_base,
        );

        cfg.tuning.novelty_factor = 3.0; // was 1.5
        cfg.trust_tier_multipliers.linked = 2.0; // was 1.0
        let mut s_over = SessionState::default();
        let boosted = apply_policy_cfg(
            &cfg,
            &base,
            1.0,
            0,
            crate::profile::TrustTier::Linked,
            "task_completed",
            &mut s_over,
        );

        assert!(
            boosted.xp > baseline.xp,
            "config override should raise the live reward: {} !> {}",
            boosted.xp,
            baseline.xp
        );
    }

    #[test]
    fn apply_policy_cfg_grind_caps_from_config() {
        // Tighten the default-event grind cap to 1 occurrence; the second fire
        // must taper. Proves grind caps are sourced from the config.
        let mut cfg = crate::economy::EconomyConfig::default();
        cfg.tuning.grind_caps_default = (1, 1);
        cfg.tuning.grind_taper_end = 1; // anything past half_cap zeroes
        let base = BaseReward::new(100, 10);
        let mut session = SessionState::default();
        let first = apply_policy_cfg(
            &cfg,
            &base,
            1.0,
            0,
            crate::profile::TrustTier::Linked,
            "novel_grind_event",
            &mut session,
        );
        assert!(!first.grind_capped);
        let second = apply_policy_cfg(
            &cfg,
            &base,
            1.0,
            0,
            crate::profile::TrustTier::Linked,
            "novel_grind_event",
            &mut session,
        );
        assert!(second.grind_capped, "second fire should be grind-capped");
    }

    #[test]
    fn resolve_falls_back_to_policy_base_when_absent() {
        let o = EventConfigOverrides::default();
        let r = o.resolve("task_completed");
        let base = base_reward("task_completed");
        // BaseReward has no PartialEq; compare observable fields.
        assert_eq!(
            (r.xp, r.crystals, r.lumens, r.grant_shield),
            (base.xp, base.crystals, base.lumens, base.grant_shield),
        );
    }
}
